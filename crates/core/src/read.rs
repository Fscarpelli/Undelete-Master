use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors surfaced by read-only source access.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadError {
    #[error("read out of bounds: offset {offset} len {len} source size {source_len}")]
    OutOfBounds {
        offset: u64,
        len: u64,
        source_len: u64,
    },
    #[error("I/O failure at offset {offset}: {message}")]
    Io { offset: u64, message: String },
    #[error("source disappeared or identity changed")]
    SourceGone,
}

/// Result of a best-effort read: how much was read and which sub-ranges failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOutcome {
    /// Bytes actually placed into the buffer (unreadable ranges are zero-filled).
    pub bytes_valid: u64,
    /// Failed sub-ranges relative to the requested offset.
    pub bad_ranges: Vec<(u64, u64)>,
}

impl ReadOutcome {
    pub fn complete(len: u64) -> Self {
        Self {
            bytes_valid: len,
            bad_ranges: Vec::new(),
        }
    }

    pub fn is_complete(&self) -> bool {
        self.bad_ranges.is_empty()
    }
}
