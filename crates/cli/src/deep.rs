use std::collections::{HashMap, HashSet};

use um_carving::{carve_jpegs_with_evidence, CarveEvidence, CarveLimits, CarveRegion, CarveReport};
use um_core::{Candidate, DiscoveryMethod, ExtentAvailability, Region, SourceReader};
use um_fs_ntfs::NtfsAllocationSnapshot;

use crate::{CliError, JpegCarveCoverage};

const DEEP_CHUNK_BYTES: usize = 1024 * 1024;
const DEEP_MAX_CANDIDATE_BYTES: u64 = 128 * 1024 * 1024;
const DEEP_MAX_CANDIDATES: usize = 10_000;
const DEEP_MAX_REGIONS: usize = 65_536;
const DEEP_MAX_SCAN_BYTES: u64 = 16 * 1024 * 1024 * 1024 * 1024;
const DEEP_MAX_SIGNATURE_ATTEMPTS: u64 = 10_000_000;
const DEEP_MAX_VALIDATION_BYTES: u64 = 8 * 1024 * 1024 * 1024;

pub(crate) struct DeepJpegOutcome {
    pub candidates: Vec<Candidate>,
    pub evidence: Vec<CarveEvidence>,
    pub coverage: JpegCarveCoverage,
    pub warnings: Vec<String>,
    pub is_complete: bool,
}

struct RegionAdmission {
    regions: Vec<CarveRegion>,
    limit_reached: bool,
}

pub(crate) fn scan_ntfs_deep_jpeg(
    reader: &dyn SourceReader,
    index: u32,
    candidates: Vec<Candidate>,
    allocation: Option<&NtfsAllocationSnapshot>,
) -> Result<DeepJpegOutcome, CliError> {
    let Some(allocation) = allocation else {
        return Ok(DeepJpegOutcome {
            candidates,
            evidence: Vec::new(),
            coverage: unavailable_coverage(),
            warnings: vec![
                "JPEG deep scan was not run because trustworthy NTFS allocation evidence was unavailable."
                    .into(),
            ],
            is_complete: false,
        });
    };

    let admission = admit_free_regions(allocation, reader.len(), index)?;
    let limits = CarveLimits {
        chunk_size: DEEP_CHUNK_BYTES,
        max_candidate_bytes: DEEP_MAX_CANDIDATE_BYTES,
        max_candidates: DEEP_MAX_CANDIDATES,
        max_regions: DEEP_MAX_REGIONS,
        max_scan_bytes: DEEP_MAX_SCAN_BYTES,
        max_signature_attempts: DEEP_MAX_SIGNATURE_ATTEMPTS,
        max_validation_bytes: DEEP_MAX_VALIDATION_BYTES,
    };
    let carved = carve_jpegs_with_evidence(reader, &admission.regions, limits)
        .map_err(|source| CliError::DeepScan { index, source })?;
    let allocation_partial = !allocation.is_complete();
    let carving_partial = carved.coverage.partial;
    let carve_coverage = carved.coverage.clone();
    let (candidates, evidence) = merge_carved_evidence(index, candidates, carved)?;
    let mut warnings = Vec::new();
    if allocation_partial {
        warnings.push(
            "JPEG deep scan examined only bitmap-proven free ranges because allocation evidence was incomplete."
                .into(),
        );
    }
    if admission.limit_reached {
        warnings.push(
            "JPEG deep scan stopped at a defensive free-region work limit; coverage is partial."
                .into(),
        );
    }
    if carving_partial {
        warnings.push(
            "JPEG deep scan encountered unreadable bytes or a carving work limit; coverage is partial."
                .into(),
        );
    }

    let partial = allocation_partial || admission.limit_reached || carving_partial;
    let coverage = JpegCarveCoverage {
        bytes_requested: carve_coverage.bytes_requested,
        bytes_scanned: carve_coverage.bytes_scanned,
        signatures_attempted: carve_coverage.signatures_attempted,
        validation_bytes_read: carve_coverage.validation_bytes_read,
        partial,
        read_error_count: usize_to_u64(
            carve_coverage
                .read_errors
                .len()
                .saturating_add(carve_coverage.read_errors_omitted),
        ),
        candidate_limit_reached: carve_coverage.candidate_limit_reached,
        candidate_byte_limit_hits: usize_to_u64(carve_coverage.candidate_byte_limit_hits),
        signature_attempt_limit_reached: carve_coverage.signature_attempt_limit_reached,
        validation_byte_limit_reached: carve_coverage.validation_byte_limit_reached,
        rejected_signatures: usize_to_u64(carve_coverage.rejected_signatures),
        truncated_signatures: usize_to_u64(carve_coverage.truncated_signatures),
        regions_submitted: usize_to_u64(admission.regions.len()),
        region_limit_reached: admission.limit_reached,
    };
    Ok(DeepJpegOutcome {
        candidates,
        evidence,
        coverage,
        warnings,
        is_complete: !partial,
    })
}

pub(crate) fn unsupported_deep_coverage() -> JpegCarveCoverage {
    unavailable_coverage()
}

fn unavailable_coverage() -> JpegCarveCoverage {
    JpegCarveCoverage {
        bytes_requested: 0,
        bytes_scanned: 0,
        signatures_attempted: 0,
        validation_bytes_read: 0,
        partial: true,
        read_error_count: 0,
        candidate_limit_reached: false,
        candidate_byte_limit_hits: 0,
        signature_attempt_limit_reached: false,
        validation_byte_limit_reached: false,
        rejected_signatures: 0,
        truncated_signatures: 0,
        regions_submitted: 0,
        region_limit_reached: false,
    }
}

fn admit_free_regions(
    allocation: &NtfsAllocationSnapshot,
    source_len: u64,
    index: u32,
) -> Result<RegionAdmission, CliError> {
    admit_regions(allocation.free_regions(), source_len, index)
}

fn admit_regions(
    free_regions: impl IntoIterator<Item = Region>,
    source_len: u64,
    index: u32,
) -> Result<RegionAdmission, CliError> {
    let mut regions: Vec<CarveRegion> = Vec::new();
    regions
        .try_reserve(DEEP_MAX_REGIONS.min(256))
        .map_err(|_| CliError::DeepScanResult { index })?;
    let mut bytes_admitted = 0u64;
    let mut limit_reached = false;

    for region in free_regions {
        if region.is_empty() {
            continue;
        }
        if region.end() > source_len {
            limit_reached = true;
            break;
        }
        let remaining = DEEP_MAX_SCAN_BYTES.saturating_sub(bytes_admitted);
        if remaining == 0 {
            limit_reached = true;
            break;
        }
        let admitted_len = region.len.min(remaining);
        let admitted =
            Region::new(region.offset, admitted_len).ok_or(CliError::DeepScanResult { index })?;

        if let Some(last) = regions.last_mut() {
            if last.region.end() == admitted.offset {
                let merged_len = last
                    .region
                    .len
                    .checked_add(admitted.len)
                    .ok_or(CliError::DeepScanResult { index })?;
                last.region = Region::new(last.region.offset, merged_len)
                    .ok_or(CliError::DeepScanResult { index })?;
            } else {
                if regions.len() == DEEP_MAX_REGIONS {
                    limit_reached = true;
                    break;
                }
                regions
                    .try_reserve(1)
                    .map_err(|_| CliError::DeepScanResult { index })?;
                regions.push(CarveRegion {
                    region: admitted,
                    availability: ExtentAvailability::FreeInSnapshot,
                });
            }
        } else {
            regions
                .try_reserve(1)
                .map_err(|_| CliError::DeepScanResult { index })?;
            regions.push(CarveRegion {
                region: admitted,
                availability: ExtentAvailability::FreeInSnapshot,
            });
        }
        bytes_admitted = bytes_admitted
            .checked_add(admitted_len)
            .ok_or(CliError::DeepScanResult { index })?;
        if admitted_len < region.len {
            limit_reached = true;
            break;
        }
    }

    Ok(RegionAdmission {
        regions,
        limit_reached,
    })
}

fn merge_carved_evidence(
    index: u32,
    mut candidates: Vec<Candidate>,
    carved: CarveReport,
) -> Result<(Vec<Candidate>, Vec<CarveEvidence>), CliError> {
    if carved.candidates.len() != carved.evidence.len() {
        return Err(CliError::DeepScanResult { index });
    }

    let mut used_ids = HashSet::new();
    used_ids
        .try_reserve(candidates.len())
        .map_err(|_| CliError::DeepScanResult { index })?;
    let mut last_id = None;
    let mut metadata_ranges: HashMap<(u64, u64), Option<u64>> = HashMap::new();
    metadata_ranges
        .try_reserve(candidates.len())
        .map_err(|_| CliError::DeepScanResult { index })?;
    let mut resolved_carve_ranges: HashMap<(u64, u64), u64> = HashMap::new();
    resolved_carve_ranges
        .try_reserve(carved.candidates.len())
        .map_err(|_| CliError::DeepScanResult { index })?;
    for candidate in &candidates {
        if !used_ids.insert(candidate.id) {
            return Err(CliError::DeepScanResult { index });
        }
        last_id = Some(last_id.map_or(candidate.id, |current: u64| current.max(candidate.id)));
        if candidate.method == DiscoveryMethod::NtfsMetadata {
            if let Some(range) = exact_physical_range(candidate) {
                match metadata_ranges.get_mut(&range) {
                    Some(owner) if *owner != Some(candidate.id) => *owner = None,
                    Some(_) => {}
                    None => {
                        metadata_ranges.insert(range, Some(candidate.id));
                    }
                }
            }
        }
    }

    let mut evidence = Vec::new();
    evidence
        .try_reserve(carved.evidence.len())
        .map_err(|_| CliError::DeepScanResult { index })?;
    let mut evidence_ranges = HashMap::new();
    evidence_ranges
        .try_reserve(carved.evidence.len())
        .map_err(|_| CliError::DeepScanResult { index })?;
    candidates
        .try_reserve(carved.candidates.len())
        .map_err(|_| CliError::DeepScanResult { index })?;
    for (mut candidate, mut item) in carved
        .candidates
        .into_iter()
        .zip(carved.evidence.into_iter())
    {
        let range = (item.physical_offset, item.len);
        if candidate.method != DiscoveryMethod::Carving
            || candidate.id != item.candidate_id
            || candidate.record_ref != item.physical_offset
            || candidate.size != item.len
            || exact_physical_range(&candidate) != Some(range)
        {
            return Err(CliError::DeepScanResult { index });
        }

        let assigned_id = if let Some(existing_id) = resolved_carve_ranges.get(&range).copied() {
            existing_id
        } else if let Some(Some(metadata_id)) = metadata_ranges.get(&range) {
            *metadata_id
        } else {
            let next_id = match last_id {
                Some(value) => value
                    .checked_add(1)
                    .ok_or(CliError::DeepScanResult { index })?,
                None => 1,
            };
            last_id = Some(next_id);
            candidate.id = next_id;
            if matches!(metadata_ranges.get(&range), Some(None)) {
                candidate.warnings.push(
                    "This carved byte range is also referenced by multiple NTFS metadata candidates, so carving evidence could not be assigned to one metadata record."
                        .into(),
                );
            }
            if !used_ids.insert(next_id) {
                return Err(CliError::DeepScanResult { index });
            }
            candidates.push(candidate);
            next_id
        };
        resolved_carve_ranges.insert(range, assigned_id);
        item.candidate_id = assigned_id;

        let fingerprint = (item.content_sha256, item.validator);
        match evidence_ranges.get(&range) {
            Some(existing) if *existing != fingerprint => {
                return Err(CliError::DeepScanResult { index });
            }
            Some(_) => {}
            None => {
                evidence_ranges.insert(range, fingerprint);
                evidence.push(item);
            }
        }
    }
    Ok((candidates, evidence))
}

fn exact_physical_range(candidate: &Candidate) -> Option<(u64, u64)> {
    if candidate.size == 0 || candidate.extents.is_empty() {
        return None;
    }
    let mut logical = 0u64;
    let mut physical_start = None;
    let mut physical = 0u64;
    for extent in &candidate.extents {
        if extent.len == 0 || extent.logical_offset != logical {
            return None;
        }
        let extent_physical = extent.physical_offset?;
        match physical_start {
            None => {
                physical_start = Some(extent_physical);
                physical = extent_physical;
            }
            Some(_) if extent_physical != physical => return None,
            Some(_) => {}
        }
        logical = logical.checked_add(extent.len)?;
        physical = physical.checked_add(extent.len)?;
        if logical > candidate.size {
            return None;
        }
    }
    (logical == candidate.size).then_some((physical_start?, candidate.size))
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{admit_regions, merge_carved_evidence, DEEP_MAX_REGIONS, DEEP_MAX_SCAN_BYTES};
    use um_carving::{CarveEvidence, CarveReport};
    use um_core::{
        Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
        MetadataConfidence, Region, Timestamps,
    };

    #[test]
    fn cli_deep_region_coalesce_009_coalesces_adjacent_proven_free_ranges() {
        let regions = [
            Region::new(0, 10).unwrap(),
            Region::new(10, 20).unwrap(),
            Region::new(40, 5).unwrap(),
        ];

        let admitted = admit_regions(regions, 45, 3).expect("admit bounded free regions");

        assert_eq!(admitted.regions.len(), 2);
        assert_eq!(admitted.regions[0].region, Region::new(0, 30).unwrap());
        assert_eq!(admitted.regions[1].region, Region::new(40, 5).unwrap());
        assert!(!admitted.limit_reached);
    }

    #[test]
    fn cli_deep_limit_010_stops_before_region_or_byte_budgets_are_exceeded() {
        let fragmented =
            (0..=DEEP_MAX_REGIONS).map(|index| Region::new(index as u64 * 2, 1).unwrap());

        let region_limited = admit_regions(fragmented, DEEP_MAX_REGIONS as u64 * 2 + 1, 4)
            .expect("bounded fragmented ranges");

        assert_eq!(region_limited.regions.len(), DEEP_MAX_REGIONS);
        assert!(region_limited.limit_reached);

        let byte_limited = admit_regions(
            [Region::new(0, DEEP_MAX_SCAN_BYTES + 1).unwrap()],
            DEEP_MAX_SCAN_BYTES + 1,
            5,
        )
        .expect("bounded large range");

        assert_eq!(byte_limited.regions.len(), 1);
        assert_eq!(byte_limited.regions[0].region.len, DEEP_MAX_SCAN_BYTES);
        assert!(byte_limited.limit_reached);
    }

    #[test]
    fn cli_deep_dedupe_011_preserves_carving_when_metadata_range_is_ambiguous() {
        let offset = 96 * 1024;
        let len = 768;
        let metadata = vec![
            candidate(
                11,
                DiscoveryMethod::NtfsMetadata,
                offset,
                len,
                "first-deleted.jpg",
            ),
            candidate(
                12,
                DiscoveryMethod::NtfsMetadata,
                offset,
                len,
                "second-deleted.jpg",
            ),
        ];

        let (merged, evidence) = merge_carved_evidence(4, metadata, carved_report(offset, len, 1))
            .expect("ambiguous metadata must remain honest");

        assert_eq!(merged.len(), 3);
        let carved = merged
            .iter()
            .find(|item| item.method == DiscoveryMethod::Carving)
            .expect("separate carving candidate");
        assert_eq!(carved.id, 13);
        assert!(carved.warnings.iter().any(|warning| {
            warning.contains("multiple NTFS metadata candidates")
                && warning.contains("could not be assigned")
        }));
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].candidate_id, carved.id);
        assert_ne!(evidence[0].candidate_id, 11);
        assert_ne!(evidence[0].candidate_id, 12);
    }

    #[test]
    fn cli_deep_dedupe_012_does_not_multiply_duplicate_carves_of_one_range() {
        let offset = 128 * 1024;
        let len = 512;

        let (merged, evidence) =
            merge_carved_evidence(5, Vec::new(), carved_report(offset, len, 2))
                .expect("duplicate carved evidence must coalesce");

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].method, DiscoveryMethod::Carving);
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].candidate_id, merged[0].id);
        assert_eq!(evidence[0].physical_offset, offset);
        assert_eq!(evidence[0].len, len);
    }

    fn candidate(
        id: u64,
        method: DiscoveryMethod,
        physical_offset: u64,
        len: u64,
        name: &str,
    ) -> Candidate {
        Candidate {
            id,
            kind: CandidateKind::File,
            method,
            state: CandidateState::StructurallyValid,
            name: name.into(),
            name_certain: method != DiscoveryMethod::Carving,
            parent_path: Vec::new(),
            metadata_confidence: if method == DiscoveryMethod::Carving {
                MetadataConfidence::Low
            } else {
                MetadataConfidence::High
            },
            size: len,
            timestamps: Timestamps::default(),
            extents: vec![ExtentRun {
                logical_offset: 0,
                physical_offset: Some(physical_offset),
                len,
                availability: ExtentAvailability::FreeInSnapshot,
            }],
            record_ref: if method == DiscoveryMethod::Carving {
                physical_offset
            } else {
                id
            },
            sequence: None,
            warnings: Vec::new(),
        }
    }

    fn carved_report(physical_offset: u64, len: u64, copies: usize) -> CarveReport {
        let carved = candidate(
            physical_offset,
            DiscoveryMethod::Carving,
            physical_offset,
            len,
            "carved.jpg",
        );
        let evidence = CarveEvidence {
            candidate_id: physical_offset,
            physical_offset,
            len,
            content_sha256: [0xA5; 32],
            validator: "jpeg-structural-v1",
        };
        CarveReport {
            candidates: vec![carved; copies],
            evidence: vec![evidence; copies],
            ..CarveReport::default()
        }
    }
}
