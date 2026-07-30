use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub(crate) const STORAGE_SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_RAW_READ_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BusType {
    Unknown,
    Ata,
    Sata,
    Scsi,
    Usb,
    Nvme,
    Virtual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInventory {
    pub schema_version: u32,
    pub generation: String,
    pub disks: Vec<StorageDisk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDisk {
    pub id: String,
    /// Physical disk number, when proven by an authoritative elevated query.
    ///
    /// Unelevated mounted-root inventory deliberately reports `None`; it must
    /// not fabricate a PhysicalDrive relationship from a drive letter.
    pub number: Option<u32>,
    pub display_name: String,
    pub bus_type: BusType,
    pub size_bytes: u64,
    pub volumes: Vec<StorageVolume>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageVolume {
    pub id: String,
    pub mount_label: String,
    pub label: String,
    pub file_system: String,
    pub size_bytes: u64,
    pub free_bytes: u64,
    pub is_system: bool,
    pub scan_supported: bool,
    pub folder_scope_supported: bool,
    pub warnings: Vec<String>,
}

/// Native-only folder authority. It deliberately contains no absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderScope {
    pub scope_id: String,
    pub volume_id: String,
    pub display_label: String,
    pub relative_components: Vec<String>,
    pub volume_serial: u32,
    pub file_index: u64,
    /// Low 48-bit MFT segment number for NTFS 3.0/3.1.
    pub ntfs_record: u64,
    /// High 16-bit reuse sequence for NTFS 3.0/3.1.
    pub ntfs_sequence: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UnsupportedReason {
    NonLocalDrive,
    MultiDiskVolume,
    MissingDiskMapping,
    UnsupportedDriveType,
    UnsupportedFileSystem,
}

impl UnsupportedReason {
    pub(crate) const fn warning_code(self) -> &'static str {
        match self {
            Self::NonLocalDrive => "UNSUPPORTED_NON_LOCAL_DRIVE",
            Self::MultiDiskVolume => "UNSUPPORTED_MULTI_DISK_LAYOUT",
            Self::MissingDiskMapping => "UNSUPPORTED_DISK_MAPPING",
            Self::UnsupportedDriveType => "UNSUPPORTED_DRIVE_TYPE",
            Self::UnsupportedFileSystem => "UNSUPPORTED_FOLDER_SCOPE_FILESYSTEM",
        }
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Windows storage discovery is unavailable on this platform")]
    UnsupportedPlatform,
    #[error("Windows storage operation {operation} failed with code {code}")]
    WindowsApi { operation: &'static str, code: u32 },
    #[error("the selected storage identity is unavailable or stale")]
    SourceIdentityUnavailable,
    #[error("the selected source is not eligible for read-only scanning: {0:?}")]
    UnsupportedSource(UnsupportedReason),
    #[error("the selected folder could not be authorized: {0}")]
    FolderScope(#[from] FolderScopeError),
    #[error("invalid read request: {0}")]
    ReadPlan(#[from] ReadPlanError),
    #[error("the read-only source returned an incomplete read")]
    ShortRead,
    #[error("the source reported invalid length or sector geometry")]
    InvalidGeometry,
    #[error("the source identity changed while it was being opened")]
    SourceIdentityChanged,
    #[error("a synchronization primitive was poisoned")]
    Synchronization,
    #[error("the broker pipe suffix is invalid")]
    InvalidPipeSuffix,
    #[error("the broker parent process id is invalid")]
    InvalidParentProcess,
    #[error("the broker executable is unavailable or outside the fixed package location")]
    BrokerExecutableUnavailable,
    #[error(
        "the named-pipe peer process did not match (expected {expected}, observed {observed})"
    )]
    UnexpectedPeerProcess { expected: u32, observed: u32 },
    #[error("the current Windows user identity could not be encoded safely")]
    InvalidUserIdentity,
    #[error("the user canceled the Windows elevation request")]
    ElevationCancelled,
    #[error("the broker did not connect before the bounded timeout")]
    PipeAcceptTimedOut,
    #[error("the broker exited before connecting to its pipe")]
    BrokerExitedBeforeConnect,
    #[error("the broker pipe accept timeout is outside the allowed bound")]
    InvalidPipeTimeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FolderScopeError {
    #[error("the path does not have an eligible local drive root")]
    UnsupportedRoot,
    #[error("the selected path is remote or redirected")]
    NonLocal,
    #[error("the selected path crosses a reparse point")]
    ReparsePoint,
    #[error("the selected path belongs to a different volume")]
    CrossVolume,
    #[error("the selected path contains an unsafe component")]
    UnsafeComponent,
    #[error("the selected path is not a directory")]
    NotDirectory,
    #[error("the selected folder identity could not be proven")]
    IdentityUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ReadPlanError {
    #[error("the requested range overflows")]
    Overflow,
    #[error("the requested range is outside the source")]
    OutOfBounds,
    #[error("the source reported an invalid sector size")]
    InvalidSectorSize,
    #[error("the aligned read does not fit in memory")]
    LengthTooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DriveKind {
    Unknown,
    NoRoot,
    Removable,
    Fixed,
    Remote,
    CdRom,
    RamDisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DiskExtent {
    pub disk_number: u32,
    pub starting_offset: u64,
    pub extent_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeVolumeSnapshot {
    pub mount_label: String,
    pub drive_kind: DriveKind,
    pub volume_guid: String,
    pub label: String,
    pub file_system: String,
    pub volume_serial: u32,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub logical_sector_bytes: u32,
    pub physical_sector_bytes: u32,
    pub extents: Vec<DiskExtent>,
    pub is_system: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanPolicy {
    Supported,
    Unsupported(UnsupportedReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FolderIdentityPath {
    pub relative_components: Vec<String>,
    pub display_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlignedReadPlan {
    pub aligned_offset: u64,
    pub aligned_length: usize,
    pub copy_offset: usize,
    pub copy_length: usize,
}

pub(crate) fn drive_scan_policy(drive_kind: DriveKind, disk_numbers: &[u32]) -> ScanPolicy {
    match drive_kind {
        DriveKind::Fixed | DriveKind::Removable => {}
        DriveKind::Remote | DriveKind::CdRom | DriveKind::RamDisk => {
            return ScanPolicy::Unsupported(UnsupportedReason::NonLocalDrive);
        }
        DriveKind::Unknown | DriveKind::NoRoot => {
            return ScanPolicy::Unsupported(UnsupportedReason::UnsupportedDriveType);
        }
    }

    match disk_numbers.len() {
        0 => ScanPolicy::Unsupported(UnsupportedReason::MissingDiskMapping),
        1 => ScanPolicy::Supported,
        _ => ScanPolicy::Unsupported(UnsupportedReason::MultiDiskVolume),
    }
}

pub(crate) fn derive_volume_id(snapshot: &NativeVolumeSnapshot) -> String {
    let mut digest = Sha256::new();
    digest.update(b"undelete-master:volume-identity:v2");
    let canonical_guid = snapshot
        .volume_guid
        .trim()
        .trim_end_matches('\\')
        .to_ascii_lowercase();
    update_digest_field(&mut digest, canonical_guid.as_bytes());
    update_digest_field(&mut digest, &snapshot.volume_serial.to_le_bytes());
    format!("vol-{}", hex::encode(digest.finalize()))
}

pub(crate) fn derive_folder_scope_id(
    volume_id: &str,
    volume_serial: u32,
    file_index: u64,
    components: &[String],
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"undelete-master:folder-scope:v1");
    update_digest_field(&mut digest, volume_id.as_bytes());
    update_digest_field(&mut digest, &volume_serial.to_le_bytes());
    update_digest_field(&mut digest, &file_index.to_le_bytes());
    for component in components {
        update_digest_field(&mut digest, component.as_bytes());
    }
    format!("scope-{}", hex::encode(digest.finalize()))
}

/// Decodes the NTFS 3.0/3.1 64-bit file-reference layout.
///
/// Callers invoke this only after the selected mounted volume has been proven
/// to report the NTFS filesystem.
pub(crate) const fn decode_ntfs_file_reference(file_reference: u64) -> (u64, u16) {
    (
        file_reference & 0x0000_FFFF_FFFF_FFFF,
        (file_reference >> 48) as u16,
    )
}

fn update_digest_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_le_bytes());
    digest.update(field);
}

#[cfg(test)]
pub(crate) fn build_inventory(
    snapshots: Vec<NativeVolumeSnapshot>,
) -> Result<StorageInventory, StorageError> {
    build_inventory_with_disk_sizes(snapshots, &BTreeMap::new())
}

pub(crate) fn build_inventory_with_disk_sizes(
    mut snapshots: Vec<NativeVolumeSnapshot>,
    _disk_sizes: &BTreeMap<u32, u64>,
) -> Result<StorageInventory, StorageError> {
    snapshots.sort_by(|left, right| {
        left.mount_label
            .cmp(&right.mount_label)
            .then_with(|| left.volume_guid.cmp(&right.volume_guid))
    });

    let mut seen_volume_ids = BTreeSet::new();
    let mut disks = Vec::with_capacity(snapshots.len());

    for snapshot in snapshots {
        let volume_id = derive_volume_id(&snapshot);
        if !seen_volume_ids.insert(volume_id.clone()) {
            continue;
        }
        let policy = display_scan_policy(snapshot.drive_kind);
        let (scan_supported, warnings) = match policy {
            ScanPolicy::Supported => (true, Vec::new()),
            ScanPolicy::Unsupported(reason) => (false, vec![reason.warning_code().to_owned()]),
        };
        let mount_label = snapshot.mount_label.clone();
        let size_bytes = snapshot.total_bytes;
        let id = derive_storage_group_id(&volume_id);
        disks.push(StorageDisk {
            id,
            number: None,
            display_name: format!("Mounted volume {mount_label}"),
            bus_type: BusType::Unknown,
            size_bytes,
            volumes: vec![StorageVolume {
                id: volume_id,
                mount_label,
                label: sanitize_display_text(&snapshot.label, 128),
                file_system: sanitize_display_text(&snapshot.file_system, 64),
                size_bytes,
                free_bytes: snapshot.free_bytes.min(size_bytes),
                is_system: snapshot.is_system,
                scan_supported,
                folder_scope_supported: scan_supported
                    && snapshot.file_system.eq_ignore_ascii_case("NTFS"),
                warnings,
            }],
        });
    }

    let generation = derive_generation(&disks);
    Ok(StorageInventory {
        schema_version: STORAGE_SCHEMA_VERSION,
        generation,
        disks,
    })
}

pub(crate) fn build_inventory_from_query_results(
    results: Vec<Result<NativeVolumeSnapshot, StorageError>>,
) -> Result<StorageInventory, StorageError> {
    let snapshots = results.into_iter().filter_map(Result::ok).collect();
    build_inventory_with_disk_sizes(snapshots, &BTreeMap::new())
}

fn display_scan_policy(drive_kind: DriveKind) -> ScanPolicy {
    match drive_kind {
        DriveKind::Fixed | DriveKind::Removable => ScanPolicy::Supported,
        DriveKind::Remote | DriveKind::CdRom | DriveKind::RamDisk => {
            ScanPolicy::Unsupported(UnsupportedReason::NonLocalDrive)
        }
        DriveKind::Unknown | DriveKind::NoRoot => {
            ScanPolicy::Unsupported(UnsupportedReason::UnsupportedDriveType)
        }
    }
}

fn derive_storage_group_id(volume_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"undelete-master:mounted-storage-group:v1");
    update_digest_field(&mut digest, volume_id.as_bytes());
    format!("group-{}", hex::encode(digest.finalize()))
}

fn derive_generation(disks: &[StorageDisk]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"undelete-master:inventory-generation:v1");
    for disk in disks {
        update_digest_field(&mut digest, disk.id.as_bytes());
        for volume in &disk.volumes {
            update_digest_field(&mut digest, volume.id.as_bytes());
        }
    }
    format!("gen-{}", hex::encode(digest.finalize()))
}

pub(crate) fn parse_folder_identity_path(
    final_path: &str,
    selected_volume_guid: &str,
) -> Result<FolderIdentityPath, FolderScopeError> {
    let mut selected_root = selected_volume_guid.trim_end_matches('\\').to_owned();
    selected_root.push('\\');
    if final_path.len() < selected_root.len()
        || !final_path.as_bytes()[..selected_root.len()]
            .eq_ignore_ascii_case(selected_root.as_bytes())
    {
        return Err(FolderScopeError::CrossVolume);
    }

    let remainder = final_path
        .get(selected_root.len()..)
        .ok_or(FolderScopeError::UnsafeComponent)?;
    let mut relative_components = Vec::new();
    if !remainder.is_empty() {
        for component in remainder.split('\\') {
            if component.is_empty()
                || component == "."
                || component == ".."
                || component.chars().count() > 255
                || !is_safe_display_text(component)
            {
                return Err(FolderScopeError::UnsafeComponent);
            }
            relative_components.push(component.to_owned());
            if relative_components.len() > 256 {
                return Err(FolderScopeError::UnsafeComponent);
            }
        }
    }

    let display_label = relative_components
        .last()
        .map(|component| sanitize_display_text(component, 128))
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| "Selected volume".to_owned());
    Ok(FolderIdentityPath {
        relative_components,
        display_label,
    })
}

fn sanitize_display_text(value: &str, max_chars: usize) -> String {
    value
        .chars()
        .filter(|character| is_safe_display_character(*character))
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn is_safe_display_text(value: &str) -> bool {
    value.chars().all(is_safe_display_character)
}

fn is_safe_display_character(character: char) -> bool {
    !character.is_control()
        && !matches!(
            character,
            '\u{202A}'
                | '\u{202B}'
                | '\u{202C}'
                | '\u{202D}'
                | '\u{202E}'
                | '\u{2066}'
                | '\u{2067}'
                | '\u{2068}'
                | '\u{2069}'
        )
}

pub(crate) fn plan_aligned_read(
    offset: u64,
    requested_length: usize,
    source_length: u64,
    sector_size: u32,
) -> Result<AlignedReadPlan, ReadPlanError> {
    let sector_size = u64::from(sector_size);
    if sector_size == 0 {
        return Err(ReadPlanError::InvalidSectorSize);
    }
    if requested_length > MAX_RAW_READ_BYTES {
        return Err(ReadPlanError::LengthTooLarge);
    }
    let requested_length_u64 =
        u64::try_from(requested_length).map_err(|_| ReadPlanError::LengthTooLarge)?;
    let requested_end = offset
        .checked_add(requested_length_u64)
        .ok_or(ReadPlanError::Overflow)?;
    if requested_end > source_length {
        return Err(ReadPlanError::OutOfBounds);
    }
    if requested_length == 0 {
        return Ok(AlignedReadPlan {
            aligned_offset: offset,
            aligned_length: 0,
            copy_offset: 0,
            copy_length: 0,
        });
    }

    let aligned_offset = offset - (offset % sector_size);
    let rounded_end = requested_end
        .checked_add(sector_size - 1)
        .ok_or(ReadPlanError::Overflow)?;
    let aligned_end = rounded_end - (rounded_end % sector_size);
    if aligned_end > source_length {
        return Err(ReadPlanError::OutOfBounds);
    }
    let aligned_length_u64 = aligned_end
        .checked_sub(aligned_offset)
        .ok_or(ReadPlanError::Overflow)?;
    let aligned_length =
        usize::try_from(aligned_length_u64).map_err(|_| ReadPlanError::LengthTooLarge)?;
    let copy_offset_u64 = offset
        .checked_sub(aligned_offset)
        .ok_or(ReadPlanError::Overflow)?;
    let copy_offset =
        usize::try_from(copy_offset_u64).map_err(|_| ReadPlanError::LengthTooLarge)?;
    let copy_end = copy_offset
        .checked_add(requested_length)
        .ok_or(ReadPlanError::Overflow)?;
    if copy_end > aligned_length {
        return Err(ReadPlanError::OutOfBounds);
    }

    Ok(AlignedReadPlan {
        aligned_offset,
        aligned_length,
        copy_offset,
        copy_length: requested_length,
    })
}
