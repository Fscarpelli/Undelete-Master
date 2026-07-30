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
}
