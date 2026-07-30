//! End-to-end bounds tests for NTFS internal metadata streams.
//!
//! The images are deterministic in-memory fixtures. The tests mutate only the
//! synthetic `$DATA.initialized_size` fields and never access a real disk.

use std::sync::Mutex;

use um_core::{
    ExtentAvailability, ReadError, ReadOutcome, SectorLayout, SourceIdentity, SourceReader,
};
use um_fixture_builder::deterministic_bytes;
use um_fixture_builder::ntfs::{FileOptions, NodeParent, NtfsImageBuilder};
use um_io_common::MemImageReader;

const MFT_OFFSET: usize = 4 * 4096;
const RECORD_SIZE: usize = 1024;
const MFT_RECORDS: usize = 64;
const MFT_BATCH_BYTES: usize = 1024 * 1024;

struct CountingReader {
    inner: MemImageReader,
    reads: Mutex<Vec<(u64, usize)>>,
}

impl CountingReader {
    fn new(label: &str, image: Vec<u8>) -> Self {
        Self {
            inner: MemImageReader::new(label, image),
            reads: Mutex::new(Vec::new()),
        }
    }

    fn read_offsets(&self) -> Vec<(u64, usize)> {
        self.reads.lock().unwrap().clone()
    }
}

impl SourceReader for CountingReader {
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
        self.reads.lock().unwrap().push((offset, buffer.len()));
        self.inner.read_exact_at(offset, buffer)
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        self.reads.lock().unwrap().push((offset, buffer.len()));
        self.inner.read_best_effort_at(offset, buffer)
    }
}

fn unnamed_nonresident_data_attribute(image: &[u8], record_no: usize) -> (usize, usize) {
    let record_start = MFT_OFFSET + record_no * RECORD_SIZE;
    let attrs_offset = u16::from_le_bytes(
        image[record_start + 20..record_start + 22]
            .try_into()
            .unwrap(),
    ) as usize;
    let mut attribute = record_start + attrs_offset;
    let record_end = record_start + RECORD_SIZE;

    while attribute + 8 <= record_end {
        let attribute_type =
            u32::from_le_bytes(image[attribute..attribute + 4].try_into().unwrap());
        if attribute_type == u32::MAX {
            break;
        }
        let attribute_len =
            u32::from_le_bytes(image[attribute + 4..attribute + 8].try_into().unwrap()) as usize;
        assert!(attribute_len >= 8 && attribute + attribute_len <= record_end);
        if attribute_type == 0x80 && image[attribute + 8] == 1 && image[attribute + 9] == 0 {
            return (attribute, attribute_len);
        }
        attribute += attribute_len;
    }

    panic!("record {record_no} has no unnamed non-resident $DATA");
}

fn set_unnamed_data_initialized_size(image: &mut [u8], record_no: usize, initialized: u64) {
    let (attribute, _) = unnamed_nonresident_data_attribute(image, record_no);
    image[attribute + 56..attribute + 64].copy_from_slice(&initialized.to_le_bytes());
}

fn set_unnamed_data_size(image: &mut [u8], record_no: usize, data_size: u64) {
    let (attribute, _) = unnamed_nonresident_data_attribute(image, record_no);
    image[attribute + 48..attribute + 56].copy_from_slice(&data_size.to_le_bytes());
}

fn set_record_flags(image: &mut [u8], record_no: usize, flags: u16) {
    let record_start = MFT_OFFSET + record_no * RECORD_SIZE;
    image[record_start + 22..record_start + 24].copy_from_slice(&flags.to_le_bytes());
}

fn set_record_base_reference(image: &mut [u8], record_no: usize, base_record: u64) {
    let record_start = MFT_OFFSET + record_no * RECORD_SIZE;
    image[record_start + 32..record_start + 40].copy_from_slice(&base_record.to_le_bytes());
}

fn set_unnamed_data_attribute_flags(image: &mut [u8], record_no: usize, flags: u16) {
    let (attribute, _) = unnamed_nonresident_data_attribute(image, record_no);
    image[attribute + 12..attribute + 14].copy_from_slice(&flags.to_le_bytes());
}

fn set_unnamed_data_starting_vcn(image: &mut [u8], record_no: usize, starting_vcn: u64) {
    let (attribute, _) = unnamed_nonresident_data_attribute(image, record_no);
    image[attribute + 16..attribute + 24].copy_from_slice(&starting_vcn.to_le_bytes());
}

fn set_unnamed_data_runlist(
    image: &mut [u8],
    record_no: usize,
    runlist: &[u8],
    total_clusters: u64,
    data_size: u64,
    initialized_size: u64,
) {
    let (attribute, attribute_len) = unnamed_nonresident_data_attribute(image, record_no);
    let runlist_offset =
        u16::from_le_bytes(image[attribute + 32..attribute + 34].try_into().unwrap()) as usize;
    assert!(runlist_offset <= attribute_len);
    assert!(runlist.len() <= attribute_len - runlist_offset);

    image[attribute + 24..attribute + 32]
        .copy_from_slice(&total_clusters.saturating_sub(1).to_le_bytes());
    image[attribute + 48..attribute + 56].copy_from_slice(&data_size.to_le_bytes());
    image[attribute + 56..attribute + 64].copy_from_slice(&initialized_size.to_le_bytes());
    image[attribute + runlist_offset..attribute + attribute_len].fill(0);
    image[attribute + runlist_offset..attribute + runlist_offset + runlist.len()]
        .copy_from_slice(runlist);
}

fn corrupt_mft_record_fixups(image: &mut [u8]) {
    let template = image[MFT_OFFSET + RECORD_SIZE..MFT_OFFSET + 2 * RECORD_SIZE].to_vec();
    for record_no in 1..MFT_RECORDS {
        let start = MFT_OFFSET + record_no * RECORD_SIZE;
        image[start..start + RECORD_SIZE].copy_from_slice(&template);
        image[start + 510] ^= 0xFF;
    }
}

fn corrupt_first_attribute_length(image: &mut [u8], record_no: usize) {
    let record_start = MFT_OFFSET + record_no * RECORD_SIZE;
    let attrs_offset = u16::from_le_bytes(
        image[record_start + 20..record_start + 22]
            .try_into()
            .unwrap(),
    ) as usize;
    let attribute = record_start + attrs_offset;
    image[attribute + 4..attribute + 8].copy_from_slice(&u32::MAX.to_le_bytes());
}

fn replace_first_attribute_with_valid_list(image: &mut [u8], record_no: usize) {
    let record_start = MFT_OFFSET + record_no * RECORD_SIZE;
    let attrs_offset = u16::from_le_bytes(
        image[record_start + 20..record_start + 22]
            .try_into()
            .unwrap(),
    ) as usize;
    let attribute = record_start + attrs_offset;
    let value_len =
        u32::from_le_bytes(image[attribute + 16..attribute + 20].try_into().unwrap()) as usize;
    let value_offset =
        u16::from_le_bytes(image[attribute + 20..attribute + 22].try_into().unwrap()) as usize;
    assert!(value_len >= 26);
    image[attribute..attribute + 4].copy_from_slice(&0x20u32.to_le_bytes());
    let value = attribute + value_offset;
    image[value..value + value_len].fill(0);
    image[value..value + 4].copy_from_slice(&0x80u32.to_le_bytes());
    image[value + 4..value + 6].copy_from_slice(&(value_len as u16).to_le_bytes());
    image[value + 16..value + 24].copy_from_slice(&17u64.to_le_bytes());
}

#[test]
fn internal_bitmap_initialized_prefix_leaves_stale_bits_unknown() {
    let mut builder = NtfsImageBuilder::new("ntfs-bitmap-initialized-prefix");
    builder.add_file(
        NodeParent::Root,
        "bitmap-stale.bin",
        deterministic_bytes(0xB17, 4096),
        true,
        FileOptions {
            force_resident: Some(false),
            ..Default::default()
        },
    );
    let (mut image, _) = builder.build();
    set_unnamed_data_initialized_size(&mut image, 6, 1);

    let output =
        um_fs_ntfs::scan_ntfs(&MemImageReader::new("bitmap-initialized-prefix", image)).unwrap();
    let candidate = output
        .candidates
        .iter()
        .find(|candidate| candidate.name == "bitmap-stale.bin")
        .expect("deleted fixture candidate");

    assert!(candidate
        .extents
        .iter()
        .all(|extent| extent.availability == ExtentAvailability::Unknown));
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("$Bitmap data truncated")));
}

#[test]
fn ntfs_mft_size_001_rejects_zero_or_unaligned_logical_size() {
    for (label, data_size) in [
        ("ntfs-zero-mft-size", 0),
        ("ntfs-unaligned-mft-size", RECORD_SIZE as u64 + 1),
        ("ntfs-reserved-only-mft-size", 15 * RECORD_SIZE as u64),
    ] {
        let builder = NtfsImageBuilder::new(label);
        let (mut image, _) = builder.build();
        set_unnamed_data_size(&mut image, 0, data_size);
        set_unnamed_data_initialized_size(&mut image, 0, data_size);

        let error = um_fs_ntfs::scan_ntfs(&MemImageReader::new(label, image))
            .expect_err("invalid $MFT logical size must be rejected");

        assert!(error.to_string().contains("$MFT data size"));
    }
}

#[test]
fn ntfs_record_signature_001_marks_nonblank_non_file_record_partial() {
    let mut builder = NtfsImageBuilder::new("ntfs-non-file-record");
    builder.add_file(
        NodeParent::Root,
        "hidden-deleted.txt",
        deterministic_bytes(0xBAD, 256),
        true,
        FileOptions::default(),
    );
    let (mut image, _) = builder.build();
    image[MFT_OFFSET + 16 * RECORD_SIZE..MFT_OFFSET + 16 * RECORD_SIZE + 4]
        .copy_from_slice(b"BAAD");

    let output =
        um_fs_ntfs::scan_ntfs(&MemImageReader::new("ntfs-non-file-record", image)).unwrap();

    assert!(!output.is_complete);
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("has no FILE signature")));
    assert!(!output
        .candidates
        .iter()
        .any(|candidate| candidate.name == "hidden-deleted.txt"));
}

#[test]
fn ntfs_attribute_bounds_001_marks_malformed_candidate_attributes_partial() {
    let mut builder = NtfsImageBuilder::new("ntfs-malformed-candidate-attributes");
    builder.add_file(
        NodeParent::Root,
        "hidden-by-attribute-corruption.txt",
        deterministic_bytes(0xA77, 256),
        true,
        FileOptions::default(),
    );
    let (mut image, _) = builder.build();
    corrupt_first_attribute_length(&mut image, 16);

    let output = um_fs_ntfs::scan_ntfs(&MemImageReader::new(
        "ntfs-malformed-candidate-attributes",
        image,
    ))
    .unwrap();

    assert!(!output.is_complete);
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("incomplete or unresolved attributes")));
    assert!(!output
        .candidates
        .iter()
        .any(|candidate| candidate.name == "hidden-by-attribute-corruption.txt"));
}

#[test]
fn ntfs_attribute_list_partial_001_never_claims_full_reference_resolution() {
    let mut builder = NtfsImageBuilder::new("ntfs-resident-attribute-list");
    builder.add_file(
        NodeParent::Root,
        "attribute-list-candidate.txt",
        deterministic_bytes(0xA11, 256),
        true,
        FileOptions::default(),
    );
    let (mut image, _) = builder.build();
    replace_first_attribute_with_valid_list(&mut image, 16);

    let output =
        um_fs_ntfs::scan_ntfs(&MemImageReader::new("ntfs-resident-attribute-list", image)).unwrap();

    assert!(!output.is_complete);
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("incomplete or unresolved attributes")));
    assert!(output
        .candidates
        .iter()
        .any(|candidate| candidate.name == "attribute-list-candidate.txt"));
}

#[test]
fn internal_mft_initialized_prefix_excludes_stale_records() {
    let mut builder = NtfsImageBuilder::new("ntfs-mft-initialized-prefix");
    builder.add_file(
        NodeParent::Root,
        "mft-stale.bin",
        deterministic_bytes(0x004D_4654, 4096),
        true,
        FileOptions {
            force_resident: Some(false),
            ..Default::default()
        },
    );
    let (mut image, _) = builder.build();
    set_unnamed_data_initialized_size(&mut image, 0, 16 * 1024);

    let output =
        um_fs_ntfs::scan_ntfs(&MemImageReader::new("mft-initialized-prefix", image)).unwrap();

    assert!(output
        .candidates
        .iter()
        .all(|candidate| candidate.name != "mft-stale.bin"));
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("MFT scan bounded to 16384 initialized bytes")));
    assert!(!output.is_complete);
}

#[test]
fn bitmap_record_outside_trusted_mft_prefix_is_never_read() {
    let builder = NtfsImageBuilder::new("ntfs-bitmap-record-prefix");
    let (mut image, _) = builder.build();
    set_unnamed_data_initialized_size(&mut image, 0, 6 * RECORD_SIZE as u64);
    let reader = CountingReader::new("bitmap-record-prefix", image);

    let output = um_fs_ntfs::scan_ntfs(&reader).unwrap();
    let bitmap_record_offset = (MFT_OFFSET + 6 * RECORD_SIZE) as u64;

    assert!(!reader
        .read_offsets()
        .iter()
        .any(|&(offset, len)| offset == bitmap_record_offset && len == RECORD_SIZE));
    assert_eq!(
        output
            .warnings
            .iter()
            .filter(|warning| warning.contains("$Bitmap record lies beyond trusted MFT prefix"))
            .count(),
        1
    );
}

#[test]
fn sparse_bitmap_run_cannot_prove_clusters_free() {
    let mut builder = NtfsImageBuilder::new("ntfs-sparse-bitmap");
    builder.add_file(
        NodeParent::Root,
        "sparse-bitmap.bin",
        deterministic_bytes(0x5A, 4096),
        true,
        FileOptions {
            force_resident: Some(false),
            ..Default::default()
        },
    );
    let (mut image, _) = builder.build();
    set_unnamed_data_runlist(&mut image, 6, &[0x01, 0x01, 0x00], 1, 128, 128);

    let output = um_fs_ntfs::scan_ntfs(&MemImageReader::new("sparse-bitmap", image)).unwrap();
    let candidate = output
        .candidates
        .iter()
        .find(|candidate| candidate.name == "sparse-bitmap.bin")
        .expect("deleted fixture candidate");

    assert!(candidate
        .extents
        .iter()
        .all(|extent| extent.availability == ExtentAvailability::Unknown));
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("$Bitmap has no trusted physical data")));
}

#[test]
fn ntfs_bitmap_authority_002_rejects_untrusted_record_or_stream_semantics() {
    const BITMAP_RECORD: usize = 6;
    const ATTR_FLAG_COMPRESSED: u16 = 0x0001;
    const ATTR_FLAG_ENCRYPTED: u16 = 0x4000;
    const ATTR_FLAG_SPARSE: u16 = 0x8000;
    type BitmapMutation = (&'static str, fn(&mut [u8]));

    let mutations: [BitmapMutation; 7] = [
        ("inactive", |image| {
            set_record_flags(image, BITMAP_RECORD, 0)
        }),
        ("directory", |image| {
            set_record_flags(image, BITMAP_RECORD, 0x0003)
        }),
        ("extension", |image| {
            set_record_base_reference(image, BITMAP_RECORD, 1)
        }),
        ("compressed", |image| {
            set_unnamed_data_attribute_flags(image, BITMAP_RECORD, ATTR_FLAG_COMPRESSED)
        }),
        ("encrypted", |image| {
            set_unnamed_data_attribute_flags(image, BITMAP_RECORD, ATTR_FLAG_ENCRYPTED)
        }),
        ("sparse-flag", |image| {
            set_unnamed_data_attribute_flags(image, BITMAP_RECORD, ATTR_FLAG_SPARSE)
        }),
        ("nonzero-starting-vcn", |image| {
            set_unnamed_data_starting_vcn(image, BITMAP_RECORD, 1)
        }),
    ];

    for (case, mutate) in mutations {
        let mut builder = NtfsImageBuilder::new(&format!("ntfs-bitmap-authority-{case}"));
        builder.add_file(
            NodeParent::Root,
            "bitmap-authority.bin",
            deterministic_bytes(0xB17A, 4096),
            true,
            FileOptions {
                force_resident: Some(false),
                ..Default::default()
            },
        );
        let (mut image, _) = builder.build();
        mutate(&mut image);
        let reader_label = format!("bitmap-authority-{case}");

        let output = um_fs_ntfs::scan_ntfs(&MemImageReader::new(&reader_label, image))
            .expect("metadata scan must survive by dropping untrusted allocation evidence");
        let candidate = output
            .candidates
            .iter()
            .find(|candidate| candidate.name == "bitmap-authority.bin")
            .expect("deleted fixture candidate");

        assert!(
            output.allocation.is_none(),
            "{case} $Bitmap semantics must not become allocation authority"
        );
        assert!(
            candidate
                .extents
                .iter()
                .all(|extent| extent.availability == ExtentAvailability::Unknown),
            "{case} $Bitmap semantics must leave allocation unknown"
        );
        assert!(
            output
                .warnings
                .iter()
                .any(|warning| warning.contains("$Bitmap record or stream semantics")),
            "{case} must retain one sanitized authority warning: {:?}",
            output.warnings
        );
    }
}

#[test]
fn sparse_mft_tail_is_excluded_from_trusted_scan_prefix() {
    let builder = NtfsImageBuilder::new("ntfs-sparse-mft");
    let (mut image, _) = builder.build();
    let total_clusters = 32;
    let stream_len = total_clusters * 4096;
    set_unnamed_data_runlist(
        &mut image,
        0,
        &[0x11, 0x10, 0x04, 0x01, 0x10, 0x00],
        total_clusters,
        stream_len,
        stream_len,
    );

    let output = um_fs_ntfs::scan_ntfs(&MemImageReader::new("sparse-mft", image)).unwrap();

    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("MFT trusted prefix stopped before sparse run")));
    assert!(!output.is_complete);
}

#[test]
fn repeated_corrupt_mft_records_have_bounded_retained_warnings() {
    let builder = NtfsImageBuilder::new("ntfs-warning-budget");
    let (mut image, _) = builder.build();
    corrupt_mft_record_fixups(&mut image);

    let output = um_fs_ntfs::scan_ntfs(&MemImageReader::new("warning-budget", image)).unwrap();
    let detailed = output
        .warnings
        .iter()
        .filter(|warning| {
            warning.starts_with("MFT record ") && !warning.contains("warnings suppressed")
        })
        .count();

    assert!(detailed <= 16, "retained {detailed} detailed warnings");
    assert!(output
        .warnings
        .iter()
        .any(|warning| warning.contains("MFT record warnings suppressed: 47")));
    assert!(!output.is_complete);
}

#[test]
fn ntfs_mft_batch_001_large_mft_uses_bounded_bulk_reads() {
    const FIRST_RECORD_AFTER_64_MIB: u64 = (64 * 1024 * 1024) / RECORD_SIZE as u64;

    let mut builder = NtfsImageBuilder::new("ntfs-batched-large-mft")
        .with_mft_layout(FIRST_RECORD_AFTER_64_MIB + 4, FIRST_RECORD_AFTER_64_MIB);
    builder.add_file(
        NodeParent::Root,
        "batched-candidate.txt",
        b"bounded MFT batch reads".to_vec(),
        true,
        FileOptions::default(),
    );
    let (image, _) = builder.build();
    let reader = CountingReader::new("ntfs-batched-large-mft", image);

    let output = um_fs_ntfs::scan_ntfs(&reader).expect("large MFT scan");
    let reads = reader.read_offsets();

    assert!(output
        .candidates
        .iter()
        .any(|candidate| candidate.name == "batched-candidate.txt"));
    assert!(
        reads.iter().all(|(_, len)| *len <= MFT_BATCH_BYTES),
        "scanner issued an oversized read: {reads:?}"
    );
    assert!(
        reads.iter().any(|(_, len)| *len == MFT_BATCH_BYTES),
        "large MFT should be read in bulk rather than one record per request"
    );
}
