//! MBR/GPT discovery tests over synthetic in-memory disk images.

use um_core::{ReadError, ReadOutcome, SectorLayout, SourceIdentity, SourceReader};
use um_io_common::MemImageReader;
use um_partition::{discover, PartitionTableKind};

const SECTOR: usize = 512;

struct LayoutReader {
    inner: MemImageReader,
    layout: SectorLayout,
}

struct PrimaryHeaderFaultReader {
    inner: MemImageReader,
}

impl SourceReader for PrimaryHeaderFaultReader {
    fn identity(&self) -> &SourceIdentity {
        self.inner.identity()
    }

    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn sector_layout(&self) -> SectorLayout {
        self.inner.sector_layout()
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        if offset == SECTOR as u64 && buffer.len() == SECTOR {
            return Err(ReadError::Io {
                offset,
                message: "synthetic primary-header read fault".into(),
            });
        }
        self.inner.read_exact_at(offset, buffer)
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        if offset == SECTOR as u64 && buffer.len() == SECTOR {
            buffer.fill(0);
            return ReadOutcome {
                bytes_valid: 0,
                bad_ranges: vec![(0, SECTOR as u64)],
            };
        }
        self.inner.read_best_effort_at(offset, buffer)
    }
}

impl SourceReader for LayoutReader {
    fn identity(&self) -> &SourceIdentity {
        self.inner.identity()
    }

    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn sector_layout(&self) -> SectorLayout {
        self.layout
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        self.inner.read_exact_at(offset, buffer)
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        self.inner.read_best_effort_at(offset, buffer)
    }
}

fn mbr_entry(buf: &mut [u8], slot: usize, part_type: u8, lba: u32, sectors: u32) {
    let base = 446 + slot * 16;
    buf[base + 4] = part_type;
    buf[base + 8..base + 12].copy_from_slice(&lba.to_le_bytes());
    buf[base + 12..base + 16].copy_from_slice(&sectors.to_le_bytes());
}

fn sign(buf: &mut [u8]) {
    buf[510] = 0x55;
    buf[511] = 0xAA;
}

#[test]
fn parses_simple_mbr() {
    let total_sectors = 8192;
    let mut image = vec![0u8; total_sectors * SECTOR];
    {
        let s0 = &mut image[..SECTOR];
        mbr_entry(s0, 0, 0x0C, 2048, 4096); // FAT32
        mbr_entry(s0, 1, 0x07, 6144, 1024); // NTFS
        sign(s0);
    }
    let r = MemImageReader::new("mbr", image);
    let t = discover(&r).unwrap();
    assert_eq!(t.kind, PartitionTableKind::Mbr);
    assert_eq!(t.partitions.len(), 2);
    assert_eq!(t.partitions[0].region.offset, 2048 * SECTOR as u64);
    assert_eq!(t.partitions[0].region.len, 4096 * SECTOR as u64);
    assert_eq!(t.partitions[1].type_description, "NTFS/exFAT (IFS)");
}

#[test]
fn ignores_partition_beyond_source() {
    let mut image = vec![0u8; 4096 * SECTOR];
    {
        let s0 = &mut image[..SECTOR];
        mbr_entry(s0, 0, 0x07, 2048, 1024);
        mbr_entry(s0, 1, 0x07, 100_000, 4096); // out of bounds
        sign(s0);
    }
    let r = MemImageReader::new("mbr-oob", image);
    let t = discover(&r).unwrap();
    assert_eq!(t.partitions.len(), 1);
    assert!(!t.warnings.is_empty());
}

#[test]
fn follows_ebr_chain_with_loop_protection() {
    let total = 20_000usize;
    let mut image = vec![0u8; total * SECTOR];
    {
        let s0 = &mut image[..SECTOR];
        mbr_entry(s0, 0, 0x05, 4096, 12_000); // extended
        sign(s0);
    }
    // EBR 1 at 4096: logical partition +64, next EBR at +6000 (self-loop back to 0 offset).
    {
        let e = &mut image[4096 * SECTOR..4096 * SECTOR + SECTOR];
        mbr_entry(e, 0, 0x07, 64, 1000);
        mbr_entry(e, 1, 0x05, 0, 6000); // next EBR = 4096 + 0 => loop!
        sign(e);
    }
    let r = MemImageReader::new("ebr-loop", image);
    let t = discover(&r).unwrap();
    assert_eq!(t.partitions.len(), 1);
    assert_eq!(t.partitions[0].region.offset, (4096 + 64) * SECTOR as u64);
    assert!(t.warnings.iter().any(|w| w.contains("loop")));
}

/// Builds a minimal valid GPT image with the given partitions `(first, last, name)`.
fn build_gpt_image(total_sectors: u64, parts: &[(u64, u64, &str)]) -> Vec<u8> {
    let mut image = vec![0u8; (total_sectors * SECTOR as u64) as usize];
    // Protective MBR.
    {
        let s0 = &mut image[..SECTOR];
        mbr_entry(s0, 0, 0xEE, 1, (total_sectors - 1) as u32);
        sign(s0);
    }
    // Entry table at LBA 2 (128 entries x 128 bytes = 16 KiB = 32 sectors).
    let mut table = vec![0u8; 128 * 128];
    let basic_data: [u8; 16] = [
        0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44, 0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26, 0x99,
        0xC7,
    ];
    for (i, (first, last, name)) in parts.iter().enumerate() {
        let e = &mut table[i * 128..(i + 1) * 128];
        e[0..16].copy_from_slice(&basic_data);
        e[16..32].copy_from_slice(&[i as u8 + 1; 16]); // unique guid
        e[32..40].copy_from_slice(&first.to_le_bytes());
        e[40..48].copy_from_slice(&last.to_le_bytes());
        for (j, u) in name.encode_utf16().enumerate().take(36) {
            e[56 + j * 2..58 + j * 2].copy_from_slice(&u.to_le_bytes());
        }
    }
    let entries_crc = crc32fast::hash(&table);
    // Header at LBA 1.
    let mut header = vec![0u8; 92];
    header[0..8].copy_from_slice(b"EFI PART");
    header[8..12].copy_from_slice(&0x00010000u32.to_le_bytes()); // revision
    header[12..16].copy_from_slice(&92u32.to_le_bytes());
    header[24..32].copy_from_slice(&1u64.to_le_bytes()); // current LBA
    header[32..40].copy_from_slice(&(total_sectors - 1).to_le_bytes()); // backup LBA
    header[40..48].copy_from_slice(&34u64.to_le_bytes()); // first usable
    header[48..56].copy_from_slice(&(total_sectors - 34).to_le_bytes()); // last usable
    header[72..80].copy_from_slice(&2u64.to_le_bytes()); // entry table LBA
    header[80..84].copy_from_slice(&128u32.to_le_bytes()); // num entries
    header[84..88].copy_from_slice(&128u32.to_le_bytes()); // entry size
    header[88..92].copy_from_slice(&entries_crc.to_le_bytes());
    let crc = crc32fast::hash(&header);
    header[16..20].copy_from_slice(&crc.to_le_bytes());

    image[SECTOR..SECTOR + 92].copy_from_slice(&header);
    image[2 * SECTOR..2 * SECTOR + table.len()].copy_from_slice(&table);
    image
}

fn update_primary_gpt_header_crc(image: &mut [u8]) {
    update_gpt_header_crc_at(image, 1);
}

fn update_gpt_header_crc_at(image: &mut [u8], header_lba: u64) {
    let offset = header_lba as usize * SECTOR;
    let header = &mut image[offset..offset + 92];
    header[16..20].fill(0);
    let crc = crc32fast::hash(header);
    header[16..20].copy_from_slice(&crc.to_le_bytes());
}

fn install_backup_gpt_at(image: &mut [u8], header_lba: u64) {
    let table_len: usize = 128 * 128;
    let table_sectors = table_len.div_ceil(SECTOR) as u64;
    let backup_table_lba = header_lba - table_sectors;
    let primary_table = image[2 * SECTOR..2 * SECTOR + table_len].to_vec();
    let backup_table_offset = backup_table_lba as usize * SECTOR;
    image[backup_table_offset..backup_table_offset + table_len].copy_from_slice(&primary_table);

    let mut header = image[SECTOR..SECTOR + 92].to_vec();
    header[16..20].fill(0);
    header[24..32].copy_from_slice(&header_lba.to_le_bytes());
    header[32..40].copy_from_slice(&1u64.to_le_bytes());
    header[72..80].copy_from_slice(&backup_table_lba.to_le_bytes());
    let crc = crc32fast::hash(&header);
    header[16..20].copy_from_slice(&crc.to_le_bytes());

    let backup_header_offset = header_lba as usize * SECTOR;
    image[backup_header_offset..backup_header_offset + header.len()].copy_from_slice(&header);
}

fn install_backup_gpt(image: &mut [u8], total_sectors: u64) {
    install_backup_gpt_at(image, total_sectors - 1);
}

#[test]
fn part_gpt_overflow_001_rejects_entry_table_end_overflow() {
    let mut image = build_gpt_image(20_480, &[(2048, 10_239, "Dados")]);
    let overflowing_lba = u64::MAX / SECTOR as u64;
    image[SECTOR + 72..SECTOR + 80].copy_from_slice(&overflowing_lba.to_le_bytes());
    update_primary_gpt_header_crc(&mut image);

    let reader = MemImageReader::new("gpt-table-overflow", image);
    assert!(discover(&reader).is_err());
}

#[test]
fn part_gpt_range_001_rejects_partition_length_overflow() {
    let mut image = build_gpt_image(20_480, &[(2048, 10_239, "Dados")]);
    image[2 * SECTOR + 32..2 * SECTOR + 40].copy_from_slice(&0u64.to_le_bytes());
    image[2 * SECTOR + 40..2 * SECTOR + 48].copy_from_slice(&u64::MAX.to_le_bytes());

    let table_crc = crc32fast::hash(&image[2 * SECTOR..2 * SECTOR + 128 * 128]);
    image[SECTOR + 40..SECTOR + 48].copy_from_slice(&0u64.to_le_bytes());
    image[SECTOR + 48..SECTOR + 56].copy_from_slice(&u64::MAX.to_le_bytes());
    image[SECTOR + 88..SECTOR + 92].copy_from_slice(&table_crc.to_le_bytes());
    update_primary_gpt_header_crc(&mut image);

    let reader = MemImageReader::new("gpt-range-overflow", image);
    assert!(discover(&reader).is_err());
}

#[test]
fn part_sector_zero_001_rejects_invalid_logical_sector() {
    let zero_logical = LayoutReader {
        inner: MemImageReader::new("zero-sector", vec![0u8; 2 * SECTOR]),
        layout: SectorLayout {
            logical: 0,
            physical: 512,
        },
    };
    assert!(discover(&zero_logical).is_err());

    let physical_smaller_than_logical = LayoutReader {
        inner: MemImageReader::new("invalid-sector-order", vec![0u8; 16 * SECTOR]),
        layout: SectorLayout {
            logical: 4096,
            physical: 512,
        },
    };
    assert!(discover(&physical_smaller_than_logical).is_err());
}

#[test]
fn parses_gpt_with_names_and_guids() {
    let image = build_gpt_image(
        20_480,
        &[(2048, 10_239, "Dados"), (10_240, 18_431, "Backup")],
    );
    let r = MemImageReader::new("gpt", image);
    let t = discover(&r).unwrap();
    assert_eq!(t.kind, PartitionTableKind::Gpt);
    assert_eq!(t.partitions.len(), 2);
    assert_eq!(t.partitions[0].name.as_deref(), Some("Dados"));
    assert_eq!(t.partitions[0].type_description, "Microsoft basic data");
    assert_eq!(t.partitions[0].region.offset, 2048 * SECTOR as u64);
    assert_eq!(
        t.partitions[1].region.len,
        (18_431 - 10_240 + 1) * SECTOR as u64
    );
}

#[test]
fn rejects_gpt_with_corrupt_entry_crc() {
    let mut image = build_gpt_image(20_480, &[(2048, 10_239, "Dados")]);
    // Corrupt one byte of the entry table (CRC now mismatches).
    image[2 * SECTOR + 40] ^= 0xFF;
    let r = MemImageReader::new("gpt-bad", image);
    assert!(discover(&r).is_err());
}

#[test]
fn part_gpt_backup_001_uses_backup_when_primary_entry_table_is_corrupt() {
    let total_sectors = 20_480;
    let mut image = build_gpt_image(total_sectors, &[(2048, 10_239, "Dados")]);
    install_backup_gpt(&mut image, total_sectors);
    image[2 * SECTOR + 40] ^= 0xFF;

    let r = MemImageReader::new("gpt-primary-table-corrupt", image);
    let t = discover(&r).unwrap();

    assert_eq!(t.kind, PartitionTableKind::Gpt);
    assert_eq!(t.partitions.len(), 1);
    assert_eq!(t.partitions[0].name.as_deref(), Some("Dados"));
    assert!(t.warnings.iter().any(|warning| warning.contains("backup")));
}

#[test]
fn part_gpt_backup_002_rejects_primary_redirect_to_interior_fake_backup() {
    let total_sectors = 20_480;
    let mut image = build_gpt_image(total_sectors, &[(2048, 10_239, "Dados")]);
    let interior_backup_lba = 1024;
    install_backup_gpt_at(&mut image, interior_backup_lba);
    image[SECTOR + 32..SECTOR + 40].copy_from_slice(&interior_backup_lba.to_le_bytes());
    update_primary_gpt_header_crc(&mut image);
    image[2 * SECTOR + 40] ^= 0xFF;

    let reader = MemImageReader::new("gpt-interior-backup-redirect", image);
    assert!(discover(&reader).is_err());
}

#[test]
fn part_gpt_backup_003_rejects_nonreciprocal_backup_header() {
    let total_sectors = 20_480;
    let mut image = build_gpt_image(total_sectors, &[(2048, 10_239, "Dados")]);
    install_backup_gpt(&mut image, total_sectors);
    image[2 * SECTOR + 40] ^= 0xFF;
    let backup_header_lba = total_sectors - 1;
    let backup_offset = backup_header_lba as usize * SECTOR;
    image[backup_offset + 32..backup_offset + 40].copy_from_slice(&2u64.to_le_bytes());
    update_gpt_header_crc_at(&mut image, backup_header_lba);

    let reader = MemImageReader::new("gpt-nonreciprocal-backup", image);
    assert!(discover(&reader).is_err());
}

#[test]
fn part_gpt_backup_004_uses_backup_when_primary_header_is_unreadable() {
    let total_sectors = 20_480;
    let mut image = build_gpt_image(total_sectors, &[(2048, 10_239, "Dados")]);
    install_backup_gpt(&mut image, total_sectors);
    let reader = PrimaryHeaderFaultReader {
        inner: MemImageReader::new("gpt-primary-read-fault", image),
    };

    let table = discover(&reader).expect("canonical backup should survive primary read fault");

    assert_eq!(table.kind, PartitionTableKind::Gpt);
    assert_eq!(table.partitions.len(), 1);
    assert!(table
        .warnings
        .iter()
        .any(|warning| warning.contains("backup")));
}

#[test]
fn part_gpt_backup_005_rejects_conflicting_valid_backup_when_primary_is_usable() {
    let total_sectors = 20_480;
    let mut image = build_gpt_image(total_sectors, &[(2048, 10_239, "Dados")]);
    install_backup_gpt(&mut image, total_sectors);
    let backup_header_lba = total_sectors - 1;
    let backup_offset = backup_header_lba as usize * SECTOR;
    image[backup_offset + 56] ^= 0x01;
    update_gpt_header_crc_at(&mut image, backup_header_lba);

    let reader = MemImageReader::new("gpt-conflicting-valid-backup", image);
    let error = discover(&reader).expect_err("conflicting valid headers must fail closed");

    assert!(error.to_string().contains("not reciprocal or consistent"));
}

#[test]
fn part_gpt_protective_001_rejects_protective_mbr_when_both_gpt_copies_are_unusable() {
    let total_sectors = 20_480;
    let mut image = build_gpt_image(total_sectors, &[(2048, 10_239, "Dados")]);
    install_backup_gpt(&mut image, total_sectors);
    image[SECTOR] ^= 0xFF;
    image[(total_sectors as usize - 1) * SECTOR] ^= 0xFF;

    let r = MemImageReader::new("gpt-both-copies-unusable", image);
    let error = discover(&r).unwrap_err();

    assert!(error.to_string().contains("protective MBR"));
}

#[test]
fn unpartitioned_source_is_not_recognized() {
    let image = vec![0u8; 1024 * SECTOR];
    let r = MemImageReader::new("blank", image);
    assert!(discover(&r).is_err());
}

#[test]
fn source_smaller_than_two_sectors_is_rejected() {
    let r = MemImageReader::new("tiny", vec![0u8; 100]);
    assert!(discover(&r).is_err());
    assert_eq!(r.len(), 100);
}
