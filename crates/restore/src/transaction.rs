use crate::journal::{hex_lower, JobJournal, JOURNAL_NAME};
use crate::manifest::{
    build_manifest, build_partial_sidecar, NamespaceDurability, RestoreCompletionStatus,
    RestoreItemResult, RestoreSummary, TemporaryFileDisposition, MANIFEST_NAME,
    PARTIAL_SIDECAR_SUFFIX,
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
const MAX_JOB_ID_CHARS: usize = 128;
const MAX_TEMP_CREATE_ATTEMPTS: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamespaceBoundary {
    SidecarPublished,
    CollisionSidecarRemoved,
    DataPublished,
    ManifestPublished,
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
enum RemovalBoundary {
    CollisionSidecarFinal,
    FailedPublicationSidecarFinal,
    ItemDataTemporary,
    ItemSidecarTemporary,
    ManifestTemporary,
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

    fn remove_file(
        &self,
        parent: &Dir,
        name: &str,
        _boundary: RemovalBoundary,
    ) -> std::io::Result<()> {
        parent.remove_file(name)
    }
}

struct ProductionRuntime;

impl TransactionRuntime for ProductionRuntime {}

#[derive(Debug, Clone)]
pub struct FileRestorePlan {
    path: SafeRelativePath,
    path_evidence_json: String,
    content: ContentPlan,
    warnings: Vec<String>,
}

impl FileRestorePlan {
    pub fn file(path: DerivedSafePath, content: ContentPlan) -> Result<Self, RestoreError> {
        let (path, path_evidence_json) = path.into_parts();
        Ok(Self {
            path,
            path_evidence_json,
            content,
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
        Ok(Self {
            job_id: job_id.to_owned(),
            items,
        })
    }
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
        for item in &plan.items {
            if cancel.is_cancelled() {
                let error = RestoreError::Cancelled { bytes_written: 0 };
                record_item_and_job_failure(&mut journal, item.content.candidate_id, &error)?;
                return Err(error);
            }
            match restore_item(
                &job_dir,
                source,
                item,
                cancel,
                progress,
                &mut journal,
                runtime,
            ) {
                Ok(result) => results.push(result),
                Err(error) => {
                    if !journal.is_poisoned() {
                        record_item_and_job_failure(
                            &mut journal,
                            item.content.candidate_id,
                            &error,
                        )?;
                    }
                    return Err(error);
                }
            }
        }

        let outcomes_prefix_sha256 = journal.head_hash().to_owned();
        let (manifest_bytes, manifest_sha256) =
            build_manifest(&plan.job_id, &outcomes_prefix_sha256, &results).map_err(|error| {
                RestoreError::ManifestSerialization {
                    message: error.to_string(),
                }
            })?;
        let mut manifest_temp =
            prepare_bytes(&job_dir, "manifest", &manifest_bytes, manifest_sha256)?;
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

        verify_prelink_identity(&job_dir, &manifest_temp)?;
        if let Err(error) = runtime.hard_link(
            &job_dir,
            &manifest_temp.name,
            MANIFEST_NAME,
            LinkBoundary::Manifest,
        ) {
            let error = hard_link_error(error);
            if let Err(journal_error) = record_job_failure(&mut journal, &error) {
                return Err(publication_reconciliation_error(
                    "manifest failure record",
                    MANIFEST_NAME,
                    journal_error,
                ));
            }
            return Err(error);
        }
        let manifest_sync = runtime.sync_parent(&job_dir, NamespaceBoundary::ManifestPublished);
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
        if manifest_sync.is_synced() {
            match runtime.remove_file(
                &job_dir,
                &manifest_temp.name,
                RemovalBoundary::ManifestTemporary,
            ) {
                Ok(()) => manifest_temp.removed = true,
                Err(_) => {
                    if let Err(error) = journal.append(
                        "manifestCleanupWarning",
                        &ManifestCleanupWarning {
                            temporary_name: manifest_temp.name.clone(),
                        },
                    ) {
                        return Err(publication_reconciliation_error(
                            "manifest cleanup warning",
                            MANIFEST_NAME,
                            error,
                        ));
                    }
                }
            }
        }
        if let Err(error) = journal.append(
            "jobCompleted",
            &JobCompleted {
                item_count: results.len(),
                completion_status: overall_status,
            },
        ) {
            return Err(publication_reconciliation_error(
                "job terminal record",
                MANIFEST_NAME,
                error,
            ));
        }

        Ok(RestoreSummary {
            job_directory_name,
            manifest_name: MANIFEST_NAME.to_owned(),
            journal_name: JOURNAL_NAME.to_owned(),
            manifest_sha256,
            items: results,
            completion_status: overall_status,
        })
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

pub fn job_directory_component(job_id: &str) -> String {
    let digest = Sha256::digest(job_id.as_bytes());
    format!(".um-recovery-{}", &hex_lower(&digest)[..32])
}

fn restore_item(
    job_dir: &Dir,
    source: &dyn SourceReader,
    plan: &FileRestorePlan,
    cancel: &dyn CancellationProbe,
    progress: &mut dyn ProgressSink,
    journal: &mut JobJournal,
    runtime: &dyn TransactionRuntime,
) -> Result<RestoreItemResult, RestoreError> {
    let parent = open_or_create_parents(job_dir, &plan.path, runtime)?;
    let mut temporary = prepare_stream(&parent, source, &plan.content, cancel, progress)?;
    let sidecar_bytes = build_partial_sidecar(plan.content.logical_size, &temporary.outcome)
        .map_err(|error| RestoreError::ManifestSerialization {
            message: error.to_string(),
        })?;
    let mut sidecar_temp = match sidecar_bytes {
        Some(bytes) => {
            let hash = Sha256::digest(&bytes).into();
            Some(prepare_bytes(&parent, "partial", &bytes, hash)?)
        }
        None => None,
    };

    journal.append(
        "itemPrepared",
        &ItemPrepared {
            candidate_id: plan.content.candidate_id,
            requested_path: plan.path.to_slash_string(),
            temporary_name: temporary.name.clone(),
            output_length: temporary.outcome.bytes_written,
            output_sha256: hex_lower(&temporary.outcome.sha256),
            expected_sha256: plan.content.expected_sha256.map(|hash| hex_lower(&hash)),
            sidecar_temporary_name: sidecar_temp.as_ref().map(|file| file.name.clone()),
        },
    )?;

    let base_name = plan.path.file_name();
    for collision_index in 0..MAX_COLLISION_ATTEMPTS {
        let final_name = collision_name(base_name, collision_index);
        let sidecar_name = sidecar_temp
            .as_ref()
            .map(|_| partial_sidecar_name(&final_name));
        let mut sidecar_linked = false;
        let mut durability = NamespaceDurability::Synced;

        if let (Some(sidecar), Some(sidecar_final)) = (sidecar_temp.as_ref(), sidecar_name.as_ref())
        {
            verify_prelink_identity(&parent, sidecar)?;
            match runtime.hard_link(
                &parent,
                &sidecar.name,
                sidecar_final,
                LinkBoundary::PartialSidecar,
            ) {
                Ok(()) => {
                    sidecar_linked = true;
                    durability = merge_durability(
                        durability,
                        runtime.sync_parent(&parent, NamespaceBoundary::SidecarPublished),
                    );
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(hard_link_error(error)),
            }
        }

        verify_prelink_identity(&parent, &temporary)?;
        match runtime.hard_link(&parent, &temporary.name, &final_name, LinkBoundary::Data) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if sidecar_linked {
                    let sidecar_final = sidecar_name
                        .as_ref()
                        .expect("linked sidecar has a final name");
                    runtime
                        .remove_file(
                            &parent,
                            sidecar_final,
                            RemovalBoundary::CollisionSidecarFinal,
                        )
                        .map_err(|cleanup_error| RestoreError::NeedsReconciliation {
                            message: format!(
                                "collision sidecar cleanup failed ({:?}): {}",
                                cleanup_error.kind(),
                                cleanup_error
                            ),
                        })?;
                    durability = merge_durability(
                        durability,
                        runtime.sync_parent(&parent, NamespaceBoundary::CollisionSidecarRemoved),
                    );
                    if matches!(durability, NamespaceDurability::Failed { .. }) {
                        return Err(RestoreError::NeedsReconciliation {
                            message: "collision cleanup namespace sync failed".into(),
                        });
                    }
                }
                continue;
            }
            Err(error) => {
                if sidecar_linked {
                    let sidecar_final = sidecar_name
                        .as_ref()
                        .expect("linked sidecar has a final name");
                    runtime
                        .remove_file(
                            &parent,
                            sidecar_final,
                            RemovalBoundary::FailedPublicationSidecarFinal,
                        )
                        .map_err(|cleanup_error| RestoreError::NeedsReconciliation {
                            message: format!(
                                "failed-publication sidecar cleanup failed ({:?}): {}",
                                cleanup_error.kind(),
                                cleanup_error
                            ),
                        })?;
                    let cleanup_durability =
                        runtime.sync_parent(&parent, NamespaceBoundary::CollisionSidecarRemoved);
                    if matches!(cleanup_durability, NamespaceDurability::Failed { .. }) {
                        return Err(RestoreError::NeedsReconciliation {
                            message: "failed-publication sidecar cleanup namespace sync failed"
                                .into(),
                        });
                    }
                }
                return Err(hard_link_error(error));
            }
        }

        durability = merge_durability(
            durability,
            runtime.sync_parent(&parent, NamespaceBoundary::DataPublished),
        );
        let completion_status = durability_status(&durability);
        let published_path = join_published_path(&plan.path, &final_name);
        if let Err(error) = journal.append(
            "itemPublished",
            &ItemPublished {
                candidate_id: plan.content.candidate_id,
                published_path: published_path.clone(),
                sidecar_name: sidecar_name.clone(),
                namespace_durability: &durability,
                completion_status,
            },
        ) {
            return Err(publication_reconciliation_error(
                "item",
                &published_path,
                error,
            ));
        }

        let mut warnings = plan.warnings.clone();
        let mut temporary_disposition = TemporaryFileDisposition::RetainedForReconciliation;
        if durability.is_synced() {
            let data_cleanup =
                runtime.remove_file(&parent, &temporary.name, RemovalBoundary::ItemDataTemporary);
            let sidecar_cleanup = match sidecar_temp.as_ref() {
                Some(sidecar) => runtime.remove_file(
                    &parent,
                    &sidecar.name,
                    RemovalBoundary::ItemSidecarTemporary,
                ),
                None => Ok(()),
            };
            if data_cleanup.is_ok() && sidecar_cleanup.is_ok() {
                temporary.removed = true;
                if let Some(sidecar) = sidecar_temp.as_mut() {
                    sidecar.removed = true;
                }
                temporary_disposition = TemporaryFileDisposition::Removed;
            } else {
                temporary_disposition = TemporaryFileDisposition::RetainedAfterCleanupFailure;
                warnings.push("published output is durable; temporary-link cleanup failed".into());
                if let Err(error) = journal.append(
                    "itemCleanupWarning",
                    &ItemCleanupWarning {
                        candidate_id: plan.content.candidate_id,
                    },
                ) {
                    return Err(publication_reconciliation_error(
                        "item cleanup warning",
                        &published_path,
                        error,
                    ));
                }
            }
        }

        let path_evidence = serde_json::from_str(&plan.path_evidence_json).map_err(|error| {
            RestoreError::ManifestSerialization {
                message: error.to_string(),
            }
        })?;
        return Ok(RestoreItemResult {
            candidate_id: plan.content.candidate_id,
            requested_path: plan.path.to_slash_string(),
            published_path,
            published_name: final_name,
            output_len: temporary.outcome.bytes_written,
            output_sha256: temporary.outcome.sha256,
            expected_sha256: plan.content.expected_sha256,
            zero_filled_ranges: temporary.outcome.zero_filled_ranges.clone(),
            sidecar_name,
            temporary_disposition,
            path_evidence,
            warnings,
            namespace_durability: durability,
            completion_status,
        });
    }

    Err(RestoreError::CollisionLimitExceeded {
        maximum: MAX_COLLISION_ATTEMPTS,
    })
}

struct PreparedFile {
    name: String,
    file: File,
    identity: FileIdentity,
    outcome: crate::StreamOutcome,
    removed: bool,
}

impl Drop for PreparedFile {
    fn drop(&mut self) {
        let _ = self.removed;
    }
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
        removed: false,
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
        removed: false,
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

fn partial_sidecar_name(final_name: &str) -> String {
    let candidate = format!("{final_name}{PARTIAL_SIDECAR_SUFFIX}");
    if SafeRelativePath::try_from_components([candidate.as_str()]).is_ok() {
        candidate
    } else {
        let digest = Sha256::digest(candidate.as_bytes());
        format!("partial-{}.json", &hex_lower(&digest)[..40])
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
fn namespace_sync_is_unsupported(_error: &std::io::Error) -> bool {
    true
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

fn record_item_and_job_failure(
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
    )?;
    journal.append(
        if cancelled {
            "jobCancelled"
        } else {
            "jobFailed"
        },
        &FailureRecord {
            candidate_id: None,
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
    requested_path: String,
    temporary_name: String,
    output_length: u64,
    output_sha256: String,
    expected_sha256: Option<String>,
    sidecar_temporary_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemPublished<'a> {
    candidate_id: u64,
    published_path: String,
    sidecar_name: Option<String>,
    namespace_durability: &'a NamespaceDurability,
    completion_status: RestoreCompletionStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemCleanupWarning {
    candidate_id: u64,
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
struct ManifestCleanupWarning {
    temporary_name: String,
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
        injected: AtomicBool,
        events: Mutex<Vec<LinkBoundary>>,
    }

    impl SidecarCollisionRecorder {
        fn new() -> Self {
            Self {
                injected: AtomicBool::new(false),
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
                && !self.injected.swap(true, Ordering::SeqCst)
            {
                let mut existing = create_new_protected(parent, final_name)
                    .expect("the first sidecar name is initially free");
                existing.write_all(b"existing sidecar sentinel").unwrap();
                existing.sync_all().unwrap();
            }
            parent.hard_link(temporary_name, parent, final_name)
        }
    }

    struct CleanupFailure;

    impl TransactionRuntime for CleanupFailure {
        fn sync_parent(&self, _parent: &Dir, _boundary: NamespaceBoundary) -> NamespaceDurability {
            NamespaceDurability::Synced
        }

        fn remove_file(
            &self,
            parent: &Dir,
            name: &str,
            boundary: RemovalBoundary,
        ) -> io::Result<()> {
            if boundary == RemovalBoundary::ItemDataTemporary {
                Err(io::Error::other("injected temporary cleanup failure"))
            } else {
                parent.remove_file(name)
            }
        }
    }

    struct ManifestCleanupFailure;

    impl TransactionRuntime for ManifestCleanupFailure {
        fn sync_parent(&self, _parent: &Dir, _boundary: NamespaceBoundary) -> NamespaceDurability {
            NamespaceDurability::Synced
        }

        fn remove_file(
            &self,
            parent: &Dir,
            name: &str,
            boundary: RemovalBoundary,
        ) -> io::Result<()> {
            if boundary == RemovalBoundary::ManifestTemporary {
                Err(io::Error::other("injected manifest cleanup failure"))
            } else {
                parent.remove_file(name)
            }
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
                    boundary: NamespaceBoundary::DataPublished,
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
            TemporaryFileDisposition::RetainedForReconciliation
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
            ["jobStarted", "itemPrepared", "itemFailed", "jobFailed"]
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
            ["jobStarted", "itemCancelled", "jobCancelled"]
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
        job.items[0].content.expected_sha256 = Some([0; 32]);

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
        assert_eq!(audit.kinds(), ["jobStarted", "itemFailed", "jobFailed"]);
    }

    #[test]
    fn transaction_sidecar_collision_is_resolved_before_any_data_name_is_linked() {
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
                LinkBoundary::Manifest,
            ]
        );
        let job_dir = temp.path().join(job_directory_component("sidecar-first"));
        assert_eq!(
            std::fs::read(job_dir.join("partial.bin.um-partial.json")).unwrap(),
            b"existing sidecar sentinel"
        );
        assert!(
            !job_dir.join("partial.bin").exists(),
            "data must not be linked after its sidecar name loses the race"
        );
        assert_eq!(
            std::fs::read(job_dir.join("partial (recovered 1).bin")).unwrap(),
            b"read\0\0\0\0"
        );
        assert!(job_dir
            .join("partial (recovered 1).bin.um-partial.json")
            .exists());
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
                    boundary: NamespaceBoundary::ManifestPublished,
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
    fn transaction_cleanup_failure_keeps_only_job_temp_and_records_warning() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"published output survives cleanup failure";
        let job = one_item_job("cleanup-failure", bytes);

        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &CleanupFailure,
            )
            .unwrap();

        let item = &summary.items()[0];
        assert_eq!(
            item.temporary_disposition(),
            TemporaryFileDisposition::RetainedAfterCleanupFailure
        );
        assert_eq!(
            item.warnings,
            ["published output is durable; temporary-link cleanup failed"]
        );
        let job_dir = temp.path().join(job_directory_component("cleanup-failure"));
        assert_eq!(std::fs::read(job_dir.join("published.bin")).unwrap(), bytes);
        assert!(std::fs::read_dir(&job_dir).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("-data-")));
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("cleanup warning journal is complete");
        assert!(audit.kinds().contains(&"itemCleanupWarning"));
    }

    #[test]
    fn transaction_manifest_cleanup_failure_is_recorded_before_job_terminal() {
        let temp = tempfile::tempdir().unwrap();
        let retained = Dir::open_ambient_dir(temp.path(), ambient_authority())
            .unwrap()
            .into_std_file();
        let destination = DestinationRoot::from_retained_file(retained).unwrap();
        let bytes = b"manifest cleanup evidence";
        let job = one_item_job("manifest-cleanup-failure", bytes);

        let summary = destination
            .restore_job_with_runtime(
                &MemoryReader::new(bytes),
                &job,
                &NeverCancel,
                &mut NoProgress,
                &ManifestCleanupFailure,
            )
            .unwrap();

        let job_dir = temp.path().join(summary.job_directory_name());
        assert!(job_dir.join(MANIFEST_NAME).exists());
        assert!(std::fs::read_dir(&job_dir).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("-manifest-")));
        let audit = crate::audit_journal(&std::fs::read(job_dir.join(JOURNAL_NAME)).unwrap())
            .expect("manifest cleanup warning journal is complete");
        assert_eq!(
            &audit.kinds()[audit.record_count() - 2..],
            ["manifestCleanupWarning", "jobCompleted"]
        );
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
