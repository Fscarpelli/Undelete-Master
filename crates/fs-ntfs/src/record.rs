use um_fs_common::le;

/// Result of applying Update Sequence Array fixups and parsing a FILE
/// record header.
#[derive(Debug, Clone)]
pub struct FileRecord {
    /// Record bytes with fixups applied.
    pub data: Vec<u8>,
    pub sequence: u16,
    pub hard_link_count: u16,
    pub attrs_offset: u16,
    pub in_use: bool,
    pub is_directory: bool,
    pub bytes_used: u32,
    pub base_record: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordParseError {
    /// Not a FILE record at all (zeroed / never used / overwritten by data).
    NotAFileRecord,
    /// FILE signature present but structure is broken.
    Corrupt(String),
    /// Fixup mismatch: the record was partially overwritten sector-wise.
    FixupMismatch,
}

/// Parses one MFT record of `record_size` bytes, applying USA fixups.
pub fn parse_file_record(
    raw: &[u8],
    bytes_per_sector: u32,
) -> Result<FileRecord, RecordParseError> {
    if raw.len() < 42 || &raw[0..4] != b"FILE" {
        return Err(RecordParseError::NotAFileRecord);
    }
    let usa_offset = le::u16_at(raw, 4).unwrap_or(0) as usize;
    let usa_count = le::u16_at(raw, 6).unwrap_or(0) as usize;
    let sector = bytes_per_sector as usize;
    let expected_count = raw.len() / sector + 1;
    if usa_count == 0 || usa_count > expected_count {
        return Err(RecordParseError::Corrupt(format!(
            "implausible USA count {usa_count}"
        )));
    }
    if usa_offset + usa_count * 2 > raw.len() {
        return Err(RecordParseError::Corrupt("USA outside record".into()));
    }

    let mut data = raw.to_vec();
    let usn = [raw[usa_offset], raw[usa_offset + 1]];
    for i in 1..usa_count {
        let sector_end = i * sector;
        if sector_end > data.len() {
            return Err(RecordParseError::Corrupt("USA covers more than record".into()));
        }
        let check = &data[sector_end - 2..sector_end];
        if check != usn {
            return Err(RecordParseError::FixupMismatch);
        }
        let fixup_pos = usa_offset + i * 2;
        let orig = [raw[fixup_pos], raw[fixup_pos + 1]];
        data[sector_end - 2..sector_end].copy_from_slice(&orig);
    }

    let sequence = le::u16_at(&data, 16).unwrap_or(0);
    let hard_link_count = le::u16_at(&data, 18).unwrap_or(0);
    let attrs_offset = le::u16_at(&data, 20).unwrap_or(0);
    let flags = le::u16_at(&data, 22).unwrap_or(0);
    let bytes_used = le::u32_at(&data, 24).unwrap_or(0);
    let base_record = le::u64_at(&data, 32).unwrap_or(0) & 0x0000_FFFF_FFFF_FFFF;

    if attrs_offset as usize >= data.len() || bytes_used as usize > data.len() {
        return Err(RecordParseError::Corrupt("header offsets out of bounds".into()));
    }

    Ok(FileRecord {
        data,
        sequence,
        hard_link_count,
        attrs_offset,
        in_use: flags & 0x0001 != 0,
        is_directory: flags & 0x0002 != 0,
        bytes_used,
        base_record,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal 1024-byte FILE record with correct fixups.
    pub(crate) fn build_record(in_use: bool, seq: u16) -> Vec<u8> {
        let mut r = vec![0u8; 1024];
        r[0..4].copy_from_slice(b"FILE");
        r[4..6].copy_from_slice(&48u16.to_le_bytes()); // usa offset
        r[6..8].copy_from_slice(&3u16.to_le_bytes()); // usa count (1 usn + 2 sectors)
        r[16..18].copy_from_slice(&seq.to_le_bytes());
        r[20..22].copy_from_slice(&56u16.to_le_bytes()); // attrs offset
        let flags: u16 = if in_use { 1 } else { 0 };
        r[22..24].copy_from_slice(&flags.to_le_bytes());
        r[24..28].copy_from_slice(&64u32.to_le_bytes()); // bytes used
        r[56..60].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // end marker
        // Fixups: USN = 0x0001, originals captured from sector ends.
        let usn = 0x0001u16.to_le_bytes();
        let orig1 = [r[510], r[511]];
        let orig2 = [r[1022], r[1023]];
        r[48..50].copy_from_slice(&usn);
        r[50..52].copy_from_slice(&orig1); // original sector 1 end
        r[52..54].copy_from_slice(&orig2); // original sector 2 end
        r[510..512].copy_from_slice(&usn);
        r[1022..1024].copy_from_slice(&usn);
        r
    }

    #[test]
    fn parses_and_restores_fixups() {
        let raw = build_record(false, 7);
        let rec = parse_file_record(&raw, 512).unwrap();
        assert!(!rec.in_use);
        assert_eq!(rec.sequence, 7);
        // fixed-up positions restored to original zeros
        assert_eq!(&rec.data[510..512], &[0, 0]);
        assert_eq!(&rec.data[1022..1024], &[0, 0]);
    }

    #[test]
    fn detects_fixup_mismatch() {
        let mut raw = build_record(true, 1);
        raw[510] ^= 0xFF; // simulate torn sector
        assert_eq!(
            parse_file_record(&raw, 512).unwrap_err(),
            RecordParseError::FixupMismatch
        );
    }

    #[test]
    fn rejects_non_file() {
        assert_eq!(
            parse_file_record(&[0u8; 1024], 512).unwrap_err(),
            RecordParseError::NotAFileRecord
        );
    }
}
