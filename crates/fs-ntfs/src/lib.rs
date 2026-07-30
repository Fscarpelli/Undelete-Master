//! Raw NTFS metadata scanner: parses the boot sector, walks the MFT
//! (including records marked free), reconstructs names/paths/extents and
//! classifies extent availability against `$Bitmap`.
//!
//! The scanner is strictly read-only and OS-independent: it operates on any
//! [`um_core::SourceReader`] scoped to an NTFS volume region.

#![forbid(unsafe_code)]

pub mod attr;
pub mod boot;
pub mod record;
pub mod runs;
mod scan;

pub use boot::NtfsBoot;
pub use scan::{
    scan_ntfs, NtfsCandidatePathEvidence, NtfsDirectoryNode, NtfsDirectoryResolution,
    NtfsNamespace, NtfsNamespaceIndex, NtfsNamespaceIndexError, NtfsNamespacePath, NtfsNodeRef,
    NtfsPathState, NtfsScanOutput, NtfsScopeMembership,
};
