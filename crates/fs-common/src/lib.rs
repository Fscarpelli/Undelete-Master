//! Shared helpers for filesystem parsers: bounded little-endian reads,
//! FILETIME/DOS time conversion, allocation bitmaps and scan errors.

#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("read error: {0}")]
    Read(#[from] um_core::ReadError),
    #[error("invalid filesystem structure: {0}")]
    Corrupt(String),
    #[error("filesystem not recognized: {0}")]
    NotRecognized(String),
}

/// Bounded little-endian accessors. All return `None` on out-of-bounds
/// instead of panicking, so parsers can degrade gracefully on hostile input.
pub mod le {
    pub fn u16_at(buf: &[u8], off: usize) -> Option<u16> {
        Some(u16::from_le_bytes(buf.get(off..off + 2)?.try_into().ok()?))
    }
    pub fn u32_at(buf: &[u8], off: usize) -> Option<u32> {
        Some(u32::from_le_bytes(buf.get(off..off + 4)?.try_into().ok()?))
    }
    pub fn u64_at(buf: &[u8], off: usize) -> Option<u64> {
        Some(u64::from_le_bytes(buf.get(off..off + 8)?.try_into().ok()?))
    }
    pub fn i8_at(buf: &[u8], off: usize) -> Option<i8> {
        buf.get(off).map(|b| *b as i8)
    }
    pub fn u8_at(buf: &[u8], off: usize) -> Option<u8> {
        buf.get(off).copied()
    }
}

/// Converts a Windows FILETIME (100ns ticks since 1601-01-01 UTC) to unix
/// milliseconds. Returns `None` for zero or implausible values.
pub fn filetime_to_unix_ms(filetime: u64) -> Option<i64> {
    if filetime == 0 {
        return None;
    }
    const EPOCH_DIFF_100NS: i128 = 116_444_736_000_000_000; // 1601 -> 1970
    let ms = (filetime as i128 - EPOCH_DIFF_100NS) / 10_000;
    // Reject dates before 1970 or after year ~4000 as implausible metadata.
    if !(0..=64_060_588_800_000).contains(&ms) {
        return None;
    }
    Some(ms as i64)
}

/// Converts DOS date+time (FAT) to unix milliseconds. Returns `None` for
/// invalid encodings.
pub fn dos_datetime_to_unix_ms(date: u16, time: u16) -> Option<i64> {
    let year = 1980 + ((date >> 9) & 0x7F) as i64;
    let month = ((date >> 5) & 0x0F) as i64;
    let day = (date & 0x1F) as i64;
    let hour = ((time >> 11) & 0x1F) as i64;
    let min = ((time >> 5) & 0x3F) as i64;
    let sec = ((time & 0x1F) * 2) as i64;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 59 {
        return None;
    }
    // Days since unix epoch via civil-from-days algorithm (Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400) + hour * 3600 + min * 60 + sec) * 1000)
}

/// A simple cluster-allocation bitmap: bit set = allocated.
#[derive(Debug, Clone)]
pub struct AllocationMap {
    bits: Vec<u8>,
    cluster_count: u64,
}

impl AllocationMap {
    pub fn new(cluster_count: u64) -> Self {
        let bytes = cluster_count.div_ceil(8) as usize;
        Self {
            bits: vec![0; bytes],
            cluster_count,
        }
    }

    pub fn from_raw(bits: Vec<u8>, cluster_count: u64) -> Self {
        Self {
            bits,
            cluster_count,
        }
    }

    pub fn cluster_count(&self) -> u64 {
        self.cluster_count
    }

    pub fn set_allocated(&mut self, cluster: u64) {
        if cluster < self.cluster_count {
            self.bits[(cluster / 8) as usize] |= 1 << (cluster % 8);
        }
    }

    /// `None` when the cluster is outside the map (unknown).
    pub fn is_allocated(&self, cluster: u64) -> Option<bool> {
        if cluster >= self.cluster_count {
            return None;
        }
        Some(self.bits[(cluster / 8) as usize] & (1 << (cluster % 8)) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_conversion() {
        // 2020-01-01 00:00:00 UTC = 132223104000000000 in FILETIME
        assert_eq!(filetime_to_unix_ms(132_223_104_000_000_000), Some(1_577_836_800_000));
        assert_eq!(filetime_to_unix_ms(0), None);
    }

    #[test]
    fn dos_datetime_conversion() {
        // 2021-03-15, 12:30:10 -> date: ((2021-1980)<<9)|(3<<5)|15, time: (12<<11)|(30<<5)|5
        let date = ((41u16) << 9) | (3 << 5) | 15;
        let time = (12u16 << 11) | (30 << 5) | 5;
        let ms = dos_datetime_to_unix_ms(date, time).unwrap();
        assert_eq!(ms, 1_615_811_410_000);
        assert_eq!(dos_datetime_to_unix_ms(0, 0), None); // month 0 invalid
    }

    #[test]
    fn allocation_map() {
        let mut m = AllocationMap::new(20);
        m.set_allocated(0);
        m.set_allocated(9);
        m.set_allocated(19);
        assert_eq!(m.is_allocated(0), Some(true));
        assert_eq!(m.is_allocated(1), Some(false));
        assert_eq!(m.is_allocated(9), Some(true));
        assert_eq!(m.is_allocated(19), Some(true));
        assert_eq!(m.is_allocated(20), None);
    }
}
