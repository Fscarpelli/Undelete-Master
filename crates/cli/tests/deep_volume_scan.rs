use std::collections::HashSet;

use sha2::{Digest, Sha256};
use um_carving::CarveError;
use um_cli::{CliError, VolumeScanMode, VolumeScanStatus};
use um_core::{
    CandidateState, DiscoveryMethod, ExtentAvailability, ReadError, ReadOutcome, Region,
    SectorLayout, SourceIdentity, SourceKind, SourceReader,
};
use um_fixture_builder::carving::structurally_valid_jpeg;
use um_fixture_builder::fat::{FatImageBuilder, FatKind};
use um_fixture_builder::ntfs::{FileOptions, NodeParent as NtfsParent, NtfsImageBuilder};
use um_io_common::MemImageReader;

#[test]
fn cli_deep_jpeg_001_discovers_orphan_content_only_in_proven_free_space() {
    let jpeg = structurally_valid_jpeg();
    let expected_hash = <[u8; 32]>::from(Sha256::digest(&jpeg));
    let mut builder = NtfsImageBuilder::new("cli-deep-jpeg");
    builder.add_orphan_content(jpeg.clone());
    let (image, _) = builder.build();
    let expected_offset = image
        .windows(jpeg.len())
        .position(|window| window == jpeg)
        .expect("fixture JPEG placement") as u64;
    let reader = MemImageReader::new("cli-deep-jpeg", image);

    let details =
        um_cli::scan_volume_reader_with_mode(&reader, VolumeScanMode::DeepJpeg).expect("deep scan");

    assert_eq!(details.candidates.len(), 1);
    let candidate = &details.candidates[0];
    assert_eq!(candidate.method, DiscoveryMethod::Carving);
    assert_eq!(candidate.state, CandidateState::StructurallyValid);
    assert_eq!(candidate.record_ref, expected_offset);
    assert_eq!(candidate.size, jpeg.len() as u64);
    assert_eq!(
        candidate.extents[0].availability,
        ExtentAvailability::FreeInSnapshot
    );

    let coverage = details
        .report
        .jpeg_carve_coverage
        .expect("deep coverage must be reported");
    assert!(coverage.bytes_requested > 0);
    assert_eq!(coverage.bytes_requested, coverage.bytes_scanned);
    assert!(!coverage.partial);

    assert_eq!(details.carve_evidence.len(), 1);
    assert_eq!(details.carve_evidence[0].candidate_id, candidate.id);
    assert_eq!(details.carve_evidence[0].physical_offset, expected_offset);
    assert_eq!(details.carve_evidence[0].content_sha256, expected_hash);
    assert_eq!(details.carve_evidence[0].validator, "jpeg-structural-v1");
    assert_eq!(details.report.candidate_count, 1);
    assert_eq!(details.report.scan_status, VolumeScanStatus::Complete);
    assert_eq!(reader.len(), details.report.length_bytes);
}

#[test]
fn cli_deep_metadata_compat_002_keeps_the_legacy_entry_point_metadata_only() {
    let jpeg = structurally_valid_jpeg();
    let mut builder = NtfsImageBuilder::new("cli-metadata-only");
    builder.add_orphan_content(jpeg);
    let (image, _) = builder.build();
    let reader = MemImageReader::new("cli-metadata-only", image);

    let legacy = um_cli::scan_volume_reader(&reader).expect("legacy metadata scan");
    let explicit = um_cli::scan_volume_reader_with_mode(&reader, VolumeScanMode::MetadataOnly)
        .expect("explicit metadata scan");

    assert!(legacy.candidates.is_empty());
    assert!(legacy.carve_evidence.is_empty());
    assert!(legacy.report.jpeg_carve_coverage.is_none());
    assert_eq!(legacy.report, explicit.report);
    assert_eq!(legacy.candidates.len(), explicit.candidates.len());
}

#[test]
fn cli_deep_free_only_003_never_carves_an_allocated_active_jpeg() {
    let jpeg = structurally_valid_jpeg();
    let mut builder = NtfsImageBuilder::new("cli-deep-allocated");
    builder.add_file(
        NtfsParent::Root,
        "active.jpg",
        jpeg,
        false,
        FileOptions {
            force_resident: Some(false),
            ..FileOptions::default()
        },
    );
    let (image, _) = builder.build();
    let reader = MemImageReader::new("cli-deep-allocated", image);

    let details =
        um_cli::scan_volume_reader_with_mode(&reader, VolumeScanMode::DeepJpeg).expect("deep scan");

    assert!(details.candidates.is_empty());
    assert!(details.carve_evidence.is_empty());
    assert_eq!(details.report.candidate_count, 0);
    assert_eq!(details.report.scan_status, VolumeScanStatus::Complete);
    let coverage = details.report.jpeg_carve_coverage.unwrap();
    assert!(!coverage.partial);
    assert!(coverage.bytes_requested > 0);
}

#[test]
fn cli_deep_unknown_004_does_not_fallback_to_raw_when_bitmap_is_unreadable() {
    let jpeg = structurally_valid_jpeg();
    let mut builder = NtfsImageBuilder::new("cli-deep-unknown");
    builder.add_orphan_content(jpeg);
    let (image, _) = builder.build();
    let reader = BitmapFaultReader::new(image);

    let details = um_cli::scan_volume_reader_with_mode(&reader, VolumeScanMode::DeepJpeg)
        .expect("metadata scan survives unavailable bitmap");

    assert!(details.candidates.is_empty());
    assert!(details.carve_evidence.is_empty());
    let coverage = details.report.jpeg_carve_coverage.unwrap();
    assert_eq!(coverage.bytes_requested, 0);
    assert_eq!(coverage.bytes_scanned, 0);
    assert!(coverage.partial);
    assert_eq!(coverage.regions_submitted, 0);
    assert_eq!(details.report.scan_status, VolumeScanStatus::Partial);
    let joined = details.report.warnings.join("\n");
    assert!(joined.contains("allocation evidence"));
    assert!(!joined.contains("UNIQUE-SECRET-BITMAP-READ"));
}

#[test]
fn cli_deep_dedupe_005_merges_metadata_and_carving_evidence_by_physical_range() {
    let jpeg = structurally_valid_jpeg();
    let expected_hash = <[u8; 32]>::from(Sha256::digest(&jpeg));
    let mut builder = NtfsImageBuilder::new("cli-deep-dedupe");
    builder.add_file(
        NtfsParent::Root,
        "deleted.jpg",
        jpeg,
        true,
        FileOptions {
            force_resident: Some(false),
            ..FileOptions::default()
        },
    );
    let (image, _) = builder.build();
    let reader = MemImageReader::new("cli-deep-dedupe", image);

    let details =
        um_cli::scan_volume_reader_with_mode(&reader, VolumeScanMode::DeepJpeg).expect("deep scan");

    assert_eq!(details.candidates.len(), 1);
    assert_eq!(details.candidates[0].method, DiscoveryMethod::NtfsMetadata);
    assert_eq!(details.carve_evidence.len(), 1);
    assert_eq!(
        details.carve_evidence[0].candidate_id,
        details.candidates[0].id
    );
    assert_eq!(details.carve_evidence[0].content_sha256, expected_hash);
    assert_eq!(details.report.candidate_count, 1);
}

#[test]
fn cli_deep_id_006_rekeys_carved_candidates_into_one_global_id_space() {
    let jpeg = structurally_valid_jpeg();
    let mut builder = NtfsImageBuilder::new("cli-deep-id");
    builder.add_file(
        NtfsParent::Root,
        "deleted.txt",
        vec![0x41; 900],
        true,
        FileOptions {
            force_resident: Some(false),
            ..FileOptions::default()
        },
    );
    builder.add_orphan_content(jpeg);
    let (image, _) = builder.build();
    let reader = MemImageReader::new("cli-deep-id", image);

    let details =
        um_cli::scan_volume_reader_with_mode(&reader, VolumeScanMode::DeepJpeg).expect("deep scan");

    assert_eq!(details.candidates.len(), 2);
    let ids: HashSet<_> = details
        .candidates
        .iter()
        .map(|candidate| candidate.id)
        .collect();
    assert_eq!(ids.len(), details.candidates.len());
    let carved = details
        .candidates
        .iter()
        .find(|candidate| candidate.method == DiscoveryMethod::Carving)
        .expect("orphan carved candidate");
    let metadata_max = details
        .candidates
        .iter()
        .filter(|candidate| candidate.method == DiscoveryMethod::NtfsMetadata)
        .map(|candidate| candidate.id)
        .max()
        .unwrap();
    assert_eq!(carved.id, metadata_max + 1);
    assert_ne!(carved.id, carved.record_ref);
    assert_eq!(details.carve_evidence[0].candidate_id, carved.id);
}

#[test]
fn cli_deep_error_privacy_007_hides_internal_carver_details_from_display() {
    let error = CliError::DeepScan {
        index: 7,
        source: CarveError::InvalidLimits("UNIQUE-SECRET-CARVER-MARKER"),
    };

    assert_eq!(
        error.to_string(),
        "JPEG deep scan failed safely for volume 7"
    );
    assert!(!error.to_string().contains("UNIQUE-SECRET-CARVER-MARKER"));
}

#[test]
fn cli_deep_fat_008_never_treats_a_non_ntfs_volume_as_raw_free_space() {
    let builder = FatImageBuilder::new("cli-deep-fat", FatKind::Fat16);
    let (mut image, _) = builder.build();
    let jpeg = structurally_valid_jpeg();
    let tail = image.len() - jpeg.len() - 512;
    image[tail..tail + jpeg.len()].copy_from_slice(&jpeg);
    let reader = MemImageReader::new("cli-deep-fat", image);

    let details = um_cli::scan_volume_reader_with_mode(&reader, VolumeScanMode::DeepJpeg)
        .expect("FAT metadata scan");

    assert!(details.candidates.is_empty());
    assert!(details.carve_evidence.is_empty());
    assert_eq!(details.report.file_system, "fat16");
    assert_eq!(details.report.scan_status, VolumeScanStatus::Partial);
    let coverage = details.report.jpeg_carve_coverage.unwrap();
    assert_eq!(coverage.bytes_requested, 0);
    assert!(coverage.partial);
}

const DEFAULT_BITMAP_OFFSET: u64 = 20 * 4096;
const DEFAULT_BITMAP_LEN: u64 = 128;

struct BitmapFaultReader {
    data: Vec<u8>,
    identity: SourceIdentity,
}

impl BitmapFaultReader {
    fn new(data: Vec<u8>) -> Self {
        Self {
            identity: SourceIdentity {
                id: "synthetic:bitmap-fault".into(),
                kind: SourceKind::ImageFile,
                label: "bitmap-fault.img".into(),
                size: data.len() as u64,
            },
            data,
        }
    }

    fn overlaps_bitmap(offset: u64, len: u64) -> bool {
        let Some(end) = offset.checked_add(len) else {
            return true;
        };
        offset < DEFAULT_BITMAP_OFFSET + DEFAULT_BITMAP_LEN && end > DEFAULT_BITMAP_OFFSET
    }
}

impl SourceReader for BitmapFaultReader {
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
        let len = buffer.len() as u64;
        let region = Region::new(offset, len).ok_or(ReadError::OutOfBounds {
            offset,
            len,
            source_len: self.len(),
        })?;
        if region.end() > self.len() {
            return Err(ReadError::OutOfBounds {
                offset,
                len,
                source_len: self.len(),
            });
        }
        if Self::overlaps_bitmap(offset, len) {
            return Err(ReadError::Io {
                offset,
                message: "UNIQUE-SECRET-BITMAP-READ".into(),
            });
        }
        buffer.copy_from_slice(&self.data[offset as usize..region.end() as usize]);
        Ok(())
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        match self.read_exact_at(offset, buffer) {
            Ok(()) => ReadOutcome::complete(buffer.len() as u64),
            Err(_) => {
                buffer.fill(0);
                ReadOutcome {
                    bytes_valid: 0,
                    bad_ranges: vec![(0, buffer.len() as u64)],
                }
            }
        }
    }
}
