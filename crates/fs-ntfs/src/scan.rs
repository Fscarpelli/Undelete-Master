use std::collections::HashMap;

use um_core::{
    Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
    MetadataConfidence, SourceReader, Timestamps,
};
use um_fs_common::{le, AllocationMap, ScanError};

use crate::attr::{
    extract_data_streams, iter_attributes, parse_file_name, parse_standard_information, AttrBody,
    DataStream, DataStreamKind, FileNameAttr, StandardInformation, ATTR_ATTRIBUTE_LIST,
    ATTR_FLAG_COMPRESSED, ATTR_FLAG_ENCRYPTED, NS_DOS,
};
use crate::boot::NtfsBoot;
use crate::record::{parse_file_record, FileRecord, RecordParseError};
use crate::runs::RunElement;

const ROOT_RECORD: u64 = 5;
const BITMAP_RECORD: u64 = 6;
const FIRST_USER_RECORD: u64 = 16;
const MAX_PATH_DEPTH: usize = 255;

/// Output of an NTFS metadata scan.
#[derive(Debug)]
pub struct NtfsScanOutput {
    pub boot: NtfsBoot,
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<String>,
}

struct ParsedEntry {
    sequence: u16,
    in_use: bool,
    is_directory: bool,
    best_name: Option<FileNameAttr>,
    std_info: StandardInformation,
    streams: Vec<DataStream>,
    /// Physical byte offset of the record within the volume region.
    record_phys_offset: u64,
    warnings: Vec<String>,
}

/// Reads a logical byte range of a non-resident stream by mapping through runs.
fn read_stream_range(
    reader: &dyn SourceReader,
    runs: &[RunElement],
    cluster_size: u64,
    logical_offset: u64,
    len: u64,
) -> Result<Vec<u8>, ScanError> {
    let mut out = vec![0u8; len as usize];
    let mut filled = 0u64;
    let mut stream_pos = 0u64;
    for run in runs {
        let run_len = run.cluster_count * cluster_size;
        let run_start = stream_pos;
        let run_end = stream_pos + run_len;
        stream_pos = run_end;
        let want_start = logical_offset + filled;
        if want_start >= run_end || filled >= len {
            continue;
        }
        if want_start < run_start {
            return Err(ScanError::Corrupt("non-contiguous stream mapping".into()));
        }
        let within = want_start - run_start;
        let take = (run_len - within).min(len - filled);
        match run.lcn {
            Some(lcn) => {
                let phys = lcn
                    .checked_mul(cluster_size)
                    .and_then(|v| v.checked_add(within))
                    .ok_or_else(|| ScanError::Corrupt("stream offset overflow".into()))?;
                reader.read_exact_at(phys, &mut out[filled as usize..(filled + take) as usize])?;
            }
            None => { /* sparse: already zero */ }
        }
        filled += take;
        if filled >= len {
            break;
        }
    }
    if filled < len {
        return Err(ScanError::Corrupt(format!(
            "stream ended early: wanted {len}, got {filled}"
        )));
    }
    Ok(out)
}

/// Maps a logical stream offset to a physical offset (for record addressing).
fn stream_phys_offset(runs: &[RunElement], cluster_size: u64, logical_offset: u64) -> Option<u64> {
    let mut stream_pos = 0u64;
    for run in runs {
        let run_len = run.cluster_count.checked_mul(cluster_size)?;
        if logical_offset < stream_pos + run_len {
            let within = logical_offset - stream_pos;
            return run.lcn?.checked_mul(cluster_size)?.checked_add(within);
        }
        stream_pos += run_len;
    }
    None
}

/// Parses a resident `$ATTRIBUTE_LIST` value, returning referenced extension
/// record numbers (excluding the base record itself).
fn attribute_list_extensions(value: &[u8], base_record: u64) -> Vec<u64> {
    let mut refs = Vec::new();
    let mut pos = 0usize;
    for _ in 0..1024 {
        if pos + 26 > value.len() {
            break;
        }
        let entry_len = le::u16_at(value, pos + 4).unwrap_or(0) as usize;
        if entry_len < 26 || pos + entry_len > value.len() {
            break;
        }
        let file_ref = le::u64_at(value, pos + 16).unwrap_or(0) & 0x0000_FFFF_FFFF_FFFF;
        if file_ref != base_record && !refs.contains(&file_ref) {
            refs.push(file_ref);
        }
        pos += entry_len;
    }
    refs
}

fn pick_best_name(names: &[FileNameAttr]) -> Option<FileNameAttr> {
    names
        .iter()
        .min_by_key(|n| if n.namespace == NS_DOS { 1u8 } else { 0u8 })
        .cloned()
}

/// Scans an NTFS volume region for deleted (and orphaned) candidates.
pub fn scan_ntfs(reader: &dyn SourceReader) -> Result<NtfsScanOutput, ScanError> {
    let mut warnings = Vec::new();
    let sector0 = reader.read_vec_at(0, 512)?;
    let boot = NtfsBoot::parse(&sector0, reader.len())?;
    let cluster_size = boot.cluster_size;
    let record_size = boot.file_record_size as u64;

    // Bootstrap: record 0 describes the MFT itself.
    let mft_start = boot.mft_lcn * cluster_size;
    let rec0_raw = reader.read_vec_at(mft_start, record_size as usize)?;
    let rec0 = parse_file_record(&rec0_raw, boot.bytes_per_sector)
        .map_err(|e| ScanError::Corrupt(format!("MFT record 0 unusable: {e:?}")))?;
    let attrs0 = iter_attributes(&rec0);
    let mut w0 = Vec::new();
    let streams0 = extract_data_streams(&attrs0, boot.total_clusters, &mut w0);
    warnings.extend(w0);
    let (mft_runs, mft_size) = match streams0.iter().find(|s| s.name.is_none()) {
        Some(DataStream {
            kind: DataStreamKind::NonResident {
                data_size, runs, ..
            },
            ..
        }) => (runs.clone(), *data_size),
        _ => {
            return Err(ScanError::Corrupt(
                "MFT record 0 has no usable non-resident $DATA".into(),
            ))
        }
    };
    let record_count = mft_size / record_size;

    // Allocation bitmap from record 6 ($Bitmap).
    let allocation = load_allocation_map(reader, &boot, &mft_runs, &mut warnings);

    // Pass 1: parse all records.
    let mut entries: HashMap<u64, ParsedEntry> = HashMap::new();
    let mut extension_data: HashMap<u64, Vec<u64>> = HashMap::new(); // base -> ext record nos
    for record_no in 0..record_count {
        let logical = record_no * record_size;
        let raw = match read_stream_range(reader, &mft_runs, cluster_size, logical, record_size) {
            Ok(b) => b,
            Err(_) => {
                warnings.push(format!("MFT record {record_no} unreadable; skipped"));
                continue;
            }
        };
        let rec = match parse_file_record(&raw, boot.bytes_per_sector) {
            Ok(r) => r,
            Err(RecordParseError::NotAFileRecord) => continue,
            Err(RecordParseError::FixupMismatch) => {
                warnings.push(format!(
                    "MFT record {record_no} has torn sectors (fixup mismatch); skipped"
                ));
                continue;
            }
            Err(RecordParseError::Corrupt(msg)) => {
                warnings.push(format!("MFT record {record_no} corrupt: {msg}"));
                continue;
            }
        };
        if rec.base_record != 0 {
            // Extension record: remember for its base.
            extension_data
                .entry(rec.base_record)
                .or_default()
                .push(record_no);
            continue;
        }
        let phys = stream_phys_offset(&mft_runs, cluster_size, logical).unwrap_or(0);
        entries.insert(record_no, parse_entry(&rec, &boot, phys));
    }

    // Pass 2: merge $DATA streams from extension records referenced by
    // resident attribute lists.
    let mut merged: Vec<(u64, Vec<DataStream>)> = Vec::new();
    for (&record_no, entry) in &entries {
        if !entry
            .warnings
            .iter()
            .any(|w| w.starts_with("has attribute list"))
        {
            continue;
        }
        let mut extra = Vec::new();
        if let Some(ext_recs) = extension_data.get(&record_no) {
            for &ext_no in ext_recs {
                let logical = ext_no * record_size;
                if let Ok(raw) =
                    read_stream_range(reader, &mft_runs, cluster_size, logical, record_size)
                {
                    if let Ok(rec) = parse_file_record(&raw, boot.bytes_per_sector) {
                        let attrs = iter_attributes(&rec);
                        let mut w = Vec::new();
                        extra.extend(extract_data_streams(&attrs, boot.total_clusters, &mut w));
                    }
                }
            }
        }
        if !extra.is_empty() {
            merged.push((record_no, extra));
        }
    }
    for (record_no, extra) in merged {
        if let Some(e) = entries.get_mut(&record_no) {
            e.streams.extend(extra);
        }
    }

    // Pass 3: build candidates from records marked free.
    let mut candidates = Vec::new();
    for (&record_no, entry) in &entries {
        if entry.in_use || record_no < FIRST_USER_RECORD {
            continue;
        }
        let Some(name_attr) = &entry.best_name else {
            continue; // No recoverable name: nothing meaningful to show yet.
        };
        let (parent_path, confidence, mut cand_warnings) =
            reconstruct_path(&entries, name_attr, record_no);
        cand_warnings.extend(entry.warnings.iter().cloned());

        let kind = if entry.is_directory {
            CandidateKind::Directory
        } else {
            CandidateKind::File
        };

        let (size, extents) = if entry.is_directory {
            (0u64, Vec::new())
        } else {
            build_extents(entry, &boot, allocation.as_ref(), &mut cand_warnings)
        };

        let state = derive_state(kind, size, &extents);
        candidates.push(Candidate {
            id: record_no,
            kind,
            method: DiscoveryMethod::NtfsMetadata,
            state,
            name: name_attr.name.clone(),
            name_certain: true,
            parent_path,
            metadata_confidence: confidence,
            size,
            timestamps: Timestamps {
                created_ms: entry.std_info.created_ms,
                modified_ms: entry.std_info.modified_ms,
                accessed_ms: entry.std_info.accessed_ms,
                deleted_ms: None,
            },
            extents,
            record_ref: record_no,
            sequence: Some(entry.sequence),
            warnings: cand_warnings,
        });
    }
    candidates.sort_by_key(|c| c.id);

    Ok(NtfsScanOutput {
        boot,
        candidates,
        warnings,
    })
}

fn parse_entry(rec: &FileRecord, boot: &NtfsBoot, record_phys_offset: u64) -> ParsedEntry {
    let attrs = iter_attributes(rec);
    let mut warnings = Vec::new();
    let mut names = Vec::new();
    let mut std_info = StandardInformation::default();
    for a in &attrs {
        match (a.type_id, &a.body) {
            (crate::attr::ATTR_STANDARD_INFORMATION, AttrBody::Resident { value, .. }) => {
                if let Some(si) = parse_standard_information(value) {
                    std_info = si;
                }
            }
            (crate::attr::ATTR_FILE_NAME, AttrBody::Resident { value, .. }) => {
                if let Some(fn_attr) = parse_file_name(value) {
                    names.push(fn_attr);
                }
            }
            (ATTR_ATTRIBUTE_LIST, AttrBody::Resident { value, .. }) => {
                let exts = attribute_list_extensions(value, 0);
                warnings.push(format!(
                    "has attribute list with {} extension reference(s)",
                    exts.len()
                ));
            }
            (ATTR_ATTRIBUTE_LIST, AttrBody::NonResident { .. }) => {
                warnings
                    .push("has non-resident attribute list; extents may be incomplete".to_string());
            }
            _ => {}
        }
    }
    let streams = extract_data_streams(&attrs, boot.total_clusters, &mut warnings);
    ParsedEntry {
        sequence: rec.sequence,
        in_use: rec.in_use,
        is_directory: rec.is_directory,
        best_name: pick_best_name(&names),
        std_info,
        streams,
        record_phys_offset,
        warnings,
    }
}

fn load_allocation_map(
    reader: &dyn SourceReader,
    boot: &NtfsBoot,
    mft_runs: &[RunElement],
    warnings: &mut Vec<String>,
) -> Option<AllocationMap> {
    let record_size = boot.file_record_size as u64;
    let logical = BITMAP_RECORD * record_size;
    let raw = read_stream_range(reader, mft_runs, boot.cluster_size, logical, record_size).ok()?;
    let rec = match parse_file_record(&raw, boot.bytes_per_sector) {
        Ok(r) => r,
        Err(_) => {
            warnings.push("$Bitmap record unusable; extent availability unknown".into());
            return None;
        }
    };
    let attrs = iter_attributes(&rec);
    let mut w = Vec::new();
    let streams = extract_data_streams(&attrs, boot.total_clusters, &mut w);
    let expected = boot.total_clusters.div_ceil(8);
    match streams.iter().find(|s| s.name.is_none()) {
        Some(DataStream {
            kind: DataStreamKind::NonResident {
                data_size, runs, ..
            },
            ..
        }) => {
            let take = expected.min(*data_size);
            match read_stream_range(reader, runs, boot.cluster_size, 0, take) {
                Ok(bits) => Some(allocation_map_from_bytes(
                    bits,
                    expected,
                    boot.total_clusters,
                    warnings,
                )),
                Err(_) => {
                    warnings.push("$Bitmap data unreadable; extent availability unknown".into());
                    None
                }
            }
        }
        Some(DataStream {
            kind:
                DataStreamKind::Resident {
                    value_offset_in_record,
                    len,
                },
            ..
        }) => {
            let phys = stream_phys_offset(mft_runs, boot.cluster_size, logical)?
                + *value_offset_in_record as u64;
            let take = expected.min(*len) as usize;
            match reader.read_vec_at(phys, take) {
                Ok(bits) => Some(allocation_map_from_bytes(
                    bits,
                    expected,
                    boot.total_clusters,
                    warnings,
                )),
                Err(_) => None,
            }
        }
        None => {
            warnings.push("$Bitmap has no data stream; extent availability unknown".into());
            None
        }
    }
}

fn allocation_map_from_bytes(
    bits: Vec<u8>,
    expected_bytes: u64,
    total_clusters: u64,
    warnings: &mut Vec<String>,
) -> AllocationMap {
    if (bits.len() as u64) < expected_bytes {
        warnings.push(format!(
            "$Bitmap data truncated: expected {expected_bytes} bytes, read {}",
            bits.len()
        ));
    }
    AllocationMap::from_raw(bits, total_clusters)
}

/// Walks parent references to rebuild the original path.
fn reconstruct_path(
    entries: &HashMap<u64, ParsedEntry>,
    name_attr: &FileNameAttr,
    self_record: u64,
) -> (Vec<String>, MetadataConfidence, Vec<String>) {
    let mut parts_rev: Vec<String> = Vec::new();
    let mut warnings = Vec::new();
    let mut confidence = MetadataConfidence::High;
    let mut visited = std::collections::HashSet::new();
    visited.insert(self_record);

    let mut parent_no = name_attr.parent_record;
    let mut parent_seq = name_attr.parent_sequence;

    for _ in 0..MAX_PATH_DEPTH {
        if parent_no == ROOT_RECORD {
            parts_rev.reverse();
            return (parts_rev, confidence, warnings);
        }
        if !visited.insert(parent_no) {
            warnings.push("path reconstruction hit a cycle; treating as orphan".into());
            return (orphan_path(self_record), MetadataConfidence::Low, warnings);
        }
        let Some(parent) = entries.get(&parent_no) else {
            warnings.push(format!(
                "parent record {parent_no} not found; treating as orphan"
            ));
            return (orphan_path(self_record), MetadataConfidence::Low, warnings);
        };
        // Sequence check: an exact match means the same generation; a deleted
        // parent whose sequence was bumped on free (+1) is still the same
        // directory, with reduced confidence.
        let seq_ok = parent.sequence == parent_seq;
        let seq_deleted_ok = !parent.in_use && parent.sequence == parent_seq.wrapping_add(1);
        if !seq_ok && !seq_deleted_ok {
            warnings.push(format!(
                "parent record {parent_no} was reused (expected seq {parent_seq}, found {}); path unreliable",
                parent.sequence
            ));
            return (orphan_path(self_record), MetadataConfidence::Low, warnings);
        }
        if !parent.in_use && confidence == MetadataConfidence::High {
            confidence = MetadataConfidence::Medium;
            warnings.push(format!(
                "ancestor directory (record {parent_no}) is deleted"
            ));
        }
        let Some(parent_name) = &parent.best_name else {
            warnings.push(format!(
                "parent record {parent_no} has no recoverable name; treating as orphan"
            ));
            return (orphan_path(self_record), MetadataConfidence::Low, warnings);
        };
        parts_rev.push(parent_name.name.clone());
        parent_no = parent_name.parent_record;
        parent_seq = parent_name.parent_sequence;
    }
    warnings.push("path deeper than limit; treating as orphan".into());
    (orphan_path(self_record), MetadataConfidence::Low, warnings)
}

fn orphan_path(record_no: u64) -> Vec<String> {
    vec![format!("Orphaned items/MFT record {record_no}")]
}

/// Builds availability-classified extents for the unnamed data stream.
fn build_extents(
    entry: &ParsedEntry,
    boot: &NtfsBoot,
    allocation: Option<&AllocationMap>,
    warnings: &mut Vec<String>,
) -> (u64, Vec<ExtentRun>) {
    let ads_count = entry.streams.iter().filter(|s| s.name.is_some()).count();
    if ads_count > 0 {
        warnings.push(format!(
            "file has {ads_count} alternate data stream(s); only the main stream is recovered"
        ));
    }
    let Some(main) = entry.streams.iter().find(|s| s.name.is_none()) else {
        return (0, Vec::new());
    };
    if main.flags & ATTR_FLAG_ENCRYPTED != 0 {
        warnings.push(
            "content is EFS-encrypted; bytes are recoverable but unusable without the key".into(),
        );
    }
    if main.flags & ATTR_FLAG_COMPRESSED != 0 {
        warnings.push(
            "content is NTFS-compressed; transparent decompression is not yet supported".into(),
        );
        // Compressed runs cannot be materialized byte-for-byte without
        // LZNT1 decompression, so report metadata only.
        let size = match &main.kind {
            DataStreamKind::NonResident { data_size, .. } => *data_size,
            DataStreamKind::Resident { len, .. } => *len,
        };
        return (size, Vec::new());
    }

    match &main.kind {
        DataStreamKind::Resident {
            value_offset_in_record,
            len,
        } => {
            let phys = entry.record_phys_offset + *value_offset_in_record as u64;
            (
                *len,
                vec![ExtentRun {
                    logical_offset: 0,
                    physical_offset: Some(phys),
                    len: *len,
                    availability: ExtentAvailability::Resident,
                }],
            )
        }
        DataStreamKind::NonResident {
            data_size,
            initialized_size,
            runs,
        } => {
            let mut extents = Vec::new();
            let mut logical = 0u64;
            for run in runs {
                if logical >= *data_size {
                    break;
                }
                let run_bytes = run.cluster_count * boot.cluster_size;
                let take = run_bytes.min(*data_size - logical);
                match run.lcn {
                    None => extents.push(ExtentRun {
                        logical_offset: logical,
                        physical_offset: None,
                        len: take,
                        availability: ExtentAvailability::Sparse,
                    }),
                    Some(lcn) => {
                        split_by_allocation(
                            &mut extents,
                            logical,
                            lcn,
                            take,
                            boot.cluster_size,
                            allocation,
                        );
                    }
                }
                logical += take;
            }
            let extents = split_at_initialized_size(extents, *initialized_size, *data_size);
            (*data_size, extents)
        }
    }
}

/// Clips extents to the logical data size and represents every byte after the
/// initialized boundary as a logical sparse range.
fn split_at_initialized_size(
    extents: Vec<ExtentRun>,
    initialized_size: u64,
    data_size: u64,
) -> Vec<ExtentRun> {
    let initialized_end = initialized_size.min(data_size);
    let mut split = Vec::with_capacity(extents.len().saturating_add(1));

    for extent in extents {
        let start = extent.logical_offset;
        let end = start.saturating_add(extent.len).min(data_size);
        if start >= end {
            continue;
        }

        let initialized_extent_end = end.min(initialized_end);
        if start < initialized_extent_end {
            let mut initialized = extent.clone();
            initialized.len = initialized_extent_end - start;
            split.push(initialized);
        }

        let tail_start = start.max(initialized_end);
        if tail_start < end {
            split.push(ExtentRun {
                logical_offset: tail_start,
                physical_offset: None,
                len: end - tail_start,
                availability: ExtentAvailability::Sparse,
            });
        }
    }

    split
}

/// Splits a physical run into extents grouped by cluster-allocation status.
fn split_by_allocation(
    extents: &mut Vec<ExtentRun>,
    logical_start: u64,
    lcn: u64,
    byte_len: u64,
    cluster_size: u64,
    allocation: Option<&AllocationMap>,
) {
    let Some(map) = allocation else {
        extents.push(ExtentRun {
            logical_offset: logical_start,
            physical_offset: Some(lcn * cluster_size),
            len: byte_len,
            availability: ExtentAvailability::Unknown,
        });
        return;
    };
    let cluster_count = byte_len.div_ceil(cluster_size);
    let mut group_start = 0u64;
    let mut group_status = map.is_allocated(lcn);
    for i in 1..=cluster_count {
        let status = if i < cluster_count {
            map.is_allocated(lcn + i)
        } else {
            None // force flush of the last group
        };
        if i == cluster_count || status != group_status {
            let start_byte = group_start * cluster_size;
            let end_byte = (i * cluster_size).min(byte_len);
            let availability = match group_status {
                Some(true) => ExtentAvailability::CurrentlyAllocated,
                Some(false) => ExtentAvailability::FreeInSnapshot,
                None => ExtentAvailability::Unknown,
            };
            extents.push(ExtentRun {
                logical_offset: logical_start + start_byte,
                physical_offset: Some((lcn + group_start) * cluster_size),
                len: end_byte - start_byte,
                availability,
            });
            group_start = i;
            group_status = status;
        }
    }
}

fn derive_state(kind: CandidateKind, size: u64, extents: &[ExtentRun]) -> CandidateState {
    if kind == CandidateKind::Directory {
        return CandidateState::ExactEvidence;
    }
    if size == 0 {
        return CandidateState::CompleteUnvalidated;
    }
    if extents.is_empty() {
        return CandidateState::MetadataOnly;
    }
    let covered: u64 = extents.iter().map(|e| e.len).sum();
    let any_conflict = extents
        .iter()
        .any(|e| e.availability == ExtentAvailability::CurrentlyAllocated);
    let any_read_error = extents
        .iter()
        .any(|e| e.availability == ExtentAvailability::ReadFailed);
    if any_read_error {
        CandidateState::ReadErrorState
    } else if any_conflict {
        CandidateState::Conflicted
    } else if covered < size {
        CandidateState::Partial
    } else {
        CandidateState::CompleteUnvalidated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use um_core::{extract_candidate, MetadataConfidence, Timestamps};
    use um_io_common::MemImageReader;

    #[test]
    fn ntfs_bitmap_trunc_001_warns_and_keeps_missing_bits_unknown() {
        let mut warnings = Vec::new();

        let map = allocation_map_from_bytes(vec![0xFF], 2, 16, &mut warnings);

        assert!(warnings
            .iter()
            .any(|warning| warning.contains("$Bitmap data truncated")));
        assert_eq!(map.is_allocated(7), Some(true));
        assert_eq!(map.is_allocated(8), None);
    }

    #[test]
    fn ntfs_init_tail_001_splits_and_zero_fills_uninitialized_tail() {
        let extents = vec![ExtentRun {
            logical_offset: 0,
            physical_offset: Some(0),
            len: 8192,
            availability: ExtentAvailability::FreeInSnapshot,
        }];

        let extents = split_at_initialized_size(extents, 4608, 8192);

        assert_eq!(
            extents,
            vec![
                ExtentRun {
                    logical_offset: 0,
                    physical_offset: Some(0),
                    len: 4608,
                    availability: ExtentAvailability::FreeInSnapshot,
                },
                ExtentRun {
                    logical_offset: 4608,
                    physical_offset: None,
                    len: 3584,
                    availability: ExtentAvailability::Sparse,
                },
            ]
        );

        let mut source = vec![0xAA; 4608];
        source.extend(vec![0xCC; 3584]);
        let reader = MemImageReader::new("initialized-tail", source);
        let candidate = Candidate {
            id: 1,
            kind: CandidateKind::File,
            method: DiscoveryMethod::NtfsMetadata,
            state: CandidateState::CompleteUnvalidated,
            name: "tail.bin".into(),
            name_certain: true,
            parent_path: Vec::new(),
            metadata_confidence: MetadataConfidence::High,
            size: 8192,
            timestamps: Timestamps::default(),
            extents,
            record_ref: 1,
            sequence: Some(1),
            warnings: Vec::new(),
        };

        let extraction = extract_candidate(&reader, &candidate).unwrap();
        assert!(extraction.bytes[..4608].iter().all(|&byte| byte == 0xAA));
        assert!(extraction.bytes[4608..].iter().all(|&byte| byte == 0));
        assert!(extraction.missing_ranges.is_empty());
    }
}
