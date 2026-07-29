//! End-to-end NTFS recovery tests against deterministic fixtures.
//!
//! Every assertion is hash-based: a candidate is only considered recovered
//! when the extracted bytes match the SHA-256 recorded in the truth manifest.

use sha2::{Digest, Sha256};
use um_core::{
    extract_candidate, Candidate, CandidateKind, CandidateState, ExtentAvailability,
    MetadataConfidence, RecoverabilityInputs,
};
use um_fixture_builder::deterministic_bytes;
use um_fixture_builder::ntfs::{FileOptions, NodeParent, NtfsImageBuilder};
use um_io_common::MemImageReader;

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn find<'a>(candidates: &'a [Candidate], name: &str) -> &'a Candidate {
    candidates
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("candidate {name} not found"))
}

#[test]
fn recovers_deleted_files_byte_exact() {
    let mut b = NtfsImageBuilder::new("ntfs-basic");
    // Live file (must NOT appear as candidate).
    b.add_file(
        NodeParent::Root,
        "alive.txt",
        b"still here".to_vec(),
        false,
        FileOptions::default(),
    );
    // Deleted resident file.
    b.add_file(
        NodeParent::Root,
        "note.txt",
        b"small resident content".to_vec(),
        true,
        FileOptions::default(),
    );
    // Deleted non-resident contiguous file (~3 clusters).
    let big = deterministic_bytes(42, 12_000);
    b.add_file(
        NodeParent::Root,
        "photo.jpg",
        big.clone(),
        true,
        FileOptions::default(),
    );
    // Deleted non-resident fragmented file.
    let frag = deterministic_bytes(77, 20_000);
    b.add_file(
        NodeParent::Root,
        "video.mp4",
        frag.clone(),
        true,
        FileOptions {
            fragmented: true,
            ..Default::default()
        },
    );

    let (image, manifest) = b.build();
    let reader = MemImageReader::new("ntfs-basic", image);
    let out = um_fs_ntfs::scan_ntfs(&reader).expect("scan must succeed");

    // Only the 3 deleted entries become candidates.
    assert_eq!(out.candidates.len(), 3, "candidates: {:?}", out.candidates);
    assert!(out.candidates.iter().all(|c| c.name != "alive.txt"));

    for expected in &manifest.expected_candidates {
        let cand = find(&out.candidates, &expected.name);
        assert_eq!(cand.size, expected.size);
        assert_eq!(cand.metadata_confidence, MetadataConfidence::High);
        assert_eq!(cand.state, CandidateState::CompleteUnvalidated);
        let extraction = extract_candidate(&reader, cand).unwrap();
        assert!(extraction.missing_ranges.is_empty());
        assert_eq!(
            sha256_hex(&extraction.bytes),
            expected.content_sha256,
            "content mismatch for {}",
            expected.name
        );
    }
}

#[test]
fn reconstructs_paths_through_deleted_directories() {
    let mut b = NtfsImageBuilder::new("ntfs-tree");
    let projects = b.add_dir(NodeParent::Root, "Projetos", true);
    let y2024 = b.add_dir(NodeParent::Node(projects), "2024", true);
    b.add_file(
        NodeParent::Node(y2024),
        "Contrato.docx",
        deterministic_bytes(7, 5000),
        true,
        FileOptions::default(),
    );

    let (image, _) = b.build();
    let reader = MemImageReader::new("ntfs-tree", image);
    let out = um_fs_ntfs::scan_ntfs(&reader).unwrap();

    let file = find(&out.candidates, "Contrato.docx");
    assert_eq!(
        file.parent_path,
        vec!["Projetos".to_string(), "2024".to_string()]
    );
    // Ancestors are deleted, so confidence drops to Medium — not Low.
    assert_eq!(file.metadata_confidence, MetadataConfidence::Medium);
    assert_eq!(file.kind, CandidateKind::File);

    // The deleted directories themselves are navigable candidates.
    let dir = find(&out.candidates, "2024");
    assert_eq!(dir.kind, CandidateKind::Directory);
    assert_eq!(dir.parent_path, vec!["Projetos".to_string()]);
}

#[test]
fn partial_overwrite_is_reported_not_hidden() {
    let content = deterministic_bytes(99, 16_384); // 4 clusters
    let mut b = NtfsImageBuilder::new("ntfs-partial");
    b.add_file(
        NodeParent::Root,
        "report.pdf",
        content.clone(),
        true,
        FileOptions {
            // Overwrite the second cluster (bytes 4096..8192) after deletion.
            overwrite_ranges: vec![(4096, 4096)],
            ..Default::default()
        },
    );

    let (image, _) = b.build();
    let reader = MemImageReader::new("ntfs-partial", image);
    let out = um_fs_ntfs::scan_ntfs(&reader).unwrap();

    let cand = find(&out.candidates, "report.pdf");
    assert_eq!(cand.state, CandidateState::Conflicted);
    let conflicted: u64 = cand
        .extents
        .iter()
        .filter(|e| e.availability == ExtentAvailability::CurrentlyAllocated)
        .map(|e| e.len)
        .sum();
    assert_eq!(conflicted, 4096);

    // Score must respect the conflict cap (<= 49) and never claim "excellent".
    let score = RecoverabilityInputs::from_candidate(cand).score();
    assert!(score.value <= 49, "score {} too high", score.value);

    // Extraction still returns the undamaged ranges byte-exact.
    let extraction = extract_candidate(&reader, cand).unwrap();
    assert_eq!(&extraction.bytes[..4096], &content[..4096]);
    assert_eq!(&extraction.bytes[8192..], &content[8192..]);
    // And the damaged range differs (it was scrambled).
    assert_ne!(&extraction.bytes[4096..8192], &content[4096..8192]);
}

#[test]
fn zero_length_and_unicode_names() {
    let mut b = NtfsImageBuilder::new("ntfs-edge");
    b.add_file(
        NodeParent::Root,
        "vazio.dat",
        Vec::new(),
        true,
        FileOptions::default(),
    );
    b.add_file(
        NodeParent::Root,
        "relatório-ção-😀.txt",
        b"unicode!".to_vec(),
        true,
        FileOptions::default(),
    );

    let (image, _) = b.build();
    let reader = MemImageReader::new("ntfs-edge", image);
    let out = um_fs_ntfs::scan_ntfs(&reader).unwrap();

    let empty = find(&out.candidates, "vazio.dat");
    assert_eq!(empty.size, 0);
    assert_eq!(empty.state, CandidateState::CompleteUnvalidated);

    let uni = find(&out.candidates, "relatório-ção-😀.txt");
    let ext = extract_candidate(&reader, uni).unwrap();
    assert_eq!(ext.bytes, b"unicode!");
}

#[test]
fn scan_survives_hostile_random_volume() {
    // A garbage volume must produce an error or empty result, never a panic.
    let junk = deterministic_bytes(1234, 1 << 20);
    let reader = MemImageReader::new("junk", junk);
    let _ = um_fs_ntfs::scan_ntfs(&reader);
}

#[test]
fn timestamps_are_recovered() {
    let mut b = NtfsImageBuilder::new("ntfs-times");
    b.add_file(
        NodeParent::Root,
        "dated.txt",
        b"x".to_vec(),
        true,
        FileOptions::default(),
    );
    let (image, _) = b.build();
    let reader = MemImageReader::new("ntfs-times", image);
    let out = um_fs_ntfs::scan_ntfs(&reader).unwrap();
    let c = find(&out.candidates, "dated.txt");
    // Fixture uses 2024-01-15 12:00:00 UTC.
    assert_eq!(c.timestamps.created_ms, Some(1_705_320_000_000));
    assert_eq!(c.timestamps.modified_ms, Some(1_705_320_000_000));
    // NTFS records carry no deletion time (spec §11.10).
    assert_eq!(c.timestamps.deleted_ms, None);
}
