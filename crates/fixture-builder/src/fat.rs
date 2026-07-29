//! Deterministic FAT12/16/32 volume image builder (see `Fat32ImageBuilder`).
//!
//! Implemented in this module: FAT32 with deleted entries, LFN chains and
//! cleared cluster chains. FAT12/16 share the directory-entry writer.
