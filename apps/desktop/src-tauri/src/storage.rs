use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri_plugin_dialog::DialogExt;
use um_broker_client::{open_windows_source, BrokerClientError};
use um_cli::{
    CarveEvidence, CliError, JpegCarveCoverage, MftScanCoverage, VolumeScanDetails, VolumeScanMode,
    VolumeScanStatus,
};
use um_core::{
    Candidate, CandidateKind, CandidateState, DiscoveryMethod, MetadataConfidence, ReadError,
    RecoverabilityInputs,
};
use um_fs_common::ScanError;
use um_fs_ntfs::{
    NtfsCandidatePathEvidence, NtfsDirectoryResolution, NtfsNamespace, NtfsNamespaceIndex,
    NtfsNodeRef, NtfsPathState, NtfsScopeMembership,
};
use um_io_windows::{
    BusType, FolderScope, FolderScopeError, StorageError, StorageInventory, StorageVolume,
};

use crate::restore::{RestoreScanBinding, RestoreScanSnapshot, RestoreSelectedCandidate};
use crate::results::{
    self, BoundCandidateQuery, CandidateQuery, CandidateQueryContext, CandidateQueryPageDto,
    CandidateRowDto, CandidateSelectionUpdateDto, CandidateSort, ResultAuthorityError,
    ScanSourceBinding, SelectionOperationDto, StoredCandidate,
};

const STORAGE_SCHEMA_VERSION: u32 = 1;
const SCAN_SUMMARY_SCHEMA_VERSION: u32 = 3;
const CANDIDATE_PAGE_SCHEMA_VERSION: u32 = 2;
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
    scan_mode: &'static str,
    file_system: String,
    scan_status: String,
    total_candidates: String,
    matched_candidates: String,
    unknown_candidates: String,
    mft_coverage: Option<DesktopMftCoverage>,
    jpeg_carve_coverage: Option<DesktopJpegCarveCoverage>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopMftCoverage {
    records_declared: String,
    records_available: String,
    records_examined: String,
    bytes_declared: String,
    bytes_available: String,
    bytes_examined: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopJpegCarveCoverage {
    bytes_requested: String,
    bytes_scanned: String,
    signatures_attempted: String,
    validation_bytes_read: String,
    partial: bool,
    read_error_count: String,
    candidate_limit_reached: bool,
    candidate_byte_limit_hits: String,
    signature_attempt_limit_reached: bool,
    validation_byte_limit_reached: bool,
    rejected_signatures: String,
    truncated_signatures: String,
    regions_submitted: String,
    region_limit_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopScanScope {
    kind: &'static str,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopCandidatePage {
    schema_version: u32,
    scan_id: String,
    cursor: Option<String>,
    next_cursor: Option<String>,
    candidates: Vec<CandidateRowDto>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopScanMode {
    Metadata,
    DeepJpeg,
}

impl DesktopScanMode {
    fn parse(value: &str) -> Result<Self, DesktopStorageError> {
        match value {
            "metadata" => Ok(Self::Metadata),
            "deepJpeg" => Ok(Self::DeepJpeg),
            _ => Err(DesktopStorageError::new(
                "SCAN_MODE_UNSUPPORTED",
                "The requested scan mode is not supported.",
            )),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::DeepJpeg => "deepJpeg",
        }
    }

    const fn cli_mode(self) -> VolumeScanMode {
        match self {
            Self::Metadata => VolumeScanMode::MetadataOnly,
            Self::DeepJpeg => VolumeScanMode::DeepJpeg,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum SessionScope<'a> {
    Volume,
    Folder(&'a FolderScope),
}

pub(crate) struct ScanSession {
    pub(crate) summary: DesktopScanSummary,
    source: ScanSourceBinding,
    candidates: Vec<StoredCandidate>,
    candidate_index: HashMap<um_core::CandidateId, usize>,
    selected: HashSet<um_core::CandidateId>,
    selection_revision: u64,
    active_query: Option<BoundCandidateQuery>,
    cursors: BTreeMap<String, usize>,
    cursor_for_offset: BTreeMap<usize, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CandidateContentEvidence {
    expected_sha256: [u8; 32],
    content_sha256: String,
    validator: String,
}

#[derive(Clone, Default)]
pub(crate) struct DesktopStorageState {
    inner: Arc<Mutex<DesktopStorageStateInner>>,
}

#[derive(Default)]
struct DesktopStorageStateInner {
    scopes: HashMap<String, StoredFolderScope>,
    scope_order: VecDeque<String>,
    scans: HashMap<String, ScanSession>,
    scan_order: VecDeque<String>,
}

pub(crate) struct RestoreStartSelectionView<'a> {
    scan_id: &'a str,
    source: &'a ScanSourceBinding,
    selection_revision: u64,
    candidates: &'a [StoredCandidate],
    selected: &'a HashSet<um_core::CandidateId>,
}

impl RestoreStartSelectionView<'_> {
    pub(crate) const fn scan_id(&self) -> &str {
        self.scan_id
    }

    pub(crate) const fn source(&self) -> &ScanSourceBinding {
        self.source
    }

    pub(crate) const fn selection_revision(&self) -> u64 {
        self.selection_revision
    }

    pub(crate) fn matches_ordered_candidate_ids(&self, expected: &[um_core::CandidateId]) -> bool {
        self.candidates
            .iter()
            .filter(|stored| self.selected.contains(&stored.candidate.id))
            .map(|stored| stored.candidate.id)
            .eq(expected.iter().copied())
    }
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

    pub(crate) fn query_candidate_page(
        &self,
        scan_id: &str,
        query: CandidateQuery,
        sort: CandidateSort,
        cursor: Option<&str>,
    ) -> Result<CandidateQueryPageDto, DesktopStorageError> {
        let mut state = self.inner.lock().map_err(|_| {
            DesktopStorageError::new("SCAN_INTERNAL", "The scanner state is unavailable.")
        })?;
        let session = state
            .scans
            .get_mut(scan_id)
            .ok_or_else(DesktopStorageError::incompatible)?;
        results::query_candidate_page(
            CandidateQueryContext {
                scan_id,
                source: &session.source,
                candidates: &session.candidates,
                selected: &session.selected,
                selection_revision: session.selection_revision,
                active_query: &mut session.active_query,
            },
            query,
            sort,
            cursor,
        )
        .map_err(map_result_error)
    }

    pub(crate) fn update_candidate_selection(
        &self,
        scan_id: &str,
        query_id: &str,
        operation: SelectionOperationDto,
        selection_revision: u64,
    ) -> Result<CandidateSelectionUpdateDto, DesktopStorageError> {
        let mut state = self.inner.lock().map_err(|_| {
            DesktopStorageError::new("SCAN_INTERNAL", "The scanner state is unavailable.")
        })?;
        let session = state
            .scans
            .get_mut(scan_id)
            .ok_or_else(DesktopStorageError::incompatible)?;
        results::update_candidate_selection(
            scan_id,
            &session.candidates,
            &session.candidate_index,
            &mut session.selected,
            &mut session.selection_revision,
            session.active_query.as_ref(),
            query_id,
            operation,
            selection_revision,
        )
        .map_err(map_result_error)
    }

    pub(crate) fn restore_snapshot(
        &self,
        scan_id: &str,
        expected_selection_revision: Option<u64>,
    ) -> Result<RestoreScanSnapshot, DesktopStorageError> {
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
        if expected_selection_revision
            .is_some_and(|expected| expected != session.selection_revision)
        {
            return Err(DesktopStorageError::new(
                "RESULT_SELECTION_STALE",
                "The recovery selection changed. Review it and create a new plan.",
            ));
        }

        let candidates = session
            .candidates
            .iter()
            .filter(|stored| session.selected.contains(&stored.candidate.id))
            .map(|stored| RestoreSelectedCandidate {
                candidate: stored.candidate.clone(),
                expected_sha256: stored.expected_sha256,
                warnings: stored.row.warnings.clone(),
            })
            .collect();

        Ok(RestoreScanSnapshot {
            scan_id: scan_id.to_owned(),
            source: session.source.clone(),
            selection_revision: session.selection_revision,
            candidates,
        })
    }

    pub(crate) fn restore_scan_binding(
        &self,
        scan_id: &str,
    ) -> Result<RestoreScanBinding, DesktopStorageError> {
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
        Ok(RestoreScanBinding {
            scan_id: scan_id.to_owned(),
            source: session.source.clone(),
        })
    }

    /// Runs a short, allocation-free authorization step while the retained
    /// selection remains locked. The callback must not perform I/O or launch
    /// work.
    ///
    /// Restore start uses this as its linearization point: destination I/O is
    /// completed first, then the exact selection revision and scan order are
    /// compared by borrow while the coordinator atomically consumes the plan.
    pub(crate) fn with_restore_start_selection<T>(
        &self,
        scan_id: &str,
        expected_selection_revision: u64,
        authorize: impl FnOnce(RestoreStartSelectionView<'_>) -> T,
    ) -> Result<T, DesktopStorageError> {
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
        if expected_selection_revision != session.selection_revision {
            return Err(DesktopStorageError::new(
                "RESULT_SELECTION_STALE",
                "The recovery selection changed. Review it and create a new plan.",
            ));
        }

        Ok(authorize(RestoreStartSelectionView {
            scan_id,
            source: &session.source,
            selection_revision: session.selection_revision,
            candidates: &session.candidates,
            selected: &session.selected,
        }))
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
    mode: String,
) -> Result<DesktopScanSummary, DesktopStorageError> {
    validate_request_id(&request_id)?;
    let mode = DesktopScanMode::parse(&mode)?;
    if let Some(scope_id) = scope_id.as_deref() {
        validate_opaque_argument(scope_id)?;
    }

    let inventory = um_io_windows::enumerate_storage().map_err(map_inventory_error)?;
    let volume = authorize_inventory_generation(&inventory, &generation, &volume_id)?;
    if !volume.scan_supported {
        return Err(DesktopStorageError::new(
            "SOURCE_UNSUPPORTED",
            "This source is not eligible for read-only scanning.",
        ));
    }
    validate_scan_mode(mode, volume, scope_id.is_some())?;
    let source_label = volume_source_label(volume);
    let folder_scope = scope_id
        .as_deref()
        .map(|scope| state.scope_for_volume(scope, &volume_id, &generation))
        .transpose()?;

    let broker_volume_id = volume_id.clone();
    let (details, physical_disk_number) = tauri::async_runtime::spawn_blocking(move || {
        let reader = open_windows_source(&broker_volume_id).map_err(map_broker_error)?;
        let physical_disk_number = reader.physical_disk_number();
        let details = um_cli::scan_volume_reader_with_mode(&reader, mode.cli_mode())
            .map_err(map_cli_scan_error)?;
        Ok::<_, DesktopStorageError>((details, physical_disk_number))
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
    let source = ScanSourceBinding {
        inventory_generation: generation,
        volume_id,
        source_len: details.report.length_bytes,
        file_system: details.report.file_system.clone(),
        physical_disk_number,
    };
    let session =
        build_scan_session_with_mode(&scan_id, &source_label, details, scope, mode, source)?;
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

#[tauri::command]
pub(crate) fn query_candidate_page(
    state: tauri::State<'_, DesktopStorageState>,
    request_id: String,
    scan_id: String,
    query: CandidateQuery,
    sort: CandidateSort,
    cursor: Option<String>,
) -> Result<CandidateQueryPageDto, DesktopStorageError> {
    validate_request_id(&request_id)?;
    validate_opaque_argument(&scan_id)?;
    if let Some(cursor) = cursor.as_deref() {
        validate_opaque_argument(cursor)?;
    }
    state.query_candidate_page(&scan_id, query, sort, cursor.as_deref())
}

#[tauri::command]
pub(crate) fn update_candidate_selection(
    state: tauri::State<'_, DesktopStorageState>,
    request_id: String,
    scan_id: String,
    query_id: String,
    operation: SelectionOperationDto,
    selection_revision: String,
) -> Result<CandidateSelectionUpdateDto, DesktopStorageError> {
    validate_request_id(&request_id)?;
    validate_opaque_argument(&scan_id)?;
    validate_opaque_argument(&query_id)?;
    let selection_revision = parse_decimal_argument(&selection_revision)?;
    state.update_candidate_selection(&scan_id, &query_id, operation, selection_revision)
}

fn parse_decimal_argument(value: &str) -> Result<u64, DesktopStorageError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(DesktopStorageError::new(
            "RESULT_QUERY_INVALID",
            "The result request is invalid.",
        ));
    }
    value.parse().map_err(|_| {
        DesktopStorageError::new("RESULT_QUERY_INVALID", "The result request is invalid.")
    })
}

fn map_result_error(error: ResultAuthorityError) -> DesktopStorageError {
    match error {
        ResultAuthorityError::InvalidQuery => {
            DesktopStorageError::new("RESULT_QUERY_INVALID", "The result request is invalid.")
        }
        ResultAuthorityError::StaleCursor => DesktopStorageError::new(
            "RESULT_CURSOR_STALE",
            "The result cursor no longer matches this query.",
        ),
        ResultAuthorityError::StaleSelection => DesktopStorageError::new(
            "RESULT_SELECTION_STALE",
            "The selection changed before this update.",
        ),
        ResultAuthorityError::UnknownCandidate => DesktopStorageError::new(
            "RESULT_SELECTION_INVALID",
            "The selection request contains an unknown candidate.",
        ),
        ResultAuthorityError::Overflow => DesktopStorageError::incompatible(),
    }
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

fn validate_scan_mode(
    mode: DesktopScanMode,
    volume: &StorageVolume,
    has_folder_scope: bool,
) -> Result<(), DesktopStorageError> {
    if mode == DesktopScanMode::DeepJpeg
        && (has_folder_scope || !volume.file_system.eq_ignore_ascii_case("ntfs"))
    {
        return Err(DesktopStorageError::new(
            "SCAN_MODE_UNSUPPORTED",
            "Deep JPEG scanning requires a whole NTFS volume.",
        ));
    }
    Ok(())
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
        CliError::DeepScan { .. } => DesktopStorageError::new(
            "SCAN_CORRUPT",
            "The bounded JPEG scan could not validate the selected source safely.",
        ),
        CliError::DeepScanResult { .. } => DesktopStorageError::incompatible(),
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

#[cfg(test)]
pub(crate) fn build_scan_session(
    scan_id: &str,
    source_label: &str,
    details: VolumeScanDetails,
    scope: SessionScope<'_>,
) -> Result<ScanSession, DesktopStorageError> {
    let source = ScanSourceBinding {
        inventory_generation: format!("test-generation-{scan_id}"),
        volume_id: format!("test-volume-{scan_id}"),
        source_len: details.report.length_bytes,
        file_system: details.report.file_system.clone(),
        physical_disk_number: 7,
    };
    build_scan_session_with_mode(
        scan_id,
        source_label,
        details,
        scope,
        DesktopScanMode::Metadata,
        source,
    )
}

fn build_scan_session_with_mode(
    scan_id: &str,
    source_label: &str,
    details: VolumeScanDetails,
    scope: SessionScope<'_>,
    mode: DesktopScanMode,
    source: ScanSourceBinding,
) -> Result<ScanSession, DesktopStorageError> {
    if !valid_opaque_id(scan_id)
        || !valid_retained_candidate_count(details.report.candidate_count, details.candidates.len())
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
    let jpeg_coverage = details.report.jpeg_carve_coverage;
    let mode_is_coherent = match mode {
        DesktopScanMode::Metadata => jpeg_coverage.is_none() && details.carve_evidence.is_empty(),
        DesktopScanMode::DeepJpeg => {
            file_system == "ntfs"
                && matches!(scope, SessionScope::Volume)
                && jpeg_coverage.is_some()
        }
    };
    if !mode_is_coherent
        || jpeg_coverage.is_some_and(|coverage| {
            coverage.bytes_scanned > coverage.bytes_requested
                || (coverage.partial && scan_status != "partial")
                || (!coverage.partial
                    && (coverage.signature_attempt_limit_reached
                        || coverage.validation_byte_limit_reached))
        })
    {
        return Err(DesktopStorageError::incompatible());
    }

    let total_candidates = details.candidates.len();
    let evidence_by_candidate = index_carve_evidence(&details.candidates, &details.carve_evidence)?;
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

    if source.inventory_generation.is_empty()
        || source.volume_id.is_empty()
        || source.source_len != details.report.length_bytes
        || source.file_system != file_system
    {
        return Err(DesktopStorageError::incompatible());
    }
    let candidates = selected
        .into_iter()
        .map(|candidate| {
            let evidence = evidence_by_candidate.get(&candidate.id);
            let row = adapt_candidate(scan_id, candidate, namespace_index.as_ref(), evidence)?;
            Ok(StoredCandidate {
                candidate: candidate.clone(),
                row,
                expected_sha256: evidence.map(|evidence| evidence.expected_sha256),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidate_index = HashMap::with_capacity(candidates.len());
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate_index
            .insert(candidate.candidate.id, index)
            .is_some()
        {
            return Err(DesktopStorageError::incompatible());
        }
    }
    let matched_candidates = candidates.len();
    let (cursors, cursor_for_offset) = build_cursors(scan_id, candidates.len());

    Ok(ScanSession {
        summary: DesktopScanSummary {
            schema_version: SCAN_SUMMARY_SCHEMA_VERSION,
            scan_id: scan_id.to_owned(),
            source_label: sanitize_required(source_label),
            scope: DesktopScanScope {
                kind: scope_kind,
                label: scope_label,
            },
            scan_mode: mode.as_str(),
            file_system,
            scan_status: scan_status.to_owned(),
            total_candidates: total_candidates.to_string(),
            matched_candidates: matched_candidates.to_string(),
            unknown_candidates: unknown_candidates.to_string(),
            mft_coverage: details.report.mft_coverage.map(adapt_mft_coverage),
            jpeg_carve_coverage: jpeg_coverage.map(adapt_jpeg_carve_coverage),
            warnings: sanitize_warnings(details.report.warnings),
        },
        source,
        candidates,
        candidate_index,
        selected: HashSet::new(),
        selection_revision: 0,
        active_query: None,
        cursors,
        cursor_for_offset,
    })
}

fn valid_retained_candidate_count(reported: usize, retained: usize) -> bool {
    reported == retained && results::retained_candidate_count_within_bound(retained)
}

fn adapt_mft_coverage(coverage: MftScanCoverage) -> DesktopMftCoverage {
    DesktopMftCoverage {
        records_declared: coverage.records_declared.to_string(),
        records_available: coverage.records_available.to_string(),
        records_examined: coverage.records_examined.to_string(),
        bytes_declared: coverage.bytes_declared.to_string(),
        bytes_available: coverage.bytes_available.to_string(),
        bytes_examined: coverage.bytes_examined.to_string(),
    }
}

fn adapt_jpeg_carve_coverage(coverage: JpegCarveCoverage) -> DesktopJpegCarveCoverage {
    DesktopJpegCarveCoverage {
        bytes_requested: coverage.bytes_requested.to_string(),
        bytes_scanned: coverage.bytes_scanned.to_string(),
        signatures_attempted: coverage.signatures_attempted.to_string(),
        validation_bytes_read: coverage.validation_bytes_read.to_string(),
        partial: coverage.partial,
        read_error_count: coverage.read_error_count.to_string(),
        candidate_limit_reached: coverage.candidate_limit_reached,
        candidate_byte_limit_hits: coverage.candidate_byte_limit_hits.to_string(),
        signature_attempt_limit_reached: coverage.signature_attempt_limit_reached,
        validation_byte_limit_reached: coverage.validation_byte_limit_reached,
        rejected_signatures: coverage.rejected_signatures.to_string(),
        truncated_signatures: coverage.truncated_signatures.to_string(),
        regions_submitted: coverage.regions_submitted.to_string(),
        region_limit_reached: coverage.region_limit_reached,
    }
}

fn index_carve_evidence(
    candidates: &[Candidate],
    evidence: &[CarveEvidence],
) -> Result<HashMap<u64, CandidateContentEvidence>, DesktopStorageError> {
    let mut candidates_by_id = HashMap::new();
    for candidate in candidates {
        if candidates_by_id.insert(candidate.id, candidate).is_some() {
            return Err(DesktopStorageError::incompatible());
        }
    }

    let mut indexed = HashMap::new();
    for item in evidence {
        let candidate = candidates_by_id
            .get(&item.candidate_id)
            .copied()
            .ok_or_else(DesktopStorageError::incompatible)?;
        if item.validator != "jpeg-structural-v1"
            || item.len == 0
            || candidate_exact_physical_range(candidate) != Some((item.physical_offset, item.len))
            || indexed
                .insert(
                    item.candidate_id,
                    CandidateContentEvidence {
                        expected_sha256: item.content_sha256,
                        content_sha256: hex::encode(item.content_sha256),
                        validator: item.validator.to_owned(),
                    },
                )
                .is_some()
        {
            return Err(DesktopStorageError::incompatible());
        }
    }
    if candidates.iter().any(|candidate| {
        candidate.method == DiscoveryMethod::Carving && !indexed.contains_key(&candidate.id)
    }) {
        return Err(DesktopStorageError::incompatible());
    }
    Ok(indexed)
}

fn candidate_exact_physical_range(candidate: &Candidate) -> Option<(u64, u64)> {
    if candidate.size == 0 || candidate.extents.is_empty() {
        return None;
    }
    let mut logical = 0u64;
    let mut physical_start = None;
    let mut next_physical = 0u64;
    for extent in &candidate.extents {
        if extent.len == 0 || extent.logical_offset != logical {
            return None;
        }
        let physical = extent.physical_offset?;
        match physical_start {
            None => {
                physical_start = Some(physical);
                next_physical = physical;
            }
            Some(_) if physical != next_physical => return None,
            Some(_) => {}
        }
        logical = logical.checked_add(extent.len)?;
        next_physical = next_physical.checked_add(extent.len)?;
        if logical > candidate.size {
            return None;
        }
    }
    (logical == candidate.size).then_some((physical_start?, candidate.size))
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
    content_evidence: Option<&CandidateContentEvidence>,
) -> Result<CandidateRowDto, DesktopStorageError> {
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
    let method = match candidate.method {
        DiscoveryMethod::NtfsMetadata => "ntfsMetadata",
        DiscoveryMethod::FatMetadata => "fatMetadata",
        DiscoveryMethod::ExfatMetadata => "exfatMetadata",
        DiscoveryMethod::Carving => "carving",
        DiscoveryMethod::RecycleBin => "recycleBin",
    };
    if candidate.method == DiscoveryMethod::Carving && content_evidence.is_none() {
        return Err(DesktopStorageError::incompatible());
    }
    let recoverability_score = (candidate.kind == CandidateKind::File).then(|| {
        let mut inputs = RecoverabilityInputs::from_candidate(candidate);
        if content_evidence.is_some() {
            inputs.validated = true;
            inputs.validator_available = true;
        }
        inputs.score().value
    });
    let row_id = derive_opaque_id("cand", &[scan_id, &candidate.id.to_string()]);
    let display_path = sanitize_candidate_path(&candidate.display_path(), &row_id);

    Ok(CandidateRowDto {
        id: row_id,
        display_path,
        kind,
        state,
        size_bytes: candidate.size.to_string(),
        metadata_confidence,
        recoverability_score,
        path_state: candidate_path_state(candidate, namespace),
        method,
        content_sha256: content_evidence.map(|evidence| evidence.content_sha256.clone()),
        validator: content_evidence.map(|evidence| evidence.validator.clone()),
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
        .map(|value| value.min(session.candidates.len()))
        .ok_or_else(DesktopStorageError::incompatible)?;
    let candidates = session
        .candidates
        .get(offset..end)
        .ok_or_else(DesktopStorageError::incompatible)?
        .iter()
        .map(|candidate| candidate.row.clone())
        .collect();
    let next_cursor = if end < session.candidates.len() {
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
        schema_version: CANDIDATE_PAGE_SCHEMA_VERSION,
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
    use um_cli::{
        CarveEvidence, JpegCarveCoverage, MftScanCoverage, VolumeReport, VolumeScanDetails,
        VolumeScanStatus,
    };
    use um_core::{
        Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
        MetadataConfidence, Timestamps,
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
                mft_coverage: None,
                jpeg_carve_coverage: None,
                warnings: Vec::new(),
            },
            candidates,
            ntfs_namespace: Some(namespace),
            carve_evidence: Vec::new(),
        }
    }

    fn source(scan_id: &str, details: &VolumeScanDetails) -> ScanSourceBinding {
        ScanSourceBinding {
            inventory_generation: format!("generation-{scan_id}"),
            volume_id: format!("volume-{scan_id}"),
            source_len: details.report.length_bytes,
            file_system: details.report.file_system.clone(),
            physical_disk_number: 7,
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
    fn desktop_disk_policy_001_allows_only_unchanged_source_and_different_ntfs_disk() {
        assert_eq!(
            um_io_windows::validate_destination_policy(Some(7), Some(7), &[9], "NTFS"),
            Ok(())
        );
    }

    #[test]
    fn desktop_disk_policy_002_rejects_same_unknown_or_multi_disk_identity() {
        use um_io_windows::DestinationError;

        assert_eq!(
            um_io_windows::validate_destination_policy(Some(7), Some(7), &[7], "NTFS"),
            Err(DestinationError::SamePhysicalDisk)
        );
        assert_eq!(
            um_io_windows::validate_destination_policy(None, Some(7), &[9], "NTFS"),
            Err(DestinationError::SourceIdentityUnavailable)
        );
        assert_eq!(
            um_io_windows::validate_destination_policy(Some(7), None, &[9], "NTFS"),
            Err(DestinationError::SourceIdentityUnavailable)
        );
        assert_eq!(
            um_io_windows::validate_destination_policy(Some(7), Some(7), &[], "NTFS"),
            Err(DestinationError::MissingDiskMapping)
        );
        assert_eq!(
            um_io_windows::validate_destination_policy(Some(7), Some(7), &[9, 10], "NTFS"),
            Err(DestinationError::MultiDiskVolume)
        );
    }

    #[test]
    fn desktop_disk_policy_003_rejects_non_ntfs_and_changed_source_identity() {
        use um_io_windows::DestinationError;

        for file_system in ["ReFS", "FAT32", "exFAT", "unrecognized"] {
            assert_eq!(
                um_io_windows::validate_destination_policy(Some(7), Some(7), &[9], file_system),
                Err(DestinationError::UnsupportedFileSystem),
                "{file_system}"
            );
        }
        assert_eq!(
            um_io_windows::validate_destination_policy(Some(7), Some(8), &[9], "NTFS"),
            Err(DestinationError::SourceIdentityChanged)
        );
    }

    #[test]
    fn desktop_disk_identity_001_keeps_source_disk_number_out_of_webview_dtos() {
        let scan_details = details(
            Vec::new(),
            NtfsNamespace {
                paths: Vec::new(),
                directories: Vec::new(),
                is_complete: true,
            },
        );
        let scan_source = source("disk-secrecy", &scan_details);
        assert_eq!(scan_source.physical_disk_number, 7);

        let session = build_scan_session_with_mode(
            "scan-disk-secrecy",
            "External disk",
            scan_details,
            SessionScope::Volume,
            DesktopScanMode::Metadata,
            scan_source,
        )
        .expect("build native scan session");
        let serialized = serde_json::to_value(&session.summary).expect("serialize summary");

        assert!(
            serialized.get("physicalDiskNumber").is_none(),
            "the WebView scan summary must not expose native disk identity"
        );
        assert!(
            !serialized.to_string().contains("physicalDiskNumber"),
            "nested WebView values must not expose native disk identity"
        );
    }

    #[test]
    fn desktop_result_bound_001_accepts_the_deep_ceiling_and_rejects_one_more() {
        assert!(valid_retained_candidate_count(110_000, 110_000));
        assert!(!valid_retained_candidate_count(110_001, 110_001));
        assert!(!valid_retained_candidate_count(110_000, 109_999));
    }

    #[test]
    fn desktop_mft_coverage_001_preserves_all_u64_counters_as_decimal_strings() {
        let mut scan_details = details(
            Vec::new(),
            NtfsNamespace {
                paths: Vec::new(),
                directories: Vec::new(),
                is_complete: false,
            },
        );
        scan_details.report.scan_status = VolumeScanStatus::Partial;
        scan_details.report.mft_coverage = Some(MftScanCoverage {
            records_declared: u64::MAX,
            records_available: 9_007_199_254_740_993,
            records_examined: 65_536,
            bytes_declared: u64::MAX,
            bytes_available: 9_223_372_036_854_775_808,
            bytes_examined: 67_108_864,
        });

        let session = build_scan_session(
            "scan-coverage",
            "C: System",
            scan_details,
            SessionScope::Volume,
        )
        .expect("build coverage session");
        let json = serde_json::to_value(session.summary).expect("serialize summary");

        assert_eq!(json["schemaVersion"], 3);
        assert_eq!(json["scanMode"], "metadata");
        assert!(json["jpegCarveCoverage"].is_null());
        assert_eq!(json["mftCoverage"]["recordsDeclared"], u64::MAX.to_string());
        assert_eq!(json["mftCoverage"]["recordsAvailable"], "9007199254740993");
        assert_eq!(json["mftCoverage"]["recordsExamined"], "65536");
        assert_eq!(json["mftCoverage"]["bytesDeclared"], u64::MAX.to_string());
        assert_eq!(json["mftCoverage"]["bytesAvailable"], "9223372036854775808");
        assert_eq!(json["mftCoverage"]["bytesExamined"], "67108864");
    }

    #[test]
    fn desktop_deep_mode_001_rejects_folder_and_non_ntfs_before_broker_use() {
        let mut volume = StorageVolume {
            id: "vol-mode".to_owned(),
            mount_label: "E:".to_owned(),
            label: "Evidence".to_owned(),
            file_system: "NTFS".to_owned(),
            size_bytes: 1_000_000,
            free_bytes: 500_000,
            is_system: false,
            scan_supported: true,
            folder_scope_supported: true,
            warnings: Vec::new(),
        };

        assert!(validate_scan_mode(DesktopScanMode::Metadata, &volume, true).is_ok());
        assert!(validate_scan_mode(DesktopScanMode::DeepJpeg, &volume, false).is_ok());
        assert_eq!(
            validate_scan_mode(DesktopScanMode::DeepJpeg, &volume, true)
                .expect_err("folder deep scan must fail")
                .code,
            "SCAN_MODE_UNSUPPORTED"
        );
        volume.file_system = "FAT32".to_owned();
        assert_eq!(
            validate_scan_mode(DesktopScanMode::DeepJpeg, &volume, false)
                .expect_err("non-NTFS deep scan must fail")
                .code,
            "SCAN_MODE_UNSUPPORTED"
        );
        assert_eq!(
            DesktopScanMode::parse("unknown")
                .expect_err("unknown mode must fail closed")
                .code,
            "SCAN_MODE_UNSUPPORTED"
        );
    }

    #[test]
    fn desktop_deep_evidence_001_serializes_coverage_and_scores_validated_free_content() {
        let mut corroborated = candidate(77, 77, Some(1), "recovered.jpg", CandidateKind::File);
        corroborated.size = 64;
        corroborated.extents = vec![ExtentRun {
            logical_offset: 0,
            physical_offset: Some(8_192),
            len: 64,
            availability: ExtentAvailability::FreeInSnapshot,
        }];
        let mut scan_details = details(
            vec![corroborated],
            NtfsNamespace {
                paths: Vec::new(),
                directories: Vec::new(),
                is_complete: true,
            },
        );
        scan_details.report.jpeg_carve_coverage = Some(JpegCarveCoverage {
            bytes_requested: 1_048_576,
            bytes_scanned: 524_288,
            signatures_attempted: 10_000_000,
            validation_bytes_read: 262_144,
            partial: true,
            read_error_count: 1,
            candidate_limit_reached: false,
            candidate_byte_limit_hits: 2,
            signature_attempt_limit_reached: true,
            validation_byte_limit_reached: false,
            rejected_signatures: 7,
            truncated_signatures: 1,
            regions_submitted: 4,
            region_limit_reached: false,
        });
        scan_details.report.scan_status = VolumeScanStatus::Partial;
        scan_details.carve_evidence = vec![CarveEvidence {
            candidate_id: 77,
            physical_offset: 8_192,
            len: 64,
            content_sha256: [0xAB; 32],
            validator: "jpeg-structural-v1",
        }];

        let scan_source = source("scan-deep", &scan_details);
        let session = build_scan_session_with_mode(
            "scan-deep",
            "E: Evidence",
            scan_details,
            SessionScope::Volume,
            DesktopScanMode::DeepJpeg,
            scan_source,
        )
        .expect("build deep session");
        let summary = serde_json::to_value(&session.summary).expect("serialize summary");
        let first_page = page_for_session(&session, None, 100).expect("deep first page");
        let page = serde_json::to_value(first_page).expect("serialize page");

        assert_eq!(summary["schemaVersion"], 3);
        assert_eq!(summary["scanMode"], "deepJpeg");
        assert_eq!(summary["jpegCarveCoverage"]["bytesRequested"], "1048576");
        assert_eq!(summary["jpegCarveCoverage"]["bytesScanned"], "524288");
        assert_eq!(
            summary["jpegCarveCoverage"]["signaturesAttempted"],
            "10000000"
        );
        assert_eq!(
            summary["jpegCarveCoverage"]["validationBytesRead"],
            "262144"
        );
        assert_eq!(
            summary["jpegCarveCoverage"]["signatureAttemptLimitReached"],
            true
        );
        assert_eq!(page["schemaVersion"], 2);
        assert_eq!(page["candidates"][0]["method"], "ntfsMetadata");
        assert_eq!(
            page["candidates"][0]["contentSha256"],
            "abababababababababababababababababababababababababababababababab"
        );
        assert_eq!(page["candidates"][0]["validator"], "jpeg-structural-v1");
        assert!(
            page["candidates"][0]["recoverabilityScore"]
                .as_u64()
                .is_some_and(|score| score >= 85),
            "validated, fully free content must receive the validation floor"
        );
    }

    #[test]
    fn desktop_deep_evidence_002_requires_evidence_for_carving_and_preserves_metadata_cap() {
        let mut unvalidated = candidate(78, 78, Some(1), "unvalidated.jpg", CandidateKind::File);
        unvalidated.size = 64;
        unvalidated.extents = vec![ExtentRun {
            logical_offset: 0,
            physical_offset: Some(16_384),
            len: 64,
            availability: ExtentAvailability::FreeInSnapshot,
        }];
        let metadata_session = build_scan_session(
            "scan-metadata-cap",
            "E: Evidence",
            details(
                vec![unvalidated.clone()],
                NtfsNamespace {
                    paths: Vec::new(),
                    directories: Vec::new(),
                    is_complete: true,
                },
            ),
            SessionScope::Volume,
        )
        .expect("metadata session");
        assert!(
            metadata_session.candidates[0]
                .row
                .recoverability_score
                .is_some_and(|score| score <= 84),
            "metadata without content evidence retains the unvalidated cap"
        );

        unvalidated.method = DiscoveryMethod::Carving;
        let mut missing_evidence = details(
            vec![unvalidated],
            NtfsNamespace {
                paths: Vec::new(),
                directories: Vec::new(),
                is_complete: true,
            },
        );
        missing_evidence.report.jpeg_carve_coverage = Some(JpegCarveCoverage {
            bytes_requested: 64,
            bytes_scanned: 64,
            signatures_attempted: 1,
            validation_bytes_read: 64,
            partial: false,
            read_error_count: 0,
            candidate_limit_reached: false,
            candidate_byte_limit_hits: 0,
            signature_attempt_limit_reached: false,
            validation_byte_limit_reached: false,
            rejected_signatures: 0,
            truncated_signatures: 0,
            regions_submitted: 1,
            region_limit_reached: false,
        });
        let scan_source = source("scan-missing-evidence", &missing_evidence);
        let result = build_scan_session_with_mode(
            "scan-missing-evidence",
            "E: Evidence",
            missing_evidence,
            SessionScope::Volume,
            DesktopScanMode::DeepJpeg,
            scan_source,
        );
        assert!(result.is_err(), "carving without evidence must fail closed");
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
        assert_eq!(session.candidates.len(), 1);
        assert!(session.candidates[0]
            .row
            .display_path
            .ends_with("inside.txt"));
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

        assert_eq!(session.candidates[0].row.recoverability_score, None);
        let serialized = serde_json::to_string(&session.candidates[0].row).expect("serialize row");
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

        assert_ne!(
            session.candidates[0].row.display_path,
            session.candidates[1].row.display_path
        );
        for row in session.candidates.iter().map(|candidate| &candidate.row) {
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

    #[test]
    fn restore_storage_snapshot_preserves_revision_candidate_order_and_native_only_authority() {
        let candidates = vec![
            candidate(91, 91, Some(1), "first.bin", CandidateKind::File),
            candidate(92, 92, Some(1), "second.bin", CandidateKind::File),
            candidate(93, 93, Some(1), "third.bin", CandidateKind::File),
        ];
        let mut session = build_scan_session(
            "scan-restore-snapshot",
            "Fixture",
            details(
                candidates,
                NtfsNamespace {
                    paths: Vec::new(),
                    directories: Vec::new(),
                    is_complete: true,
                },
            ),
            SessionScope::Volume,
        )
        .unwrap();
        session.selected.extend([93, 91]);
        session.selection_revision = 7;
        let state = DesktopStorageState::default();
        state.store_session(session).unwrap();

        let snapshot = state
            .restore_snapshot("scan-restore-snapshot", Some(7))
            .unwrap();

        assert_eq!(snapshot.selection_revision, 7);
        assert_eq!(
            snapshot
                .candidates
                .iter()
                .map(|item| item.candidate.id)
                .collect::<Vec<_>>(),
            vec![91, 93],
            "selection must follow immutable scan order, never HashSet order"
        );
        assert_eq!(snapshot.source.physical_disk_number, 7);
        assert_eq!(
            snapshot.source.volume_id,
            "test-volume-scan-restore-snapshot"
        );
        let shared_state = state.clone();
        let ordered_selection_matches = shared_state
            .with_restore_start_selection("scan-restore-snapshot", 7, |selection| {
                assert_eq!(selection.scan_id(), "scan-restore-snapshot");
                assert_eq!(selection.selection_revision(), 7);
                assert_eq!(selection.source(), &snapshot.source);
                selection.matches_ordered_candidate_ids(&[91, 93])
                    && !selection.matches_ordered_candidate_ids(&[93, 91])
            })
            .unwrap();
        assert!(
            ordered_selection_matches,
            "the cheaply cloned storage authority must compare borrowed IDs in immutable scan order"
        );
        assert_eq!(
            shared_state
                .with_restore_start_selection("scan-restore-snapshot", 6, |_| ())
                .unwrap_err()
                .code,
            "RESULT_SELECTION_STALE"
        );
        assert_eq!(
            shared_state
                .restore_scan_binding("scan-restore-snapshot")
                .unwrap(),
            RestoreScanBinding::from(&snapshot),
            "destination selection must clone only the scan/source binding, never candidate payloads"
        );
        assert_eq!(
            state
                .restore_snapshot("scan-restore-snapshot", Some(6))
                .unwrap_err()
                .code,
            "RESULT_SELECTION_STALE"
        );

        let public_summary = serde_json::to_string(
            &state
                .candidate_page("scan-restore-snapshot", None, 100)
                .unwrap(),
        )
        .unwrap();
        assert!(!public_summary.contains("physicalDisk"));
        assert!(!public_summary.contains("test-volume-scan-restore-snapshot"));
    }
}
