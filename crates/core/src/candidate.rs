use serde::{Deserialize, Serialize};

pub type CandidateId = u64;

/// How a candidate was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    NtfsMetadata,
    FatMetadata,
    ExfatMetadata,
    Carving,
    RecycleBin,
}

/// File vs directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateKind {
    File,
    Directory,
}

/// Minimum candidate states required by the master spec (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateState {
    ExactEvidence,
    LikelyComplete,
    CompleteUnvalidated,
    StructurallyValid,
    Partial,
    Conflicted,
    ReadErrorState,
    ZeroedOrTrimmed,
    Overwritten,
    MetadataOnly,
    Unknown,
}

/// Availability classification of an extent at scan time (§11.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtentAvailability {
    /// Free in the allocation snapshot: strong evidence bytes are untouched.
    FreeInSnapshot,
    /// Currently allocated to something else: strong risk of overwrite.
    CurrentlyAllocated,
    /// Range lies outside the analyzed volume.
    OutOfVolume,
    /// Read error while accessing the range.
    ReadFailed,
    /// Observed zeroed (possible TRIM on SSDs, but never asserted from zeros alone).
    Zeroed,
    /// Resident data stored inside the metadata record itself.
    Resident,
    /// Sparse range: logically zero by design, no physical clusters.
    Sparse,
    Unknown,
}

/// One physical run of a candidate's content.
///
/// `logical_offset` is the position inside the file; `physical_offset` is the
/// absolute byte position within the analyzed region (`None` for resident or
/// sparse data that has no physical location of its own).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtentRun {
    pub logical_offset: u64,
    pub physical_offset: Option<u64>,
    pub len: u64,
    pub availability: ExtentAvailability,
}

/// Timestamps recovered from metadata (100ns FILETIME or unix epoch — the
/// scanner normalizes to unix milliseconds; `None` when not recoverable).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamps {
    pub created_ms: Option<i64>,
    pub modified_ms: Option<i64>,
    pub accessed_ms: Option<i64>,
    /// Deletion time is only present when derived from a concrete source
    /// (e.g. Recycle Bin metadata); NTFS records do not carry one (§11.10).
    pub deleted_ms: Option<i64>,
}

/// A potentially recoverable item found during a scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: CandidateId,
    pub kind: CandidateKind,
    pub method: DiscoveryMethod,
    pub state: CandidateState,
    /// Best-known name (may be partially reconstructed; see `name_certain`).
    pub name: String,
    /// True when the full original name was recovered without inference
    /// (e.g. FAT deleted entries lose the first character).
    pub name_certain: bool,
    /// Reconstructed original path components, root-first, not including the
    /// file name. Empty for orphans/carved candidates.
    pub parent_path: Vec<String>,
    pub metadata_confidence: MetadataConfidence,
    /// Logical file size in bytes (0 for directories).
    pub size: u64,
    pub timestamps: Timestamps,
    /// Content extents in logical order.
    pub extents: Vec<ExtentRun>,
    /// Filesystem-specific record reference (MFT record no, dir entry offset, carve offset).
    pub record_ref: u64,
    /// Sequence/generation info when the filesystem provides it.
    pub sequence: Option<u16>,
    /// Non-fatal findings worth surfacing (inferred chains, reused parents...).
    pub warnings: Vec<String>,
}

/// Confidence in name/path/dates metadata (§16.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataConfidence {
    High,
    Medium,
    Low,
}

impl Candidate {
    /// Total bytes covered by extents (logical content coverage).
    pub fn covered_len(&self) -> u64 {
        self.extents.iter().map(|e| e.len).sum()
    }

    /// True when some logical range of the file has no extent at all.
    pub fn has_missing_ranges(&self) -> bool {
        self.covered_len() < self.size
    }

    pub fn display_path(&self) -> String {
        let mut parts = self.parent_path.clone();
        parts.push(self.name.clone());
        parts.join("/")
    }
}
