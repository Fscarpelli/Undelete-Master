//! FAT12/16/32 scanner: parses the BPB, reads the FAT, walks directories
//! (including deleted entries and deleted directory trees), reconstructs
//! long names from orphaned LFN chains and recovers cluster chains —
//! following retained chains when present and falling back to a clearly
//! flagged contiguous inference when the chain was cleared on delete.

#![forbid(unsafe_code)]

pub mod boot;
pub mod dir;
pub mod fat;
mod scan;

pub use boot::{FatBoot, FatVariant};
pub use scan::{scan_fat, FatScanOutput};
