use crate::read::{ReadError, ReadOutcome};
use serde::{Deserialize, Serialize};

/// What kind of source is being analyzed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    /// A raw image file (`.img`, `.dd`, `.raw`).
    ImageFile,
    /// A physical disk exposed by the OS (future: via elevated broker).
    PhysicalDisk,
    /// A mounted volume/partition.
    Volume,
}

/// Stable identity of a source. For image files this is derived from the
/// path and size; for devices it will combine serial/unique-id/layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceIdentity {
    pub id: String,
    pub kind: SourceKind,
    pub label: String,
    pub size: u64,
}

/// Logical/physical sector sizes of the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectorLayout {
    pub logical: u32,
    pub physical: u32,
}

impl SectorLayout {
    pub const DEFAULT_512: SectorLayout = SectorLayout {
        logical: 512,
        physical: 512,
    };

    pub const fn new(logical: u32, physical: u32) -> Option<Self> {
        if logical == 0 || physical == 0 || physical < logical {
            return None;
        }
        Some(Self { logical, physical })
    }
}

/// Read-only access to a scan source.
///
/// **Invariant:** this trait intentionally has no write, trim, lock, dismount
/// or metadata-mutation operations, and none may ever be added. See
/// `AGENTS.md` and the master spec (§1.1 rule 9).
pub trait SourceReader: Send + Sync {
    fn identity(&self) -> &SourceIdentity;

    /// Total size in bytes.
    fn len(&self) -> u64;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn sector_layout(&self) -> SectorLayout;

    /// Reads exactly `buffer.len()` bytes at `offset`, failing on any error.
    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError>;

    /// Reads as much as possible at `offset`; unreadable ranges are
    /// zero-filled and reported in the outcome.
    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome;

    /// Convenience helper: allocate and read exactly `len` bytes.
    fn read_vec_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, ReadError> {
        let mut buf = vec![0u8; len];
        self.read_exact_at(offset, &mut buf)?;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::SectorLayout;

    #[test]
    fn core_sector_valid_001_rejects_invalid_sector_relationships() {
        assert_eq!(
            SectorLayout::new(512, 4096),
            Some(SectorLayout {
                logical: 512,
                physical: 4096,
            })
        );
        assert_eq!(SectorLayout::new(0, 4096), None);
        assert_eq!(SectorLayout::new(512, 0), None);
        assert_eq!(SectorLayout::new(4096, 512), None);
    }
}
