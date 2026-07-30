//! Deterministic raw-byte fixtures for bounded signature-carving tests.

use crate::sha256_hex;
use um_core::Region;

pub const JPEG_BOUNDARY_CHUNK_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub struct JpegCarvingFixture {
    pub image: Vec<u8>,
    pub scan_region: Region,
    pub valid_offset: u64,
    pub valid_jpeg: Vec<u8>,
    pub valid_sha256: String,
    pub false_positive_offsets: Vec<u64>,
    pub truncated_offset: u64,
}

/// Produces one structurally valid JPEG whose `FF D8` signature crosses the
/// first chunk boundary, followed by malformed and truncated lookalikes.
pub fn jpeg_carving_fixture() -> JpegCarvingFixture {
    let valid_jpeg = structurally_valid_jpeg();
    let valid_sha256 = sha256_hex(&valid_jpeg);
    let mut image: Vec<u8> = (0..JPEG_BOUNDARY_CHUNK_SIZE - 1)
        .map(|index| 0x20 + (index as u8 % 0x40))
        .collect();
    let valid_offset = image.len() as u64;
    image.extend_from_slice(&valid_jpeg);

    image.extend_from_slice(&[0x15; 13]);
    let invalid_segment_offset = image.len() as u64;
    image.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x01, 0xAA]);

    image.extend_from_slice(&[0x25; 11]);
    let zero_dimension_offset = image.len() as u64;
    image.extend_from_slice(&jpeg_with_zero_height());

    image.extend_from_slice(&[0x35; 9]);
    let invalid_entropy_offset = image.len() as u64;
    image.extend_from_slice(&jpeg_with_invalid_entropy_marker());

    image.extend_from_slice(&[0x45; 7]);
    let truncated_offset = image.len() as u64;
    let mut truncated = structurally_valid_jpeg();
    truncated.truncate(truncated.len() - 2);
    image.extend_from_slice(&truncated);

    let scan_region =
        Region::new(0, image.len() as u64).expect("fixture image length cannot overflow u64");
    JpegCarvingFixture {
        image,
        scan_region,
        valid_offset,
        valid_jpeg,
        valid_sha256,
        false_positive_offsets: vec![
            invalid_segment_offset,
            zero_dimension_offset,
            invalid_entropy_offset,
        ],
        truncated_offset,
    }
}

pub fn structurally_valid_jpeg() -> Vec<u8> {
    vec![
        0xFF, 0xD8, // SOI
        0xFF, 0xE0, 0x00, 0x04, 0x12, 0x34, // bounded APP0 segment
        0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11,
        0x00, // SOF0: 1x1, one component
        0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, // SOS: one component
        0x11, 0xFF, 0x00, 0x22, // entropy data with a stuffed FF byte
        0xFF, 0xD0, 0x33, // restart marker in entropy data
        0xFF, 0xD9, // EOI
    ]
}

fn jpeg_with_zero_height() -> Vec<u8> {
    let mut bytes = structurally_valid_jpeg();
    bytes[13] = 0;
    bytes[14] = 0;
    bytes
}

fn jpeg_with_invalid_entropy_marker() -> Vec<u8> {
    let mut bytes = structurally_valid_jpeg();
    let entropy_offset = 31;
    bytes.splice(entropy_offset..entropy_offset + 2, [0xFF, 0x02]);
    bytes
}
