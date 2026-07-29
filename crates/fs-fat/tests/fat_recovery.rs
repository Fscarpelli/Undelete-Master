//! End-to-end FAT16/FAT32 recovery tests against deterministic fixtures.

use sha2::{Digest, Sha256};
use um_core::{
    extract_candidate, Candidate, CandidateKind, CandidateState, MetadataConfidence, ReadError,
    ReadOutcome, RecoverabilityInputs, SectorLayout, SourceIdentity, SourceReader,
};
use um_fixture_builder::deterministic_bytes;
use um_fixture_builder::fat::{FatFileOptions, FatImageBuilder, FatKind, NodeParent};
use um_fs_fat::FatVariant;
use um_io_common::MemImageReader;

struct DeclaredLengthReader {
    sector0: MemImageReader,
    declared_len: u64,
}

impl SourceReader for DeclaredLengthReader {
    fn identity(&self) -> &SourceIdentity {
        self.sector0.identity()
    }

    fn len(&self) -> u64 {
        self.declared_len
    }

    fn sector_layout(&self) -> SectorLayout {
        self.sector0.sector_layout()
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        if offset == 0 && buffer.len() <= 512 {
            return self.sector0.read_exact_at(offset, buffer);
        }
        Err(ReadError::Io {
            offset,
            message: "synthetic unexpected read beyond boot sector".into(),
        })
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        if offset == 0 && buffer.len() <= 512 {
            return self.sector0.read_best_effort_at(offset, buffer);
        }
        buffer.fill(0);
        ReadOutcome {
            bytes_valid: 0,
            bad_ranges: vec![(0, buffer.len() as u64)],
        }
    }
}

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
fn fat_completeness_001_marks_broken_directory_chain_partial() {
    let mut builder = FatImageBuilder::new("fat32-broken-directory", FatKind::Fat32);
    builder.add_dir(NodeParent::Root, "ARCHIVE", false);
    let (mut image, _) = builder.build();
    let boot = um_fs_fat::FatBoot::parse(&image[..512], image.len() as u64).unwrap();
    let root_offset = boot.cluster_offset(boot.root_cluster as u64).unwrap() as usize;
    let root_end = root_offset + boot.cluster_size as usize;
    let directory_entry = image[root_offset..root_end]
        .chunks_exact(32)
        .find(|entry| {
            entry[0] != 0
                && entry[0] != 0xE5
                && entry[0] != b'.'
                && entry[11] & 0x10 != 0
                && entry[11] != 0x0F
        })
        .expect("active directory entry");
    let first_cluster = u32::from(u16::from_le_bytes([
        directory_entry[20],
        directory_entry[21],
    ])) << 16
        | u32::from(u16::from_le_bytes([
            directory_entry[26],
            directory_entry[27],
        ]));
    let fat_entry_offset = boot.fat_offset as usize + first_cluster as usize * 4;
    image[fat_entry_offset..fat_entry_offset + 4].copy_from_slice(&0x0FFF_FFF7u32.to_le_bytes());

    let output =
        um_fs_fat::scan_fat(&MemImageReader::new("fat32-broken-directory", image)).unwrap();

    assert!(!output.is_complete);
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("broken cluster chain")));
}

#[test]
fn fat_completeness_002_marks_divergent_fat_copies_partial() {
    let builder = FatImageBuilder::new("fat32-divergent-copies", FatKind::Fat32);
    let (mut image, _) = builder.build();
    let boot = um_fs_fat::FatBoot::parse(&image[..512], image.len() as u64).unwrap();
    assert!(boot.num_fats > 1);
    let fat_bytes = boot.fat_size_sectors as usize * boot.bytes_per_sector as usize;
    let second_fat_offset = boot.fat_offset as usize + fat_bytes;
    image[second_fat_offset + 8] ^= 0x01;

    let output =
        um_fs_fat::scan_fat(&MemImageReader::new("fat32-divergent-copies", image)).unwrap();

    assert!(!output.is_complete);
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("FAT copies disagree")));
}

#[test]
fn fat_completeness_003_marks_directory_without_start_cluster_partial() {
    let mut builder = FatImageBuilder::new("fat32-directory-no-cluster", FatKind::Fat32);
    builder.add_dir(NodeParent::Root, "ORPHAN", false);
    let (mut image, _) = builder.build();
    let boot = um_fs_fat::FatBoot::parse(&image[..512], image.len() as u64).unwrap();
    let root_offset = boot.cluster_offset(boot.root_cluster as u64).unwrap() as usize;
    let root_end = root_offset + boot.cluster_size as usize;
    let directory_index = image[root_offset..root_end]
        .chunks_exact(32)
        .position(|entry| {
            entry[0] != 0
                && entry[0] != 0xE5
                && entry[0] != b'.'
                && entry[11] & 0x10 != 0
                && entry[11] != 0x0F
        })
        .expect("active directory entry");
    let directory_offset = root_offset + directory_index * 32;
    image[directory_offset + 20..directory_offset + 22].fill(0);
    image[directory_offset + 26..directory_offset + 28].fill(0);

    let output =
        um_fs_fat::scan_fat(&MemImageReader::new("fat32-directory-no-cluster", image)).unwrap();

    assert!(!output.is_complete);
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("no usable start cluster")));
}

#[test]
fn fat_completeness_004_checks_every_declared_fat_copy() {
    let builder = FatImageBuilder::new("fat32-three-copies", FatKind::Fat32);
    let (mut image, _) = builder.build();
    let original_boot = um_fs_fat::FatBoot::parse(&image[..512], image.len() as u64).unwrap();
    assert_eq!(original_boot.num_fats, 2);
    let fat_bytes =
        original_boot.fat_size_sectors as usize * original_boot.bytes_per_sector as usize;
    let first_fat_offset = original_boot.fat_offset as usize;
    let third_fat = image[first_fat_offset..first_fat_offset + fat_bytes].to_vec();
    let data_offset = first_fat_offset + 2 * fat_bytes;
    image.splice(data_offset..data_offset, third_fat);
    image[16] = 3;
    let total_sectors =
        u32::from_le_bytes(image[32..36].try_into().unwrap()) + original_boot.fat_size_sectors;
    image[32..36].copy_from_slice(&total_sectors.to_le_bytes());
    let third_fat_offset = first_fat_offset + 2 * fat_bytes;
    image[third_fat_offset + 8] ^= 0x01;

    let output = um_fs_fat::scan_fat(&MemImageReader::new("fat32-three-copies", image)).unwrap();

    assert!(!output.is_complete);
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("FAT copies disagree")));
}

#[test]
fn fat_table_bound_001_rejects_oversized_table_before_allocation() {
    let builder = FatImageBuilder::new("fat32-hostile-table-size", FatKind::Fat32);
    let (image, _) = builder.build();
    let mut sector0 = image[..512].to_vec();
    let fat_sectors = (64 * 1024 * 1024 / 512 + 1) as u32;
    let total_sectors = 1_000_000u32;
    sector0[36..40].copy_from_slice(&fat_sectors.to_le_bytes());
    sector0[32..36].copy_from_slice(&total_sectors.to_le_bytes());
    let reader = DeclaredLengthReader {
        sector0: MemImageReader::new("fat32-hostile-table-size", sector0),
        declared_len: u64::from(total_sectors) * 512,
    };

    let error =
        um_fs_fat::scan_fat(&reader).expect_err("oversized FAT must fail before allocation");

    assert!(error.to_string().contains("FAT table exceeds"));
}

#[test]
fn fat_depth_bound_001_marks_excessive_directory_nesting_partial() {
    let mut builder = FatImageBuilder::new("fat32-deep-directory", FatKind::Fat32);
    let mut parent = NodeParent::Root;
    for depth in 0..260 {
        let directory = builder.add_dir(parent, &format!("D{depth:03}"), false);
        parent = NodeParent::Node(directory);
    }
    let (image, _) = builder.build();

    let output = um_fs_fat::scan_fat(&MemImageReader::new("fat32-deep-directory", image)).unwrap();

    assert!(!output.is_complete);
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("depth bound")));
}

#[test]
fn fat_directory_chain_bound_001_caps_chain_before_directory_read() {
    let mut builder = FatImageBuilder::new("fat32-long-directory-chain", FatKind::Fat32);
    builder.add_dir(NodeParent::Root, "LONGDIR", false);
    let (mut image, _) = builder.build();
    let boot = um_fs_fat::FatBoot::parse(&image[..512], image.len() as u64).unwrap();
    let root_offset = boot.cluster_offset(boot.root_cluster as u64).unwrap() as usize;
    let root_end = root_offset + boot.cluster_size as usize;
    let directory_entry = image[root_offset..root_end]
        .chunks_exact(32)
        .find(|entry| {
            entry[0] != 0
                && entry[0] != 0xE5
                && entry[0] != b'.'
                && entry[11] & 0x10 != 0
                && entry[11] != 0x0F
        })
        .expect("active directory entry");
    let first_cluster = u32::from(u16::from_le_bytes([
        directory_entry[20],
        directory_entry[21],
    ])) << 16
        | u32::from(u16::from_le_bytes([
            directory_entry[26],
            directory_entry[27],
        ]));
    let chain_clusters = (8 * 1024 * 1024 / boot.cluster_size) as u32 + 1;
    let fat_bytes = boot.fat_size_sectors as usize * boot.bytes_per_sector as usize;
    for copy_index in 0..boot.num_fats as usize {
        let copy_offset = boot.fat_offset as usize + copy_index * fat_bytes;
        for index in 0..chain_clusters {
            let cluster = first_cluster + index;
            let entry_offset = copy_offset + cluster as usize * 4;
            image[entry_offset..entry_offset + 4].copy_from_slice(&(cluster + 1).to_le_bytes());
        }
        let last_cluster = first_cluster + chain_clusters;
        let last_offset = copy_offset + last_cluster as usize * 4;
        image[last_offset..last_offset + 4].copy_from_slice(&0x0FFF_FFFFu32.to_le_bytes());
    }

    let output =
        um_fs_fat::scan_fat(&MemImageReader::new("fat32-long-directory-chain", image)).unwrap();

    assert!(!output.is_complete);
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("chain reached the scan bound")));
}

#[test]
fn scan_survives_garbage() {
    let junk = deterministic_bytes(555, 1 << 20);
    let reader = MemImageReader::new("junk", junk);
    let _ = um_fs_fat::scan_fat(&reader);
}
