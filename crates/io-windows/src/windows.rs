use std::collections::BTreeSet;
use std::ffi::c_void;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::mem::{offset_of, size_of};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Component, Path, PathBuf, Prefix};
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use std::time::Duration;

use um_core::{SectorLayout, SourceIdentity, SourceKind};
use windows_sys::Win32::Foundation::{
    LocalFree, ERROR_CANCELLED, ERROR_PIPE_CONNECTED, ERROR_PIPE_LISTENING, GENERIC_READ,
    GENERIC_WRITE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::Storage::FileSystem::{
    BusTypeAta, BusTypeNvme, BusTypeSata, BusTypeUsb, CreateFileW, GetDiskFreeSpaceExW,
    GetDiskFreeSpaceW, GetDriveTypeW, GetFileInformationByHandle, GetFinalPathNameByHandleW,
    GetLogicalDrives, GetVolumeInformationByHandleW, GetVolumeInformationW,
    GetVolumeNameForVolumeMountPointW, ReadFile, SetFilePointerEx, BY_HANDLE_FILE_INFORMATION,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT, FILE_BEGIN,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_LIST_DIRECTORY, FILE_NAME_NORMALIZED, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, SYNCHRONIZE,
    VOLUME_NAME_GUID,
};
use windows_sys::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageAccessAlignmentProperty, StorageDeviceProperty, DISK_EXTENT,
    GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO, IOCTL_STORAGE_QUERY_PROPERTY,
    STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR, STORAGE_DESCRIPTOR_HEADER, STORAGE_DEVICE_DESCRIPTOR,
    STORAGE_PROPERTY_QUERY, VOLUME_DISK_EXTENTS,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, GetNamedPipeServerProcessId,
    SetNamedPipeHandleState, WaitNamedPipeW, PIPE_NOWAIT, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetProcessId, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

use crate::model::{
    build_inventory_from_query_results, decode_ntfs_file_reference, derive_folder_scope_id,
    derive_volume_id, drive_scan_policy, open_destination_root_with, parse_folder_identity_path,
    plan_aligned_read, DestinationRootEvidence, DestinationRootInformation, DestinationRootQuery,
    DestinationVolumeInformation, DiskExtent, DriveKind, NativeVolumeSnapshot, PhysicalBacking,
    ScanPolicy,
};
use crate::transport_config::{
    broker_executable_from_current, build_broker_arguments, build_pipe_name, build_pipe_sddl,
    desktop_executable_from_broker,
};
use crate::{
    DestinationError, FolderScope, FolderScopeError, LocationError, StorageError, StorageInventory,
    StorageLocation, UnsupportedReason,
};

const DRIVE_UNKNOWN: u32 = 0;
const DRIVE_NO_ROOT_DIR: u32 = 1;
const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;
const DRIVE_CDROM: u32 = 5;
const DRIVE_RAMDISK: u32 = 6;
const MAX_VOLUME_NAME_UNITS: usize = 261;
const MAX_VOLUME_EXTENTS: usize = 128;
const MAX_STORAGE_DEVICE_DESCRIPTOR_BYTES: usize = 64 * 1024;
const INITIAL_EXTENT_BUFFER_BYTES: usize = 1024;
const MAX_EXTENT_BUFFER_BYTES: usize = 64 * 1024;
const MAX_PROCESS_IMAGE_UNITS: usize = 32_768;
// Extended-length Windows paths are bounded to 32,767 UTF-16 code units.
// Reserve one additional unit only within this reviewed allocation ceiling.
const MAX_FINAL_GUID_PATH_UTF16_UNITS: usize = 32_768;

pub(super) fn classify_path(path: &Path) -> Result<StorageLocation, LocationError> {
    let drive_letter = drive_letter(path).map_err(|_| LocationError::UnsupportedRoot)?;
    let root = drive_root_wide(drive_letter);
    // SAFETY: `root` is a live fixed-size NUL-terminated UTF-16 drive root.
    // GetDriveTypeW reads the buffer and does not mutate storage.
    let drive_type = unsafe { GetDriveTypeW(root.as_ptr()) };
    classify_drive_type(drive_type)
}

fn classify_drive_type(drive_type: u32) -> Result<StorageLocation, LocationError> {
    match drive_type {
        DRIVE_REMOTE => Ok(StorageLocation::Remote),
        DRIVE_REMOVABLE | DRIVE_FIXED | DRIVE_CDROM | DRIVE_RAMDISK => Ok(StorageLocation::Local),
        DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR | 7..=u32::MAX => {
            Err(LocationError::DriveTypeUnavailable)
        }
    }
}

pub(super) fn enumerate_storage() -> Result<StorageInventory, StorageError> {
    build_inventory_from_query_results(enumerate_mounted_volume_results()?)
}

pub(super) fn validate_folder_scope(
    volume_id: &str,
    folder: &Path,
) -> Result<FolderScope, StorageError> {
    let drive_letter = drive_letter(folder)?;
    let root = drive_root_wide(drive_letter);
    // SAFETY: `root` is a live fixed-size NUL-terminated UTF-16 drive root.
    // GetDriveTypeW only classifies the root.
    let drive_type = unsafe { GetDriveTypeW(root.as_ptr()) };
    if !matches!(
        drive_kind(drive_type),
        DriveKind::Fixed | DriveKind::Removable
    ) {
        return Err(FolderScopeError::NonLocal.into());
    }
    reject_reparse_ancestry(folder)?;

    let before = enumerate_native_volumes()?;
    let candidates = before
        .iter()
        .filter(|snapshot| derive_volume_id(snapshot) == volume_id)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(StorageError::SourceIdentityUnavailable);
    }
    if candidates
        .iter()
        .all(|snapshot| !matches!(snapshot.drive_kind, DriveKind::Fixed | DriveKind::Removable))
    {
        return Err(StorageError::UnsupportedSource(
            UnsupportedReason::NonLocalDrive,
        ));
    }
    if candidates
        .iter()
        .all(|snapshot| !snapshot.file_system.eq_ignore_ascii_case("NTFS"))
    {
        return Err(StorageError::UnsupportedSource(
            crate::UnsupportedReason::UnsupportedFileSystem,
        ));
    }

    let handle = open_folder_attributes(folder)?;
    let information = query_file_information(&handle)?;
    if information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(FolderScopeError::NotDirectory.into());
    }
    if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(FolderScopeError::ReparsePoint.into());
    }

    let final_path = query_final_guid_path(&handle)?;
    let selected = candidates
        .into_iter()
        .find(|snapshot| {
            parse_folder_identity_path(&final_path, &snapshot.volume_guid).is_ok()
                && snapshot.volume_serial == information.dwVolumeSerialNumber
        })
        .ok_or(FolderScopeError::CrossVolume)?;
    let parsed = parse_folder_identity_path(&final_path, &selected.volume_guid)?;
    let file_index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    let (ntfs_record, ntfs_sequence) = decode_ntfs_file_reference(file_index);

    let after = enumerate_native_volumes()?;
    let still_present = after.iter().any(|snapshot| {
        derive_volume_id(snapshot) == volume_id
            && snapshot
                .volume_guid
                .eq_ignore_ascii_case(&selected.volume_guid)
            && snapshot.volume_serial == information.dwVolumeSerialNumber
    });
    if !still_present {
        return Err(StorageError::SourceIdentityChanged);
    }

    Ok(FolderScope {
        scope_id: derive_folder_scope_id(
            volume_id,
            information.dwVolumeSerialNumber,
            file_index,
            &parsed.relative_components,
        ),
        volume_id: volume_id.to_owned(),
        display_label: parsed.display_label,
        relative_components: parsed.relative_components,
        volume_serial: information.dwVolumeSerialNumber,
        file_index,
        ntfs_record,
        ntfs_sequence,
    })
}

pub(super) fn open_destination_root_binding(
    destination_root: &Path,
) -> Result<DestinationRootBindingInner, StorageError> {
    let mut query = WindowsDestinationRootQuery;
    let evidence = open_destination_root_with(destination_root, &mut query)?;
    let DestinationRootEvidence {
        root_handle,
        final_volume_guid,
        display_label,
        volume_label,
        file_system,
        free_bytes,
        physical_disks,
        physical_backing,
        reparse_safe,
        volume_serial,
        file_index,
    } = evidence;
    let physical_disk_number = physical_disks.single()?;
    Ok(DestinationRootBindingInner {
        _root_handle: File::from(root_handle),
        _final_volume_guid: final_volume_guid,
        display_label,
        volume_label,
        file_system,
        free_bytes,
        physical_disk_number,
        reparse_safe,
        _physical_backing: physical_backing,
        _volume_serial: volume_serial,
        _file_index: file_index,
    })
}

pub(super) struct DestinationRootBindingInner {
    _root_handle: File,
    _final_volume_guid: String,
    display_label: String,
    volume_label: String,
    file_system: String,
    free_bytes: u64,
    physical_disk_number: u32,
    reparse_safe: bool,
    _physical_backing: PhysicalBacking,
    _volume_serial: u32,
    _file_index: u64,
}

impl DestinationRootBindingInner {
    pub(super) fn display_label(&self) -> &str {
        &self.display_label
    }

    pub(super) fn volume_label(&self) -> &str {
        &self.volume_label
    }

    pub(super) fn file_system(&self) -> &str {
        &self.file_system
    }

    pub(super) fn free_bytes(&self) -> u64 {
        self.free_bytes
    }

    pub(super) fn physical_disk_number(&self) -> u32 {
        self.physical_disk_number
    }

    pub(super) fn reparse_safe(&self) -> bool {
        self.reparse_safe
    }

    pub(super) fn into_directory_file(self) -> File {
        self._root_handle
    }
}

struct WindowsDestinationRootQuery;

impl DestinationRootQuery for WindowsDestinationRootQuery {
    type RootHandle = OwnedHandle;

    fn classify_root(&mut self, path: &Path) -> Result<(), DestinationError> {
        let letter = drive_letter(path).map_err(|_| DestinationError::UnsupportedRoot)?;
        let root = drive_root_wide(letter);
        // SAFETY: `root` is a live fixed-size NUL-terminated UTF-16 drive
        // root. GetDriveTypeW performs only a locality/classification query.
        match drive_kind(unsafe { GetDriveTypeW(root.as_ptr()) }) {
            DriveKind::Fixed | DriveKind::Removable => Ok(()),
            DriveKind::Remote => Err(DestinationError::NonLocal),
            DriveKind::Unknown | DriveKind::NoRoot | DriveKind::CdRom | DriveKind::RamDisk => {
                Err(DestinationError::UnsupportedRoot)
            }
        }
    }

    fn open_root(&mut self, path: &Path) -> Result<Self::RootHandle, DestinationError> {
        open_destination_root_handle(path).map_err(|_| DestinationError::IdentityUnavailable)
    }

    fn query_root(
        &mut self,
        handle: &Self::RootHandle,
    ) -> Result<DestinationRootInformation, DestinationError> {
        let information =
            query_file_information(handle).map_err(|_| DestinationError::IdentityUnavailable)?;
        Ok(DestinationRootInformation {
            is_directory: information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
            is_reparse_point: information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0,
            volume_serial: information.dwVolumeSerialNumber,
            file_index: (u64::from(information.nFileIndexHigh) << 32)
                | u64::from(information.nFileIndexLow),
        })
    }

    fn query_final_path(&mut self, handle: &Self::RootHandle) -> Result<String, DestinationError> {
        query_final_guid_path(handle).map_err(|_| DestinationError::IdentityUnavailable)
    }

    fn query_volume(
        &mut self,
        volume_guid: &str,
    ) -> Result<DestinationVolumeInformation, DestinationError> {
        query_destination_volume(volume_guid).map_err(|_| DestinationError::IdentityUnavailable)
    }
}

pub(super) fn open_raw_volume(volume_id: &str) -> Result<RawVolumeInner, StorageError> {
    let before = enumerate_native_volumes()?;
    let selected = before
        .into_iter()
        .find(|snapshot| derive_volume_id(snapshot) == volume_id)
        .ok_or(StorageError::SourceIdentityUnavailable)?;
    if !matches!(selected.drive_kind, DriveKind::Fixed | DriveKind::Removable) {
        return Err(StorageError::UnsupportedSource(
            UnsupportedReason::NonLocalDrive,
        ));
    }

    let handle = open_volume_for_read(&selected)?;
    let handle_serial = query_handle_volume_serial(&handle)?;
    validate_handle_volume_serial(selected.volume_serial, handle_serial)?;
    let storage_bus_type = query_storage_bus_type(&handle)?;
    if physical_backing_from_bus_type(storage_bus_type) != PhysicalBacking::Direct {
        return Err(StorageError::UnsupportedSource(
            UnsupportedReason::UnprovenPhysicalBacking,
        ));
    }
    let extents = query_volume_disk_extents(&handle)?;
    let disk_numbers = distinct_disk_numbers(&extents);
    if !matches!(
        drive_scan_policy(selected.drive_kind, &disk_numbers),
        ScanPolicy::Supported
    ) {
        return Err(StorageError::UnsupportedSource(
            authoritative_policy_reason(selected.drive_kind, &disk_numbers),
        ));
    }
    let physical_disk_number = disk_numbers
        .first()
        .copied()
        .ok_or(StorageError::InvalidGeometry)?;
    let length = query_handle_length(&handle)?;
    let layout = query_handle_sector_layout(&handle)?;
    if length == 0 || length > i64::MAX as u64 || length % u64::from(layout.logical) != 0 {
        return Err(StorageError::InvalidGeometry);
    }

    let after = enumerate_native_volumes()?;
    let identity_stable = after.iter().any(|snapshot| {
        derive_volume_id(snapshot) == volume_id
            && snapshot
                .volume_guid
                .eq_ignore_ascii_case(&selected.volume_guid)
    });
    if !identity_stable {
        return Err(StorageError::SourceIdentityChanged);
    }

    let label = if selected.label.is_empty() {
        selected.mount_label.clone()
    } else {
        format!("{} {}", selected.mount_label, selected.label)
    };
    Ok(RawVolumeInner {
        handle: Mutex::new(handle),
        identity: SourceIdentity {
            id: volume_id.to_owned(),
            kind: SourceKind::Volume,
            label,
            size: length,
        },
        length,
        layout,
        physical_disk_number,
        expected_volume_id: volume_id.to_owned(),
        expected_volume_guid: selected.volume_guid,
        expected_volume_serial: selected.volume_serial,
        expected_storage_bus_type: storage_bus_type,
        expected_extents: extents,
    })
}

pub(super) struct RawVolumeInner {
    handle: Mutex<OwnedHandle>,
    identity: SourceIdentity,
    length: u64,
    layout: SectorLayout,
    physical_disk_number: u32,
    expected_volume_id: String,
    expected_volume_guid: String,
    expected_volume_serial: u32,
    expected_storage_bus_type: i32,
    expected_extents: Vec<DiskExtent>,
}

impl RawVolumeInner {
    pub(super) fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub(super) fn len(&self) -> u64 {
        self.length
    }

    pub(super) fn sector_layout(&self) -> SectorLayout {
        self.layout
    }

    pub(super) fn physical_disk_number(&self) -> u32 {
        self.physical_disk_number
    }

    pub(super) fn revalidate_identity(&self) -> Result<(), StorageError> {
        let snapshots = enumerate_native_volumes()?;
        if !snapshots.iter().any(|snapshot| {
            derive_volume_id(snapshot) == self.expected_volume_id
                && snapshot
                    .volume_guid
                    .eq_ignore_ascii_case(&self.expected_volume_guid)
        }) {
            return Err(StorageError::SourceIdentityChanged);
        }

        let guard = self
            .handle
            .lock()
            .map_err(|_| StorageError::Synchronization)?;
        let handle_serial = query_handle_volume_serial(&guard)?;
        validate_handle_volume_serial(self.expected_volume_serial, handle_serial)?;
        let storage_bus_type = query_storage_bus_type(&guard)?;
        let extents = query_volume_disk_extents(&guard)?;
        let length = query_handle_length(&guard)?;
        let layout = query_handle_sector_layout(&guard)?;
        if storage_bus_type != self.expected_storage_bus_type
            || physical_backing_from_bus_type(storage_bus_type) != PhysicalBacking::Direct
            || extents != self.expected_extents
            || length != self.length
            || layout != self.layout
        {
            return Err(StorageError::SourceIdentityChanged);
        }
        Ok(())
    }

    pub(super) fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), StorageError> {
        let plan = plan_aligned_read(offset, buffer.len(), self.length, self.layout.logical)?;
        if plan.copy_length == 0 {
            return Ok(());
        }

        let mut aligned = vec![0u8; plan.aligned_length];
        let guard = self
            .handle
            .lock()
            .map_err(|_| StorageError::Synchronization)?;
        let raw_handle = guard.as_raw_handle();
        let file_offset =
            i64::try_from(plan.aligned_offset).map_err(|_| StorageError::InvalidGeometry)?;
        // SAFETY: `raw_handle` is a live read-only volume handle owned by `guard`.
        // The new pointer output is unused, and FILE_BEGIN with a validated i64
        // offset changes only this handle's read cursor.
        let seek_ok = unsafe { SetFilePointerEx(raw_handle, file_offset, null_mut(), FILE_BEGIN) };
        if seek_ok == 0 {
            return Err(last_windows_error("SetFilePointerEx"));
        }

        let mut completed = 0usize;
        while completed < aligned.len() {
            let chunk = (aligned.len() - completed).min(u32::MAX as usize);
            let mut bytes_read = 0u32;
            // SAFETY: `raw_handle` remains live under `guard`; the destination
            // points to `chunk` writable bytes within `aligned`; bytes_read is a
            // live u32; the null OVERLAPPED pointer requests synchronous access.
            let read_ok = unsafe {
                ReadFile(
                    raw_handle,
                    aligned[completed..].as_mut_ptr(),
                    chunk as u32,
                    &mut bytes_read,
                    null_mut(),
                )
            };
            if read_ok == 0 {
                return Err(last_windows_error("ReadFile"));
            }
            if bytes_read == 0 {
                return Err(StorageError::ShortRead);
            }
            completed = completed
                .checked_add(bytes_read as usize)
                .ok_or(StorageError::ReadPlan(crate::ReadPlanError::Overflow))?;
        }

        let copy_end = plan
            .copy_offset
            .checked_add(plan.copy_length)
            .ok_or(StorageError::ReadPlan(crate::ReadPlanError::Overflow))?;
        buffer.copy_from_slice(&aligned[plan.copy_offset..copy_end]);
        Ok(())
    }
}

pub(super) fn create_named_pipe_server(
    pipe_suffix: &str,
) -> Result<NamedPipeServerInner, StorageError> {
    let pipe_name = build_pipe_name(pipe_suffix)?;
    let wide_name = wide_string(OsStr::new(&pipe_name));
    let security_descriptor = current_user_pipe_security_descriptor()?;
    let security_attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security_descriptor.0,
        bInheritHandle: 0,
    };
    // SAFETY: `wide_name` is a live NUL-terminated internal pipe name;
    // open/mode flags are fixed to one duplex byte-stream instance with remote
    // clients rejected; buffer sizes are bounded; security_attributes points
    // to a live current-user descriptor for the duration of this call.
    let handle = unsafe {
        CreateNamedPipeW(
            wide_name.as_ptr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            64 * 1024,
            64 * 1024,
            0,
            &security_attributes,
        )
    };
    let handle = owned_handle(handle, "CreateNamedPipeW")?;
    Ok(NamedPipeServerInner { handle })
}

pub(super) fn connect_broker_pipe(
    pipe_suffix: &str,
    expected_server_pid: u32,
) -> Result<ConnectedPipeInner, StorageError> {
    if expected_server_pid == 0 {
        return Err(StorageError::InvalidParentProcess);
    }
    let pipe_name = build_pipe_name(pipe_suffix)?;
    let wide_name = wide_string(OsStr::new(&pipe_name));
    // SAFETY: `wide_name` is a live NUL-terminated internal pipe name and the
    // timeout is a fixed finite wait. WaitNamedPipeW performs no filesystem
    // or storage mutation.
    let wait_ok = unsafe { WaitNamedPipeW(wide_name.as_ptr(), 10_000) };
    if wait_ok == 0 {
        return Err(last_windows_error("WaitNamedPipeW"));
    }
    // SAFETY: `wide_name` is internal and NUL-terminated. Access is fixed to
    // duplex pipe I/O, share mode is zero, OPEN_EXISTING cannot create a file,
    // and the name can only target the validated `\\.\pipe\` namespace.
    let handle = unsafe {
        CreateFileW(
            wide_name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            null(),
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    let handle = owned_handle(handle, "CreateFileW(named-pipe)")?;
    let observed_server_pid = query_named_pipe_server_pid(&handle)?;
    if observed_server_pid != expected_server_pid {
        return Err(StorageError::UnexpectedPeerProcess {
            expected: expected_server_pid,
            observed: observed_server_pid,
        });
    }
    connected_pipe(handle, observed_server_pid)
}

pub(super) struct NamedPipeServerInner {
    handle: OwnedHandle,
}

impl NamedPipeServerInner {
    pub(super) fn accept_with_timeout(
        self,
        timeout: Duration,
        broker: Option<&ElevatedBrokerProcessInner>,
    ) -> Result<ConnectedPipeInner, StorageError> {
        if timeout.is_zero() || timeout > Duration::from_secs(10) {
            return Err(StorageError::InvalidPipeTimeout);
        }
        let started = std::time::Instant::now();
        loop {
            // SAFETY: `self.handle` is a live one-instance PIPE_NOWAIT server
            // pipe. The null OVERLAPPED pointer performs a nonblocking connect
            // poll and no data buffer is read or written.
            let connected = unsafe { ConnectNamedPipe(self.handle.as_raw_handle(), null_mut()) };
            if connected != 0 {
                break;
            }
            let code = std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or_default() as u32;
            if code == ERROR_PIPE_CONNECTED {
                break;
            }
            if code != ERROR_PIPE_LISTENING {
                return Err(StorageError::WindowsApi {
                    operation: "ConnectNamedPipe",
                    code,
                });
            }

            if let Some(broker) = broker {
                if !broker.is_alive()? {
                    return Err(StorageError::BrokerExitedBeforeConnect);
                }
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return Err(StorageError::PipeAcceptTimedOut);
            }
            let remaining = timeout.saturating_sub(elapsed);
            std::thread::sleep(remaining.min(Duration::from_millis(5)));
        }

        let blocking_byte_mode = PIPE_READMODE_BYTE | PIPE_WAIT;
        // SAFETY: the server pipe is connected and live. `blocking_byte_mode`
        // is a live fixed mode value; optional quota/timeout pointers are null.
        // This changes only pipe I/O mode from bounded accept polling to normal
        // blocking byte-stream transport.
        let mode_set = unsafe {
            SetNamedPipeHandleState(
                self.handle.as_raw_handle(),
                &blocking_byte_mode,
                null(),
                null(),
            )
        };
        if mode_set == 0 {
            return Err(last_windows_error("SetNamedPipeHandleState"));
        }
        let client_pid = query_named_pipe_client_pid(&self.handle)?;
        connected_pipe(self.handle, client_pid)
    }
}

pub(super) struct ConnectedPipeInner {
    stream: File,
    peer_process_id: u32,
    peer_process: OwnedHandle,
}

impl ConnectedPipeInner {
    pub(super) fn peer_process_id(&self) -> u32 {
        self.peer_process_id
    }

    pub(super) fn peer_is_alive(&self) -> Result<bool, StorageError> {
        process_is_alive(&self.peer_process)
    }

    pub(super) fn peer_is_packaged_desktop(&self) -> Result<bool, StorageError> {
        let broker_executable =
            std::env::current_exe().map_err(|_| StorageError::BrokerExecutableUnavailable)?;
        let expected = desktop_executable_from_broker(&broker_executable)?
            .canonicalize()
            .map_err(|_| StorageError::BrokerExecutableUnavailable)?;
        let observed = query_process_image_path(&self.peer_process)?
            .canonicalize()
            .map_err(|_| StorageError::BrokerExecutableUnavailable)?;
        Ok(windows_paths_equal(&expected, &observed))
    }
}

impl Read for ConnectedPipeInner {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.stream.read(buffer)
    }
}

impl Write for ConnectedPipeInner {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

pub(super) fn launch_elevated_broker(
    pipe_suffix: &str,
    parent_pid: u32,
) -> Result<ElevatedBrokerProcessInner, StorageError> {
    if parent_pid == 0 || parent_pid != std::process::id() {
        return Err(StorageError::InvalidParentProcess);
    }
    let arguments = build_broker_arguments(pipe_suffix, parent_pid)?;
    let current_executable =
        std::env::current_exe().map_err(|_| StorageError::BrokerExecutableUnavailable)?;
    let broker_candidate = broker_executable_from_current(&current_executable)?;
    if !broker_candidate.is_file() {
        return Err(StorageError::BrokerExecutableUnavailable);
    }
    let broker_executable = broker_candidate
        .canonicalize()
        .map_err(|_| StorageError::BrokerExecutableUnavailable)?;
    let package_directory = current_executable
        .parent()
        .ok_or(StorageError::BrokerExecutableUnavailable)?
        .canonicalize()
        .map_err(|_| StorageError::BrokerExecutableUnavailable)?;
    if broker_executable.parent() != Some(package_directory.as_path())
        || !broker_executable
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("undelete-master-broker.exe"))
            .unwrap_or(false)
    {
        return Err(StorageError::BrokerExecutableUnavailable);
    }

    let verb = wide_string(OsStr::new("runas"));
    let executable = wide_string(broker_executable.as_os_str());
    let parameters = wide_string(OsStr::new(&arguments));
    let directory = wide_string(package_directory.as_os_str());
    let mut execution = ShellExecuteInfoW {
        cb_size: size_of::<ShellExecuteInfoW>() as u32,
        mask: SEE_MASK_NOCLOSEPROCESS,
        window: null_mut(),
        verb: verb.as_ptr(),
        file: executable.as_ptr(),
        parameters: parameters.as_ptr(),
        directory: directory.as_ptr(),
        show: SW_HIDE,
        instance: null_mut(),
        item_id_list: null_mut(),
        class_name: null(),
        class_key: null_mut(),
        hot_key: 0,
        icon_or_monitor: null_mut(),
        process: null_mut(),
    };
    // SAFETY: every UTF-16 pointer in `execution` references a live
    // NUL-terminated buffer for this call; verb, executable, arguments and
    // directory are fixed or validated above; output process ownership is
    // transferred only after a successful return.
    let launched = unsafe { ShellExecuteExW(&mut execution) };
    if launched == 0 {
        let code = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or_default() as u32;
        return Err(if code == ERROR_CANCELLED {
            StorageError::ElevationCancelled
        } else {
            StorageError::WindowsApi {
                operation: "ShellExecuteExW",
                code,
            }
        });
    }
    let process = owned_handle(execution.process, "ShellExecuteExW(process)")?;
    // SAFETY: `process` is a live process handle returned by ShellExecuteExW.
    let pid = unsafe { GetProcessId(process.as_raw_handle()) };
    if pid == 0 {
        return Err(last_windows_error("GetProcessId"));
    }
    Ok(ElevatedBrokerProcessInner { process, pid })
}

pub(super) struct ElevatedBrokerProcessInner {
    process: OwnedHandle,
    pid: u32,
}

impl ElevatedBrokerProcessInner {
    pub(super) fn pid(&self) -> u32 {
        self.pid
    }

    pub(super) fn is_alive(&self) -> Result<bool, StorageError> {
        process_is_alive(&self.process)
    }
}

fn connected_pipe(
    pipe: OwnedHandle,
    peer_process_id: u32,
) -> Result<ConnectedPipeInner, StorageError> {
    if peer_process_id == 0 {
        return Err(StorageError::UnexpectedPeerProcess {
            expected: 1,
            observed: 0,
        });
    }
    let peer_process = open_process_for_liveness(peer_process_id)?;
    Ok(ConnectedPipeInner {
        stream: File::from(pipe),
        peer_process_id,
        peer_process,
    })
}

fn query_named_pipe_client_pid(handle: &OwnedHandle) -> Result<u32, StorageError> {
    let mut pid = 0u32;
    // SAFETY: `handle` is a connected server pipe and `pid` is a live writable
    // u32. The API only queries peer metadata.
    let ok = unsafe { GetNamedPipeClientProcessId(handle.as_raw_handle(), &mut pid) };
    if ok == 0 {
        return Err(last_windows_error("GetNamedPipeClientProcessId"));
    }
    Ok(pid)
}

fn query_named_pipe_server_pid(handle: &OwnedHandle) -> Result<u32, StorageError> {
    let mut pid = 0u32;
    // SAFETY: `handle` is a connected client pipe and `pid` is a live writable
    // u32. The API only queries peer metadata.
    let ok = unsafe { GetNamedPipeServerProcessId(handle.as_raw_handle(), &mut pid) };
    if ok == 0 {
        return Err(last_windows_error("GetNamedPipeServerProcessId"));
    }
    Ok(pid)
}

fn open_process_for_liveness(process_id: u32) -> Result<OwnedHandle, StorageError> {
    // SAFETY: process_id is an observed nonzero peer PID. Rights are fixed to
    // query-limited plus synchronize; inheritance is disabled. No process
    // memory, token, thread, termination or mutation right is requested.
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE,
            0,
            process_id,
        )
    };
    owned_handle(handle, "OpenProcess(peer-query)")
}

fn process_is_alive(process: &OwnedHandle) -> Result<bool, StorageError> {
    // SAFETY: `process` is a live query/synchronize process handle and a zero
    // timeout is a nonblocking liveness query.
    match unsafe { WaitForSingleObject(process.as_raw_handle(), 0) } {
        WAIT_TIMEOUT => Ok(true),
        WAIT_OBJECT_0 => Ok(false),
        _ => Err(last_windows_error("WaitForSingleObject")),
    }
}

fn query_process_image_path(process: &OwnedHandle) -> Result<PathBuf, StorageError> {
    let mut buffer = vec![0u16; MAX_PROCESS_IMAGE_UNITS];
    let mut length = u32::try_from(buffer.len()).map_err(|_| StorageError::InvalidGeometry)?;
    // SAFETY: `process` is a live query-limited process handle. `buffer` is
    // writable for `length` UTF-16 units, and `length` is a live in/out value.
    // The API only queries the executable image path of the already-bound peer.
    let ok = unsafe {
        QueryFullProcessImageNameW(process.as_raw_handle(), 0, buffer.as_mut_ptr(), &mut length)
    };
    if ok == 0 {
        return Err(last_windows_error("QueryFullProcessImageNameW"));
    }
    let length = usize::try_from(length).map_err(|_| StorageError::InvalidGeometry)?;
    if length == 0 || length > buffer.len() {
        return Err(StorageError::BrokerExecutableUnavailable);
    }
    Ok(PathBuf::from(OsString::from_wide(&buffer[..length])))
}

fn windows_paths_equal(expected: &Path, observed: &Path) -> bool {
    expected
        .as_os_str()
        .encode_wide()
        .map(ascii_lowercase_utf16)
        .eq(observed
            .as_os_str()
            .encode_wide()
            .map(ascii_lowercase_utf16))
}

fn ascii_lowercase_utf16(unit: u16) -> u16 {
    if unit >= u16::from(b'A') && unit <= u16::from(b'Z') {
        unit + u16::from(b'a' - b'A')
    } else {
        unit
    }
}

fn current_user_pipe_security_descriptor() -> Result<LocalAllocation, StorageError> {
    let user_sid = current_user_sid_string()?;
    let sddl = build_pipe_sddl(&user_sid)?;
    let wide_sddl = wide_string(OsStr::new(&sddl));
    let mut descriptor = null_mut();
    // SAFETY: `wide_sddl` is a live NUL-terminated validated SDDL string;
    // `descriptor` is a live output pointer. Windows allocates the returned
    // descriptor with LocalAlloc, and LocalAllocation frees it exactly once.
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            wide_sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    };
    if ok == 0 {
        return Err(last_windows_error(
            "ConvertStringSecurityDescriptorToSecurityDescriptorW",
        ));
    }
    if descriptor.is_null() {
        return Err(StorageError::InvalidUserIdentity);
    }
    Ok(LocalAllocation(descriptor))
}

fn current_user_sid_string() -> Result<String, StorageError> {
    let mut token = null_mut();
    // SAFETY: GetCurrentProcess returns a pseudo-handle valid in this process;
    // `token` is a live output pointer; requested rights are TOKEN_QUERY only.
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
    if opened == 0 {
        return Err(last_windows_error("OpenProcessToken"));
    }
    let token = owned_handle(token, "OpenProcessToken(handle)")?;

    let mut required = 0u32;
    // SAFETY: `token` is live; null/zero is the documented sizing query and
    // `required` is a live output u32.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            null_mut(),
            0,
            &mut required,
        );
    }
    if required < size_of::<TOKEN_USER>() as u32 || required > 64 * 1024 {
        return Err(StorageError::InvalidUserIdentity);
    }
    let mut buffer = vec![0u8; required as usize];
    // SAFETY: `buffer` is writable for `required` bytes; token is live;
    // `required` is also a live output and the information class is TokenUser.
    let queried = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut required,
        )
    };
    if queried == 0 {
        return Err(last_windows_error("GetTokenInformation(TokenUser)"));
    }
    // SAFETY: the successful TokenUser query wrote at least TOKEN_USER bytes
    // into `buffer`. read_unaligned avoids relying on Vec<u8> alignment; the
    // embedded SID pointer remains valid while `buffer` is live below.
    let token_user = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast::<TOKEN_USER>()) };
    if token_user.User.Sid.is_null() {
        return Err(StorageError::InvalidUserIdentity);
    }
    let mut string_sid = null_mut();
    // SAFETY: the SID pointer belongs to the live TokenUser buffer and
    // `string_sid` is a live output pointer. Windows returns a LocalAlloc-owned
    // NUL-terminated string, wrapped immediately for one-time LocalFree.
    let converted = unsafe { ConvertSidToStringSidW(token_user.User.Sid, &mut string_sid) };
    if converted == 0 || string_sid.is_null() {
        return Err(last_windows_error("ConvertSidToStringSidW"));
    }
    let string_sid_allocation = LocalAllocation(string_sid.cast());
    let mut length = 0usize;
    // SAFETY: ConvertSidToStringSidW guarantees a NUL-terminated allocation.
    // A Windows SID string is bounded by MAX_SID_TEXT_LEN; each read remains
    // within that allocation until its guaranteed terminator is observed.
    unsafe {
        while length <= 184 && *string_sid.add(length) != 0 {
            length += 1;
        }
    }
    if length == 0 || length > 184 {
        return Err(StorageError::InvalidUserIdentity);
    }
    // SAFETY: the loop above proved that `length` initialized UTF-16 code
    // units precede the terminator in the live LocalAlloc allocation.
    let units = unsafe { std::slice::from_raw_parts(string_sid, length) };
    let result = String::from_utf16(units).map_err(|_| StorageError::InvalidUserIdentity)?;
    drop(string_sid_allocation);
    Ok(result)
}

struct LocalAllocation(*mut c_void);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: this pointer was returned by a Windows API documented to use
        // LocalAlloc and is owned uniquely by this guard. LocalFree consumes it
        // once; null is never stored.
        unsafe {
            LocalFree(self.0);
        }
    }
}

const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0000_0040;
const SW_HIDE: i32 = 0;

#[repr(C)]
struct ShellExecuteInfoW {
    cb_size: u32,
    mask: u32,
    window: *mut c_void,
    verb: *const u16,
    file: *const u16,
    parameters: *const u16,
    directory: *const u16,
    show: i32,
    instance: *mut c_void,
    item_id_list: *mut c_void,
    class_name: *const u16,
    class_key: *mut c_void,
    hot_key: u32,
    icon_or_monitor: *mut c_void,
    process: *mut c_void,
}

#[link(name = "shell32")]
// SAFETY: declaration only. The sole call site supplies the exact repr(C)
// structure above and documents every pointer lifetime and fixed argument.
unsafe extern "system" {
    fn ShellExecuteExW(execution: *mut ShellExecuteInfoW) -> i32;
}

fn enumerate_native_volumes() -> Result<Vec<NativeVolumeSnapshot>, StorageError> {
    Ok(enumerate_mounted_volume_results()?
        .into_iter()
        .filter_map(Result::ok)
        .collect())
}

fn enumerate_mounted_volume_results(
) -> Result<Vec<Result<NativeVolumeSnapshot, StorageError>>, StorageError> {
    // SAFETY: GetLogicalDrives takes no pointers and performs a query only.
    let drive_mask = unsafe { GetLogicalDrives() };
    if drive_mask == 0 {
        return Err(last_windows_error("GetLogicalDrives"));
    }

    let mut snapshots = Vec::new();
    for index in 0u8..26 {
        if drive_mask & (1u32 << index) == 0 {
            continue;
        }
        let drive_letter = b'A' + index;
        let root = drive_root_wide(drive_letter);
        // SAFETY: `root` is a live fixed-size NUL-terminated UTF-16 drive root.
        let kind = drive_kind(unsafe { GetDriveTypeW(root.as_ptr()) });
        if !matches!(kind, DriveKind::Fixed | DriveKind::Removable) {
            continue;
        }
        snapshots.push(query_mounted_volume(drive_letter, kind));
    }
    Ok(snapshots)
}

fn query_mounted_volume(
    drive_letter: u8,
    drive_kind: DriveKind,
) -> Result<NativeVolumeSnapshot, StorageError> {
    let root = drive_root_wide(drive_letter);
    let mut label = [0u16; MAX_VOLUME_NAME_UNITS];
    let mut file_system = [0u16; MAX_VOLUME_NAME_UNITS];
    let mut volume_serial = 0u32;
    let mut maximum_component_length = 0u32;
    let mut file_system_flags = 0u32;
    // SAFETY: both output arrays are writable for the declared lengths; every
    // scalar output pointer is live; `root` is NUL-terminated and read-only.
    let information_ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            &mut volume_serial,
            &mut maximum_component_length,
            &mut file_system_flags,
            file_system.as_mut_ptr(),
            file_system.len() as u32,
        )
    };
    if information_ok == 0 {
        return Err(last_windows_error("GetVolumeInformationW"));
    }

    let mut total_bytes = 0u64;
    let mut free_bytes = 0u64;
    // SAFETY: `root` is NUL-terminated; both requested output pointers are
    // live u64 values; the optional caller-available output is null.
    let free_space_ok = unsafe {
        GetDiskFreeSpaceExW(root.as_ptr(), null_mut(), &mut total_bytes, &mut free_bytes)
    };
    if free_space_ok == 0 {
        return Err(last_windows_error("GetDiskFreeSpaceExW"));
    }

    let mut sectors_per_cluster = 0u32;
    let mut logical_sector_bytes = 0u32;
    let mut free_clusters = 0u32;
    let mut total_clusters = 0u32;
    // SAFETY: `root` is NUL-terminated and all four outputs are live u32
    // locations. GetDiskFreeSpaceW performs no storage mutation.
    let sector_ok = unsafe {
        GetDiskFreeSpaceW(
            root.as_ptr(),
            &mut sectors_per_cluster,
            &mut logical_sector_bytes,
            &mut free_clusters,
            &mut total_clusters,
        )
    };
    if sector_ok == 0 || logical_sector_bytes == 0 {
        return Err(if sector_ok == 0 {
            last_windows_error("GetDiskFreeSpaceW")
        } else {
            StorageError::InvalidGeometry
        });
    }

    let mut volume_guid = [0u16; MAX_VOLUME_NAME_UNITS];
    // SAFETY: `root` is NUL-terminated; `volume_guid` is writable for the
    // declared number of UTF-16 code units.
    let volume_name_ok = unsafe {
        GetVolumeNameForVolumeMountPointW(
            root.as_ptr(),
            volume_guid.as_mut_ptr(),
            volume_guid.len() as u32,
        )
    };
    if volume_name_ok == 0 {
        return Err(last_windows_error("GetVolumeNameForVolumeMountPointW"));
    }
    let volume_guid = utf16_buffer_to_string(&volume_guid);
    let mount_label = format!("{}:", char::from(drive_letter));
    let system_drive = std::env::var("SystemDrive")
        .ok()
        .map(|value| value.eq_ignore_ascii_case(&mount_label))
        .unwrap_or(false);
    Ok(NativeVolumeSnapshot {
        mount_label,
        drive_kind,
        volume_guid,
        label: utf16_buffer_to_string(&label),
        file_system: utf16_buffer_to_string(&file_system),
        volume_serial,
        total_bytes,
        free_bytes,
        logical_sector_bytes,
        physical_sector_bytes: logical_sector_bytes,
        extents: Vec::new(),
        is_system: system_drive,
    })
}

fn open_volume_for_read(snapshot: &NativeVolumeSnapshot) -> Result<OwnedHandle, StorageError> {
    let selector = volume_selector(&snapshot.volume_guid)?;
    let wide = wide_string(OsStr::new(&selector));
    // SAFETY: `wide` is a live NUL-terminated selector freshly obtained from
    // Windows inventory. Desired access is exactly GENERIC_READ; sharing is
    // read/write/delete; OPEN_EXISTING cannot create or mutate the volume.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    };
    owned_handle(handle, "CreateFileW(volume-read)")
}

fn open_destination_root_handle(destination_root: &Path) -> Result<OwnedHandle, StorageError> {
    let wide = wide_string(destination_root.as_os_str());
    // SAFETY: `wide` is a live NUL-terminated path owned by native Rust after
    // native picker selection. Access is fixed to query/list only; sharing
    // deliberately omits delete so the retained directory cannot be renamed
    // or substituted; OPEN_EXISTING cannot create or mutate; reparse-point
    // opening prevents following a final reparse object.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    owned_handle(handle, "CreateFileW(destination-root-query)")
}

fn open_destination_volume_for_query(volume_guid: &str) -> Result<OwnedHandle, StorageError> {
    let selector = volume_selector(volume_guid)?;
    let wide = wide_string(OsStr::new(&selector));
    // SAFETY: `wide` is a live NUL-terminated volume GUID derived from the
    // retained root handle. Desired access is exactly zero; sharing is
    // read/write/delete; OPEN_EXISTING cannot create or mutate the volume.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    };
    owned_handle(handle, "CreateFileW(destination-volume-query)")
}

fn query_destination_volume(
    volume_guid: &str,
) -> Result<DestinationVolumeInformation, StorageError> {
    let handle = open_destination_volume_for_query(volume_guid)?;
    let (label, file_system, volume_serial) = query_destination_volume_information(&handle)?;
    let physical_backing = physical_backing_from_bus_type(query_storage_bus_type(&handle)?);
    let extents = query_volume_disk_extents(&handle)?;
    let disk_numbers = distinct_disk_numbers(&extents);

    let wide = wide_string(OsStr::new(volume_guid));
    let mut free_bytes = 0u64;
    let mut total_bytes = 0u64;
    // SAFETY: `wide` is the NUL-terminated volume GUID root derived from the
    // retained directory handle. Both outputs are live u64 values; the API is
    // query-only and does not create or mutate destination entries.
    let ok = unsafe {
        GetDiskFreeSpaceExW(wide.as_ptr(), &mut free_bytes, &mut total_bytes, null_mut())
    };
    if ok == 0 {
        return Err(last_windows_error(
            "GetDiskFreeSpaceExW(destination-volume)",
        ));
    }

    Ok(DestinationVolumeInformation {
        label,
        file_system,
        volume_serial,
        total_bytes,
        free_bytes,
        disk_numbers,
        physical_backing,
    })
}

fn query_destination_volume_information(
    handle: &OwnedHandle,
) -> Result<(String, String, u32), StorageError> {
    let mut label = [0u16; MAX_VOLUME_NAME_UNITS];
    let mut file_system = [0u16; MAX_VOLUME_NAME_UNITS];
    let mut volume_serial = 0u32;
    let mut maximum_component_length = 0u32;
    let mut file_system_flags = 0u32;
    // SAFETY: `handle` is the live desired-access-zero handle for the fixed
    // volume GUID derived from the retained root. Both text buffers and all
    // scalar outputs are live for their declared sizes; this API only queries
    // volume metadata.
    let ok = unsafe {
        GetVolumeInformationByHandleW(
            handle.as_raw_handle(),
            label.as_mut_ptr(),
            label.len() as u32,
            &mut volume_serial,
            &mut maximum_component_length,
            &mut file_system_flags,
            file_system.as_mut_ptr(),
            file_system.len() as u32,
        )
    };
    if ok == 0 {
        return Err(last_windows_error(
            "GetVolumeInformationByHandleW(destination-volume)",
        ));
    }
    Ok((
        utf16_buffer_to_string(&label),
        utf16_buffer_to_string(&file_system),
        volume_serial,
    ))
}

fn open_folder_attributes(folder: &Path) -> Result<OwnedHandle, StorageError> {
    let wide = wide_string(folder.as_os_str());
    // SAFETY: `wide` is a live NUL-terminated path owned by native Rust code.
    // Access is limited to FILE_READ_ATTRIBUTES; BACKUP_SEMANTICS permits a
    // directory handle and OPEN_REPARSE_POINT prevents following the final
    // reparse point. OPEN_EXISTING cannot create or mutate.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    owned_handle(handle, "CreateFileW(folder-attributes)")
}

fn query_volume_disk_extents(handle: &OwnedHandle) -> Result<Vec<DiskExtent>, StorageError> {
    let mut buffer_size = INITIAL_EXTENT_BUFFER_BYTES;
    loop {
        let mut buffer = vec![0u8; buffer_size];
        let mut bytes_returned = 0u32;
        // SAFETY: `handle` is a live query-only volume handle. The input is
        // null with zero length as required by this IOCTL; `buffer` is writable
        // for `buffer.len()` bytes; bytes_returned is a live u32; synchronous
        // operation uses a null OVERLAPPED pointer.
        let ok = unsafe {
            DeviceIoControl(
                handle.as_raw_handle(),
                windows_sys::Win32::Storage::FileSystem::IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                null(),
                0,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut bytes_returned,
                null_mut(),
            )
        };
        if ok != 0 {
            return parse_volume_extents(&buffer, bytes_returned as usize);
        }
        let error = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or_default() as u32;
        if error != windows_sys::Win32::Foundation::ERROR_MORE_DATA {
            return Err(StorageError::WindowsApi {
                operation: "IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS",
                code: error,
            });
        }
        buffer_size = buffer_size
            .checked_mul(2)
            .filter(|size| *size <= MAX_EXTENT_BUFFER_BYTES)
            .ok_or(StorageError::InvalidGeometry)?;
    }
}

fn query_handle_volume_serial(handle: &OwnedHandle) -> Result<u32, StorageError> {
    let mut serial = 0u32;
    // SAFETY: `handle` is a live read-only volume handle. All optional text and
    // metadata outputs are null with zero capacities; `serial` is a live u32
    // output for the query-only volume identity call.
    let ok = unsafe {
        GetVolumeInformationByHandleW(
            handle.as_raw_handle(),
            null_mut(),
            0,
            &mut serial,
            null_mut(),
            null_mut(),
            null_mut(),
            0,
        )
    };
    if ok == 0 {
        return Err(last_windows_error("GetVolumeInformationByHandleW"));
    }
    Ok(serial)
}

fn validate_handle_volume_serial(expected: u32, observed: u32) -> Result<(), StorageError> {
    if expected == observed {
        Ok(())
    } else {
        Err(StorageError::SourceIdentityChanged)
    }
}

fn parse_volume_extents(
    buffer: &[u8],
    bytes_returned: usize,
) -> Result<Vec<DiskExtent>, StorageError> {
    let header_size = offset_of!(VOLUME_DISK_EXTENTS, Extents);
    if bytes_returned < header_size || bytes_returned > buffer.len() {
        return Err(StorageError::InvalidGeometry);
    }
    let count_bytes = buffer
        .get(..size_of::<u32>())
        .ok_or(StorageError::InvalidGeometry)?;
    let count = u32::from_le_bytes(
        count_bytes
            .try_into()
            .map_err(|_| StorageError::InvalidGeometry)?,
    ) as usize;
    if count == 0 || count > MAX_VOLUME_EXTENTS {
        return Err(StorageError::InvalidGeometry);
    }
    let extent_bytes = count
        .checked_mul(size_of::<DISK_EXTENT>())
        .ok_or(StorageError::InvalidGeometry)?;
    let required = header_size
        .checked_add(extent_bytes)
        .ok_or(StorageError::InvalidGeometry)?;
    if required > bytes_returned {
        return Err(StorageError::InvalidGeometry);
    }

    let mut extents = Vec::with_capacity(count);
    for index in 0..count {
        let offset = header_size
            .checked_add(
                index
                    .checked_mul(size_of::<DISK_EXTENT>())
                    .ok_or(StorageError::InvalidGeometry)?,
            )
            .ok_or(StorageError::InvalidGeometry)?;
        // SAFETY: `required <= bytes_returned <= buffer.len()` proves that a
        // complete DISK_EXTENT lies at this byte offset. read_unaligned avoids
        // assuming the Vec<u8> allocation has DISK_EXTENT alignment.
        let native =
            unsafe { std::ptr::read_unaligned(buffer.as_ptr().add(offset).cast::<DISK_EXTENT>()) };
        if native.StartingOffset < 0 || native.ExtentLength <= 0 {
            return Err(StorageError::InvalidGeometry);
        }
        extents.push(DiskExtent {
            disk_number: native.DiskNumber,
            starting_offset: native.StartingOffset as u64,
            extent_length: native.ExtentLength as u64,
        });
    }
    Ok(extents)
}

fn query_handle_length(handle: &OwnedHandle) -> Result<u64, StorageError> {
    let mut information = GET_LENGTH_INFORMATION::default();
    let mut bytes_returned = 0u32;
    // SAFETY: `handle` is a live read/query handle; this hard-coded IOCTL has
    // no input; `information` is writable for its exact size; bytes_returned is
    // live; synchronous operation uses a null OVERLAPPED pointer.
    let ok = unsafe {
        DeviceIoControl(
            handle.as_raw_handle(),
            IOCTL_DISK_GET_LENGTH_INFO,
            null(),
            0,
            (&mut information as *mut GET_LENGTH_INFORMATION).cast(),
            size_of::<GET_LENGTH_INFORMATION>() as u32,
            &mut bytes_returned,
            null_mut(),
        )
    };
    if ok == 0 {
        return Err(last_windows_error("IOCTL_DISK_GET_LENGTH_INFO"));
    }
    if bytes_returned < size_of::<GET_LENGTH_INFORMATION>() as u32 || information.Length <= 0 {
        return Err(StorageError::InvalidGeometry);
    }
    Ok(information.Length as u64)
}

fn query_storage_alignment(handle: &OwnedHandle) -> Result<SectorLayout, StorageError> {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageAccessAlignmentProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut descriptor = STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR::default();
    let mut bytes_returned = 0u32;
    // SAFETY: `handle` is live; input points to the exact immutable query
    // structure selecting StorageAccessAlignmentProperty; output points to a
    // writable descriptor; bytes_returned is live; operation is synchronous.
    let ok = unsafe {
        DeviceIoControl(
            handle.as_raw_handle(),
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const STORAGE_PROPERTY_QUERY).cast(),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            (&mut descriptor as *mut STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR).cast(),
            size_of::<STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR>() as u32,
            &mut bytes_returned,
            null_mut(),
        )
    };
    if ok == 0 {
        return Err(last_windows_error(
            "IOCTL_STORAGE_QUERY_PROPERTY(alignment)",
        ));
    }
    if bytes_returned < size_of::<STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR>() as u32 {
        return Err(StorageError::InvalidGeometry);
    }
    SectorLayout::new(
        descriptor.BytesPerLogicalSector,
        descriptor.BytesPerPhysicalSector,
    )
    .ok_or(StorageError::InvalidGeometry)
}

fn physical_backing_from_bus_type(bus_type: i32) -> PhysicalBacking {
    if [BusTypeAta, BusTypeSata, BusTypeUsb, BusTypeNvme].contains(&bus_type) {
        PhysicalBacking::Direct
    } else {
        PhysicalBacking::Unproven
    }
}

fn query_storage_bus_type(handle: &OwnedHandle) -> Result<i32, StorageError> {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };
    let mut header = STORAGE_DESCRIPTOR_HEADER::default();
    let mut bytes_returned = 0u32;
    // SAFETY: `handle` is a live read/query volume handle; input points to the
    // exact immutable StorageDeviceProperty query; output points to a live
    // fixed-size descriptor header; bytes_returned is live; the synchronous
    // query uses a null OVERLAPPED pointer.
    let header_ok = unsafe {
        DeviceIoControl(
            handle.as_raw_handle(),
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const STORAGE_PROPERTY_QUERY).cast(),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            (&mut header as *mut STORAGE_DESCRIPTOR_HEADER).cast(),
            size_of::<STORAGE_DESCRIPTOR_HEADER>() as u32,
            &mut bytes_returned,
            null_mut(),
        )
    };
    if header_ok == 0 {
        return Err(last_windows_error(
            "IOCTL_STORAGE_QUERY_PROPERTY(device-header)",
        ));
    }
    let bus_end = offset_of!(STORAGE_DEVICE_DESCRIPTOR, BusType)
        .checked_add(size_of::<i32>())
        .ok_or(StorageError::InvalidGeometry)?;
    let descriptor_bytes =
        usize::try_from(header.Size).map_err(|_| StorageError::InvalidGeometry)?;
    if bytes_returned < size_of::<STORAGE_DESCRIPTOR_HEADER>() as u32
        || descriptor_bytes < bus_end
        || descriptor_bytes > MAX_STORAGE_DEVICE_DESCRIPTOR_BYTES
    {
        return Err(StorageError::InvalidGeometry);
    }

    let mut descriptor = vec![0u8; descriptor_bytes];
    bytes_returned = 0;
    // SAFETY: `handle` and the immutable fixed property query remain live;
    // `descriptor` is writable for its bounded declared length;
    // bytes_returned is live; the synchronous query uses a null OVERLAPPED
    // pointer and cannot select another property or control code.
    let descriptor_ok = unsafe {
        DeviceIoControl(
            handle.as_raw_handle(),
            IOCTL_STORAGE_QUERY_PROPERTY,
            (&query as *const STORAGE_PROPERTY_QUERY).cast(),
            size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            descriptor.as_mut_ptr().cast(),
            descriptor.len() as u32,
            &mut bytes_returned,
            null_mut(),
        )
    };
    if descriptor_ok == 0 {
        return Err(last_windows_error("IOCTL_STORAGE_QUERY_PROPERTY(device)"));
    }
    parse_storage_bus_type(&descriptor, bytes_returned as usize)
}

fn parse_storage_bus_type(buffer: &[u8], bytes_returned: usize) -> Result<i32, StorageError> {
    let size_offset = offset_of!(STORAGE_DEVICE_DESCRIPTOR, Size);
    let size_end = size_offset
        .checked_add(size_of::<u32>())
        .ok_or(StorageError::InvalidGeometry)?;
    let bus_offset = offset_of!(STORAGE_DEVICE_DESCRIPTOR, BusType);
    let bus_end = bus_offset
        .checked_add(size_of::<i32>())
        .ok_or(StorageError::InvalidGeometry)?;
    if bytes_returned > buffer.len() || bytes_returned < bus_end {
        return Err(StorageError::InvalidGeometry);
    }
    let declared_size = u32::from_ne_bytes(
        buffer
            .get(size_offset..size_end)
            .ok_or(StorageError::InvalidGeometry)?
            .try_into()
            .map_err(|_| StorageError::InvalidGeometry)?,
    ) as usize;
    if declared_size < bus_end || declared_size > bytes_returned {
        return Err(StorageError::InvalidGeometry);
    }
    Ok(i32::from_ne_bytes(
        buffer
            .get(bus_offset..bus_end)
            .ok_or(StorageError::InvalidGeometry)?
            .try_into()
            .map_err(|_| StorageError::InvalidGeometry)?,
    ))
}

fn query_handle_sector_layout(handle: &OwnedHandle) -> Result<SectorLayout, StorageError> {
    query_storage_alignment(handle)
}

fn query_file_information(
    handle: &OwnedHandle,
) -> Result<BY_HANDLE_FILE_INFORMATION, StorageError> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `handle` is a live FILE_READ_ATTRIBUTES directory handle and
    // `information` is writable for the exact API output structure.
    let ok = unsafe { GetFileInformationByHandle(handle.as_raw_handle(), &mut information) };
    if ok == 0 {
        return Err(last_windows_error("GetFileInformationByHandle"));
    }
    Ok(information)
}

fn checked_final_guid_path_capacity(required: u32) -> Result<(usize, u32), StorageError> {
    let required = usize::try_from(required).map_err(|_| StorageError::InvalidGeometry)?;
    if required == 0 {
        return Err(StorageError::InvalidGeometry);
    }
    let capacity = required
        .checked_add(1)
        .ok_or(StorageError::InvalidGeometry)?;
    if capacity > MAX_FINAL_GUID_PATH_UTF16_UNITS {
        return Err(StorageError::InvalidGeometry);
    }
    let api_capacity = u32::try_from(capacity).map_err(|_| StorageError::InvalidGeometry)?;
    Ok((capacity, api_capacity))
}

fn query_final_guid_path(handle: &OwnedHandle) -> Result<String, StorageError> {
    // SAFETY: a null buffer with zero capacity is the documented sizing query;
    // `handle` remains live and VOLUME_NAME_GUID requests a read-only name.
    let required = unsafe {
        GetFinalPathNameByHandleW(
            handle.as_raw_handle(),
            null_mut(),
            0,
            FILE_NAME_NORMALIZED | VOLUME_NAME_GUID,
        )
    };
    if required == 0 {
        return Err(last_windows_error("GetFinalPathNameByHandleW(size)"));
    }
    let (buffer_units, api_capacity) = checked_final_guid_path_capacity(required)?;
    let mut buffer = vec![0u16; buffer_units];
    // SAFETY: `buffer` is writable for its declared capacity; `handle` remains
    // live; the same query-only flags are used.
    let written = unsafe {
        GetFinalPathNameByHandleW(
            handle.as_raw_handle(),
            buffer.as_mut_ptr(),
            api_capacity,
            FILE_NAME_NORMALIZED | VOLUME_NAME_GUID,
        )
    };
    if written == 0 {
        return Err(last_windows_error("GetFinalPathNameByHandleW"));
    }
    let written = usize::try_from(written).map_err(|_| StorageError::InvalidGeometry)?;
    if written >= buffer.len() {
        return Err(StorageError::InvalidGeometry);
    }
    Ok(String::from_utf16_lossy(&buffer[..written]))
}

fn reject_reparse_ancestry(path: &Path) -> Result<(), StorageError> {
    let mut current = PathBuf::new();
    let mut rooted = false;
    for component in path.components() {
        match component {
            Component::Prefix(prefix) if matches!(prefix.kind(), Prefix::Disk(_)) => {
                current.push(component.as_os_str());
            }
            Component::RootDir => {
                current.push(component.as_os_str());
                rooted = true;
            }
            Component::Normal(_) if rooted => {
                current.push(component.as_os_str());
            }
            Component::CurDir | Component::ParentDir | Component::Normal(_) => {
                return Err(FolderScopeError::UnsafeComponent.into());
            }
            Component::Prefix(_) => return Err(FolderScopeError::UnsupportedRoot.into()),
        }
        if rooted {
            let metadata = std::fs::symlink_metadata(&current)
                .map_err(|_| FolderScopeError::IdentityUnavailable)?;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(FolderScopeError::ReparsePoint.into());
            }
        }
    }
    if !rooted {
        return Err(FolderScopeError::UnsupportedRoot.into());
    }
    Ok(())
}

fn drive_letter(path: &Path) -> Result<u8, FolderScopeError> {
    let mut components = path.components();
    let letter = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(letter) => letter.to_ascii_uppercase(),
            _ => return Err(FolderScopeError::UnsupportedRoot),
        },
        _ => return Err(FolderScopeError::UnsupportedRoot),
    };
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(FolderScopeError::UnsupportedRoot);
    }
    Ok(letter)
}

fn drive_root_wide(letter: u8) -> [u16; 4] {
    [
        u16::from(letter.to_ascii_uppercase()),
        u16::from(b':'),
        u16::from(b'\\'),
        0,
    ]
}

fn drive_kind(value: u32) -> DriveKind {
    match value {
        DRIVE_NO_ROOT_DIR => DriveKind::NoRoot,
        DRIVE_REMOVABLE => DriveKind::Removable,
        DRIVE_FIXED => DriveKind::Fixed,
        DRIVE_REMOTE => DriveKind::Remote,
        DRIVE_CDROM => DriveKind::CdRom,
        DRIVE_RAMDISK => DriveKind::RamDisk,
        DRIVE_UNKNOWN | 7..=u32::MAX => DriveKind::Unknown,
    }
}

fn distinct_disk_numbers(extents: &[DiskExtent]) -> Vec<u32> {
    extents
        .iter()
        .map(|extent| extent.disk_number)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn authoritative_policy_reason(drive_kind: DriveKind, disk_numbers: &[u32]) -> UnsupportedReason {
    match drive_scan_policy(drive_kind, disk_numbers) {
        ScanPolicy::Supported => UnsupportedReason::UnsupportedDriveType,
        ScanPolicy::Unsupported(reason) => reason,
    }
}

fn volume_selector(volume_guid: &str) -> Result<String, StorageError> {
    let selector = volume_guid.trim_end_matches('\\');
    if !selector.starts_with(r"\\?\Volume{")
        || !selector.ends_with('}')
        || selector.chars().any(char::is_control)
    {
        return Err(StorageError::SourceIdentityUnavailable);
    }
    Ok(selector.to_owned())
}

fn wide_string(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn utf16_buffer_to_string(buffer: &[u16]) -> String {
    let length = buffer
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..length])
}

fn owned_handle(
    handle: windows_sys::Win32::Foundation::HANDLE,
    operation: &'static str,
) -> Result<OwnedHandle, StorageError> {
    if handle == INVALID_HANDLE_VALUE || handle.is_null() {
        return Err(last_windows_error(operation));
    }
    // SAFETY: successful CreateFileW returned a unique owned HANDLE. Ownership
    // is transferred exactly once into OwnedHandle, which closes it on drop.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

fn last_windows_error(operation: &'static str) -> StorageError {
    StorageError::WindowsApi {
        operation,
        code: std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or_default() as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_drive_type_001_classifies_mapped_remote_drives() {
        assert_eq!(
            classify_drive_type(DRIVE_REMOTE),
            Ok(StorageLocation::Remote)
        );
        for value in [DRIVE_REMOVABLE, DRIVE_FIXED, DRIVE_CDROM, DRIVE_RAMDISK] {
            assert_eq!(classify_drive_type(value), Ok(StorageLocation::Local));
        }
        for value in [DRIVE_UNKNOWN, DRIVE_NO_ROOT_DIR, 99] {
            assert_eq!(
                classify_drive_type(value),
                Err(LocationError::DriveTypeUnavailable)
            );
        }
    }

    #[test]
    fn windows_drive_root_001_rejects_non_drive_prefixes() {
        assert_eq!(
            classify_path(Path::new(r"\\server\share\image.img")),
            Err(LocationError::UnsupportedRoot)
        );
        assert_eq!(
            classify_path(Path::new(r"relative\image.img")),
            Err(LocationError::UnsupportedRoot)
        );
    }

    #[test]
    fn windows_extent_parser_001_checks_count_and_buffer_bounds() {
        let header = offset_of!(VOLUME_DISK_EXTENTS, Extents);
        let extent_size = size_of::<DISK_EXTENT>();
        let mut buffer = vec![0u8; header + 2 * extent_size];
        buffer[..4].copy_from_slice(&2u32.to_le_bytes());
        write_extent_bytes(&mut buffer, header, 0, 3, 1_048_576, 4_194_304);
        write_extent_bytes(&mut buffer, header, 1, 3, 5_242_880, 8_388_608);

        assert_eq!(
            parse_volume_extents(&buffer, buffer.len()).unwrap(),
            vec![
                DiskExtent {
                    disk_number: 3,
                    starting_offset: 1_048_576,
                    extent_length: 4_194_304,
                },
                DiskExtent {
                    disk_number: 3,
                    starting_offset: 5_242_880,
                    extent_length: 8_388_608,
                },
            ]
        );
        assert!(parse_volume_extents(&buffer, header + extent_size).is_err());
        buffer[..4].copy_from_slice(&0u32.to_le_bytes());
        assert!(parse_volume_extents(&buffer, buffer.len()).is_err());
    }

    #[test]
    fn windows_volume_selector_001_rejects_arbitrary_device_paths() {
        assert_eq!(
            volume_selector(r"\\?\Volume{ABC-123}\").unwrap(),
            r"\\?\Volume{ABC-123}"
        );
        for forbidden in [r"\\.\PhysicalDrive0", r"C:\", r"\\server\share"] {
            assert!(volume_selector(forbidden).is_err());
        }
    }

    #[test]
    fn windows_inventory_boundary_001_has_no_dasd_or_device_control_path() {
        let source = include_str!("windows.rs");
        let inventory_start = source
            .find("pub(super) fn enumerate_storage")
            .expect("inventory function");
        let folder_start = source
            .find("pub(super) fn validate_folder_scope")
            .expect("folder function");
        let inventory_path = &source[inventory_start..folder_start];

        let mounted_query_start = source
            .find("fn query_mounted_volume")
            .expect("mounted volume query");
        let volume_open_start = source
            .find("fn open_volume_for_read")
            .expect("elevated raw-volume open marker");
        let mounted_query_path = &source[mounted_query_start..volume_open_start];

        for path in [inventory_path, mounted_query_path] {
            for forbidden in [
                "CreateFileW",
                "DeviceIoControl",
                "PhysicalDrive",
                "open_volume_for_query",
                "query_volume_disk_extents",
                "query_physical_disk_length",
            ] {
                assert!(
                    !path.contains(forbidden),
                    "unelevated inventory contains forbidden {forbidden}"
                );
            }
        }
    }

    #[test]
    fn windows_raw_identity_001_rejects_a_handle_serial_mismatch() {
        assert!(validate_handle_volume_serial(0xA1B2_C3D4, 0xA1B2_C3D4).is_ok());
        assert!(matches!(
            validate_handle_volume_serial(0xA1B2_C3D4, 0xA1B2_C3D5),
            Err(StorageError::SourceIdentityChanged)
        ));
    }

    #[test]
    fn windows_raw_identity_002_queries_handle_serial_on_open_and_revalidation() {
        let source = include_str!("windows.rs");
        let open_start = source
            .find("pub(super) fn open_raw_volume")
            .expect("raw open function");
        let raw_inner_start = source
            .find("pub(super) struct RawVolumeInner")
            .expect("raw inner marker");
        let open_path = &source[open_start..raw_inner_start];
        assert!(
            open_path.contains("query_handle_volume_serial(&handle)"),
            "raw open must bind the selected serial to the opened handle"
        );
        assert!(
            open_path.contains("query_storage_bus_type(&handle)"),
            "raw open must reject unproven virtual or composite backing"
        );

        let revalidate_start = source
            .find("pub(super) fn revalidate_identity")
            .expect("revalidation function");
        let read_start = source[revalidate_start..]
            .find("pub(super) fn read_exact_at")
            .map(|offset| revalidate_start + offset)
            .expect("read function marker");
        let revalidation_path = &source[revalidate_start..read_start];
        assert!(
            revalidation_path.contains("query_handle_volume_serial(&guard)"),
            "identity revalidation must query serial from the live handle"
        );
        assert!(
            revalidation_path.contains("query_storage_bus_type(&guard)"),
            "identity revalidation must recheck the storage bus"
        );
    }

    #[test]
    fn windows_physical_backing_003_rejects_virtual_spaces_network_array_and_unknown_buses() {
        use windows_sys::Win32::Storage::FileSystem::{
            BusType1394, BusTypeAtapi, BusTypeFibre, BusTypeFileBackedVirtual, BusTypeMax,
            BusTypeMaxReserved, BusTypeMmc, BusTypeRAID, BusTypeSCM, BusTypeSas, BusTypeScsi,
            BusTypeSd, BusTypeSpaces, BusTypeSsa, BusTypeUfs, BusTypeUnknown, BusTypeVirtual,
            BusTypeiScsi,
        };

        for bus_type in [BusTypeAta, BusTypeUsb, BusTypeSata, BusTypeNvme] {
            assert_eq!(
                physical_backing_from_bus_type(bus_type),
                PhysicalBacking::Direct,
                "{bus_type}"
            );
        }
        for bus_type in [
            BusTypeUnknown,
            BusTypeScsi,
            BusTypeAtapi,
            BusType1394,
            BusTypeSsa,
            BusTypeFibre,
            BusTypeRAID,
            BusTypeiScsi,
            BusTypeSas,
            BusTypeSd,
            BusTypeMmc,
            BusTypeVirtual,
            BusTypeFileBackedVirtual,
            BusTypeSpaces,
            BusTypeSCM,
            BusTypeUfs,
            BusTypeMax,
            BusTypeMaxReserved,
            -1,
        ] {
            assert_eq!(
                physical_backing_from_bus_type(bus_type),
                PhysicalBacking::Unproven,
                "{bus_type}"
            );
        }
    }

    #[test]
    fn windows_broker_peer_002_compares_canonical_paths_fail_closed() {
        assert!(windows_paths_equal(
            Path::new(r"C:\Program Files\Undelete Master\undelete-master-desktop.exe"),
            Path::new(r"c:\Program Files\Undelete Master\UNDELETE-MASTER-DESKTOP.EXE"),
        ));
        assert!(!windows_paths_equal(
            Path::new(r"C:\Program Files\Undelete Master\undelete-master-desktop.exe"),
            Path::new(r"C:\Temp\undelete-master-desktop.exe"),
        ));
        assert!(!windows_paths_equal(
            Path::new(r"C:\Program Files\Undelete Master\undelete-master-desktop.exe"),
            Path::new(r"C:\Program Files\Undelete Master\malware.exe"),
        ));
    }

    #[test]
    fn windows_final_guid_path_001_bounds_and_checks_the_api_capacity() {
        assert_eq!(checked_final_guid_path_capacity(1).unwrap(), (2usize, 2u32));
        assert_eq!(
            checked_final_guid_path_capacity(
                u32::try_from(MAX_FINAL_GUID_PATH_UTF16_UNITS - 1).unwrap()
            )
            .unwrap(),
            (
                MAX_FINAL_GUID_PATH_UTF16_UNITS,
                u32::try_from(MAX_FINAL_GUID_PATH_UTF16_UNITS).unwrap()
            )
        );

        for invalid in [
            0,
            u32::try_from(MAX_FINAL_GUID_PATH_UTF16_UNITS).unwrap(),
            u32::MAX,
        ] {
            assert!(matches!(
                checked_final_guid_path_capacity(invalid),
                Err(StorageError::InvalidGeometry)
            ));
        }
    }

    fn write_extent_bytes(
        buffer: &mut [u8],
        header: usize,
        index: usize,
        disk_number: u32,
        starting_offset: i64,
        extent_length: i64,
    ) {
        let base = header + index * size_of::<DISK_EXTENT>();
        let disk_offset = base + offset_of!(DISK_EXTENT, DiskNumber);
        let start_offset = base + offset_of!(DISK_EXTENT, StartingOffset);
        let length_offset = base + offset_of!(DISK_EXTENT, ExtentLength);
        buffer[disk_offset..disk_offset + 4].copy_from_slice(&disk_number.to_le_bytes());
        buffer[start_offset..start_offset + 8].copy_from_slice(&starting_offset.to_le_bytes());
        buffer[length_offset..length_offset + 8].copy_from_slice(&extent_length.to_le_bytes());
    }
}
