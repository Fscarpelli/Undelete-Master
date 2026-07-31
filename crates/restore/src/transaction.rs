use crate::journal::{hex_lower, JobJournal, JOURNAL_NAME};
use crate::manifest::{
    build_manifest, build_partial_sidecar, DirectoryValidation, ManifestItemDisposition,
    ManifestItemOutcome, NamespaceDurability, PublishedSidecarEvidence, RestoreCompletionStatus,
    RestoreItemResult, RestoreSummary, TemporaryFileDisposition, MANIFEST_NAME,
};
use crate::{
    stream_candidate, CancellationProbe, ContentPlan, DerivedSafePath, ProgressSink, RestoreError,
    SafeRelativePath,
};
use cap_fs_ext::{
    DirExt, FollowSymlinks, MetadataExt, OpenOptionsFollowExt, OpenOptionsMaybeDirExt,
};
use cap_std::fs::{Dir, File, OpenOptions};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use um_core::SourceReader;

pub const RESTORE_SCRATCH_BYTES: usize = 1024 * 1024;
pub const MAX_COLLISION_ATTEMPTS: usize = 10_000;
pub const MAX_RESTORE_JOB_ITEMS: usize = 100_000;
pub const MAX_RESTORE_PATH_COMPONENTS_PER_JOB: usize = 1_000_000;
pub const MAX_PATH_EVIDENCE_BYTES_PER_JOB: usize = 8 * 1024 * 1024;
const MAX_JOB_ID_CHARS: usize = 128;
const MAX_TEMP_CREATE_ATTEMPTS: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamespaceBoundary {
    Sidecar,
    Data,
    Manifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkBoundary {
    PartialSidecar,
    Data,
    Manifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentTransition {
    CreatedBeforeBind,
    Bound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobTerminal {
    Completed,
    Failed { error_kind: &'static str },
    Cancelled,
}

struct TerminalItemOutcome {
    index: usize,
    disposition: ManifestItemDisposition,
    failure_kind: &'static str,
    published_sidecar: Option<PublishedSidecarEvidence>,
    published: Option<RestoreItemResult>,
}

struct ItemRestoreFailure {
    error: Box<RestoreError>,
    published_sidecar: Option<Box<PublishedSidecarEvidence>>,
    published: Option<Box<RestoreItemResult>>,
}

impl ItemRestoreFailure {
    fn with_sidecar(
        error: RestoreError,
        published_sidecar: Option<PublishedSidecarEvidence>,
    ) -> Self {
        Self {
            error: Box::new(error),
            published_sidecar: published_sidecar.map(Box::new),
            published: None,
        }
    }

    fn with_publication(error: RestoreError, published: RestoreItemResult) -> Self {
        Self {
            error: Box::new(error),
            published_sidecar: None,
            published: Some(Box::new(published)),
        }
    }
}

impl From<RestoreError> for ItemRestoreFailure {
    fn from(error: RestoreError) -> Self {
        Self {
            error: Box::new(error),
            published_sidecar: None,
            published: None,
        }
    }
}

struct ItemRestoreContext<'a> {
    job_dir: &'a Dir,
    source: &'a dyn SourceReader,
    cancel: &'a dyn CancellationProbe,
    progress: &'a mut dyn ProgressSink,
    journal: &'a mut JobJournal,
    runtime: &'a dyn TransactionRuntime,
}

trait TransactionRuntime {
    fn create_journal(&self, file: File, job_id: &str) -> JobJournal {
        JobJournal::new(file, job_id)
    }

    fn sync_parent(&self, parent: &Dir, _boundary: NamespaceBoundary) -> NamespaceDurability {
        sync_parent(parent)
    }

    fn hard_link(
        &self,
        parent: &Dir,
        temporary_name: &str,
        final_name: &str,
        _boundary: LinkBoundary,
    ) -> std::io::Result<()> {
        parent.hard_link(temporary_name, parent, final_name)
    }

    fn parent_transition(
        &self,
        _component_index: usize,
        _component: &str,
        _transition: ParentTransition,
    ) -> Result<(), RestoreError> {
        Ok(())
    }
}

struct ProductionRuntime;

impl TransactionRuntime for ProductionRuntime {}

#[derive(Debug, Clone)]
pub struct FileRestorePlan {
    path: SafeRelativePath,
    path_evidence_json: String,
    item: RestorePlanItem,
    warnings: Vec<String>,
}

#[derive(Debug, Clone)]
enum RestorePlanItem {
    File(ContentPlan),
    Directory { candidate_id: u64 },
}

impl FileRestorePlan {
    pub fn file(path: DerivedSafePath, content: ContentPlan) -> Result<Self, RestoreError> {
        let (path, path_evidence_json) = path.into_parts();
        Ok(Self {
            path,
            path_evidence_json,
            item: RestorePlanItem::File(content),
            warnings: Vec::new(),
        })
    }

    pub fn directory(candidate_id: u64, path: DerivedSafePath) -> Result<Self, RestoreError> {
        let (path, path_evidence_json) = path.into_parts();
        Ok(Self {
            path,
            path_evidence_json,
            item: RestorePlanItem::Directory { candidate_id },
            warnings: Vec::new(),
        })
    }

    pub fn with_warnings(mut self, warnings: Vec<String>) -> Result<Self, RestoreError> {
        if warnings.len() > 1024
            || warnings
                .iter()
                .any(|warning| warning.chars().count() > 4096)
        {
            return Err(RestoreError::ManifestSerialization {
                message: "item warning evidence exceeds its bound".into(),
            });
        }
        self.warnings = warnings;
        Ok(self)
    }

    fn candidate_id(&self) -> u64 {
        match &self.item {
            RestorePlanItem::File(content) => content.candidate_id,
            RestorePlanItem::Directory { candidate_id } => *candidate_id,
        }
    }

    fn expected_sha256(&self) -> Option<[u8; 32]> {
        match &self.item {
            RestorePlanItem::File(content) => content.expected_sha256,
            RestorePlanItem::Directory { .. } => None,
        }
    }

    fn item_kind(&self) -> &'static str {
        match &self.item {
            RestorePlanItem::File(_) => "file",
            RestorePlanItem::Directory { .. } => "directory",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RestoreJobPlan {
    job_id: String,
    items: Vec<FileRestorePlan>,
}

impl RestoreJobPlan {
    pub fn new(job_id: &str, items: Vec<FileRestorePlan>) -> Result<Self, RestoreError> {
        let job_id_chars = job_id.chars().count();
        if job_id_chars == 0
            || job_id_chars > MAX_JOB_ID_CHARS
            || job_id
                .chars()
                .any(|character| character <= '\u{1f}' || character == '\u{7f}')
        {
            return Err(RestoreError::InvalidJobId {
                maximum: MAX_JOB_ID_CHARS,
            });
        }
        if items.is_empty() || items.len() > MAX_RESTORE_JOB_ITEMS {
            return Err(RestoreError::InvalidJobItemCount {
                actual: items.len(),
                maximum: MAX_RESTORE_JOB_ITEMS,
            });
        }
        let path_components = checked_job_budget_total(
            items.iter().map(|item| item.path.components().len()),
            MAX_RESTORE_PATH_COMPONENTS_PER_JOB,
        )
        .map_err(|actual| RestoreError::JobPathComponentLimit {
            actual,
            maximum: MAX_RESTORE_PATH_COMPONENTS_PER_JOB,
        })?;
        debug_assert!(path_components <= MAX_RESTORE_PATH_COMPONENTS_PER_JOB);
        let path_evidence_bytes = checked_job_budget_total(
            items.iter().map(|item| item.path_evidence_json.len()),
            MAX_PATH_EVIDENCE_BYTES_PER_JOB,
        )
        .map_err(|actual| RestoreError::JobPathEvidenceLimit {
            actual,
            maximum: MAX_PATH_EVIDENCE_BYTES_PER_JOB,
        })?;
        debug_assert!(path_evidence_bytes <= MAX_PATH_EVIDENCE_BYTES_PER_JOB);
        Ok(Self {
            job_id: job_id.to_owned(),
            items,
        })
    }
}

fn checked_job_budget_total(
    values: impl IntoIterator<Item = usize>,
    maximum: usize,
) -> Result<usize, usize> {
    let mut total = 0usize;
    for value in values {
        total = total.checked_add(value).ok_or(usize::MAX)?;
        if total > maximum {
            return Err(total);
        }
    }
    Ok(total)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

pub struct DestinationRoot {
    dir: Dir,
    baseline_identity: FileIdentity,
}

impl DestinationRoot {
    pub fn from_retained_file(retained_directory: std::fs::File) -> Result<Self, RestoreError> {
        let dir = Dir::from_std_file(retained_directory);
        let metadata = dir
            .dir_metadata()
            .map_err(|error| destination_io("query retained root metadata", error))?;
        if !metadata.is_dir() {
            return Err(RestoreError::DestinationRootNotDirectory);
        }
        let baseline_identity = metadata_identity(&metadata);
        Ok(Self {
            dir,
            baseline_identity,
        })
    }

    pub fn restore_job(
        &self,
        source: &dyn SourceReader,
        plan: &RestoreJobPlan,
        cancel: &dyn CancellationProbe,
        progress: &mut dyn ProgressSink,
    ) -> Result<RestoreSummary, RestoreError> {
        self.restore_job_with_runtime(source, plan, cancel, progress, &ProductionRuntime)
    }

    fn restore_job_with_runtime(
        &self,
        source: &dyn SourceReader,
        plan: &RestoreJobPlan,
        cancel: &dyn CancellationProbe,
        progress: &mut dyn ProgressSink,
        runtime: &dyn TransactionRuntime,
    ) -> Result<RestoreSummary, RestoreError> {
        self.revalidate()?;
        let job_directory_name = job_directory_component(&plan.job_id);
        self.dir
            .create_dir(&job_directory_name)
            .map_err(|error| destination_io("create unique job directory", error))?;
        let job_dir = self
            .dir
            .open_dir_nofollow(&job_directory_name)
            .map_err(|error| destination_io("bind unique job directory", error))?;

        let journal_file = create_new_protected(&job_dir, JOURNAL_NAME)?;
        let mut journal = runtime.create_journal(journal_file, &plan.job_id);
        journal.append(
            "jobStarted",
            &JobStarted {
                item_count: plan.items.len(),
                root_identity_revalidated: true,
            },
        )?;

        let mut results = Vec::with_capacity(plan.items.len());
        for (item_index, item) in plan.items.iter().enumerate() {
            if cancel.is_cancelled() {
                let error = RestoreError::Cancelled { bytes_written: 0 };
                record_item_failure(&mut journal, item.candidate_id(), &error)?;
                let outcomes = build_manifest_outcomes(
                    plan,
                    &results,
                    Some(TerminalItemOutcome {
                        index: item_index,
                        disposition: ManifestItemDisposition::Cancelled,
                        failure_kind: "cancelled",
                        published_sidecar: None,
                        published: None,
                    }),
                )?;
                publish_final_manifest(
                    &job_dir,
                    &job_directory_name,
                    plan,
                    &results,
                    &outcomes,
                    &mut journal,
                    runtime,
                    JobTerminal::Cancelled,
                )?;
                return Err(error);
            }
            match restore_item(
                ItemRestoreContext {
                    job_dir: &job_dir,
                    source,
                    cancel,
                    progress: &mut *progress,
                    journal: &mut journal,
                    runtime,
                },
                item,
                item_index,
            ) {
                Ok(result) => results.push(result),
                Err(failure) => {
                    let ItemRestoreFailure {
                        error,
                        published_sidecar,
                        published,
                    } = failure;
                    let error = *error;
                    if journal.is_poisoned() {
                        return Err(error);
                    }
                    let failure_kind = error_kind(&error);
                    let cancelled = matches!(error, RestoreError::Cancelled { .. });
                    let published = published.map(|result| *result);
                    if published.is_none() {
                        record_item_failure(&mut journal, item.candidate_id(), &error)?;
                    }
                    let outcomes = build_manifest_outcomes(
                        plan,
                        &results,
                        Some(TerminalItemOutcome {
                            index: item_index,
                            disposition: if published.is_some()
                                && matches!(&item.item, RestorePlanItem::Directory { .. })
                            {
                                ManifestItemDisposition::DirectoryNeedsReconciliation
                            } else if cancelled {
                                ManifestItemDisposition::Cancelled
                            } else {
                                ManifestItemDisposition::Failed
                            },
                            failure_kind,
                            published_sidecar: published_sidecar.map(|sidecar| *sidecar),
                            published,
                        }),
                    )?;
                    publish_final_manifest(
                        &job_dir,
                        &job_directory_name,
                        plan,
                        &results,
                        &outcomes,
                        &mut journal,
                        runtime,
                        if cancelled {
                            JobTerminal::Cancelled
                        } else {
                            JobTerminal::Failed {
                                error_kind: failure_kind,
                            }
                        },
                    )?;
                    return Err(error);
                }
            }
        }

        let outcomes = build_manifest_outcomes(plan, &results, None)?;
        publish_final_manifest(
            &job_dir,
            &job_directory_name,
            plan,
            &results,
            &outcomes,
            &mut journal,
            runtime,
            JobTerminal::Completed,
        )
    }

    fn revalidate(&self) -> Result<(), RestoreError> {
        let metadata = self
            .dir
            .dir_metadata()
            .map_err(|error| destination_io("revalidate retained root metadata", error))?;
        if !metadata.is_dir() || metadata_identity(&metadata) != self.baseline_identity {
            return Err(RestoreError::DestinationRootIdentityChanged);
        }
        Ok(())
    }
}

fn build_manifest_outcomes(
    plan: &RestoreJobPlan,
    published: &[RestoreItemResult],
    terminal_item: Option<TerminalItemOutcome>,
) -> Result<Vec<ManifestItemOutcome>, RestoreError> {
    if published.len() > plan.items.len() {
        return Err(RestoreError::ManifestSerialization {
            message: "published outcomes exceed immutable plan items".into(),
        });
    }
    let mut outcomes = Vec::with_capacity(plan.items.len());
    for (index, item) in plan.items.iter().enumerate() {
        let path_evidence = serde_json::from_str(&item.path_evidence_json).map_err(|error| {
            RestoreError::ManifestSerialization {
                message: error.to_string(),
            }
        })?;
        let (disposition, failure_kind, published_result, warnings, evidence) =
            if let Some(result) = published.get(index) {
                (
                    match &item.item {
                        RestorePlanItem::File(_) => ManifestItemDisposition::Published,
                        RestorePlanItem::Directory { .. }
                            if result.completion_status
                                == RestoreCompletionStatus::NeedsReconciliation =>
                        {
                            ManifestItemDisposition::DirectoryNeedsReconciliation
                        }
                        RestorePlanItem::Directory { .. } => {
                            ManifestItemDisposition::DirectoryCreated
                        }
                    },
                    (matches!(&item.item, RestorePlanItem::Directory { .. })
                        && result.completion_status
                            == RestoreCompletionStatus::NeedsReconciliation)
                        .then_some("needsReconciliation"),
                    Some(result.clone()),
                    result.warnings.clone(),
                    result.path_evidence.clone(),
                )
            } else if let Some(terminal) = terminal_item.as_ref() {
                if index == terminal.index {
                    let terminal_published = terminal.published.clone();
                    let warnings = terminal_published
                        .as_ref()
                        .map(|result| result.warnings.clone())
                        .unwrap_or_else(|| item.warnings.clone());
                    let evidence = terminal_published
                        .as_ref()
                        .map(|result| result.path_evidence.clone())
                        .unwrap_or(path_evidence);
                    (
                        terminal.disposition,
                        Some(terminal.failure_kind),
                        terminal_published,
                        warnings,
                        evidence,
                    )
                } else {
                    (
                        ManifestItemDisposition::NotAttempted,
                        None,
                        None,
                        item.warnings.clone(),
                        path_evidence,
                    )
                }
            } else {
                return Err(RestoreError::ManifestSerialization {
                    message: "successful outcomes do not match immutable plan items".into(),
                });
            };
        outcomes.push(ManifestItemOutcome {
            candidate_id: item.candidate_id(),
            item_key: restore_item_key(index, item.candidate_id()),
            item_kind: item.item_kind(),
            disposition,
            requested_path: item.path.to_slash_string(),
            expected_sha256: item.expected_sha256(),
            warnings,
            path_evidence: evidence,
            failure_kind,
            published_sidecar: if let Some(result) = published_result.as_ref() {
                match (&result.sidecar_name, result.sidecar_sha256) {
                    (Some(name), Some(sha256)) => Some(PublishedSidecarEvidence {
                        item_key: result.item_key.clone(),
                        name: name.clone(),
                        sha256,
                    }),
                    _ => None,
                }
            } else if terminal_item
                .as_ref()
                .is_some_and(|terminal| terminal.index == index)
            {
                terminal_item
                    .as_ref()
                    .and_then(|terminal| terminal.published_sidecar.clone())
            } else {
                None
            },
            published: published_result,
        });
    }
    Ok(outcomes)
}

#[allow(clippy::too_many_arguments)]
fn publish_final_manifest(
    job_dir: &Dir,
    job_directory_name: &str,
    plan: &RestoreJobPlan,
    results: &[RestoreItemResult],
    outcomes: &[ManifestItemOutcome],
    journal: &mut JobJournal,
    runtime: &dyn TransactionRuntime,
    terminal: JobTerminal,
) -> Result<RestoreSummary, RestoreError> {
    let outcomes_prefix_sha256 = journal.head_hash().to_owned();
    let (manifest_bytes, manifest_sha256) =
        build_manifest(&plan.job_id, &outcomes_prefix_sha256, outcomes).map_err(|error| {
            RestoreError::ManifestSerialization {
                message: error.to_string(),
            }
        })?;
    let manifest_temp = prepare_bytes(job_dir, "manifest", &manifest_bytes, manifest_sha256)?;
    journal.append(
        "manifestPrepared",
        &ManifestPrepared {
            name: MANIFEST_NAME,
            length: manifest_bytes.len(),
            sha256: hex_lower(&manifest_sha256),
            outcomes_prefix_sha256: outcomes_prefix_sha256.clone(),
            temporary_name: manifest_temp.name.clone(),
        },
    )?;

    verify_prelink_identity(job_dir, &manifest_temp)?;
    if let Err(error) = runtime.hard_link(
        job_dir,
        &manifest_temp.name,
        MANIFEST_NAME,
        LinkBoundary::Manifest,
    ) {
        let error = hard_link_error(error);
        if let Err(journal_error) = record_job_failure(journal, &error) {
            return Err(publication_reconciliation_error(
                "manifest failure record",
                MANIFEST_NAME,
                journal_error,
            ));
        }
        return Err(error);
    }
    let manifest_sync = runtime.sync_parent(job_dir, NamespaceBoundary::Manifest);
    let manifest_status = durability_status(&manifest_sync);
    if let Err(error) = journal.append(
        "manifestPublished",
        &ManifestPublished {
            name: MANIFEST_NAME,
            namespace_durability: &manifest_sync,
            completion_status: manifest_status,
        },
    ) {
        return Err(publication_reconciliation_error(
            "manifest",
            MANIFEST_NAME,
            error,
        ));
    }

    let overall_status = if manifest_status == RestoreCompletionStatus::NeedsReconciliation
        || results
            .iter()
            .any(|item| item.completion_status == RestoreCompletionStatus::NeedsReconciliation)
    {
        RestoreCompletionStatus::NeedsReconciliation
    } else {
        RestoreCompletionStatus::CompletedDurable
    };
    let terminal_result = match terminal {
        JobTerminal::Completed => journal.append(
            "jobCompleted",
            &JobCompleted {
                item_count: outcomes.len(),
                completion_status: overall_status,
            },
        ),
        JobTerminal::Failed { error_kind } => journal.append(
            "jobFailed",
            &FailureRecord {
                candidate_id: None,
                error_kind,
            },
        ),
        JobTerminal::Cancelled => journal.append(
            "jobCancelled",
            &FailureRecord {
                candidate_id: None,
                error_kind: "cancelled",
            },
        ),
    };
    if let Err(error) = terminal_result {
        return Err(publication_reconciliation_error(
            "job terminal record",
            MANIFEST_NAME,
            error,
        ));
    }

    Ok(RestoreSummary {
        job_directory_name: job_directory_name.to_owned(),
        manifest_name: MANIFEST_NAME.to_owned(),
        journal_name: JOURNAL_NAME.to_owned(),
        manifest_sha256,
        items: results.to_vec(),
        completion_status: overall_status,
    })
}

pub fn job_directory_component(job_id: &str) -> String {
    let digest = Sha256::digest(job_id.as_bytes());
    format!(".um-recovery-{}", &hex_lower(&digest)[..32])
}

fn restore_item_key(item_index: usize, candidate_id: u64) -> String {
    format!("{item_index:06}-{candidate_id:016x}")
}

fn partial_sidecar_base_name(item_key: &str) -> String {
    format!(".um-partial-{item_key}.json")
}

fn restore_item(
    context: ItemRestoreContext<'_>,
    plan: &FileRestorePlan,
    item_index: usize,
) -> Result<RestoreItemResult, ItemRestoreFailure> {
    let item_key = restore_item_key(item_index, plan.candidate_id());
    match &plan.item {
        RestorePlanItem::File(content) => restore_file_item(
            context.job_dir,
            context.source,
            plan,
            content,
            &item_key,
            context.cancel,
            context.progress,
            context.journal,
            context.runtime,
        ),
        RestorePlanItem::Directory { candidate_id } => restore_directory_item(
            context.job_dir,
            plan,
            *candidate_id,
            &item_key,
            context.journal,
            context.runtime,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn restore_file_item(
    job_dir: &Dir,
    source: &dyn SourceReader,
    plan: &FileRestorePlan,
    content: &ContentPlan,
    item_key: &str,
    cancel: &dyn CancellationProbe,
    progress: &mut dyn ProgressSink,
    journal: &mut JobJournal,
    runtime: &dyn TransactionRuntime,
) -> Result<RestoreItemResult, ItemRestoreFailure> {
    let parent = open_or_create_parents(job_dir, &plan.path, runtime)?;
    let temporary = prepare_stream(&parent, source, content, cancel, progress)?;
    let sidecar_bytes = build_partial_sidecar(
        item_key,
        content.candidate_id,
        content.logical_size,
        &temporary.outcome,
        content.expected_sha256,
        &plan.warnings,
    )
    .map_err(|error| RestoreError::ManifestSerialization {
        message: error.to_string(),
    })?;
    let sidecar_temp = match sidecar_bytes {
        Some(bytes) => {
            let hash = Sha256::digest(&bytes).into();
            Some(prepare_bytes(&parent, "partial", &bytes, hash)?)
        }
        None => None,
    };

    journal.append(
        "itemPrepared",
        &ItemPrepared {
            candidate_id: content.candidate_id,
            item_key: item_key.to_owned(),
            requested_path: plan.path.to_slash_string(),
            temporary_name: temporary.name.clone(),
            output_length: temporary.outcome.bytes_written,
            output_sha256: hex_lower(&temporary.outcome.sha256),
            expected_sha256: content.expected_sha256.map(|hash| hex_lower(&hash)),
            sidecar_temporary_name: sidecar_temp.as_ref().map(|file| file.name.clone()),
        },
    )?;

    let (published_sidecar, sidecar_durability) = match sidecar_temp.as_ref() {
        Some(sidecar) => {
            let base_name = partial_sidecar_base_name(item_key);
            let mut published = None;
            let mut durability = NamespaceDurability::Synced;
            for collision_index in 0..MAX_COLLISION_ATTEMPTS {
                let sidecar_name = collision_name(&base_name, collision_index);
                verify_prelink_identity(&parent, sidecar)?;
                match runtime.hard_link(
                    &parent,
                    &sidecar.name,
                    &sidecar_name,
                    LinkBoundary::PartialSidecar,
                ) {
                    Ok(()) => {
                        durability = merge_durability(
                            durability,
                            runtime.sync_parent(&parent, NamespaceBoundary::Sidecar),
                        );
                        let evidence = PublishedSidecarEvidence {
                            item_key: item_key.to_owned(),
                            name: sidecar_name,
                            sha256: sidecar.outcome.sha256,
                        };
                        if let Err(error) = journal.append(
                            "itemSidecarPublished",
                            &ItemSidecarPublished {
                                candidate_id: content.candidate_id,
                                item_key,
                                name: &evidence.name,
                                sha256: hex_lower(&evidence.sha256),
                                namespace_durability: &durability,
                            },
                        ) {
                            return Err(ItemRestoreFailure::with_sidecar(
                                publication_reconciliation_error("sidecar", &evidence.name, error),
                                Some(evidence),
                            ));
                        }
                        published = Some(evidence);
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(hard_link_error(error).into()),
                }
            }
            let published = published.ok_or_else(|| {
                ItemRestoreFailure::from(RestoreError::CollisionLimitExceeded {
                    maximum: MAX_COLLISION_ATTEMPTS,
                })
            })?;
            (Some(published), durability)
        }
        None => (None, NamespaceDurability::Synced),
    };

    let base_name = plan.path.file_name();
    for collision_index in 0..MAX_COLLISION_ATTEMPTS {
        let final_name = collision_name(base_name, collision_index);
        let mut durability = sidecar_durability.clone();

        verify_prelink_identity(&parent, &temporary)
            .map_err(|error| ItemRestoreFailure::with_sidecar(error, published_sidecar.clone()))?;
        match runtime.hard_link(&parent, &temporary.name, &final_name, LinkBoundary::Data) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(ItemRestoreFailure::with_sidecar(
                    hard_link_error(error),
                    published_sidecar.clone(),
                ));
            }
        }

        durability = merge_durability(
            durability,
            runtime.sync_parent(&parent, NamespaceBoundary::Data),
        );
        let completion_status = durability_status(&durability);
        let published_path = join_published_path(&plan.path, &final_name);
        if let Err(error) = journal.append(
            "itemPublished",
            &ItemPublished {
                candidate_id: content.candidate_id,
                item_key,
                item_kind: "file",
                published_path: published_path.clone(),
                sidecar_name: published_sidecar
                    .as_ref()
                    .map(|sidecar| sidecar.name.clone()),
                no_follow_bind_completed: None,
                directory_validation: None,
                namespace_durability: &durability,
                completion_status,
            },
        ) {
            return Err(ItemRestoreFailure::with_sidecar(
                publication_reconciliation_error("item", &published_path, error),
                published_sidecar.clone(),
            ));
        }

        let warnings = plan.warnings.clone();
        let temporary_disposition = if durability.is_synced() {
            TemporaryFileDisposition::RetainedBySafeCleanupPolicy
        } else {
            TemporaryFileDisposition::RetainedForReconciliation
        };

        let path_evidence = serde_json::from_str(&plan.path_evidence_json).map_err(|error| {
            RestoreError::ManifestSerialization {
                message: error.to_string(),
            }
        })?;
        return Ok(RestoreItemResult {
            candidate_id: content.candidate_id,
            item_key: item_key.to_owned(),
            requested_path: plan.path.to_slash_string(),
            published_path,
            published_name: final_name,
            output_len: Some(temporary.outcome.bytes_written),
            output_sha256: Some(temporary.outcome.sha256),
            expected_sha256: content.expected_sha256,
            zero_filled_ranges: temporary.outcome.zero_filled_ranges.clone(),
            sidecar_name: published_sidecar
                .as_ref()
                .map(|sidecar| sidecar.name.clone()),
            sidecar_sha256: published_sidecar.as_ref().map(|sidecar| sidecar.sha256),
            temporary_disposition: Some(temporary_disposition),
            path_evidence,
            warnings,
            namespace_durability: durability,
            completion_status,
            directory_no_follow_bind_completed: None,
            directory_validation: None,
        });
    }

    Err(ItemRestoreFailure::with_sidecar(
        RestoreError::CollisionLimitExceeded {
            maximum: MAX_COLLISION_ATTEMPTS,
        },
        published_sidecar,
    ))
}

fn restore_directory_item(
    job_dir: &Dir,
    plan: &FileRestorePlan,
    candidate_id: u64,
    item_key: &str,
    journal: &mut JobJournal,
    runtime: &dyn TransactionRuntime,
) -> Result<RestoreItemResult, ItemRestoreFailure> {
    let parent = open_or_create_parents(job_dir, &plan.path, runtime)?;
    let path_evidence: serde_json::Value =
        serde_json::from_str(&plan.path_evidence_json).map_err(|error| {
            RestoreError::ManifestSerialization {
                message: error.to_string(),
            }
        })?;
    journal.append(
        "itemPrepared",
        &DirectoryPrepared {
            candidate_id,
            item_key,
            requested_path: plan.path.to_slash_string(),
            item_kind: "directory",
        },
    )?;

    let base_name = plan.path.file_name();
    for collision_index in 0..MAX_COLLISION_ATTEMPTS {
        let final_name = collision_name(base_name, collision_index);
        let published_path = join_published_path(&plan.path, &final_name);
        journal.append(
            "directoryPublicationPlanned",
            &DirectoryPublicationPlanned {
                candidate_id,
                item_key,
                item_kind: "directory",
                published_path: &published_path,
                published_name: &final_name,
                collision_index,
            },
        )?;
        match parent.create_dir(&final_name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(destination_io("create selected destination directory", error).into());
            }
        }
        let final_component_index = plan.path.components().len() - 1;
        if let Err(error) = runtime.parent_transition(
            final_component_index,
            &final_name,
            ParentTransition::CreatedBeforeBind,
        ) {
            return Err(directory_reconciliation_failure(
                journal,
                plan,
                path_evidence.clone(),
                DirectoryPublicationState::unconfirmed(
                    candidate_id,
                    item_key,
                    published_path,
                    final_name,
                    false,
                    "no-follow bind was not attempted after the post-create transition failed",
                ),
                "createdBeforeBind",
                error,
            ));
        }
        let bound = match parent.open_dir_nofollow(&final_name) {
            Ok(bound) => bound,
            Err(error) => {
                return Err(directory_reconciliation_failure(
                    journal,
                    plan,
                    path_evidence.clone(),
                    DirectoryPublicationState::unconfirmed(
                        candidate_id,
                        item_key,
                        published_path,
                        final_name,
                        false,
                        "the visible directory name could not be bound with no-follow semantics",
                    ),
                    "noFollowBind",
                    destination_io("bind selected destination directory", error),
                ));
            }
        };
        if let Err(error) =
            runtime.parent_transition(final_component_index, &final_name, ParentTransition::Bound)
        {
            let failure = directory_reconciliation_failure(
                journal,
                plan,
                path_evidence.clone(),
                DirectoryPublicationState::unconfirmed(
                    candidate_id,
                    item_key,
                    published_path,
                    final_name,
                    true,
                    "the no-follow handle was bound but post-bind validation did not complete",
                ),
                "boundBeforeValidation",
                error,
            );
            drop(bound);
            return Err(failure);
        }
        if let Err(error) = verify_directory_binding(&parent, &final_name, &bound) {
            let failure = directory_reconciliation_failure(
                journal,
                plan,
                path_evidence.clone(),
                DirectoryPublicationState::unconfirmed(
                    candidate_id,
                    item_key,
                    published_path,
                    final_name,
                    true,
                    "the no-follow handle and visible directory name did not retain one identity",
                ),
                "noFollowIdentityValidation",
                error,
            );
            drop(bound);
            return Err(failure);
        }

        let durability = runtime.sync_parent(&parent, NamespaceBoundary::Data);
        let completion_status = durability_status(&durability);
        let state = DirectoryPublicationState {
            candidate_id,
            item_key: item_key.to_owned(),
            published_path,
            published_name: final_name,
            no_follow_bind_completed: true,
            validation: DirectoryValidation::BoundNoFollow,
            namespace_durability: durability,
        };
        let result = state.clone().into_result(plan, path_evidence.clone());
        if let Err(error) = journal.append(
            "itemPublished",
            &ItemPublished {
                candidate_id,
                item_key,
                item_kind: "directory",
                published_path: state.published_path.clone(),
                sidecar_name: None,
                no_follow_bind_completed: Some(true),
                directory_validation: Some(DirectoryValidation::BoundNoFollow),
                namespace_durability: &state.namespace_durability,
                completion_status,
            },
        ) {
            let failure = ItemRestoreFailure::with_publication(
                publication_reconciliation_error("directory item", &state.published_path, error),
                result,
            );
            drop(bound);
            return Err(failure);
        }
        drop(bound);
        return Ok(result);
    }

    Err(RestoreError::CollisionLimitExceeded {
        maximum: MAX_COLLISION_ATTEMPTS,
    }
    .into())
}

#[derive(Clone)]
struct DirectoryPublicationState {
    candidate_id: u64,
    item_key: String,
    published_path: String,
    published_name: String,
    no_follow_bind_completed: bool,
    validation: DirectoryValidation,
    namespace_durability: NamespaceDurability,
}

impl DirectoryPublicationState {
    fn unconfirmed(
        candidate_id: u64,
        item_key: &str,
        published_path: String,
        published_name: String,
        no_follow_bind_completed: bool,
        reason: &str,
    ) -> Self {
        Self {
            candidate_id,
            item_key: item_key.to_owned(),
            published_path,
            published_name,
            no_follow_bind_completed,
            validation: DirectoryValidation::Unconfirmed,
            namespace_durability: NamespaceDurability::Unconfirmed {
                reason: reason.to_owned(),
            },
        }
    }

    fn into_result(
        self,
        plan: &FileRestorePlan,
        path_evidence: serde_json::Value,
    ) -> RestoreItemResult {
        let completion_status = durability_status(&self.namespace_durability);
        RestoreItemResult {
            candidate_id: self.candidate_id,
            item_key: self.item_key,
            requested_path: plan.path.to_slash_string(),
            published_path: self.published_path,
            published_name: self.published_name,
            output_len: None,
            output_sha256: None,
            expected_sha256: None,
            zero_filled_ranges: Vec::new(),
            sidecar_name: None,
            sidecar_sha256: None,
            temporary_disposition: None,
            path_evidence,
            warnings: plan.warnings.clone(),
            namespace_durability: self.namespace_durability,
            completion_status,
            directory_no_follow_bind_completed: Some(self.no_follow_bind_completed),
            directory_validation: Some(self.validation),
        }
    }
}

fn directory_reconciliation_failure(
    journal: &mut JobJournal,
    plan: &FileRestorePlan,
    path_evidence: serde_json::Value,
    state: DirectoryPublicationState,
    stage: &'static str,
    error: RestoreError,
) -> ItemRestoreFailure {
    let reconciliation_error = RestoreError::NeedsReconciliation {
        message: format!(
            "directory {:?} is visible after {stage} failed and requires reconciliation: {error}",
            state.published_path
        ),
    };
    let result = state.clone().into_result(plan, path_evidence);
    if let Err(journal_error) = journal.append(
        "directoryReconciliationRequired",
        &DirectoryReconciliationRequired {
            candidate_id: state.candidate_id,
            item_key: &state.item_key,
            item_kind: "directory",
            published_path: &state.published_path,
            published_name: &state.published_name,
            stage,
            no_follow_bind_completed: state.no_follow_bind_completed,
            directory_validation: state.validation,
            namespace_durability: &state.namespace_durability,
            disposition: "directoryNeedsReconciliation",
            completion_status: RestoreCompletionStatus::NeedsReconciliation,
        },
    ) {
        return ItemRestoreFailure::with_publication(
            publication_reconciliation_error(
                "directory reconciliation record",
                &state.published_path,
                journal_error,
            ),
            result,
        );
    }
    ItemRestoreFailure::with_publication(reconciliation_error, result)
}

fn verify_directory_binding(parent: &Dir, name: &str, bound: &Dir) -> Result<(), RestoreError> {
    let bound_metadata = bound
        .dir_metadata()
        .map_err(|error| destination_io("query bound directory identity", error))?;
    let name_metadata = parent
        .symlink_metadata(name)
        .map_err(|error| destination_io("query visible directory identity", error))?;
    if !bound_metadata.is_dir()
        || !name_metadata.is_dir()
        || metadata_identity(&bound_metadata) != metadata_identity(&name_metadata)
    {
        return Err(RestoreError::DestinationIo {
            operation: "validate selected destination directory identity",
            kind: std::io::ErrorKind::Other,
            message: "bound capability and visible name do not identify one directory".into(),
        });
    }
    Ok(())
}

struct PreparedFile {
    name: String,
    file: File,
    identity: FileIdentity,
    outcome: crate::StreamOutcome,
}

fn prepare_stream(
    parent: &Dir,
    source: &dyn SourceReader,
    content: &ContentPlan,
    cancel: &dyn CancellationProbe,
    progress: &mut dyn ProgressSink,
) -> Result<PreparedFile, RestoreError> {
    let (name, mut file) = create_unique_temp(parent, content.candidate_id, "data")?;
    let metadata = file
        .metadata()
        .map_err(|error| destination_io("query temporary file", error))?;
    let identity = metadata_identity(&metadata);
    let mut scratch = vec![0u8; RESTORE_SCRATCH_BYTES];
    let outcome = stream_candidate(source, content, &mut file, &mut scratch, cancel, progress)?;
    file.flush()
        .map_err(|error| destination_io("flush temporary file", error))?;
    file.sync_all()
        .map_err(|error| destination_io("sync temporary file", error))?;
    let actual_len = file
        .metadata()
        .map_err(|error| destination_io("verify temporary file", error))?
        .len();
    if actual_len != outcome.bytes_written {
        return Err(RestoreError::TemporaryLengthMismatch {
            expected: outcome.bytes_written,
            actual: actual_len,
        });
    }
    Ok(PreparedFile {
        name,
        file,
        identity,
        outcome,
    })
}

fn prepare_bytes(
    parent: &Dir,
    label: &str,
    bytes: &[u8],
    sha256: [u8; 32],
) -> Result<PreparedFile, RestoreError> {
    let candidate_id = u64::from_le_bytes(sha256[..8].try_into().expect("fixed digest"));
    let (name, mut file) = create_unique_temp(parent, candidate_id, label)?;
    let metadata = file
        .metadata()
        .map_err(|error| destination_io("query transactional side file", error))?;
    let identity = metadata_identity(&metadata);
    file.write_all(bytes)
        .map_err(|error| destination_io("write transactional side file", error))?;
    file.flush()
        .map_err(|error| destination_io("flush transactional side file", error))?;
    file.sync_all()
        .map_err(|error| destination_io("sync transactional side file", error))?;
    let actual_len = file
        .metadata()
        .map_err(|error| destination_io("verify transactional side file", error))?
        .len();
    let expected_len =
        u64::try_from(bytes.len()).map_err(|_| RestoreError::TemporaryLengthMismatch {
            expected: u64::MAX,
            actual: actual_len,
        })?;
    if actual_len != expected_len {
        return Err(RestoreError::TemporaryLengthMismatch {
            expected: expected_len,
            actual: actual_len,
        });
    }
    Ok(PreparedFile {
        name,
        file,
        identity,
        outcome: crate::StreamOutcome {
            bytes_written: expected_len,
            sha256,
            zero_filled_ranges: Vec::new(),
            requires_best_effort: false,
        },
    })
}

fn create_unique_temp(
    parent: &Dir,
    candidate_id: u64,
    label: &str,
) -> Result<(String, File), RestoreError> {
    for attempt in 0..MAX_TEMP_CREATE_ATTEMPTS {
        let name = format!(".um-{candidate_id:016x}-{label}-{attempt:04}.umrecovering");
        match create_new_protected(parent, &name) {
            Ok(file) => return Ok((name, file)),
            Err(RestoreError::DestinationIo {
                kind: std::io::ErrorKind::AlreadyExists,
                ..
            }) => {
                continue;
            }
            Err(error) => return Err(error),
        }
    }
    Err(RestoreError::CollisionLimitExceeded {
        maximum: MAX_TEMP_CREATE_ATTEMPTS,
    })
}

fn create_new_protected(parent: &Dir, name: &str) -> Result<File, RestoreError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    options.follow(FollowSymlinks::No);
    options.maybe_dir(true);
    let file = parent
        .open_with(name, &options)
        .map_err(|error| destination_io("create no-follow file", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| destination_io("query created no-follow file", error))?;
    if !metadata.is_file() {
        return Err(RestoreError::TemporaryNotRegularFile);
    }
    Ok(file)
}

fn verify_prelink_identity(parent: &Dir, prepared: &PreparedFile) -> Result<(), RestoreError> {
    let handle_metadata = prepared
        .file
        .metadata()
        .map_err(|error| destination_io("query retained temporary handle", error))?;
    let handle_identity = metadata_identity(&handle_metadata);
    if !handle_metadata.is_file()
        || handle_identity != prepared.identity
        || handle_metadata.len() != prepared.outcome.bytes_written
    {
        return Err(RestoreError::TemporaryIdentityChanged);
    }

    let mut options = OpenOptions::new();
    options.read(true);
    options.follow(FollowSymlinks::No);
    options.maybe_dir(true);
    let lookup = parent
        .open_with(&prepared.name, &options)
        .map_err(|error| destination_io("rebind temporary name", error))?;
    let lookup_metadata = lookup
        .metadata()
        .map_err(|error| destination_io("query rebound temporary name", error))?;
    if !lookup_metadata.is_file() || metadata_identity(&lookup_metadata) != prepared.identity {
        return Err(RestoreError::TemporaryIdentityChanged);
    }
    Ok(())
}

fn open_or_create_parents(
    job_dir: &Dir,
    path: &SafeRelativePath,
    runtime: &dyn TransactionRuntime,
) -> Result<Dir, RestoreError> {
    let mut current = job_dir
        .try_clone()
        .map_err(|error| destination_io("clone job directory capability", error))?;
    for (component_index, component) in path.parent_components().enumerate() {
        match current.open_dir_nofollow(component) {
            Ok(next) => {
                current = next;
                runtime.parent_transition(component_index, component, ParentTransition::Bound)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match current.create_dir(component) {
                    Ok(()) => {
                        runtime.parent_transition(
                            component_index,
                            component,
                            ParentTransition::CreatedBeforeBind,
                        )?;
                    }
                    Err(create_error)
                        if create_error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(create_error) => {
                        return Err(destination_io(
                            "create sanitized destination directory",
                            create_error,
                        ));
                    }
                }
                current = current.open_dir_nofollow(component).map_err(|open_error| {
                    destination_io("bind sanitized destination directory", open_error)
                })?;
                runtime.parent_transition(component_index, component, ParentTransition::Bound)?;
            }
            Err(error) => {
                return Err(destination_io(
                    "open sanitized destination directory without following",
                    error,
                ));
            }
        }
    }
    Ok(current)
}

fn collision_name(base: &str, collision_index: usize) -> String {
    if collision_index == 0 {
        return base.to_owned();
    }
    let suffix = format!(" (recovered {collision_index})");
    let candidate = match base.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => {
            format!("{stem}{suffix}.{extension}")
        }
        _ => format!("{base}{suffix}"),
    };
    if SafeRelativePath::try_from_components([candidate.as_str()]).is_ok() {
        candidate
    } else {
        let digest = Sha256::digest(candidate.as_bytes());
        format!("recovered-{}", &hex_lower(&digest)[..40])
    }
}

fn join_published_path(path: &SafeRelativePath, final_name: &str) -> String {
    let mut components = path.parent_components().collect::<Vec<_>>();
    components.push(final_name);
    components.join("/")
}

fn metadata_identity(metadata: &cap_std::fs::Metadata) -> FileIdentity {
    FileIdentity {
        device: MetadataExt::dev(metadata),
        inode: MetadataExt::ino(metadata),
    }
}

fn sync_parent(parent: &Dir) -> NamespaceDurability {
    let clone = match parent.try_clone() {
        Ok(clone) => clone,
        Err(error) => {
            return NamespaceDurability::Failed {
                kind: format!("{:?}", error.kind()),
                message: error.to_string(),
            };
        }
    };
    match clone.into_std_file().sync_all() {
        Ok(()) => NamespaceDurability::Synced,
        Err(error) if namespace_sync_is_unsupported(&error) => NamespaceDurability::Unsupported {
            kind: format!("{:?}", error.kind()),
            message: error.to_string(),
        },
        Err(error) => NamespaceDurability::Failed {
            kind: format!("{:?}", error.kind()),
            message: error.to_string(),
        },
    }
}

#[cfg(windows)]
fn namespace_sync_is_unsupported(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(5 | 50 | 120))
}

#[cfg(not(windows))]
fn namespace_sync_is_unsupported(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::Unsupported | std::io::ErrorKind::InvalidInput
    )
}

fn merge_durability(
    first: NamespaceDurability,
    second: NamespaceDurability,
) -> NamespaceDurability {
    match (&first, &second) {
        (NamespaceDurability::Failed { .. }, _) => first,
        (_, NamespaceDurability::Failed { .. }) => second,
        (NamespaceDurability::Unconfirmed { .. }, _) => first,
        (_, NamespaceDurability::Unconfirmed { .. }) => second,
        (NamespaceDurability::Unsupported { .. }, _) => first,
        (_, NamespaceDurability::Unsupported { .. }) => second,
        _ => NamespaceDurability::Synced,
    }
}

fn durability_status(durability: &NamespaceDurability) -> RestoreCompletionStatus {
    if durability.is_synced() {
        RestoreCompletionStatus::CompletedDurable
    } else {
        RestoreCompletionStatus::NeedsReconciliation
    }
}

fn record_item_failure(
    journal: &mut JobJournal,
    candidate_id: u64,
    error: &RestoreError,
) -> Result<(), RestoreError> {
    let cancelled = matches!(error, RestoreError::Cancelled { .. });
    journal.append(
        if cancelled {
            "itemCancelled"
        } else {
            "itemFailed"
        },
        &FailureRecord {
            candidate_id: Some(candidate_id),
            error_kind: error_kind(error),
        },
    )
}

fn record_job_failure(journal: &mut JobJournal, error: &RestoreError) -> Result<(), RestoreError> {
    journal.append(
        "jobFailed",
        &FailureRecord {
            candidate_id: None,
            error_kind: error_kind(error),
        },
    )
}

fn error_kind(error: &RestoreError) -> &'static str {
    match error {
        RestoreError::Cancelled { .. } => "cancelled",
        RestoreError::HashMismatch { .. } => "hashMismatch",
        RestoreError::CollisionLimitExceeded { .. } => "collisionLimit",
        RestoreError::HardLinkPublication { .. } => "hardLinkPublication",
        RestoreError::TemporaryIdentityChanged => "temporaryIdentityChanged",
        RestoreError::NeedsReconciliation { .. } => "needsReconciliation",
        RestoreError::JournalIo { .. } | RestoreError::JournalPoisoned => "journalDurability",
        _ => "restoreFailure",
    }
}

fn destination_io(operation: &'static str, error: std::io::Error) -> RestoreError {
    RestoreError::DestinationIo {
        operation,
        kind: error.kind(),
        message: error.to_string(),
    }
}

fn hard_link_error(error: std::io::Error) -> RestoreError {
    RestoreError::HardLinkPublication {
        kind: error.kind(),
        message: error.to_string(),
    }
}

fn publication_reconciliation_error(
    boundary: &str,
    published_name: &str,
    error: RestoreError,
) -> RestoreError {
    RestoreError::NeedsReconciliation {
        message: format!(
            "{boundary} {published_name:?} was linked before its durable journal boundary failed: {error}"
        ),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobStarted {
    item_count: usize,
    root_identity_revalidated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemPrepared {
    candidate_id: u64,
    item_key: String,
    requested_path: String,
    temporary_name: String,
    output_length: u64,
    output_sha256: String,
    expected_sha256: Option<String>,
    sidecar_temporary_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectoryPrepared<'a> {
    candidate_id: u64,
    item_key: &'a str,
    requested_path: String,
    item_kind: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectoryPublicationPlanned<'a> {
    candidate_id: u64,
    item_key: &'a str,
    item_kind: &'static str,
    published_path: &'a str,
    published_name: &'a str,
    collision_index: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectoryReconciliationRequired<'a> {
    candidate_id: u64,
    item_key: &'a str,
    item_kind: &'static str,
    published_path: &'a str,
    published_name: &'a str,
    stage: &'static str,
    no_follow_bind_completed: bool,
    directory_validation: DirectoryValidation,
    namespace_durability: &'a NamespaceDurability,
    disposition: &'static str,
    completion_status: RestoreCompletionStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemSidecarPublished<'a> {
    candidate_id: u64,
    item_key: &'a str,
    name: &'a str,
    sha256: String,
    namespace_durability: &'a NamespaceDurability,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemPublished<'a> {
    candidate_id: u64,
    item_key: &'a str,
    item_kind: &'static str,
    published_path: String,
    sidecar_name: Option<String>,
    no_follow_bind_completed: Option<bool>,
    directory_validation: Option<DirectoryValidation>,
    namespace_durability: &'a NamespaceDurability,
    completion_status: RestoreCompletionStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestPrepared {
    name: &'static str,
    length: usize,
    sha256: String,
    outcomes_prefix_sha256: String,
    temporary_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestPublished<'a> {
    name: &'static str,
    namespace_durability: &'a NamespaceDurability,
    completion_status: RestoreCompletionStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JobCompleted {
    item_count: usize,
    completion_status: RestoreCompletionStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureRecord {
    candidate_id: Option<u64>,
    error_kind: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::ambient_authority;
    use std::io;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use um_core::{
        Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
        MetadataConfidence, ReadError, ReadOutcome, SectorLayout, SourceIdentity, SourceKind,
        Timestamps,
    };

    struct MemoryReader {
        bytes: Vec<u8>,
        identity: SourceIdentity,
    }

    impl MemoryReader {
        fn new(bytes: &[u8]) -> Self {
            Self {
                bytes: bytes.to_vec(),
                identity: SourceIdentity {
                    id: "transaction-runtime-fixture".into(),
                    kind: SourceKind::ImageFile,
                    label: "synthetic".into(),
                    size: bytes.len() as u64,
                },
            }
        }
    }

    impl SourceReader for MemoryReader {
        fn identity(&self) -> &SourceIdentity {
            &self.identity
        }

        fn len(&self) -> u64 {
            self.bytes.len() as u64
        }

        fn sector_layout(&self) -> SectorLayout {
            SectorLayout::DEFAULT_512
        }

        fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
            let start = usize::try_from(offset).map_err(|_| ReadError::OutOfBounds {
                offset,
                len: buffer.len() as u64,
                source_len: self.len(),
            })?;
            let end = start
                .checked_add(buffer.len())
                .filter(|end| *end <= self.bytes.len())
                .ok_or(ReadError::OutOfBounds {
                    offset,
                    len: buffer.len() as u64,
                    source_len: self.len(),
                })?;
            buffer.copy_from_slice(&self.bytes[start..end]);
            Ok(())
        }

        fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
            match self.read_exact_at(offset, buffer) {
                Ok(()) => ReadOutcome::complete(buffer.len() as u64),
                Err(_) => ReadOutcome {
                    bytes_valid: 0,
                    bad_ranges: vec![(0, buffer.len() as u64)],
                },
            }
        }
    }

    struct NeverCancel;

    impl CancellationProbe for NeverCancel {
        fn is_cancelled(&self) -> bool {
            false
        }
    }

    struct NoProgress;

    impl ProgressSink for NoProgress {
        fn advanced(&mut self, _progress: crate::StreamProgress) {}
    }

    struct SharedCancel(Arc<AtomicBool>);

    impl CancellationProbe for SharedCancel {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    struct CancelOnProgress(Arc<AtomicBool>);

    impl ProgressSink for CancelOnProgress {
        fn advanced(&mut self, _progress: crate::StreamProgress) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    struct PatternReader {
        len: u64,
        max_read: AtomicUsize,
        identity: SourceIdentity,
    }

    impl PatternReader {
        fn new(len: u64) -> Self {
            Self {
                len,
                max_read: AtomicUsize::new(0),
                identity: SourceIdentity {
                    id: "bounded-pattern-source".into(),
                    kind: SourceKind::ImageFile,
                    label: "synthetic".into(),
                    size: len,
                },
            }
        }
    }

    impl SourceReader for PatternReader {
        fn identity(&self) -> &SourceIdentity {
            &self.identity
        }

        fn len(&self) -> u64 {
            self.len
        }

        fn sector_layout(&self) -> SectorLayout {
            SectorLayout::DEFAULT_512
        }

        fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
            let end = offset
                .checked_add(buffer.len() as u64)
                .filter(|end| *end <= self.len)
                .ok_or(ReadError::OutOfBounds {
                    offset,
                    len: buffer.len() as u64,
                    source_len: self.len,
                })?;
            self.max_read.fetch_max(buffer.len(), Ordering::SeqCst);
            for (index, byte) in buffer.iter_mut().enumerate() {
                *byte = (offset + index as u64) as u8;
            }
            debug_assert!(end <= self.len);
            Ok(())
        }

        fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
            match self.read_exact_at(offset, buffer) {
                Ok(()) => ReadOutcome::complete(buffer.len() as u64),
                Err(_) => ReadOutcome {
                    bytes_valid: 0,
                    bad_ranges: vec![(0, buffer.len() as u64)],
                },
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum JournalFaultStage {
        Write,
        Flush,
        Sync,
    }

    struct FaultingJournalSink {
        inner: File,
        completed_records: Mutex<usize>,
        fail_record: usize,
        fail_stage: JournalFaultStage,
    }

    impl FaultingJournalSink {
        fn should_fail(&self, stage: JournalFaultStage) -> bool {
            *self.completed_records.lock().unwrap() == self.fail_record && self.fail_stage == stage
        }
    }

    impl Write for FaultingJournalSink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.should_fail(JournalFaultStage::Write) {
                return Err(io::Error::other("injected journal write failure"));
            }
            self.inner.write(bytes)
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.should_fail(JournalFaultStage::Flush) {
                return Err(io::Error::other("injected journal flush failure"));
            }
            self.inner.flush()
        }
    }

    impl crate::journal::DurableJournalSink for FaultingJournalSink {
        fn sync_all(&self) -> io::Result<()> {
            let mut completed = self.completed_records.lock().unwrap();
            if *completed == self.fail_record && matches!(self.fail_stage, JournalFaultStage::Sync)
            {
                return Err(io::Error::other("injected journal sync failure"));
            }
            self.inner.sync_all()?;
            *completed += 1;
            Ok(())
        }
    }

    struct JournalFault {
        record: usize,
        stage: JournalFaultStage,
    }

    impl TransactionRuntime for JournalFault {
        fn create_journal(&self, file: File, job_id: &str) -> JobJournal {
            JobJournal::with_sink(
                Box::new(FaultingJournalSink {
                    inner: file,
                    completed_records: Mutex::new(0),
                    fail_record: self.record,
                    fail_stage: self.stage,
                }),
                Sha256::digest(job_id.as_bytes()).into(),
            )
        }
    }

    struct KindWriteFaultJournalSink {
        inner: File,
        kind: &'static str,
    }

    impl Write for KindWriteFaultJournalSink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let needle = format!("\"kind\":\"{}\"", self.kind);
            if bytes
                .windows(needle.len())
                .any(|window| window == needle.as_bytes())
            {
                return Err(io::Error::other("injected journal kind write failure"));
            }
            self.inner.write(bytes)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.inner.flush()
        }
    }

    impl crate::journal::DurableJournalSink for KindWriteFaultJournalSink {
        fn sync_all(&self) -> io::Result<()> {
            self.inner.sync_all()
        }
    }

    struct JournalKindWriteFault {
        kind: &'static str,
    }

    impl TransactionRuntime for JournalKindWriteFault {
        fn create_journal(&self, file: File, job_id: &str) -> JobJournal {
            JobJournal::with_sink(
                Box::new(KindWriteFaultJournalSink {
                    inner: file,
                    kind: self.kind,
                }),
                Sha256::digest(job_id.as_bytes()).into(),
            )
        }
    }

    struct NamespaceFault {
        boundary: NamespaceBoundary,
    }

    impl TransactionRuntime for NamespaceFault {
        fn sync_parent(&self, parent: &Dir, boundary: NamespaceBoundary) -> NamespaceDurability {
            if boundary == self.boundary {
                NamespaceDurability::Failed {
                    kind: "Other".into(),
                    message: "injected namespace sync failure".into(),
                }
            } else {
                sync_parent(parent)
            }
        }
    }

    struct UnsupportedHardLinks;

    impl TransactionRuntime for UnsupportedHardLinks {
        fn hard_link(
            &self,
            parent: &Dir,
            temporary_name: &str,
            final_name: &str,
            boundary: LinkBoundary,
        ) -> io::Result<()> {
            if boundary == LinkBoundary::Data {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "injected unsupported hard link",
                ))
            } else {
                parent.hard_link(temporary_name, parent, final_name)
            }
        }
    }

    struct UnsupportedDataAfterSidecar;

    impl TransactionRuntime for UnsupportedDataAfterSidecar {
        fn hard_link(
            &self,
            parent: &Dir,
            temporary_name: &str,
            final_name: &str,
            boundary: LinkBoundary,
        ) -> io::Result<()> {
            if boundary == LinkBoundary::Data {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "injected data publication failure after sidecar",
                ))
            } else {
                parent.hard_link(temporary_name, parent, final_name)
            }
        }
    }

    struct RacingCreator {
        collisions: usize,
        attempts: Mutex<usize>,
    }

    impl RacingCreator {
        fn new(collisions: usize) -> Self {
            Self {
                collisions,
                attempts: Mutex::new(0),
            }
        }

        fn attempts(&self) -> usize {
            *self.attempts.lock().unwrap()
        }
    }

    impl TransactionRuntime for RacingCreator {
        fn hard_link(
            &self,
            parent: &Dir,
            temporary_name: &str,
            final_name: &str,
            boundary: LinkBoundary,
        ) -> io::Result<()> {
            if boundary == LinkBoundary::Data {
                let mut attempts = self.attempts.lock().unwrap();
                if *attempts < self.collisions {
                    let mut racer = create_new_protected(parent, final_name)
                        .expect("each generated collision name is unique");
                    write!(racer, "racer-{}", *attempts).unwrap();
                }
                *attempts += 1;
            }
            parent.hard_link(temporary_name, parent, final_name)
        }
    }

    struct ReplaceCreatedComponent {
        job_dir: std::path::PathBuf,
        parents: Vec<String>,
        outside: std::path::PathBuf,
        target_index: usize,
        replacements: AtomicUsize,
    }

    impl ReplaceCreatedComponent {
        fn target_path(&self) -> std::path::PathBuf {
            self.parents[..=self.target_index]
                .iter()
                .fold(self.job_dir.clone(), |path, component| path.join(component))
        }
    }

    impl TransactionRuntime for ReplaceCreatedComponent {
        fn parent_transition(
            &self,
            component_index: usize,
            _component: &str,
            transition: ParentTransition,
        ) -> Result<(), RestoreError> {
            if component_index != self.target_index
                || transition != ParentTransition::CreatedBeforeBind
            {
                return Ok(());
            }
            let target = self.target_path();
            let held = target.with_extension(format!("held-{}", self.target_index));
            std::fs::rename(&target, &held)
                .map_err(|error| destination_io("inject transition replacement rename", error))?;
            create_test_directory_redirect(&self.outside, &target)
                .map_err(|error| destination_io("inject transition redirect", error))?;
            self.replacements.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FailFinalDirectoryTransitionAfterCollision {
        job_dir: std::path::PathBuf,
        parent_name: &'static str,
        requested_name: &'static str,
    }

    impl TransactionRuntime for FailFinalDirectoryTransitionAfterCollision {
        fn parent_transition(
            &self,
            component_index: usize,
            _component: &str,
            transition: ParentTransition,
        ) -> Result<(), RestoreError> {
            if component_index == 0 && transition == ParentTransition::Bound {
                std::fs::create_dir(
                    self.job_dir
                        .join(self.parent_name)
                        .join(self.requested_name),
                )
                .map_err(|error| destination_io("inject final directory collision", error))?;
            }
            if component_index == 1 && transition == ParentTransition::CreatedBeforeBind {
                return Err(RestoreError::DestinationIo {
                    operation: "injected final directory created-before-bind transition",
                    kind: io::ErrorKind::Other,
                    message: "injected post-create transition failure".into(),
                });
            }
            Ok(())
        }
    }

    #[cfg(windows)]
    fn create_test_directory_redirect(
        target: &std::path::Path,
        redirect: &std::path::Path,
    ) -> io::Result<()> {
        junction::create(target, redirect)
    }

    #[cfg(windows)]
    fn remove_test_directory_redirect(redirect: &std::path::Path) -> io::Result<()> {
        junction::delete(redirect)
    }

    #[cfg(unix)]
    fn create_test_directory_redirect(
        target: &std::path::Path,
        redirect: &std::path::Path,
    ) -> io::Result<()> {
        std::os::unix::fs::symlink(target, redirect)
    }

    #[cfg(unix)]
    fn remove_test_directory_redirect(redirect: &std::path::Path) -> io::Result<()> {
        std::fs::remove_file(redirect)
    }

    struct SidecarCollisionRecorder {
        sidecar_injected: AtomicBool,
        data_injected: AtomicBool,
        events: Mutex<Vec<LinkBoundary>>,
    }

    impl SidecarCollisionRecorder {
        fn new() -> Self {
            Self {
                sidecar_injected: AtomicBool::new(false),
                data_injected: AtomicBool::new(false),
                events: Mutex::new(Vec::new()),
            }
        }
    }

    impl TransactionRuntime for SidecarCollisionRecorder {
        fn hard_link(
            &self,
            parent: &Dir,
            temporary_name: &str,
            final_name: &str,
            boundary: LinkBoundary,
        ) -> io::Result<()> {
            self.events.lock().unwrap().push(boundary);
            if boundary == LinkBoundary::PartialSidecar
                && !self.sidecar_injected.swap(true, Ordering::SeqCst)
            {
                let mut existing = create_new_protected(parent, final_name)
                    .expect("the first sidecar name is initially free");
                existing.write_all(b"existing sidecar sentinel").unwrap();
                existing.sync_all().unwrap();
            }
            if boundary == LinkBoundary::Data && !self.data_injected.swap(true, Ordering::SeqCst) {
                let mut existing = create_new_protected(parent, final_name)
                    .expect("the first data name is initially free");
                existing.write_all(b"existing data sentinel").unwrap();
                existing.sync_all().unwrap();
            }
            parent.hard_link(temporary_name, parent, final_name)
        }
    }

    struct AlwaysSynced;

    impl TransactionRuntime for AlwaysSynced {
        fn sync_parent(&self, _parent: &Dir, _boundary: NamespaceBoundary) -> NamespaceDurability {
            NamespaceDurability::Synced
        }
    }

    #[cfg(windows)]
    struct SyncedDataCollisionOnce {
        injected: AtomicBool,
    }

    #[cfg(windows)]
    impl TransactionRuntime for SyncedDataCollisionOnce {
        fn sync_parent(&self, _parent: &Dir, _boundary: NamespaceBoundary) -> NamespaceDurability {
            NamespaceDurability::Synced
        }

        fn hard_link(
            &self,
            parent: &Dir,
            temporary_name: &str,
            final_name: &str,
            boundary: LinkBoundary,
        ) -> io::Result<()> {
            if boundary == LinkBoundary::Data && !self.injected.swap(true, Ordering::SeqCst) {
                let mut existing = create_new_protected(parent, final_name)
                    .expect("the first data collision name is initially free");
                existing.write_all(b"existing data sentinel").unwrap();
                existing.sync_all().unwrap();
            }
            parent.hard_link(temporary_name, parent, final_name)
        }
    }

    struct ManifestCollision;

    impl TransactionRuntime for ManifestCollision {
        fn hard_link(
            &self,
            parent: &Dir,
            temporary_name: &str,
            final_name: &str,
            boundary: LinkBoundary,
        ) -> io::Result<()> {
            if boundary == LinkBoundary::Manifest {
                let mut existing = create_new_protected(parent, final_name)
                    .expect("the manifest collision is injected exactly once");
                existing.write_all(b"existing manifest sentinel").unwrap();
                existing.sync_all().unwrap();
            }
            parent.hard_link(temporary_name, parent, final_name)
        }
    }

    fn one_item_job(job_id: &str, bytes: &[u8]) -> RestoreJobPlan {
        one_item_job_for_len(job_id, bytes.len() as u64)
    }

    fn one_item_job_for_len(job_id: &str, len: u64) -> RestoreJobPlan {
        let candidate = Candidate {
            id: 41,
            kind: CandidateKind::File,
            method: DiscoveryMethod::NtfsMetadata,
            state: CandidateState::CompleteUnvalidated,
            name: "fixture.bin".into(),
            name_certain: true,
            parent_path: Vec::new(),
            metadata_confidence: MetadataConfidence::High,
            size: len,
            timestamps: Timestamps::default(),
            extents: vec![ExtentRun {
                logical_offset: 0,
                physical_offset: Some(0),
                len,
                availability: ExtentAvailability::FreeInSnapshot,
            }],
            record_ref: 41,
            sequence: Some(1),
            warnings: Vec::new(),
        };
        let content = crate::plan_candidate(
            &candidate,
            len,
            None,
            crate::PartialPolicy::CompleteOnly,
            crate::PlanLimits::default(),
        )
        .unwrap()
        .unwrap();
        let path = SafeRelativePath::derive_for_recovery(&[] as &[&str], "published.bin").unwrap();
        RestoreJobPlan::new(job_id, vec![FileRestorePlan::file(path, content).unwrap()]).unwrap()
    }

    fn partial_item_job(job_id: &str) -> RestoreJobPlan {
        let candidate = Candidate {
            id: 52,
            kind: CandidateKind::File,
            method: DiscoveryMethod::NtfsMetadata,
            state: CandidateState::Partial,
            name: "partial.bin".into(),
            name_certain: true,
            parent_path: Vec::new(),
            metadata_confidence: MetadataConfidence::High,
            size: 8,
            timestamps: Timestamps::default(),
            extents: vec![ExtentRun {
                logical_offset: 0,
                physical_offset: Some(0),
                len: 4,
                availability: ExtentAvailability::FreeInSnapshot,
            }],
            record_ref: 52,
            sequence: Some(1),
            warnings: Vec::new(),
        };
        let content = crate::plan_candidate(
            &candidate,
            4,
            None,
            crate::PartialPolicy::ZeroFillAndMap,
            crate::PlanLimits::default(),
        )
        .unwrap()
        .unwrap();
        let path = SafeRelativePath::derive_for_recovery(&[] as &[&str], "partial.bin").unwrap();
        RestoreJobPlan::new(job_id, vec![FileRestorePlan::file(path, content).unwrap()]).unwrap()
    }

    fn directory_item_job(
        job_id: &str,
        candidate_id: u64,
        parents: &[&str],
        name: &str,
    ) -> RestoreJobPlan {
        let path = SafeRelativePath::derive_for_recovery(parents, name).unwrap();
        RestoreJobPlan::new(
            job_id,
            vec![FileRestorePlan::directory(candidate_id, path).unwrap()],
        )
        .unwrap()
    }

    fn read_manifest_value(job_dir: &std::path::Path) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(job_dir.join(MANIFEST_NAME)).unwrap()).unwrap()
    }

    fn read_journal_values(job_dir: &std::path::Path) -> Vec<serde_json::Value> {
        std::fs::read_to_string(job_dir.join(JOURNAL_NAME))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn restore_job_plan_rejects_more_than_one_million_sanitized_components() {
        let parents = (0..(crate::MAX_SAFE_PATH_COMPONENTS - 1))
            .map(|index| format!("p{index}"))
            .collect::<Vec<_>>();
        let path = SafeRelativePath::derive_for_recovery(&parents, "published.bin").unwrap();
        let content = match &one_item_job("component-template", b"x").items[0].item {
            RestorePlanItem::File(content) => content.clone(),
            RestorePlanItem::Directory { .. } => unreachable!("fixture is a file"),
        };
        let item = FileRestorePlan::file(path, content).unwrap();
        let item_count =
            (MAX_RESTORE_PATH_COMPONENTS_PER_JOB / crate::MAX_SAFE_PATH_COMPONENTS) + 1;

        assert!(matches!(
            RestoreJobPlan::new("component-overflow", vec![item; item_count]),
            Err(RestoreError::JobPathComponentLimit {
                maximum: MAX_RESTORE_PATH_COMPONENTS_PER_JOB,
                ..
            })
        ));
    }

    #[test]
    fn restore_job_plan_rejects_aggregate_untrusted_path_evidence_over_budget() {
        let unsafe_name = format!("{}:", "x".repeat(32 * 1024));
        let path = SafeRelativePath::derive_for_recovery(&[] as &[&str], &unsafe_name).unwrap();
        let evidence_len = path.evidence_json().len();
        let content = match &one_item_job("evidence-template", b"x").items[0].item {
            RestorePlanItem::File(content) => content.clone(),
            RestorePlanItem::Directory { .. } => unreachable!("fixture is a file"),
        };
        let item = FileRestorePlan::file(path, content).unwrap();
        let item_count = (MAX_PATH_EVIDENCE_BYTES_PER_JOB / evidence_len) + 1;

        assert!(matches!(
            RestoreJobPlan::new("evidence-overflow", vec![item; item_count]),
            Err(RestoreError::JobPathEvidenceLimit {
                maximum: MAX_PATH_EVIDENCE_BYTES_PER_JOB,
                ..
            })
        ));
    }

    #[test]
    fn restore_job_path_budget_checked_arithmetic_accepts_boundary_and_rejects_overflow() {
        assert_eq!(
            checked_job_budget_total([999_999usize, 1], MAX_RESTORE_PATH_COMPONENTS_PER_JOB)
                .unwrap(),
            MAX_RESTORE_PATH_COMPONENTS_PER_JOB
        );
        assert!(matches!(
            checked_job_budget_total(
                [MAX_RESTORE_PATH_COMPONENTS_PER_JOB, 1],
                MAX_RESTORE_PATH_COMPONENTS_PER_JOB
            ),
            Err(actual) if actual == MAX_RESTORE_PATH_COMPONENTS_PER_JOB + 1
        ));
        assert!(matches!(
            checked_job_budget_total([usize::MAX, 1], usize::MAX),
            Err(usize::MAX)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_namespace_sync_classifier_distinguishes_unsupported_from_media_failures() {
        for raw in [5, 50, 120] {
            assert!(
                namespace_sync_is_unsupported(&io::Error::from_raw_os_error(raw)),
                "raw Windows error {raw} is an expected unsupported directory-flush method"
            );
        }
        for raw in [1, 6, 21, 23, 29, 31, 87, 112, 1117, 1167] {
            assert!(
                !namespace_sync_is_unsupported(&io::Error::from_raw_os_error(raw)),
                "raw Windows error {raw} must remain a failed durability boundary"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn transaction_windows_data_collision_keeps_item_sidecar_and_safe_temp_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let job_id = "windows-protected-cleanup";
        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(b"read"),
                &partial_item_job(job_id),
                &NeverCancel,
                &mut NoProgress,
                &SyncedDataCollisionOnce {
                    injected: AtomicBool::new(false),
                },
            )
            .unwrap();
        let job_dir = temp.path().join(summary.job_directory_name());

        assert_eq!(
            std::fs::read(job_dir.join("partial.bin")).unwrap(),
            b"existing data sentinel"
        );
        assert_eq!(
            std::fs::read(job_dir.join("partial (recovered 1).bin")).unwrap(),
            b"read\0\0\0\0"
        );
        assert!(
            !job_dir.join("partial.bin.um-partial.json").exists(),
            "the losing data collision retained a misbound partial sidecar"
        );
        assert!(job_dir
            .join(".um-partial-000000-0000000000000034.json")
            .exists());
        assert!(
            std::fs::read_dir(&job_dir).unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".umrecovering")),
            "protected temp links are retained because path cleanup cannot be identity-atomic"
        );
        assert_eq!(
            summary.items()[0].temporary_disposition(),
            Some(TemporaryFileDisposition::RetainedBySafeCleanupPolicy)
        );
    }

    #[test]
    fn transaction_directory_created_before_bind_failure_is_manifested_for_reconciliation() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let job_id = "directory-created-before-bind-fault";
        let job = directory_item_job(job_id, 91, &["selected"], "only-directory");
        let job_dir = temp.path().join(job_directory_component(job_id));

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(b""),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &FailFinalDirectoryTransitionAfterCollision {
                    job_dir: job_dir.clone(),
                    parent_name: "selected",
                    requested_name: "only-directory",
                },
            )
            .unwrap_err();

        assert!(matches!(error, RestoreError::NeedsReconciliation { .. }));
        assert!(
            job_dir
                .join("selected")
                .join("only-directory (recovered 1)")
                .is_dir(),
            "the collision-resolved directory is already visible and must be reconciled"
        );
        assert!(
            job_dir.join("selected").join("only-directory").is_dir(),
            "the injected collision sentinel must never be removed"
        );
        let manifest = read_manifest_value(&job_dir);
        let item = &manifest["items"][0];
        assert_eq!(item["itemKey"], "000000-000000000000005b");
        assert_eq!(item["itemKind"], "directory");
        assert_eq!(item["disposition"], "directoryNeedsReconciliation");
        assert_eq!(
            item["publishedPath"],
            "selected/only-directory (recovered 1)"
        );
        assert_eq!(item["directoryNoFollowBindCompleted"], false);
        assert_eq!(item["directoryValidation"], "unconfirmed");
        assert_eq!(item["namespaceDurability"]["state"], "unconfirmed");
        assert_eq!(item["completionStatus"], "needsReconciliation");
        assert_eq!(item["failureKind"], "needsReconciliation");

        let records = read_journal_values(&job_dir);
        let planned = records
            .iter()
            .filter(|record| record["kind"] == "directoryPublicationPlanned")
            .collect::<Vec<_>>();
        assert_eq!(planned.len(), 2);
        assert_eq!(
            planned[1]["payload"]["publishedPath"],
            "selected/only-directory (recovered 1)"
        );
        assert_eq!(
            planned[1]["payload"]["publishedName"],
            "only-directory (recovered 1)"
        );
        assert!(records
            .iter()
            .any(|record| record["kind"] == "directoryReconciliationRequired"));
    }

    #[test]
    fn transaction_directory_nofollow_bind_substitution_is_contained_and_manifested() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let sentinel = outside.path().join("sentinel");
        std::fs::write(&sentinel, b"outside remains unchanged").unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let job_id = "directory-bind-substitution";
        let job = directory_item_job(job_id, 92, &[], "substituted-directory");
        let job_dir = temp.path().join(job_directory_component(job_id));
        let runtime = ReplaceCreatedComponent {
            job_dir: job_dir.clone(),
            parents: vec!["substituted-directory".into()],
            outside: outside.path().to_owned(),
            target_index: 0,
            replacements: AtomicUsize::new(0),
        };

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(b""),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &runtime,
            )
            .unwrap_err();

        let manifest = read_manifest_value(&job_dir);
        let item = manifest["items"][0].clone();
        let redirect_was_retained = runtime.target_path().is_dir();
        let created_directory_was_retained =
            runtime.target_path().with_extension("held-0").is_dir();
        let outside_bytes = std::fs::read(&sentinel).unwrap();
        let outside_entries = std::fs::read_dir(outside.path()).unwrap().count();
        remove_test_directory_redirect(&runtime.target_path()).unwrap();

        assert!(matches!(error, RestoreError::NeedsReconciliation { .. }));
        assert!(
            redirect_was_retained,
            "production removed a substituted name"
        );
        assert!(
            created_directory_was_retained,
            "production removed the created directory after substitution"
        );
        assert_eq!(outside_bytes, b"outside remains unchanged");
        assert_eq!(outside_entries, 1, "recovery wrote outside its capability");
        assert_eq!(item["itemKey"], "000000-000000000000005c");
        assert_eq!(item["itemKind"], "directory");
        assert_eq!(item["disposition"], "directoryNeedsReconciliation");
        assert_eq!(item["publishedPath"], "substituted-directory");
        assert_eq!(item["directoryNoFollowBindCompleted"], false);
        assert_eq!(item["directoryValidation"], "unconfirmed");
        assert_eq!(item["namespaceDurability"]["state"], "unconfirmed");
        assert_eq!(item["completionStatus"], "needsReconciliation");
    }

    #[test]
    fn transaction_directory_namespace_sync_failure_is_manifested_for_reconciliation() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let job_id = "directory-namespace-fault";
        let job = directory_item_job(job_id, 93, &[], "directory");

        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(b""),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &NamespaceFault {
                    boundary: NamespaceBoundary::Data,
                },
            )
            .unwrap();

        assert_eq!(
            summary.completion_status(),
            RestoreCompletionStatus::NeedsReconciliation
        );
        let job_dir = temp.path().join(summary.job_directory_name());
        let manifest = read_manifest_value(&job_dir);
        let item = &manifest["items"][0];
        assert_eq!(item["itemKey"], "000000-000000000000005d");
        assert_eq!(item["itemKind"], "directory");
        assert_eq!(item["disposition"], "directoryNeedsReconciliation");
        assert_eq!(item["publishedPath"], "directory");
        assert_eq!(item["directoryNoFollowBindCompleted"], true);
        assert_eq!(item["directoryValidation"], "boundNoFollow");
        assert_eq!(item["namespaceDurability"]["state"], "failed");
        assert_eq!(item["completionStatus"], "needsReconciliation");
        assert_eq!(item["failureKind"], "needsReconciliation");
    }

    #[test]
    fn transaction_directory_item_published_journal_failure_stops_with_durable_candidate_evidence()
    {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let job_id = "directory-item-published-fault";
        let job = directory_item_job(job_id, 94, &[], "directory");

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(b""),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &JournalKindWriteFault {
                    kind: "itemPublished",
                },
            )
            .unwrap_err();

        assert!(matches!(error, RestoreError::NeedsReconciliation { .. }));
        let job_dir = temp.path().join(job_directory_component(job_id));
        assert!(job_dir.join("directory").is_dir());
        assert!(
            !job_dir.join(MANIFEST_NAME).exists(),
            "a poisoned journal must suppress the final manifest"
        );
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("the durable prefix before itemPublished remains auditable");
        assert_eq!(
            audit.kinds(),
            ["jobStarted", "itemPrepared", "directoryPublicationPlanned"]
        );
        let records = read_journal_values(&job_dir);
        let planned = &records[2]["payload"];
        assert_eq!(planned["itemKey"], "000000-000000000000005e");
        assert_eq!(planned["itemKind"], "directory");
        assert_eq!(planned["publishedPath"], "directory");
        assert_eq!(planned["publishedName"], "directory");
    }

    #[test]
    fn transaction_item_published_sync_failure_is_explicitly_reconcilable() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"published before journal failure";
        let job = one_item_job("published-sync-fault", bytes);

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &JournalFault {
                    record: 2,
                    stage: JournalFaultStage::Sync,
                },
            )
            .unwrap_err();

        assert!(matches!(error, RestoreError::NeedsReconciliation { .. }));
        let job_dir = temp
            .path()
            .join(job_directory_component("published-sync-fault"));
        assert_eq!(std::fs::read(job_dir.join("published.bin")).unwrap(), bytes);
        assert!(
            std::fs::read_dir(job_dir).unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".umrecovering")),
            "the temporary link must remain for reconciliation"
        );
    }

    #[test]
    fn transaction_item_prepared_faults_never_begin_publication() {
        for (label, stage) in [
            ("write", JournalFaultStage::Write),
            ("flush", JournalFaultStage::Flush),
            ("sync", JournalFaultStage::Sync),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
                .unwrap()
                .into_std_file();
            let destination = DestinationRoot::from_retained_file(retained).unwrap();
            let bytes = b"prepared journal boundary";
            let job_id = format!("prepared-{label}-fault");
            let job = one_item_job(&job_id, bytes);

            let error = destination
                .restore_job_with_runtime(
                    &MemoryReader::new(bytes),
                    &job,
                    &NeverCancel,
                    &mut NoProgress,
                    &JournalFault { record: 1, stage },
                )
                .unwrap_err();

            assert!(matches!(error, RestoreError::JournalIo { .. }));
            let job_dir = temp.path().join(job_directory_component(&job_id));
            assert!(
                !job_dir.join("published.bin").exists(),
                "{label} failure published a final name"
            );
            assert!(
                !job_dir.join(MANIFEST_NAME).exists(),
                "{label} failure published a manifest"
            );
        }
    }

    #[test]
    fn transaction_item_published_faults_are_all_reconcilable() {
        for (label, stage) in [
            ("write", JournalFaultStage::Write),
            ("flush", JournalFaultStage::Flush),
            ("sync", JournalFaultStage::Sync),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
                .unwrap()
                .into_std_file();
            let destination = DestinationRoot::from_retained_file(retained).unwrap();
            let bytes = b"published journal boundary";
            let job_id = format!("published-{label}-fault");
            let job = one_item_job(&job_id, bytes);

            let error = destination
                .restore_job_with_runtime(
                    &MemoryReader::new(bytes),
                    &job,
                    &NeverCancel,
                    &mut NoProgress,
                    &JournalFault { record: 2, stage },
                )
                .unwrap_err();

            assert!(matches!(error, RestoreError::NeedsReconciliation { .. }));
            let job_dir = temp.path().join(job_directory_component(&job_id));
            assert_eq!(std::fs::read(job_dir.join("published.bin")).unwrap(), bytes);
            assert!(
                !job_dir.join(MANIFEST_NAME).exists(),
                "{label} failure continued to manifest publication"
            );
            assert!(
                std::fs::read_dir(job_dir).unwrap().any(|entry| entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".umrecovering")),
                "{label} failure removed reconciliation evidence"
            );
        }
    }

    #[test]
    fn transaction_namespace_failure_after_data_link_is_reconcilable() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"namespace failure";
        let job = one_item_job("namespace-failure", bytes);

        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &NamespaceFault {
                    boundary: NamespaceBoundary::Data,
                },
            )
            .unwrap();

        assert_eq!(
            summary.completion_status(),
            RestoreCompletionStatus::NeedsReconciliation
        );
        assert!(matches!(
            summary.items()[0].namespace_durability(),
            NamespaceDurability::Failed { .. }
        ));
        assert_eq!(
            summary.items()[0].temporary_disposition(),
            Some(TemporaryFileDisposition::RetainedForReconciliation)
        );
        let job_dir = temp
            .path()
            .join(job_directory_component("namespace-failure"));
        assert_eq!(std::fs::read(job_dir.join("published.bin")).unwrap(), bytes);
        assert!(std::fs::read_dir(job_dir).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".umrecovering")));
    }

    #[test]
    fn transaction_unsupported_hard_link_never_falls_back_to_copy_or_rename() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"unsupported link";
        let job = one_item_job("unsupported-link", bytes);

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &UnsupportedHardLinks,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RestoreError::HardLinkPublication {
                kind: io::ErrorKind::Unsupported,
                ..
            }
        ));
        let job_dir = temp
            .path()
            .join(job_directory_component("unsupported-link"));
        assert!(!job_dir.join("published.bin").exists());
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("hard-link rejection has a complete failure journal");
        assert_eq!(
            audit.kinds(),
            [
                "jobStarted",
                "itemPrepared",
                "itemFailed",
                "manifestPrepared",
                "manifestPublished",
                "jobFailed"
            ]
        );
    }

    #[test]
    fn transaction_concurrent_creators_win_atomically_until_a_free_variant() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"recovered content";
        let job = one_item_job("racing-creators", bytes);
        let runtime = RacingCreator::new(32);

        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &runtime,
            )
            .unwrap();

        assert_eq!(runtime.attempts(), 33);
        assert_eq!(
            summary.items()[0].published_name(),
            "published (recovered 32).bin"
        );
        let job_dir = temp.path().join(job_directory_component("racing-creators"));
        for index in 0..32 {
            let name = if index == 0 {
                "published.bin".to_owned()
            } else {
                format!("published (recovered {index}).bin")
            };
            assert_eq!(
                std::fs::read_to_string(job_dir.join(name)).unwrap(),
                format!("racer-{index}")
            );
        }
        assert_eq!(
            std::fs::read(job_dir.join("published (recovered 32).bin")).unwrap(),
            bytes
        );
    }

    #[test]
    fn transaction_collision_retry_stops_at_the_exact_bound() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"never published";
        let job = one_item_job("collision-bound", bytes);
        let runtime = RacingCreator::new(MAX_COLLISION_ATTEMPTS);

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &runtime,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RestoreError::CollisionLimitExceeded {
                maximum: MAX_COLLISION_ATTEMPTS
            }
        ));
        assert_eq!(runtime.attempts(), MAX_COLLISION_ATTEMPTS);
        let job_dir = temp.path().join(job_directory_component("collision-bound"));
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("collision exhaustion has a complete terminal journal");
        assert_eq!(audit.kinds().last(), Some(&"jobFailed"));
    }

    #[test]
    fn transaction_scripted_replacement_at_every_create_bind_transition_stays_contained() {
        let parents = vec!["level-0", "level-1", "level-2"];
        for target_index in 0..parents.len() {
            let temp = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let sentinel = outside.path().join("sentinel.bin");
            std::fs::write(&sentinel, b"outside remains unchanged").unwrap();
            let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
                .unwrap()
                .into_std_file();
            let destination = DestinationRoot::from_retained_file(retained).unwrap();
            let bytes = b"must stay inside retained capabilities";
            let job_id = format!("transition-{target_index}");
            let mut job = one_item_job(&job_id, bytes);
            job.items[0].path = SafeRelativePath::derive_for_recovery(&parents, "published.bin")
                .unwrap()
                .path()
                .clone();
            let runtime = ReplaceCreatedComponent {
                job_dir: temp.path().join(job_directory_component(&job_id)),
                parents: parents.iter().map(|value| (*value).to_owned()).collect(),
                outside: outside.path().to_owned(),
                target_index,
                replacements: AtomicUsize::new(0),
            };

            let error = destination
                .restore_job_with_runtime(
                    &MemoryReader::new(bytes),
                    &job,
                    &NeverCancel,
                    &mut NoProgress,
                    &runtime,
                )
                .unwrap_err();

            assert!(matches!(error, RestoreError::DestinationIo { .. }));
            assert_eq!(runtime.replacements.load(Ordering::SeqCst), 1);
            assert_eq!(
                std::fs::read(&sentinel).unwrap(),
                b"outside remains unchanged"
            );
            assert_eq!(
                std::fs::read_dir(outside.path()).unwrap().count(),
                1,
                "replacement at transition {target_index} created an outside entry"
            );
            remove_test_directory_redirect(&runtime.target_path()).unwrap();
        }
    }

    #[test]
    fn transaction_cancellation_after_the_final_write_does_not_split_commit() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"commit is non-cancellable";
        let job = one_item_job("non-cancellable-commit", bytes);
        let cancelled = Arc::new(AtomicBool::new(false));

        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &SharedCancel(Arc::clone(&cancelled)),
                &mut CancelOnProgress(Arc::clone(&cancelled)),
                &ProductionRuntime,
            )
            .unwrap();

        assert!(cancelled.load(Ordering::SeqCst));
        assert!(matches!(
            summary.completion_status(),
            RestoreCompletionStatus::CompletedDurable
                | RestoreCompletionStatus::NeedsReconciliation
        ));
        let job_dir = temp
            .path()
            .join(job_directory_component("non-cancellable-commit"));
        assert_eq!(std::fs::read(job_dir.join("published.bin")).unwrap(), bytes);
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("the non-cancellable commit journal is complete");
        assert!(audit
            .kinds()
            .windows(2)
            .any(|kinds| kinds == ["itemPrepared", "itemPublished"]));
    }

    #[test]
    fn transaction_multi_gigabyte_candidate_stays_bounded_before_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let logical_size = 5 * 1024 * 1024 * 1024u64;
        let source = PatternReader::new(logical_size);
        let job = one_item_job_for_len("bounded-cancel", logical_size);
        let cancelled = Arc::new(AtomicBool::new(false));

        let error = destination
            .restore_job_with_runtime(
                &source,
                &job,
                &SharedCancel(Arc::clone(&cancelled)),
                &mut CancelOnProgress(Arc::clone(&cancelled)),
                &ProductionRuntime,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RestoreError::Cancelled {
                bytes_written
            } if bytes_written == RESTORE_SCRATCH_BYTES as u64
        ));
        assert_eq!(
            source.max_read.load(Ordering::SeqCst),
            RESTORE_SCRATCH_BYTES
        );
        let job_dir = temp.path().join(job_directory_component("bounded-cancel"));
        assert!(!job_dir.join("published.bin").exists());
        let temporary_len = std::fs::read_dir(job_dir)
            .unwrap()
            .find_map(|entry| {
                let entry = entry.unwrap();
                entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".umrecovering")
                    .then(|| entry.metadata().unwrap().len())
            })
            .expect("cancelled output keeps one bounded temporary file");
        assert_eq!(temporary_len, RESTORE_SCRATCH_BYTES as u64);
        let audit = crate::audit_journal(
            &std::fs::read(
                temp.path()
                    .join(job_directory_component("bounded-cancel"))
                    .join(JOURNAL_NAME),
            )
            .unwrap(),
        )
        .expect("cancellation journal is complete");
        assert_eq!(
            audit.kinds(),
            [
                "jobStarted",
                "itemCancelled",
                "manifestPrepared",
                "manifestPublished",
                "jobCancelled"
            ]
        );
    }

    #[test]
    fn transaction_hash_mismatch_never_publishes_and_has_terminal_failure_records() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"hash mismatch fixture";
        let mut job = one_item_job("hash-mismatch", bytes);
        match &mut job.items[0].item {
            RestorePlanItem::File(content) => content.expected_sha256 = Some([0; 32]),
            RestorePlanItem::Directory { .. } => unreachable!("fixture is a file"),
        }

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &ProductionRuntime,
            )
            .unwrap_err();

        assert!(matches!(error, RestoreError::HashMismatch { .. }));
        let job_dir = temp.path().join(job_directory_component("hash-mismatch"));
        assert!(!job_dir.join("published.bin").exists());
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("hash mismatch journal is complete");
        assert_eq!(
            audit.kinds(),
            [
                "jobStarted",
                "itemFailed",
                "manifestPrepared",
                "manifestPublished",
                "jobFailed"
            ]
        );
    }

    #[test]
    fn transaction_independent_sidecar_and_data_collisions_do_not_retract_the_sidecar() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let runtime = SidecarCollisionRecorder::new();

        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(b"read"),
                &partial_item_job("sidecar-first"),
                &NeverCancel,
                &mut NoProgress,
                &runtime,
            )
            .unwrap();

        assert_eq!(
            summary.items()[0].published_name(),
            "partial (recovered 1).bin"
        );
        let events = runtime.events.lock().unwrap().clone();
        assert_eq!(
            events,
            [
                LinkBoundary::PartialSidecar,
                LinkBoundary::PartialSidecar,
                LinkBoundary::Data,
                LinkBoundary::Data,
                LinkBoundary::Manifest,
            ]
        );
        let job_dir = temp.path().join(job_directory_component("sidecar-first"));
        assert_eq!(
            std::fs::read(job_dir.join(".um-partial-000000-0000000000000034.json")).unwrap(),
            b"existing sidecar sentinel"
        );
        assert!(
            job_dir.join("partial.bin").exists(),
            "the data racer must retain the first data collision name"
        );
        assert_eq!(
            std::fs::read(job_dir.join("partial (recovered 1).bin")).unwrap(),
            b"read\0\0\0\0"
        );
        assert!(job_dir
            .join(".um-partial-000000-0000000000000034 (recovered 1).json")
            .exists());
        assert!(!job_dir.join("partial.bin.um-partial.json").exists());
        assert!(!job_dir
            .join("partial (recovered 1).bin.um-partial.json")
            .exists());
    }

    #[test]
    fn transaction_failed_data_publication_manifest_binds_the_published_sidecar() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let job_id = "sidecar-before-data-failure";

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(b"read"),
                &partial_item_job(job_id),
                &NeverCancel,
                &mut NoProgress,
                &UnsupportedDataAfterSidecar,
            )
            .unwrap_err();

        assert!(matches!(error, RestoreError::HardLinkPublication { .. }));
        let job_dir = temp.path().join(job_directory_component(job_id));
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(job_dir.join(MANIFEST_NAME)).unwrap()).unwrap();
        let item = &manifest["items"][0];
        assert_eq!(item["disposition"], "failed");
        assert_eq!(item["itemKey"], "000000-0000000000000034");
        let sidecar_name = item["partialSidecar"].as_str().unwrap();
        let sidecar = std::fs::read(job_dir.join(sidecar_name)).unwrap();
        assert_eq!(
            item["partialSidecarSha256"],
            hex_lower(&Sha256::digest(&sidecar))
        );
        let sidecar_json: serde_json::Value = serde_json::from_slice(&sidecar).unwrap();
        assert_eq!(sidecar_json["itemKey"], item["itemKey"]);
        assert_eq!(sidecar_json["candidateId"], item["candidateId"]);
        assert_eq!(
            sidecar_json["outputSha256"],
            "abf55adc5afcfa05c3c6f2d044c07faaf1d7da502801d43afcbdbb3a07bdb2c0"
        );
    }

    #[test]
    fn transaction_sidecar_journal_fault_stops_before_data_and_keeps_reconciliation_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let job_id = "sidecar-journal-fault";

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(b"read"),
                &partial_item_job(job_id),
                &NeverCancel,
                &mut NoProgress,
                &JournalFault {
                    record: 2,
                    stage: JournalFaultStage::Write,
                },
            )
            .unwrap_err();

        assert!(matches!(error, RestoreError::NeedsReconciliation { .. }));
        let job_dir = temp.path().join(job_directory_component(job_id));
        assert!(job_dir
            .join(".um-partial-000000-0000000000000034.json")
            .exists());
        assert!(!job_dir.join("partial.bin").exists());
        assert!(!job_dir.join(MANIFEST_NAME).exists());
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("the durable prefix remains auditable");
        assert_eq!(audit.kinds(), ["jobStarted", "itemPrepared"]);
    }

    #[test]
    fn transaction_manifest_and_terminal_journal_faults_preserve_reconciliation_evidence() {
        for (record_label, record) in [("manifestPublished", 4), ("jobCompleted", 5)] {
            for (stage_label, stage) in [
                ("write", JournalFaultStage::Write),
                ("flush", JournalFaultStage::Flush),
                ("sync", JournalFaultStage::Sync),
            ] {
                let temp = tempfile::tempdir().unwrap();
                let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
                    .unwrap()
                    .into_std_file();
                let destination = DestinationRoot::from_retained_file(retained).unwrap();
                let bytes = b"manifest was linked";
                let job_id = format!("{record_label}-{stage_label}-fault");
                let job = one_item_job(&job_id, bytes);

                let error = destination
                    .restore_job_with_runtime(
                        &MemoryReader::new(bytes),
                        &job,
                        &NeverCancel,
                        &mut NoProgress,
                        &JournalFault { record, stage },
                    )
                    .unwrap_err();

                assert!(matches!(error, RestoreError::NeedsReconciliation { .. }));
                let job_dir = temp.path().join(job_directory_component(&job_id));
                assert_eq!(std::fs::read(job_dir.join("published.bin")).unwrap(), bytes);
                assert!(
                    job_dir.join(MANIFEST_NAME).exists(),
                    "{record_label} {stage_label} failure lost the linked manifest"
                );
                assert!(
                    std::fs::read_dir(job_dir).unwrap().any(|entry| entry
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .ends_with(".umrecovering")),
                    "{record_label} {stage_label} failure removed reconciliation evidence"
                );
            }
        }
    }

    #[test]
    fn transaction_manifest_namespace_failure_is_reported_and_keeps_its_temp_link() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"manifest namespace";
        let job = one_item_job("manifest-namespace", bytes);

        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &NamespaceFault {
                    boundary: NamespaceBoundary::Manifest,
                },
            )
            .unwrap();

        assert_eq!(
            summary.completion_status(),
            RestoreCompletionStatus::NeedsReconciliation
        );
        let job_dir = temp
            .path()
            .join(job_directory_component("manifest-namespace"));
        assert!(job_dir.join(MANIFEST_NAME).exists());
        assert!(std::fs::read_dir(job_dir).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".umrecovering")));
    }

    #[test]
    fn transaction_protected_temps_are_retained_without_path_based_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"published output keeps protected temp evidence";
        let job = one_item_job("safe-cleanup-policy", bytes);

        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &AlwaysSynced,
            )
            .unwrap();

        let item = &summary.items()[0];
        assert_eq!(
            item.temporary_disposition(),
            Some(TemporaryFileDisposition::RetainedBySafeCleanupPolicy)
        );
        assert!(item.warnings.is_empty());
        let job_dir = temp
            .path()
            .join(job_directory_component("safe-cleanup-policy"));
        assert_eq!(std::fs::read(job_dir.join("published.bin")).unwrap(), bytes);
        assert!(std::fs::read_dir(&job_dir).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("-data-")));
        assert!(job_dir.join(MANIFEST_NAME).exists());
        assert!(std::fs::read_dir(&job_dir).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("-manifest-")));
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("safe-retention journal is complete");
        assert!(!audit.kinds().iter().any(|kind| kind.contains("Cleanup")));
        assert_eq!(audit.kinds().last(), Some(&"jobCompleted"));
    }

    #[test]
    fn transaction_manifest_collision_is_no_clobber_and_has_a_terminal_record() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"item published before manifest collision";
        let job = one_item_job("manifest-collision", bytes);

        let error = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &ManifestCollision,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RestoreError::HardLinkPublication {
                kind: io::ErrorKind::AlreadyExists,
                ..
            }
        ));
        let job_dir = temp
            .path()
            .join(job_directory_component("manifest-collision"));
        assert_eq!(
            std::fs::read(job_dir.join(MANIFEST_NAME)).unwrap(),
            b"existing manifest sentinel"
        );
        assert_eq!(std::fs::read(job_dir.join("published.bin")).unwrap(), bytes);
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("the manifest collision journal remains complete");
        assert_eq!(audit.kinds().last(), Some(&"jobFailed"));
    }
}
