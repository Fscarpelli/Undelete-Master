use crate::ZeroFillReason;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RestoreError {
    #[error("invalid content-plan segment limit {requested}; expected 1..={maximum}")]
    InvalidPlanLimits { requested: usize, maximum: usize },

    #[error("extent {index} has zero length")]
    ZeroLengthExtent { index: usize },

    #[error("logical extent overflow at offset {logical_offset} with length {len}")]
    LogicalRangeOverflow { logical_offset: u64, len: u64 },

    #[error(
        "logical extent at offset {logical_offset} with length {len} exceeds file size {logical_size}"
    )]
    LogicalRangeOutOfBounds {
        logical_offset: u64,
        len: u64,
        logical_size: u64,
    },

    #[error("logical extents overlap: previous end {previous_end}, next start {next_start}")]
    OverlappingExtents { previous_end: u64, next_start: u64 },

    #[error("physical extent overflow at offset {physical_offset} with length {len}")]
    PhysicalRangeOverflow { physical_offset: u64, len: u64 },

    #[error(
        "physical read at offset {physical_offset} with length {len} exceeds source size {source_len}"
    )]
    PhysicalRangeOutOfBounds {
        physical_offset: u64,
        len: u64,
        source_len: u64,
    },

    #[error(
        "content unavailable at logical offset {logical_offset} with length {len}: {reason:?}"
    )]
    UnavailableContent {
        logical_offset: u64,
        len: u64,
        reason: ZeroFillReason,
    },

    #[error("content plan exceeds its segment limit of {limit}")]
    SegmentLimitExceeded { limit: usize },

    #[error("invalid stream scratch length {len}; expected 1..={maximum}")]
    InvalidScratchLength { len: usize, maximum: usize },

    #[error("content plan segment {index} has zero length")]
    InvalidPlanSegment { index: usize },

    #[error(
        "content plan coverage is not contiguous at segment {index}: expected logical offset {expected_offset}, got {actual_offset}"
    )]
    InvalidPlanCoverage {
        index: usize,
        expected_offset: u64,
        actual_offset: u64,
    },

    #[error("content plan best-effort flag is inconsistent: expected {expected}, got {actual}")]
    InvalidPlanBestEffortFlag { expected: bool, actual: bool },

    #[error("source read failed at logical offset {logical_offset} with length {len}")]
    ReadFailure { logical_offset: u64, len: u64 },

    #[error("source returned an invalid best-effort outcome for {requested_len} requested bytes")]
    InvalidReadOutcome { requested_len: usize },

    #[error("zero-fill evidence exceeds its range limit of {limit}")]
    ZeroFillRangeLimitExceeded { limit: usize },

    #[error("stream cancelled after {bytes_written} bytes")]
    Cancelled { bytes_written: u64 },

    #[error("destination write failed after {bytes_written} bytes ({kind:?}): {message}")]
    OutputWrite {
        kind: std::io::ErrorKind,
        message: String,
        bytes_written: u64,
    },

    #[error("stream wrote {bytes_written} bytes but the content plan requires {logical_size}")]
    OutputLengthMismatch {
        bytes_written: u64,
        logical_size: u64,
    },

    #[error("stream SHA-256 does not match the expected content hash")]
    HashMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },

    #[error("destination root handle does not identify a directory")]
    DestinationRootNotDirectory,

    #[error("destination root identity changed on the retained handle")]
    DestinationRootIdentityChanged,

    #[error("restore job ID must contain 1..={maximum} scalar values")]
    InvalidJobId { maximum: usize },

    #[error("restore job has {actual} items; expected 1..={maximum}")]
    InvalidJobItemCount { actual: usize, maximum: usize },

    #[error("restore job has {actual} sanitized path components; maximum is {maximum}")]
    JobPathComponentLimit { actual: usize, maximum: usize },

    #[error("restore job retains {actual} path-evidence bytes; maximum is {maximum}")]
    JobPathEvidenceLimit { actual: usize, maximum: usize },

    #[error("destination {operation} failed ({kind:?}): {message}")]
    DestinationIo {
        operation: &'static str,
        kind: std::io::ErrorKind,
        message: String,
    },

    #[error("temporary capability did not identify a regular file")]
    TemporaryNotRegularFile,

    #[error("temporary capability identity changed before publication")]
    TemporaryIdentityChanged,

    #[error("temporary file length {actual} does not match expected length {expected}")]
    TemporaryLengthMismatch { expected: u64, actual: u64 },

    #[error("collision retry limit of {maximum} was exhausted")]
    CollisionLimitExceeded { maximum: usize },

    #[error("hard-link publication failed ({kind:?}): {message}")]
    HardLinkPublication {
        kind: std::io::ErrorKind,
        message: String,
    },

    #[error("namespace durability is unconfirmed and requires reconciliation: {message}")]
    NeedsReconciliation { message: String },

    #[error("journal {operation} failed ({kind:?}): {message}")]
    JournalIo {
        operation: &'static str,
        kind: std::io::ErrorKind,
        message: String,
    },

    #[error("journal serialization failed: {message}")]
    JournalSerialization { message: String },

    #[error("journal payload is {actual} bytes; maximum is {maximum}")]
    JournalPayloadTooLarge { actual: usize, maximum: usize },

    #[error("journal record is {actual} bytes; maximum is {maximum}")]
    JournalRecordTooLarge { actual: usize, maximum: usize },

    #[error("journal record count reached its limit of {maximum}")]
    JournalRecordLimit { maximum: u64 },

    #[error("journal sequence overflow at {sequence}")]
    JournalSequenceOverflow { sequence: u64 },

    #[error("journal is poisoned after a durability-boundary failure")]
    JournalPoisoned,

    #[error("journal audit failed: {message}")]
    JournalAudit { message: String },

    #[error("manifest serialization failed: {message}")]
    ManifestSerialization { message: String },
}
