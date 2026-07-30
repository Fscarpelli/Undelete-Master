use std::collections::HashSet;

use um_core::{
    Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
    MetadataConfidence, SourceReader, Timestamps,
};
use um_fs_common::ScanError;

use crate::boot::{FatBoot, FatVariant};
use crate::dir::{parse_directory, DirEntry};
use crate::fat::{FatEntry, FatTable};

const MAX_DIRS: usize = 10_000;
const MAX_DIR_DEPTH: usize = 256;
const MAX_DIR_BYTES: u64 = 8 * 1024 * 1024;
const MAX_FAT_READ_BYTES: u64 = 64 * 1024 * 1024;

/// Output of a FAT metadata scan.
#[derive(Debug)]
pub struct FatScanOutput {
    pub boot: FatBoot,
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<String>,
    /// True only when every reachable directory stream was enumerated without
    /// hitting a safety bound, corrupt chain, cycle, or read failure.
    pub is_complete: bool,
}

struct ScanCtx<'a> {
    reader: &'a dyn SourceReader,
    boot: FatBoot,
    fat: FatTable,
    candidates: Vec<Candidate>,
    warnings: Vec<String>,
    visited_dir_clusters: HashSet<u32>,
    dirs_processed: usize,
    next_id: u64,
    is_complete: bool,
}

/// Scans a FAT12/16/32 volume region for deleted candidates.
pub fn scan_fat(reader: &dyn SourceReader) -> Result<FatScanOutput, ScanError> {
    let sector0 = reader.read_vec_at(0, 512)?;
    let boot = FatBoot::parse(&sector0, reader.len())?;

    let fat_bytes = boot.fat_size_sectors as u64 * boot.bytes_per_sector as u64;
    if fat_bytes > MAX_FAT_READ_BYTES {
        return Err(ScanError::Corrupt(format!(
            "FAT table exceeds the {MAX_FAT_READ_BYTES}-byte scan bound"
        )));
    }
    let fat_len = usize::try_from(fat_bytes)
        .map_err(|_| ScanError::Corrupt("FAT size does not fit address space".into()))?;
    let raw_fat = reader.read_vec_at(boot.fat_offset, fat_len)?;

    // Compare every declared FAT copy and warn on divergence (the first copy
    // remains authoritative). The BPB bounds this loop to at most four copies.
    let mut warnings = Vec::new();
    let mut is_complete = true;
    for copy_index in 1..boot.num_fats {
        let copy_offset = fat_bytes
            .checked_mul(u64::from(copy_index))
            .and_then(|relative| boot.fat_offset.checked_add(relative))
            .ok_or_else(|| ScanError::Corrupt("FAT copy offset overflow".into()))?;
        match reader.read_vec_at(copy_offset, fat_len) {
            Ok(copy) if raw_fat != copy => {
                is_complete = false;
                if !warnings
                    .iter()
                    .any(|warning| warning == "FAT copies disagree; using the first copy")
                {
                    warnings.push("FAT copies disagree; using the first copy".into());
                }
            }
            Ok(_) => {}
            Err(_) => {
                is_complete = false;
                warnings.push(format!(
                    "FAT copy {} unreadable; using the first copy",
                    copy_index + 1
                ));
            }
        }
    }
    let fat = FatTable::from_boot(&boot, raw_fat);

    let mut ctx = ScanCtx {
        reader,
        boot,
        fat,
        candidates: Vec::new(),
        warnings,
        visited_dir_clusters: HashSet::new(),
        dirs_processed: 0,
        next_id: 1,
        is_complete,
    };

    // Root directory.
    let root_data = read_root_dir(&mut ctx)?;
    walk_directory(&mut ctx, &root_data, &[], true);

    let ScanCtx {
        boot,
        mut candidates,
        warnings,
        is_complete,
        ..
    } = ctx;
    candidates.sort_by_key(|c| c.record_ref);
    Ok(FatScanOutput {
        boot,
        candidates,
        warnings,
        is_complete,
    })
}

impl ScanCtx<'_> {
    fn mark_incomplete(&mut self, warning: impl Into<String>) {
        self.is_complete = false;
        let warning = warning.into();
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }
}

fn read_root_dir(ctx: &mut ScanCtx<'_>) -> Result<Vec<u8>, ScanError> {
    match ctx.boot.variant {
        FatVariant::Fat12 | FatVariant::Fat16 => Ok(ctx
            .reader
            .read_vec_at(ctx.boot.root_dir_offset, ctx.boot.root_dir_bytes as usize)?),
        FatVariant::Fat32 => {
            let chain_limit = directory_chain_limit(ctx);
            let (clusters, complete) = ctx.fat.chain(ctx.boot.root_cluster, chain_limit);
            if !complete {
                let warning = if clusters.len() as u64 >= chain_limit {
                    "FAT32 root directory chain reached the scan bound"
                } else {
                    "FAT32 root directory chain incomplete"
                };
                ctx.mark_incomplete(warning);
            }
            read_clusters(ctx, &clusters)
        }
    }
}

fn directory_chain_limit(ctx: &ScanCtx<'_>) -> u64 {
    (MAX_DIR_BYTES / ctx.boot.cluster_size)
        .max(1)
        .min(ctx.boot.cluster_count)
}

fn read_clusters(ctx: &mut ScanCtx<'_>, clusters: &[u32]) -> Result<Vec<u8>, ScanError> {
    let mut out = Vec::new();
    for &c in clusters {
        let next_len = (out.len() as u64).checked_add(ctx.boot.cluster_size);
        if next_len.is_none_or(|len| len > MAX_DIR_BYTES) {
            ctx.mark_incomplete(format!(
                "directory stream exceeded the {MAX_DIR_BYTES}-byte scan bound"
            ));
            break;
        }
        let Some(off) = ctx.boot.cluster_offset(c as u64) else {
            ctx.mark_incomplete(format!(
                "directory cluster {c} lies outside the FAT volume; skipped"
            ));
            continue;
        };
        out.extend(
            ctx.reader
                .read_vec_at(off, ctx.boot.cluster_size as usize)?,
        );
    }
    Ok(out)
}

/// Recursively walks a directory stream, emitting candidates for deleted
/// entries and descending into both active and deleted subdirectories.
fn walk_directory(ctx: &mut ScanCtx<'_>, data: &[u8], path: &[String], parent_active: bool) {
    if path.len() >= MAX_DIR_DEPTH {
        ctx.mark_incomplete(format!(
            "directory traversal stopped at the {MAX_DIR_DEPTH}-level depth bound"
        ));
        return;
    }
    if ctx.dirs_processed >= MAX_DIRS {
        ctx.mark_incomplete(format!(
            "directory traversal stopped at the {MAX_DIRS}-directory scan bound"
        ));
        return;
    }
    ctx.dirs_processed += 1;

    for entry in parse_directory(data, 0) {
        if entry.is_volume_label {
            continue;
        }
        if entry.is_directory {
            handle_directory(ctx, &entry, path, parent_active);
        } else if entry.is_deleted {
            emit_file_candidate(ctx, &entry, path, parent_active);
        }
    }
}

fn handle_directory(ctx: &mut ScanCtx<'_>, entry: &DirEntry, path: &[String], parent_active: bool) {
    let mut child_path = path.to_vec();
    child_path.push(entry.name.clone());

    if entry.is_deleted {
        // Candidate for the deleted directory itself.
        let id = ctx.next_id;
        ctx.next_id += 1;
        ctx.candidates.push(Candidate {
            id,
            kind: CandidateKind::Directory,
            method: DiscoveryMethod::FatMetadata,
            state: CandidateState::ExactEvidence,
            name: entry.name.clone(),
            name_certain: entry.name_certain,
            parent_path: path.to_vec(),
            metadata_confidence: if parent_active && entry.name_certain {
                MetadataConfidence::High
            } else {
                MetadataConfidence::Medium
            },
            size: 0,
            timestamps: Timestamps {
                created_ms: entry.created_ms,
                modified_ms: entry.modified_ms,
                accessed_ms: None,
                deleted_ms: None,
            },
            extents: Vec::new(),
            record_ref: entry.entry_offset,
            sequence: None,
            warnings: Vec::new(),
        });
    }

    // Descend. For active dirs follow the chain; for deleted dirs the chain
    // is usually cleared, so read the first cluster only while it is free.
    if entry.first_cluster < 2 {
        ctx.mark_incomplete(format!(
            "directory '{}' has no usable start cluster; skipped",
            entry.name
        ));
        return;
    }
    if ctx.visited_dir_clusters.contains(&entry.first_cluster) {
        ctx.mark_incomplete(format!(
            "directory cycle at cluster {}; skipped",
            entry.first_cluster
        ));
        return;
    }

    let clusters: Vec<u32> = if !entry.is_deleted {
        let chain_limit = directory_chain_limit(ctx);
        let (chain, complete) = ctx.fat.chain(entry.first_cluster, chain_limit);
        if !complete {
            let warning = if chain.len() as u64 >= chain_limit {
                format!("directory '{}' chain reached the scan bound", entry.name)
            } else {
                format!("directory '{}' has a broken cluster chain", entry.name)
            };
            ctx.mark_incomplete(warning);
        }
        chain
    } else {
        match ctx.fat.entry(entry.first_cluster) {
            Some(FatEntry::Free) => {
                ctx.mark_incomplete(format!(
                    "deleted directory '{}' has a cleared FAT chain; scanning only its first cluster",
                    entry.name
                ));
                vec![entry.first_cluster]
            }
            Some(FatEntry::Next(_)) | Some(FatEntry::EndOfChain) => {
                // Chain unexpectedly retained: follow it.
                let chain_limit = directory_chain_limit(ctx);
                let (chain, complete) = ctx.fat.chain(entry.first_cluster, chain_limit);
                if !complete {
                    let warning = if chain.len() as u64 >= chain_limit {
                        format!(
                            "directory '{}' retained chain reached the scan bound",
                            entry.name
                        )
                    } else {
                        format!(
                            "directory '{}' has a broken retained cluster chain",
                            entry.name
                        )
                    };
                    ctx.mark_incomplete(warning);
                }
                chain
            }
            _ => {
                ctx.mark_incomplete(format!(
                    "directory '{}' starts at an unusable cluster; skipped",
                    entry.name
                ));
                return;
            }
        }
    };
    for &c in &clusters {
        ctx.visited_dir_clusters.insert(c);
    }
    match read_clusters(ctx, &clusters) {
        Ok(data) => walk_directory(ctx, &data, &child_path, parent_active && !entry.is_deleted),
        Err(_) => ctx.mark_incomplete(format!("directory '{}' unreadable", entry.name)),
    }
}

fn emit_file_candidate(
    ctx: &mut ScanCtx<'_>,
    entry: &DirEntry,
    path: &[String],
    parent_active: bool,
) {
    let id = ctx.next_id;
    ctx.next_id += 1;
    let mut warnings = Vec::new();
    let size = entry.size as u64;

    let (extents, chain_inferred) = build_file_extents(ctx, entry, &mut warnings);

    let covered: u64 = extents.iter().map(|e| e.len).sum();
    let any_conflict = extents
        .iter()
        .any(|e| e.availability == ExtentAvailability::CurrentlyAllocated);
    let state = if size == 0 {
        CandidateState::CompleteUnvalidated
    } else if extents.is_empty() {
        CandidateState::MetadataOnly
    } else if any_conflict {
        CandidateState::Conflicted
    } else if covered < size {
        CandidateState::Partial
    } else {
        CandidateState::CompleteUnvalidated
    };

    let mut confidence = if parent_active && entry.name_certain {
        MetadataConfidence::High
    } else {
        MetadataConfidence::Medium
    };
    if !entry.name_certain && !parent_active {
        confidence = MetadataConfidence::Low;
    }
    if chain_inferred {
        warnings.push("cluster chain cleared on delete; contiguous layout assumed".into());
    }

    ctx.candidates.push(Candidate {
        id,
        kind: CandidateKind::File,
        method: DiscoveryMethod::FatMetadata,
        state,
        name: entry.name.clone(),
        name_certain: entry.name_certain,
        parent_path: path.to_vec(),
        metadata_confidence: confidence,
        size,
        timestamps: Timestamps {
            created_ms: entry.created_ms,
            modified_ms: entry.modified_ms,
            accessed_ms: None,
            deleted_ms: None,
        },
        extents,
        record_ref: entry.entry_offset,
        sequence: None,
        warnings,
    });
}

/// Builds extents for a deleted file. Returns `(extents, chain_inferred)`.
fn build_file_extents(
    ctx: &ScanCtx<'_>,
    entry: &DirEntry,
    warnings: &mut Vec<String>,
) -> (Vec<ExtentRun>, bool) {
    let size = entry.size as u64;
    if size == 0 || entry.first_cluster < 2 {
        return (Vec::new(), false);
    }
    let cluster_size = ctx.boot.cluster_size;
    let clusters_needed = size.div_ceil(cluster_size);

    let (clusters, inferred) = match ctx.fat.entry(entry.first_cluster) {
        Some(FatEntry::Next(_)) | Some(FatEntry::EndOfChain) => {
            let (chain, complete) = ctx.fat.chain(entry.first_cluster, clusters_needed + 1);
            if !complete && (chain.len() as u64) < clusters_needed {
                warnings.push("retained FAT chain is shorter than the file size".into());
            } else {
                warnings.push("FAT chain still present; following it".into());
            }
            (chain, false)
        }
        Some(FatEntry::Free) => {
            // Chain cleared: assume contiguity across free clusters.
            let mut chain = Vec::new();
            for i in 0..clusters_needed {
                let c = entry.first_cluster as u64 + i;
                if c >= ctx.boot.cluster_count + 2 {
                    break;
                }
                chain.push(c as u32);
            }
            (chain, true)
        }
        _ => return (Vec::new(), false),
    };

    // Group consecutive clusters into runs, split by allocation status.
    let mut extents: Vec<ExtentRun> = Vec::new();
    let mut logical = 0u64;
    for &c in &clusters {
        if logical >= size {
            break;
        }
        let take = cluster_size.min(size - logical);
        let free = ctx.fat.is_free(c).unwrap_or(false);
        // A retained chain means the FAT still references these clusters for
        // this very file, so non-free is not a conflict in that case.
        let availability = if free || !inferred {
            ExtentAvailability::FreeInSnapshot
        } else {
            ExtentAvailability::CurrentlyAllocated
        };
        let phys = ctx.boot.cluster_offset(c as u64);
        match (extents.last_mut(), phys) {
            (Some(last), Some(p))
                if last.availability == availability
                    && last.physical_offset.map(|lp| lp + last.len) == Some(p) =>
            {
                last.len += take;
            }
            (_, Some(p)) => extents.push(ExtentRun {
                logical_offset: logical,
                physical_offset: Some(p),
                len: take,
                availability,
            }),
            (_, None) => {}
        }
        logical += take;
    }
    (extents, inferred)
}
