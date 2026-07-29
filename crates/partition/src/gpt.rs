use crate::{check_overlaps, PartitionEntry, PartitionTable, PartitionTableKind};
use um_core::{Region, SourceReader};
use um_fs_common::{le, ScanError};

const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";
const MAX_ENTRIES: u32 = 1024;
const MAX_ENTRY_SIZE: u32 = 4096;

struct GptHeader {
    header_size: u32,
    current_lba: u64,
    backup_lba: u64,
    first_usable: u64,
    last_usable: u64,
    entry_lba: u64,
    num_entries: u32,
    entry_size: u32,
    entries_crc: u32,
}

fn parse_header(buf: &[u8], expected_lba: u64) -> Option<GptHeader> {
    if buf.get(0..8)? != GPT_SIGNATURE {
        return None;
    }
    let header_size = le::u32_at(buf, 12)?;
    if !(92..=512).contains(&header_size) || header_size as usize > buf.len() {
        return None;
    }
    let stored_crc = le::u32_at(buf, 16)?;
    let mut h = buf[..header_size as usize].to_vec();
    h[16..20].fill(0);
    if crc32fast::hash(&h) != stored_crc {
        return None;
    }
    let current_lba = le::u64_at(buf, 24)?;
    if current_lba != expected_lba {
        return None;
    }
    Some(GptHeader {
        header_size,
        current_lba,
        backup_lba: le::u64_at(buf, 32)?,
        first_usable: le::u64_at(buf, 40)?,
        last_usable: le::u64_at(buf, 48)?,
        entry_lba: le::u64_at(buf, 72)?,
        num_entries: le::u32_at(buf, 80)?,
        entry_size: le::u32_at(buf, 84)?,
        entries_crc: le::u32_at(buf, 88)?,
    })
}

/// Formats a GPT GUID (stored mixed-endian) in canonical text form.
fn format_guid(raw: &[u8]) -> Option<String> {
    if raw.len() != 16 {
        return None;
    }
    let d1 = u32::from_le_bytes(raw[0..4].try_into().ok()?);
    let d2 = u16::from_le_bytes(raw[4..6].try_into().ok()?);
    let d3 = u16::from_le_bytes(raw[6..8].try_into().ok()?);
    Some(format!(
        "{d1:08X}-{d2:04X}-{d3:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        raw[8], raw[9], raw[10], raw[11], raw[12], raw[13], raw[14], raw[15]
    ))
}

fn guid_description(guid: &str) -> &'static str {
    match guid {
        "EBD0A0A2-B9E5-4433-87C0-68B6B72699C7" => "Microsoft basic data",
        "C12A7328-F81F-11D2-BA4B-00A0C93EC93B" => "EFI system",
        "E3C9E316-0B5C-4DB8-817D-F92DF00215AE" => "Microsoft reserved",
        "DE94BBA4-06D1-4D40-A16A-BFD50179D6AC" => "Windows recovery",
        "0FC63DAF-8483-4772-8E79-3D69D8477DE4" => "Linux filesystem",
        _ => "Unknown GPT type",
    }
}

pub(crate) fn parse_gpt(reader: &dyn SourceReader) -> Result<PartitionTable, ScanError> {
    let sector = reader.sector_layout().logical as u64;
    let source_len = reader.len();
    let last_lba = source_len / sector - 1;
    let mut warnings = Vec::new();

    // Primary header at LBA 1, fall back to backup at the last LBA.
    let primary_buf = reader.read_vec_at(sector, sector as usize)?;
    let header = match parse_header(&primary_buf, 1) {
        Some(h) => h,
        None => {
            let backup_off = last_lba
                .checked_mul(sector)
                .ok_or_else(|| ScanError::Corrupt("backup LBA overflow".into()))?;
            let backup_buf = reader.read_vec_at(backup_off, sector as usize)?;
            match parse_header(&backup_buf, last_lba) {
                Some(h) => {
                    warnings.push("primary GPT header invalid; using backup header".into());
                    h
                }
                None => {
                    return Err(ScanError::NotRecognized("no valid GPT header".into()));
                }
            }
        }
    };

    if header.num_entries == 0 || header.num_entries > MAX_ENTRIES {
        return Err(ScanError::Corrupt(format!(
            "implausible GPT entry count {}",
            header.num_entries
        )));
    }
    if header.entry_size < 128 || header.entry_size > MAX_ENTRY_SIZE {
        return Err(ScanError::Corrupt(format!(
            "implausible GPT entry size {}",
            header.entry_size
        )));
    }
    let _ = header.header_size;
    let _ = (header.current_lba, header.backup_lba);

    let table_len = header.num_entries as u64 * header.entry_size as u64;
    let table_off = header
        .entry_lba
        .checked_mul(sector)
        .filter(|off| off + table_len <= source_len)
        .ok_or_else(|| ScanError::Corrupt("GPT entry table out of bounds".into()))?;
    let table = reader.read_vec_at(table_off, table_len as usize)?;
    if crc32fast::hash(&table) != header.entries_crc {
        return Err(ScanError::Corrupt("GPT entry table CRC mismatch".into()));
    }

    let mut partitions = Vec::new();
    let mut index = 0u32;
    for i in 0..header.num_entries as usize {
        let base = i * header.entry_size as usize;
        let entry = &table[base..base + header.entry_size as usize];
        let type_guid_raw = &entry[0..16];
        if type_guid_raw.iter().all(|&b| b == 0) {
            continue;
        }
        let first_lba = le::u64_at(entry, 32).unwrap_or(0);
        let last = le::u64_at(entry, 40).unwrap_or(0);
        if last < first_lba {
            warnings.push(format!("GPT entry {i} has last LBA before first; ignored"));
            continue;
        }
        if first_lba < header.first_usable || last > header.last_usable {
            warnings.push(format!("GPT entry {i} outside usable area; ignored"));
            continue;
        }
        let offset = match first_lba.checked_mul(sector) {
            Some(v) => v,
            None => continue,
        };
        let len = match (last - first_lba + 1).checked_mul(sector) {
            Some(v) => v,
            None => continue,
        };
        let Some(region) = Region::new(offset, len).filter(|r| r.end() <= source_len) else {
            warnings.push(format!("GPT entry {i} exceeds source bounds; ignored"));
            continue;
        };
        // Name: UTF-16LE, 36 code units at offset 56.
        let name_units: Vec<u16> = entry[56..128]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        let name = String::from_utf16_lossy(&name_units);
        let type_guid = format_guid(type_guid_raw);
        let desc = type_guid
            .as_deref()
            .map(guid_description)
            .unwrap_or("Unknown GPT type");
        partitions.push(PartitionEntry {
            index,
            region,
            mbr_type: None,
            type_guid,
            name: if name.is_empty() { None } else { Some(name) },
            type_description: desc.to_string(),
        });
        index += 1;
    }

    if partitions.is_empty() {
        return Err(ScanError::NotRecognized("GPT valid but empty".into()));
    }
    check_overlaps(&partitions, &mut warnings);
    Ok(PartitionTable {
        kind: PartitionTableKind::Gpt,
        partitions,
        warnings,
    })
}
