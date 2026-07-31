//! Audited read-only Windows storage boundary.
//!
//! The public API exposes opaque storage identities, sanitized display
//! metadata, folder identities without absolute paths, and a read-only RAW
//! volume reader. Callers cannot supply a device path, access mask, IOCTL, or
//! write operation.

#![deny(unsafe_op_in_unsafe_fn)]

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use um_core::{ReadError, ReadOutcome, SectorLayout, SourceIdentity, SourceReader};

mod model;
mod transport_config;

#[cfg(test)]
use model::{
    build_inventory, build_inventory_with_disk_sizes, decode_ntfs_file_reference, derive_volume_id,
    drive_scan_policy, parse_folder_identity_path, plan_aligned_read, AlignedReadPlan, DiskExtent,
    DriveKind, NativeVolumeSnapshot, ScanPolicy,
};
pub use model::{
    BusType, DestinationError, FolderScope, FolderScopeError, ReadPlanError, StorageDisk,
    StorageError, StorageInventory, StorageVolume, UnsupportedReason,
};
#[cfg(test)]
use transport_config::{
    broker_executable_from_current, build_broker_arguments, build_pipe_name, build_pipe_sddl,
    desktop_executable_from_broker,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageLocation {
    Local,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LocationError {
    #[error("path does not have a supported local drive root")]
    UnsupportedRoot,
    #[error("Windows could not classify the drive root")]
    DriveTypeUnavailable,
}

/// Classifies an absolute path without opening the path or any source file.
///
/// On Windows, mapped SMB/WebDAV drive letters are reported as `Remote`.
/// Callers must reject both `Remote` and errors. Other platforms do not have
/// Windows mapped drives and return `Local`; their own path policy remains the
/// caller's responsibility.
pub fn classify_path(path: &Path) -> Result<StorageLocation, LocationError> {
    platform::classify_path(path)
}

/// Enumerates real mounted Windows volumes into logical display groups.
///
/// The result contains only opaque identities and sanitized display metadata.
/// Remote, optical, RAM-backed, unmapped, and composite volumes cannot receive
/// read authority. Physical disk numbers are intentionally unavailable until
/// the elevated broker opens and verifies the selected source.
pub fn enumerate_storage() -> Result<StorageInventory, StorageError> {
    platform::enumerate_storage()
}

/// Validates a Rust-owned native folder selection against an opaque volume ID.
///
/// The returned value contains no absolute path. Reparse traversal, remote
/// roots, stale identities, and cross-volume selections fail closed.
pub fn validate_folder_scope(volume_id: &str, folder: &Path) -> Result<FolderScope, StorageError> {
    platform::validate_folder_scope(volume_id, folder)
}

/// Applies the native-only, fail-closed physical-disk destination policy.
///
/// Disk numbers are never included in a serializable desktop DTO. Empty,
/// multi-disk, changed-source, same-disk, and non-NTFS inputs are rejected.
pub fn validate_destination_policy(
    source_at_scan: Option<u32>,
    source_now: Option<u32>,
    destination_disk_numbers: &[u32],
    destination_file_system: &str,
) -> Result<(), DestinationError> {
    let destination_disks = model::PhysicalDiskSet::from_numbers(destination_disk_numbers);
    model::validate_destination_policy_inner(
        source_at_scan,
        source_now,
        &destination_disks,
        destination_file_system,
    )
}

/// Opaque native destination-root authority.
///
/// This value is intentionally neither cloneable nor serializable. It retains
/// the exact query-only directory handle opened by native Rust; no path,
/// volume GUID, physical-disk number, or native handle is exposed to the
/// WebView contract.
pub struct DestinationRootBinding {
    inner: platform::DestinationRootBindingInner,
}

/// Bounded native-only observation of a revalidated destination root.
///
/// This value contains no path, volume GUID, extent, handle, or shell input
/// and deliberately does not implement serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestinationRootSnapshot {
    file_system: String,
    free_bytes: u64,
    physical_disk_number: u32,
    reparse_safe: bool,
}

impl DestinationRootSnapshot {
    fn new(
        file_system: String,
        free_bytes: u64,
        physical_disk_number: u32,
        reparse_safe: bool,
    ) -> Self {
        Self {
            file_system,
            free_bytes,
            physical_disk_number,
            reparse_safe,
        }
    }

    pub fn file_system(&self) -> &str {
        &self.file_system
    }

    pub fn free_bytes(&self) -> u64 {
        self.free_bytes
    }

    pub fn physical_disk_number(&self) -> u32 {
        self.physical_disk_number
    }

    pub fn reparse_safe(&self) -> bool {
        self.reparse_safe
    }
}

impl DestinationRootBinding {
    pub fn display_label(&self) -> &str {
        self.inner.display_label()
    }

    pub fn volume_label(&self) -> &str {
        self.inner.volume_label()
    }

    pub fn file_system(&self) -> &str {
        self.inner.file_system()
    }

    pub fn free_bytes(&self) -> u64 {
        self.inner.free_bytes()
    }

    pub fn physical_disk_number(&self) -> u32 {
        self.inner.physical_disk_number()
    }

    pub fn reparse_safe(&self) -> bool {
        self.inner.reparse_safe()
    }

    /// Duplicates only the already-authorized retained directory handle.
    ///
    /// The duplicate refers to the same Windows file object and retains the
    /// admission handle's no-delete-share protection. No path is reopened or
    /// exposed.
    pub fn try_clone_directory_file(&self) -> Result<std::fs::File, StorageError> {
        self.inner.try_clone_directory_file()
    }

    /// Revalidates the retained authority through the same live directory
    /// handle and returns only bounded native policy evidence.
    pub fn revalidate(&self) -> Result<DestinationRootSnapshot, StorageError> {
        self.inner.revalidate()
    }

    /// Consumes the opaque binding and transfers the exact retained directory
    /// handle to native capability-relative restore code.
    pub fn into_directory_file(self) -> std::fs::File {
        self.inner.into_directory_file()
    }
}

/// Opens and queries a Rust-owned native destination selection without
/// creating, removing, renaming, truncating, or writing any entry.
pub fn open_destination_root_binding(
    destination_root: &Path,
) -> Result<DestinationRootBinding, StorageError> {
    platform::open_destination_root_binding(destination_root)
        .map(|inner| DestinationRootBinding { inner })
}

/// Opens an already-retained native job directory in the Windows shell.
///
/// The caller supplies only the owned directory handle. The platform adapter
/// verifies that handle and derives the bounded normalized shell target from
/// it; callers cannot select a path, verb, executable, or argument.
pub fn open_retained_directory_in_shell(
    retained_directory: std::fs::File,
) -> Result<(), StorageError> {
    platform::open_retained_directory_in_shell(retained_directory)
}

/// Broker-facing read-only RAW volume.
///
/// Construction is identity-bound: the caller supplies only an opaque ID and
/// the Windows selector is rebuilt internally from a fresh inventory.
pub struct RawVolume {
    inner: platform::RawVolumeInner,
}

impl RawVolume {
    pub fn open_by_identity(volume_id: &str) -> Result<Self, StorageError> {
        platform::open_raw_volume(volume_id).map(|inner| Self { inner })
    }

    pub fn identity(&self) -> &SourceIdentity {
        self.inner.identity()
    }

    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn sector_layout(&self) -> SectorLayout {
        self.inner.sector_layout()
    }

    pub fn physical_disk_number(&self) -> u32 {
        self.inner.physical_disk_number()
    }

    pub fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), StorageError> {
        self.inner.read_exact_at(offset, buffer)
    }

    /// Re-enumerates Windows storage and proves that the opened volume still
    /// has the same multi-property identity.
    pub fn revalidate_identity(&self) -> Result<(), StorageError> {
        self.inner.revalidate_identity()
    }
}

impl SourceReader for RawVolume {
    fn identity(&self) -> &SourceIdentity {
        RawVolume::identity(self)
    }

    fn len(&self) -> u64 {
        RawVolume::len(self)
    }

    fn sector_layout(&self) -> SectorLayout {
        RawVolume::sector_layout(self)
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        RawVolume::read_exact_at(self, offset, buffer)
            .map_err(|error| storage_error_to_read_error(error, offset, buffer.len(), self.len()))
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        match <Self as SourceReader>::read_exact_at(self, offset, buffer) {
            Ok(()) => ReadOutcome::complete(buffer.len() as u64),
            Err(_) => {
                buffer.fill(0);
                ReadOutcome {
                    bytes_valid: 0,
                    bad_ranges: vec![(0, buffer.len() as u64)],
                }
            }
        }
    }
}

/// One-instance local broker pipe server with a current-user DACL.
pub struct NamedPipeServer {
    inner: platform::NamedPipeServerInner,
}

impl NamedPipeServer {
    pub fn create(pipe_suffix: &str) -> Result<Self, StorageError> {
        platform::create_named_pipe_server(pipe_suffix).map(|inner| Self { inner })
    }

    pub fn accept(self) -> Result<ConnectedPipe, StorageError> {
        self.accept_with_timeout(Duration::from_secs(10))
    }

    pub fn accept_with_timeout(self, timeout: Duration) -> Result<ConnectedPipe, StorageError> {
        self.inner
            .accept_with_timeout(timeout, None)
            .map(|inner| ConnectedPipe { inner })
    }

    pub fn accept_for_process(
        self,
        broker: &ElevatedBrokerProcess,
    ) -> Result<ConnectedPipe, StorageError> {
        self.inner
            .accept_with_timeout(Duration::from_secs(10), Some(&broker.inner))
            .map(|inner| ConnectedPipe { inner })
    }
}

/// Authenticated local named-pipe byte stream bound to an observed peer
/// process. The elevated side additionally verifies the packaged desktop image.
pub struct ConnectedPipe {
    inner: platform::ConnectedPipeInner,
}

impl ConnectedPipe {
    pub fn peer_process_id(&self) -> u32 {
        self.inner.peer_process_id()
    }

    pub fn peer_is_alive(&self) -> Result<bool, StorageError> {
        self.inner.peer_is_alive()
    }

    /// Proves that the connected client is the fixed desktop executable beside
    /// this broker. This path check narrows the elevated trust boundary but
    /// does not replace the production code-signing requirement.
    pub fn peer_is_packaged_desktop(&self) -> Result<bool, StorageError> {
        self.inner.peer_is_packaged_desktop()
    }
}

impl Read for ConnectedPipe {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buffer)
    }
}

impl Write for ConnectedPipe {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Connects to the internal broker pipe and verifies the server process ID.
pub fn connect_broker_pipe(
    pipe_suffix: &str,
    expected_server_pid: u32,
) -> Result<ConnectedPipe, StorageError> {
    platform::connect_broker_pipe(pipe_suffix, expected_server_pid)
        .map(|inner| ConnectedPipe { inner })
}

/// Handle guard for the fixed packaged elevated broker process.
pub struct ElevatedBrokerProcess {
    inner: platform::ElevatedBrokerProcessInner,
}

impl ElevatedBrokerProcess {
    pub fn pid(&self) -> u32 {
        self.inner.pid()
    }

    pub fn is_alive(&self) -> Result<bool, StorageError> {
        self.inner.is_alive()
    }
}

/// Launches the fixed sibling broker executable through Windows `runas`.
pub fn launch_elevated_broker(
    pipe_suffix: &str,
    parent_pid: u32,
) -> Result<ElevatedBrokerProcess, StorageError> {
    platform::launch_elevated_broker(pipe_suffix, parent_pid)
        .map(|inner| ElevatedBrokerProcess { inner })
}

fn storage_error_to_read_error(
    error: StorageError,
    offset: u64,
    requested_len: usize,
    source_len: u64,
) -> ReadError {
    match error {
        StorageError::ReadPlan(ReadPlanError::OutOfBounds | ReadPlanError::Overflow) => {
            ReadError::OutOfBounds {
                offset,
                len: requested_len as u64,
                source_len,
            }
        }
        StorageError::SourceIdentityUnavailable | StorageError::SourceIdentityChanged => {
            ReadError::SourceGone
        }
        other => ReadError::Io {
            offset,
            message: other.to_string(),
        },
    }
}

#[cfg(windows)]
#[path = "windows.rs"]
mod platform;

#[cfg(not(windows))]
#[path = "unsupported.rs"]
mod platform;

#[cfg(test)]
mod storage_contract_tests {
    use std::collections::BTreeMap;
    #[cfg(windows)]
    use std::io::{Read, Write};
    #[cfg(windows)]
    use std::path::PathBuf;
    #[cfg(windows)]
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(windows)]
    use std::time::Duration;

    use super::*;

    #[cfg(windows)]
    static NEXT_PIPE: AtomicU64 = AtomicU64::new(1);

    fn local_volume(mount_label: &str, disk_number: u32) -> NativeVolumeSnapshot {
        NativeVolumeSnapshot {
            mount_label: mount_label.to_owned(),
            drive_kind: DriveKind::Fixed,
            volume_guid: format!(r"\\?\Volume{{fixture-{disk_number}-{mount_label}}}\"),
            label: format!("Volume {mount_label}"),
            file_system: "NTFS".to_owned(),
            volume_serial: 0xA1B2_C3D4,
            total_bytes: 8 * 1024 * 1024,
            free_bytes: 3 * 1024 * 1024,
            logical_sector_bytes: 512,
            physical_sector_bytes: 4096,
            extents: vec![DiskExtent {
                disk_number,
                starting_offset: 1_048_576,
                extent_length: 8 * 1024 * 1024,
            }],
            is_system: mount_label.eq_ignore_ascii_case("C:"),
        }
    }

    #[test]
    fn windows_inventory_identity_001_uses_only_canonical_guid_and_serial() {
        let original = local_volume("C:", 2);
        let original_id = derive_volume_id(&original);

        assert!(original_id.starts_with("vol-"));
        assert_eq!(original_id.len(), 68);
        assert!(!original_id.contains("fixture"));
        assert!(!original_id.contains("C:"));

        let mut display_and_authority_changes = original.clone();
        display_and_authority_changes.mount_label = "Z:".to_owned();
        display_and_authority_changes.label = "Renamed by user".to_owned();
        display_and_authority_changes.file_system = "ReFS".to_owned();
        display_and_authority_changes.free_bytes = 1;
        display_and_authority_changes.logical_sector_bytes = 4096;
        display_and_authority_changes.physical_sector_bytes = 8192;
        display_and_authority_changes.extents[0].disk_number = 99;
        display_and_authority_changes.extents[0].starting_offset += 4096;
        display_and_authority_changes.extents[0].extent_length -= 4096;
        let mut changed_size = display_and_authority_changes.clone();
        changed_size.total_bytes += 512;
        assert_eq!(original_id, derive_volume_id(&changed_size));

        let mut changed_serial = original.clone();
        changed_serial.volume_serial ^= 1;
        assert_ne!(original_id, derive_volume_id(&changed_serial));

        let mut changed_guid_case = original;
        changed_guid_case.volume_guid = changed_guid_case.volume_guid.to_ascii_uppercase();
        assert_eq!(
            original_id,
            derive_volume_id(&changed_guid_case),
            "canonical GUID casing must not change the opaque ID"
        );
    }

    #[test]
    fn windows_inventory_group_001_uses_honest_logical_groups_without_physical_numbers() {
        let inventory =
            build_inventory(vec![local_volume("D:", 7), local_volume("C:", 7)]).unwrap();

        assert_eq!(inventory.schema_version, 1);
        assert_eq!(inventory.disks.len(), 2);
        assert!(inventory
            .disks
            .iter()
            .all(|group| group.display_name.starts_with("Mounted volume ")));
        assert_eq!(
            inventory
                .disks
                .iter()
                .flat_map(|group| group.volumes.iter())
                .map(|volume| volume.mount_label.as_str())
                .collect::<Vec<_>>(),
            ["C:", "D:"]
        );
        assert!(inventory
            .disks
            .iter()
            .flat_map(|group| group.volumes.iter())
            .all(|volume| volume.scan_supported && volume.folder_scope_supported));

        let serialized = serde_json::to_value(&inventory).unwrap();
        assert!(serialized["disks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|group| group["number"].is_null()));
    }

    #[test]
    fn windows_inventory_identity_002_does_not_gain_physical_authority_from_display_queries() {
        let snapshot = local_volume("C:", 7);
        let without_disk_length = build_inventory(vec![snapshot.clone()]).unwrap();
        let with_disk_length = build_inventory_with_disk_sizes(
            vec![snapshot],
            &BTreeMap::from([(7, 64 * 1024 * 1024)]),
        )
        .unwrap();

        assert_eq!(
            without_disk_length.disks[0].id,
            with_disk_length.disks[0].id
        );
        assert_eq!(
            without_disk_length.disks[0].size_bytes,
            with_disk_length.disks[0].size_bytes
        );
    }

    #[test]
    fn windows_inventory_partial_001_keeps_valid_volumes_when_one_root_query_fails() {
        let inventory = model::build_inventory_from_query_results(vec![
            Err(StorageError::WindowsApi {
                operation: "fixture-root-query",
                code: 5,
            }),
            Ok(local_volume("E:", 12)),
        ])
        .unwrap();

        assert_eq!(inventory.disks.len(), 1);
        assert_eq!(inventory.disks[0].volumes[0].mount_label, "E:");
    }

    #[test]
    fn windows_inventory_policy_001_rejects_non_local_and_composite_sources() {
        for drive_kind in [DriveKind::Remote, DriveKind::CdRom, DriveKind::RamDisk] {
            assert_eq!(
                drive_scan_policy(drive_kind, &[2]),
                ScanPolicy::Unsupported(UnsupportedReason::NonLocalDrive)
            );
        }

        assert_eq!(
            drive_scan_policy(DriveKind::Fixed, &[2, 3]),
            ScanPolicy::Unsupported(UnsupportedReason::MultiDiskVolume)
        );
        assert_eq!(
            drive_scan_policy(DriveKind::Fixed, &[2]),
            ScanPolicy::Supported
        );
        assert_eq!(
            drive_scan_policy(DriveKind::Removable, &[9]),
            ScanPolicy::Supported
        );
    }

    #[test]
    fn windows_inventory_policy_002_limits_folder_identity_scope_to_ntfs() {
        let mut fat = local_volume("E:", 4);
        fat.file_system = "FAT32".to_owned();
        let inventory = build_inventory(vec![fat]).unwrap();
        let volume = &inventory.disks[0].volumes[0];

        assert!(volume.scan_supported);
        assert!(!volume.folder_scope_supported);
    }

    #[test]
    fn windows_folder_scope_001_keeps_only_relative_identity_components() {
        let parsed = parse_folder_identity_path(
            r"\\?\Volume{4A3B-2C1D}\Users\Alice\Documents",
            r"\\?\Volume{4a3b-2c1d}\",
        )
        .unwrap();

        assert_eq!(parsed.relative_components, ["Users", "Alice", "Documents"]);
        assert_eq!(parsed.display_label, "Documents");
        assert!(!parsed.display_label.contains(r"\\?\"));
    }

    #[test]
    fn windows_folder_scope_002_rejects_cross_volume_or_parent_components() {
        assert_eq!(
            parse_folder_identity_path(r"\\?\Volume{OTHER}\Documents", r"\\?\Volume{SELECTED}\",),
            Err(FolderScopeError::CrossVolume)
        );
        assert_eq!(
            parse_folder_identity_path(
                r"\\?\Volume{SELECTED}\Users\..\Secrets",
                r"\\?\Volume{SELECTED}\",
            ),
            Err(FolderScopeError::UnsafeComponent)
        );
    }

    #[test]
    fn windows_folder_scope_003_decodes_ntfs_file_reference_identity() {
        assert_eq!(
            decode_ntfs_file_reference(0x1234_0000_0000_5678),
            (0x5678, 0x1234)
        );
    }

    #[test]
    fn windows_raw_read_plan_001_aligns_without_escaping_source_bounds() {
        assert_eq!(
            plan_aligned_read(513, 1000, 8192, 512).unwrap(),
            AlignedReadPlan {
                aligned_offset: 512,
                aligned_length: 1024,
                copy_offset: 1,
                copy_length: 1000,
            }
        );
        assert_eq!(
            plan_aligned_read(8190, 3, 8192, 512),
            Err(ReadPlanError::OutOfBounds)
        );
        assert_eq!(
            plan_aligned_read(u64::MAX, 2, u64::MAX, 512),
            Err(ReadPlanError::Overflow)
        );
        assert_eq!(
            plan_aligned_read(0, 1, 8192, 0),
            Err(ReadPlanError::InvalidSectorSize)
        );
        assert_eq!(
            plan_aligned_read(0, 4 * 1024 * 1024 + 1, 8 * 1024 * 1024, 512),
            Err(ReadPlanError::LengthTooLarge)
        );
    }

    #[test]
    fn windows_broker_config_001_accepts_only_opaque_pipe_suffixes() {
        let suffix = "0123456789abcdef0123456789ABCDEF";
        assert_eq!(
            build_pipe_name(suffix).unwrap(),
            r"\\.\pipe\UndeleteMaster-v1-0123456789abcdef0123456789ABCDEF"
        );
        for invalid in [
            "short",
            "../escape-0123456789",
            r"nested\pipe-0123456789",
            "contains.dot.0123456789",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdefX",
        ] {
            assert!(build_pipe_name(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn windows_broker_config_002_builds_fixed_arguments_and_current_user_dacl() {
        let suffix = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            build_broker_arguments(suffix, 4242).unwrap(),
            "--pipe 0123456789abcdef0123456789abcdef --parent-pid 4242"
        );
        assert!(build_broker_arguments(suffix, 0).is_err());
        assert_eq!(
            build_pipe_sddl("S-1-5-21-123-456-789-1001").unwrap(),
            "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;S-1-5-21-123-456-789-1001)"
        );
        assert!(build_pipe_sddl("S-1-5-21);(A;;GA;;;WD").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_named_pipe_001_binds_one_local_instance_and_peer_pid() {
        let suffix = format!(
            "{:016x}{:016x}",
            std::process::id(),
            NEXT_PIPE.fetch_add(1, Ordering::Relaxed)
        );
        let server = NamedPipeServer::create(&suffix).unwrap();
        let server_thread = std::thread::spawn(move || {
            let mut connection = server.accept().unwrap();
            assert_eq!(connection.peer_process_id(), std::process::id());
            let mut request = [0u8; 4];
            connection.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"ping");
            connection.write_all(b"pong").unwrap();
        });

        let mut client = connect_broker_pipe(&suffix, std::process::id()).unwrap();
        assert_eq!(client.peer_process_id(), std::process::id());
        assert!(client.peer_is_alive().unwrap());
        client.write_all(b"ping").unwrap();
        let mut response = [0u8; 4];
        client.read_exact(&mut response).unwrap();
        assert_eq!(&response, b"pong");
        server_thread.join().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_named_pipe_002_cancels_a_bounded_accept() {
        let suffix = format!(
            "{:016x}{:016x}",
            std::process::id(),
            NEXT_PIPE.fetch_add(1, Ordering::Relaxed)
        );
        let server = NamedPipeServer::create(&suffix).unwrap();
        let started = std::time::Instant::now();
        assert!(matches!(
            server.accept_with_timeout(Duration::from_millis(10)),
            Err(StorageError::PipeAcceptTimedOut)
        ));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[cfg(windows)]
    #[test]
    fn windows_broker_launch_001_resolves_only_the_fixed_sibling_binary() {
        let current = Path::new(r"C:\Program Files\Undelete Master\undelete-master.exe");
        assert_eq!(
            broker_executable_from_current(current).unwrap(),
            PathBuf::from(r"C:\Program Files\Undelete Master\undelete-master-broker.exe")
        );
        assert!(broker_executable_from_current(Path::new("relative.exe")).is_err());
    }

    #[test]
    fn windows_broker_peer_001_resolves_only_the_packaged_desktop_sibling() {
        let broker = Path::new(r"C:\Program Files\Undelete Master\undelete-master-broker.exe");
        assert_eq!(
            desktop_executable_from_broker(broker).unwrap(),
            PathBuf::from(r"C:\Program Files\Undelete Master\undelete-master-desktop.exe")
        );
        assert!(desktop_executable_from_broker(Path::new(
            r"C:\Program Files\Undelete Master\renamed-broker.exe"
        ))
        .is_err());
        assert!(desktop_executable_from_broker(Path::new("relative-broker.exe")).is_err());
    }
}

#[cfg(test)]
mod destination_root_contract_tests {
    use std::path::Path;

    use super::*;
    use crate::model::{
        DestinationRootInformation, DestinationRootQuery, DestinationVolumeInformation,
        PhysicalBacking,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct MarkerHandle(u32);

    struct ScriptedDestinationQuery {
        calls: Vec<&'static str>,
        root: DestinationRootInformation,
        final_path: String,
        volume: DestinationVolumeInformation,
    }

    impl ScriptedDestinationQuery {
        fn ntfs_single_disk() -> Self {
            Self {
                calls: Vec::new(),
                root: DestinationRootInformation {
                    is_directory: true,
                    is_reparse_point: false,
                    volume_serial: 0xA1B2_C3D4,
                    file_index: 42,
                },
                final_path: r"\\?\Volume{DEST-0001}\Recovery".to_owned(),
                volume: DestinationVolumeInformation {
                    label: "\u{202e}Recovery\n".to_owned(),
                    file_system: "ntfs".to_owned(),
                    volume_serial: 0xA1B2_C3D4,
                    total_bytes: 1_000,
                    free_bytes: 700,
                    disk_numbers: vec![9],
                    physical_backing: PhysicalBacking::Direct,
                },
            }
        }
    }

    impl DestinationRootQuery for ScriptedDestinationQuery {
        type RootHandle = MarkerHandle;

        fn classify_root(&mut self, _path: &Path) -> Result<(), DestinationError> {
            self.calls.push("classify");
            Ok(())
        }

        fn open_root(&mut self, _path: &Path) -> Result<Self::RootHandle, DestinationError> {
            self.calls.push("open_root");
            Ok(MarkerHandle(17))
        }

        fn query_root(
            &mut self,
            _handle: &Self::RootHandle,
        ) -> Result<DestinationRootInformation, DestinationError> {
            self.calls.push("query_root");
            Ok(self.root)
        }

        fn query_final_path(
            &mut self,
            _handle: &Self::RootHandle,
        ) -> Result<String, DestinationError> {
            self.calls.push("query_final_path");
            Ok(self.final_path.clone())
        }

        fn query_volume(
            &mut self,
            _volume_guid: &str,
        ) -> Result<DestinationVolumeInformation, DestinationError> {
            self.calls.push("query_volume");
            Ok(self.volume.clone())
        }
    }

    #[test]
    fn windows_destination_binding_001_retains_the_exact_query_handle_and_sanitized_evidence() {
        let mut query = ScriptedDestinationQuery::ntfs_single_disk();

        let binding =
            crate::model::open_destination_root_with(Path::new(r"E:\Recovery"), &mut query)
                .expect("authorize destination root");

        assert_eq!(
            query.calls,
            [
                "classify",
                "open_root",
                "query_root",
                "query_final_path",
                "query_volume",
            ]
        );
        assert_eq!(binding.root_handle, MarkerHandle(17));
        assert_eq!(binding.final_volume_guid, r"\\?\Volume{DEST-0001}\");
        assert_eq!(binding.display_label, "Recovery");
        assert_eq!(binding.volume_label, "Recovery");
        assert_eq!(binding.file_system, "ntfs");
        assert_eq!(binding.free_bytes, 700);
        assert_eq!(binding.physical_disks.single(), Ok(9));
        assert!(binding.reparse_safe);
        assert_eq!(binding.volume_serial, 0xA1B2_C3D4);
        assert_eq!(binding.file_index, 42);
    }

    #[test]
    fn windows_destination_binding_002_rejects_non_directory_or_reparse_before_volume_query() {
        for (is_directory, is_reparse_point, expected) in [
            (false, false, DestinationError::NotDirectory),
            (true, true, DestinationError::ReparsePoint),
        ] {
            let mut query = ScriptedDestinationQuery::ntfs_single_disk();
            query.root.is_directory = is_directory;
            query.root.is_reparse_point = is_reparse_point;

            let error =
                crate::model::open_destination_root_with(Path::new(r"E:\Recovery"), &mut query)
                    .expect_err("unsafe destination root must be rejected");

            assert_eq!(error, expected);
            assert_eq!(query.calls, ["classify", "open_root", "query_root"]);
        }
    }

    #[test]
    fn windows_destination_binding_003_rejects_changed_unknown_multi_disk_or_non_ntfs_volume() {
        let cases = [
            (
                Some(0xA1B2_C3D5),
                vec![9],
                "NTFS",
                DestinationError::IdentityUnavailable,
            ),
            (
                None,
                Vec::new(),
                "NTFS",
                DestinationError::MissingDiskMapping,
            ),
            (None, vec![9, 10], "NTFS", DestinationError::MultiDiskVolume),
            (
                None,
                vec![9],
                "ReFS",
                DestinationError::UnsupportedFileSystem,
            ),
            (
                None,
                vec![9],
                "N\0TFS",
                DestinationError::UnsupportedFileSystem,
            ),
        ];

        for (changed_serial, disk_numbers, file_system, expected) in cases {
            let mut query = ScriptedDestinationQuery::ntfs_single_disk();
            if let Some(serial) = changed_serial {
                query.volume.volume_serial = serial;
            }
            query.volume.disk_numbers = disk_numbers;
            query.volume.file_system = file_system.to_owned();

            assert_eq!(
                crate::model::open_destination_root_with(Path::new(r"E:\Recovery"), &mut query)
                    .expect_err("unsafe destination volume must be rejected"),
                expected
            );
        }
    }

    #[test]
    fn windows_destination_binding_004_parses_only_a_final_volume_guid_root() {
        assert_eq!(
            crate::model::volume_guid_root_from_final_path(
                r"\\?\Volume{ABC-123}\folder\destination"
            ),
            Ok(r"\\?\Volume{ABC-123}\".to_owned())
        );
        let physical_drive = [r"\\.\Physical", "Drive0"].concat();
        let global_root = [r"\\?\GLOBAL", r"ROOT\Device\HarddiskVolume1"].concat();
        for invalid in [
            r"C:\destination".to_owned(),
            r"\\server\share\destination".to_owned(),
            physical_drive,
            global_root,
            "\\\\?\\Volume{ABC\u{202e}}\u{5c}destination".to_owned(),
        ] {
            assert_eq!(
                crate::model::volume_guid_root_from_final_path(&invalid),
                Err(DestinationError::IdentityUnavailable),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn windows_destination_binding_005_exposes_only_the_opaque_native_binding_api() {
        let _open: fn(&Path) -> Result<DestinationRootBinding, StorageError> =
            open_destination_root_binding;
        let _consume: fn(DestinationRootBinding) -> std::fs::File =
            DestinationRootBinding::into_directory_file;
        let _duplicate: fn(&DestinationRootBinding) -> Result<std::fs::File, StorageError> =
            DestinationRootBinding::try_clone_directory_file;
        let _revalidate: fn(
            &DestinationRootBinding,
        ) -> Result<DestinationRootSnapshot, StorageError> = DestinationRootBinding::revalidate;
        let _open_shell: fn(std::fs::File) -> Result<(), StorageError> =
            open_retained_directory_in_shell;
    }

    #[test]
    fn windows_destination_binding_006_rejects_unproven_virtual_or_composite_backing() {
        let mut query = ScriptedDestinationQuery::ntfs_single_disk();
        query.volume.physical_backing = PhysicalBacking::Unproven;

        assert_eq!(
            crate::model::open_destination_root_with(Path::new(r"E:\Recovery"), &mut query)
                .expect_err("a distinct virtual disk number is not physical separation"),
            DestinationError::UnprovenPhysicalBacking
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn windows_destination_binding_007_non_windows_is_structured_unsupported() {
        assert!(matches!(
            open_destination_root_binding(Path::new("/tmp")),
            Err(StorageError::Destination(
                DestinationError::UnsupportedPlatform
            ))
        ));

        let executable = std::fs::File::open(std::env::current_exe().expect("current executable"))
            .expect("open a disposable read-only handle");
        assert!(matches!(
            open_retained_directory_in_shell(executable),
            Err(StorageError::Destination(
                DestinationError::UnsupportedPlatform
            ))
        ));
    }
}
