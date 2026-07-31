//! Bounded content planning and streaming extraction.

#![forbid(unsafe_code)]

mod error;
mod journal;
mod manifest;
mod path;
mod plan;
mod stream;
mod transaction;

pub use error::RestoreError;
pub use journal::{audit_journal, JournalAudit};
pub use manifest::{
    NamespaceDurability, RestoreCompletionStatus, RestoreItemResult, RestoreSummary,
    TemporaryFileDisposition,
};
pub use path::{
    DerivedSafePath, PathSafetyError, SafeRelativePath, MAX_PATH_EVIDENCE_BYTES_PER_ITEM,
    MAX_SAFE_COMPONENT_UTF16, MAX_SAFE_PATH_COMPONENTS, MAX_SAFE_PATH_UTF16,
};
pub use plan::{
    plan_candidate, ContentPlan, ContentSegment, PartialPolicy, PlanLimits, ZeroFillReason,
    MAX_CONTENT_PLAN_SEGMENTS, MAX_STREAM_BUFFER_BYTES,
};
pub use stream::{
    stream_candidate, CancellationProbe, ProgressSink, StreamOutcome, StreamProgress,
    ZeroFilledRange, MAX_ZERO_FILLED_RANGES,
};
pub use transaction::{
    job_directory_component, DestinationRoot, FileRestorePlan, RestoreJobPlan,
    MAX_COLLISION_ATTEMPTS, MAX_PATH_EVIDENCE_BYTES_PER_JOB, MAX_RESTORE_JOB_ITEMS,
    MAX_RESTORE_PATH_COMPONENTS_PER_JOB, RESTORE_SCRATCH_BYTES,
};
