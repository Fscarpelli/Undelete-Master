use std::{
    collections::HashMap,
    ops::Deref,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use cap_fs_ext::DirExt;
use cap_std::fs::Dir;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri_plugin_dialog::DialogExt;
use um_core::SourceReader;
use um_restore::{
    CancellationProbe, DestinationRoot, FileRestorePlan, PartialPolicy, PlanLimits, ProgressSink,
    RestoreCompletionStatus, RestoreItemKind, RestoreItemOutcome, RestoreJobEvent,
    RestoreJobObserver, RestoreJobPlan, RestoreSummary, SafeRelativePath, StreamProgress,
};

use crate::{
    results::{self, RecoveryEligibility, ScanSourceBinding},
    storage::{
        validate_request_id, DesktopStorageError, DesktopStorageState, RestoreStartSelectionView,
    },
};

pub(crate) const MAX_DESTINATION_AUTHORITIES: usize = 32;
pub(crate) const MAX_RETAINED_RESTORE_PLANS: usize = 8;
pub(crate) const MAX_RETAINED_RESTORE_JOBS: usize = 8;
const RESTORE_SCHEMA_VERSION: u32 = 1;
const MAX_RESTORE_WARNINGS: usize = 128;
const MAX_RESTORE_WARNING_SCALARS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopRestoreError {
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
}

impl DesktopRestoreError {
    const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }

    const fn destination_expired() -> Self {
        Self::new(
            "RESTORE_DESTINATION_EXPIRED",
            "The selected recovery destination is no longer valid.",
        )
    }

    const fn internal() -> Self {
        Self::new(
            "RESTORE_INTERNAL",
            "The native recovery coordinator could not complete the request.",
        )
    }
}

impl From<DesktopStorageError> for DesktopRestoreError {
    fn from(error: DesktopStorageError) -> Self {
        match error.code {
            "RESULT_SELECTION_STALE" => Self::new(
                "RESTORE_SELECTION_STALE",
                "The recovery selection changed before the plan could be used.",
            ),
            "REPORT_INCOMPATIBLE" => Self::new(
                "RESTORE_PLAN_EXPIRED",
                "The scan retained by this recovery plan is no longer available.",
            ),
            "SOURCE_IDENTITY_CHANGED" | "SOURCE_GONE" => Self::new(
                "RESTORE_SOURCE_CHANGED",
                "The recovery source identity changed.",
            ),
            _ => Self::internal(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CollisionPolicyDto {
    Rename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PartialFilePolicyDto {
    CompleteOnly,
    ZeroFillAndMap,
}

impl PartialFilePolicyDto {
    const fn engine_policy(self) -> PartialPolicy {
        match self {
            Self::CompleteOnly => PartialPolicy::CompleteOnly,
            Self::ZeroFillAndMap => PartialPolicy::ZeroFillAndMap,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DestinationSummaryDto {
    schema_version: u32,
    pub(crate) destination_id: String,
    label: String,
    volume_label: String,
    file_system: String,
    free_bytes: String,
    relation: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestorePlanSummaryDto {
    schema_version: u32,
    pub(crate) plan_id: String,
    pub(crate) plan_digest: String,
    pub(crate) scan_id: String,
    pub(crate) destination_id: String,
    pub(crate) selection_revision: String,
    pub(crate) collision_policy: CollisionPolicyDto,
    pub(crate) partial_file_policy: PartialFilePolicyDto,
    pub(crate) items_total: String,
    files_total: String,
    directories_total: String,
    pub(crate) logical_bytes: String,
    pub(crate) best_effort_items: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RestoreJobStatusDto {
    Queued,
    Running,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

impl RestoreJobStatusDto {
    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreCurrentItemDto {
    ordinal: String,
    candidate_id: String,
    kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreManifestSummaryDto {
    manifest_sha256: String,
    completion_status: &'static str,
    pub(crate) published_items: String,
    pub(crate) partial_items: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreJobSnapshotDto {
    schema_version: u32,
    pub(crate) job_id: String,
    plan_id: String,
    pub(crate) status: RestoreJobStatusDto,
    items_total: String,
    pub(crate) items_completed: String,
    pub(crate) items_failed: String,
    pub(crate) items_cancelled: String,
    bytes_total: String,
    pub(crate) bytes_completed: String,
    current_item: Option<RestoreCurrentItemDto>,
    warnings: Vec<String>,
    pub(crate) manifest: Option<RestoreManifestSummaryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenRestoreDestinationDto {
    pub(crate) schema_version: u32,
    pub(crate) opened: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RestoreSelectedCandidate {
    pub(crate) candidate: um_core::Candidate,
    pub(crate) expected_sha256: Option<[u8; 32]>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RestoreScanSnapshot {
    pub(crate) scan_id: String,
    pub(crate) source: ScanSourceBinding,
    pub(crate) selection_revision: u64,
    pub(crate) candidates: Vec<RestoreSelectedCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestoreScanBinding {
    pub(crate) scan_id: String,
    pub(crate) source: ScanSourceBinding,
}

impl From<&RestoreScanSnapshot> for RestoreScanBinding {
    fn from(snapshot: &RestoreScanSnapshot) -> Self {
        Self {
            scan_id: snapshot.scan_id.clone(),
            source: snapshot.source.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DestinationRevalidation {
    pub(crate) file_system: String,
    pub(crate) free_bytes: u64,
    pub(crate) physical_disk_number: u32,
    pub(crate) reparse_safe: bool,
}

pub(crate) trait DestinationCapability: Send + Sync {
    fn revalidate(&self) -> Result<DestinationRevalidation, DesktopRestoreError>;
    fn try_clone_root(&self) -> Result<std::fs::File, DesktopRestoreError>;
    fn open_job_directory(&self, job_component: &str) -> Result<(), DesktopRestoreError>;
}

struct NativeDestinationCapability {
    binding: um_io_windows::DestinationRootBinding,
}

impl DestinationCapability for NativeDestinationCapability {
    fn revalidate(&self) -> Result<DestinationRevalidation, DesktopRestoreError> {
        let snapshot = self
            .binding
            .revalidate()
            .map_err(map_destination_storage_error)?;
        Ok(DestinationRevalidation {
            file_system: snapshot.file_system().to_owned(),
            free_bytes: snapshot.free_bytes(),
            physical_disk_number: snapshot.physical_disk_number(),
            reparse_safe: snapshot.reparse_safe(),
        })
    }

    fn try_clone_root(&self) -> Result<std::fs::File, DesktopRestoreError> {
        self.binding
            .try_clone_directory_file()
            .map_err(map_destination_storage_error)
    }

    fn open_job_directory(&self, job_component: &str) -> Result<(), DesktopRestoreError> {
        self.revalidate()?;
        let root = Dir::from_std_file(self.try_clone_root()?);
        let job_dir = root
            .open_dir_nofollow(job_component)
            .map_err(|_| DesktopRestoreError::destination_expired())?;
        let metadata = job_dir
            .dir_metadata()
            .map_err(|_| DesktopRestoreError::destination_expired())?;
        if !metadata.is_dir() {
            return Err(DesktopRestoreError::destination_expired());
        }
        um_io_windows::open_retained_directory_in_shell(job_dir.into_std_file())
            .map_err(map_destination_storage_error)
    }
}

pub(crate) struct AdmittedDestination {
    pub(crate) label: String,
    pub(crate) volume_label: String,
    pub(crate) capability: Arc<dyn DestinationCapability>,
}

pub(crate) struct OpenedRestoreSource {
    pub(crate) reader: Box<dyn SourceReader>,
    pub(crate) physical_disk_number: u32,
}

pub(crate) trait RestoreRuntime: Send + Sync {
    fn open_source(
        &self,
        binding: &ScanSourceBinding,
    ) -> Result<OpenedRestoreSource, DesktopRestoreError>;

    /// `Err` means the worker was not scheduled and the closure was dropped.
    fn spawn_worker(
        &self,
        worker: Box<dyn FnOnce() + Send + 'static>,
    ) -> Result<(), DesktopRestoreError> {
        std::thread::Builder::new()
            .name("undelete-restore-worker".into())
            .spawn(worker)
            .map(|_| ())
            .map_err(|_| DesktopRestoreError::internal())
    }
}

struct ProductionRestoreRuntime;

impl RestoreRuntime for ProductionRestoreRuntime {
    fn open_source(
        &self,
        binding: &ScanSourceBinding,
    ) -> Result<OpenedRestoreSource, DesktopRestoreError> {
        let reader =
            um_broker_client::open_windows_source(&binding.volume_id).map_err(map_broker_error)?;
        let physical_disk_number = reader.physical_disk_number();
        Ok(OpenedRestoreSource {
            reader: Box::new(reader),
            physical_disk_number,
        })
    }
}

#[derive(Clone)]
pub(crate) struct RestoreCoordinator {
    inner: Arc<Mutex<RestoreCoordinatorInner>>,
    runtime: Arc<dyn RestoreRuntime>,
}

impl Default for RestoreCoordinator {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RestoreCoordinatorInner::default())),
            runtime: Arc::new(ProductionRestoreRuntime),
        }
    }
}

#[derive(Default)]
struct RestoreCoordinatorInner {
    destinations: HashMap<String, RetainedDestinationAuthority>,
    plans: HashMap<String, RetainedRestorePlan>,
    jobs: HashMap<String, Arc<RetainedRestoreJob>>,
}

struct RetainedDestinationAuthority {
    scan_id: String,
    source: ScanSourceBinding,
    admitted: AdmittedDestination,
    baseline: DestinationRevalidation,
}

#[derive(Clone)]
struct RetainedRestorePlan {
    immutable: Arc<ImmutableRestorePlan>,
    started: bool,
}

struct ImmutableRestorePlan {
    summary: RestorePlanSummaryDto,
    source: ScanSourceBinding,
    selection_revision: u64,
    ordered_candidate_ids: Vec<u64>,
    engine_job_id: String,
    engine_plan: RestoreJobPlan,
}

impl Deref for RetainedRestorePlan {
    type Target = ImmutableRestorePlan;

    fn deref(&self) -> &Self::Target {
        &self.immutable
    }
}

struct RetainedRestoreJob {
    job_id: String,
    plan_id: String,
    destination_id: String,
    engine_job_id: String,
    items_total: u64,
    bytes_total: u64,
    cancel: Arc<AtomicBool>,
    state: Mutex<RestoreJobMutable>,
}

struct RestoreJobMutable {
    status: RestoreJobStatusDto,
    items_completed: u64,
    items_failed: u64,
    items_cancelled: u64,
    committed_bytes: u64,
    current_bytes: u64,
    current_item: Option<RestoreCurrentItemDto>,
    warnings: Vec<String>,
    manifest: Option<RestoreManifestSummaryDto>,
}

struct PreparedRestoreStart {
    plan: RetainedRestorePlan,
    capability: Arc<dyn DestinationCapability>,
    destination_baseline: DestinationRevalidation,
    destination_root: DestinationRoot,
    required_bytes: u64,
}

struct CommittedRestoreStart {
    job: Arc<RetainedRestoreJob>,
    plan: RetainedRestorePlan,
    destination_root: DestinationRoot,
}

impl RestoreCoordinator {
    #[cfg(test)]
    pub(crate) fn with_runtime(runtime: Arc<dyn RestoreRuntime>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RestoreCoordinatorInner::default())),
            runtime,
        }
    }

    fn admit_destination_binding(
        &self,
        scan: &RestoreScanBinding,
        admitted: Option<AdmittedDestination>,
    ) -> Result<Option<DestinationSummaryDto>, DesktopRestoreError> {
        let Some(admitted) = admitted else {
            return Ok(None);
        };
        let baseline = admitted.capability.revalidate()?;
        validate_destination_separation(&scan.source, &baseline)?;

        let mut inner = self
            .inner
            .lock()
            .map_err(|_| DesktopRestoreError::internal())?;
        if inner.destinations.len() >= MAX_DESTINATION_AUTHORITIES {
            return Err(DesktopRestoreError::new(
                "RESTORE_DESTINATION_LIMIT",
                "Too many recovery destinations are retained. Finish or restart the workflow.",
            ));
        }
        let destination_id = create_opaque_id("destination")?;
        let summary = DestinationSummaryDto {
            schema_version: RESTORE_SCHEMA_VERSION,
            destination_id: destination_id.clone(),
            label: sanitize_restore_label(&admitted.label, "Selected destination"),
            volume_label: sanitize_restore_label(&admitted.volume_label, "NTFS volume"),
            file_system: "NTFS".into(),
            free_bytes: baseline.free_bytes.to_string(),
            relation: "different",
        };
        inner.destinations.insert(
            destination_id,
            RetainedDestinationAuthority {
                scan_id: scan.scan_id.clone(),
                source: scan.source.clone(),
                admitted,
                baseline,
            },
        );
        Ok(Some(summary))
    }

    #[cfg(test)]
    pub(crate) fn admit_destination(
        &self,
        scan: &RestoreScanSnapshot,
        admitted: Option<AdmittedDestination>,
    ) -> Result<Option<DestinationSummaryDto>, DesktopRestoreError> {
        self.admit_destination_binding(&RestoreScanBinding::from(scan), admitted)
    }

    pub(crate) fn create_restore_plan(
        &self,
        scan: &RestoreScanSnapshot,
        destination_id: &str,
        collision_policy: CollisionPolicyDto,
        partial_file_policy: PartialFilePolicyDto,
    ) -> Result<RestorePlanSummaryDto, DesktopRestoreError> {
        if scan.candidates.is_empty() {
            return Err(DesktopRestoreError::new(
                "RESTORE_SELECTION_EMPTY",
                "Select at least one recoverable item.",
            ));
        }
        let (destination_capability, destination_baseline) = {
            let inner = self
                .inner
                .lock()
                .map_err(|_| DesktopRestoreError::internal())?;
            if inner.plans.len() >= MAX_RETAINED_RESTORE_PLANS {
                return Err(DesktopRestoreError::new(
                    "RESTORE_PLAN_LIMIT",
                    "Too many recovery plans are retained.",
                ));
            }
            let destination = inner
                .destinations
                .get(destination_id)
                .ok_or_else(DesktopRestoreError::destination_expired)?;
            validate_scan_destination_binding(scan, destination)?;
            (
                Arc::clone(&destination.admitted.capability),
                destination.baseline.clone(),
            )
        };

        // Destination I/O and per-candidate planning may be slow. The coordinator
        // mutex protects only retained authority state so polling and cancellation
        // remain responsive while a plan is being built.
        let current = destination_capability.revalidate()?;
        validate_destination_baseline(&destination_baseline, &current)?;
        validate_destination_separation(&scan.source, &current)?;

        let mut engine_items = Vec::with_capacity(scan.candidates.len());
        let mut ordered_candidate_ids = Vec::with_capacity(scan.candidates.len());
        let mut files = 0u64;
        let mut directories = 0u64;
        let mut logical_bytes = 0u64;
        let mut best_effort = 0u64;
        for selected in &scan.candidates {
            match results::candidate_eligibility(&selected.candidate) {
                RecoveryEligibility::Ineligible => {
                    return Err(DesktopRestoreError::new(
                        "RESTORE_ITEM_INELIGIBLE",
                        "The selection contains an item without a bounded recovery plan.",
                    ));
                }
                RecoveryEligibility::BestEffort
                    if partial_file_policy != PartialFilePolicyDto::ZeroFillAndMap =>
                {
                    return Err(DesktopRestoreError::new(
                        "RESTORE_PARTIAL_POLICY_REQUIRED",
                        "Best-effort items require explicit zero-fill and range-map consent.",
                    ));
                }
                RecoveryEligibility::BestEffort => {
                    best_effort = best_effort
                        .checked_add(1)
                        .ok_or_else(DesktopRestoreError::internal)?;
                }
                RecoveryEligibility::Complete => {}
            }
            let path = SafeRelativePath::derive_for_recovery(
                &selected.candidate.parent_path,
                &selected.candidate.name,
            )
            .map_err(|_| {
                DesktopRestoreError::new(
                    "RESTORE_PLAN_INVALID",
                    "A selected item has unsafe or excessive path evidence.",
                )
            })?;
            let warnings = sanitize_restore_warnings(&selected.warnings);
            let item = if selected.candidate.kind == um_core::CandidateKind::Directory {
                directories = directories
                    .checked_add(1)
                    .ok_or_else(DesktopRestoreError::internal)?;
                FileRestorePlan::directory(selected.candidate.id, path)
            } else {
                files = files
                    .checked_add(1)
                    .ok_or_else(DesktopRestoreError::internal)?;
                logical_bytes = logical_bytes
                    .checked_add(selected.candidate.size)
                    .ok_or_else(DesktopRestoreError::internal)?;
                let content = um_restore::plan_candidate(
                    &selected.candidate,
                    scan.source.source_len,
                    selected.expected_sha256,
                    partial_file_policy.engine_policy(),
                    PlanLimits::default(),
                )
                .map_err(map_restore_plan_error)?
                .ok_or_else(|| {
                    DesktopRestoreError::new(
                        "RESTORE_ITEM_INELIGIBLE",
                        "The selection contains an item without file content.",
                    )
                })?;
                FileRestorePlan::file(path, content)
            }
            .and_then(|item| item.with_warnings(warnings))
            .map_err(map_restore_plan_error)?;
            ordered_candidate_ids.push(selected.candidate.id);
            engine_items.push(item);
        }
        if logical_bytes > current.free_bytes {
            return Err(DesktopRestoreError::new(
                "RESTORE_DESTINATION_INVALID",
                "The selected destination does not have enough available space.",
            ));
        }

        let plan_id = create_opaque_id("plan")?;
        let engine_job_id = create_opaque_id("job")?;
        let plan_digest = plan_digest(scan, destination_id, collision_policy, partial_file_policy)?;
        let engine_plan =
            RestoreJobPlan::new(&engine_job_id, engine_items).map_err(map_restore_plan_error)?;
        let summary = RestorePlanSummaryDto {
            schema_version: RESTORE_SCHEMA_VERSION,
            plan_id: plan_id.clone(),
            plan_digest,
            scan_id: scan.scan_id.clone(),
            destination_id: destination_id.to_owned(),
            selection_revision: scan.selection_revision.to_string(),
            collision_policy,
            partial_file_policy,
            items_total: scan.candidates.len().to_string(),
            files_total: files.to_string(),
            directories_total: directories.to_string(),
            logical_bytes: logical_bytes.to_string(),
            best_effort_items: best_effort.to_string(),
        };
        let retained_plan = RetainedRestorePlan {
            immutable: Arc::new(ImmutableRestorePlan {
                summary: summary.clone(),
                source: scan.source.clone(),
                selection_revision: scan.selection_revision,
                ordered_candidate_ids,
                engine_job_id,
                engine_plan,
            }),
            started: false,
        };

        let mut inner = self
            .inner
            .lock()
            .map_err(|_| DesktopRestoreError::internal())?;
        if inner.plans.len() >= MAX_RETAINED_RESTORE_PLANS {
            return Err(DesktopRestoreError::new(
                "RESTORE_PLAN_LIMIT",
                "Too many recovery plans are retained.",
            ));
        }
        let destination = inner
            .destinations
            .get(destination_id)
            .ok_or_else(DesktopRestoreError::destination_expired)?;
        validate_scan_destination_binding(scan, destination)?;
        if !Arc::ptr_eq(&destination_capability, &destination.admitted.capability)
            || destination.baseline != destination_baseline
        {
            return Err(DesktopRestoreError::destination_expired());
        }
        inner.plans.insert(plan_id, retained_plan);
        Ok(summary)
    }

    pub(crate) fn plan_binding(&self, plan_id: &str) -> Result<(String, u64), DesktopRestoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| DesktopRestoreError::internal())?;
        let plan = inner.plans.get(plan_id).ok_or_else(|| {
            DesktopRestoreError::new(
                "RESTORE_PLAN_EXPIRED",
                "The recovery plan is no longer retained.",
            )
        })?;
        Ok((plan.summary.scan_id.clone(), plan.selection_revision))
    }

    fn preflight_restore_start(
        &self,
        plan_id: &str,
    ) -> Result<PreparedRestoreStart, DesktopRestoreError> {
        let (plan, capability, destination_baseline) = {
            let inner = self
                .inner
                .lock()
                .map_err(|_| DesktopRestoreError::internal())?;
            if inner.jobs.len() >= MAX_RETAINED_RESTORE_JOBS {
                return Err(DesktopRestoreError::new(
                    "RESTORE_JOB_LIMIT",
                    "Too many recovery jobs are retained.",
                ));
            }
            let plan = inner.plans.get(plan_id).ok_or_else(|| {
                DesktopRestoreError::new(
                    "RESTORE_PLAN_EXPIRED",
                    "The recovery plan is no longer retained.",
                )
            })?;
            if plan.started {
                return Err(DesktopRestoreError::new(
                    "RESTORE_PLAN_INVALID",
                    "A recovery plan can be started only once.",
                ));
            }
            let destination = inner
                .destinations
                .get(&plan.summary.destination_id)
                .ok_or_else(DesktopRestoreError::destination_expired)?;
            (
                plan.clone(),
                Arc::clone(&destination.admitted.capability),
                destination.baseline.clone(),
            )
        };

        // Destination identity/free-space queries and retained-handle cloning
        // are I/O. They intentionally run after releasing the coordinator lock.
        let current_destination = capability.revalidate()?;
        validate_destination_baseline(&destination_baseline, &current_destination)?;
        validate_destination_separation(&plan.source, &current_destination)?;
        let required_bytes = parse_internal_decimal(&plan.summary.logical_bytes)?;
        if required_bytes > current_destination.free_bytes {
            return Err(DesktopRestoreError::new(
                "RESTORE_DESTINATION_INVALID",
                "The selected destination does not have enough available space.",
            ));
        }
        let root_file = capability.try_clone_root()?;
        let destination_root =
            DestinationRoot::from_retained_file(root_file).map_err(map_restore_start_error)?;

        Ok(PreparedRestoreStart {
            plan,
            capability,
            destination_baseline,
            destination_root,
            required_bytes,
        })
    }

    /// Consumes a preflight only after storage has supplied its current
    /// selection snapshot. This method performs no I/O and is safe to call
    /// while the storage selection lock is held.
    fn commit_prepared_restore_start(
        &self,
        selection: &RestoreStartSelectionView<'_>,
        prepared: PreparedRestoreStart,
    ) -> Result<CommittedRestoreStart, DesktopRestoreError> {
        self.commit_prepared_restore_start_fields(
            selection.scan_id(),
            selection.source(),
            selection.selection_revision(),
            |expected| selection.matches_ordered_candidate_ids(expected),
            prepared,
        )
    }

    fn commit_prepared_restore_start_fields(
        &self,
        scan_id: &str,
        source: &ScanSourceBinding,
        selection_revision: u64,
        selection_matches: impl FnOnce(&[u64]) -> bool,
        prepared: PreparedRestoreStart,
    ) -> Result<CommittedRestoreStart, DesktopRestoreError> {
        let plan = prepared.plan;
        let required_bytes = prepared.required_bytes;
        let job = Arc::new(RetainedRestoreJob {
            job_id: plan.engine_job_id.clone(),
            plan_id: plan.summary.plan_id.clone(),
            destination_id: plan.summary.destination_id.clone(),
            engine_job_id: plan.engine_job_id.clone(),
            items_total: u64::try_from(plan.ordered_candidate_ids.len())
                .map_err(|_| DesktopRestoreError::internal())?,
            bytes_total: required_bytes,
            cancel: Arc::new(AtomicBool::new(false)),
            state: Mutex::new(RestoreJobMutable {
                status: RestoreJobStatusDto::Queued,
                items_completed: 0,
                items_failed: 0,
                items_cancelled: 0,
                committed_bytes: 0,
                current_bytes: 0,
                current_item: None,
                warnings: Vec::new(),
                manifest: None,
            }),
        });
        {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| DesktopRestoreError::internal())?;
            if inner.jobs.len() >= MAX_RETAINED_RESTORE_JOBS {
                return Err(DesktopRestoreError::new(
                    "RESTORE_JOB_LIMIT",
                    "Too many recovery jobs are retained.",
                ));
            }
            let retained = inner.plans.get(&plan.summary.plan_id).ok_or_else(|| {
                DesktopRestoreError::new(
                    "RESTORE_PLAN_EXPIRED",
                    "The recovery plan is no longer retained.",
                )
            })?;
            validate_plan_start_fields(
                scan_id,
                source,
                selection_revision,
                selection_matches,
                retained,
            )?;
            if retained.started {
                return Err(DesktopRestoreError::new(
                    "RESTORE_PLAN_INVALID",
                    "A recovery plan can be started only once.",
                ));
            }
            if !Arc::ptr_eq(&retained.immutable, &plan.immutable) {
                return Err(DesktopRestoreError::new(
                    "RESTORE_PLAN_EXPIRED",
                    "The recovery plan changed during start authorization.",
                ));
            }
            let destination = inner
                .destinations
                .get(&plan.summary.destination_id)
                .ok_or_else(DesktopRestoreError::destination_expired)?;
            validate_start_destination_binding(scan_id, source, destination)?;
            if !Arc::ptr_eq(&prepared.capability, &destination.admitted.capability)
                || destination.baseline != prepared.destination_baseline
            {
                return Err(DesktopRestoreError::destination_expired());
            }
            if inner.jobs.contains_key(&job.job_id) {
                return Err(DesktopRestoreError::internal());
            }
            inner
                .plans
                .get_mut(&plan.summary.plan_id)
                .ok_or_else(|| {
                    DesktopRestoreError::new(
                        "RESTORE_PLAN_EXPIRED",
                        "The recovery plan is no longer retained.",
                    )
                })?
                .started = true;
            inner.jobs.insert(job.job_id.clone(), Arc::clone(&job));
        }

        Ok(CommittedRestoreStart {
            job,
            plan,
            destination_root: prepared.destination_root,
        })
    }

    fn launch_restore(
        &self,
        committed: CommittedRestoreStart,
    ) -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {
        let CommittedRestoreStart {
            job,
            plan,
            destination_root,
        } = committed;
        let immediate = job.snapshot()?;
        let retained_plan_id = plan.summary.plan_id.clone();
        let retained_engine_job_id = plan.engine_job_id.clone();
        let runtime = Arc::clone(&self.runtime);
        let spawn_result = self.runtime.spawn_worker(Box::new({
            let job = Arc::clone(&job);
            move || {
                run_restore_worker(job, plan, destination_root, runtime);
            }
        }));
        if let Err(error) = spawn_result {
            if let Ok(mut state) = job.state.lock() {
                state.status = RestoreJobStatusDto::Failed;
                push_job_warning(&mut state, "RESTORE_WORKER_START_FAILED");
            }
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| DesktopRestoreError::internal())?;
            let retained_job_is_this_attempt = inner
                .jobs
                .get(&job.job_id)
                .is_some_and(|retained| Arc::ptr_eq(retained, &job));
            if retained_job_is_this_attempt {
                inner.jobs.remove(&job.job_id);
                if let Some(retained_plan) = inner.plans.get_mut(&retained_plan_id) {
                    if retained_plan.engine_job_id == retained_engine_job_id {
                        retained_plan.started = false;
                    }
                }
            }
            return Err(error);
        }
        Ok(immediate)
    }

    #[cfg(test)]
    pub(crate) fn start_restore(
        &self,
        scan: &RestoreScanSnapshot,
        plan_id: &str,
    ) -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {
        let prepared = self.preflight_restore_start(plan_id)?;
        let committed = self.commit_prepared_restore_start_for_snapshot(scan, prepared)?;
        self.launch_restore(committed)
    }

    #[cfg(test)]
    fn commit_prepared_restore_start_for_snapshot(
        &self,
        scan: &RestoreScanSnapshot,
        prepared: PreparedRestoreStart,
    ) -> Result<CommittedRestoreStart, DesktopRestoreError> {
        self.commit_prepared_restore_start_fields(
            &scan.scan_id,
            &scan.source,
            scan.selection_revision,
            |expected| {
                scan.candidates
                    .iter()
                    .map(|selected| selected.candidate.id)
                    .eq(expected.iter().copied())
            },
            prepared,
        )
    }

    pub(crate) fn get_restore_job(
        &self,
        job_id: &str,
    ) -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {
        let job = self.retained_job(job_id)?;
        job.snapshot()
    }

    pub(crate) fn cancel_restore(
        &self,
        job_id: &str,
    ) -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {
        let job = self.retained_job(job_id)?;
        job.cancel.swap(true, Ordering::SeqCst);
        let mut state = job
            .state
            .lock()
            .map_err(|_| DesktopRestoreError::internal())?;
        if matches!(
            state.status,
            RestoreJobStatusDto::Queued | RestoreJobStatusDto::Running
        ) {
            state.status = RestoreJobStatusDto::Cancelling;
        }
        job.snapshot_from(&state)
    }

    pub(crate) fn open_restore_destination(
        &self,
        job_id: &str,
    ) -> Result<OpenRestoreDestinationDto, DesktopRestoreError> {
        let (job, capability) = {
            let inner = self
                .inner
                .lock()
                .map_err(|_| DesktopRestoreError::internal())?;
            let job = Arc::clone(inner.jobs.get(job_id).ok_or_else(|| {
                DesktopRestoreError::new(
                    "RESTORE_JOB_NOT_FOUND",
                    "The recovery job is no longer retained.",
                )
            })?);
            let destination = inner
                .destinations
                .get(&job.destination_id)
                .ok_or_else(DesktopRestoreError::destination_expired)?;
            (job, Arc::clone(&destination.admitted.capability))
        };
        let state = job
            .state
            .lock()
            .map_err(|_| DesktopRestoreError::internal())?;
        if state.status != RestoreJobStatusDto::Completed || state.manifest.is_none() {
            return Err(DesktopRestoreError::new(
                "RESTORE_JOB_NOT_COMPLETE",
                "Only a completed recovery job can open its destination.",
            ));
        }
        drop(state);
        let expected_component = um_restore::job_directory_component(&job.engine_job_id);
        if expected_component != um_restore::job_directory_component(&job.job_id) {
            return Err(DesktopRestoreError::internal());
        }
        capability.open_job_directory(&expected_component)?;
        Ok(OpenRestoreDestinationDto {
            schema_version: RESTORE_SCHEMA_VERSION,
            opened: true,
        })
    }

    fn retained_job(&self, job_id: &str) -> Result<Arc<RetainedRestoreJob>, DesktopRestoreError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| DesktopRestoreError::internal())?;
        inner.jobs.get(job_id).cloned().ok_or_else(|| {
            DesktopRestoreError::new(
                "RESTORE_JOB_NOT_FOUND",
                "The recovery job is no longer retained.",
            )
        })
    }

    #[cfg(test)]
    fn retained_destination_count(&self) -> usize {
        self.inner.lock().unwrap().destinations.len()
    }

    #[cfg(test)]
    fn retained_plan_count(&self) -> usize {
        self.inner.lock().unwrap().plans.len()
    }

    #[cfg(test)]
    fn retained_job_count(&self) -> usize {
        self.inner.lock().unwrap().jobs.len()
    }
}

#[tauri::command]
pub(crate) fn select_restore_destination(
    app: tauri::AppHandle,
    storage: tauri::State<'_, DesktopStorageState>,
    restore: tauri::State<'_, RestoreCoordinator>,
    request_id: String,
    scan_id: String,
) -> Result<Option<DestinationSummaryDto>, DesktopRestoreError> {
    validate_request_id(&request_id)?;
    let before_picker = storage.restore_scan_binding(&scan_id)?;
    let selected = app
        .dialog()
        .file()
        .set_title("Choose a different physical disk for recovered files")
        .blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|_| {
        DesktopRestoreError::new(
            "RESTORE_DESTINATION_INVALID",
            "The selected recovery destination could not be authorized.",
        )
    })?;
    let binding = um_io_windows::open_destination_root_binding(&path)
        .map_err(map_destination_admission_error)?;
    let label = binding.display_label().to_owned();
    let volume_label = binding.volume_label().to_owned();
    let after_picker = storage.restore_scan_binding(&scan_id)?;
    if before_picker.source != after_picker.source {
        return Err(DesktopRestoreError::new(
            "RESTORE_SOURCE_CHANGED",
            "The recovery source changed while the destination was being selected.",
        ));
    }
    restore.admit_destination_binding(
        &after_picker,
        Some(AdmittedDestination {
            label,
            volume_label,
            capability: Arc::new(NativeDestinationCapability { binding }),
        }),
    )
}

#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "Tauri command arguments intentionally mirror the deny-unknown-fields desktop contract"
)]
pub(crate) async fn create_restore_plan(
    storage: tauri::State<'_, DesktopStorageState>,
    restore: tauri::State<'_, RestoreCoordinator>,
    request_id: String,
    scan_id: String,
    selection_revision: String,
    destination_id: String,
    collision_policy: CollisionPolicyDto,
    partial_file_policy: PartialFilePolicyDto,
) -> Result<RestorePlanSummaryDto, DesktopRestoreError> {
    validate_request_id(&request_id)?;
    validate_restore_id(&destination_id, RestoreIdKind::Destination)?;
    let selection_revision = parse_selection_revision(&selection_revision)?;
    let storage = storage.inner().clone();
    let coordinator = restore.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let scan = storage.restore_snapshot(&scan_id, Some(selection_revision))?;
        coordinator.create_restore_plan(
            &scan,
            &destination_id,
            collision_policy,
            partial_file_policy,
        )
    })
    .await
    .map_err(|_| DesktopRestoreError::internal())?
}

#[tauri::command]
pub(crate) async fn start_restore(
    storage: tauri::State<'_, DesktopStorageState>,
    restore: tauri::State<'_, RestoreCoordinator>,
    request_id: String,
    plan_id: String,
) -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {
    validate_request_id(&request_id)?;
    validate_restore_id(&plan_id, RestoreIdKind::Plan)?;
    let storage = storage.inner().clone();
    let coordinator = restore.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (scan_id, selection_revision) = coordinator.plan_binding(&plan_id)?;
        let prepared = coordinator.preflight_restore_start(&plan_id)?;

        // This short callback is the start linearization point. It re-reads
        // the retained selection by borrow and consumes the immutable plan
        // while selection updates are excluded. Destination I/O already
        // finished above; the worker is launched only after the storage and
        // coordinator locks have both been released.
        let committed =
            storage.with_restore_start_selection(&scan_id, selection_revision, |selection| {
                coordinator.commit_prepared_restore_start(&selection, prepared)
            })??;
        coordinator.launch_restore(committed)
    })
    .await
    .map_err(|_| DesktopRestoreError::internal())?
}

#[tauri::command]
pub(crate) fn get_restore_job(
    restore: tauri::State<'_, RestoreCoordinator>,
    request_id: String,
    job_id: String,
) -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {
    validate_request_id(&request_id)?;
    validate_restore_id(&job_id, RestoreIdKind::Job)?;
    restore.get_restore_job(&job_id)
}

#[tauri::command]
pub(crate) fn cancel_restore(
    restore: tauri::State<'_, RestoreCoordinator>,
    request_id: String,
    job_id: String,
) -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {
    validate_request_id(&request_id)?;
    validate_restore_id(&job_id, RestoreIdKind::Job)?;
    restore.cancel_restore(&job_id)
}

#[tauri::command]
pub(crate) fn open_restore_destination(
    restore: tauri::State<'_, RestoreCoordinator>,
    request_id: String,
    job_id: String,
) -> Result<OpenRestoreDestinationDto, DesktopRestoreError> {
    validate_request_id(&request_id)?;
    validate_restore_id(&job_id, RestoreIdKind::Job)?;
    restore.open_restore_destination(&job_id)
}

impl RetainedRestoreJob {
    fn snapshot(&self) -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {
        let state = self
            .state
            .lock()
            .map_err(|_| DesktopRestoreError::internal())?;
        self.snapshot_from(&state)
    }

    fn snapshot_from(
        &self,
        state: &RestoreJobMutable,
    ) -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {
        let bytes_completed = state
            .committed_bytes
            .checked_add(state.current_bytes)
            .ok_or_else(DesktopRestoreError::internal)?
            .min(self.bytes_total);
        Ok(RestoreJobSnapshotDto {
            schema_version: RESTORE_SCHEMA_VERSION,
            job_id: self.job_id.clone(),
            plan_id: self.plan_id.clone(),
            status: state.status,
            items_total: self.items_total.to_string(),
            items_completed: state.items_completed.to_string(),
            items_failed: state.items_failed.to_string(),
            items_cancelled: state.items_cancelled.to_string(),
            bytes_total: self.bytes_total.to_string(),
            bytes_completed: bytes_completed.to_string(),
            current_item: state.current_item.clone(),
            warnings: state.warnings.clone(),
            manifest: visible_manifest(state.status, state.manifest.as_ref()),
        })
    }
}

fn visible_manifest(
    status: RestoreJobStatusDto,
    manifest: Option<&RestoreManifestSummaryDto>,
) -> Option<RestoreManifestSummaryDto> {
    if status.is_terminal() {
        manifest.cloned()
    } else {
        None
    }
}

struct JobCancellation {
    cancelled: Arc<AtomicBool>,
}

impl CancellationProbe for JobCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

struct JobProgress {
    job: Arc<RetainedRestoreJob>,
}

impl ProgressSink for JobProgress {
    fn advanced(&mut self, progress: StreamProgress) {
        if let Ok(mut state) = self.job.state.lock() {
            state.current_bytes = progress.bytes_written.min(progress.logical_size);
        }
    }
}

struct JobObserver {
    job: Arc<RetainedRestoreJob>,
    pending_manifest: Option<RestoreManifestSummaryDto>,
}

impl RestoreJobObserver for JobObserver {
    fn observe(&mut self, event: RestoreJobEvent) {
        let Ok(mut state) = self.job.state.lock() else {
            return;
        };
        match event {
            RestoreJobEvent::ItemStarted {
                item_index,
                candidate_id,
                item_kind,
                ..
            } => {
                state.current_bytes = 0;
                state.current_item = Some(RestoreCurrentItemDto {
                    ordinal: item_index.to_string(),
                    candidate_id: candidate_id.to_string(),
                    kind: match item_kind {
                        RestoreItemKind::File => "file",
                        RestoreItemKind::Directory => "directory",
                    },
                });
            }
            RestoreJobEvent::ItemFinished { outcome, .. } => {
                state.committed_bytes = state
                    .committed_bytes
                    .checked_add(state.current_bytes)
                    .unwrap_or(u64::MAX);
                state.current_bytes = 0;
                state.current_item = None;
                match outcome {
                    RestoreItemOutcome::Published
                    | RestoreItemOutcome::DirectoryCreated
                    | RestoreItemOutcome::DirectoryNeedsReconciliation => {
                        state.items_completed = state.items_completed.saturating_add(1);
                        if outcome == RestoreItemOutcome::DirectoryNeedsReconciliation {
                            push_job_warning(&mut state, "RESTORE_DIRECTORY_NEEDS_RECONCILIATION");
                        }
                    }
                    RestoreItemOutcome::Failed => {
                        state.items_failed = state.items_failed.saturating_add(1);
                    }
                    RestoreItemOutcome::Cancelled => {
                        state.items_cancelled = state.items_cancelled.saturating_add(1);
                    }
                }
            }
        }
    }

    fn manifest_published(&mut self, summary: &RestoreSummary) {
        self.pending_manifest = Some(manifest_summary(summary));
    }
}

fn run_restore_worker(
    job: Arc<RetainedRestoreJob>,
    plan: RetainedRestorePlan,
    destination_root: DestinationRoot,
    runtime: Arc<dyn RestoreRuntime>,
) {
    if let Ok(mut state) = job.state.lock() {
        if state.status == RestoreJobStatusDto::Queued {
            state.status = RestoreJobStatusDto::Running;
        }
    }
    let opened = match runtime.open_source(&plan.source) {
        Ok(opened) => opened,
        Err(error) => {
            fail_job(&job, error.code, false, None);
            return;
        }
    };
    if opened.reader.len() != plan.source.source_len
        || opened.physical_disk_number != plan.source.physical_disk_number
    {
        fail_job(&job, "RESTORE_SOURCE_CHANGED", false, None);
        return;
    }
    let cancellation = JobCancellation {
        cancelled: Arc::clone(&job.cancel),
    };
    let mut progress = JobProgress {
        job: Arc::clone(&job),
    };
    let mut observer = JobObserver {
        job: Arc::clone(&job),
        pending_manifest: None,
    };
    let engine_result = destination_root.restore_job_observed(
        opened.reader.as_ref(),
        &plan.engine_plan,
        &cancellation,
        &mut progress,
        &mut observer,
    );
    let pending_manifest = observer.pending_manifest;
    match engine_result {
        Ok(summary) => complete_job(&job, summary),
        Err(error) => {
            let cancelled = matches!(error, um_restore::RestoreError::Cancelled { .. });
            fail_job(
                &job,
                if cancelled {
                    "RESTORE_CANCELLED"
                } else {
                    "RESTORE_ENGINE_FAILED"
                },
                cancelled,
                pending_manifest,
            );
        }
    }
}

fn complete_job(job: &RetainedRestoreJob, summary: RestoreSummary) {
    let Ok(mut state) = job.state.lock() else {
        return;
    };
    let summary_item_count = summary.items().len();
    let manifest = manifest_summary(&summary);
    commit_engine_success_terminal(
        &mut state,
        job.items_total,
        job.bytes_total,
        summary_item_count,
        manifest,
    );
}

fn commit_engine_success_terminal(
    state: &mut RestoreJobMutable,
    items_total: u64,
    bytes_total: u64,
    summary_item_count: usize,
    manifest: RestoreManifestSummaryDto,
) {
    let summary_items = u64::try_from(summary_item_count).unwrap_or(u64::MAX);
    let observer_matches_manifest = state.items_completed == summary_items;
    let all_items_completed = observer_matches_manifest
        && state.items_completed == items_total
        && state.items_failed == 0
        && state.items_cancelled == 0
        && summary_item_count == usize::try_from(items_total).unwrap_or(usize::MAX);
    state.current_bytes = 0;
    state.current_item = None;
    state.manifest = Some(manifest);
    if !all_items_completed {
        // The engine returned success only after publishing this manifest, so
        // its summary is the durable terminal authority. Normalize all public
        // counters together: mixed observer dispositions could otherwise
        // exceed the immutable total and make already-published evidence
        // unparsable.
        state.items_completed = summary_items;
        state.items_failed = 0;
        state.items_cancelled = 0;
        state.committed_bytes = bytes_total;
        state.status = RestoreJobStatusDto::Failed;
        push_job_warning(state, "RESTORE_TERMINAL_COUNT_MISMATCH");
        return;
    }
    state.committed_bytes = bytes_total;
    state.status = RestoreJobStatusDto::Completed;
}

fn manifest_summary(summary: &RestoreSummary) -> RestoreManifestSummaryDto {
    RestoreManifestSummaryDto {
        manifest_sha256: hex::encode(summary.manifest_sha256()),
        completion_status: match summary.completion_status() {
            RestoreCompletionStatus::CompletedDurable => "completedDurable",
            RestoreCompletionStatus::NeedsReconciliation => "needsReconciliation",
        },
        published_items: summary.items().len().to_string(),
        partial_items: summary
            .items()
            .iter()
            .filter(|item| item.is_partial())
            .count()
            .to_string(),
    }
}

fn fail_job(
    job: &RetainedRestoreJob,
    warning: &'static str,
    cancelled: bool,
    manifest: Option<RestoreManifestSummaryDto>,
) {
    let Ok(mut state) = job.state.lock() else {
        return;
    };
    state.current_item = None;
    state.current_bytes = 0;
    state.manifest = manifest;
    state.status = if cancelled {
        RestoreJobStatusDto::Cancelled
    } else {
        RestoreJobStatusDto::Failed
    };
    push_job_warning(&mut state, warning);
}

fn push_job_warning(state: &mut RestoreJobMutable, warning: &'static str) {
    if state.warnings.len() < MAX_RESTORE_WARNINGS
        && !state.warnings.iter().any(|item| item == warning)
    {
        state.warnings.push(warning.to_owned());
    }
}

fn validate_scan_destination_binding(
    scan: &RestoreScanSnapshot,
    destination: &RetainedDestinationAuthority,
) -> Result<(), DesktopRestoreError> {
    if destination.scan_id != scan.scan_id || destination.source != scan.source {
        return Err(DesktopRestoreError::new(
            "RESTORE_SOURCE_CHANGED",
            "The destination is not bound to this scan source.",
        ));
    }
    Ok(())
}

fn validate_plan_start_fields(
    scan_id: &str,
    source: &ScanSourceBinding,
    selection_revision: u64,
    selection_matches: impl FnOnce(&[u64]) -> bool,
    plan: &RetainedRestorePlan,
) -> Result<(), DesktopRestoreError> {
    if plan.summary.scan_id != scan_id || plan.source != *source {
        return Err(DesktopRestoreError::new(
            "RESTORE_SOURCE_CHANGED",
            "The recovery source changed after plan creation.",
        ));
    }
    if plan.selection_revision != selection_revision {
        return Err(DesktopRestoreError::new(
            "RESTORE_SELECTION_STALE",
            "The recovery selection changed after plan creation.",
        ));
    }
    if !selection_matches(&plan.ordered_candidate_ids) {
        return Err(DesktopRestoreError::new(
            "RESTORE_SELECTION_STALE",
            "The ordered recovery selection changed after plan creation.",
        ));
    }
    Ok(())
}

fn validate_start_destination_binding(
    scan_id: &str,
    source: &ScanSourceBinding,
    destination: &RetainedDestinationAuthority,
) -> Result<(), DesktopRestoreError> {
    if destination.scan_id != scan_id || destination.source != *source {
        return Err(DesktopRestoreError::new(
            "RESTORE_SOURCE_CHANGED",
            "The destination is not bound to this scan source.",
        ));
    }
    Ok(())
}

fn validate_destination_baseline(
    baseline: &DestinationRevalidation,
    current: &DestinationRevalidation,
) -> Result<(), DesktopRestoreError> {
    if !current.reparse_safe
        || !current.file_system.eq_ignore_ascii_case("NTFS")
        || baseline.physical_disk_number != current.physical_disk_number
        || !baseline
            .file_system
            .eq_ignore_ascii_case(&current.file_system)
    {
        return Err(DesktopRestoreError::destination_expired());
    }
    Ok(())
}

fn validate_destination_separation(
    source: &ScanSourceBinding,
    destination: &DestinationRevalidation,
) -> Result<(), DesktopRestoreError> {
    um_io_windows::validate_destination_policy(
        Some(source.physical_disk_number),
        Some(source.physical_disk_number),
        &[destination.physical_disk_number],
        &destination.file_system,
    )
    .map_err(|_| {
        DesktopRestoreError::new(
            "RESTORE_DIFFERENT_DISK_REQUIRED",
            "Recovery requires a known NTFS destination on a different physical disk.",
        )
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DigestDocument<'a> {
    version: u32,
    scan_id: &'a str,
    source: DigestSource<'a>,
    destination_id: &'a str,
    selection_revision: u64,
    collision_policy: CollisionPolicyDto,
    partial_file_policy: PartialFilePolicyDto,
    ordered_items: Vec<DigestItem<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DigestSource<'a> {
    inventory_generation: &'a str,
    volume_id: &'a str,
    source_len: u64,
    file_system: &'a str,
    physical_disk_number: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DigestItem<'a> {
    candidate: &'a um_core::Candidate,
    expected_sha256: Option<String>,
    warnings: &'a [String],
}

fn plan_digest(
    scan: &RestoreScanSnapshot,
    destination_id: &str,
    collision_policy: CollisionPolicyDto,
    partial_file_policy: PartialFilePolicyDto,
) -> Result<String, DesktopRestoreError> {
    let document = DigestDocument {
        version: 1,
        scan_id: &scan.scan_id,
        source: DigestSource {
            inventory_generation: &scan.source.inventory_generation,
            volume_id: &scan.source.volume_id,
            source_len: scan.source.source_len,
            file_system: &scan.source.file_system,
            physical_disk_number: scan.source.physical_disk_number,
        },
        destination_id,
        selection_revision: scan.selection_revision,
        collision_policy,
        partial_file_policy,
        ordered_items: scan
            .candidates
            .iter()
            .map(|selected| DigestItem {
                candidate: &selected.candidate,
                expected_sha256: selected.expected_sha256.map(hex::encode),
                warnings: &selected.warnings,
            })
            .collect(),
    };
    let bytes = serde_json::to_vec(&document).map_err(|_| DesktopRestoreError::internal())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn sanitize_restore_label(value: &str, fallback: &str) -> String {
    let sanitized = value
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    *character,
                    '/' | '\\'
                        | '\u{202a}'
                        | '\u{202b}'
                        | '\u{202c}'
                        | '\u{202d}'
                        | '\u{202e}'
                        | '\u{2066}'
                        | '\u{2067}'
                        | '\u{2068}'
                        | '\u{2069}'
                )
        })
        .take(128)
        .collect::<String>();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn sanitize_restore_warnings(warnings: &[String]) -> Vec<String> {
    warnings
        .iter()
        .take(MAX_RESTORE_WARNINGS)
        .map(|warning| {
            warning
                .chars()
                .filter(|character| {
                    !character.is_control() && !matches!(*character, '/' | '\\' | ':')
                })
                .take(MAX_RESTORE_WARNING_SCALARS)
                .collect::<String>()
        })
        .filter(|warning| !warning.trim().is_empty())
        .collect()
}

fn parse_internal_decimal(value: &str) -> Result<u64, DesktopRestoreError> {
    value.parse().map_err(|_| DesktopRestoreError::internal())
}

#[derive(Clone, Copy)]
enum RestoreIdKind {
    Destination,
    Plan,
    Job,
}

fn validate_restore_id(value: &str, kind: RestoreIdKind) -> Result<(), DesktopRestoreError> {
    let valid = !value.is_empty()
        && value.chars().count() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        return Ok(());
    }
    Err(match kind {
        RestoreIdKind::Destination => DesktopRestoreError::destination_expired(),
        RestoreIdKind::Plan => DesktopRestoreError::new(
            "RESTORE_PLAN_EXPIRED",
            "The recovery plan is no longer retained.",
        ),
        RestoreIdKind::Job => DesktopRestoreError::new(
            "RESTORE_JOB_NOT_FOUND",
            "The recovery job is no longer retained.",
        ),
    })
}

fn parse_selection_revision(value: &str) -> Result<u64, DesktopRestoreError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(DesktopRestoreError::new(
            "RESTORE_PLAN_INVALID",
            "The recovery selection revision is invalid.",
        ));
    }
    value.parse().map_err(|_| {
        DesktopRestoreError::new(
            "RESTORE_PLAN_INVALID",
            "The recovery selection revision is invalid.",
        )
    })
}

fn create_opaque_id(prefix: &str) -> Result<String, DesktopRestoreError> {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|_| DesktopRestoreError::internal())?;
    Ok(format!("{prefix}-{}", hex::encode(random)))
}

fn map_destination_admission_error(_error: um_io_windows::StorageError) -> DesktopRestoreError {
    DesktopRestoreError::new(
        "RESTORE_DESTINATION_INVALID",
        "Choose a direct, local NTFS folder on one known physical disk.",
    )
}

fn map_destination_storage_error(_error: um_io_windows::StorageError) -> DesktopRestoreError {
    DesktopRestoreError::destination_expired()
}

fn map_broker_error(error: um_broker_client::BrokerClientError) -> DesktopRestoreError {
    match error {
        um_broker_client::BrokerClientError::SourceNotFound
        | um_broker_client::BrokerClientError::SourceIdentityMismatch
        | um_broker_client::BrokerClientError::SourceUnavailable
        | um_broker_client::BrokerClientError::SourceChanged => DesktopRestoreError::new(
            "RESTORE_SOURCE_CHANGED",
            "The recovery source identity changed.",
        ),
        _ => DesktopRestoreError::new(
            "RESTORE_INTERNAL",
            "The read-only recovery source could not be reopened.",
        ),
    }
}

fn map_restore_plan_error(_error: um_restore::RestoreError) -> DesktopRestoreError {
    DesktopRestoreError::new(
        "RESTORE_PLAN_INVALID",
        "The selected items could not form a bounded recovery plan.",
    )
}

fn map_restore_start_error(_error: um_restore::RestoreError) -> DesktopRestoreError {
    DesktopRestoreError::destination_expired()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::{ambient_authority, fs::Dir};
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc, Condvar, Mutex,
    };
    use std::time::{Duration, Instant};
    use um_core::{
        Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
        MetadataConfidence, Timestamps,
    };
    use um_io_common::MemImageReader;

    struct OpenGate {
        released: Mutex<bool>,
        changed: Condvar,
    }

    impl OpenGate {
        fn blocked() -> Self {
            Self {
                released: Mutex::new(false),
                changed: Condvar::new(),
            }
        }

        fn release(&self) {
            *self.released.lock().unwrap() = true;
            self.changed.notify_all();
        }

        fn wait(&self) {
            let mut released = self.released.lock().unwrap();
            while !*released {
                released = self.changed.wait(released).unwrap();
            }
        }
    }

    struct TestRuntime {
        bytes: Vec<u8>,
        disk_number: u32,
        gate: Option<Arc<OpenGate>>,
        opens: AtomicUsize,
    }

    impl RestoreRuntime for TestRuntime {
        fn open_source(
            &self,
            binding: &crate::results::ScanSourceBinding,
        ) -> Result<OpenedRestoreSource, DesktopRestoreError> {
            self.opens.fetch_add(1, Ordering::SeqCst);
            if let Some(gate) = &self.gate {
                gate.wait();
            }
            Ok(OpenedRestoreSource {
                reader: Box::new(MemImageReader::new(&binding.volume_id, self.bytes.clone())),
                physical_disk_number: self.disk_number,
            })
        }
    }

    struct FailingSpawnRuntime {
        inner: TestRuntime,
        fail_next_spawn: AtomicBool,
    }

    impl RestoreRuntime for FailingSpawnRuntime {
        fn open_source(
            &self,
            binding: &crate::results::ScanSourceBinding,
        ) -> Result<OpenedRestoreSource, DesktopRestoreError> {
            self.inner.open_source(binding)
        }

        fn spawn_worker(
            &self,
            worker: Box<dyn FnOnce() + Send + 'static>,
        ) -> Result<(), DesktopRestoreError> {
            if self.fail_next_spawn.swap(false, Ordering::SeqCst) {
                return Err(DesktopRestoreError::internal());
            }
            std::thread::Builder::new()
                .name("undelete-restore-worker-test".into())
                .spawn(worker)
                .map(|_| ())
                .map_err(|_| DesktopRestoreError::internal())
        }
    }

    struct TestDestination {
        root: std::fs::File,
        snapshot: Mutex<DestinationRevalidation>,
        valid: AtomicBool,
        opened_job_components: Mutex<Vec<String>>,
    }

    impl TestDestination {
        fn new(root: std::fs::File, disk_number: u32, free_bytes: u64) -> Self {
            Self {
                root,
                snapshot: Mutex::new(DestinationRevalidation {
                    file_system: "NTFS".into(),
                    free_bytes,
                    physical_disk_number: disk_number,
                    reparse_safe: true,
                }),
                valid: AtomicBool::new(true),
                opened_job_components: Mutex::new(Vec::new()),
            }
        }

        fn set_physical_disk_number(&self, physical_disk_number: u32) {
            self.snapshot.lock().unwrap().physical_disk_number = physical_disk_number;
        }
    }

    impl DestinationCapability for TestDestination {
        fn revalidate(&self) -> Result<DestinationRevalidation, DesktopRestoreError> {
            self.valid
                .load(Ordering::SeqCst)
                .then(|| self.snapshot.lock().unwrap().clone())
                .ok_or_else(DesktopRestoreError::destination_expired)
        }

        fn try_clone_root(&self) -> Result<std::fs::File, DesktopRestoreError> {
            self.root
                .try_clone()
                .map_err(|_| DesktopRestoreError::destination_expired())
        }

        fn open_job_directory(&self, job_component: &str) -> Result<(), DesktopRestoreError> {
            let root = Dir::from_std_file(self.try_clone_root()?);
            let job_dir = cap_fs_ext::DirExt::open_dir_nofollow(&root, job_component)
                .map_err(|_| DesktopRestoreError::destination_expired())?;
            let metadata = job_dir
                .dir_metadata()
                .map_err(|_| DesktopRestoreError::destination_expired())?;
            if !metadata.is_dir() {
                return Err(DesktopRestoreError::destination_expired());
            }
            self.opened_job_components
                .lock()
                .unwrap()
                .push(job_component.to_owned());
            Ok(())
        }
    }

    struct BlockingRevalidation {
        blocking: AtomicBool,
        entered: Mutex<bool>,
        released: Mutex<bool>,
        changed: Condvar,
    }

    impl BlockingRevalidation {
        fn new() -> Self {
            Self {
                blocking: AtomicBool::new(false),
                entered: Mutex::new(false),
                released: Mutex::new(false),
                changed: Condvar::new(),
            }
        }

        fn block_next(&self) {
            *self.entered.lock().unwrap() = false;
            *self.released.lock().unwrap() = false;
            self.blocking.store(true, Ordering::SeqCst);
        }

        fn wait_until_entered(&self) {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut entered = self.entered.lock().unwrap();
            while !*entered {
                let remaining = deadline.saturating_duration_since(Instant::now());
                assert!(
                    !remaining.is_zero(),
                    "destination revalidation never started"
                );
                let (next, timeout) = self.changed.wait_timeout(entered, remaining).unwrap();
                entered = next;
                assert!(
                    !timeout.timed_out(),
                    "destination revalidation never started"
                );
            }
        }

        fn wait_if_blocked(&self) {
            if !self.blocking.swap(false, Ordering::SeqCst) {
                return;
            }
            *self.entered.lock().unwrap() = true;
            self.changed.notify_all();
            let mut released = self.released.lock().unwrap();
            while !*released {
                released = self.changed.wait(released).unwrap();
            }
        }

        fn release(&self) {
            *self.released.lock().unwrap() = true;
            self.changed.notify_all();
        }
    }

    struct BlockingDestination {
        inner: TestDestination,
        gate: Arc<BlockingRevalidation>,
    }

    impl DestinationCapability for BlockingDestination {
        fn revalidate(&self) -> Result<DestinationRevalidation, DesktopRestoreError> {
            self.gate.wait_if_blocked();
            self.inner.revalidate()
        }

        fn try_clone_root(&self) -> Result<std::fs::File, DesktopRestoreError> {
            self.inner.try_clone_root()
        }

        fn open_job_directory(&self, job_component: &str) -> Result<(), DesktopRestoreError> {
            self.inner.open_job_directory(job_component)
        }
    }

    fn directory_file(path: &std::path::Path) -> std::fs::File {
        Dir::open_ambient_dir(path, ambient_authority())
            .unwrap()
            .into_std_file()
    }

    fn source_binding(source_len: u64) -> crate::results::ScanSourceBinding {
        crate::results::ScanSourceBinding {
            inventory_generation: "generation-fixture".into(),
            volume_id: "volume-fixture".into(),
            source_len,
            file_system: "ntfs".into(),
            physical_disk_number: 7,
        }
    }

    fn file_candidate(
        id: u64,
        name: &str,
        size: u64,
        extent_len: u64,
        availability: ExtentAvailability,
        state: CandidateState,
    ) -> RestoreSelectedCandidate {
        RestoreSelectedCandidate {
            candidate: Candidate {
                id,
                kind: CandidateKind::File,
                method: DiscoveryMethod::NtfsMetadata,
                state,
                name: name.into(),
                name_certain: true,
                parent_path: vec!["Recovered".into()],
                metadata_confidence: MetadataConfidence::High,
                size,
                timestamps: Timestamps::default(),
                extents: (extent_len > 0)
                    .then_some(ExtentRun {
                        logical_offset: 0,
                        physical_offset: Some(0),
                        len: extent_len,
                        availability,
                    })
                    .into_iter()
                    .collect(),
                record_ref: id,
                sequence: Some(1),
                warnings: Vec::new(),
            },
            expected_sha256: None,
            warnings: Vec::new(),
        }
    }

    fn directory_candidate(id: u64, name: &str, reported_size: u64) -> RestoreSelectedCandidate {
        RestoreSelectedCandidate {
            candidate: Candidate {
                id,
                kind: CandidateKind::Directory,
                method: DiscoveryMethod::NtfsMetadata,
                state: CandidateState::CompleteUnvalidated,
                name: name.into(),
                name_certain: true,
                parent_path: vec!["Recovered".into()],
                metadata_confidence: MetadataConfidence::High,
                size: reported_size,
                timestamps: Timestamps::default(),
                extents: Vec::new(),
                record_ref: id,
                sequence: Some(1),
                warnings: Vec::new(),
            },
            expected_sha256: None,
            warnings: Vec::new(),
        }
    }

    fn snapshot(candidates: Vec<RestoreSelectedCandidate>, source_len: u64) -> RestoreScanSnapshot {
        RestoreScanSnapshot {
            scan_id: "scan-fixture".into(),
            source: source_binding(source_len),
            selection_revision: 4,
            candidates,
        }
    }

    fn new_coordinator(runtime: Arc<TestRuntime>) -> RestoreCoordinator {
        RestoreCoordinator::with_runtime(runtime)
    }

    fn admit(
        coordinator: &RestoreCoordinator,
        snapshot: &RestoreScanSnapshot,
        root: &std::path::Path,
    ) -> (DestinationSummaryDto, Arc<TestDestination>) {
        let capability = Arc::new(TestDestination::new(directory_file(root), 9, 1_000_000));
        let summary = coordinator
            .admit_destination(
                snapshot,
                Some(AdmittedDestination {
                    label: "Fixture destination".into(),
                    volume_label: "Fixture volume".into(),
                    capability: capability.clone(),
                }),
            )
            .unwrap()
            .unwrap();
        (summary, capability)
    }

    fn wait_terminal(coordinator: &RestoreCoordinator, job_id: &str) -> RestoreJobSnapshotDto {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let job = coordinator.get_restore_job(job_id).unwrap();
            if job.status.is_terminal() {
                return job;
            }
            assert!(Instant::now() < deadline, "restore job did not terminate");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn restore_destination_map_is_bounded_and_picker_cancellation_creates_no_authority() {
        let runtime = Arc::new(TestRuntime {
            bytes: vec![0; 8],
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let scan = snapshot(
            vec![file_candidate(
                1,
                "one.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        assert!(coordinator
            .admit_destination(&scan, None)
            .unwrap()
            .is_none());
        assert_eq!(coordinator.retained_destination_count(), 0);

        let temp = tempfile::tempdir().unwrap();
        for index in 0..MAX_DESTINATION_AUTHORITIES {
            let capability = Arc::new(TestDestination::new(
                directory_file(temp.path()),
                9,
                1_000_000,
            ));
            coordinator
                .admit_destination(
                    &scan,
                    Some(AdmittedDestination {
                        label: format!("Destination {index}"),
                        volume_label: "Fixture".into(),
                        capability,
                    }),
                )
                .unwrap();
        }
        assert_eq!(
            coordinator.retained_destination_count(),
            MAX_DESTINATION_AUTHORITIES
        );

        let error = coordinator
            .admit_destination(
                &scan,
                Some(AdmittedDestination {
                    label: "Overflow".into(),
                    volume_label: "Fixture".into(),
                    capability: Arc::new(TestDestination::new(
                        directory_file(temp.path()),
                        9,
                        1_000_000,
                    )),
                }),
            )
            .unwrap_err();
        assert_eq!(error.code, "RESTORE_DESTINATION_LIMIT");
        assert_eq!(
            coordinator.retained_destination_count(),
            MAX_DESTINATION_AUTHORITIES
        );
    }

    #[test]
    fn restore_plan_binds_selection_source_destination_policies_and_digest() {
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let scan = snapshot(
            vec![file_candidate(
                11,
                "complete.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (first_destination, _) = admit(&coordinator, &scan, temp.path());
        let first = coordinator
            .create_restore_plan(
                &scan,
                &first_destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();
        assert_eq!(first.scan_id, scan.scan_id);
        assert_eq!(first.destination_id, first_destination.destination_id);
        assert_eq!(first.selection_revision, "4");
        assert_eq!(first.collision_policy, CollisionPolicyDto::Rename);
        assert_eq!(
            first.partial_file_policy,
            PartialFilePolicyDto::CompleteOnly
        );
        assert_eq!(first.items_total, "1");
        assert_eq!(first.logical_bytes, "8");
        assert_eq!(first.plan_digest.len(), 64);
        assert!(first
            .plan_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit()));

        let (second_destination, _) = admit(&coordinator, &scan, temp.path());
        let second = coordinator
            .create_restore_plan(
                &scan,
                &second_destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::ZeroFillAndMap,
            )
            .unwrap();
        assert_ne!(first.plan_digest, second.plan_digest);

        let mut stale = scan.clone();
        stale.selection_revision += 1;
        assert_eq!(
            coordinator
                .start_restore(&stale, &first.plan_id)
                .unwrap_err()
                .code,
            "RESTORE_SELECTION_STALE"
        );
        let mut changed_source = scan.clone();
        changed_source.source.source_len += 1;
        assert_eq!(
            coordinator
                .start_restore(&changed_source, &first.plan_id)
                .unwrap_err()
                .code,
            "RESTORE_SOURCE_CHANGED"
        );
    }

    #[test]
    fn restore_plan_counts_only_file_content_as_logical_bytes() {
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let scan = snapshot(
            vec![
                directory_candidate(12, "Folder", 65_536),
                file_candidate(
                    13,
                    "content.bin",
                    8,
                    8,
                    ExtentAvailability::FreeInSnapshot,
                    CandidateState::CompleteUnvalidated,
                ),
            ],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (destination, _) = admit(&coordinator, &scan, temp.path());

        let plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();

        assert_eq!(plan.files_total, "1");
        assert_eq!(plan.directories_total, "1");
        assert_eq!(plan.logical_bytes, "8");
    }

    #[test]
    fn restore_start_preflight_shares_the_immutable_plan_payload() {
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let scan = snapshot(
            vec![file_candidate(
                131,
                "shared-plan.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (destination, _) = admit(&coordinator, &scan, temp.path());
        let plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();
        let retained_payload = Arc::clone(
            &coordinator
                .inner
                .lock()
                .unwrap()
                .plans
                .get(&plan.plan_id)
                .unwrap()
                .immutable,
        );

        let prepared = coordinator.preflight_restore_start(&plan.plan_id).unwrap();

        assert!(
            Arc::ptr_eq(&prepared.plan.immutable, &retained_payload),
            "preflight must clone only the immutable Arc, never the 110k-item plan payload"
        );
    }

    #[test]
    fn restore_plan_item_bound_covers_the_complete_native_selection_bound() {
        assert_eq!(
            um_restore::MAX_RESTORE_JOB_ITEMS,
            crate::results::MAX_RETAINED_CANDIDATES_PER_SCAN,
            "every candidate that native selection can retain must fit one immutable restore plan"
        );
    }

    #[test]
    fn restore_job_snapshot_never_exposes_a_manifest_before_terminal_status() {
        let manifest = RestoreManifestSummaryDto {
            manifest_sha256: "00".repeat(32),
            completion_status: "completedDurable",
            published_items: "0".into(),
            partial_items: "0".into(),
        };

        assert!(visible_manifest(RestoreJobStatusDto::Queued, Some(&manifest)).is_none());
        assert!(visible_manifest(RestoreJobStatusDto::Running, Some(&manifest)).is_none());
        assert!(visible_manifest(RestoreJobStatusDto::Cancelling, Some(&manifest)).is_none());
        assert_eq!(
            visible_manifest(RestoreJobStatusDto::Failed, Some(&manifest)),
            Some(manifest)
        );
    }

    #[test]
    fn restore_late_cancel_request_does_not_rewrite_a_failed_engine_terminal() {
        let manifest = RestoreManifestSummaryDto {
            manifest_sha256: "11".repeat(32),
            completion_status: "completedDurable",
            published_items: "0".into(),
            partial_items: "0".into(),
        };
        let job = RetainedRestoreJob {
            job_id: "job-test".into(),
            plan_id: "plan-test".into(),
            destination_id: "destination-test".into(),
            engine_job_id: "engine-test".into(),
            items_total: 1,
            bytes_total: 8,
            cancel: Arc::new(AtomicBool::new(true)),
            state: Mutex::new(RestoreJobMutable {
                status: RestoreJobStatusDto::Running,
                items_completed: 0,
                items_failed: 1,
                items_cancelled: 0,
                committed_bytes: 0,
                current_bytes: 0,
                current_item: None,
                warnings: Vec::new(),
                manifest: None,
            }),
        };

        fail_job(&job, "RESTORE_ENGINE_FAILED", false, Some(manifest.clone()));

        let terminal = job.snapshot().unwrap();
        assert_eq!(terminal.status, RestoreJobStatusDto::Failed);
        assert_eq!(terminal.manifest, Some(manifest));
    }

    #[test]
    fn restore_terminal_count_mismatch_retains_the_published_manifest_summary() {
        let manifest = RestoreManifestSummaryDto {
            manifest_sha256: "22".repeat(32),
            completion_status: "completedDurable",
            published_items: "1".into(),
            partial_items: "0".into(),
        };
        let mut state = RestoreJobMutable {
            status: RestoreJobStatusDto::Running,
            items_completed: 1,
            items_failed: 1,
            items_cancelled: 1,
            committed_bytes: 8,
            current_bytes: 0,
            current_item: None,
            warnings: Vec::new(),
            manifest: None,
        };

        commit_engine_success_terminal(&mut state, 2, 8, 1, manifest.clone());

        assert_eq!(state.status, RestoreJobStatusDto::Failed);
        assert_eq!(state.items_completed, 1);
        assert_eq!(state.items_failed, 0);
        assert_eq!(state.items_cancelled, 0);
        assert_eq!(state.committed_bytes, 8);
        assert_eq!(state.manifest, Some(manifest));
        assert_eq!(state.warnings, vec!["RESTORE_TERMINAL_COUNT_MISMATCH"]);
    }

    #[test]
    fn restore_terminal_missed_observer_event_uses_manifest_publication_count() {
        let manifest = RestoreManifestSummaryDto {
            manifest_sha256: "33".repeat(32),
            completion_status: "needsReconciliation",
            published_items: "1".into(),
            partial_items: "0".into(),
        };
        let mut state = RestoreJobMutable {
            status: RestoreJobStatusDto::Running,
            items_completed: 0,
            items_failed: 0,
            items_cancelled: 0,
            committed_bytes: 0,
            current_bytes: 8,
            current_item: Some(RestoreCurrentItemDto {
                ordinal: "0".into(),
                candidate_id: "91".into(),
                kind: "file",
            }),
            warnings: Vec::new(),
            manifest: None,
        };

        commit_engine_success_terminal(&mut state, 2, 16, 1, manifest.clone());

        assert_eq!(state.status, RestoreJobStatusDto::Failed);
        assert_eq!(state.items_completed, 1);
        assert_eq!(state.items_failed, 0);
        assert_eq!(state.items_cancelled, 0);
        assert_eq!(state.committed_bytes, 16);
        assert_eq!(state.manifest, Some(manifest));
        assert!(state.current_item.is_none());
        assert_eq!(state.current_bytes, 0);
        assert_eq!(state.warnings, vec!["RESTORE_TERMINAL_COUNT_MISMATCH"]);
    }

    #[test]
    fn restore_plan_revalidation_does_not_hold_the_coordinator_lock() {
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let scan = snapshot(
            vec![file_candidate(
                14,
                "content.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (destination, _) = admit(&coordinator, &scan, temp.path());
        let first_plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();
        let first_job = coordinator
            .start_restore(&scan, &first_plan.plan_id)
            .unwrap();
        let completed = wait_terminal(&coordinator, &first_job.job_id);
        assert_eq!(completed.status, RestoreJobStatusDto::Completed);

        let gate = Arc::new(BlockingRevalidation::new());
        let blocking_capability = Arc::new(BlockingDestination {
            inner: TestDestination::new(directory_file(temp.path()), 9, 1_000_000),
            gate: Arc::clone(&gate),
        });
        let blocking_destination = coordinator
            .admit_destination(
                &scan,
                Some(AdmittedDestination {
                    label: "Blocking destination".into(),
                    volume_label: "Fixture".into(),
                    capability: blocking_capability,
                }),
            )
            .unwrap()
            .unwrap();
        gate.block_next();

        let planning_coordinator = coordinator.clone();
        let planning_scan = scan.clone();
        let planning_destination_id = blocking_destination.destination_id.clone();
        let planning = std::thread::spawn(move || {
            planning_coordinator.create_restore_plan(
                &planning_scan,
                &planning_destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
        });
        gate.wait_until_entered();

        let lookup_coordinator = coordinator.clone();
        let lookup_job_id = first_job.job_id.clone();
        let (sent, received) = mpsc::channel();
        let lookup = std::thread::spawn(move || {
            sent.send(lookup_coordinator.get_restore_job(&lookup_job_id))
                .unwrap();
        });
        let lookup_completed_while_revalidation_was_blocked =
            received.recv_timeout(Duration::from_millis(250)).is_ok();
        gate.release();
        planning.join().unwrap().unwrap();
        lookup.join().unwrap();

        assert!(
            lookup_completed_while_revalidation_was_blocked,
            "slow destination I/O must not block job polling or cancellation"
        );
    }

    #[test]
    fn restore_retains_exactly_eight_plan_job_lifecycles_without_evicting_active_work() {
        let gate = Arc::new(OpenGate::blocked());
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: Some(Arc::clone(&gate)),
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let scan = snapshot(
            vec![file_candidate(
                15,
                "bounded.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (destination, _capability) = admit(&coordinator, &scan, temp.path());
        let mut jobs = Vec::new();

        for _ in 0..MAX_RETAINED_RESTORE_PLANS {
            let plan = coordinator
                .create_restore_plan(
                    &scan,
                    &destination.destination_id,
                    CollisionPolicyDto::Rename,
                    PartialFilePolicyDto::CompleteOnly,
                )
                .unwrap();
            jobs.push(coordinator.start_restore(&scan, &plan.plan_id).unwrap());
        }

        assert_eq!(
            coordinator.retained_plan_count(),
            MAX_RETAINED_RESTORE_PLANS
        );
        assert_eq!(coordinator.retained_job_count(), MAX_RETAINED_RESTORE_JOBS);
        let overflow = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap_err();
        assert_eq!(overflow.code, "RESTORE_PLAN_LIMIT");
        assert_eq!(
            coordinator.get_restore_job(&jobs[0].job_id).unwrap().job_id,
            jobs[0].job_id,
            "bounded admission must not evict a running job"
        );

        gate.release();
        for job in jobs {
            assert_eq!(
                wait_terminal(&coordinator, &job.job_id).status,
                RestoreJobStatusDto::Completed
            );
        }
    }

    #[test]
    fn restore_start_rechecks_job_limit_after_blocking_destination_preflight() {
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let scan = snapshot(
            vec![file_candidate(
                151,
                "bounded.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let gate = Arc::new(BlockingRevalidation::new());
        let capability = Arc::new(BlockingDestination {
            inner: TestDestination::new(directory_file(temp.path()), 9, 1_000_000),
            gate: Arc::clone(&gate),
        });
        let destination = coordinator
            .admit_destination(
                &scan,
                Some(AdmittedDestination {
                    label: "Blocking destination".into(),
                    volume_label: "Fixture".into(),
                    capability,
                }),
            )
            .unwrap()
            .unwrap();
        let plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();

        gate.block_next();
        let preflight_coordinator = coordinator.clone();
        let preflight_plan_id = plan.plan_id.clone();
        let preflight = std::thread::spawn(move || {
            preflight_coordinator.preflight_restore_start(&preflight_plan_id)
        });
        gate.wait_until_entered();

        {
            let mut inner = coordinator.inner.lock().unwrap();
            for index in 0..MAX_RETAINED_RESTORE_JOBS {
                let job_id = format!("job-existing-{index}");
                inner.jobs.insert(
                    job_id.clone(),
                    Arc::new(RetainedRestoreJob {
                        job_id,
                        plan_id: format!("plan-existing-{index}"),
                        destination_id: destination.destination_id.clone(),
                        engine_job_id: format!("job-existing-{index}"),
                        items_total: 1,
                        bytes_total: 1,
                        cancel: Arc::new(AtomicBool::new(false)),
                        state: Mutex::new(RestoreJobMutable {
                            status: RestoreJobStatusDto::Queued,
                            items_completed: 0,
                            items_failed: 0,
                            items_cancelled: 0,
                            committed_bytes: 0,
                            current_bytes: 0,
                            current_item: None,
                            warnings: Vec::new(),
                            manifest: None,
                        }),
                    }),
                );
            }
        }
        gate.release();
        let prepared = preflight.join().unwrap().unwrap();
        let error = match coordinator.commit_prepared_restore_start_for_snapshot(&scan, prepared) {
            Ok(_) => panic!("commit must recheck the retained-job limit"),
            Err(error) => error,
        };

        assert_eq!(error.code, "RESTORE_JOB_LIMIT");
        assert_eq!(coordinator.retained_job_count(), MAX_RETAINED_RESTORE_JOBS);
        assert!(
            !coordinator
                .inner
                .lock()
                .unwrap()
                .plans
                .get(&plan.plan_id)
                .unwrap()
                .started,
            "commit-time admission failure must not consume the one-shot plan"
        );
    }

    #[test]
    fn restore_revalidates_destination_identity_before_opening_the_source() {
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(Arc::clone(&runtime));
        let scan = snapshot(
            vec![file_candidate(
                16,
                "identity.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (destination, capability) = admit(&coordinator, &scan, temp.path());
        let plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();

        capability.set_physical_disk_number(10);
        let error = coordinator.start_restore(&scan, &plan.plan_id).unwrap_err();
        assert_eq!(error.code, "RESTORE_DESTINATION_EXPIRED");
        assert_eq!(
            runtime.opens.load(Ordering::SeqCst),
            0,
            "destination drift must fail before the privileged source is opened"
        );
        assert_eq!(coordinator.retained_job_count(), 0);

        capability.set_physical_disk_number(9);
        let started = coordinator.start_restore(&scan, &plan.plan_id).unwrap();
        assert_eq!(
            wait_terminal(&coordinator, &started.job_id).status,
            RestoreJobStatusDto::Completed
        );
    }

    #[test]
    fn restore_worker_rejects_reopened_source_identity_drift_before_destination_writes() {
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 8,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(Arc::clone(&runtime));
        let scan = snapshot(
            vec![file_candidate(
                17,
                "source-drift.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (destination, _) = admit(&coordinator, &scan, temp.path());
        let plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();

        let started = coordinator.start_restore(&scan, &plan.plan_id).unwrap();
        let terminal = wait_terminal(&coordinator, &started.job_id);

        assert_eq!(terminal.status, RestoreJobStatusDto::Failed);
        assert_eq!(terminal.warnings, vec!["RESTORE_SOURCE_CHANGED"]);
        assert!(terminal.manifest.is_none());
        assert_eq!(terminal.items_completed, "0");
        assert_eq!(terminal.items_failed, "0");
        assert_eq!(runtime.opens.load(Ordering::SeqCst), 1);
        assert_eq!(
            std::fs::read_dir(temp.path()).unwrap().count(),
            0,
            "source identity drift must be rejected before any destination entry is created"
        );
    }

    #[test]
    fn restore_worker_spawn_failure_rolls_back_the_invisible_job_and_plan_start() {
        let runtime = Arc::new(FailingSpawnRuntime {
            inner: TestRuntime {
                bytes: b"abcdefgh".to_vec(),
                disk_number: 7,
                gate: None,
                opens: AtomicUsize::new(0),
            },
            fail_next_spawn: AtomicBool::new(true),
        });
        let coordinator = RestoreCoordinator::with_runtime(runtime);
        let scan = snapshot(
            vec![file_candidate(
                18,
                "spawn-failure.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (destination, _) = admit(&coordinator, &scan, temp.path());
        let plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();

        assert_eq!(
            coordinator
                .start_restore(&scan, &plan.plan_id)
                .unwrap_err()
                .code,
            "RESTORE_INTERNAL"
        );
        assert_eq!(
            coordinator.retained_job_count(),
            0,
            "a caller that received no job ID must not leave an invisible retained job"
        );

        let started = coordinator.start_restore(&scan, &plan.plan_id).unwrap();
        assert_eq!(
            wait_terminal(&coordinator, &started.job_id).status,
            RestoreJobStatusDto::Completed,
            "spawn failure must roll back the one-shot plan-start marker"
        );
    }

    #[test]
    fn restore_plan_rejects_ineligible_and_unconsented_best_effort_items() {
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let temp = tempfile::tempdir().unwrap();

        let ineligible = snapshot(
            vec![file_candidate(
                21,
                "metadata-only.bin",
                8,
                0,
                ExtentAvailability::Unknown,
                CandidateState::MetadataOnly,
            )],
            8,
        );
        let (destination, _) = admit(&coordinator, &ineligible, temp.path());
        assert_eq!(
            coordinator
                .create_restore_plan(
                    &ineligible,
                    &destination.destination_id,
                    CollisionPolicyDto::Rename,
                    PartialFilePolicyDto::ZeroFillAndMap,
                )
                .unwrap_err()
                .code,
            "RESTORE_ITEM_INELIGIBLE"
        );

        let partial = snapshot(
            vec![file_candidate(
                22,
                "partial.bin",
                8,
                4,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::Partial,
            )],
            8,
        );
        let partial_coordinator = new_coordinator(Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        }));
        let (destination, _) = admit(&partial_coordinator, &partial, temp.path());
        assert_eq!(
            partial_coordinator
                .create_restore_plan(
                    &partial,
                    &destination.destination_id,
                    CollisionPolicyDto::Rename,
                    PartialFilePolicyDto::CompleteOnly,
                )
                .unwrap_err()
                .code,
            "RESTORE_PARTIAL_POLICY_REQUIRED"
        );
        let plan = partial_coordinator
            .create_restore_plan(
                &partial,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::ZeroFillAndMap,
            )
            .unwrap();
        assert_eq!(plan.best_effort_items, "1");
        let started = partial_coordinator
            .start_restore(&partial, &plan.plan_id)
            .unwrap();
        let completed = wait_terminal(&partial_coordinator, &started.job_id);
        assert_eq!(completed.manifest.as_ref().unwrap().partial_items, "1");
    }

    #[test]
    fn restore_start_returns_immediately_and_publishes_real_terminal_counters_and_manifest() {
        let gate = Arc::new(OpenGate::blocked());
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: Some(gate.clone()),
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime.clone());
        let scan = snapshot(
            vec![file_candidate(
                31,
                "complete.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (destination, _) = admit(&coordinator, &scan, temp.path());
        let plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();

        let started_at = Instant::now();
        let started = coordinator.start_restore(&scan, &plan.plan_id).unwrap();
        assert!(started_at.elapsed() < Duration::from_secs(1));
        assert!(matches!(
            started.status,
            RestoreJobStatusDto::Queued | RestoreJobStatusDto::Running
        ));
        assert_eq!(started.items_completed, "0");
        assert_eq!(started.bytes_completed, "0");
        assert!(started.manifest.is_none());
        assert_eq!(
            coordinator
                .start_restore(&scan, &plan.plan_id)
                .unwrap_err()
                .code,
            "RESTORE_PLAN_INVALID"
        );

        gate.release();
        let completed = wait_terminal(&coordinator, &started.job_id);
        assert_eq!(completed.status, RestoreJobStatusDto::Completed);
        assert_eq!(completed.items_completed, "1");
        assert_eq!(completed.items_failed, "0");
        assert_eq!(completed.items_cancelled, "0");
        assert_eq!(completed.bytes_completed, "8");
        assert_eq!(completed.manifest.as_ref().unwrap().published_items, "1");
        assert_eq!(completed.manifest.as_ref().unwrap().partial_items, "0");
        assert_eq!(runtime.opens.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn restore_cancellation_is_idempotent_and_only_completed_jobs_open_the_bound_job_directory() {
        let gate = Arc::new(OpenGate::blocked());
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: Some(gate.clone()),
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let scan = snapshot(
            vec![file_candidate(
                41,
                "cancel.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        let temp = tempfile::tempdir().unwrap();
        let (destination, capability) = admit(&coordinator, &scan, temp.path());
        let plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();
        let started = coordinator.start_restore(&scan, &plan.plan_id).unwrap();
        assert_eq!(
            coordinator
                .open_restore_destination(&started.job_id)
                .unwrap_err()
                .code,
            "RESTORE_JOB_NOT_COMPLETE"
        );
        let first = coordinator.cancel_restore(&started.job_id).unwrap();
        let second = coordinator.cancel_restore(&started.job_id).unwrap();
        assert!(matches!(
            first.status,
            RestoreJobStatusDto::Cancelling | RestoreJobStatusDto::Cancelled
        ));
        assert_eq!(first.job_id, second.job_id);
        gate.release();
        let cancelled = wait_terminal(&coordinator, &started.job_id);
        assert_eq!(cancelled.status, RestoreJobStatusDto::Cancelled);
        let manifest = cancelled
            .manifest
            .as_ref()
            .expect("ordinary cooperative cancellation publishes a final manifest");
        assert_eq!(manifest.published_items, "0");
        assert_eq!(manifest.partial_items, "0");
        assert!(capability.opened_job_components.lock().unwrap().is_empty());

        let completed_runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let completed_coordinator = new_coordinator(completed_runtime);
        let (destination, completed_capability) = admit(&completed_coordinator, &scan, temp.path());
        let plan = completed_coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();
        let job = completed_coordinator
            .start_restore(&scan, &plan.plan_id)
            .unwrap();
        let completed = wait_terminal(&completed_coordinator, &job.job_id);
        assert_eq!(completed.status, RestoreJobStatusDto::Completed);
        assert_eq!(
            completed_coordinator
                .open_restore_destination(&job.job_id)
                .unwrap(),
            OpenRestoreDestinationDto {
                schema_version: 1,
                opened: true,
            }
        );
        assert_eq!(
            *completed_capability.opened_job_components.lock().unwrap(),
            vec![um_restore::job_directory_component(&job.job_id)]
        );
    }

    #[test]
    fn restore_failure_retains_the_published_manifest_summary() {
        let runtime = Arc::new(TestRuntime {
            bytes: b"abcdefgh".to_vec(),
            disk_number: 7,
            gate: None,
            opens: AtomicUsize::new(0),
        });
        let coordinator = new_coordinator(runtime);
        let mut scan = snapshot(
            vec![file_candidate(
                42,
                "hash-mismatch.bin",
                8,
                8,
                ExtentAvailability::FreeInSnapshot,
                CandidateState::CompleteUnvalidated,
            )],
            8,
        );
        scan.candidates[0].expected_sha256 = Some([0xa5; 32]);
        let temp = tempfile::tempdir().unwrap();
        let (destination, _) = admit(&coordinator, &scan, temp.path());
        let plan = coordinator
            .create_restore_plan(
                &scan,
                &destination.destination_id,
                CollisionPolicyDto::Rename,
                PartialFilePolicyDto::CompleteOnly,
            )
            .unwrap();

        let started = coordinator.start_restore(&scan, &plan.plan_id).unwrap();
        let failed = wait_terminal(&coordinator, &started.job_id);

        assert_eq!(failed.status, RestoreJobStatusDto::Failed);
        assert_eq!(failed.items_failed, "1");
        let manifest = failed
            .manifest
            .as_ref()
            .expect("ordinary item failure publishes a truthful final manifest");
        assert_eq!(manifest.published_items, "0");
        assert_eq!(manifest.partial_items, "0");
        assert!(temp
            .path()
            .join(um_restore::job_directory_component(&started.job_id))
            .join("recovery-manifest.json")
            .is_file());
    }
}
