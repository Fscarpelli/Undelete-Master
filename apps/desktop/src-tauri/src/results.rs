use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use um_core::{
    Candidate, CandidateId, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability,
    MetadataConfidence,
};

pub(crate) const CANDIDATE_PAGE_LIMIT: usize = 100;
const RESULT_SCHEMA_VERSION: u32 = 1;
const MAX_QUERY_TEXT_SCALARS: usize = 512;
const MAX_QUERY_EXTENSIONS: usize = 128;
const MAX_EXTENSION_SCALARS: usize = 255;
// NTFS metadata retains at most 100,000 deleted candidates and a deep scan can
// append at most 10,000 distinct carved candidates.
pub(crate) const MAX_RETAINED_CANDIDATES_PER_SCAN: usize = 110_000;
const MAX_DIRECT_SELECTION_IDS: usize = 100;

pub(crate) fn retained_candidate_count_within_bound(candidate_count: usize) -> bool {
    candidate_count <= MAX_RETAINED_CANDIDATES_PER_SCAN
}

#[derive(Debug, Clone)]
pub(crate) struct StoredCandidate {
    pub(crate) candidate: Candidate,
    pub(crate) row: CandidateRowDto,
    #[allow(dead_code)] // Retained native-only authority consumed by the restore-planning slice.
    pub(crate) expected_sha256: Option<[u8; 32]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScanSourceBinding {
    pub(crate) inventory_generation: String,
    pub(crate) volume_id: String,
    pub(crate) source_len: u64,
    pub(crate) file_system: String,
    pub(crate) physical_disk_number: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RecoveryEligibility {
    Complete,
    BestEffort,
    Ineligible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CandidateRowDto {
    pub(crate) id: String,
    pub(crate) display_path: String,
    pub(crate) kind: &'static str,
    pub(crate) state: &'static str,
    pub(crate) size_bytes: String,
    pub(crate) metadata_confidence: &'static str,
    pub(crate) recoverability_score: Option<u8>,
    pub(crate) path_state: &'static str,
    pub(crate) method: &'static str,
    pub(crate) content_sha256: Option<String>,
    pub(crate) validator: Option<String>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CandidateKindDto {
    File,
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MetadataConfidenceDto {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DiscoveryMethodDto {
    NtfsMetadata,
    FatMetadata,
    ExfatMetadata,
    Carving,
    RecycleBin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CandidateStateDto {
    ExactEvidence,
    LikelyComplete,
    CompleteUnvalidated,
    StructurallyValid,
    Partial,
    Conflicted,
    ReadError,
    ZeroedOrTrimmed,
    Overwritten,
    MetadataOnly,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CandidateQuery {
    pub(crate) revision: String,
    pub(crate) search: String,
    pub(crate) extensions: Vec<String>,
    pub(crate) kinds: Vec<CandidateKindDto>,
    pub(crate) metadata_confidences: Vec<MetadataConfidenceDto>,
    pub(crate) methods: Vec<DiscoveryMethodDto>,
    pub(crate) states: Vec<CandidateStateDto>,
    pub(crate) min_recoverability_score: Option<u8>,
    pub(crate) max_recoverability_score: Option<u8>,
    pub(crate) eligibilities: Vec<RecoveryEligibility>,
    pub(crate) selected_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CandidateSortField {
    Path,
    Extension,
    Size,
    State,
    Confidence,
    RecoverabilityScore,
    Method,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CandidateSortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CandidateSort {
    pub(crate) field: CandidateSortField,
    pub(crate) direction: CandidateSortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtensionFacetDto {
    pub(crate) extension: String,
    pub(crate) count: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CandidateSelectionSummaryDto {
    pub(crate) selection_revision: String,
    pub(crate) selected_candidates: String,
    pub(crate) selected_files: String,
    pub(crate) selected_directories: String,
    pub(crate) selected_logical_bytes: String,
    pub(crate) best_effort_candidates: String,
    pub(crate) conflicted_candidates: String,
    pub(crate) ineligible_candidates: String,
    pub(crate) matching_selected_candidates: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionableCandidateRowDto {
    pub(crate) id: String,
    pub(crate) display_path: String,
    pub(crate) extension: String,
    pub(crate) kind: &'static str,
    pub(crate) state: &'static str,
    pub(crate) size_bytes: String,
    pub(crate) metadata_confidence: &'static str,
    pub(crate) recoverability_score: Option<u8>,
    pub(crate) path_state: &'static str,
    pub(crate) method: &'static str,
    pub(crate) eligibility: RecoveryEligibility,
    pub(crate) selected: bool,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CandidateQueryPageDto {
    pub(crate) schema_version: u32,
    pub(crate) scan_id: String,
    pub(crate) query_id: String,
    pub(crate) query_revision: String,
    pub(crate) cursor: Option<String>,
    pub(crate) next_cursor: Option<String>,
    pub(crate) filtered_total: String,
    pub(crate) extension_facets: Vec<ExtensionFacetDto>,
    pub(crate) selection: CandidateSelectionSummaryDto,
    pub(crate) candidates: Vec<ActionableCandidateRowDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum SelectionOperationDto {
    SetIds {
        candidate_ids: Vec<String>,
        selected: bool,
    },
    SelectAllMatching,
    ClearMatching,
    ClearAll,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CandidateSelectionUpdateDto {
    pub(crate) schema_version: u32,
    pub(crate) scan_id: String,
    pub(crate) query_id: String,
    pub(crate) selection_revision: String,
    pub(crate) selection: CandidateSelectionSummaryDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultAuthorityError {
    InvalidQuery,
    StaleCursor,
    StaleSelection,
    UnknownCandidate,
    Overflow,
}

#[derive(Debug, Clone)]
pub(crate) struct BoundCandidateQuery {
    query_id: String,
    source_fingerprint: String,
    query: CandidateQuery,
    sort: CandidateSort,
    selection_revision: u64,
    cursors: HashMap<String, usize>,
}

pub(crate) struct CandidateQueryContext<'a> {
    pub(crate) scan_id: &'a str,
    pub(crate) source: &'a ScanSourceBinding,
    pub(crate) candidates: &'a [StoredCandidate],
    pub(crate) selected: &'a HashSet<CandidateId>,
    pub(crate) selection_revision: u64,
    pub(crate) active_query: &'a mut Option<BoundCandidateQuery>,
}

pub(crate) fn query_candidate_page(
    context: CandidateQueryContext<'_>,
    query: CandidateQuery,
    sort: CandidateSort,
    cursor: Option<&str>,
) -> Result<CandidateQueryPageDto, ResultAuthorityError> {
    let query = canonical_query(query)?;
    let source_fingerprint = source_fingerprint(context.scan_id, context.source)?;
    let query_id = derive_id(
        "query",
        &[
            source_fingerprint.as_bytes(),
            &serde_json::to_vec(&query).map_err(|_| ResultAuthorityError::InvalidQuery)?,
        ],
    );
    let offset = if let Some(cursor) = cursor {
        let binding = context
            .active_query
            .as_ref()
            .filter(|binding| {
                binding.query_id == query_id
                    && binding.source_fingerprint == source_fingerprint
                    && binding.query == query
                    && binding.sort == sort
                    && (!query.selected_only
                        || binding.selection_revision == context.selection_revision)
            })
            .ok_or(ResultAuthorityError::StaleCursor)?;
        binding
            .cursors
            .get(cursor)
            .copied()
            .ok_or(ResultAuthorityError::StaleCursor)?
    } else {
        *context.active_query = Some(BoundCandidateQuery {
            query_id: query_id.clone(),
            source_fingerprint: source_fingerprint.clone(),
            query: query.clone(),
            sort,
            selection_revision: context.selection_revision,
            cursors: HashMap::new(),
        });
        0
    };

    let mut filtered = context
        .candidates
        .iter()
        .filter(|candidate| candidate_matches(candidate, &query, context.selected))
        .collect::<Vec<_>>();
    filtered.sort_by(|left, right| compare_candidates(left, right, sort));
    let filtered_total =
        u64::try_from(filtered.len()).map_err(|_| ResultAuthorityError::Overflow)?;
    let end = offset
        .checked_add(CANDIDATE_PAGE_LIMIT)
        .map(|end| end.min(filtered.len()))
        .ok_or(ResultAuthorityError::Overflow)?;
    let page = filtered
        .get(offset..end)
        .ok_or(ResultAuthorityError::StaleCursor)?;
    let next_cursor = if end < filtered.len() {
        let sort_bytes =
            serde_json::to_vec(&sort).map_err(|_| ResultAuthorityError::InvalidQuery)?;
        let cursor = derive_id(
            "cursor",
            &[
                source_fingerprint.as_bytes(),
                query_id.as_bytes(),
                &sort_bytes,
                &if query.selected_only {
                    context.selection_revision
                } else {
                    0
                }
                .to_le_bytes(),
                &u64::try_from(end)
                    .map_err(|_| ResultAuthorityError::Overflow)?
                    .to_le_bytes(),
            ],
        );
        context
            .active_query
            .as_mut()
            .ok_or(ResultAuthorityError::StaleCursor)?
            .cursors
            .insert(cursor.clone(), end);
        Some(cursor)
    } else {
        None
    };
    let selection = selection_summary(
        context.candidates,
        context.selected,
        context.selection_revision,
        Some(&query),
    )?;

    Ok(CandidateQueryPageDto {
        schema_version: RESULT_SCHEMA_VERSION,
        scan_id: context.scan_id.to_owned(),
        query_id,
        query_revision: query.revision,
        cursor: cursor.map(str::to_owned),
        next_cursor,
        filtered_total: filtered_total.to_string(),
        extension_facets: extension_facets(context.candidates)?,
        selection,
        candidates: page
            .iter()
            .map(|candidate| actionable_row(candidate, context.selected))
            .collect(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_candidate_selection(
    scan_id: &str,
    candidates: &[StoredCandidate],
    candidate_index: &HashMap<CandidateId, usize>,
    selected_candidates: &mut HashSet<CandidateId>,
    selection_revision: &mut u64,
    active_query: Option<&BoundCandidateQuery>,
    query_id: &str,
    operation: SelectionOperationDto,
    expected_revision: u64,
) -> Result<CandidateSelectionUpdateDto, ResultAuthorityError> {
    if *selection_revision != expected_revision {
        return Err(ResultAuthorityError::StaleSelection);
    }
    let binding = active_query
        .filter(|binding| binding.query_id == query_id)
        .ok_or(ResultAuthorityError::InvalidQuery)?;
    let next_revision = selection_revision
        .checked_add(1)
        .ok_or(ResultAuthorityError::Overflow)?;
    let mut proposed_selection = selected_candidates.clone();

    match operation {
        SelectionOperationDto::SetIds {
            candidate_ids,
            selected,
        } => {
            if candidate_ids.is_empty() || candidate_ids.len() > MAX_DIRECT_SELECTION_IDS {
                return Err(ResultAuthorityError::InvalidQuery);
            }
            let mut parsed = Vec::with_capacity(candidate_ids.len());
            let mut unique = HashSet::with_capacity(candidate_ids.len());
            for candidate_id in candidate_ids {
                let id = parse_decimal_u64(&candidate_id)?;
                if !unique.insert(id) || !candidate_index.contains_key(&id) {
                    return Err(ResultAuthorityError::UnknownCandidate);
                }
                parsed.push(id);
            }
            for id in parsed {
                if selected {
                    proposed_selection.insert(id);
                } else {
                    proposed_selection.remove(&id);
                }
            }
        }
        SelectionOperationDto::SelectAllMatching => {
            let matching = candidates
                .iter()
                .filter(|candidate| {
                    candidate_matches(candidate, &binding.query, &proposed_selection)
                })
                .map(|candidate| candidate.candidate.id)
                .collect::<Vec<_>>();
            for id in matching {
                proposed_selection.insert(id);
            }
        }
        SelectionOperationDto::ClearMatching => {
            let matching = candidates
                .iter()
                .filter(|candidate| {
                    candidate_matches(candidate, &binding.query, &proposed_selection)
                })
                .map(|candidate| candidate.candidate.id)
                .collect::<Vec<_>>();
            for id in matching {
                proposed_selection.remove(&id);
            }
        }
        SelectionOperationDto::ClearAll => proposed_selection.clear(),
    }
    let selection = selection_summary(
        candidates,
        &proposed_selection,
        next_revision,
        Some(&binding.query),
    )?;
    *selected_candidates = proposed_selection;
    *selection_revision = next_revision;
    Ok(CandidateSelectionUpdateDto {
        schema_version: RESULT_SCHEMA_VERSION,
        scan_id: scan_id.to_owned(),
        query_id: query_id.to_owned(),
        selection_revision: selection_revision.to_string(),
        selection,
    })
}

fn canonical_query(mut query: CandidateQuery) -> Result<CandidateQuery, ResultAuthorityError> {
    query.revision = parse_decimal_u64(&query.revision)?.to_string();
    if query.search.chars().count() > MAX_QUERY_TEXT_SCALARS
        || query.search.chars().any(forbidden_query_character)
        || query.extensions.len() > MAX_QUERY_EXTENSIONS
    {
        return Err(ResultAuthorityError::InvalidQuery);
    }
    query.search = query.search.to_lowercase();
    for extension in &mut query.extensions {
        if !extension.is_empty() {
            *extension =
                canonical_extension(extension).ok_or(ResultAuthorityError::InvalidQuery)?;
        }
    }
    query.extensions.sort();
    query.extensions.dedup();
    canonicalize_enum_list(&mut query.kinds);
    canonicalize_enum_list(&mut query.metadata_confidences);
    canonicalize_enum_list(&mut query.methods);
    canonicalize_enum_list(&mut query.states);
    canonicalize_enum_list(&mut query.eligibilities);
    if query
        .min_recoverability_score
        .is_some_and(|score| score > 100)
        || query
            .max_recoverability_score
            .is_some_and(|score| score > 100)
        || matches!(
            (
                query.min_recoverability_score,
                query.max_recoverability_score
            ),
            (Some(min), Some(max)) if min > max
        )
    {
        return Err(ResultAuthorityError::InvalidQuery);
    }
    Ok(query)
}

fn canonicalize_enum_list<T: Ord>(values: &mut Vec<T>) {
    values.sort();
    values.dedup();
}

fn forbidden_query_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{061C}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{202A}'
                | '\u{202B}'
                | '\u{202C}'
                | '\u{202D}'
                | '\u{202E}'
                | '\u{2066}'
                | '\u{2067}'
                | '\u{2068}'
                | '\u{2069}'
                | '\u{FEFF}'
        )
}

fn candidate_matches(
    stored: &StoredCandidate,
    query: &CandidateQuery,
    selected: &HashSet<CandidateId>,
) -> bool {
    let candidate = &stored.candidate;
    let score = stored.row.recoverability_score;
    (query.search.is_empty()
        || stored
            .row
            .display_path
            .to_lowercase()
            .contains(&query.search))
        && (query.extensions.is_empty()
            || query.extensions.contains(&candidate_extension(candidate)))
        && (query.kinds.is_empty()
            || query
                .kinds
                .contains(&CandidateKindDto::from(candidate.kind)))
        && (query.metadata_confidences.is_empty()
            || query
                .metadata_confidences
                .contains(&MetadataConfidenceDto::from(candidate.metadata_confidence)))
        && (query.methods.is_empty()
            || query
                .methods
                .contains(&DiscoveryMethodDto::from(candidate.method)))
        && (query.states.is_empty()
            || query
                .states
                .contains(&CandidateStateDto::from(candidate.state)))
        && query
            .min_recoverability_score
            .is_none_or(|minimum| score.is_some_and(|score| score >= minimum))
        && query
            .max_recoverability_score
            .is_none_or(|maximum| score.is_some_and(|score| score <= maximum))
        && (query.eligibilities.is_empty()
            || query
                .eligibilities
                .contains(&candidate_eligibility(candidate)))
        && (!query.selected_only || selected.contains(&candidate.id))
}

fn compare_candidates(
    left: &StoredCandidate,
    right: &StoredCandidate,
    sort: CandidateSort,
) -> Ordering {
    let primary = match sort.field {
        CandidateSortField::Path => left
            .row
            .display_path
            .to_lowercase()
            .cmp(&right.row.display_path.to_lowercase()),
        CandidateSortField::Extension => {
            candidate_extension(&left.candidate).cmp(&candidate_extension(&right.candidate))
        }
        CandidateSortField::Size => left.candidate.size.cmp(&right.candidate.size),
        CandidateSortField::State => CandidateStateDto::from(left.candidate.state)
            .cmp(&CandidateStateDto::from(right.candidate.state)),
        CandidateSortField::Confidence => {
            MetadataConfidenceDto::from(left.candidate.metadata_confidence).cmp(
                &MetadataConfidenceDto::from(right.candidate.metadata_confidence),
            )
        }
        CandidateSortField::RecoverabilityScore => {
            return compare_nullable_scores(left, right, sort.direction)
                .then_with(|| left.candidate.id.cmp(&right.candidate.id));
        }
        CandidateSortField::Method => DiscoveryMethodDto::from(left.candidate.method)
            .cmp(&DiscoveryMethodDto::from(right.candidate.method)),
    };
    let directed = match sort.direction {
        CandidateSortDirection::Ascending => primary,
        CandidateSortDirection::Descending => primary.reverse(),
    };
    directed.then_with(|| left.candidate.id.cmp(&right.candidate.id))
}

fn compare_nullable_scores(
    left: &StoredCandidate,
    right: &StoredCandidate,
    direction: CandidateSortDirection,
) -> Ordering {
    match (
        left.row.recoverability_score,
        right.row.recoverability_score,
    ) {
        (Some(left), Some(right)) => match direction {
            CandidateSortDirection::Ascending => left.cmp(&right),
            CandidateSortDirection::Descending => right.cmp(&left),
        },
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn extension_facets(
    candidates: &[StoredCandidate],
) -> Result<Vec<ExtensionFacetDto>, ResultAuthorityError> {
    let mut counts = BTreeMap::<String, u64>::new();
    for candidate in candidates {
        let extension = candidate_extension(&candidate.candidate);
        if !counts.contains_key(&extension) && counts.len() == MAX_RETAINED_CANDIDATES_PER_SCAN {
            return Err(ResultAuthorityError::Overflow);
        }
        let count = counts.entry(extension).or_default();
        *count = count.checked_add(1).ok_or(ResultAuthorityError::Overflow)?;
    }
    Ok(counts
        .into_iter()
        .map(|(extension, count)| ExtensionFacetDto {
            extension,
            count: count.to_string(),
        })
        .collect())
}

fn selection_summary(
    candidates: &[StoredCandidate],
    selected: &HashSet<CandidateId>,
    revision: u64,
    matching_query: Option<&CandidateQuery>,
) -> Result<CandidateSelectionSummaryDto, ResultAuthorityError> {
    let mut selected_candidates = 0_u64;
    let mut selected_files = 0_u64;
    let mut selected_directories = 0_u64;
    let mut selected_logical_bytes = 0_u64;
    let mut best_effort_candidates = 0_u64;
    let mut conflicted_candidates = 0_u64;
    let mut ineligible_candidates = 0_u64;
    let mut matching_selected_candidates = 0_u64;

    for stored in candidates {
        let is_selected = selected.contains(&stored.candidate.id);
        if is_selected {
            selected_candidates = checked_increment(selected_candidates)?;
            match stored.candidate.kind {
                CandidateKind::File => selected_files = checked_increment(selected_files)?,
                CandidateKind::Directory => {
                    selected_directories = checked_increment(selected_directories)?;
                }
            }
            selected_logical_bytes = selected_logical_bytes
                .checked_add(stored.candidate.size)
                .ok_or(ResultAuthorityError::Overflow)?;
            match candidate_eligibility(&stored.candidate) {
                RecoveryEligibility::Complete => {}
                RecoveryEligibility::BestEffort => {
                    best_effort_candidates = checked_increment(best_effort_candidates)?;
                }
                RecoveryEligibility::Ineligible => {
                    ineligible_candidates = checked_increment(ineligible_candidates)?;
                }
            }
            if stored.candidate.state == CandidateState::Conflicted {
                conflicted_candidates = checked_increment(conflicted_candidates)?;
            }
        }
        if is_selected
            && matching_query.is_some_and(|query| candidate_matches(stored, query, selected))
        {
            matching_selected_candidates = checked_increment(matching_selected_candidates)?;
        }
    }
    Ok(CandidateSelectionSummaryDto {
        selection_revision: revision.to_string(),
        selected_candidates: selected_candidates.to_string(),
        selected_files: selected_files.to_string(),
        selected_directories: selected_directories.to_string(),
        selected_logical_bytes: selected_logical_bytes.to_string(),
        best_effort_candidates: best_effort_candidates.to_string(),
        conflicted_candidates: conflicted_candidates.to_string(),
        ineligible_candidates: ineligible_candidates.to_string(),
        matching_selected_candidates: matching_selected_candidates.to_string(),
    })
}

fn checked_increment(value: u64) -> Result<u64, ResultAuthorityError> {
    value.checked_add(1).ok_or(ResultAuthorityError::Overflow)
}

fn actionable_row(
    stored: &StoredCandidate,
    selected: &HashSet<CandidateId>,
) -> ActionableCandidateRowDto {
    ActionableCandidateRowDto {
        id: stored.candidate.id.to_string(),
        display_path: stored.row.display_path.clone(),
        extension: candidate_extension(&stored.candidate),
        kind: stored.row.kind,
        state: stored.row.state,
        size_bytes: stored.row.size_bytes.clone(),
        metadata_confidence: stored.row.metadata_confidence,
        recoverability_score: stored.row.recoverability_score,
        path_state: stored.row.path_state,
        method: stored.row.method,
        eligibility: candidate_eligibility(&stored.candidate),
        selected: selected.contains(&stored.candidate.id),
        warnings: stored.row.warnings.clone(),
    }
}

fn candidate_extension(candidate: &Candidate) -> String {
    let Some((stem, extension)) = candidate.name.rsplit_once('.') else {
        return String::new();
    };
    if stem.is_empty() || extension.is_empty() {
        String::new()
    } else {
        canonical_extension(extension).unwrap_or_default()
    }
}

fn canonical_extension(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_EXTENSION_SCALARS
        || trimmed.chars().any(forbidden_query_character)
        || trimmed.contains(['.', '/', '\\'])
    {
        None
    } else {
        let normalized = trimmed.to_lowercase();
        (normalized.chars().count() <= MAX_EXTENSION_SCALARS).then_some(normalized)
    }
}

pub(crate) fn candidate_eligibility(candidate: &Candidate) -> RecoveryEligibility {
    if candidate.kind == CandidateKind::Directory || candidate.size == 0 {
        return RecoveryEligibility::Complete;
    }
    if matches!(
        candidate.state,
        CandidateState::Overwritten | CandidateState::MetadataOnly | CandidateState::Unknown
    ) {
        return RecoveryEligibility::Ineligible;
    }
    let has_readable = candidate.extents.iter().any(|extent| {
        matches!(
            extent.availability,
            ExtentAvailability::FreeInSnapshot
                | ExtentAvailability::CurrentlyAllocated
                | ExtentAvailability::Zeroed
                | ExtentAvailability::Resident
                | ExtentAvailability::Sparse
        )
    });
    if !has_readable {
        return RecoveryEligibility::Ineligible;
    }
    let all_complete = !candidate.has_missing_ranges()
        && candidate.extents.iter().all(|extent| {
            matches!(
                extent.availability,
                ExtentAvailability::FreeInSnapshot
                    | ExtentAvailability::Resident
                    | ExtentAvailability::Sparse
            )
        });
    if all_complete {
        RecoveryEligibility::Complete
    } else {
        RecoveryEligibility::BestEffort
    }
}

fn parse_decimal_u64(value: &str) -> Result<u64, ResultAuthorityError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(ResultAuthorityError::InvalidQuery);
    }
    value
        .parse()
        .map_err(|_| ResultAuthorityError::InvalidQuery)
}

fn source_fingerprint(
    scan_id: &str,
    source: &ScanSourceBinding,
) -> Result<String, ResultAuthorityError> {
    if scan_id.is_empty()
        || source.inventory_generation.is_empty()
        || source.volume_id.is_empty()
        || source.file_system.is_empty()
    {
        return Err(ResultAuthorityError::InvalidQuery);
    }
    Ok(derive_id(
        "source",
        &[
            scan_id.as_bytes(),
            source.inventory_generation.as_bytes(),
            source.volume_id.as_bytes(),
            &source.source_len.to_le_bytes(),
            source.file_system.as_bytes(),
        ],
    ))
}

fn derive_id(prefix: &str, fields: &[&[u8]]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"undelete-master:result-authority:v1");
    for field in fields {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    format!("{prefix}-{}", hex::encode(digest.finalize()))
}

impl From<CandidateKind> for CandidateKindDto {
    fn from(value: CandidateKind) -> Self {
        match value {
            CandidateKind::File => Self::File,
            CandidateKind::Directory => Self::Directory,
        }
    }
}

impl From<MetadataConfidence> for MetadataConfidenceDto {
    fn from(value: MetadataConfidence) -> Self {
        match value {
            MetadataConfidence::High => Self::High,
            MetadataConfidence::Medium => Self::Medium,
            MetadataConfidence::Low => Self::Low,
        }
    }
}

impl From<DiscoveryMethod> for DiscoveryMethodDto {
    fn from(value: DiscoveryMethod) -> Self {
        match value {
            DiscoveryMethod::NtfsMetadata => Self::NtfsMetadata,
            DiscoveryMethod::FatMetadata => Self::FatMetadata,
            DiscoveryMethod::ExfatMetadata => Self::ExfatMetadata,
            DiscoveryMethod::Carving => Self::Carving,
            DiscoveryMethod::RecycleBin => Self::RecycleBin,
        }
    }
}

impl From<CandidateState> for CandidateStateDto {
    fn from(value: CandidateState) -> Self {
        match value {
            CandidateState::ExactEvidence => Self::ExactEvidence,
            CandidateState::LikelyComplete => Self::LikelyComplete,
            CandidateState::CompleteUnvalidated => Self::CompleteUnvalidated,
            CandidateState::StructurallyValid => Self::StructurallyValid,
            CandidateState::Partial => Self::Partial,
            CandidateState::Conflicted => Self::Conflicted,
            CandidateState::ReadErrorState => Self::ReadError,
            CandidateState::ZeroedOrTrimmed => Self::ZeroedOrTrimmed,
            CandidateState::Overwritten => Self::Overwritten,
            CandidateState::MetadataOnly => Self::MetadataOnly,
            CandidateState::Unknown => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use um_core::{
        Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
        MetadataConfidence, Timestamps,
    };

    use super::{
        query_candidate_page, update_candidate_selection, BoundCandidateQuery, CandidateKindDto,
        CandidateQuery, CandidateQueryContext, CandidateRowDto, CandidateSort,
        CandidateSortDirection, CandidateSortField, DiscoveryMethodDto, MetadataConfidenceDto,
        RecoveryEligibility, ResultAuthorityError, ScanSourceBinding, SelectionOperationDto,
        StoredCandidate, CANDIDATE_PAGE_LIMIT, MAX_EXTENSION_SCALARS,
        MAX_RETAINED_CANDIDATES_PER_SCAN,
    };

    struct Harness {
        scan_id: String,
        source: ScanSourceBinding,
        candidates: Vec<StoredCandidate>,
        index: HashMap<u64, usize>,
        selected: HashSet<u64>,
        selection_revision: u64,
        active_query: Option<BoundCandidateQuery>,
    }

    impl Harness {
        fn new(scan_id: &str, candidates: Vec<StoredCandidate>) -> Self {
            let index = candidates
                .iter()
                .enumerate()
                .map(|(index, stored)| (stored.candidate.id, index))
                .collect();
            Self {
                scan_id: scan_id.to_owned(),
                source: ScanSourceBinding {
                    inventory_generation: "inventory-fixture".to_owned(),
                    volume_id: "volume-fixture".to_owned(),
                    source_len: 1_000_000,
                    file_system: "ntfs".to_owned(),
                    physical_disk_number: 7,
                },
                candidates,
                index,
                selected: HashSet::new(),
                selection_revision: 0,
                active_query: None,
            }
        }

        fn page(
            &mut self,
            query: CandidateQuery,
            sort: CandidateSort,
            cursor: Option<&str>,
        ) -> Result<super::CandidateQueryPageDto, super::ResultAuthorityError> {
            query_candidate_page(
                CandidateQueryContext {
                    scan_id: &self.scan_id,
                    source: &self.source,
                    candidates: &self.candidates,
                    selected: &self.selected,
                    selection_revision: self.selection_revision,
                    active_query: &mut self.active_query,
                },
                query,
                sort,
                cursor,
            )
        }

        fn update(
            &mut self,
            query_id: &str,
            operation: SelectionOperationDto,
            revision: u64,
        ) -> Result<super::CandidateSelectionUpdateDto, super::ResultAuthorityError> {
            update_candidate_selection(
                &self.scan_id,
                &self.candidates,
                &self.index,
                &mut self.selected,
                &mut self.selection_revision,
                self.active_query.as_ref(),
                query_id,
                operation,
                revision,
            )
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn stored(
        id: u64,
        path: &str,
        kind: CandidateKind,
        method: DiscoveryMethod,
        state: CandidateState,
        confidence: MetadataConfidence,
        size: u64,
        score: Option<u8>,
        availability: Option<ExtentAvailability>,
    ) -> StoredCandidate {
        let name = path.rsplit('/').next().expect("fixture name").to_owned();
        let parent_path = path
            .split('/')
            .take(path.split('/').count().saturating_sub(1))
            .map(str::to_owned)
            .collect();
        let candidate = Candidate {
            id,
            kind,
            method,
            state,
            name,
            name_certain: true,
            parent_path,
            metadata_confidence: confidence,
            size,
            timestamps: Timestamps::default(),
            extents: availability
                .map(|availability| ExtentRun {
                    logical_offset: 0,
                    physical_offset: Some(id.saturating_mul(4096)),
                    len: size,
                    availability,
                })
                .into_iter()
                .collect(),
            record_ref: id,
            sequence: Some(1),
            warnings: Vec::new(),
        };
        StoredCandidate {
            candidate,
            row: CandidateRowDto {
                id: format!("legacy-{id}"),
                display_path: path.to_owned(),
                kind: match kind {
                    CandidateKind::File => "file",
                    CandidateKind::Directory => "directory",
                },
                state: match state {
                    CandidateState::ExactEvidence => "exactEvidence",
                    CandidateState::LikelyComplete => "likelyComplete",
                    CandidateState::CompleteUnvalidated => "completeUnvalidated",
                    CandidateState::StructurallyValid => "structurallyValid",
                    CandidateState::Partial => "partial",
                    CandidateState::Conflicted => "conflicted",
                    CandidateState::ReadErrorState => "readError",
                    CandidateState::ZeroedOrTrimmed => "zeroedOrTrimmed",
                    CandidateState::Overwritten => "overwritten",
                    CandidateState::MetadataOnly => "metadataOnly",
                    CandidateState::Unknown => "unknown",
                },
                size_bytes: size.to_string(),
                metadata_confidence: match confidence {
                    MetadataConfidence::High => "high",
                    MetadataConfidence::Medium => "medium",
                    MetadataConfidence::Low => "low",
                },
                recoverability_score: score,
                path_state: "exact",
                method: match method {
                    DiscoveryMethod::NtfsMetadata => "ntfsMetadata",
                    DiscoveryMethod::FatMetadata => "fatMetadata",
                    DiscoveryMethod::ExfatMetadata => "exfatMetadata",
                    DiscoveryMethod::Carving => "carving",
                    DiscoveryMethod::RecycleBin => "recycleBin",
                },
                content_sha256: None,
                validator: None,
                warnings: Vec::new(),
            },
            expected_sha256: None,
        }
    }

    fn fixture_candidates() -> Vec<StoredCandidate> {
        vec![
            stored(
                12,
                "Docs/Alpha.TXT",
                CandidateKind::File,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::ExactEvidence,
                MetadataConfidence::High,
                120,
                Some(95),
                Some(ExtentAvailability::FreeInSnapshot),
            ),
            stored(
                2,
                "Docs/beta.txt",
                CandidateKind::File,
                DiscoveryMethod::FatMetadata,
                CandidateState::LikelyComplete,
                MetadataConfidence::Medium,
                220,
                Some(80),
                Some(ExtentAvailability::FreeInSnapshot),
            ),
            stored(
                9,
                "Media/photo.JPG",
                CandidateKind::File,
                DiscoveryMethod::Carving,
                CandidateState::StructurallyValid,
                MetadataConfidence::Low,
                320,
                Some(88),
                Some(ExtentAvailability::FreeInSnapshot),
            ),
            stored(
                4,
                "Media/partial.jpg",
                CandidateKind::File,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::Partial,
                MetadataConfidence::High,
                420,
                Some(55),
                Some(ExtentAvailability::ReadFailed),
            ),
            stored(
                7,
                "Archive/conflict.zip",
                CandidateKind::File,
                DiscoveryMethod::ExfatMetadata,
                CandidateState::Conflicted,
                MetadataConfidence::Medium,
                520,
                Some(30),
                Some(ExtentAvailability::CurrentlyAllocated),
            ),
            stored(
                1,
                "Archive/zero.bin",
                CandidateKind::File,
                DiscoveryMethod::RecycleBin,
                CandidateState::ZeroedOrTrimmed,
                MetadataConfidence::Low,
                620,
                Some(10),
                Some(ExtentAvailability::Zeroed),
            ),
            stored(
                6,
                "orphan",
                CandidateKind::File,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::MetadataOnly,
                MetadataConfidence::Low,
                720,
                Some(0),
                None,
            ),
            stored(
                3,
                "Folder",
                CandidateKind::Directory,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::CompleteUnvalidated,
                MetadataConfidence::High,
                0,
                None,
                None,
            ),
            stored(
                11,
                "Docs/read.err",
                CandidateKind::File,
                DiscoveryMethod::FatMetadata,
                CandidateState::ReadErrorState,
                MetadataConfidence::Medium,
                820,
                Some(0),
                Some(ExtentAvailability::ReadFailed),
            ),
            stored(
                5,
                "Docs/old.doc",
                CandidateKind::File,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::Overwritten,
                MetadataConfidence::Low,
                920,
                Some(0),
                Some(ExtentAvailability::CurrentlyAllocated),
            ),
            stored(
                10,
                "Docs/unknown.raw",
                CandidateKind::File,
                DiscoveryMethod::ExfatMetadata,
                CandidateState::Unknown,
                MetadataConfidence::Medium,
                1020,
                Some(0),
                Some(ExtentAvailability::Unknown),
            ),
            stored(
                8,
                "Docs/no-score",
                CandidateKind::Directory,
                DiscoveryMethod::RecycleBin,
                CandidateState::CompleteUnvalidated,
                MetadataConfidence::Low,
                0,
                None,
                None,
            ),
        ]
    }

    fn query(revision: u64) -> CandidateQuery {
        CandidateQuery {
            revision: revision.to_string(),
            search: String::new(),
            extensions: Vec::new(),
            kinds: Vec::new(),
            metadata_confidences: Vec::new(),
            methods: Vec::new(),
            states: Vec::new(),
            min_recoverability_score: None,
            max_recoverability_score: None,
            eligibilities: Vec::new(),
            selected_only: false,
        }
    }

    fn sort(field: CandidateSortField) -> CandidateSort {
        CandidateSort {
            field,
            direction: CandidateSortDirection::Ascending,
        }
    }

    #[test]
    fn result_query_filter_001_filters_the_complete_scan_and_all_dimensions() {
        let mut candidates = fixture_candidates();
        for id in 100..205 {
            candidates.push(stored(
                id,
                &format!("Bulk/file-{id}.tmp"),
                CandidateKind::File,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::CompleteUnvalidated,
                MetadataConfidence::High,
                1,
                Some(70),
                Some(ExtentAvailability::FreeInSnapshot),
            ));
        }
        candidates.push(stored(
            999,
            "Late/needle.TxT",
            CandidateKind::File,
            DiscoveryMethod::Carving,
            CandidateState::StructurallyValid,
            MetadataConfidence::Low,
            333,
            Some(88),
            Some(ExtentAvailability::FreeInSnapshot),
        ));
        let mut harness = Harness::new("scan-filter", candidates);

        let mut individual = query(1);
        individual.search = "NEEDLE".to_owned();
        assert_eq!(
            harness
                .page(individual, sort(CandidateSortField::Path), None)
                .expect("search")
                .filtered_total,
            "1"
        );
        let cases = [
            (
                CandidateQuery {
                    extensions: vec!["jpg".to_owned()],
                    ..query(2)
                },
                2,
            ),
            (
                CandidateQuery {
                    kinds: vec![CandidateKindDto::Directory],
                    ..query(3)
                },
                2,
            ),
            (
                CandidateQuery {
                    metadata_confidences: vec![MetadataConfidenceDto::Medium],
                    ..query(4)
                },
                4,
            ),
            (
                CandidateQuery {
                    methods: vec![DiscoveryMethodDto::RecycleBin],
                    ..query(5)
                },
                2,
            ),
            (
                CandidateQuery {
                    states: vec![super::CandidateStateDto::Conflicted],
                    ..query(6)
                },
                1,
            ),
            (
                CandidateQuery {
                    min_recoverability_score: Some(80),
                    max_recoverability_score: Some(90),
                    ..query(7)
                },
                3,
            ),
            (
                CandidateQuery {
                    eligibilities: vec![RecoveryEligibility::Ineligible],
                    ..query(8)
                },
                5,
            ),
        ];
        for (candidate_query, expected) in cases {
            assert_eq!(
                harness
                    .page(candidate_query, sort(CandidateSortField::Path), None)
                    .expect("independent filter")
                    .filtered_total,
                expected.to_string()
            );
        }
        let combined = CandidateQuery {
            search: "late".to_owned(),
            extensions: vec!["TXT".to_owned()],
            kinds: vec![CandidateKindDto::File],
            metadata_confidences: vec![MetadataConfidenceDto::Low],
            methods: vec![DiscoveryMethodDto::Carving],
            states: vec![super::CandidateStateDto::StructurallyValid],
            min_recoverability_score: Some(88),
            max_recoverability_score: Some(88),
            eligibilities: vec![RecoveryEligibility::Complete],
            ..query(9)
        };
        assert_eq!(
            harness
                .page(combined, sort(CandidateSortField::Path), None)
                .expect("combined")
                .candidates[0]
                .id,
            "999"
        );
    }

    #[test]
    fn result_query_filter_002_extension_matching_and_facets_are_exact() {
        let mut harness = Harness::new("scan-facets", fixture_candidates());
        let page = harness
            .page(query(1), sort(CandidateSortField::Extension), None)
            .expect("facets");
        let facets: HashMap<_, _> = page
            .extension_facets
            .into_iter()
            .map(|facet| (facet.extension, facet.count))
            .collect();
        assert_eq!(facets.get("txt").map(String::as_str), Some("2"));
        assert_eq!(facets.get("jpg").map(String::as_str), Some("2"));
        assert_eq!(facets.get("").map(String::as_str), Some("3"));

        let no_extension = CandidateQuery {
            extensions: vec![String::new()],
            ..query(2)
        };
        assert_eq!(
            harness
                .page(no_extension, sort(CandidateSortField::Path), None)
                .expect("no extension")
                .filtered_total,
            "3"
        );
    }

    #[test]
    fn result_query_sort_002_all_sorts_are_stable_with_null_scores_last() {
        let fields = [
            CandidateSortField::Path,
            CandidateSortField::Extension,
            CandidateSortField::Size,
            CandidateSortField::State,
            CandidateSortField::Confidence,
            CandidateSortField::RecoverabilityScore,
            CandidateSortField::Method,
        ];
        for (revision, field) in fields.into_iter().enumerate() {
            for direction in [
                CandidateSortDirection::Ascending,
                CandidateSortDirection::Descending,
            ] {
                let mut harness = Harness::new("scan-sort", fixture_candidates());
                let candidate_sort = CandidateSort { field, direction };
                let first = harness
                    .page(query(revision as u64 + 1), candidate_sort, None)
                    .expect("sort");
                let ids: Vec<u64> = first
                    .candidates
                    .iter()
                    .map(|row| row.id.parse().expect("decimal id"))
                    .collect();
                let repeated = harness
                    .page(query(revision as u64 + 1), candidate_sort, None)
                    .expect("repeat");
                assert_eq!(
                    ids,
                    repeated
                        .candidates
                        .iter()
                        .map(|row| row.id.parse::<u64>().expect("decimal id"))
                        .collect::<Vec<_>>()
                );
                if field == CandidateSortField::RecoverabilityScore {
                    assert!(first.candidates[first.candidates.len() - 2..]
                        .iter()
                        .all(|row| row.recoverability_score.is_none()));
                }
            }
        }

        let ties = vec![
            stored(
                20,
                "same.bin",
                CandidateKind::File,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::ExactEvidence,
                MetadataConfidence::High,
                10,
                Some(80),
                Some(ExtentAvailability::FreeInSnapshot),
            ),
            stored(
                10,
                "same.bin",
                CandidateKind::File,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::ExactEvidence,
                MetadataConfidence::High,
                10,
                Some(80),
                Some(ExtentAvailability::FreeInSnapshot),
            ),
        ];
        let mut harness = Harness::new("scan-ties", ties);
        for field in fields {
            let ids = harness
                .page(query(field as u64 + 20), sort(field), None)
                .expect("tie sort")
                .candidates
                .into_iter()
                .map(|row| row.id)
                .collect::<Vec<_>>();
            assert_eq!(ids, ["10", "20"]);
        }
    }

    #[test]
    fn result_page_bound_007_and_cursor_binding_003_fail_closed() {
        let candidates = (1..=205)
            .map(|id| {
                stored(
                    id,
                    &format!("file-{id}.bin"),
                    CandidateKind::File,
                    DiscoveryMethod::NtfsMetadata,
                    CandidateState::CompleteUnvalidated,
                    MetadataConfidence::High,
                    id,
                    Some(70),
                    Some(ExtentAvailability::FreeInSnapshot),
                )
            })
            .collect();
        let mut harness = Harness::new("scan-pages", candidates);
        let first = harness
            .page(query(1), sort(CandidateSortField::Path), None)
            .expect("first");
        assert_eq!(first.candidates.len(), CANDIDATE_PAGE_LIMIT);
        let cursor = first.next_cursor.expect("cursor");
        assert!(harness
            .page(
                CandidateQuery {
                    search: "2".to_owned(),
                    ..query(1)
                },
                sort(CandidateSortField::Path),
                Some(&cursor)
            )
            .is_err());
        assert!(harness
            .page(query(1), sort(CandidateSortField::Size), Some(&cursor))
            .is_err());
        assert!(harness
            .page(query(2), sort(CandidateSortField::Path), Some(&cursor))
            .is_err());

        let mut foreign = Harness::new("scan-foreign", fixture_candidates());
        assert!(foreign
            .page(query(1), sort(CandidateSortField::Path), Some(&cursor))
            .is_err());
        harness.source.volume_id = "volume-changed".to_owned();
        assert!(harness
            .page(query(1), sort(CandidateSortField::Path), Some(&cursor))
            .is_err());
    }

    #[test]
    fn result_selection_persist_004_select_all_005_and_stale_006() {
        let mut harness = Harness::new("scan-selection", fixture_candidates());
        let txt_query = CandidateQuery {
            extensions: vec!["txt".to_owned()],
            ..query(1)
        };
        let first = harness
            .page(txt_query, sort(CandidateSortField::Path), None)
            .expect("query");
        let query_id = first.query_id;
        let selected = harness
            .update(&query_id, SelectionOperationDto::SelectAllMatching, 0)
            .expect("select matching");
        assert_eq!(selected.selection.selected_candidates, "2");
        assert_eq!(selected.selection.matching_selected_candidates, "2");
        assert_eq!(selected.selection_revision, "1");

        let changed_page = harness
            .page(query(2), sort(CandidateSortField::Size), None)
            .expect("changed page");
        assert_eq!(changed_page.selection.selected_candidates, "2");
        assert_eq!(changed_page.selection.matching_selected_candidates, "2");
        assert!(
            changed_page
                .selection
                .matching_selected_candidates
                .parse::<u64>()
                .expect("count")
                <= changed_page.filtered_total.parse::<u64>().expect("total")
        );

        let before = harness.selected.clone();
        let stale = harness.update(&changed_page.query_id, SelectionOperationDto::ClearAll, 0);
        assert!(stale.is_err());
        assert_eq!(harness.selected, before);
        assert_eq!(harness.selection_revision, 1);

        let direct = harness
            .update(
                &changed_page.query_id,
                SelectionOperationDto::SetIds {
                    candidate_ids: vec!["9".to_owned()],
                    selected: true,
                },
                1,
            )
            .expect("direct");
        assert_eq!(direct.selection_revision, "2");
        assert_eq!(direct.selection.selected_candidates, "3");
    }

    #[test]
    fn result_selection_persist_004_direct_id_mutations_are_unique_and_bounded() {
        let candidates = (1..=101)
            .map(|id| {
                stored(
                    id,
                    &format!("file-{id}.bin"),
                    CandidateKind::File,
                    DiscoveryMethod::NtfsMetadata,
                    CandidateState::CompleteUnvalidated,
                    MetadataConfidence::High,
                    1,
                    Some(70),
                    Some(ExtentAvailability::FreeInSnapshot),
                )
            })
            .collect();
        let mut harness = Harness::new("scan-direct", candidates);
        let page = harness
            .page(query(1), sort(CandidateSortField::Path), None)
            .expect("query");
        let query_id = page.query_id;
        for candidate_ids in [
            Vec::new(),
            vec!["1".to_owned(), "1".to_owned()],
            (1..=101).map(|id| id.to_string()).collect(),
        ] {
            let before = harness.selected.clone();
            assert!(harness
                .update(
                    &query_id,
                    SelectionOperationDto::SetIds {
                        candidate_ids,
                        selected: true,
                    },
                    0,
                )
                .is_err());
            assert_eq!(harness.selected, before);
            assert_eq!(harness.selection_revision, 0);
        }

        let hundred = (1..=100).map(|id| id.to_string()).collect();
        let update = harness
            .update(
                &query_id,
                SelectionOperationDto::SetIds {
                    candidate_ids: hundred,
                    selected: true,
                },
                0,
            )
            .expect("one hundred unique ids");
        assert_eq!(update.selection.selected_candidates, "100");
    }

    #[test]
    fn result_cursor_binding_003_selected_only_cursor_expires_on_selection_change() {
        let candidates = (1..=205)
            .map(|id| {
                stored(
                    id,
                    &format!("file-{id}.bin"),
                    CandidateKind::File,
                    DiscoveryMethod::NtfsMetadata,
                    CandidateState::CompleteUnvalidated,
                    MetadataConfidence::High,
                    1,
                    Some(70),
                    Some(ExtentAvailability::FreeInSnapshot),
                )
            })
            .collect();
        let mut harness = Harness::new("scan-selected-cursor", candidates);
        let all = harness
            .page(query(1), sort(CandidateSortField::Path), None)
            .expect("all query");
        harness
            .update(&all.query_id, SelectionOperationDto::SelectAllMatching, 0)
            .expect("select all");

        let selected_query = CandidateQuery {
            selected_only: true,
            ..query(2)
        };
        let selected_page = harness
            .page(selected_query.clone(), sort(CandidateSortField::Path), None)
            .expect("selected first page");
        let cursor = selected_page.next_cursor.expect("selected cursor");
        harness
            .update(
                &selected_page.query_id,
                SelectionOperationDto::SetIds {
                    candidate_ids: vec!["1".to_owned()],
                    selected: false,
                },
                1,
            )
            .expect("change selected dataset");

        assert_eq!(
            harness
                .page(
                    selected_query,
                    sort(CandidateSortField::Path),
                    Some(&cursor),
                )
                .expect_err("selected-only cursor must expire"),
            ResultAuthorityError::StaleCursor
        );
    }

    #[test]
    fn result_selection_stale_006_failed_updates_are_transactional() {
        let candidates = vec![
            stored(
                1,
                "huge.bin",
                CandidateKind::File,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::CompleteUnvalidated,
                MetadataConfidence::High,
                u64::MAX,
                Some(70),
                Some(ExtentAvailability::FreeInSnapshot),
            ),
            stored(
                2,
                "one.bin",
                CandidateKind::File,
                DiscoveryMethod::NtfsMetadata,
                CandidateState::CompleteUnvalidated,
                MetadataConfidence::High,
                1,
                Some(70),
                Some(ExtentAvailability::FreeInSnapshot),
            ),
        ];
        let mut harness = Harness::new("scan-selection-overflow", candidates);
        let page = harness
            .page(query(1), sort(CandidateSortField::Path), None)
            .expect("query");
        assert_eq!(
            harness
                .update(
                    &page.query_id,
                    SelectionOperationDto::SetIds {
                        candidate_ids: vec!["1".to_owned(), "2".to_owned()],
                        selected: true,
                    },
                    0,
                )
                .expect_err("summary overflow must fail"),
            ResultAuthorityError::Overflow
        );
        assert!(harness.selected.is_empty());
        assert_eq!(harness.selection_revision, 0);

        let mut revision_harness = Harness::new("scan-revision-overflow", fixture_candidates());
        revision_harness.selected.insert(12);
        revision_harness.selection_revision = u64::MAX;
        let page = revision_harness
            .page(query(1), sort(CandidateSortField::Path), None)
            .expect("max revision query");
        assert_eq!(
            revision_harness
                .update(&page.query_id, SelectionOperationDto::ClearAll, u64::MAX,)
                .expect_err("revision overflow must fail"),
            ResultAuthorityError::Overflow
        );
        assert_eq!(revision_harness.selected, HashSet::from([12]));
        assert_eq!(revision_harness.selection_revision, u64::MAX);
    }

    #[test]
    fn result_query_filter_003_facets_cover_129_distinct_extensions() {
        let candidates = (1..=129)
            .map(|id| {
                stored(
                    id,
                    &format!("file-{id}.ext{id}"),
                    CandidateKind::File,
                    DiscoveryMethod::NtfsMetadata,
                    CandidateState::CompleteUnvalidated,
                    MetadataConfidence::High,
                    1,
                    Some(70),
                    Some(ExtentAvailability::FreeInSnapshot),
                )
            })
            .collect();
        let mut harness = Harness::new("scan-many-facets", candidates);
        let page = harness
            .page(query(1), sort(CandidateSortField::Extension), None)
            .expect("complete facets");
        assert_eq!(page.extension_facets.len(), 129);
    }

    #[test]
    fn result_query_filter_004_hostile_extensions_share_one_canonical_policy() {
        let names = [
            "report.TXT".to_owned(),
            "report.txt ".to_owned(),
            "report.t/xt".to_owned(),
            "report.bad\u{0000}".to_owned(),
            ".hidden".to_owned(),
            format!("report.{}", "x".repeat(MAX_EXTENSION_SCALARS + 1)),
        ];
        let candidates = names
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                let mut candidate = stored(
                    index as u64 + 1,
                    "placeholder.bin",
                    CandidateKind::File,
                    DiscoveryMethod::NtfsMetadata,
                    CandidateState::CompleteUnvalidated,
                    MetadataConfidence::High,
                    1,
                    Some(70),
                    Some(ExtentAvailability::FreeInSnapshot),
                );
                candidate.candidate.name = name;
                candidate
            })
            .collect();
        let mut harness = Harness::new("scan-hostile-extensions", candidates);
        let page = harness
            .page(query(1), sort(CandidateSortField::Extension), None)
            .expect("canonical facets");
        let facets = page
            .extension_facets
            .into_iter()
            .map(|facet| (facet.extension, facet.count))
            .collect::<HashMap<_, _>>();
        assert_eq!(
            facets,
            HashMap::from([
                ("".to_owned(), "4".to_owned()),
                ("txt".to_owned(), "2".to_owned())
            ])
        );

        let query = CandidateQuery {
            extensions: vec![" TXT ".to_owned()],
            ..query(2)
        };
        assert_eq!(
            harness
                .page(query, sort(CandidateSortField::Path), None)
                .expect("canonical query")
                .filtered_total,
            "2"
        );
    }

    #[test]
    fn result_query_filter_005_retained_bound_covers_the_deep_producer_ceiling() {
        assert_eq!(MAX_RETAINED_CANDIDATES_PER_SCAN, 110_000);
        assert!(super::retained_candidate_count_within_bound(110_000));
        assert!(!super::retained_candidate_count_within_bound(110_001));
    }

    #[test]
    fn result_query_filter_006_normalization_bounds_lowercase_expansion_and_rejects_bom() {
        let safe_raw = "\u{0130}".repeat(127);
        let overflowing_raw = "\u{0130}".repeat(128);
        let safe_normalized = safe_raw.to_lowercase();
        assert_eq!(safe_normalized.chars().count(), 254);
        assert_eq!(overflowing_raw.to_lowercase().chars().count(), 256);

        let names = [
            format!("report.{safe_raw}"),
            format!("report.{overflowing_raw}"),
            "report.\u{FEFF}txt".to_owned(),
            "report.t\u{FEFF}xt".to_owned(),
        ];
        let candidates = names
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                let mut candidate = stored(
                    index as u64 + 1,
                    "placeholder.bin",
                    CandidateKind::File,
                    DiscoveryMethod::NtfsMetadata,
                    CandidateState::CompleteUnvalidated,
                    MetadataConfidence::High,
                    1,
                    Some(70),
                    Some(ExtentAvailability::FreeInSnapshot),
                );
                candidate.candidate.name = name;
                candidate
            })
            .collect();
        let mut harness = Harness::new("scan-unicode-extensions", candidates);
        let page = harness
            .page(query(1), sort(CandidateSortField::Extension), None)
            .expect("bounded canonical extensions");
        let facets = page
            .extension_facets
            .into_iter()
            .map(|facet| (facet.extension, facet.count))
            .collect::<HashMap<_, _>>();
        assert_eq!(
            facets,
            HashMap::from([
                (String::new(), "3".to_owned()),
                (safe_normalized, "1".to_owned())
            ])
        );

        let safe_query = CandidateQuery {
            extensions: vec![safe_raw],
            ..query(2)
        };
        assert_eq!(
            harness
                .page(safe_query, sort(CandidateSortField::Path), None)
                .expect("safe lowercase expansion")
                .filtered_total,
            "1"
        );
        for (revision, extension) in [
            (3, overflowing_raw),
            (4, "\u{FEFF}txt".to_owned()),
            (5, "t\u{FEFF}xt".to_owned()),
        ] {
            let invalid_query = CandidateQuery {
                extensions: vec![extension],
                ..query(revision)
            };
            assert_eq!(
                harness
                    .page(invalid_query, sort(CandidateSortField::Path), None)
                    .expect_err("non-canonical extension query must fail"),
                ResultAuthorityError::InvalidQuery
            );
        }
    }
}
