use serde::{Deserialize, Serialize};

/// Truth manifest describing what a fixture contains and what a correct
/// scanner must find.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureManifest {
    pub fixture_id: String,
    pub filesystem: String,
    pub sector_size: u32,
    pub cluster_size: u32,
    pub expected_candidates: Vec<ExpectedCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedCandidate {
    pub name: String,
    /// Path components of the parent, root-first.
    pub parent_path: Vec<String>,
    pub is_directory: bool,
    pub size: u64,
    /// SHA-256 of the full original content (empty string for directories).
    pub content_sha256: String,
    /// True when full byte-exact recovery is expected.
    pub fully_recoverable: bool,
    /// Logical ranges expected to be missing/overwritten `(offset, len)`.
    pub damaged_ranges: Vec<(u64, u64)>,
}
