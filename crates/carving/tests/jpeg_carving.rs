use sha2::{Digest, Sha256};
use um_carving::{
    carve_jpegs, carve_jpegs_with_evidence, CarveError, CarveLimits, CarvePartialReason,
    CarveRegion,
};
use um_core::{
    CandidateState, DiscoveryMethod, ExtentAvailability, ReadError, ReadOutcome, Region,
    SectorLayout, SourceIdentity, SourceKind, SourceReader,
};
use um_fixture_builder::carving::{
    jpeg_carving_fixture, structurally_valid_jpeg, JPEG_BOUNDARY_CHUNK_SIZE,
};
use um_io_common::MemImageReader;

fn limits() -> CarveLimits {
    CarveLimits {
        chunk_size: JPEG_BOUNDARY_CHUNK_SIZE,
        max_candidate_bytes: 1_024,
        max_candidates: 16,
        max_regions: 8,
        max_scan_bytes: 1_000_000,
        max_signature_attempts: 64,
        max_validation_bytes: 1_000_000,
    }
}

#[test]
fn carve_jpeg_boundary_001_finds_signature_split_across_chunks() {
    let fixture = jpeg_carving_fixture();
    let reader = MemImageReader::new("jpeg-boundary", fixture.image.clone());

    let report =
        carve_jpegs(&reader, &[fixture.scan_region], limits()).expect("bounded fixture scan");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].record_ref, fixture.valid_offset);
    assert_eq!(report.coverage.bytes_requested, fixture.image.len() as u64);
    assert_eq!(report.coverage.bytes_scanned, fixture.image.len() as u64);
    assert!(!report.coverage.partial);
}

#[test]
fn carve_jpeg_validation_001_rejects_malformed_structures_and_accepts_entropy_rules() {
    let fixture = jpeg_carving_fixture();
    let reader = MemImageReader::new("jpeg-validation", fixture.image.clone());

    let report =
        carve_jpegs(&reader, &[fixture.scan_region], limits()).expect("bounded fixture scan");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.coverage.rejected_signatures, 3);
    assert_eq!(report.coverage.truncated_signatures, 1);
    assert!(fixture.false_positive_offsets.iter().all(|offset| report
        .candidates
        .iter()
        .all(|item| item.record_ref != *offset)));
    assert!(report
        .candidates
        .iter()
        .all(|item| item.record_ref != fixture.truncated_offset));
}

#[test]
fn carve_provenance_sha256_001_emits_physical_structurally_valid_evidence() {
    let fixture = jpeg_carving_fixture();
    let reader = MemImageReader::new("jpeg-provenance", fixture.image.clone());

    let report = carve_jpegs_with_evidence(
        &reader,
        &[CarveRegion {
            region: fixture.scan_region,
            availability: ExtentAvailability::FreeInSnapshot,
        }],
        limits(),
    )
    .expect("bounded fixture scan");
    let candidate = &report.candidates[0];
    let evidence = &report.evidence[0];

    assert_eq!(
        fixture.valid_sha256,
        "c13f28eef684e4d67e6ad66c841fe8034edd7ed780feff3dac9119d39779bcc7"
    );
    assert_eq!(candidate.method, DiscoveryMethod::Carving);
    assert_eq!(candidate.state, CandidateState::StructurallyValid);
    assert!(!candidate.name_certain);
    assert_eq!(candidate.size, fixture.valid_jpeg.len() as u64);
    assert_eq!(candidate.extents.len(), 1);
    assert_eq!(candidate.extents[0].logical_offset, 0);
    assert_eq!(
        candidate.extents[0].physical_offset,
        Some(fixture.valid_offset)
    );
    assert_eq!(
        candidate.extents[0].availability,
        ExtentAvailability::FreeInSnapshot
    );
    assert_eq!(evidence.candidate_id, candidate.id);
    assert_eq!(evidence.physical_offset, fixture.valid_offset);
    assert_eq!(evidence.len, fixture.valid_jpeg.len() as u64);
    assert_eq!(evidence.validator, "jpeg-structural-v1");
    assert_eq!(hex::encode(evidence.content_sha256), fixture.valid_sha256);
    let start = candidate.record_ref as usize;
    let end = start + candidate.size as usize;
    assert_eq!(
        hex::encode(Sha256::digest(&fixture.image[start..end])),
        fixture.valid_sha256
    );
}

#[test]
fn carve_region_bounds_001_rejects_out_of_source_region() {
    let reader = MemImageReader::new("bounds", vec![0; 64]);
    let region = Region::new(60, 8).unwrap();

    let error = carve_jpegs(&reader, &[region], limits())
        .expect_err("out-of-bounds region must fail closed");

    assert_eq!(
        error,
        CarveError::RegionOutOfBounds {
            index: 0,
            region,
            source_len: 64,
        }
    );
}

#[test]
fn carve_limit_candidate_bytes_001_marks_unknown_oversized_signature_partial() {
    let jpeg = structurally_valid_jpeg();
    let reader = MemImageReader::new("candidate-byte-limit", jpeg.clone());
    let mut bounded = limits();
    bounded.max_candidate_bytes = jpeg.len() as u64 - 1;

    let report = carve_jpegs(
        &reader,
        &[Region::new(0, jpeg.len() as u64).unwrap()],
        bounded,
    )
    .expect("bounded scan");

    assert!(report.candidates.is_empty());
    assert_eq!(report.coverage.candidate_byte_limit_hits, 1);
    assert!(report.coverage.partial);
    assert!(report
        .coverage
        .partial_reasons
        .contains(&CarvePartialReason::CandidateByteLimit));
}

#[test]
fn carve_limit_candidate_count_001_stops_at_output_budget_and_marks_partial() {
    let jpeg = structurally_valid_jpeg();
    let mut image = jpeg.clone();
    image.extend_from_slice(&[0x11; 17]);
    image.extend_from_slice(&jpeg);
    let reader = MemImageReader::new("candidate-count-limit", image.clone());
    let mut bounded = limits();
    bounded.chunk_size = 8;
    bounded.max_candidates = 1;

    let report = carve_jpegs(
        &reader,
        &[Region::new(0, image.len() as u64).unwrap()],
        bounded,
    )
    .expect("bounded scan");

    assert_eq!(report.candidates.len(), 1);
    assert!(report.coverage.candidate_limit_reached);
    assert!(report.coverage.partial);
    assert!(report
        .coverage
        .partial_reasons
        .contains(&CarvePartialReason::CandidateLimit));
}

#[test]
fn carve_read_partial_001_preserves_valid_results_and_reports_unread_bytes() {
    let fixture = jpeg_carving_fixture();
    let reader = FaultyReader::new(fixture.image.clone(), Region::new(4, 5).unwrap());

    let report = carve_jpegs(&reader, &[fixture.scan_region], limits()).expect("best-effort scan");

    assert_eq!(report.candidates.len(), 1);
    assert!(report.coverage.partial);
    assert_eq!(
        report.coverage.bytes_scanned,
        fixture.image.len() as u64 - 5
    );
    assert_eq!(
        report.coverage.read_errors,
        vec![um_carving::CarveReadIssue { offset: 4, len: 5 }]
    );
    assert!(report
        .coverage
        .partial_reasons
        .contains(&CarvePartialReason::ReadError));
}

#[test]
fn carve_signature_budget_002_stops_hostile_false_positive_fanout() {
    let mut image = Vec::new();
    for _ in 0..10 {
        image.extend_from_slice(&[0xFF, 0xD8, 0x00, 0x11]);
    }
    let reader = MemImageReader::new("signature-budget", image.clone());
    let mut bounded = limits();
    bounded.chunk_size = 8;
    bounded.max_signature_attempts = 3;

    let report = carve_jpegs(
        &reader,
        &[Region::new(0, image.len() as u64).unwrap()],
        bounded,
    )
    .expect("bounded hostile signature scan");

    assert!(report.candidates.is_empty());
    assert_eq!(report.coverage.signatures_attempted, 3);
    assert!(report.coverage.signature_attempt_limit_reached);
    assert!(report.coverage.partial);
    assert!(report
        .coverage
        .partial_reasons
        .contains(&CarvePartialReason::SignatureAttemptLimit));
}

#[test]
fn carve_validation_budget_003_caps_aggregate_candidate_reads() {
    let mut image = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x01, 0x00];
    image.extend_from_slice(&[0x42; 250]);
    let reader = MemImageReader::new("validation-budget", image.clone());
    let mut bounded = limits();
    bounded.chunk_size = 8;
    bounded.max_validation_bytes = 32;

    let report = carve_jpegs(
        &reader,
        &[Region::new(0, image.len() as u64).unwrap()],
        bounded,
    )
    .expect("bounded aggregate validation");

    assert!(report.candidates.is_empty());
    assert_eq!(report.coverage.signatures_attempted, 1);
    assert_eq!(report.coverage.validation_bytes_read, 32);
    assert!(report.coverage.validation_byte_limit_reached);
    assert!(report.coverage.partial);
    assert!(report
        .coverage
        .partial_reasons
        .contains(&CarvePartialReason::ValidationByteLimit));
}

#[test]
fn carve_jpeg_tem_004_accepts_the_legal_standalone_tem_marker() {
    let jpeg = structurally_valid_jpeg();
    let mut with_tem = vec![0xFF, 0xD8, 0xFF, 0x01];
    with_tem.extend_from_slice(&jpeg[2..]);
    let reader = MemImageReader::new("jpeg-tem", with_tem.clone());

    let report = carve_jpegs(
        &reader,
        &[Region::new(0, with_tem.len() as u64).unwrap()],
        limits(),
    )
    .expect("TEM-bearing JPEG");

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.candidates[0].size, with_tem.len() as u64);
    assert_eq!(report.coverage.rejected_signatures, 0);
}

struct FaultyReader {
    data: Vec<u8>,
    bad: Region,
    identity: SourceIdentity,
}

impl FaultyReader {
    fn new(data: Vec<u8>, bad: Region) -> Self {
        Self {
            identity: SourceIdentity {
                id: "faulty:jpeg".into(),
                kind: SourceKind::ImageFile,
                label: "faulty-jpeg".into(),
                size: data.len() as u64,
            },
            data,
            bad,
        }
    }
}

impl SourceReader for FaultyReader {
    fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn sector_layout(&self) -> SectorLayout {
        SectorLayout::DEFAULT_512
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        let requested = Region::new(offset, buffer.len() as u64).ok_or(ReadError::OutOfBounds {
            offset,
            len: buffer.len() as u64,
            source_len: self.len(),
        })?;
        if requested.end() > self.len() || requested.intersect(&self.bad).is_some() {
            return Err(ReadError::Io {
                offset,
                message: "synthetic unreadable range".into(),
            });
        }
        buffer.copy_from_slice(&self.data[offset as usize..requested.end() as usize]);
        Ok(())
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        let Some(requested) = Region::new(offset, buffer.len() as u64) else {
            buffer.fill(0);
            return ReadOutcome {
                bytes_valid: 0,
                bad_ranges: vec![(0, buffer.len() as u64)],
            };
        };
        if requested.end() > self.len() {
            buffer.fill(0);
            return ReadOutcome {
                bytes_valid: 0,
                bad_ranges: vec![(0, buffer.len() as u64)],
            };
        }
        buffer.copy_from_slice(&self.data[offset as usize..requested.end() as usize]);
        let Some(bad) = requested.intersect(&self.bad) else {
            return ReadOutcome::complete(buffer.len() as u64);
        };
        let relative = bad.offset - offset;
        buffer[relative as usize..(relative + bad.len) as usize].fill(0);
        ReadOutcome {
            bytes_valid: buffer.len() as u64 - bad.len,
            bad_ranges: vec![(relative, bad.len)],
        }
    }
}
