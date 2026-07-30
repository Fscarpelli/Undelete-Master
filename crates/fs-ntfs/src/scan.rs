use std::collections::{HashMap, HashSet};

use um_core::{
    Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
    MetadataConfidence, Region, SourceReader, Timestamps,
};
use um_fs_common::{le, AllocationMap, ScanError};

use crate::attr::{
    extract_data_streams, iter_attributes, parse_file_name, parse_standard_information, AttrBody,
    DataStream, DataStreamKind, FileNameAttr, StandardInformation, ATTR_ATTRIBUTE_LIST, ATTR_DATA,
    ATTR_FILE_NAME, ATTR_FLAG_COMPRESSED, ATTR_FLAG_ENCRYPTED, ATTR_FLAG_SPARSE, NS_DOS,
};
use crate::boot::NtfsBoot;
use crate::record::{parse_file_record, FileRecord, RecordParseError};
use crate::runs::{decode_runlist, RunElement};

const ROOT_RECORD: u64 = 5;
const BITMAP_RECORD: u64 = 6;
const FIRST_USER_RECORD: u64 = 16;
const MAX_PATH_DEPTH: usize = 255;
// Defense-in-depth work ceiling for hostile metadata. Records are processed in
// bounded batches and only directories/deleted base records are retained, so
// this limit no longer acts as a small prefix on ordinary Windows volumes.
const MAX_MFT_RECORDS: u64 = 8 * 1024 * 1024;
// One batch maps to at most one broker protocol request on Windows. Readers
// remain free to split it further, while the scanner never allocates an entire
// MFT or performs one IPC round-trip per ordinary record.
const MFT_BATCH_BYTES: u64 = 1024 * 1024;
// Parsing may examine millions of records, but retained metadata and product
// output must remain bounded independently of that coverage. Active regular
// files are discarded; these limits apply to the directory graph, deleted base
// records, and extension references that are actually useful to recovery.
const MAX_RETAINED_DELETED_ENTRIES: usize = 100_000;
const MAX_RETAINED_DIRECTORY_ENTRIES: usize = 100_000;
const MAX_RETAINED_EXTENSION_REFERENCES: usize = 100_000;
const MAX_RETAINED_EXTENSION_STREAMS: usize = 100_000;
// A record-count limit alone is insufficient because one retained record may
// contain many hard-link names, alternate data streams, or fragmented runs.
// These aggregate limits cover every nested element kept in `ParsedEntry`.
// Extension streams remain subject to their stricter independent cap above.
const MAX_RETAINED_ENTRY_NAMES: usize = 400_000;
const MAX_RETAINED_ENTRY_STREAMS: usize = 200_000;
const MAX_RETAINED_ENTRY_RUN_ELEMENTS: usize = 1_000_000;
// Allocation metadata is advisory for recoverability classification. Retaining
// the first 64 MiB covers 536,870,912 cluster states while bounding both the
// source read and the backing Vec for hostile or unusually large volumes.
// Clusters whose bits fall beyond this prefix remain Unknown.
const MAX_BITMAP_READ_BYTES: u64 = 64 * 1024 * 1024;
// Hostile metadata must not retain one formatted warning per attempted record.
const MAX_MFT_DETAILED_WARNINGS: usize = 16;
// Namespace evidence is exported for folder scoping, but hostile hard-link
// graphs must not cause unbounded path expansion or retained components.
const MAX_NAMESPACE_PATHS: usize = 65_536;
const MAX_NAMESPACE_PATHS_PER_NAME: usize = 256;
const MAX_NAMESPACE_COMPONENTS: usize = 1_048_576;

#[cfg(test)]
std::thread_local! {
    static NAMESPACE_EXPANSION_CALLS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

/// Stable reference to one observed MFT record generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NtfsNodeRef {
    pub record: u64,
    pub sequence: u16,
}

/// Truthfulness state for one reconstructed namespace path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NtfsPathState {
    /// Every parent reference reached the active root with exact sequences.
    Exact,
    /// The path used a deleted-directory generation match.
    Reconstructed,
    /// Known metadata or a safety bound prevents complete path evidence.
    Incomplete,
    /// A parent is missing, reused, cyclic, or not a directory.
    Orphaned,
    /// More than one namespace identity can satisfy a displayed path.
    Ambiguous,
}

/// One non-DOS `$FILE_NAME` path for an observed MFT record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtfsNamespacePath {
    pub node: NtfsNodeRef,
    pub namespace: u8,
    pub name: String,
    /// Root-first display components, excluding `name`.
    pub parent_path: Vec<String>,
    /// Root-first observed directory identities, including the root and the
    /// immediate parent when those identities were validated.
    pub ancestors: Vec<NtfsNodeRef>,
    pub state: NtfsPathState,
}

/// Directory identity retained independently from recoverable candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NtfsDirectoryNode {
    pub node: NtfsNodeRef,
    pub active: bool,
}

/// Result of resolving mounted-folder components against active directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NtfsDirectoryResolution {
    Unique(NtfsNodeRef),
    NotFound,
    Ambiguous,
    Unknown,
}

/// Three-valued folder membership. Unknown is never collapsed into NoMatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NtfsScopeMembership {
    Match,
    NoMatch,
    Unknown,
}

/// Bounded namespace evidence used for identity-based folder scoping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtfsNamespace {
    pub paths: Vec<NtfsNamespacePath>,
    pub directories: Vec<NtfsDirectoryNode>,
    /// True only when the scanned MFT and namespace expansion were exhaustive
    /// within the supported metadata model and safety budgets.
    pub is_complete: bool,
}

/// Bounded-index construction failure for externally assembled namespaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NtfsNamespaceIndexError {
    TooManyPaths,
}

/// Summary of the path evidence retained for one record generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NtfsCandidatePathEvidence {
    Missing,
    One(NtfsPathState),
    Multiple,
}

/// One-pass, bounded lookup index over namespace paths.
///
/// The index owns only path offsets. Path components and ancestry remain in
/// the namespace, so aggregate retained evidence stays bounded by the scanner's
/// namespace limits.
#[derive(Debug)]
pub struct NtfsNamespaceIndex<'a> {
    namespace: &'a NtfsNamespace,
    paths_by_node: HashMap<NtfsNodeRef, Vec<usize>>,
}

impl NtfsNamespace {
    /// Builds a path-offset index in one pass.
    pub fn build_index(&self) -> Result<NtfsNamespaceIndex<'_>, NtfsNamespaceIndexError> {
        if self.paths.len() > MAX_NAMESPACE_PATHS {
            return Err(NtfsNamespaceIndexError::TooManyPaths);
        }
        let mut paths_by_node: HashMap<NtfsNodeRef, Vec<usize>> =
            HashMap::with_capacity(self.paths.len());
        for (offset, path) in self.paths.iter().enumerate() {
            paths_by_node.entry(path.node).or_default().push(offset);
        }
        Ok(NtfsNamespaceIndex {
            namespace: self,
            paths_by_node,
        })
    }

    /// Resolves exact, root-first components to one active directory identity.
    ///
    /// Component equality is deliberate: this model does not claim to emulate
    /// NTFS case-folding or per-directory case-sensitivity rules.
    pub fn resolve_active_directory(&self, components: &[String]) -> NtfsDirectoryResolution {
        let active: HashSet<NtfsNodeRef> = self
            .directories
            .iter()
            .filter(|directory| directory.active)
            .map(|directory| directory.node)
            .collect();

        if components.is_empty() {
            let roots: HashSet<NtfsNodeRef> = active
                .iter()
                .copied()
                .filter(|node| node.record == ROOT_RECORD)
                .collect();
            return match roots.len() {
                1 => {
                    NtfsDirectoryResolution::Unique(*roots.iter().next().expect("one active root"))
                }
                0 if self.is_complete => NtfsDirectoryResolution::NotFound,
                0 => NtfsDirectoryResolution::Unknown,
                _ => NtfsDirectoryResolution::Ambiguous,
            };
        }

        let mut exact_nodes = HashSet::new();
        let mut uncertain_match = false;
        for path in &self.paths {
            if !active.contains(&path.node) || !path_matches_components(path, components) {
                continue;
            }
            if path.state == NtfsPathState::Exact {
                exact_nodes.insert(path.node);
            } else {
                uncertain_match = true;
            }
        }

        if exact_nodes.len() > 1 {
            NtfsDirectoryResolution::Ambiguous
        } else if exact_nodes.len() == 1 && !uncertain_match {
            NtfsDirectoryResolution::Unique(*exact_nodes.iter().next().expect("one exact node"))
        } else if uncertain_match || !self.is_complete {
            NtfsDirectoryResolution::Unknown
        } else {
            NtfsDirectoryResolution::NotFound
        }
    }

    /// Classifies one candidate generation by directory identity reachability.
    pub fn classify_candidate(
        &self,
        candidate: NtfsNodeRef,
        directory: NtfsNodeRef,
    ) -> NtfsScopeMembership {
        let Ok(index) = self.build_index() else {
            return NtfsScopeMembership::Unknown;
        };
        index.classify_candidate(candidate, directory)
    }
}

impl NtfsNamespaceIndex<'_> {
    fn path_offsets(&self, node: NtfsNodeRef) -> &[usize] {
        self.paths_by_node.get(&node).map_or(&[], Vec::as_slice)
    }

    /// Classifies one candidate using only the paths indexed for that node.
    pub fn classify_candidate(
        &self,
        candidate: NtfsNodeRef,
        directory: NtfsNodeRef,
    ) -> NtfsScopeMembership {
        let path_offsets = self.path_offsets(candidate);
        if path_offsets
            .iter()
            .any(|offset| self.namespace.paths[*offset].ancestors.contains(&directory))
        {
            return NtfsScopeMembership::Match;
        }
        if path_offsets.is_empty()
            || !self.namespace.is_complete
            || path_offsets.iter().any(|offset| {
                !matches!(
                    self.namespace.paths[*offset].state,
                    NtfsPathState::Exact | NtfsPathState::Reconstructed
                )
            })
        {
            NtfsScopeMembership::Unknown
        } else {
            NtfsScopeMembership::NoMatch
        }
    }

    /// Returns the presentation-relevant evidence without scanning unrelated
    /// namespace paths.
    pub fn candidate_path_evidence(&self, node: NtfsNodeRef) -> NtfsCandidatePathEvidence {
        match self.path_offsets(node) {
            [] => NtfsCandidatePathEvidence::Missing,
            [offset] => NtfsCandidatePathEvidence::One(self.namespace.paths[*offset].state),
            _ => NtfsCandidatePathEvidence::Multiple,
        }
    }

    #[cfg(test)]
    fn indexed_node_count(&self) -> usize {
        self.paths_by_node.len()
    }
}

fn path_matches_components(path: &NtfsNamespacePath, components: &[String]) -> bool {
    path.parent_path.len().checked_add(1) == Some(components.len())
        && path
            .parent_path
            .iter()
            .zip(components)
            .all(|(actual, expected)| actual == expected)
        && components.last() == Some(&path.name)
}

/// Output of an NTFS metadata scan.
#[derive(Debug)]
pub struct NtfsScanOutput {
    pub boot: NtfsBoot,
    pub candidates: Vec<Candidate>,
    pub namespace: NtfsNamespace,
    pub coverage: NtfsScanCoverage,
    pub allocation: Option<NtfsAllocationSnapshot>,
    pub warnings: Vec<String>,
    /// `false` when metadata bounds or skipped corrupt/unreadable records mean
    /// the scanner cannot claim it enumerated every possible MFT candidate.
    pub is_complete: bool,
}

/// Quantitative MFT coverage. These counters make a zero-result scan
/// distinguishable from an exhaustive absence of deleted metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NtfsScanCoverage {
    /// Records declared by the unnamed `$MFT::$DATA` logical size.
    pub records_declared: u64,
    /// Whole records inside the initialized, source-bounded physical prefix.
    pub records_available: u64,
    /// Records the scanner attempted within its explicit work ceiling.
    pub records_examined: u64,
    pub bytes_declared: u64,
    pub bytes_available: u64,
    pub bytes_examined: u64,
}

/// Trusted allocation evidence derived from `$Bitmap`.
#[derive(Debug, Clone)]
pub struct NtfsAllocationSnapshot {
    cluster_size: u64,
    map: AllocationMap,
}

impl NtfsAllocationSnapshot {
    pub fn cluster_size(&self) -> u64 {
        self.cluster_size
    }

    pub fn is_complete(&self) -> bool {
        self.map.is_complete()
    }

    pub fn known_cluster_count(&self) -> u64 {
        self.map.known_cluster_count()
    }

    /// Coalesces only bitmap-backed, explicitly free clusters into physical
    /// source regions. Checked arithmetic drops impossible hostile ranges.
    pub fn free_regions(&self) -> impl Iterator<Item = Region> + '_ {
        self.map.free_cluster_runs().filter_map(|run| {
            let offset = run.start_cluster.checked_mul(self.cluster_size)?;
            let len = run.cluster_count.checked_mul(self.cluster_size)?;
            Region::new(offset, len)
        })
    }
}

struct ParsedEntry {
    sequence: u16,
    in_use: bool,
    is_directory: bool,
    best_name: Option<FileNameAttr>,
    names: Vec<FileNameAttr>,
    attributes_complete: bool,
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
    let output_len = usize::try_from(len)
        .map_err(|_| ScanError::Corrupt("stream read length does not fit address space".into()))?;
    let mut out = Vec::new();
    out.try_reserve_exact(output_len)
        .map_err(|_| ScanError::Corrupt("stream read allocation failed safely".into()))?;
    out.resize(output_len, 0);
    let mut filled = 0u64;
    let mut stream_pos = 0u64;
    for run in runs {
        let run_len = run
            .cluster_count
            .checked_mul(cluster_size)
            .ok_or_else(|| ScanError::Corrupt("stream run length overflow".into()))?;
        let run_start = stream_pos;
        let run_end = stream_pos
            .checked_add(run_len)
            .ok_or_else(|| ScanError::Corrupt("stream logical range overflow".into()))?;
        stream_pos = run_end;
        let want_start = logical_offset
            .checked_add(filled)
            .ok_or_else(|| ScanError::Corrupt("stream requested range overflow".into()))?;
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
                let output_start = usize::try_from(filled).map_err(|_| {
                    ScanError::Corrupt("stream output offset does not fit address space".into())
                })?;
                let output_end_u64 = filled
                    .checked_add(take)
                    .ok_or_else(|| ScanError::Corrupt("stream output range overflow".into()))?;
                let output_end = usize::try_from(output_end_u64).map_err(|_| {
                    ScanError::Corrupt("stream output end does not fit address space".into())
                })?;
                reader.read_exact_at(phys, &mut out[output_start..output_end])?;
            }
            None => { /* sparse: already zero */ }
        }
        filled = filled
            .checked_add(take)
            .ok_or_else(|| ScanError::Corrupt("stream filled length overflow".into()))?;
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
        let run_end = stream_pos.checked_add(run_len)?;
        if logical_offset < run_end {
            let within = logical_offset - stream_pos;
            return run.lcn?.checked_mul(cluster_size)?.checked_add(within);
        }
        stream_pos = run_end;
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrustedStreamPrefix {
    len: u64,
    stopped_at_untrusted_run: bool,
    stopped_at_sparse_run: bool,
}

fn trusted_physical_stream_prefix_len(
    runs: &[RunElement],
    cluster_size: u64,
    source_len: u64,
    data_size: u64,
    initialized_size: u64,
) -> Result<TrustedStreamPrefix, ScanError> {
    if cluster_size == 0 {
        return Err(ScanError::Corrupt("zero stream cluster size".into()));
    }
    let logical_limit = data_size.min(initialized_size);
    let mut trusted_len = 0u64;
    for run in runs {
        if trusted_len >= logical_limit {
            break;
        }
        let run_len = run
            .cluster_count
            .checked_mul(cluster_size)
            .ok_or_else(|| ScanError::Corrupt("stream run coverage overflow".into()))?;
        let take = run_len.min(logical_limit - trusted_len);
        let Some(lcn) = run.lcn else {
            return Ok(TrustedStreamPrefix {
                len: trusted_len,
                stopped_at_untrusted_run: true,
                stopped_at_sparse_run: true,
            });
        };
        let physical_start = lcn
            .checked_mul(cluster_size)
            .ok_or_else(|| ScanError::Corrupt("stream physical offset overflow".into()))?;
        let physical_end = physical_start
            .checked_add(take)
            .ok_or_else(|| ScanError::Corrupt("stream physical range overflow".into()))?;
        if physical_end > source_len {
            return Ok(TrustedStreamPrefix {
                len: trusted_len,
                stopped_at_untrusted_run: true,
                stopped_at_sparse_run: false,
            });
        }
        trusted_len = trusted_len
            .checked_add(take)
            .ok_or_else(|| ScanError::Corrupt("trusted stream prefix overflow".into()))?;
    }
    Ok(TrustedStreamPrefix {
        len: trusted_len,
        stopped_at_untrusted_run: trusted_len < logical_limit,
        stopped_at_sparse_run: false,
    })
}

fn bounded_mft_record_count(
    initialized_stream_len: u64,
    record_size: u64,
    warnings: &mut Vec<String>,
) -> Result<u64, ScanError> {
    if record_size == 0 {
        return Err(ScanError::Corrupt("zero MFT record size".into()));
    }
    let available_records = initialized_stream_len / record_size;
    let bounded = available_records.min(MAX_MFT_RECORDS);
    if bounded < available_records {
        warnings.push(format!(
            "MFT scan work budget capped parsing at {bounded} of {available_records} records \
             ({MAX_MFT_RECORDS} records maximum)"
        ));
    }
    Ok(bounded)
}

fn bounded_bitmap_read_len(
    expected_bytes: u64,
    trusted_bytes: u64,
    warnings: &mut Vec<String>,
) -> u64 {
    let available = expected_bytes.min(trusted_bytes);
    let bounded = available.min(MAX_BITMAP_READ_BYTES);
    if bounded < available {
        warnings.push(format!(
            "$Bitmap read budget capped data at {MAX_BITMAP_READ_BYTES} of {expected_bytes} \
             expected bytes; later bits unknown"
        ));
    }
    bounded
}

#[derive(Debug, Default)]
struct MftWarningBudget {
    retained: usize,
    suppressed: u64,
}

impl MftWarningBudget {
    fn push(&mut self, warnings: &mut Vec<String>, warning: String) {
        if self.retained < MAX_MFT_DETAILED_WARNINGS {
            warnings.push(warning);
            self.retained = self.retained.saturating_add(1);
        } else {
            self.suppressed = self.suppressed.saturating_add(1);
        }
    }

    fn finish(self, warnings: &mut Vec<String>) {
        if self.suppressed > 0 {
            warnings.push(format!(
                "MFT record warnings suppressed: {} additional record error(s)",
                self.suppressed
            ));
        }
    }
}

#[derive(Default)]
struct MftRetentionBudget {
    deleted_entries: usize,
    directory_entries: usize,
    extension_references: usize,
    extension_streams: usize,
    entry_names: usize,
    entry_streams: usize,
    entry_run_elements: usize,
    deleted_warning_emitted: bool,
    directory_warning_emitted: bool,
    extension_warning_emitted: bool,
    extension_stream_warning_emitted: bool,
    entry_evidence_warning_emitted: bool,
    allocation_warning_emitted: bool,
}

#[derive(Clone, Copy)]
struct EntryRetentionCost {
    names: usize,
    streams: usize,
    run_elements: usize,
}

impl MftRetentionBudget {
    fn planned_base_record_counts(
        &mut self,
        in_use: bool,
        is_directory: bool,
        is_complete: &mut bool,
        warnings: &mut Vec<String>,
    ) -> Option<(usize, usize)> {
        let next_deleted = if in_use {
            Some(self.deleted_entries)
        } else {
            self.deleted_entries.checked_add(1)
        };
        let next_directory = if is_directory {
            self.directory_entries.checked_add(1)
        } else {
            Some(self.directory_entries)
        };
        let deleted_limit_reached =
            next_deleted.is_none_or(|next| next > MAX_RETAINED_DELETED_ENTRIES);
        let directory_limit_reached =
            next_directory.is_none_or(|next| next > MAX_RETAINED_DIRECTORY_ENTRIES);
        if deleted_limit_reached {
            *is_complete = false;
            if !self.deleted_warning_emitted {
                warnings.push(format!(
                    "MFT deleted-entry retention budget reached {MAX_RETAINED_DELETED_ENTRIES}; later candidates were not retained"
                ));
                self.deleted_warning_emitted = true;
            }
        }
        if directory_limit_reached {
            *is_complete = false;
            if !self.directory_warning_emitted {
                warnings.push(format!(
                    "MFT directory retention budget reached {MAX_RETAINED_DIRECTORY_ENTRIES}; later path evidence was not retained"
                ));
                self.directory_warning_emitted = true;
            }
        }
        if deleted_limit_reached || directory_limit_reached {
            return None;
        }
        Some((next_deleted?, next_directory?))
    }

    fn base_record_slot_available(
        &mut self,
        in_use: bool,
        is_directory: bool,
        is_complete: &mut bool,
        warnings: &mut Vec<String>,
    ) -> bool {
        self.planned_base_record_counts(in_use, is_directory, is_complete, warnings)
            .is_some()
    }

    fn admit_base_record(
        &mut self,
        in_use: bool,
        is_directory: bool,
        cost: EntryRetentionCost,
        is_complete: &mut bool,
        warnings: &mut Vec<String>,
    ) -> bool {
        let Some((next_deleted, next_directory)) =
            self.planned_base_record_counts(in_use, is_directory, is_complete, warnings)
        else {
            return false;
        };
        let Some(next_names) = self.entry_names.checked_add(cost.names) else {
            self.note_entry_evidence_limit(is_complete, warnings);
            return false;
        };
        let Some(next_streams) = self.entry_streams.checked_add(cost.streams) else {
            self.note_entry_evidence_limit(is_complete, warnings);
            return false;
        };
        let Some(next_run_elements) = self.entry_run_elements.checked_add(cost.run_elements) else {
            self.note_entry_evidence_limit(is_complete, warnings);
            return false;
        };
        if next_names > MAX_RETAINED_ENTRY_NAMES
            || next_streams > MAX_RETAINED_ENTRY_STREAMS
            || next_run_elements > MAX_RETAINED_ENTRY_RUN_ELEMENTS
        {
            self.note_entry_evidence_limit(is_complete, warnings);
            return false;
        }

        self.deleted_entries = next_deleted;
        self.directory_entries = next_directory;
        self.entry_names = next_names;
        self.entry_streams = next_streams;
        self.entry_run_elements = next_run_elements;
        true
    }

    fn admit_extension_reference(
        &mut self,
        is_complete: &mut bool,
        warnings: &mut Vec<String>,
    ) -> bool {
        if self.extension_references >= MAX_RETAINED_EXTENSION_REFERENCES {
            *is_complete = false;
            if !self.extension_warning_emitted {
                warnings.push(format!(
                    "MFT extension-reference retention budget reached {MAX_RETAINED_EXTENSION_REFERENCES}; later data streams were not retained"
                ));
                self.extension_warning_emitted = true;
            }
            return false;
        }
        self.extension_references = self.extension_references.saturating_add(1);
        true
    }

    fn admit_extension_streams(
        &mut self,
        count: usize,
        run_element_count: usize,
        is_complete: &mut bool,
        warnings: &mut Vec<String>,
    ) -> bool {
        let Some(next) = self.extension_streams.checked_add(count) else {
            *is_complete = false;
            if !self.extension_stream_warning_emitted {
                warnings.push(
                    "MFT extension-stream retention budget overflowed; later data streams were not retained"
                        .into(),
                );
                self.extension_stream_warning_emitted = true;
            }
            return false;
        };
        if next > MAX_RETAINED_EXTENSION_STREAMS {
            *is_complete = false;
            if !self.extension_stream_warning_emitted {
                warnings.push(format!(
                    "MFT extension-stream retention budget reached {MAX_RETAINED_EXTENSION_STREAMS}; later data streams were not retained"
                ));
                self.extension_stream_warning_emitted = true;
            }
            return false;
        }
        let Some(next_entry_streams) = self.entry_streams.checked_add(count) else {
            self.note_entry_evidence_limit(is_complete, warnings);
            return false;
        };
        let Some(next_entry_run_elements) = self.entry_run_elements.checked_add(run_element_count)
        else {
            self.note_entry_evidence_limit(is_complete, warnings);
            return false;
        };
        if next_entry_streams > MAX_RETAINED_ENTRY_STREAMS
            || next_entry_run_elements > MAX_RETAINED_ENTRY_RUN_ELEMENTS
        {
            self.note_entry_evidence_limit(is_complete, warnings);
            return false;
        }
        self.extension_streams = next;
        self.entry_streams = next_entry_streams;
        self.entry_run_elements = next_entry_run_elements;
        true
    }

    fn note_entry_evidence_limit(&mut self, is_complete: &mut bool, warnings: &mut Vec<String>) {
        *is_complete = false;
        if !self.entry_evidence_warning_emitted {
            warnings.push(format!(
                "MFT retained-entry evidence budget reached (names {MAX_RETAINED_ENTRY_NAMES}, streams {MAX_RETAINED_ENTRY_STREAMS}, run elements {MAX_RETAINED_ENTRY_RUN_ELEMENTS}); over-budget records or streams were not retained"
            ));
            self.entry_evidence_warning_emitted = true;
        }
    }

    fn note_allocation_failure(&mut self, is_complete: &mut bool, warnings: &mut Vec<String>) {
        *is_complete = false;
        if !self.allocation_warning_emitted {
            warnings.push(
                "MFT metadata retention allocation failed; later recovery evidence was skipped"
                    .into(),
            );
            self.allocation_warning_emitted = true;
        }
    }
}

fn retained_run_element_count(streams: &[DataStream]) -> Option<usize> {
    streams.iter().try_fold(0usize, |total, stream| {
        let stream_runs = match &stream.kind {
            DataStreamKind::Resident { .. } => 0,
            DataStreamKind::NonResident { runs, .. } => runs.len(),
        };
        total.checked_add(stream_runs)
    })
}

fn retain_base_entry(
    record_no: u64,
    entry: ParsedEntry,
    entries: &mut HashMap<u64, ParsedEntry>,
    retention_budget: &mut MftRetentionBudget,
    is_complete: &mut bool,
    warnings: &mut Vec<String>,
) -> bool {
    let Some(run_element_count) = retained_run_element_count(&entry.streams) else {
        retention_budget.note_entry_evidence_limit(is_complete, warnings);
        return false;
    };
    if entries.try_reserve(1).is_err() {
        retention_budget.note_allocation_failure(is_complete, warnings);
        return false;
    }
    if !retention_budget.admit_base_record(
        entry.in_use,
        entry.is_directory,
        EntryRetentionCost {
            names: entry.names.len(),
            streams: entry.streams.len(),
            run_elements: run_element_count,
        },
        is_complete,
        warnings,
    ) {
        return false;
    }
    entries.insert(record_no, entry);
    true
}

#[allow(clippy::too_many_arguments)]
fn process_mft_record(
    raw: &[u8],
    record_no: u64,
    logical_offset: u64,
    boot: &NtfsBoot,
    mft_runs: &[RunElement],
    entries: &mut HashMap<u64, ParsedEntry>,
    extension_data: &mut HashMap<u64, Vec<u64>>,
    retention_budget: &mut MftRetentionBudget,
    is_complete: &mut bool,
    warning_budget: &mut MftWarningBudget,
    warnings: &mut Vec<String>,
) {
    let rec = match parse_file_record(raw, boot.bytes_per_sector) {
        Ok(record) => record,
        Err(RecordParseError::NotAFileRecord) if raw.iter().all(|byte| *byte == 0) => return,
        Err(RecordParseError::NotAFileRecord) => {
            *is_complete = false;
            warning_budget.push(
                warnings,
                format!("MFT record {record_no} has no FILE signature; skipped"),
            );
            return;
        }
        Err(RecordParseError::FixupMismatch) => {
            *is_complete = false;
            warning_budget.push(
                warnings,
                format!("MFT record {record_no} has torn sectors (fixup mismatch); skipped"),
            );
            return;
        }
        Err(RecordParseError::Corrupt(message)) => {
            *is_complete = false;
            warning_budget.push(
                warnings,
                format!("MFT record {record_no} corrupt: {message}"),
            );
            return;
        }
    };

    if rec.base_record != 0 {
        // Deleted base records are the only current candidates that need data
        // from extension records. Avoid retaining the extension graph of every
        // active file on large system volumes.
        if !rec.in_use && retention_budget.admit_extension_reference(is_complete, warnings) {
            if extension_data.try_reserve(1).is_err() {
                retention_budget.note_allocation_failure(is_complete, warnings);
                return;
            }
            let references = extension_data.entry(rec.base_record).or_default();
            if references.try_reserve(1).is_err() {
                retention_budget.note_allocation_failure(is_complete, warnings);
                return;
            }
            references.push(record_no);
        }
        return;
    }

    // Active regular files cannot become undelete candidates. Directories are
    // retained for path reconstruction; free user records are retained for
    // candidate construction. This keeps memory proportional to useful
    // recovery evidence rather than to every live file in the volume.
    if !rec.is_directory && (rec.in_use || record_no < FIRST_USER_RECORD) {
        return;
    }
    if !retention_budget.base_record_slot_available(
        rec.in_use,
        rec.is_directory,
        is_complete,
        warnings,
    ) {
        return;
    }

    let physical_offset =
        stream_phys_offset(mft_runs, boot.cluster_size, logical_offset).unwrap_or(0);
    let (entry, attributes_complete) = parse_entry(&rec, boot, physical_offset);
    if !attributes_complete {
        *is_complete = false;
        warning_budget.push(
            warnings,
            format!(
                "MFT record {record_no} has incomplete or unresolved attributes; metadata may be incomplete"
            ),
        );
    }
    retain_base_entry(
        record_no,
        entry,
        entries,
        retention_budget,
        is_complete,
        warnings,
    );
}

/// Parses a resident `$ATTRIBUTE_LIST` value, returning referenced extension
/// record numbers (excluding the base record itself).
fn attribute_list_extensions(value: &[u8], base_record: u64) -> (Vec<u64>, bool) {
    let mut refs = Vec::new();
    let mut pos = 0usize;
    for _ in 0..1024 {
        if pos == value.len() {
            return (refs, true);
        }
        let Some(header_end) = pos.checked_add(26) else {
            return (refs, false);
        };
        if header_end > value.len() {
            return (refs, false);
        }
        let Some(entry_len_offset) = pos.checked_add(4) else {
            return (refs, false);
        };
        let entry_len = le::u16_at(value, entry_len_offset).unwrap_or(0) as usize;
        let Some(entry_end) = pos.checked_add(entry_len) else {
            return (refs, false);
        };
        if entry_len < 26 || entry_end > value.len() {
            return (refs, false);
        }
        let Some(file_ref_offset) = pos.checked_add(16) else {
            return (refs, false);
        };
        let file_ref = le::u64_at(value, file_ref_offset).unwrap_or(0) & 0x0000_FFFF_FFFF_FFFF;
        if file_ref != base_record && !refs.contains(&file_ref) {
            refs.push(file_ref);
        }
        pos = entry_end;
    }
    (refs, false)
}

fn pick_best_name(names: &[FileNameAttr]) -> Option<FileNameAttr> {
    names
        .iter()
        .min_by_key(|n| if n.namespace == NS_DOS { 1u8 } else { 0u8 })
        .cloned()
}

fn useful_namespace_names(entry: &ParsedEntry) -> impl Iterator<Item = &FileNameAttr> {
    entry
        .names
        .iter()
        .filter(|name| name.namespace != NS_DOS && !name.name.is_empty())
}

#[derive(Debug, Clone)]
struct ParentPathEvidence {
    parent_path: Vec<String>,
    ancestors: Vec<NtfsNodeRef>,
    state: NtfsPathState,
}

struct ParentPathExpansion {
    paths: Vec<ParentPathEvidence>,
    complete: bool,
}

fn merge_path_state(left: NtfsPathState, right: NtfsPathState) -> NtfsPathState {
    left.max(right)
}

fn orphaned_parent_path() -> ParentPathExpansion {
    ParentPathExpansion {
        paths: vec![ParentPathEvidence {
            parent_path: Vec::new(),
            ancestors: Vec::new(),
            state: NtfsPathState::Orphaned,
        }],
        complete: true,
    }
}

fn expand_parent_paths(
    entries: &HashMap<u64, ParsedEntry>,
    parent_record: u64,
    parent_sequence: u16,
    visited: &HashSet<u64>,
    depth: usize,
) -> ParentPathExpansion {
    #[cfg(test)]
    NAMESPACE_EXPANSION_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));

    if depth >= MAX_PATH_DEPTH {
        return ParentPathExpansion {
            paths: vec![ParentPathEvidence {
                parent_path: Vec::new(),
                ancestors: Vec::new(),
                state: NtfsPathState::Incomplete,
            }],
            complete: false,
        };
    }

    let mut visited = visited.clone();
    if !visited.insert(parent_record) {
        return orphaned_parent_path();
    }
    let Some(parent) = entries.get(&parent_record) else {
        return orphaned_parent_path();
    };
    if !parent.is_directory {
        return orphaned_parent_path();
    }

    let node = NtfsNodeRef {
        record: parent_record,
        sequence: parent.sequence,
    };
    if parent_record == ROOT_RECORD {
        if !parent.in_use || parent.sequence != parent_sequence {
            return orphaned_parent_path();
        }
        return ParentPathExpansion {
            paths: vec![ParentPathEvidence {
                parent_path: Vec::new(),
                ancestors: vec![node],
                state: if parent.attributes_complete {
                    NtfsPathState::Exact
                } else {
                    NtfsPathState::Incomplete
                },
            }],
            complete: true,
        };
    }

    let exact_sequence = parent.sequence == parent_sequence;
    let deleted_generation = !parent.in_use && parent.sequence == parent_sequence.wrapping_add(1);
    if !exact_sequence && !deleted_generation {
        return orphaned_parent_path();
    }

    let relation_state = if !parent.attributes_complete {
        NtfsPathState::Incomplete
    } else if !parent.in_use {
        NtfsPathState::Reconstructed
    } else {
        NtfsPathState::Exact
    };
    let names: Vec<&FileNameAttr> = useful_namespace_names(parent).collect();
    if names.is_empty() {
        return ParentPathExpansion {
            paths: vec![ParentPathEvidence {
                parent_path: Vec::new(),
                ancestors: vec![node],
                state: NtfsPathState::Orphaned,
            }],
            complete: true,
        };
    }

    let mut paths = Vec::new();
    let mut complete = true;
    'names: for name in names {
        if paths.len() >= MAX_NAMESPACE_PATHS_PER_NAME {
            complete = false;
            break 'names;
        }
        let upstream = expand_parent_paths(
            entries,
            name.parent_record,
            name.parent_sequence,
            &visited,
            depth + 1,
        );
        complete &= upstream.complete;
        for mut path in upstream.paths {
            if paths.len() >= MAX_NAMESPACE_PATHS_PER_NAME {
                complete = false;
                break 'names;
            }
            path.parent_path.push(name.name.clone());
            path.ancestors.push(node);
            path.state = merge_path_state(path.state, relation_state);
            paths.push(path);
        }
    }

    if paths.is_empty() {
        ParentPathExpansion {
            paths: vec![ParentPathEvidence {
                parent_path: Vec::new(),
                ancestors: vec![node],
                state: NtfsPathState::Incomplete,
            }],
            complete: false,
        }
    } else {
        ParentPathExpansion { paths, complete }
    }
}

fn build_namespace(
    entries: &HashMap<u64, ParsedEntry>,
    scan_complete: bool,
    warnings: &mut Vec<String>,
) -> NtfsNamespace {
    let mut directories: Vec<NtfsDirectoryNode> = entries
        .iter()
        .filter_map(|(&record, entry)| {
            entry.is_directory.then_some(NtfsDirectoryNode {
                node: NtfsNodeRef {
                    record,
                    sequence: entry.sequence,
                },
                active: entry.in_use,
            })
        })
        .collect();
    directories.sort_by_key(|directory| directory.node);

    let mut records: Vec<u64> = entries.keys().copied().collect();
    records.sort_unstable();
    let mut paths = Vec::new();
    let mut components_used = 0usize;
    let mut namespace_complete = scan_complete;
    let mut budget_reached = false;

    'records: for record in records {
        if record == ROOT_RECORD {
            continue;
        }
        let entry = &entries[&record];
        let node = NtfsNodeRef {
            record,
            sequence: entry.sequence,
        };
        for name in useful_namespace_names(entry) {
            if paths.len() >= MAX_NAMESPACE_PATHS {
                namespace_complete = false;
                budget_reached = true;
                break 'records;
            }
            let visited = HashSet::from([record]);
            let expansion = expand_parent_paths(
                entries,
                name.parent_record,
                name.parent_sequence,
                &visited,
                0,
            );
            if !expansion.complete {
                namespace_complete = false;
                budget_reached = true;
            }
            for parent in expansion.paths {
                if paths.len() >= MAX_NAMESPACE_PATHS {
                    namespace_complete = false;
                    budget_reached = true;
                    break 'records;
                }
                let component_cost = parent
                    .parent_path
                    .len()
                    .checked_add(parent.ancestors.len())
                    .and_then(|cost| cost.checked_add(1));
                let Some(next_component_count) =
                    component_cost.and_then(|cost| components_used.checked_add(cost))
                else {
                    namespace_complete = false;
                    budget_reached = true;
                    break 'records;
                };
                if next_component_count > MAX_NAMESPACE_COMPONENTS {
                    namespace_complete = false;
                    budget_reached = true;
                    break 'records;
                }
                components_used = next_component_count;
                paths.push(NtfsNamespacePath {
                    node,
                    namespace: name.namespace,
                    name: name.name.clone(),
                    parent_path: parent.parent_path,
                    ancestors: parent.ancestors,
                    state: merge_path_state(
                        parent.state,
                        if entry.attributes_complete {
                            NtfsPathState::Exact
                        } else {
                            NtfsPathState::Incomplete
                        },
                    ),
                });
            }
        }
    }

    paths.sort_by(|left, right| {
        left.node
            .cmp(&right.node)
            .then_with(|| left.parent_path.cmp(&right.parent_path))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.namespace.cmp(&right.namespace))
    });
    if budget_reached {
        warnings.push(format!(
            "NTFS namespace evidence reached its bounded path/component budget ({MAX_NAMESPACE_PATHS} paths, {MAX_NAMESPACE_COMPONENTS} components); folder scope is incomplete"
        ));
    }

    NtfsNamespace {
        paths,
        directories,
        is_complete: namespace_complete,
    }
}

/// Scans an NTFS volume region for deleted (and orphaned) candidates.
pub fn scan_ntfs(reader: &dyn SourceReader) -> Result<NtfsScanOutput, ScanError> {
    let mut warnings = Vec::new();
    let sector0 = reader.read_vec_at(0, 512)?;
    let boot = NtfsBoot::parse(&sector0, reader.len())?;
    let cluster_size = boot.cluster_size;
    let record_size = boot.file_record_size as u64;

    // Bootstrap: record 0 describes the MFT itself.
    let mft_start = boot
        .mft_lcn
        .checked_mul(cluster_size)
        .ok_or_else(|| ScanError::Corrupt("MFT byte offset overflow".into()))?;
    let record_size_usize = usize::try_from(record_size)
        .map_err(|_| ScanError::Corrupt("MFT record size does not fit address space".into()))?;
    let rec0_raw = reader.read_vec_at(mft_start, record_size_usize)?;
    let rec0 = parse_file_record(&rec0_raw, boot.bytes_per_sector)
        .map_err(|e| ScanError::Corrupt(format!("MFT record 0 unusable: {e:?}")))?;
    let (attrs0, attrs0_complete) = iter_attributes(&rec0);
    if !attrs0_complete {
        return Err(ScanError::Corrupt(
            "MFT record 0 has malformed attributes".into(),
        ));
    }
    let mut w0 = Vec::new();
    let streams0 = extract_data_streams(&attrs0, boot.total_clusters, &mut w0);
    warnings.extend(w0);
    let (mft_runs, mft_data_size, mft_initialized_size) =
        match streams0.iter().find(|s| s.name.is_none()) {
            Some(DataStream {
                kind:
                    DataStreamKind::NonResident {
                        data_size,
                        initialized_size,
                        runs,
                    },
                ..
            }) => (runs.clone(), *data_size, *initialized_size),
            _ => {
                return Err(ScanError::Corrupt(
                    "MFT record 0 has no usable non-resident $DATA".into(),
                ))
            }
        };
    if mft_data_size % record_size != 0 {
        return Err(ScanError::Corrupt(format!(
            "$MFT data size {mft_data_size} is not aligned to the {record_size}-byte file record size"
        )));
    }
    let minimum_mft_size = FIRST_USER_RECORD
        .checked_mul(record_size)
        .ok_or_else(|| ScanError::Corrupt("minimum $MFT size overflow".into()))?;
    if mft_data_size < minimum_mft_size {
        return Err(ScanError::Corrupt(format!(
            "$MFT data size {mft_data_size} does not cover the {FIRST_USER_RECORD} reserved file records"
        )));
    }
    let trusted_mft_prefix = trusted_physical_stream_prefix_len(
        &mft_runs,
        cluster_size,
        reader.len(),
        mft_data_size,
        mft_initialized_size,
    )?;
    let mut is_complete = trusted_mft_prefix.len >= mft_data_size;
    if trusted_mft_prefix.stopped_at_sparse_run {
        warnings.push(format!(
            "MFT trusted prefix stopped before sparse run at {} bytes; remaining records ignored",
            trusted_mft_prefix.len
        ));
    } else if trusted_mft_prefix.stopped_at_untrusted_run {
        warnings.push(format!(
            "MFT trusted prefix stopped at {} source-bounded bytes; remaining records ignored",
            trusted_mft_prefix.len
        ));
    } else if trusted_mft_prefix.len < mft_data_size {
        warnings.push(format!(
            "MFT scan bounded to {} initialized bytes from declared {mft_data_size}",
            trusted_mft_prefix.len
        ));
    }
    let record_count =
        bounded_mft_record_count(trusted_mft_prefix.len, record_size, &mut warnings)?;
    if record_count < trusted_mft_prefix.len / record_size {
        is_complete = false;
    }

    // Allocation bitmap from record 6 ($Bitmap).
    let allocation = load_allocation_map(
        reader,
        &boot,
        &mft_runs,
        trusted_mft_prefix.len,
        &mut warnings,
    );

    let records_declared = mft_data_size / record_size;
    let records_available = trusted_mft_prefix.len / record_size;
    let bytes_available = records_available
        .checked_mul(record_size)
        .ok_or_else(|| ScanError::Corrupt("available MFT byte count overflow".into()))?;
    let bytes_examined = record_count
        .checked_mul(record_size)
        .ok_or_else(|| ScanError::Corrupt("examined MFT byte count overflow".into()))?;
    let coverage = NtfsScanCoverage {
        records_declared,
        records_available,
        records_examined: record_count,
        bytes_declared: mft_data_size,
        bytes_available,
        bytes_examined,
    };

    // Pass 1: parse the trusted MFT in bounded batches. Only directories and
    // free base records are retained; active regular files are classified and
    // discarded immediately.
    let mut entries: HashMap<u64, ParsedEntry> = HashMap::new();
    let mut extension_data: HashMap<u64, Vec<u64>> = HashMap::new(); // base -> ext record nos
    let mut mft_warning_budget = MftWarningBudget::default();
    let mut mft_retention_budget = MftRetentionBudget::default();
    let records_per_batch = (MFT_BATCH_BYTES / record_size).max(1);
    let mut batch_start_record = 0u64;
    while batch_start_record < record_count {
        let batch_record_count = (record_count - batch_start_record).min(records_per_batch);
        let logical = batch_start_record
            .checked_mul(record_size)
            .ok_or_else(|| ScanError::Corrupt("MFT record offset overflow".into()))?;
        let batch_len = batch_record_count
            .checked_mul(record_size)
            .ok_or_else(|| ScanError::Corrupt("MFT batch length overflow".into()))?;

        match read_stream_range(reader, &mft_runs, cluster_size, logical, batch_len) {
            Ok(batch) => {
                if !batch.iter().all(|byte| *byte == 0) {
                    for batch_index in 0..batch_record_count {
                        let record_no = batch_start_record + batch_index;
                        let start_u64 = batch_index.checked_mul(record_size).ok_or_else(|| {
                            ScanError::Corrupt("MFT batch record offset overflow".into())
                        })?;
                        let end_u64 = start_u64.checked_add(record_size).ok_or_else(|| {
                            ScanError::Corrupt("MFT batch record end overflow".into())
                        })?;
                        let start = usize::try_from(start_u64).map_err(|_| {
                            ScanError::Corrupt(
                                "MFT batch record offset does not fit address space".into(),
                            )
                        })?;
                        let end = usize::try_from(end_u64).map_err(|_| {
                            ScanError::Corrupt(
                                "MFT batch record end does not fit address space".into(),
                            )
                        })?;
                        let record_logical = logical.checked_add(start_u64).ok_or_else(|| {
                            ScanError::Corrupt("MFT record logical offset overflow".into())
                        })?;
                        process_mft_record(
                            &batch[start..end],
                            record_no,
                            record_logical,
                            &boot,
                            &mft_runs,
                            &mut entries,
                            &mut extension_data,
                            &mut mft_retention_budget,
                            &mut is_complete,
                            &mut mft_warning_budget,
                            &mut warnings,
                        );
                    }
                }
            }
            Err(_) => {
                // Isolate a bad range rather than losing the whole batch.
                for batch_index in 0..batch_record_count {
                    let record_no = batch_start_record + batch_index;
                    let record_logical = record_no
                        .checked_mul(record_size)
                        .ok_or_else(|| ScanError::Corrupt("MFT record offset overflow".into()))?;
                    match read_stream_range(
                        reader,
                        &mft_runs,
                        cluster_size,
                        record_logical,
                        record_size,
                    ) {
                        Ok(raw) => process_mft_record(
                            &raw,
                            record_no,
                            record_logical,
                            &boot,
                            &mft_runs,
                            &mut entries,
                            &mut extension_data,
                            &mut mft_retention_budget,
                            &mut is_complete,
                            &mut mft_warning_budget,
                            &mut warnings,
                        ),
                        Err(_) => {
                            is_complete = false;
                            mft_warning_budget.push(
                                &mut warnings,
                                format!("MFT record {record_no} unreadable; skipped"),
                            );
                        }
                    }
                }
            }
        }

        batch_start_record = batch_start_record
            .checked_add(batch_record_count)
            .ok_or_else(|| ScanError::Corrupt("MFT batch cursor overflow".into()))?;
    }
    // Pass 2: merge $DATA streams from extension records referenced by
    // resident attribute lists.
    let mut merge_targets = Vec::new();
    merge_targets
        .try_reserve(extension_data.len().min(entries.len()))
        .map_err(|_| ScanError::Corrupt("extension merge allocation failed safely".into()))?;
    for &record_no in extension_data.keys() {
        if entries.get(&record_no).is_some_and(|entry| {
            entry
                .warnings
                .iter()
                .any(|warning| warning.starts_with("has attribute list"))
        }) {
            merge_targets.push(record_no);
        }
    }
    merge_targets.sort_unstable();
    'merge_targets: for record_no in merge_targets {
        let mut extra = Vec::new();
        if let Some(ext_recs) = extension_data.get(&record_no) {
            for &ext_no in ext_recs {
                let Some(logical) = ext_no.checked_mul(record_size) else {
                    is_complete = false;
                    mft_warning_budget.push(
                        &mut warnings,
                        format!(
                            "MFT extension record {ext_no} offset overflow; data streams may be incomplete"
                        ),
                    );
                    continue;
                };
                match read_stream_range(reader, &mft_runs, cluster_size, logical, record_size) {
                    Ok(raw) => match parse_file_record(&raw, boot.bytes_per_sector) {
                        Ok(rec) => {
                            let (attrs, attrs_complete) = iter_attributes(&rec);
                            if !attrs_complete {
                                is_complete = false;
                                mft_warning_budget.push(
                                    &mut warnings,
                                    format!(
                                        "MFT extension record {ext_no} has incomplete or unresolved attributes; data streams may be incomplete"
                                    ),
                                );
                            }
                            let mut w = Vec::new();
                            let streams = extract_data_streams(&attrs, boot.total_clusters, &mut w);
                            if !w.is_empty() {
                                is_complete = false;
                                mft_warning_budget.push(
                                    &mut warnings,
                                    format!(
                                        "MFT extension record {ext_no} contains unusable data streams; candidate extents may be incomplete"
                                    ),
                                );
                            }
                            let Some(run_element_count) = retained_run_element_count(&streams)
                            else {
                                mft_retention_budget
                                    .note_entry_evidence_limit(&mut is_complete, &mut warnings);
                                break 'merge_targets;
                            };
                            if !mft_retention_budget.admit_extension_streams(
                                streams.len(),
                                run_element_count,
                                &mut is_complete,
                                &mut warnings,
                            ) {
                                break 'merge_targets;
                            }
                            if extra.try_reserve(streams.len()).is_err() {
                                mft_retention_budget
                                    .note_allocation_failure(&mut is_complete, &mut warnings);
                                break 'merge_targets;
                            }
                            extra.extend(streams);
                        }
                        Err(_) => {
                            is_complete = false;
                            mft_warning_budget.push(
                                &mut warnings,
                                format!(
                                    "MFT extension record {ext_no} corrupt on merge; data streams may be incomplete"
                                ),
                            );
                        }
                    },
                    Err(_) => {
                        is_complete = false;
                        mft_warning_budget.push(
                            &mut warnings,
                            format!(
                                "MFT extension record {ext_no} unreadable on merge; data streams may be incomplete"
                            ),
                        );
                    }
                }
            }
        }
        if !extra.is_empty() {
            if let Some(entry) = entries.get_mut(&record_no) {
                if entry.streams.try_reserve(extra.len()).is_err() {
                    mft_retention_budget.note_allocation_failure(&mut is_complete, &mut warnings);
                    break 'merge_targets;
                }
                entry.streams.extend(extra);
            }
        }
    }
    mft_warning_budget.finish(&mut warnings);

    let namespace = build_namespace(&entries, is_complete, &mut warnings);

    // Pass 3: build candidates from records marked free.
    let mut candidates = Vec::new();
    candidates
        .try_reserve(mft_retention_budget.deleted_entries.min(entries.len()))
        .map_err(|_| ScanError::Corrupt("candidate result allocation failed safely".into()))?;
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
    let is_complete = namespace.is_complete;
    let allocation = allocation.map(|map| NtfsAllocationSnapshot { cluster_size, map });

    Ok(NtfsScanOutput {
        boot,
        candidates,
        namespace,
        coverage,
        allocation,
        warnings,
        is_complete,
    })
}

fn parse_entry(rec: &FileRecord, boot: &NtfsBoot, record_phys_offset: u64) -> (ParsedEntry, bool) {
    let (attrs, mut attributes_complete) = iter_attributes(rec);
    let mut warnings = Vec::new();
    let mut names = Vec::new();
    let mut std_info = StandardInformation::default();
    for a in &attrs {
        match (a.type_id, &a.body) {
            (crate::attr::ATTR_STANDARD_INFORMATION, AttrBody::Resident { value, .. }) => {
                if let Some(si) = parse_standard_information(value) {
                    std_info = si;
                } else {
                    attributes_complete = false;
                }
            }
            (crate::attr::ATTR_FILE_NAME, AttrBody::Resident { value, .. }) => {
                if let Some(fn_attr) = parse_file_name(value) {
                    names.push(fn_attr);
                } else {
                    attributes_complete = false;
                }
            }
            (ATTR_ATTRIBUTE_LIST, AttrBody::Resident { value, .. }) => {
                let (exts, list_complete) = attribute_list_extensions(value, 0);
                // The current merge pass uses extension records only for
                // additional data streams and does not prove that every
                // referenced record or candidate-defining attribute was
                // resolved. Preserve the references, but never claim an
                // exhaustive scan for a record that depends on a list.
                attributes_complete = false;
                if !list_complete {
                    warnings.push("attribute list value is malformed".to_string());
                }
                warnings.push(format!(
                    "has attribute list with {} extension reference(s)",
                    exts.len()
                ));
            }
            (ATTR_ATTRIBUTE_LIST, AttrBody::NonResident { .. }) => {
                attributes_complete = false;
                warnings
                    .push("has non-resident attribute list; extents may be incomplete".to_string());
            }
            (crate::attr::ATTR_FILE_NAME | crate::attr::ATTR_STANDARD_INFORMATION, _) => {
                attributes_complete = false;
            }
            _ => {}
        }
    }
    let streams = extract_data_streams(&attrs, boot.total_clusters, &mut warnings);
    let best_name = pick_best_name(&names);
    (
        ParsedEntry {
            sequence: rec.sequence,
            in_use: rec.in_use,
            is_directory: rec.is_directory,
            best_name,
            names,
            attributes_complete,
            std_info,
            streams,
            record_phys_offset,
            warnings,
        },
        attributes_complete,
    )
}

fn load_allocation_map(
    reader: &dyn SourceReader,
    boot: &NtfsBoot,
    mft_runs: &[RunElement],
    trusted_mft_prefix_len: u64,
    warnings: &mut Vec<String>,
) -> Option<AllocationMap> {
    let record_size = boot.file_record_size as u64;
    let bitmap_record_end = BITMAP_RECORD
        .checked_add(1)
        .and_then(|record| record.checked_mul(record_size));
    if bitmap_record_end
        .map(|end| end > trusted_mft_prefix_len)
        .unwrap_or(true)
    {
        warnings.push(
            "$Bitmap record lies beyond trusted MFT prefix; extent availability unknown".into(),
        );
        return None;
    }
    let Some(logical) = BITMAP_RECORD.checked_mul(record_size) else {
        warnings.push("$Bitmap record offset overflow; extent availability unknown".into());
        return None;
    };
    let raw = match read_stream_range(reader, mft_runs, boot.cluster_size, logical, record_size) {
        Ok(raw) => raw,
        Err(_) => {
            warnings.push("$Bitmap record unreadable; extent availability unknown".into());
            return None;
        }
    };
    let rec = match parse_file_record(&raw, boot.bytes_per_sector) {
        Ok(r) => r,
        Err(_) => {
            warnings.push("$Bitmap record unusable; extent availability unknown".into());
            return None;
        }
    };
    let (attrs, attrs_complete) = iter_attributes(&rec);
    if !attrs_complete {
        warnings
            .push("$Bitmap record has malformed attributes; extent availability unknown".into());
        return None;
    }
    let has_bitmap_name = attrs.iter().any(|attribute| {
        attribute.type_id == ATTR_FILE_NAME
            && matches!(
                &attribute.body,
                AttrBody::Resident { value, .. }
                    if parse_file_name(value).is_some_and(|name| {
                        name.name == "$Bitmap" && name.parent_record == ROOT_RECORD
                    })
            )
    });
    let mut unnamed_data = attrs
        .iter()
        .filter(|attribute| attribute.type_id == ATTR_DATA && attribute.name.is_none());
    let data_attribute = unnamed_data.next();
    let record_and_stream_semantics_trusted = rec.in_use
        && !rec.is_directory
        && rec.base_record == 0
        && has_bitmap_name
        && !attrs
            .iter()
            .any(|attribute| attribute.type_id == ATTR_ATTRIBUTE_LIST)
        && data_attribute.is_some()
        && unnamed_data.next().is_none()
        && data_attribute.is_some_and(|attribute| {
            attribute.flags & (ATTR_FLAG_COMPRESSED | ATTR_FLAG_ENCRYPTED | ATTR_FLAG_SPARSE) == 0
                && match &attribute.body {
                    AttrBody::Resident { .. } => true,
                    AttrBody::NonResident {
                        starting_vcn,
                        data_size,
                        initialized_size,
                        allocated_size,
                        ..
                    } => {
                        *starting_vcn == 0
                            && initialized_size <= data_size
                            && data_size <= allocated_size
                    }
                }
        });
    if !record_and_stream_semantics_trusted {
        warnings.push(
            "$Bitmap record or stream semantics are not trustworthy; extent availability unknown"
                .into(),
        );
        return None;
    }

    let expected = boot.total_clusters.div_ceil(8);
    match &data_attribute?.body {
        AttrBody::NonResident {
            data_size,
            initialized_size,
            runlist,
            ..
        } => {
            let runs = match decode_runlist(runlist, boot.total_clusters) {
                Ok(runs) => runs,
                Err(_) => {
                    warnings
                        .push("$Bitmap stream bounds invalid; extent availability unknown".into());
                    return None;
                }
            };
            let trusted_bitmap_prefix = match trusted_physical_stream_prefix_len(
                &runs,
                boot.cluster_size,
                reader.len(),
                *data_size,
                *initialized_size,
            ) {
                Ok(prefix) => prefix,
                Err(_) => {
                    warnings
                        .push("$Bitmap stream bounds invalid; extent availability unknown".into());
                    return None;
                }
            };
            if trusted_bitmap_prefix.len == 0 && expected > 0 {
                warnings.push(
                    "$Bitmap has no trusted physical data; extent availability unknown".into(),
                );
                return None;
            }
            if trusted_bitmap_prefix.stopped_at_sparse_run {
                warnings.push(format!(
                    "$Bitmap trusted prefix stopped before sparse run at {} bytes; later bits unknown",
                    trusted_bitmap_prefix.len
                ));
            } else if trusted_bitmap_prefix.stopped_at_untrusted_run {
                warnings.push(format!(
                    "$Bitmap trusted prefix stopped at {} source-bounded bytes; later bits unknown",
                    trusted_bitmap_prefix.len
                ));
            }
            let take = bounded_bitmap_read_len(expected, trusted_bitmap_prefix.len, warnings);
            match read_stream_range(reader, &runs, boot.cluster_size, 0, take) {
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
        AttrBody::Resident {
            value,
            value_offset_in_record,
        } => {
            let phys = stream_phys_offset(mft_runs, boot.cluster_size, logical)?
                .checked_add(*value_offset_in_record as u64)?;
            let take = usize::try_from(bounded_bitmap_read_len(
                expected,
                value.len() as u64,
                warnings,
            ))
            .ok()?;
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
            let Some(root) = entries.get(&ROOT_RECORD) else {
                warnings.push("root directory record not found; treating as orphan".into());
                return (orphan_path(self_record), MetadataConfidence::Low, warnings);
            };
            if !root.in_use || !root.is_directory {
                warnings.push(
                    "root directory record is not an active directory; treating as orphan".into(),
                );
                return (orphan_path(self_record), MetadataConfidence::Low, warnings);
            }
            if root.sequence != parent_seq {
                warnings.push(format!(
                    "root directory record was reused (expected seq {parent_seq}, found {}); path unreliable",
                    root.sequence
                ));
                return (orphan_path(self_record), MetadataConfidence::Low, warnings);
            }
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
        if !parent.is_directory {
            warnings.push(format!(
                "parent record {parent_no} is not a directory; treating as orphan"
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

    #[test]
    fn ntfs_internal_init_002_bounds_bitmap_and_mft_to_initialized_run_coverage() {
        let runs = vec![RunElement {
            cluster_count: 2,
            lcn: Some(0),
        }];

        assert_eq!(
            trusted_physical_stream_prefix_len(&runs, 4096, 8192, u64::MAX, 1024)
                .unwrap()
                .len,
            1024
        );
        assert_eq!(
            trusted_physical_stream_prefix_len(&runs, 4096, 8192, u64::MAX, u64::MAX)
                .unwrap()
                .len,
            8192
        );

        let mut warnings = Vec::new();
        let map = allocation_map_from_bytes(vec![0xFF], 2, 16, &mut warnings);
        assert_eq!(map.is_allocated(7), Some(true));
        assert_eq!(map.is_allocated(8), None);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("$Bitmap data truncated")));
    }

    #[test]
    fn ntfs_mft_bound_003_caps_record_iteration() {
        let mut warnings = Vec::new();
        let available_bytes = (MAX_MFT_RECORDS + 1) * 1024;

        let count = bounded_mft_record_count(available_bytes, 1024, &mut warnings).unwrap();

        assert_eq!(count, MAX_MFT_RECORDS);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("MFT scan work budget")));
    }

    #[test]
    fn ntfs_mft_retention_004_bounds_entries_extensions_and_warning_growth() {
        let mut warnings = Vec::new();
        let mut complete = true;
        let mut budget = MftRetentionBudget {
            deleted_entries: MAX_RETAINED_DELETED_ENTRIES,
            directory_entries: MAX_RETAINED_DIRECTORY_ENTRIES,
            extension_references: MAX_RETAINED_EXTENSION_REFERENCES,
            extension_streams: MAX_RETAINED_EXTENSION_STREAMS,
            ..MftRetentionBudget::default()
        };

        let no_nested_evidence = EntryRetentionCost {
            names: 0,
            streams: 0,
            run_elements: 0,
        };
        assert!(!budget.admit_base_record(
            false,
            false,
            no_nested_evidence,
            &mut complete,
            &mut warnings
        ));
        assert!(!budget.admit_base_record(
            true,
            true,
            no_nested_evidence,
            &mut complete,
            &mut warnings
        ));
        assert!(!budget.admit_extension_reference(&mut complete, &mut warnings));
        assert!(!budget.admit_extension_streams(1, 0, &mut complete, &mut warnings));
        assert!(!complete);
        assert_eq!(warnings.len(), 4);

        assert!(!budget.admit_base_record(
            false,
            false,
            no_nested_evidence,
            &mut complete,
            &mut warnings
        ));
        assert!(!budget.admit_base_record(
            true,
            true,
            no_nested_evidence,
            &mut complete,
            &mut warnings
        ));
        assert!(!budget.admit_extension_reference(&mut complete, &mut warnings));
        assert!(!budget.admit_extension_streams(1, 0, &mut complete, &mut warnings));
        assert_eq!(
            warnings.len(),
            4,
            "each exhausted budget must retain only one aggregate warning"
        );
    }

    #[test]
    fn ntfs_bitmap_bound_004_caps_hostile_declared_length_before_allocation() {
        let mut warnings = Vec::new();

        let read_len = bounded_bitmap_read_len(u64::MAX, u64::MAX, &mut warnings);

        assert_eq!(read_len, MAX_BITMAP_READ_BYTES);
        assert_eq!(
            warnings,
            vec![format!(
                "$Bitmap read budget capped data at {MAX_BITMAP_READ_BYTES} of {} expected bytes; later bits unknown",
                u64::MAX
            )]
        );
    }

    #[test]
    fn ntfs_sparse_mft_005_stops_trust_at_first_sparse_run() {
        let runs = vec![
            RunElement {
                cluster_count: 2,
                lcn: Some(4),
            },
            RunElement {
                cluster_count: 100,
                lcn: None,
            },
            RunElement {
                cluster_count: 2,
                lcn: Some(20),
            },
        ];

        let prefix =
            trusted_physical_stream_prefix_len(&runs, 4096, 1024 * 4096, u64::MAX, u64::MAX)
                .unwrap();

        assert_eq!(prefix.len, 8192);
        assert!(prefix.stopped_at_untrusted_run);
    }

    #[test]
    fn ntfs_fragmented_mft_006_keeps_non_sparse_runs_trusted() {
        let runs = vec![
            RunElement {
                cluster_count: 2,
                lcn: Some(4),
            },
            RunElement {
                cluster_count: 3,
                lcn: Some(20),
            },
        ];

        let stream_len = 5 * 4096;
        let prefix =
            trusted_physical_stream_prefix_len(&runs, 4096, 1024 * 4096, stream_len, stream_len)
                .unwrap();

        assert_eq!(prefix.len, stream_len);
        assert!(!prefix.stopped_at_untrusted_run);
    }

    fn namespace_name(
        name: &str,
        parent_record: u64,
        parent_sequence: u16,
        namespace: u8,
    ) -> FileNameAttr {
        FileNameAttr {
            parent_record,
            parent_sequence,
            namespace,
            name: name.to_string(),
            logical_size: 0,
            allocated_size: 0,
        }
    }

    fn namespace_entry(
        sequence: u16,
        in_use: bool,
        is_directory: bool,
        names: Vec<FileNameAttr>,
        attributes_complete: bool,
    ) -> ParsedEntry {
        let best_name = pick_best_name(&names);
        ParsedEntry {
            sequence,
            in_use,
            is_directory,
            best_name,
            names,
            attributes_complete,
            std_info: StandardInformation::default(),
            streams: Vec::new(),
            record_phys_offset: 0,
            warnings: Vec::new(),
        }
    }

    fn retention_entry(name_count: usize, stream_run_counts: &[usize]) -> ParsedEntry {
        let names: Vec<FileNameAttr> = (0..name_count)
            .map(|index| {
                namespace_name(
                    &format!("retained-{index}"),
                    ROOT_RECORD,
                    1,
                    crate::attr::NS_WIN32,
                )
            })
            .collect();
        let mut entry = namespace_entry(1, false, false, names, true);
        entry.streams = stream_run_counts
            .iter()
            .map(|&run_count| DataStream {
                name: None,
                flags: 0,
                kind: DataStreamKind::NonResident {
                    data_size: run_count as u64,
                    initialized_size: run_count as u64,
                    runs: vec![
                        RunElement {
                            cluster_count: 1,
                            lcn: Some(1),
                        };
                        run_count
                    ],
                },
            })
            .collect();
        entry
    }

    #[test]
    fn ntfs_mft_retention_005_rejects_entire_base_entry_for_each_nested_budget() {
        fn assert_rejected(mut budget: MftRetentionBudget, entry: ParsedEntry) {
            let name_count = entry.names.len();
            let stream_count = entry.streams.len();
            let run_element_count =
                retained_run_element_count(&entry.streams).expect("small test costs fit");
            let initial_names = budget.entry_names;
            let initial_streams = budget.entry_streams;
            let initial_runs = budget.entry_run_elements;
            let mut entries = HashMap::new();
            let mut complete = true;
            let mut warnings = Vec::new();

            assert!(!retain_base_entry(
                42,
                entry,
                &mut entries,
                &mut budget,
                &mut complete,
                &mut warnings,
            ));
            assert!(entries.is_empty(), "an over-budget entry must be dropped");
            assert_eq!(budget.deleted_entries, 0);
            assert_eq!(budget.directory_entries, 0);
            assert_eq!(budget.entry_names, initial_names);
            assert_eq!(budget.entry_streams, initial_streams);
            assert_eq!(budget.entry_run_elements, initial_runs);
            assert!(!complete);
            assert_eq!(warnings.len(), 1);
            assert!(warnings[0].contains("retained-entry evidence budget"));

            assert!(!budget.admit_base_record(
                false,
                false,
                EntryRetentionCost {
                    names: name_count,
                    streams: stream_count,
                    run_elements: run_element_count,
                },
                &mut complete,
                &mut warnings,
            ));
            assert_eq!(
                warnings.len(),
                1,
                "nested-budget saturation must retain one aggregate warning"
            );
        }

        assert_rejected(
            MftRetentionBudget {
                entry_names: MAX_RETAINED_ENTRY_NAMES - 1,
                ..MftRetentionBudget::default()
            },
            retention_entry(2, &[]),
        );
        assert_rejected(
            MftRetentionBudget {
                entry_streams: MAX_RETAINED_ENTRY_STREAMS,
                ..MftRetentionBudget::default()
            },
            retention_entry(1, &[0]),
        );
        assert_rejected(
            MftRetentionBudget {
                entry_run_elements: MAX_RETAINED_ENTRY_RUN_ELEMENTS - 1,
                ..MftRetentionBudget::default()
            },
            retention_entry(1, &[2]),
        );
    }

    #[test]
    fn ntfs_mft_retention_006_extension_merge_obeys_global_nested_budget() {
        let mut warnings = Vec::new();
        let mut complete = true;
        let mut budget = MftRetentionBudget {
            entry_streams: MAX_RETAINED_ENTRY_STREAMS,
            ..MftRetentionBudget::default()
        };

        assert!(!budget.admit_extension_streams(1, 0, &mut complete, &mut warnings));
        assert_eq!(budget.extension_streams, 0);
        assert_eq!(warnings.len(), 1);

        budget.entry_streams = 0;
        budget.entry_run_elements = usize::MAX;
        assert!(!budget.admit_extension_streams(1, 1, &mut complete, &mut warnings));
        assert_eq!(budget.extension_streams, 0);
        assert_eq!(
            warnings.len(),
            1,
            "all nested evidence limits share one aggregate warning"
        );
        assert!(!complete);
    }

    fn namespace_root(entries: &mut HashMap<u64, ParsedEntry>) -> NtfsNodeRef {
        let root = NtfsNodeRef {
            record: ROOT_RECORD,
            sequence: 5,
        };
        entries.insert(
            ROOT_RECORD,
            namespace_entry(root.sequence, true, true, Vec::new(), true),
        );
        root
    }

    fn add_active_directory(
        entries: &mut HashMap<u64, ParsedEntry>,
        record: u64,
        sequence: u16,
        name: &str,
        parent: NtfsNodeRef,
    ) -> NtfsNodeRef {
        let node = NtfsNodeRef { record, sequence };
        entries.insert(
            record,
            namespace_entry(
                sequence,
                true,
                true,
                vec![namespace_name(
                    name,
                    parent.record,
                    parent.sequence,
                    crate::attr::NS_WIN32,
                )],
                true,
            ),
        );
        node
    }

    #[test]
    fn ntfs_namespace_hardlink_001_preserves_every_useful_non_dos_name() {
        let mut entries = HashMap::new();
        let root = namespace_root(&mut entries);
        let alpha = add_active_directory(&mut entries, 20, 1, "Alpha", root);
        let beta = add_active_directory(&mut entries, 30, 1, "Beta", root);
        let gamma = add_active_directory(&mut entries, 35, 1, "Gamma", root);
        let candidate = NtfsNodeRef {
            record: 40,
            sequence: 7,
        };
        entries.insert(
            candidate.record,
            namespace_entry(
                candidate.sequence,
                false,
                false,
                vec![
                    namespace_name(
                        "linked.txt",
                        alpha.record,
                        alpha.sequence,
                        crate::attr::NS_WIN32,
                    ),
                    namespace_name(
                        "alias.txt",
                        beta.record,
                        beta.sequence,
                        crate::attr::NS_POSIX,
                    ),
                    namespace_name("LINKED~1TXT", alpha.record, alpha.sequence, NS_DOS),
                ],
                true,
            ),
        );

        let mut warnings = Vec::new();
        let namespace = build_namespace(&entries, true, &mut warnings);
        let candidate_paths: Vec<&NtfsNamespacePath> = namespace
            .paths
            .iter()
            .filter(|path| path.node == candidate)
            .collect();

        assert_eq!(candidate_paths.len(), 2);
        assert!(candidate_paths
            .iter()
            .any(|path| path.name == "linked.txt" && path.parent_path == ["Alpha"]));
        assert!(candidate_paths
            .iter()
            .any(|path| path.name == "alias.txt" && path.parent_path == ["Beta"]));
        assert_eq!(
            namespace.classify_candidate(candidate, alpha),
            NtfsScopeMembership::Match
        );
        assert_eq!(
            namespace.classify_candidate(candidate, beta),
            NtfsScopeMembership::Match
        );
        assert_eq!(
            namespace.classify_candidate(candidate, gamma),
            NtfsScopeMembership::NoMatch
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn ntfs_namespace_work_bound_001_stops_before_expanding_a_saturated_sibling() {
        let mut entries = HashMap::new();
        let mut parent = namespace_root(&mut entries);
        for level in 0..12_u64 {
            let node = NtfsNodeRef {
                record: 100 + level,
                sequence: 1,
            };
            entries.insert(
                node.record,
                namespace_entry(
                    node.sequence,
                    true,
                    true,
                    vec![
                        namespace_name(
                            &format!("left-{level}"),
                            parent.record,
                            parent.sequence,
                            crate::attr::NS_WIN32,
                        ),
                        namespace_name(
                            &format!("right-{level}"),
                            parent.record,
                            parent.sequence,
                            crate::attr::NS_POSIX,
                        ),
                    ],
                    true,
                ),
            );
            parent = node;
        }

        NAMESPACE_EXPANSION_CALLS.with(|calls| calls.set(0));
        let expansion =
            expand_parent_paths(&entries, parent.record, parent.sequence, &HashSet::new(), 0);
        let calls = NAMESPACE_EXPANSION_CALLS.with(std::cell::Cell::get);

        assert_eq!(expansion.paths.len(), MAX_NAMESPACE_PATHS_PER_NAME);
        assert!(!expansion.complete);
        assert!(
            calls <= 600,
            "saturated sibling expansion exceeded the recursive work bound: {calls} calls"
        );
    }

    #[test]
    fn ntfs_namespace_parent_validation_001_marks_invalid_ancestry_orphaned() {
        let mut entries = HashMap::new();
        let root = namespace_root(&mut entries);
        let target = add_active_directory(&mut entries, 20, 1, "Target", root);
        entries.insert(
            50,
            namespace_entry(
                1,
                true,
                false,
                vec![namespace_name(
                    "not-a-directory",
                    root.record,
                    root.sequence,
                    crate::attr::NS_WIN32,
                )],
                true,
            ),
        );
        entries.insert(
            60,
            namespace_entry(
                9,
                true,
                true,
                vec![namespace_name(
                    "reused",
                    root.record,
                    root.sequence,
                    crate::attr::NS_WIN32,
                )],
                true,
            ),
        );

        let invalid = [
            (
                NtfsNodeRef {
                    record: 40,
                    sequence: 1,
                },
                namespace_name("root-sequence.txt", ROOT_RECORD, 4, crate::attr::NS_WIN32),
            ),
            (
                NtfsNodeRef {
                    record: 41,
                    sequence: 1,
                },
                namespace_name("missing.txt", 99, 1, crate::attr::NS_WIN32),
            ),
            (
                NtfsNodeRef {
                    record: 42,
                    sequence: 1,
                },
                namespace_name("file-parent.txt", 50, 1, crate::attr::NS_WIN32),
            ),
            (
                NtfsNodeRef {
                    record: 43,
                    sequence: 1,
                },
                namespace_name("reused-parent.txt", 60, 1, crate::attr::NS_WIN32),
            ),
        ];
        for (node, name) in &invalid {
            entries.insert(
                node.record,
                namespace_entry(node.sequence, false, false, vec![name.clone()], true),
            );
        }

        let namespace = build_namespace(&entries, true, &mut Vec::new());
        for (node, _) in invalid {
            let path = namespace
                .paths
                .iter()
                .find(|path| path.node == node)
                .expect("invalid candidate path retained as evidence");
            assert_eq!(path.state, NtfsPathState::Orphaned);
            assert_eq!(
                namespace.classify_candidate(node, target),
                NtfsScopeMembership::Unknown
            );
        }
    }

    #[test]
    fn ntfs_primary_path_parent_validation_001_checks_root_sequence_and_directory_kind() {
        let mut entries = HashMap::new();
        let root = namespace_root(&mut entries);
        entries.insert(
            50,
            namespace_entry(
                1,
                true,
                false,
                vec![namespace_name(
                    "ordinary-file",
                    root.record,
                    root.sequence,
                    crate::attr::NS_WIN32,
                )],
                true,
            ),
        );

        for name in [
            namespace_name("wrong-root.txt", ROOT_RECORD, 4, crate::attr::NS_WIN32),
            namespace_name("file-parent.txt", 50, 1, crate::attr::NS_WIN32),
        ] {
            let (path, confidence, _) = reconstruct_path(&entries, &name, 80);
            assert_eq!(confidence, MetadataConfidence::Low);
            assert_eq!(path, orphan_path(80));
        }
    }

    #[test]
    fn ntfs_namespace_directory_resolution_001_rejects_ambiguous_display_path() {
        let mut entries = HashMap::new();
        let root = namespace_root(&mut entries);
        add_active_directory(&mut entries, 20, 1, "Docs", root);
        add_active_directory(&mut entries, 21, 1, "Docs", root);

        let namespace = build_namespace(&entries, true, &mut Vec::new());
        assert_eq!(
            namespace.resolve_active_directory(&["Docs".to_string()]),
            NtfsDirectoryResolution::Ambiguous
        );
    }

    #[test]
    fn ntfs_namespace_directory_resolution_002_accepts_one_proven_path_in_partial_scan() {
        let mut entries = HashMap::new();
        let root = namespace_root(&mut entries);
        let expected = add_active_directory(&mut entries, 20, 1, "Docs", root);

        let namespace = build_namespace(&entries, false, &mut Vec::new());
        assert!(!namespace.is_complete);
        assert_eq!(
            namespace.resolve_active_directory(&["Docs".to_string()]),
            NtfsDirectoryResolution::Unique(expected),
            "global MFT incompleteness must preserve a positively proven active directory; only negative membership stays unknown"
        );
    }

    #[test]
    fn ntfs_namespace_scope_unknown_001_never_becomes_no_match() {
        let mut entries = HashMap::new();
        let root = namespace_root(&mut entries);
        let target = add_active_directory(&mut entries, 20, 1, "Target", root);
        let candidate = NtfsNodeRef {
            record: 40,
            sequence: 1,
        };
        entries.insert(
            candidate.record,
            namespace_entry(
                candidate.sequence,
                false,
                false,
                vec![namespace_name("unknown.txt", 999, 1, crate::attr::NS_WIN32)],
                true,
            ),
        );

        let namespace = build_namespace(&entries, true, &mut Vec::new());
        assert_eq!(
            namespace.classify_candidate(candidate, target),
            NtfsScopeMembership::Unknown
        );
    }

    #[test]
    fn ntfs_namespace_index_001_groups_offsets_by_node_for_bounded_lookup() {
        let target = NtfsNodeRef {
            record: 50_000,
            sequence: 9,
        };
        let directory = NtfsNodeRef {
            record: 42,
            sequence: 3,
        };
        let mut paths = (0..4_096_u64)
            .map(|record| NtfsNamespacePath {
                node: NtfsNodeRef {
                    record,
                    sequence: 1,
                },
                namespace: crate::attr::NS_WIN32,
                name: format!("unrelated-{record}.bin"),
                parent_path: Vec::new(),
                ancestors: vec![NtfsNodeRef {
                    record: ROOT_RECORD,
                    sequence: 5,
                }],
                state: NtfsPathState::Exact,
            })
            .collect::<Vec<_>>();
        paths.push(NtfsNamespacePath {
            node: target,
            namespace: crate::attr::NS_WIN32,
            name: "target.bin".to_owned(),
            parent_path: vec!["Documents".to_owned()],
            ancestors: vec![
                NtfsNodeRef {
                    record: ROOT_RECORD,
                    sequence: 5,
                },
                directory,
            ],
            state: NtfsPathState::Exact,
        });
        let namespace = NtfsNamespace {
            paths,
            directories: Vec::new(),
            is_complete: true,
        };

        let index = namespace.build_index().expect("bounded namespace index");

        assert_eq!(index.indexed_node_count(), 4_097);
        assert_eq!(
            index.candidate_path_evidence(target),
            NtfsCandidatePathEvidence::One(NtfsPathState::Exact)
        );
        assert_eq!(
            index.classify_candidate(target, directory),
            NtfsScopeMembership::Match
        );
    }

    #[test]
    fn ntfs_namespace_index_002_rejects_externally_unbounded_path_evidence() {
        let path = NtfsNamespacePath {
            node: NtfsNodeRef {
                record: 40,
                sequence: 1,
            },
            namespace: crate::attr::NS_WIN32,
            name: "candidate.bin".to_owned(),
            parent_path: Vec::new(),
            ancestors: Vec::new(),
            state: NtfsPathState::Exact,
        };
        let namespace = NtfsNamespace {
            paths: vec![path; MAX_NAMESPACE_PATHS + 1],
            directories: Vec::new(),
            is_complete: true,
        };

        assert_eq!(
            namespace.build_index().expect_err("unbounded index"),
            NtfsNamespaceIndexError::TooManyPaths
        );
    }
}
