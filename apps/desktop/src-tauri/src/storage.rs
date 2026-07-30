use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Mutex,
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri_plugin_dialog::DialogExt;
use um_broker_client::{open_windows_source, BrokerClientError};
use um_cli::{CliError, VolumeScanDetails, VolumeScanStatus};
use um_core::{
    Candidate, CandidateKind, CandidateState, MetadataConfidence, ReadError, RecoverabilityInputs,
};
use um_fs_common::ScanError;
use um_fs_ntfs::{
    NtfsCandidatePathEvidence, NtfsDirectoryResolution, NtfsNamespace, NtfsNamespaceIndex,
    NtfsNodeRef, NtfsPathState, NtfsScopeMembership,
};
use um_io_windows::{
    BusType, FolderScope, FolderScopeError, StorageError, StorageInventory, StorageVolume,
};

const STORAGE_SCHEMA_VERSION: u32 = 1;
const PAGE_SIZE: usize = 100;
const MAX_DISKS: usize = 128;
const MAX_VOLUMES_PER_DISK: usize = 128;
const MAX_WARNINGS: usize = 128;
const MAX_TEXT_SCALARS: usize = 512;
const MAX_REQUEST_ID_SCALARS: usize = 128;
const MAX_SCOPES: usize = 32;
const MAX_SCAN_SESSIONS: usize = 4;
const REDACTED_TEXT: &str = "Unreadable metadata";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopStorageInventory {
    schema_version: u32,
    generation: String,
    disks: Vec<DesktopStorageDisk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopStorageDisk {
    id: String,
    display_name: String,
    bus_type: BusType,
    size_bytes: String,
    volumes: Vec<DesktopStorageVolume>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopStorageVolume {
    id: String,
    mount_label: String,
    label: String,
    file_system: String,
    size_bytes: String,
    free_bytes: String,
    is_system: bool,
    scan_supported: bool,
    folder_scope_supported: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopScanSummary {
    schema_version: u32,
    scan_id: String,
    source_label: String,
    scope: DesktopScanScope,
    file_system: String,
    scan_status: String,
    total_candidates: String,
    matched_candidates: String,
    unknown_candidates: String,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopScanScope {
    kind: &'static str,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopCandidateRow {
    id: String,
    display_path: String,
    kind: &'static str,
    state: &'static str,
    size_bytes: String,
    metadata_confidence: &'static str,
    recoverability_score: Option<u8>,
    path_state: &'static str,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopCandidatePage {
    schema_version: u32,
    scan_id: String,
    cursor: Option<String>,
    next_cursor: Option<String>,
    candidates: Vec<DesktopCandidateRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DesktopStorageError {
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
}

impl DesktopStorageError {
    const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }

    const fn incompatible() -> Self {
        Self::new(
            "REPORT_INCOMPATIBLE",
            "The native scanner returned an incompatible bounded result.",
        )
    }

    const fn folder_mismatch() -> Self {
        Self::new(
            "FOLDER_SCOPE_MISMATCH",
            "The selected folder could not be proven inside this scan.",
        )
    }
}

pub(crate) enum SessionScope<'a> {
    Volume,
    Folder(&'a FolderScope),
}

pub(crate) struct ScanSession {
    pub(crate) summary: DesktopScanSummary,
    rows: Vec<DesktopCandidateRow>,
    cursors: BTreeMap<String, usize>,
    cursor_for_offset: BTreeMap<usize, String>,
}

#[derive(Default)]
pub(crate) struct DesktopStorageState {
    inner: Mutex<DesktopStorageStateInner>,
}

#[derive(Default)]
struct DesktopStorageStateInner {
    scopes: HashMap<String, StoredFolderScope>,
    scope_order: VecDeque<String>,
    scans: HashMap<String, ScanSession>,
    scan_order: VecDeque<String>,
}

struct StoredFolderScope {
    generation: String,
    scope: FolderScope,
}

impl DesktopStorageState {
    pub(crate) fn store_scope(
        &self,
        scope: FolderScope,
        generation: &str,
    ) -> Result<(), DesktopStorageError> {
        const NTFS_RECORD_MASK: u64 = (1_u64 << 48) - 1;
        let identity_consistent = scope.ntfs_record == scope.file_index & NTFS_RECORD_MASK
            && scope.ntfs_sequence == (scope.file_index >> 48) as u16;
        if !valid_opaque_id(generation)
            || !valid_opaque_id(&scope.scope_id)
            || !valid_opaque_id(&scope.volume_id)
            || !identity_consistent
            || scope.relative_components.len() > 256
            || scope.relative_components.iter().any(|component| {
                component.is_empty()
                    || component.chars().count() > 255
                    || sanitize_text(component) != *component
            })
        {
            return Err(DesktopStorageError::folder_mismatch());
        }
        let mut state = self.inner.lock().map_err(|_| {
            DesktopStorageError::new("SCAN_INTERNAL", "The scanner state is unavailable.")
        })?;
        if !state.scopes.contains_key(&scope.scope_id) {
            state.scope_order.push_back(scope.scope_id.clone());
        }
        state.scopes.insert(
            scope.scope_id.clone(),
            StoredFolderScope {
                generation: generation.to_owned(),
                scope,
            },
        );
        while state.scope_order.len() > MAX_SCOPES {
            if let Some(expired) = state.scope_order.pop_front() {
                state.scopes.remove(&expired);
            }
        }
        Ok(())
    }

    pub(crate) fn scope_for_volume(
        &self,
        scope_id: &str,
        volume_id: &str,
        generation: &str,
    ) -> Result<FolderScope, DesktopStorageError> {
        if !valid_opaque_id(scope_id) || !valid_opaque_id(volume_id) || !valid_opaque_id(generation)
        {
            return Err(DesktopStorageError::folder_mismatch());
        }
        let state = self.inner.lock().map_err(|_| {
            DesktopStorageError::new("SCAN_INTERNAL", "The scanner state is unavailable.")
        })?;
        let scope = state
            .scopes
            .get(scope_id)
            .filter(|stored| stored.generation == generation && stored.scope.volume_id == volume_id)
            .ok_or_else(DesktopStorageError::folder_mismatch)?;
        Ok(scope.scope.clone())
    }

    pub(crate) fn store_session(
        &self,
        session: ScanSession,
    ) -> Result<DesktopScanSummary, DesktopStorageError> {
        let scan_id = session.summary.scan_id.clone();
        let summary = session.summary.clone();
        let mut state = self.inner.lock().map_err(|_| {
            DesktopStorageError::new("SCAN_INTERNAL", "The scanner state is unavailable.")
        })?;
        if !state.scans.contains_key(&scan_id) {
            state.scan_order.push_back(scan_id.clone());
        }
        state.scans.insert(scan_id, session);
        while state.scan_order.len() > MAX_SCAN_SESSIONS {
            if let Some(expired) = state.scan_order.pop_front() {
                state.scans.remove(&expired);
            }
        }
        Ok(summary)
    }

    pub(crate) fn candidate_page(
        &self,
        scan_id: &str,
        cursor: Option<&str>,
        limit: u16,
    ) -> Result<DesktopCandidatePage, DesktopStorageError> {
        if !valid_opaque_id(scan_id) {
            return Err(DesktopStorageError::incompatible());
        }
        let state = self.inner.lock().map_err(|_| {
            DesktopStorageError::new("SCAN_INTERNAL", "The scanner state is unavailable.")
        })?;
        let session = state
            .scans
            .get(scan_id)
            .ok_or_else(DesktopStorageError::incompatible)?;
        page_for_session(session, cursor, limit)
    }
}

pub(crate) fn validate_request_id(request_id: &str) -> Result<(), DesktopStorageError> {
    let valid = !request_id.is_empty()
        && request_id.chars().count() <= MAX_REQUEST_ID_SCALARS
        && request_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(DesktopStorageError::new(
            "SCAN_INTERNAL",
            "The request identifier is invalid.",
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopFolderSelection {
    schema_version: u32,
    scope_id: String,
    volume_id: String,
    label: String,
}

#[tauri::command]
pub(crate) fn list_storage_sources(
    request_id: String,
) -> Result<DesktopStorageInventory, DesktopStorageError> {
    validate_request_id(&request_id)?;
    let inventory = um_io_windows::enumerate_storage().map_err(map_inventory_error)?;
    adapt_inventory(inventory)
}

#[tauri::command]
pub(crate) fn select_scan_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, DesktopStorageState>,
    request_id: String,
    generation: String,
    volume_id: String,
) -> Result<Option<DesktopFolderSelection>, DesktopStorageError> {
    validate_request_id(&request_id)?;

    let inventory = um_io_windows::enumerate_storage().map_err(map_inventory_error)?;
    let volume = authorize_inventory_generation(&inventory, &generation, &volume_id)?;
    if !volume.scan_supported || !volume.folder_scope_supported {
        return Err(DesktopStorageError::new(
            "FOLDER_SCOPE_UNSUPPORTED",
            "This source does not support a verifiable folder scope.",
        ));
    }

    let selected = app
        .dialog()
        .file()
        .set_title("Choose a folder to filter recoverable candidates")
        .blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|_| DesktopStorageError::folder_mismatch())?;
    let scope =
        um_io_windows::validate_folder_scope(&volume_id, &path).map_err(map_folder_error)?;
    let response = DesktopFolderSelection {
        schema_version: STORAGE_SCHEMA_VERSION,
        scope_id: scope.scope_id.clone(),
        volume_id: scope.volume_id.clone(),
        label: sanitize_required(&scope.display_label),
    };
    state.store_scope(scope, &generation)?;
    Ok(Some(response))
}

#[tauri::command]
pub(crate) async fn scan_storage_volume(
    state: tauri::State<'_, DesktopStorageState>,
    request_id: String,
    generation: String,
    volume_id: String,
    scope_id: Option<String>,
) -> Result<DesktopScanSummary, DesktopStorageError> {
    validate_request_id(&request_id)?;
    if let Some(scope_id) = scope_id.as_deref() {
        validate_opaque_argument(scope_id)?;
    }

    let inventory = um_io_windows::enumerate_storage().map_err(map_inventory_error)?;
    let volume = authorize_inventory_generation(&inventory, &generation, &volume_id)?;
    if !volume.scan_supported {
        return Err(DesktopStorageError::new(
            "SOURCE_UNSUPPORTED",
            "This source is not eligible for read-only RAW scanning.",
        ));
    }
    let source_label = volume_source_label(volume);
    let folder_scope = scope_id
        .as_deref()
        .map(|scope| state.scope_for_volume(scope, &volume_id, &generation))
        .transpose()?;

    let broker_volume_id = volume_id.clone();
    let details = tauri::async_runtime::spawn_blocking(move || {
        let reader = open_windows_source(&broker_volume_id).map_err(map_broker_error)?;
        um_cli::scan_volume_reader(&reader).map_err(map_cli_scan_error)
    })
    .await
    .map_err(|_| {
        DesktopStorageError::new(
            "SCAN_INTERNAL",
            "The native scan worker could not complete.",
        )
    })??;

    let scan_id = create_scan_id()?;
    let scope = folder_scope
        .as_ref()
        .map_or(SessionScope::Volume, SessionScope::Folder);
    let session = build_scan_session(&scan_id, &source_label, details, scope)?;
    state.store_session(session)
}

#[tauri::command]
pub(crate) fn get_candidate_page(
    state: tauri::State<'_, DesktopStorageState>,
    request_id: String,
    scan_id: String,
    cursor: Option<String>,
    limit: u16,
) -> Result<DesktopCandidatePage, DesktopStorageError> {
    validate_request_id(&request_id)?;
    validate_opaque_argument(&scan_id)?;
    if let Some(cursor) = cursor.as_deref() {
        validate_opaque_argument(cursor)?;
    }
    state.candidate_page(&scan_id, cursor.as_deref(), limit)
}

fn validate_opaque_argument(value: &str) -> Result<(), DesktopStorageError> {
    if valid_opaque_id(value) {
        Ok(())
    } else {
        Err(DesktopStorageError::new(
            "SCAN_INTERNAL",
            "The native request contains an invalid opaque identifier.",
        ))
    }
}

fn find_volume<'a>(
    inventory: &'a StorageInventory,
    volume_id: &str,
) -> Result<&'a StorageVolume, DesktopStorageError> {
    inventory
        .disks
        .iter()
        .flat_map(|disk| &disk.volumes)
        .find(|volume| volume.id == volume_id)
        .ok_or_else(|| {
            DesktopStorageError::new("SOURCE_GONE", "The selected source is no longer connected.")
        })
}

fn authorize_inventory_generation<'a>(
    inventory: &'a StorageInventory,
    generation: &str,
    volume_id: &str,
) -> Result<&'a StorageVolume, DesktopStorageError> {
    validate_opaque_argument(generation)?;
    validate_opaque_argument(volume_id)?;
    if inventory.generation != generation {
        return Err(DesktopStorageError::new(
            "SOURCE_IDENTITY_CHANGED",
            "The connected-storage inventory changed. Refresh it before scanning.",
        ));
    }
    find_volume(inventory, volume_id)
}

fn volume_source_label(volume: &StorageVolume) -> String {
    let label = sanitize_text(&volume.label);
    if label.is_empty() {
        volume.mount_label.clone()
    } else {
        sanitize_required(&format!("{} · {label}", volume.mount_label))
    }
}

fn create_scan_id() -> Result<String, DesktopStorageError> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| {
        DesktopStorageError::new(
            "SCAN_INTERNAL",
            "The scan session could not be initialized.",
        )
    })?;
    Ok(format!("scan-{}", hex::encode(random)))
}

fn map_inventory_error(_error: StorageError) -> DesktopStorageError {
    DesktopStorageError::new(
        "INVENTORY_UNAVAILABLE",
        "Windows storage inventory is unavailable.",
    )
}

fn map_folder_error(error: StorageError) -> DesktopStorageError {
    match error {
        StorageError::UnsupportedSource(_) => DesktopStorageError::new(
            "FOLDER_SCOPE_UNSUPPORTED",
            "This source does not support a verifiable folder scope.",
        ),
        StorageError::SourceIdentityUnavailable | StorageError::SourceIdentityChanged => {
            DesktopStorageError::new(
                "SOURCE_IDENTITY_CHANGED",
                "The selected source identity changed.",
            )
        }
        StorageError::FolderScope(
            FolderScopeError::UnsupportedRoot
            | FolderScopeError::NonLocal
            | FolderScopeError::ReparsePoint
            | FolderScopeError::CrossVolume
            | FolderScopeError::UnsafeComponent
            | FolderScopeError::NotDirectory
            | FolderScopeError::IdentityUnavailable,
        ) => DesktopStorageError::folder_mismatch(),
        _ => DesktopStorageError::folder_mismatch(),
    }
}

fn map_broker_error(error: BrokerClientError) -> DesktopStorageError {
    match error {
        BrokerClientError::ElevationDenied => DesktopStorageError::new(
            "UAC_CANCELLED",
            "The Windows elevation request was canceled.",
        ),
        BrokerClientError::SourceNotFound | BrokerClientError::SourceUnavailable => {
            DesktopStorageError::new("SOURCE_GONE", "The selected source is no longer available.")
        }
        BrokerClientError::SourceIdentityMismatch | BrokerClientError::SourceChanged => {
            DesktopStorageError::new(
                "SOURCE_IDENTITY_CHANGED",
                "The selected source identity changed.",
            )
        }
        BrokerClientError::ProtocolFailure
        | BrokerClientError::UnexpectedResponse
        | BrokerClientError::AuthenticationFailed
        | BrokerClientError::PeerVerificationFailed
        | BrokerClientError::InvalidHandle => DesktopStorageError::new(
            "BROKER_PROTOCOL",
            "The read-only broker session failed closed.",
        ),
        BrokerClientError::ReadOutOfRange
        | BrokerClientError::ReadFailed
        | BrokerClientError::ShortRead => DesktopStorageError::new(
            "SOURCE_IO",
            "The selected source could not be read completely.",
        ),
        BrokerClientError::TransportFailure
        | BrokerClientError::SessionUnavailable
        | BrokerClientError::BrokerUnavailable
        | BrokerClientError::RandomnessUnavailable
        | BrokerClientError::UnsupportedPlatform => {
            DesktopStorageError::new("BROKER_UNAVAILABLE", "The read-only broker is unavailable.")
        }
        BrokerClientError::InvalidRequest | BrokerClientError::InternalFailure => {
            DesktopStorageError::new(
                "SCAN_INTERNAL",
                "The native scan request could not be completed.",
            )
        }
    }
}

fn map_cli_scan_error(error: CliError) -> DesktopStorageError {
    match error {
        CliError::VolumeScan {
            source: ScanError::Read(ReadError::SourceGone),
            ..
        } => DesktopStorageError::new(
            "SOURCE_IDENTITY_CHANGED",
            "The selected source identity changed during the scan.",
        ),
        CliError::VolumeScan {
            source: ScanError::Read(_),
            ..
        }
        | CliError::EmptyImage => DesktopStorageError::new(
            "SOURCE_IO",
            "The selected source could not be read completely.",
        ),
        CliError::VolumeScan { .. } => DesktopStorageError::new(
            "SCAN_CORRUPT",
            "The source contains invalid or unsupported filesystem structures.",
        ),
        CliError::ForbiddenSourcePath
        | CliError::UnsupportedExtension { .. }
        | CliError::NotRegularFile
        | CliError::Metadata(_)
        | CliError::Open(_)
        | CliError::Partition(_)
        | CliError::InvalidRegion { .. } => DesktopStorageError::new(
            "SCAN_INTERNAL",
            "The native volume scanner returned an unexpected failure.",
        ),
    }
}

pub(crate) fn adapt_inventory(
    inventory: StorageInventory,
) -> Result<DesktopStorageInventory, DesktopStorageError> {
    if inventory.schema_version != STORAGE_SCHEMA_VERSION
        || inventory.disks.len() > MAX_DISKS
        || !valid_opaque_id(&inventory.generation)
    {
        return Err(DesktopStorageError::incompatible());
    }

    let mut disk_ids = HashSet::new();
    let disks = inventory
        .disks
        .into_iter()
        .map(|disk| {
            if !valid_opaque_id(&disk.id)
                || !disk_ids.insert(disk.id.clone())
                || disk.volumes.len() > MAX_VOLUMES_PER_DISK
            {
                return Err(DesktopStorageError::incompatible());
            }
            let mut volume_ids = HashSet::new();
            let volumes = disk
                .volumes
                .into_iter()
                .map(|volume| {
                    if !valid_opaque_id(&volume.id)
                        || !volume_ids.insert(volume.id.clone())
                        || !valid_mount_label(&volume.mount_label)
                    {
                        return Err(DesktopStorageError::incompatible());
                    }
                    adapt_volume(volume)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(DesktopStorageDisk {
                id: disk.id,
                display_name: sanitize_required(&disk.display_name),
                bus_type: disk.bus_type,
                size_bytes: disk.size_bytes.to_string(),
                volumes,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DesktopStorageInventory {
        schema_version: STORAGE_SCHEMA_VERSION,
        generation: inventory.generation,
        disks,
    })
}

fn adapt_volume(volume: StorageVolume) -> Result<DesktopStorageVolume, DesktopStorageError> {
    if volume.free_bytes > volume.size_bytes || volume.warnings.len() > MAX_WARNINGS {
        return Err(DesktopStorageError::incompatible());
    }
    Ok(DesktopStorageVolume {
        id: volume.id,
        mount_label: volume.mount_label,
        label: sanitize_text(&volume.label),
        file_system: sanitize_required(&volume.file_system),
        size_bytes: volume.size_bytes.to_string(),
        free_bytes: volume.free_bytes.to_string(),
        is_system: volume.is_system,
        scan_supported: volume.scan_supported,
        folder_scope_supported: volume.folder_scope_supported,
        warnings: sanitize_warnings(volume.warnings),
    })
}

pub(crate) fn build_scan_session(
    scan_id: &str,
    source_label: &str,
    details: VolumeScanDetails,
    scope: SessionScope<'_>,
) -> Result<ScanSession, DesktopStorageError> {
    if !valid_opaque_id(scan_id)
        || details.report.candidate_count != details.candidates.len()
        || details.report.warnings.len() > MAX_WARNINGS
    {
        return Err(DesktopStorageError::incompatible());
    }

    let file_system = match details.report.file_system.as_str() {
        "ntfs" | "fat12" | "fat16" | "fat32" | "unrecognized" => details.report.file_system.clone(),
        _ => return Err(DesktopStorageError::incompatible()),
    };
    let scan_status = match (&*file_system, details.report.scan_status) {
        ("unrecognized", VolumeScanStatus::Unrecognized) => "unrecognized",
        ("ntfs" | "fat12" | "fat16" | "fat32", VolumeScanStatus::Complete) => "complete",
        ("ntfs" | "fat12" | "fat16" | "fat32", VolumeScanStatus::Partial) => "partial",
        _ => return Err(DesktopStorageError::incompatible()),
    };

    let total_candidates = details.candidates.len();
    let namespace_index = details
        .ntfs_namespace
        .as_ref()
        .map(NtfsNamespace::build_index)
        .transpose()
        .map_err(|_| DesktopStorageError::incompatible())?;
    let mut unknown_candidates = 0usize;
    let mut selected = Vec::new();
    let (scope_kind, scope_label) = match scope {
        SessionScope::Volume => {
            selected = details.candidates.iter().collect();
            ("volume", sanitize_required(source_label))
        }
        SessionScope::Folder(folder) => {
            if file_system != "ntfs" {
                return Err(DesktopStorageError::new(
                    "FOLDER_SCOPE_UNSUPPORTED",
                    "Folder scope currently requires an NTFS volume.",
                ));
            }
            let namespace = details
                .ntfs_namespace
                .as_ref()
                .ok_or_else(DesktopStorageError::folder_mismatch)?;
            let index = namespace_index
                .as_ref()
                .ok_or_else(DesktopStorageError::folder_mismatch)?;
            let directory = match namespace.resolve_active_directory(&folder.relative_components) {
                NtfsDirectoryResolution::Unique(node) => node,
                NtfsDirectoryResolution::NotFound
                | NtfsDirectoryResolution::Ambiguous
                | NtfsDirectoryResolution::Unknown => {
                    return Err(DesktopStorageError::folder_mismatch());
                }
            };
            if !folder_file_reference_matches(directory, folder) {
                return Err(DesktopStorageError::folder_mismatch());
            }
            for candidate in &details.candidates {
                match candidate_membership(index, candidate, directory) {
                    NtfsScopeMembership::Match => selected.push(candidate),
                    NtfsScopeMembership::NoMatch => {}
                    NtfsScopeMembership::Unknown => {
                        unknown_candidates = unknown_candidates.saturating_add(1);
                    }
                }
            }
            ("folder", sanitize_required(&folder.display_label))
        }
    };

    let rows = selected
        .into_iter()
        .map(|candidate| adapt_candidate(scan_id, candidate, namespace_index.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    let matched_candidates = rows.len();
    let (cursors, cursor_for_offset) = build_cursors(scan_id, rows.len());

    Ok(ScanSession {
        summary: DesktopScanSummary {
            schema_version: STORAGE_SCHEMA_VERSION,
            scan_id: scan_id.to_owned(),
            source_label: sanitize_required(source_label),
            scope: DesktopScanScope {
                kind: scope_kind,
                label: scope_label,
            },
            file_system,
            scan_status: scan_status.to_owned(),
            total_candidates: total_candidates.to_string(),
            matched_candidates: matched_candidates.to_string(),
            unknown_candidates: unknown_candidates.to_string(),
            warnings: sanitize_warnings(details.report.warnings),
        },
        rows,
        cursors,
        cursor_for_offset,
    })
}

fn folder_file_reference_matches(directory: NtfsNodeRef, folder: &FolderScope) -> bool {
    directory.record == folder.ntfs_record && directory.sequence == folder.ntfs_sequence
}

fn candidate_membership(
    namespace: &NtfsNamespaceIndex<'_>,
    candidate: &Candidate,
    directory: NtfsNodeRef,
) -> NtfsScopeMembership {
    let Some(sequence) = candidate.sequence else {
        return NtfsScopeMembership::Unknown;
    };
    namespace.classify_candidate(
        NtfsNodeRef {
            record: candidate.record_ref,
            sequence,
        },
        directory,
    )
}

fn adapt_candidate(
    scan_id: &str,
    candidate: &Candidate,
    namespace: Option<&NtfsNamespaceIndex<'_>>,
) -> Result<DesktopCandidateRow, DesktopStorageError> {
    if candidate.warnings.len() > MAX_WARNINGS {
        return Err(DesktopStorageError::incompatible());
    }
    let kind = match candidate.kind {
        CandidateKind::File => "file",
        CandidateKind::Directory => "directory",
    };
    let state = match candidate.state {
        CandidateState::ExactEvidence => "exactEvidence",
        CandidateState::LikelyComplete => "likelyComplete",
        CandidateState::CompleteUnvalidated => "completeUnvalidated",
        CandidateState::StructurallyValid => "structurallyValid",
        CandidateState::Partial => "partial",
        CandidateState::Conflicted => "conflicted",
        CandidateState::ReadErrorState => "readError",
        CandidateState::ZeroedOrTrimmed => "zeroedOrTrimmed",
        CandidateState::Overwritten => "overwritten",
        CandidateState::MetadataOnly => "metadataOnly",
        CandidateState::Unknown => "unknown",
    };
    let metadata_confidence = match candidate.metadata_confidence {
        MetadataConfidence::High => "high",
        MetadataConfidence::Medium => "medium",
        MetadataConfidence::Low => "low",
    };
    let recoverability_score = (candidate.kind == CandidateKind::File).then(|| {
        RecoverabilityInputs::from_candidate(candidate)
            .score()
            .value
    });
    let row_id = derive_opaque_id("cand", &[scan_id, &candidate.id.to_string()]);
    let display_path = sanitize_candidate_path(&candidate.display_path(), &row_id);

    Ok(DesktopCandidateRow {
        id: row_id,
        display_path,
        kind,
        state,
        size_bytes: candidate.size.to_string(),
        metadata_confidence,
        recoverability_score,
        path_state: candidate_path_state(candidate, namespace),
        warnings: sanitize_warnings(candidate.warnings.clone()),
    })
}

fn candidate_path_state(
    candidate: &Candidate,
    namespace: Option<&NtfsNamespaceIndex<'_>>,
) -> &'static str {
    let Some(namespace) = namespace else {
        return if candidate.name_certain {
            "reconstructed"
        } else {
            "incomplete"
        };
    };
    let Some(sequence) = candidate.sequence else {
        return "incomplete";
    };
    let node = NtfsNodeRef {
        record: candidate.record_ref,
        sequence,
    };
    match namespace.candidate_path_evidence(node) {
        NtfsCandidatePathEvidence::Multiple => "ambiguous",
        NtfsCandidatePathEvidence::One(NtfsPathState::Exact) => "exact",
        NtfsCandidatePathEvidence::One(NtfsPathState::Reconstructed) => "reconstructed",
        NtfsCandidatePathEvidence::One(NtfsPathState::Incomplete) => "incomplete",
        NtfsCandidatePathEvidence::One(NtfsPathState::Orphaned) => "orphaned",
        NtfsCandidatePathEvidence::One(NtfsPathState::Ambiguous) => "ambiguous",
        NtfsCandidatePathEvidence::Missing if candidate.name_certain => "reconstructed",
        NtfsCandidatePathEvidence::Missing => "incomplete",
    }
}

fn build_cursors(
    scan_id: &str,
    row_count: usize,
) -> (BTreeMap<String, usize>, BTreeMap<usize, String>) {
    let mut cursors = BTreeMap::new();
    let mut cursor_for_offset = BTreeMap::new();
    for offset in (PAGE_SIZE..row_count).step_by(PAGE_SIZE) {
        let cursor = derive_opaque_id("cur", &[scan_id, &offset.to_string()]);
        cursors.insert(cursor.clone(), offset);
        cursor_for_offset.insert(offset, cursor);
    }
    (cursors, cursor_for_offset)
}

pub(crate) fn page_for_session(
    session: &ScanSession,
    cursor: Option<&str>,
    limit: u16,
) -> Result<DesktopCandidatePage, DesktopStorageError> {
    if usize::from(limit) != PAGE_SIZE {
        return Err(DesktopStorageError::incompatible());
    }
    let offset = match cursor {
        None => 0,
        Some(value) => session
            .cursors
            .get(value)
            .copied()
            .ok_or_else(DesktopStorageError::incompatible)?,
    };
    let end = offset
        .checked_add(PAGE_SIZE)
        .map(|value| value.min(session.rows.len()))
        .ok_or_else(DesktopStorageError::incompatible)?;
    let candidates = session
        .rows
        .get(offset..end)
        .ok_or_else(DesktopStorageError::incompatible)?
        .to_vec();
    let next_cursor = if end < session.rows.len() {
        Some(
            session
                .cursor_for_offset
                .get(&end)
                .cloned()
                .ok_or_else(DesktopStorageError::incompatible)?,
        )
    } else {
        None
    };

    Ok(DesktopCandidatePage {
        schema_version: STORAGE_SCHEMA_VERSION,
        scan_id: session.summary.scan_id.clone(),
        cursor: cursor.map(str::to_owned),
        next_cursor,
        candidates,
    })
}

fn derive_opaque_id(prefix: &str, fields: &[&str]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"undelete-master:desktop-id:v1");
    for field in fields {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field.as_bytes());
    }
    format!("{prefix}-{}", hex::encode(digest.finalize()))
}

fn valid_opaque_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn valid_mount_label(value: &str) -> bool {
    value.len() == 2 && value.as_bytes()[0].is_ascii_alphabetic() && value.as_bytes()[1] == b':'
}

fn sanitize_warnings(warnings: Vec<String>) -> Vec<String> {
    warnings
        .into_iter()
        .take(MAX_WARNINGS)
        .map(|warning| sanitize_required(&warning))
        .collect()
}

fn sanitize_required(value: &str) -> String {
    let sanitized = sanitize_text(value);
    if sanitized.trim().is_empty() {
        REDACTED_TEXT.to_owned()
    } else {
        sanitized
    }
}

fn sanitize_candidate_path(value: &str, candidate_id: &str) -> String {
    const REFERENCE_SCALARS: usize = 12;

    let sanitized = value
        .chars()
        .filter(|character| {
            !character.is_control() && !is_bidirectional_formatting_control(*character)
        })
        .take(MAX_TEXT_SCALARS + 1)
        .collect::<Vec<_>>();
    if sanitized.len() <= MAX_TEXT_SCALARS {
        let path = sanitized.into_iter().collect::<String>().trim().to_owned();
        return if path.is_empty() {
            REDACTED_TEXT.to_owned()
        } else {
            path
        };
    }

    let mut reference = candidate_id
        .rsplit_once('-')
        .map_or(candidate_id, |(_, digest)| digest)
        .chars()
        .rev()
        .take(REFERENCE_SCALARS)
        .collect::<Vec<_>>();
    reference.reverse();
    let suffix = format!("… [ref {}]", reference.into_iter().collect::<String>());
    let prefix_limit = MAX_TEXT_SCALARS.saturating_sub(suffix.chars().count());
    let prefix = sanitized
        .into_iter()
        .take(prefix_limit)
        .collect::<String>()
        .trim_end()
        .to_owned();
    let prefix = if prefix.is_empty() {
        REDACTED_TEXT
    } else {
        &prefix
    };
    format!("{prefix}{suffix}")
}

fn sanitize_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_control() && !is_bidirectional_formatting_control(*character)
        })
        .take(MAX_TEXT_SCALARS)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn is_bidirectional_formatting_control(character: char) -> bool {
    matches!(
        character,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'
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

#[cfg(test)]
mod tests {
    use um_cli::{VolumeReport, VolumeScanDetails, VolumeScanStatus};
    use um_core::{
        Candidate, CandidateKind, CandidateState, DiscoveryMethod, MetadataConfidence, Timestamps,
    };
    use um_fs_ntfs::{
        NtfsDirectoryNode, NtfsNamespace, NtfsNamespacePath, NtfsNodeRef, NtfsPathState,
    };
    use um_io_windows::{BusType, FolderScope, StorageDisk, StorageInventory, StorageVolume};

    use super::*;

    fn candidate(
        id: u64,
        record: u64,
        sequence: Option<u16>,
        name: &str,
        kind: CandidateKind,
    ) -> Candidate {
        Candidate {
            id,
            kind,
            method: DiscoveryMethod::NtfsMetadata,
            state: CandidateState::CompleteUnvalidated,
            name: name.to_owned(),
            name_certain: true,
            parent_path: vec!["Users".to_owned(), "Alice".to_owned()],
            metadata_confidence: MetadataConfidence::High,
            size: if kind == CandidateKind::File { 64 } else { 0 },
            timestamps: Timestamps::default(),
            extents: Vec::new(),
            record_ref: record,
            sequence,
            warnings: Vec::new(),
        }
    }

    fn details(candidates: Vec<Candidate>, namespace: NtfsNamespace) -> VolumeScanDetails {
        VolumeScanDetails {
            report: VolumeReport {
                index: 0,
                offset_bytes: 0,
                length_bytes: 4_096,
                file_system: "ntfs".to_owned(),
                scan_status: VolumeScanStatus::Complete,
                candidate_count: candidates.len(),
                warnings: Vec::new(),
            },
            candidates,
            ntfs_namespace: Some(namespace),
        }
    }

    #[test]
    fn desktop_inventory_001_serializes_u64_as_decimal_strings_and_no_native_paths() {
        let inventory = StorageInventory {
            schema_version: 1,
            generation: "gen-fixture".to_owned(),
            disks: vec![StorageDisk {
                id: "disk-fixture".to_owned(),
                number: Some(7),
                display_name: "External disk".to_owned(),
                bus_type: BusType::Usb,
                size_bytes: u64::MAX,
                volumes: vec![StorageVolume {
                    id: "vol-fixture".to_owned(),
                    mount_label: "E:".to_owned(),
                    label: "Recovery".to_owned(),
                    file_system: "NTFS".to_owned(),
                    size_bytes: 9_007_199_254_740_993,
                    free_bytes: 1,
                    is_system: false,
                    scan_supported: true,
                    folder_scope_supported: true,
                    warnings: Vec::new(),
                }],
            }],
        };

        let desktop = adapt_inventory(inventory).expect("adapt inventory");
        let json = serde_json::to_value(desktop).expect("serialize inventory");

        assert_eq!(json["disks"][0]["sizeBytes"], u64::MAX.to_string());
        assert_eq!(
            json["disks"][0]["volumes"][0]["sizeBytes"],
            "9007199254740993"
        );
        assert!(
            json["disks"][0].get("number").is_none(),
            "the WebView contract must not expose the native disk number"
        );
        let serialized = json.to_string();
        assert!(!serialized.contains(r"\\.\"));
        assert!(!serialized.contains(r"\\?\"));
    }

    #[test]
    fn desktop_inventory_generation_001_rejects_stale_scan_authority() {
        let inventory = StorageInventory {
            schema_version: 1,
            generation: "gen-current".to_owned(),
            disks: vec![StorageDisk {
                id: "disk-fixture".to_owned(),
                number: Some(7),
                display_name: "External disk".to_owned(),
                bus_type: BusType::Usb,
                size_bytes: 1_000_000,
                volumes: vec![StorageVolume {
                    id: "vol-fixture".to_owned(),
                    mount_label: "E:".to_owned(),
                    label: "Recovery".to_owned(),
                    file_system: "NTFS".to_owned(),
                    size_bytes: 900_000,
                    free_bytes: 400_000,
                    is_system: false,
                    scan_supported: true,
                    folder_scope_supported: true,
                    warnings: Vec::new(),
                }],
            }],
        };

        assert_eq!(
            authorize_inventory_generation(&inventory, "gen-current", "vol-fixture")
                .expect("current generation")
                .id,
            "vol-fixture"
        );
        let stale = authorize_inventory_generation(&inventory, "gen-stale", "vol-fixture")
            .expect_err("stale generation must fail before broker launch");
        assert_eq!(stale.code, "SOURCE_IDENTITY_CHANGED");
        for invalid in ["bad\ngeneration".to_owned(), "g".repeat(129)] {
            let error = authorize_inventory_generation(&inventory, &invalid, "vol-fixture")
                .expect_err("generation must be a bounded opaque token");
            assert_eq!(error.code, "SCAN_INTERNAL");
        }
    }

    #[test]
    fn desktop_folder_scope_001_lists_only_proven_members_and_counts_unknown_separately() {
        let directory = NtfsNodeRef {
            record: 42,
            sequence: 3,
        };
        let matching = NtfsNodeRef {
            record: 50,
            sequence: 7,
        };
        let outside = NtfsNodeRef {
            record: 51,
            sequence: 8,
        };
        let namespace = NtfsNamespace {
            paths: vec![
                NtfsNamespacePath {
                    node: directory,
                    namespace: 1,
                    name: "Documents".to_owned(),
                    parent_path: vec!["Users".to_owned(), "Alice".to_owned()],
                    ancestors: vec![NtfsNodeRef {
                        record: 5,
                        sequence: 1,
                    }],
                    state: NtfsPathState::Exact,
                },
                NtfsNamespacePath {
                    node: matching,
                    namespace: 1,
                    name: "inside.txt".to_owned(),
                    parent_path: vec![
                        "Users".to_owned(),
                        "Alice".to_owned(),
                        "Documents".to_owned(),
                    ],
                    ancestors: vec![
                        NtfsNodeRef {
                            record: 5,
                            sequence: 1,
                        },
                        directory,
                    ],
                    state: NtfsPathState::Exact,
                },
                NtfsNamespacePath {
                    node: outside,
                    namespace: 1,
                    name: "outside.txt".to_owned(),
                    parent_path: vec!["Elsewhere".to_owned()],
                    ancestors: vec![NtfsNodeRef {
                        record: 5,
                        sequence: 1,
                    }],
                    state: NtfsPathState::Exact,
                },
            ],
            directories: vec![NtfsDirectoryNode {
                node: directory,
                active: true,
            }],
            is_complete: true,
        };
        let candidates = vec![
            candidate(
                1,
                matching.record,
                Some(matching.sequence),
                "inside.txt",
                CandidateKind::File,
            ),
            candidate(
                2,
                outside.record,
                Some(outside.sequence),
                "outside.txt",
                CandidateKind::File,
            ),
            candidate(3, 52, None, "uncertain.txt", CandidateKind::File),
        ];
        let scope = FolderScope {
            scope_id: "scope-fixture".to_owned(),
            volume_id: "vol-fixture".to_owned(),
            display_label: "Documents".to_owned(),
            relative_components: vec![
                "Users".to_owned(),
                "Alice".to_owned(),
                "Documents".to_owned(),
            ],
            volume_serial: 7,
            file_index: (3_u64 << 48) | 42,
            ntfs_record: 42,
            ntfs_sequence: 3,
        };

        let session = build_scan_session(
            "scan-fixture",
            "E: Recovery",
            details(candidates, namespace),
            SessionScope::Folder(&scope),
        )
        .expect("build folder-scoped session");

        assert_eq!(session.summary.total_candidates, "3");
        assert_eq!(session.summary.matched_candidates, "1");
        assert_eq!(session.summary.unknown_candidates, "1");
        assert_eq!(session.rows.len(), 1);
        assert!(session.rows[0].display_path.ends_with("inside.txt"));
    }

    #[test]
    fn desktop_folder_scope_002_rejects_a_path_match_with_the_wrong_directory_identity() {
        let directory = NtfsNodeRef {
            record: 42,
            sequence: 3,
        };
        let namespace = NtfsNamespace {
            paths: vec![NtfsNamespacePath {
                node: directory,
                namespace: 1,
                name: "Documents".to_owned(),
                parent_path: vec!["Users".to_owned(), "Alice".to_owned()],
                ancestors: vec![NtfsNodeRef {
                    record: 5,
                    sequence: 1,
                }],
                state: NtfsPathState::Exact,
            }],
            directories: vec![NtfsDirectoryNode {
                node: directory,
                active: true,
            }],
            is_complete: true,
        };
        let scope = FolderScope {
            scope_id: "scope-stale".to_owned(),
            volume_id: "vol-fixture".to_owned(),
            display_label: "Documents".to_owned(),
            relative_components: vec![
                "Users".to_owned(),
                "Alice".to_owned(),
                "Documents".to_owned(),
            ],
            volume_serial: 7,
            file_index: (3_u64 << 48) | 99,
            ntfs_record: 99,
            ntfs_sequence: 3,
        };

        let result = build_scan_session(
            "scan-stale-scope",
            "E: Recovery",
            details(Vec::new(), namespace),
            SessionScope::Folder(&scope),
        );

        assert!(result.is_err());
    }

    #[test]
    fn desktop_pagination_001_is_bounded_to_one_hundred_and_rejects_foreign_cursor() {
        let namespace = NtfsNamespace {
            paths: Vec::new(),
            directories: Vec::new(),
            is_complete: true,
        };
        let candidates = (0..205)
            .map(|index| {
                candidate(
                    index,
                    index + 16,
                    Some(1),
                    &format!("file-{index}.bin"),
                    CandidateKind::File,
                )
            })
            .collect();
        let session = build_scan_session(
            "scan-pages",
            "E:",
            details(candidates, namespace),
            SessionScope::Volume,
        )
        .expect("build volume session");

        let first = page_for_session(&session, None, 100).expect("first page");
        assert_eq!(first.candidates.len(), 100);
        let next = first.next_cursor.expect("next cursor");
        assert!(next.starts_with("cur-"));
        let second = page_for_session(&session, Some(&next), 100).expect("second page");
        assert_eq!(second.candidates.len(), 100);
        assert!(page_for_session(&session, Some("cur-foreign"), 100).is_err());
        assert!(page_for_session(&session, None, 101).is_err());
    }

    #[test]
    fn desktop_candidate_001_directories_have_no_score_and_untrusted_text_is_sanitized() {
        let mut directory = candidate(
            1,
            20,
            Some(1),
            "bad\u{202E}\nname",
            CandidateKind::Directory,
        );
        directory.warnings = vec!["warn\u{0000}\u{2066}ing".to_owned()];
        let details = details(
            vec![directory],
            NtfsNamespace {
                paths: Vec::new(),
                directories: Vec::new(),
                is_complete: false,
            },
        );

        let session = build_scan_session("scan-sanitize", "E:", details, SessionScope::Volume)
            .expect("build session");

        assert_eq!(session.rows[0].recoverability_score, None);
        let serialized = serde_json::to_string(&session.rows[0]).expect("serialize row");
        assert!(!serialized.contains('\u{202E}'));
        assert!(!serialized.contains('\u{2066}'));
        assert!(!serialized.contains("\\n"));
    }

    #[test]
    fn desktop_candidate_002_long_colliding_paths_are_visibly_disambiguated() {
        let shared_prefix = "x".repeat(MAX_TEXT_SCALARS + 64);
        let first_name = format!("{shared_prefix}-first.txt");
        let second_name = format!("{shared_prefix}-second.txt");
        let candidates = vec![
            candidate(1, 30, Some(1), &first_name, CandidateKind::File),
            candidate(2, 31, Some(1), &second_name, CandidateKind::File),
        ];
        let session = build_scan_session(
            "scan-long-paths",
            "E:",
            details(
                candidates,
                NtfsNamespace {
                    paths: Vec::new(),
                    directories: Vec::new(),
                    is_complete: false,
                },
            ),
            SessionScope::Volume,
        )
        .expect("build long-path session");

        assert_ne!(session.rows[0].display_path, session.rows[1].display_path);
        for row in &session.rows {
            assert!(row.display_path.chars().count() <= MAX_TEXT_SCALARS);
            assert!(row.display_path.contains("… [ref "));
        }
    }

    #[test]
    fn desktop_state_001_folder_authority_is_bound_to_its_volume() {
        let state = DesktopStorageState::default();
        let scope = FolderScope {
            scope_id: "scope-bound".to_owned(),
            volume_id: "vol-one".to_owned(),
            display_label: "Documents".to_owned(),
            relative_components: vec!["Documents".to_owned()],
            volume_serial: 9,
            file_index: (1_u64 << 48) | 44,
            ntfs_record: 44,
            ntfs_sequence: 1,
        };

        state
            .store_scope(scope.clone(), "gen-one")
            .expect("store scope");

        assert_eq!(
            state
                .scope_for_volume("scope-bound", "vol-one", "gen-one")
                .expect("same volume"),
            scope
        );
        assert!(state
            .scope_for_volume("scope-bound", "vol-two", "gen-one")
            .is_err());
        assert!(state
            .scope_for_volume("scope-missing", "vol-one", "gen-one")
            .is_err());
        assert!(
            state
                .scope_for_volume("scope-bound", "vol-one", "gen-two")
                .is_err(),
            "a folder authority must not survive an inventory-generation change"
        );

        let mut inconsistent = scope;
        inconsistent.scope_id = "scope-inconsistent".to_owned();
        inconsistent.file_index = 99;
        assert!(state.store_scope(inconsistent, "gen-one").is_err());
    }

    #[test]
    fn desktop_request_id_001_rejects_control_text_and_unbounded_values() {
        assert!(validate_request_id("req-abc_123").is_ok());
        assert!(validate_request_id("").is_err());
        assert!(validate_request_id("bad\nrequest").is_err());
        assert!(validate_request_id(&"x".repeat(129)).is_err());
    }
}
