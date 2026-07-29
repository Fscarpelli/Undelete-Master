use serde::{Deserialize, Serialize};

/// Stable, privacy-preserving report for one regular image scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageScanReport {
    pub schema_version: u32,
    pub source: SourceReport,
    pub partition_table: Option<String>,
    pub volumes: Vec<VolumeReport>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceReport {
    /// Deliberately redacted: never contains the supplied filesystem path.
    pub id: String,
    pub label: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeReport {
    pub index: u32,
    pub offset_bytes: u64,
    pub length_bytes: u64,
    /// One of `ntfs`, `fat12`, `fat16`, `fat32`, or `unrecognized`.
    pub file_system: String,
    pub candidate_count: usize,
    pub warnings: Vec<String>,
}
