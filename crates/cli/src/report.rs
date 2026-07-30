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
    /// Whether candidate enumeration covered the complete recognized metadata
    /// region, stopped at an explicit safety bound, or found no supported
    /// filesystem.
    pub scan_status: VolumeScanStatus,
    pub candidate_count: usize,
    /// Quantitative coverage of the NTFS master file table when the recognized
    /// filesystem exposes that evidence. Other filesystems leave this absent;
    /// future deep/content coverage remains a separate concern.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mft_coverage: Option<MftScanCoverage>,
    /// Content coverage for an explicitly requested JPEG deep scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jpeg_carve_coverage: Option<JpegCarveCoverage>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MftScanCoverage {
    pub records_declared: u64,
    pub records_available: u64,
    pub records_examined: u64,
    pub bytes_declared: u64,
    pub bytes_available: u64,
    pub bytes_examined: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JpegCarveCoverage {
    pub bytes_requested: u64,
    pub bytes_scanned: u64,
    pub signatures_attempted: u64,
    pub validation_bytes_read: u64,
    pub partial: bool,
    pub read_error_count: u64,
    pub candidate_limit_reached: bool,
    pub candidate_byte_limit_hits: u64,
    pub signature_attempt_limit_reached: bool,
    pub validation_byte_limit_reached: bool,
    pub rejected_signatures: u64,
    pub truncated_signatures: u64,
    pub regions_submitted: u64,
    pub region_limit_reached: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeScanStatus {
    Complete,
    Partial,
    Unrecognized,
}
