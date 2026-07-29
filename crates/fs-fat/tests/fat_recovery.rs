//! End-to-end FAT16/FAT32 recovery tests against deterministic fixtures.

use sha2::{Digest, Sha256};
use um_core::{
    extract_candidate, Candidate, CandidateKind, CandidateState, MetadataConfidence,
    RecoverabilityInputs,
};
use um_fixture_builder::deterministic_bytes;
use um_fixture_builder::fat::{FatFileOptions, FatImageBuilder, FatKind, NodeParent};
use um_fs_fat::FatVariant;
use um_io_common::MemImageReader;

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn find<'a>(candidates: &'a [Candidate], name: &str) -> &'a Candidate {
    candidates
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| {
            panic!(
                "candidate {name} not found in {:?}",
                candidates.iter().map(|c| &c.name).collect::<Vec<_>>()
            )
        })
}

#[test]
fn fat32_recovers_deleted_files_byte_exact() {
    let mut b = FatImageBuilder::new("fat32-basic", FatKind::Fat32);
    b.add_file(
        NodeParent::Root,
        "alive.txt",
        b"still alive".to_vec(),
        false,
        FatFileOptions::default(),
    );
    let content = deterministic_bytes(11, 5000);
    b.add_file(
        NodeParent::Root,
        "Foto de Família.jpg",
        content.clone(),
        true,
        FatFileOptions::default(),
    );

    let (image, manifest) = b.build();
    let reader = MemImageReader::new("fat32", image);
    let out = um_fs_fat::scan_fat(&reader).unwrap();
    assert_eq!(out.boot.variant, FatVariant::Fat32);
    assert_eq!(out.candidates.len(), 1);

    let cand = find(&out.candidates, "Foto de Família.jpg");
    assert!(cand.name_certain, "LFN must give the exact name");
    assert_eq!(cand.size, 5000);
    let ext = extract_candidate(&reader, cand).unwrap();
    assert_eq!(
        sha256_hex(&ext.bytes),
        manifest.expected_candidates[0].content_sha256
    );
    // Chain was cleared: contiguity assumption must be flagged and capped.
    assert!(cand.warnings.iter().any(|w| w.contains("assumed")));
    let score = RecoverabilityInputs::from_candidate(cand).score();
    assert!(
        score.value <= 74,
        "inferred chain must cap score, got {}",
        score.value
    );
}

#[test]
fn fat32_deleted_without_lfn_loses_first_char() {
    let mut b = FatImageBuilder::new("fat32-nolfn", FatKind::Fat32);
    b.add_file(
        NodeParent::Root,
        "REPORT.PDF",
        b"pdf bytes".to_vec(),
        true,
        FatFileOptions {
            no_lfn: true,
            ..Default::default()
        },
    );
    let (image, _) = b.build();
    let reader = MemImageReader::new("fat32-nolfn", image);
    let out = um_fs_fat::scan_fat(&reader).unwrap();
    let cand = &out.candidates[0];
    assert_eq!(cand.name, "_EPORT.PDF");
    assert!(!cand.name_certain);
}

#[test]
fn fat32_deleted_directory_tree_is_navigable() {
    let mut b = FatImageBuilder::new("fat32-tree", FatKind::Fat32);
    let docs = b.add_dir(NodeParent::Root, "Documentos", true);
    let inner = b.add_dir(NodeParent::Node(docs), "Fiscal", true);
    let data = deterministic_bytes(3, 2000);
    b.add_file(
        NodeParent::Node(inner),
        "nota.xml",
        data.clone(),
        true,
        FatFileOptions::default(),
    );

    let (image, _) = b.build();
    let reader = MemImageReader::new("fat32-tree", image);
    let out = um_fs_fat::scan_fat(&reader).unwrap();

    let dir = find(&out.candidates, "Documentos");
    assert_eq!(dir.kind, CandidateKind::Directory);

    let file = find(&out.candidates, "nota.xml");
    assert_eq!(
        file.parent_path,
        vec!["Documentos".to_string(), "Fiscal".to_string()]
    );
    // Parent chain includes deleted dirs -> not High confidence.
    assert_ne!(file.metadata_confidence, MetadataConfidence::High);
    let ext = extract_candidate(&reader, file).unwrap();
    assert_eq!(ext.bytes, data);
}

#[test]
fn fat32_fragmented_with_retained_chain_recovers_exactly() {
    let mut b = FatImageBuilder::new("fat32-frag-chain", FatKind::Fat32);
    let content = deterministic_bytes(21, 4000);
    b.add_file(
        NodeParent::Root,
        "planilha.xlsx",
        content.clone(),
        true,
        FatFileOptions {
            fragmented: true,
            keep_chain: true,
            ..Default::default()
        },
    );
    let (image, _) = b.build();
    let reader = MemImageReader::new("fat32-frag-chain", image);
    let out = um_fs_fat::scan_fat(&reader).unwrap();
    let cand = find(&out.candidates, "planilha.xlsx");
    assert!(cand
        .warnings
        .iter()
        .any(|w| w.contains("chain still present")));
    let ext = extract_candidate(&reader, cand).unwrap();
    assert_eq!(
        ext.bytes, content,
        "retained chain must recover fragmented file"
    );
}

#[test]
fn fat32_fragmented_with_cleared_chain_is_flagged_not_faked() {
    let mut b = FatImageBuilder::new("fat32-frag-cleared", FatKind::Fat32);
    let content = deterministic_bytes(22, 4000);
    b.add_file(
        NodeParent::Root,
        "backup.zip",
        content.clone(),
        true,
        FatFileOptions {
            fragmented: true,
            ..Default::default()
        },
    );
    let (image, _) = b.build();
    let reader = MemImageReader::new("fat32-frag-cleared", image);
    let out = um_fs_fat::scan_fat(&reader).unwrap();
    let cand = find(&out.candidates, "backup.zip");
    // Contiguity was assumed for a fragmented file: recovery differs, and the
    // scanner must have flagged the inference (honesty requirement).
    assert!(cand.warnings.iter().any(|w| w.contains("assumed")));
    let ext = extract_candidate(&reader, cand).unwrap();
    assert_ne!(ext.bytes, content);
    let score = RecoverabilityInputs::from_candidate(cand).score();
    assert!(score.value <= 74);
}

#[test]
fn fat32_overwritten_cluster_reports_conflict() {
    let mut b = FatImageBuilder::new("fat32-conflict", FatKind::Fat32);
    let content = deterministic_bytes(31, 2048); // 4 clusters of 512
    b.add_file(
        NodeParent::Root,
        "carta.txt",
        content.clone(),
        true,
        FatFileOptions {
            overwrite_ranges: vec![(512, 512)],
            ..Default::default()
        },
    );
    let (image, _) = b.build();
    let reader = MemImageReader::new("fat32-conflict", image);
    let out = um_fs_fat::scan_fat(&reader).unwrap();
    let cand = find(&out.candidates, "carta.txt");
    assert_eq!(cand.state, CandidateState::Conflicted);
    let ext = extract_candidate(&reader, cand).unwrap();
    assert_eq!(&ext.bytes[..512], &content[..512]);
    assert_ne!(&ext.bytes[512..1024], &content[512..1024]);
    assert_eq!(&ext.bytes[1024..], &content[1024..]);
}

#[test]
fn fat16_basic_recovery() {
    let mut b = FatImageBuilder::new("fat16-basic", FatKind::Fat16);
    let content = deterministic_bytes(5, 3000);
    b.add_file(
        NodeParent::Root,
        "antigo.doc",
        content.clone(),
        true,
        FatFileOptions::default(),
    );
    let (image, _) = b.build();
    let reader = MemImageReader::new("fat16", image);
    let out = um_fs_fat::scan_fat(&reader).unwrap();
    assert_eq!(out.boot.variant, FatVariant::Fat16);
    let cand = find(&out.candidates, "antigo.doc");
    let ext = extract_candidate(&reader, cand).unwrap();
    assert_eq!(ext.bytes, content);
}

#[test]
fn scan_survives_garbage() {
    let junk = deterministic_bytes(555, 1 << 20);
    let reader = MemImageReader::new("junk", junk);
    let _ = um_fs_fat::scan_fat(&reader);
}
