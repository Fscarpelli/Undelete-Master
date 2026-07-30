//! Deterministic synthetic filesystem images for testing.
//!
//! Every builder produces an in-memory image plus a truth manifest with
//! expected candidates and SHA-256 hashes, so recovery correctness can be
//! asserted exactly. No fixture ever touches a real device.

#![forbid(unsafe_code)]

pub mod carving;
pub mod exfat;
pub mod fat;
pub mod manifest;
pub mod ntfs;

pub use manifest::{ExpectedCandidate, FixtureManifest};

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}

/// Deterministic pseudo-random bytes (xorshift) for fixture file contents.
pub fn deterministic_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(len);
    out
}
