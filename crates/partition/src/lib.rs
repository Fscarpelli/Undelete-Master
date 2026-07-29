//! MBR and GPT partition table discovery over a read-only source.
//!
//! Parsing is defensive: every offset is validated against the source size,
//! EBR chains are loop-protected and GPT header/entry CRCs are verified with
//! automatic fallback to the complete backup GPT copy.

#![forbid(unsafe_code)]

mod gpt;
mod mbr;

use serde::{Deserialize, Serialize};
use um_core::{Region, SectorLayout, SourceReader};
use um_fs_common::ScanError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionTableKind {
    Mbr,
    Gpt,
}

/// One discovered partition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionEntry {
    pub index: u32,
    /// Byte region within the source.
    pub region: Region,
    /// MBR type byte, when applicable.
    pub mbr_type: Option<u8>,
    /// GPT partition type GUID (mixed-endian textual form), when applicable.
    pub type_guid: Option<String>,
    /// GPT partition name, when applicable.
    pub name: Option<String>,
    /// Human description of the partition type.
    pub type_description: String,
}

/// The discovered partition layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionTable {
    pub kind: PartitionTableKind,
    pub partitions: Vec<PartitionEntry>,
    /// Non-fatal findings: overlaps, CRC fallbacks, out-of-bounds entries...
    pub warnings: Vec<String>,
}

/// Discovers the partition table of a source.
///
/// Returns `ScanError::NotRecognized` when no valid MBR/GPT is present; the
/// caller should then treat the whole source as a single volume region.
pub fn discover(reader: &dyn SourceReader) -> Result<PartitionTable, ScanError> {
    let supplied_layout = reader.sector_layout();
    let layout = SectorLayout::new(supplied_layout.logical, supplied_layout.physical)
        .ok_or_else(|| ScanError::Corrupt("invalid sector layout".into()))?;
    let sector = layout.logical as u64;
    let minimum_len = sector
        .checked_mul(2)
        .ok_or_else(|| ScanError::Corrupt("minimum source size overflow".into()))?;
    if reader.len() < minimum_len {
        return Err(ScanError::NotRecognized(
            "source smaller than two sectors".into(),
        ));
    }

    // GPT has priority. MBR parsing rejects a protective 0xEE entry when no
    // usable GPT copy exists, so the protective container is never scanned as
    // an ordinary partition.
    match gpt::parse_gpt(reader) {
        Ok(table) => return Ok(table),
        Err(ScanError::NotRecognized(_)) => {}
        Err(e) => return Err(e),
    }
    mbr::parse_mbr(reader)
}

pub(crate) fn check_overlaps(parts: &[PartitionEntry], warnings: &mut Vec<String>) {
    for i in 0..parts.len() {
        for j in (i + 1)..parts.len() {
            if parts[i].region.intersect(&parts[j].region).is_some() {
                warnings.push(format!(
                    "partitions {} and {} overlap",
                    parts[i].index, parts[j].index
                ));
            }
        }
    }
}
