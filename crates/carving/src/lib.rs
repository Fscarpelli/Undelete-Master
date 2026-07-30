//! Bounded, read-only signature carving over caller-provided source regions.
//!
//! This crate never opens a path or device. Callers retain authority over which
//! regions are eligible for carving and provide only the read-only
//! [`SourceReader`] capability.

#![forbid(unsafe_code)]

mod jpeg;

use jpeg::{JpegStatus, JpegValidator};
use sha2::{Digest, Sha256};
use thiserror::Error;
use um_core::{
    Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
    MetadataConfidence, Region, SourceReader, Timestamps,
};

const SIGNATURE_OVERLAP: usize = 1;
const MAX_CHUNK_SIZE: usize = 4 * 1024 * 1024;
const MAX_CANDIDATE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CANDIDATES: usize = 100_000;
const MAX_REGIONS: usize = 1_000_000;
const MAX_SIGNATURE_ATTEMPTS: u64 = 100_000_000;
const MAX_VALIDATION_BYTES: u64 = 16 * 1024 * 1024 * 1024 * 1024;
const INITIAL_VALIDATION_READ_BYTES: usize = 256;
const MAX_BAD_RANGES_PER_READ: usize = 8_192;
const MAX_REPORTED_READ_ISSUES: usize = 1_024;
pub const JPEG_VALIDATOR_VERSION: &str = "jpeg-structural-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarveLimits {
    /// Unique source bytes requested per scan read.
    pub chunk_size: usize,
    /// Maximum bytes inspected from one JPEG SOI before the candidate is
    /// conservatively left unresolved.
    pub max_candidate_bytes: u64,
    /// Maximum candidates returned before enumeration stops as partial.
    pub max_candidates: usize,
    /// Maximum explicit regions accepted from the caller.
    pub max_regions: usize,
    /// Maximum aggregate size of all explicit regions.
    pub max_scan_bytes: u64,
    /// Maximum JPEG signatures that may enter structural validation.
    pub max_signature_attempts: u64,
    /// Maximum aggregate bytes read across every candidate validation.
    pub max_validation_bytes: u64,
}

impl Default for CarveLimits {
    fn default() -> Self {
        Self {
            chunk_size: 1024 * 1024,
            max_candidate_bytes: 128 * 1024 * 1024,
            max_candidates: 10_000,
            max_regions: 100_000,
            max_scan_bytes: 16 * 1024 * 1024 * 1024 * 1024,
            max_signature_attempts: 10_000_000,
            max_validation_bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}

/// A region plus the allocation evidence already established by the caller.
///
/// The carver does not infer allocation state. Use [`carve_jpegs`] when only
/// physical bounds are known; it records `Unknown` availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarveRegion {
    pub region: Region,
    pub availability: ExtentAvailability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarveReadIssue {
    pub offset: u64,
    pub len: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarvePartialReason {
    ReadError,
    CandidateLimit,
    CandidateByteLimit,
    SignatureAttemptLimit,
    ValidationByteLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CarveCoverage {
    /// Aggregate bytes in the caller-approved, non-overlapping regions.
    pub bytes_requested: u64,
    /// Unique readable bytes visited by the signature scan. Candidate
    /// validation re-reads are intentionally not double-counted.
    pub bytes_scanned: u64,
    /// True when read failures or a work/output budget prevented exhaustive
    /// evidence within the requested regions.
    pub partial: bool,
    pub read_errors: Vec<CarveReadIssue>,
    pub read_errors_omitted: usize,
    pub candidate_limit_reached: bool,
    pub candidate_byte_limit_hits: usize,
    pub rejected_signatures: usize,
    pub truncated_signatures: usize,
    pub signatures_attempted: u64,
    pub validation_bytes_read: u64,
    pub signature_attempt_limit_reached: bool,
    pub validation_byte_limit_reached: bool,
    pub partial_reasons: Vec<CarvePartialReason>,
}

#[derive(Debug, Clone, Default)]
pub struct CarveReport {
    pub candidates: Vec<Candidate>,
    pub evidence: Vec<CarveEvidence>,
    pub coverage: CarveCoverage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarveEvidence {
    pub candidate_id: u64,
    pub physical_offset: u64,
    pub len: u64,
    pub content_sha256: [u8; 32],
    pub validator: &'static str,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CarveError {
    #[error("invalid carving limits: {0}")]
    InvalidLimits(&'static str),
    #[error("requested {requested} regions, exceeding configured limit {max}")]
    RegionLimitExceeded { requested: usize, max: usize },
    #[error("aggregate region size overflowed u64")]
    RegionSizeOverflow,
    #[error("requested {requested} scan bytes, exceeding configured limit {max}")]
    ScanByteLimitExceeded { requested: u64, max: u64 },
    #[error(
        "carve region {index} ({region:?}) is outside source bounds (source length {source_len})"
    )]
    RegionOutOfBounds {
        index: usize,
        region: Region,
        source_len: u64,
    },
    #[error("carve regions {first_index} and {second_index} overlap")]
    OverlappingRegions {
        first_index: usize,
        second_index: usize,
    },
    #[error("bounded carving buffer allocation failed")]
    AllocationFailed,
}

/// Carves JPEGs from explicit regions, recording unknown allocation
/// availability. This is the simplest integration API for a caller that only
/// has coalesced physical ranges.
pub fn carve_jpegs(
    reader: &dyn SourceReader,
    regions: &[Region],
    limits: CarveLimits,
) -> Result<CarveReport, CarveError> {
    validate_limits(limits)?;
    if regions.len() > limits.max_regions {
        return Err(CarveError::RegionLimitExceeded {
            requested: regions.len(),
            max: limits.max_regions,
        });
    }
    let mut evidenced = Vec::new();
    evidenced
        .try_reserve_exact(regions.len())
        .map_err(|_| CarveError::AllocationFailed)?;
    evidenced.extend(regions.iter().copied().map(|region| CarveRegion {
        region,
        availability: ExtentAvailability::Unknown,
    }));
    carve_jpegs_with_evidence(reader, &evidenced, limits)
}

/// Carves structurally valid JPEGs from explicit, non-overlapping source
/// regions. No path is opened and no bytes are written.
pub fn carve_jpegs_with_evidence(
    reader: &dyn SourceReader,
    regions: &[CarveRegion],
    limits: CarveLimits,
) -> Result<CarveReport, CarveError> {
    validate_limits(limits)?;
    let bytes_requested = validate_regions(reader, regions, limits)?;
    let mut report = CarveReport {
        candidates: Vec::new(),
        evidence: Vec::new(),
        coverage: CarveCoverage {
            bytes_requested,
            ..CarveCoverage::default()
        },
    };
    report
        .candidates
        .try_reserve(limits.max_candidates.min(256))
        .map_err(|_| CarveError::AllocationFailed)?;
    report
        .evidence
        .try_reserve(limits.max_candidates.min(256))
        .map_err(|_| CarveError::AllocationFailed)?;
    report
        .coverage
        .partial_reasons
        .try_reserve_exact(5)
        .map_err(|_| CarveError::AllocationFailed)?;

    for carve_region in regions {
        if scan_region(reader, *carve_region, limits, &mut report)? {
            break;
        }
    }
    Ok(report)
}

fn validate_limits(limits: CarveLimits) -> Result<(), CarveError> {
    if !(2..=MAX_CHUNK_SIZE).contains(&limits.chunk_size) {
        return Err(CarveError::InvalidLimits(
            "chunk_size must be between 2 bytes and 4 MiB",
        ));
    }
    if limits.max_candidate_bytes == 0 || limits.max_candidate_bytes > MAX_CANDIDATE_BYTES {
        return Err(CarveError::InvalidLimits(
            "max_candidate_bytes must be between 1 byte and 512 MiB",
        ));
    }
    usize::try_from(limits.max_candidate_bytes).map_err(|_| {
        CarveError::InvalidLimits("max_candidate_bytes does not fit the current platform")
    })?;
    if limits.max_candidates == 0 || limits.max_candidates > MAX_CANDIDATES {
        return Err(CarveError::InvalidLimits(
            "max_candidates must be between 1 and 100000",
        ));
    }
    if limits.max_regions == 0 || limits.max_regions > MAX_REGIONS {
        return Err(CarveError::InvalidLimits(
            "max_regions must be between 1 and 1000000",
        ));
    }
    if limits.max_scan_bytes == 0 {
        return Err(CarveError::InvalidLimits(
            "max_scan_bytes must be greater than zero",
        ));
    }
    if limits.max_signature_attempts == 0 || limits.max_signature_attempts > MAX_SIGNATURE_ATTEMPTS
    {
        return Err(CarveError::InvalidLimits(
            "max_signature_attempts must be between 1 and 100000000",
        ));
    }
    if limits.max_validation_bytes == 0 || limits.max_validation_bytes > MAX_VALIDATION_BYTES {
        return Err(CarveError::InvalidLimits(
            "max_validation_bytes must be between 1 byte and 16 TiB",
        ));
    }
    Ok(())
}

fn validate_regions(
    reader: &dyn SourceReader,
    regions: &[CarveRegion],
    limits: CarveLimits,
) -> Result<u64, CarveError> {
    if regions.len() > limits.max_regions {
        return Err(CarveError::RegionLimitExceeded {
            requested: regions.len(),
            max: limits.max_regions,
        });
    }

    let mut requested = 0u64;
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(regions.len())
        .map_err(|_| CarveError::AllocationFailed)?;
    for (index, carve_region) in regions.iter().enumerate() {
        if carve_region.region.end() > reader.len() {
            return Err(CarveError::RegionOutOfBounds {
                index,
                region: carve_region.region,
                source_len: reader.len(),
            });
        }
        requested = requested
            .checked_add(carve_region.region.len)
            .ok_or(CarveError::RegionSizeOverflow)?;
        ordered.push((carve_region.region.offset, carve_region.region.end(), index));
    }
    if requested > limits.max_scan_bytes {
        return Err(CarveError::ScanByteLimitExceeded {
            requested,
            max: limits.max_scan_bytes,
        });
    }

    ordered.sort_unstable();
    for adjacent in ordered.windows(2) {
        let (_, first_end, first_index) = adjacent[0];
        let (second_start, _, second_index) = adjacent[1];
        if second_start < first_end {
            return Err(CarveError::OverlappingRegions {
                first_index,
                second_index,
            });
        }
    }
    Ok(requested)
}

fn scan_region(
    reader: &dyn SourceReader,
    carve_region: CarveRegion,
    limits: CarveLimits,
    report: &mut CarveReport,
) -> Result<bool, CarveError> {
    if carve_region.region.is_empty() {
        return Ok(false);
    }

    let buffer_len = limits
        .chunk_size
        .min(usize::try_from(carve_region.region.len).unwrap_or(usize::MAX));
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(buffer_len)
        .map_err(|_| CarveError::AllocationFailed)?;
    buffer.resize(buffer_len, 0);

    let mut relative_offset = 0u64;
    let mut tail: Option<(u64, u8, bool)> = None;
    while relative_offset < carve_region.region.len {
        let absolute = carve_region
            .region
            .offset
            .checked_add(relative_offset)
            .ok_or(CarveError::RegionSizeOverflow)?;
        let remaining = carve_region.region.len - relative_offset;
        let read_len = limits
            .chunk_size
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let slice = &mut buffer[..read_len];
        let outcome = reader.read_best_effort_at(absolute, slice);
        let bad_ranges = normalize_bad_ranges(read_len, &outcome);
        let readable = read_len as u64
            - bad_ranges
                .iter()
                .map(|(start, end)| (end - start) as u64)
                .sum::<u64>();
        report.coverage.bytes_scanned = report
            .coverage
            .bytes_scanned
            .checked_add(readable)
            .ok_or(CarveError::RegionSizeOverflow)?;
        for (start, end) in &bad_ranges {
            push_read_issue(
                &mut report.coverage,
                CarveReadIssue {
                    offset: absolute + *start as u64,
                    len: (*end - *start) as u64,
                },
            );
        }

        if let Some((tail_offset, tail_byte, tail_valid)) = tail {
            if tail_valid
                && is_readable(0, &bad_ranges)
                && tail_byte == 0xFF
                && slice[0] == 0xD8
                && process_signature(reader, tail_offset, carve_region, limits, report)?
            {
                return Ok(true);
            }
        }

        for index in 0..read_len.saturating_sub(SIGNATURE_OVERLAP) {
            if slice[index] == 0xFF
                && slice[index + 1] == 0xD8
                && is_readable(index, &bad_ranges)
                && is_readable(index + 1, &bad_ranges)
            {
                let signature_offset = absolute
                    .checked_add(index as u64)
                    .ok_or(CarveError::RegionSizeOverflow)?;
                if process_signature(reader, signature_offset, carve_region, limits, report)? {
                    return Ok(true);
                }
            }
        }

        let last_index = read_len - 1;
        tail = Some((
            absolute + last_index as u64,
            slice[last_index],
            is_readable(last_index, &bad_ranges),
        ));
        relative_offset = relative_offset
            .checked_add(read_len as u64)
            .ok_or(CarveError::RegionSizeOverflow)?;
    }
    Ok(false)
}

fn process_signature(
    reader: &dyn SourceReader,
    signature_offset: u64,
    carve_region: CarveRegion,
    limits: CarveLimits,
    report: &mut CarveReport,
) -> Result<bool, CarveError> {
    if report.coverage.signatures_attempted >= limits.max_signature_attempts {
        report.coverage.signature_attempt_limit_reached = true;
        mark_partial(
            &mut report.coverage,
            CarvePartialReason::SignatureAttemptLimit,
        );
        return Ok(true);
    }
    report.coverage.signatures_attempted += 1;
    match read_and_inspect_candidate(
        reader,
        signature_offset,
        carve_region.region.end(),
        limits,
        &mut report.coverage,
    )? {
        CandidateRead::Valid {
            len,
            content_sha256,
        } => {
            report
                .candidates
                .try_reserve(1)
                .map_err(|_| CarveError::AllocationFailed)?;
            report
                .evidence
                .try_reserve(1)
                .map_err(|_| CarveError::AllocationFailed)?;
            let candidate = carved_candidate(signature_offset, len, carve_region.availability);
            report.evidence.push(CarveEvidence {
                candidate_id: candidate.id,
                physical_offset: signature_offset,
                len,
                content_sha256,
                validator: JPEG_VALIDATOR_VERSION,
            });
            report.candidates.push(candidate);
            if report.candidates.len() == limits.max_candidates {
                report.coverage.candidate_limit_reached = true;
                mark_partial(&mut report.coverage, CarvePartialReason::CandidateLimit);
                return Ok(true);
            }
        }
        CandidateRead::Invalid => {
            report.coverage.rejected_signatures =
                report.coverage.rejected_signatures.saturating_add(1);
        }
        CandidateRead::Truncated => {
            report.coverage.truncated_signatures =
                report.coverage.truncated_signatures.saturating_add(1);
        }
        CandidateRead::ByteLimit => {
            report.coverage.candidate_byte_limit_hits =
                report.coverage.candidate_byte_limit_hits.saturating_add(1);
            mark_partial(&mut report.coverage, CarvePartialReason::CandidateByteLimit);
        }
        CandidateRead::ValidationByteLimit => {
            report.coverage.validation_byte_limit_reached = true;
            mark_partial(
                &mut report.coverage,
                CarvePartialReason::ValidationByteLimit,
            );
            return Ok(true);
        }
        CandidateRead::ReadFailure => {}
    }
    Ok(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateRead {
    Valid { len: u64, content_sha256: [u8; 32] },
    Invalid,
    Truncated,
    ByteLimit,
    ValidationByteLimit,
    ReadFailure,
}

fn read_and_inspect_candidate(
    reader: &dyn SourceReader,
    start: u64,
    region_end: u64,
    limits: CarveLimits,
    coverage: &mut CarveCoverage,
) -> Result<CandidateRead, CarveError> {
    let available = region_end
        .checked_sub(start)
        .ok_or(CarveError::RegionSizeOverflow)?;
    let candidate_permitted = available.min(limits.max_candidate_bytes);
    let validation_remaining = limits
        .max_validation_bytes
        .saturating_sub(coverage.validation_bytes_read);
    if validation_remaining == 0 {
        return Ok(CandidateRead::ValidationByteLimit);
    }
    let permitted = candidate_permitted.min(validation_remaining);
    let mut read_buffer = Vec::new();
    let buffer_len = limits.chunk_size.min(permitted as usize);
    read_buffer
        .try_reserve_exact(buffer_len)
        .map_err(|_| CarveError::AllocationFailed)?;
    read_buffer.resize(buffer_len, 0);

    let mut validator = JpegValidator::new();
    let mut hasher = Sha256::new();
    let mut consumed = 0u64;
    while consumed < permitted {
        let remaining = permitted - consumed;
        let preferred = if consumed == 0 {
            INITIAL_VALIDATION_READ_BYTES.min(limits.chunk_size)
        } else {
            limits.chunk_size
        };
        let read_len = preferred.min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let absolute = start
            .checked_add(consumed)
            .ok_or(CarveError::RegionSizeOverflow)?;
        let outcome = reader.read_best_effort_at(absolute, &mut read_buffer[..read_len]);
        coverage.validation_bytes_read = coverage
            .validation_bytes_read
            .checked_add(read_len as u64)
            .ok_or(CarveError::RegionSizeOverflow)?;
        let bad_ranges = normalize_bad_ranges(read_len, &outcome);
        if !bad_ranges.is_empty() {
            for (bad_start, bad_end) in bad_ranges {
                push_read_issue(
                    coverage,
                    CarveReadIssue {
                        offset: absolute + bad_start as u64,
                        len: (bad_end - bad_start) as u64,
                    },
                );
            }
            return Ok(CandidateRead::ReadFailure);
        }
        let feed = validator.feed(&read_buffer[..read_len]);
        hasher.update(&read_buffer[..feed.consumed]);
        consumed = consumed
            .checked_add(read_len as u64)
            .ok_or(CarveError::RegionSizeOverflow)?;
        match feed.status {
            JpegStatus::Valid => {
                return Ok(CandidateRead::Valid {
                    len: validator.processed_bytes(),
                    content_sha256: hasher.finalize().into(),
                });
            }
            JpegStatus::Invalid => return Ok(CandidateRead::Invalid),
            JpegStatus::NeedMore => {}
        }
    }

    if permitted < candidate_permitted {
        Ok(CandidateRead::ValidationByteLimit)
    } else if candidate_permitted < available {
        Ok(CandidateRead::ByteLimit)
    } else {
        Ok(CandidateRead::Truncated)
    }
}

fn normalize_bad_ranges(
    requested_len: usize,
    outcome: &um_core::ReadOutcome,
) -> Vec<(usize, usize)> {
    if outcome.bad_ranges.len() > MAX_BAD_RANGES_PER_READ {
        return vec![(0, requested_len)];
    }
    let mut ranges = Vec::with_capacity(outcome.bad_ranges.len());
    for &(offset, len) in &outcome.bad_ranges {
        let Some(end) = offset.checked_add(len) else {
            return vec![(0, requested_len)];
        };
        let Ok(start) = usize::try_from(offset) else {
            return vec![(0, requested_len)];
        };
        let Ok(end) = usize::try_from(end) else {
            return vec![(0, requested_len)];
        };
        if start > end || end > requested_len {
            return vec![(0, requested_len)];
        }
        if start < end {
            ranges.push((start, end));
        }
    }
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    let bad_bytes = merged
        .iter()
        .map(|(start, end)| (end - start) as u64)
        .sum::<u64>();
    let expected_valid = requested_len as u64 - bad_bytes;
    if outcome.bytes_valid != expected_valid {
        vec![(0, requested_len)]
    } else {
        merged
    }
}

fn is_readable(index: usize, bad_ranges: &[(usize, usize)]) -> bool {
    bad_ranges
        .iter()
        .all(|(start, end)| index < *start || index >= *end)
}

fn push_read_issue(coverage: &mut CarveCoverage, issue: CarveReadIssue) {
    mark_partial(coverage, CarvePartialReason::ReadError);
    if let Some(last) = coverage.read_errors.last_mut() {
        let last_end = last.offset.saturating_add(last.len);
        let issue_end = issue.offset.saturating_add(issue.len);
        if issue.offset <= last_end && issue_end >= last.offset {
            let merged_start = last.offset.min(issue.offset);
            let merged_end = last_end.max(issue_end);
            last.offset = merged_start;
            last.len = merged_end - merged_start;
            return;
        }
    }
    if coverage.read_errors.len() < MAX_REPORTED_READ_ISSUES {
        coverage.read_errors.push(issue);
    } else {
        coverage.read_errors_omitted = coverage.read_errors_omitted.saturating_add(1);
    }
}

fn mark_partial(coverage: &mut CarveCoverage, reason: CarvePartialReason) {
    coverage.partial = true;
    if !coverage.partial_reasons.contains(&reason) {
        coverage.partial_reasons.push(reason);
    }
}

fn carved_candidate(physical_offset: u64, len: u64, availability: ExtentAvailability) -> Candidate {
    Candidate {
        id: physical_offset,
        kind: CandidateKind::File,
        method: DiscoveryMethod::Carving,
        state: CandidateState::StructurallyValid,
        name: format!("carved-{physical_offset:016X}.jpg"),
        name_certain: false,
        parent_path: Vec::new(),
        metadata_confidence: MetadataConfidence::Low,
        size: len,
        timestamps: Timestamps::default(),
        extents: vec![ExtentRun {
            logical_offset: 0,
            physical_offset: Some(physical_offset),
            len,
            availability,
        }],
        record_ref: physical_offset,
        sequence: None,
        warnings: vec![
            "Signature carving cannot establish the original file name, path, or timestamps."
                .into(),
        ],
    }
}
