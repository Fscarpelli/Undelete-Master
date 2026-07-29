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
const MAX_DIR_BYTES: u64 = 8 * 1024 * 1024;

/// Output of a FAT metadata scan.
#[derive(Debug)]
pub struct FatScanOutput {
    pub boot: FatBoot,
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<String>,
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
}

/// Scans a FAT12/16/32 volume region for deleted candidates.
pub fn scan_fat(reader: &dyn SourceReader) -> Result<FatScanOutput, ScanError> {
    let sector0 = reader.read_vec_at(0, 512)?;
    let boot = FatBoot::parse(&sector0, reader.len())?;

    let fat_bytes = boot.fat_size_sectors as u64 * boot.bytes_per_sector as u64;
    let raw_fat = reader.read_vec_at(boot.fat_offset, fat_bytes as usize)?;
    let fat = FatTable::from_boot(&boot, raw_fat);

    // Compare FAT copies and warn on divergence (first copy remains authoritative).
    let mut warnings = Vec::new();
    if boot.num_fats > 1 {
        let second = reader.read_vec_at(boot.fat_offset + fat_bytes, fat_bytes as usize)?;
        let first = reader.read_vec_at(boot.fat_offset, fat_bytes as usize)?;
        if first != second {
            warnings.push("FAT copies disagree; using the first copy".into());
        }
    }

    let mut ctx = ScanCtx {
        reader,
        boot,
        fat,
        candidates: Vec::new(),
        warnings,
        visited_dir_clusters: HashSet::new(),
        dirs_processed: 0,
        next_id: 1,
    };

    // Root directory.
    let root_data = read_root_dir(&mut ctx)?;
    walk_directory(&mut ctx, &root_data, &[], true);

    let ScanCtx {
        boot,
        mut candidates,
        warnings,
        ..
    } = ctx;
    candidates.sort_by_key(|c| c.record_ref);
    Ok(FatScanOutput {
        boot,
        candidates,
        warnings,
    })
}

fn read_root_dir(ctx: &mut ScanCtx<'_>) -> Result<Vec<u8>, ScanError> {
    match ctx.boot.variant {
        FatVariant::Fat12 | FatVariant::Fat16 => Ok(ctx
            .reader
            .read_vec_at(ctx.boot.root_dir_offset, ctx.boot.root_dir_bytes as usize)?),
        FatVariant::Fat32 => {
            let (clusters, complete) = ctx
                .fat
                .chain(ctx.boot.root_cluster, ctx.boot.cluster_count);
            if !complete {
                ctx.warnings
                    .push("FAT32 root directory chain incomplete".into());
            }
            read_clusters(ctx, &clusters)
        }
    }
}

fn read_clusters(ctx: &ScanCtx<'_>, clusters: &[u32]) -> Result<Vec<u8>, ScanError> {
    let mut out = Vec::new();
    for &c in clusters {
        if out.len() as u64 > MAX_DIR_BYTES {
            break;
        }
        let Some(off) = ctx.boot.cluster_offset(c as u64) else {
            continue;
        };
        out.extend(ctx.reader.read_vec_at(off, ctx.boot.cluster_size as usize)?);
    }
    Ok(out)
}

/// Recursively walks a directory stream, emitting candidates for deleted
/// entries and descending into both active and deleted subdirectories.
fn walk_directory(ctx: &mut ScanCtx<'_>, data: &[u8], path: &[String], parent_active: bool) {
    if ctx.dirs_processed >= MAX_DIRS {
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

fn handle_directory(
    ctx: &mut ScanCtx<'_>,
    entry: &DirEntry,
    path: &[String],
    parent_active: bool,
) {
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
        return;
    }
    if ctx.visited_dir_clusters.contains(&entry.first_cluster) {
        ctx.warnings.push(format!(
            "directory cycle at cluster {}; skipped",
            entry.first_cluster
        ));
        return;
    }

    let clusters: Vec<u32> = if !entry.is_deleted {
        let (chain, complete) = ctx.fat.chain(entry.first_cluster, ctx.boot.cluster_count);
        if !complete {
            ctx.warnings.push(format!(
                "directory '{}' has a broken cluster chain",
                entry.name
            ));
        }
        chain
    } else {
        match ctx.fat.entry(entry.first_cluster) {
            Some(FatEntry::Free) => vec![entry.first_cluster],
            Some(FatEntry::Next(_)) | Some(FatEntry::EndOfChain) => {
                // Chain unexpectedly retained: follow it.
                ctx.fat
                    .chain(entry.first_cluster, ctx.boot.cluster_count)
                    .0
            }
            _ => return,
        }
    };
    for &c in &clusters {
        ctx.visited_dir_clusters.insert(c);
    }
    match read_clusters(ctx, &clusters) {
        Ok(data) => {
            walk_directory(ctx, &data, &child_path, parent_active && !entry.is_deleted)
        }
        Err(_) => ctx
            .warnings
            .push(format!("directory '{}' unreadable", entry.name)),
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
    } else if entry.name_certain {
        MetadataConfidence::Medium
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
