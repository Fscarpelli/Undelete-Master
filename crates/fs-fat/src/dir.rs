use um_fs_common::{dos_datetime_to_unix_ms, le};

pub const ATTR_READ_ONLY: u8 = 0x01;
pub const ATTR_HIDDEN: u8 = 0x02;
pub const ATTR_SYSTEM: u8 = 0x04;
pub const ATTR_VOLUME_ID: u8 = 0x08;
pub const ATTR_DIRECTORY: u8 = 0x10;
pub const ATTR_LFN: u8 = 0x0F;

/// One parsed 8.3 directory entry (possibly deleted).
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Decoded name. For deleted entries without an LFN, the lost first
    /// character is replaced by `_` and `name_certain` is false.
    pub name: String,
    pub name_certain: bool,
    pub is_deleted: bool,
    pub is_directory: bool,
    pub is_volume_label: bool,
    pub first_cluster: u32,
    pub size: u32,
    pub created_ms: Option<i64>,
    pub modified_ms: Option<i64>,
    /// Byte offset of the 32-byte entry within its directory stream.
    pub entry_offset: u64,
}

/// Checksum used to tie LFN entries to their 8.3 entry.
pub fn lfn_checksum(short_name_bytes: &[u8; 11]) -> u8 {
    let mut sum: u8 = 0;
    for b in short_name_bytes {
        sum = sum.rotate_right(1).wrapping_add(*b);
    }
    sum
}

fn decode_short_name(raw: &[u8; 11], deleted: bool) -> (String, bool) {
    let mut base: Vec<u8> = raw[..8].to_vec();
    let ext: Vec<u8> = raw[8..11].iter().copied().take_while(|&b| b != b' ').collect();
    while base.last() == Some(&b' ') {
        base.pop();
    }
    let mut certain = true;
    if deleted {
        // First byte was overwritten with 0xE5 on delete.
        if !base.is_empty() {
            base[0] = b'_';
        }
        certain = false;
    } else if base.first() == Some(&0x05) {
        base[0] = 0xE5; // KANJI escape
    }
    let mut name = String::from_utf8_lossy(&base).into_owned();
    if !ext.is_empty() {
        name.push('.');
        name.push_str(&String::from_utf8_lossy(&ext));
    }
    (name, certain)
}

/// Extracts the 13 UTF-16 code units carried by one LFN entry.
fn lfn_units(entry: &[u8]) -> Vec<u16> {
    let mut units = Vec::with_capacity(13);
    for range in [(1usize, 11usize), (14, 26), (28, 32)] {
        for pos in (range.0..range.1).step_by(2) {
            units.push(u16::from_le_bytes([entry[pos], entry[pos + 1]]));
        }
    }
    units
}

/// Parses a directory stream (concatenated 32-byte entries), reconstructing
/// long names — including for deleted files, whose LFN entries also carry the
/// 0xE5 marker but keep their name characters intact.
pub fn parse_directory(data: &[u8], base_offset: u64) -> Vec<DirEntry> {
    let mut out = Vec::new();
    // Pending LFN fragments in on-disk order (last part first).
    let mut pending_lfn: Vec<Vec<u16>> = Vec::new();
    let mut pending_checksum: Option<u8> = None;
    let mut pending_deleted = false;

    for (idx, entry) in data.chunks_exact(32).enumerate() {
        let first = entry[0];
        if first == 0x00 {
            break; // end of directory
        }
        let attrs = entry[11];
        let deleted = first == 0xE5;

        if attrs == ATTR_LFN {
            let checksum = entry[13];
            if !deleted {
                if pending_checksum.is_some() && pending_checksum != Some(checksum) {
                    pending_lfn.clear();
                }
                pending_checksum = Some(checksum);
                pending_deleted = false;
            } else {
                // Deleted LFN entry: sequence byte lost, chars preserved.
                if !pending_deleted {
                    pending_lfn.clear();
                }
                pending_checksum = Some(checksum);
                pending_deleted = true;
            }
            pending_lfn.push(lfn_units(entry));
            continue;
        }

        // 8.3 entry.
        let raw_name: [u8; 11] = entry[0..11].try_into().unwrap();
        let is_volume_label = attrs & ATTR_VOLUME_ID != 0;
        let is_directory = attrs & ATTR_DIRECTORY != 0;
        let (short_name, mut name_certain) = decode_short_name(&raw_name, deleted);

        // Attach LFN if the checksum ties it to this entry. For deleted
        // entries the first short-name byte is lost, so the checksum can no
        // longer be verified exactly; accept an immediately preceding deleted
        // LFN chain as best evidence.
        let mut name = short_name;
        if !pending_lfn.is_empty() {
            let checksum_ok = if deleted {
                pending_deleted
            } else {
                pending_checksum == Some(lfn_checksum(&raw_name))
            };
            if checksum_ok {
                let mut units = Vec::new();
                for part in pending_lfn.iter().rev() {
                    units.extend_from_slice(part);
                }
                while matches!(units.last(), Some(&0x0000) | Some(&0xFFFF)) {
                    units.pop();
                }
                if !units.is_empty() {
                    name = String::from_utf16_lossy(&units);
                    name_certain = true;
                }
            }
        }
        pending_lfn.clear();
        pending_checksum = None;
        pending_deleted = false;

        if is_volume_label && !deleted {
            continue;
        }
        if name == "." || name == ".." {
            continue;
        }

        let cluster_hi = le::u16_at(entry, 20).unwrap_or(0) as u32;
        let cluster_lo = le::u16_at(entry, 26).unwrap_or(0) as u32;
        let ctime = le::u16_at(entry, 14).unwrap_or(0);
        let cdate = le::u16_at(entry, 16).unwrap_or(0);
        let mtime = le::u16_at(entry, 22).unwrap_or(0);
        let mdate = le::u16_at(entry, 24).unwrap_or(0);

        out.push(DirEntry {
            name,
            name_certain,
            is_deleted: deleted,
            is_directory,
            is_volume_label,
            first_cluster: (cluster_hi << 16) | cluster_lo,
            size: le::u32_at(entry, 28).unwrap_or(0),
            created_ms: dos_datetime_to_unix_ms(cdate, ctime),
            modified_ms: dos_datetime_to_unix_ms(mdate, mtime),
            entry_offset: base_offset + (idx as u64) * 32,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn short_entry(name11: &[u8; 11], attrs: u8, cluster: u32, size: u32) -> [u8; 32] {
        let mut e = [0u8; 32];
        e[0..11].copy_from_slice(name11);
        e[11] = attrs;
        e[20..22].copy_from_slice(&((cluster >> 16) as u16).to_le_bytes());
        e[26..28].copy_from_slice(&((cluster & 0xFFFF) as u16).to_le_bytes());
        e[28..32].copy_from_slice(&size.to_le_bytes());
        e
    }

    fn lfn_entry(seq: u8, checksum: u8, chars: &[u16]) -> [u8; 32] {
        let mut e = [0u8; 32];
        e[0] = seq;
        e[11] = ATTR_LFN;
        e[13] = checksum;
        let mut padded = chars.to_vec();
        if padded.len() < 13 {
            padded.push(0);
            while padded.len() < 13 {
                padded.push(0xFFFF);
            }
        }
        let ranges = [(1usize, 11usize), (14, 26), (28, 32)];
        let mut i = 0;
        for (start, end) in ranges {
            for pos in (start..end).step_by(2) {
                e[pos..pos + 2].copy_from_slice(&padded[i].to_le_bytes());
                i += 1;
            }
        }
        e
    }

    #[test]
    fn parses_active_lfn() {
        let short: [u8; 11] = *b"HELLO   TXT";
        let ck = lfn_checksum(&short);
        let units: Vec<u16> = "hello-world.txt".encode_utf16().collect();
        let mut data = Vec::new();
        data.extend_from_slice(&lfn_entry(0x42, ck, &units[13..])); // part 2, last
        data.extend_from_slice(&lfn_entry(0x01, ck, &units[..13])); // part 1
        data.extend_from_slice(&short_entry(&short, 0x20, 5, 100));
        data.extend_from_slice(&[0u8; 32]);
        let entries = parse_directory(&data, 0);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "hello-world.txt");
        assert!(entries[0].name_certain);
        assert_eq!(entries[0].first_cluster, 5);
    }

    #[test]
    fn recovers_deleted_lfn_name() {
        let short: [u8; 11] = *b"DOCUME~1DOC";
        let ck = lfn_checksum(&short);
        let units: Vec<u16> = "Documento Final.docx".encode_utf16().collect();
        let mut e1 = lfn_entry(0x42, ck, &units[13..]);
        let mut e2 = lfn_entry(0x01, ck, &units[..13]);
        let mut se = short_entry(&short, 0x20, 8, 4096);
        e1[0] = 0xE5;
        e2[0] = 0xE5;
        se[0] = 0xE5;
        let mut data = Vec::new();
        data.extend_from_slice(&e1);
        data.extend_from_slice(&e2);
        data.extend_from_slice(&se);
        data.extend_from_slice(&[0u8; 32]);
        let entries = parse_directory(&data, 0);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_deleted);
        assert_eq!(entries[0].name, "Documento Final.docx");
        assert!(entries[0].name_certain);
    }

    #[test]
    fn deleted_without_lfn_marks_uncertain_first_char() {
        let mut se = short_entry(b"REPORT  PDF", 0x20, 9, 1000);
        se[0] = 0xE5;
        let mut data = se.to_vec();
        data.extend_from_slice(&[0u8; 32]);
        let entries = parse_directory(&data, 0);
        assert_eq!(entries[0].name, "_EPORT.PDF");
        assert!(!entries[0].name_certain);
    }

    #[test]
    fn never_panics_on_garbage(){
        let junk: Vec<u8> = (0..4096).map(|i| (i * 31 % 251) as u8).collect();
        let _ = parse_directory(&junk, 0);
    }
}
