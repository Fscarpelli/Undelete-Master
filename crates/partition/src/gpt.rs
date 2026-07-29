use crate::{check_overlaps, PartitionEntry, PartitionTable, PartitionTableKind};
use um_core::{Region, SourceReader};
use um_fs_common::{le, ScanError};

const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";
const MAX_ENTRIES: u32 = 1024;
const MAX_ENTRY_SIZE: u32 = 4096;

struct GptHeader {
    current_lba: u64,
    backup_lba: u64,
    first_usable: u64,
    last_usable: u64,
    disk_guid: [u8; 16],
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
        current_lba,
        backup_lba: le::u64_at(buf, 32)?,
        first_usable: le::u64_at(buf, 40)?,
        last_usable: le::u64_at(buf, 48)?,
        disk_guid: buf.get(56..72)?.try_into().ok()?,
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
    let sector_count = source_len
        .checked_div(sector)
        .filter(|count| *count >= 2)
        .ok_or_else(|| ScanError::Corrupt("GPT source has fewer than two sectors".into()))?;
    let last_lba = sector_count - 1;

    // The alternate header has a canonical location at the final logical
    // block. Never follow a possibly corrupt primary header to an arbitrary
    // interior LBA.
    let backup_off = last_lba
        .checked_mul(sector)
        .filter(|offset| {
            offset
                .checked_add(sector)
                .is_some_and(|end| end <= source_len)
        })
        .ok_or_else(|| ScanError::Corrupt("backup GPT header out of bounds".into()))?;

    // Validate both canonical copies even when the primary is usable. This
    // detects a stale or attacker-controlled backup instead of silently
    // reporting one side of a contradictory partition map.
    let (primary_header, mut primary_failure) =
        read_validated_header(reader, sector, sector, 1, last_lba, true);
    let (backup_header, mut backup_failure) =
        read_validated_header(reader, backup_off, sector, last_lba, last_lba, false);
    if let (Some(primary), Some(backup)) = (&primary_header, &backup_header) {
        if !headers_are_reciprocal(primary, backup) {
            return Err(ScanError::Corrupt(
                "primary and backup GPT headers are not reciprocal or consistent".into(),
            ));
        }
    }

    let primary_table = primary_header.as_ref().and_then(|header| {
        match parse_gpt_copy(reader, header, sector, source_len) {
            Ok(table) => Some(table),
            Err(error) => {
                primary_failure = Some(error);
                None
            }
        }
    });
    let backup_table = backup_header.as_ref().and_then(|header| {
        match parse_gpt_copy(reader, header, sector, source_len) {
            Ok(table) => Some(table),
            Err(error) => {
                backup_failure = Some(error);
                None
            }
        }
    });

    match (primary_table, backup_table) {
        (Some(table), Some(_)) => Ok(table),
        (Some(mut table), None) => {
            table
                .warnings
                .insert(0, "backup GPT copy unusable; using primary GPT copy".into());
            Ok(table)
        }
        (None, Some(mut table)) => {
            table
                .warnings
                .insert(0, "primary GPT copy unusable; using backup GPT copy".into());
            Ok(table)
        }
        (None, None) => Err(prefer_copy_failure(primary_failure, backup_failure)),
    }
}

fn read_validated_header(
    reader: &dyn SourceReader,
    offset: u64,
    sector: u64,
    expected_lba: u64,
    last_lba: u64,
    primary: bool,
) -> (Option<GptHeader>, Option<ScanError>) {
    let sector_len = match usize::try_from(sector) {
        Ok(len) => len,
        Err(_) => {
            return (
                None,
                Some(ScanError::Corrupt(
                    "logical sector size does not fit address space".into(),
                )),
            )
        }
    };
    let buffer = match reader.read_vec_at(offset, sector_len) {
        Ok(buffer) => buffer,
        Err(error) => return (None, Some(error.into())),
    };
    let Some(header) = parse_header(&buffer, expected_lba) else {
        return (
            None,
            Some(ScanError::NotRecognized("no valid GPT header".into())),
        );
    };
    match validate_header_topology(&header, last_lba, primary) {
        Ok(()) => (Some(header), None),
        Err(error) => (None, Some(error)),
    }
}

fn prefer_copy_failure(primary: Option<ScanError>, backup: Option<ScanError>) -> ScanError {
    let primary = match primary {
        Some(error @ (ScanError::Read(_) | ScanError::Corrupt(_))) => return error,
        other => other,
    };
    let backup = match backup {
        Some(error @ (ScanError::Read(_) | ScanError::Corrupt(_))) => return error,
        other => other,
    };
    primary
        .or(backup)
        .unwrap_or_else(|| ScanError::NotRecognized("no valid GPT copy".into()))
}

fn validate_header_topology(
    header: &GptHeader,
    last_lba: u64,
    primary: bool,
) -> Result<(), ScanError> {
    let (expected_current, expected_backup) = if primary {
        (1, last_lba)
    } else {
        (last_lba, 1)
    };
    if header.current_lba != expected_current || header.backup_lba != expected_backup {
        return Err(ScanError::Corrupt(
            "GPT header self/alternate locations are not canonical".into(),
        ));
    }
    if header.first_usable > header.last_usable
        || header.first_usable <= 1
        || header.last_usable >= last_lba
    {
        return Err(ScanError::Corrupt("GPT usable LBA range is invalid".into()));
    }
    Ok(())
}

fn headers_are_reciprocal(primary: &GptHeader, backup: &GptHeader) -> bool {
    primary.current_lba == backup.backup_lba
        && primary.backup_lba == backup.current_lba
        && primary.first_usable == backup.first_usable
        && primary.last_usable == backup.last_usable
        && primary.disk_guid == backup.disk_guid
        && primary.num_entries == backup.num_entries
        && primary.entry_size == backup.entry_size
        && primary.entries_crc == backup.entries_crc
}

fn parse_gpt_copy(
    reader: &dyn SourceReader,
    header: &GptHeader,
    sector: u64,
    source_len: u64,
) -> Result<PartitionTable, ScanError> {
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
    let table_len = (header.num_entries as u64)
        .checked_mul(header.entry_size as u64)
        .ok_or_else(|| ScanError::Corrupt("GPT entry table length overflow".into()))?;
    let table_sectors = table_len
        .checked_add(sector - 1)
        .and_then(|rounded| rounded.checked_div(sector))
        .ok_or_else(|| ScanError::Corrupt("GPT entry table sector span overflow".into()))?;
    let table_end_lba = header
        .entry_lba
        .checked_add(table_sectors)
        .ok_or_else(|| ScanError::Corrupt("GPT entry table LBA span overflow".into()))?;
    let table_is_reserved = if header.current_lba == 1 {
        header.entry_lba > header.current_lba && table_end_lba <= header.first_usable
    } else {
        header.entry_lba > header.last_usable && table_end_lba <= header.current_lba
    };
    if !table_is_reserved {
        return Err(ScanError::Corrupt(
            "GPT entry table is outside its reserved metadata area".into(),
        ));
    }
    let table_off = header
        .entry_lba
        .checked_mul(sector)
        .filter(|off| {
            off.checked_add(table_len)
                .map(|end| end <= source_len)
                .unwrap_or(false)
        })
        .ok_or_else(|| ScanError::Corrupt("GPT entry table out of bounds".into()))?;
    let table_len = usize::try_from(table_len)
        .map_err(|_| ScanError::Corrupt("GPT entry table does not fit address space".into()))?;
    let table = reader.read_vec_at(table_off, table_len)?;
    if crc32fast::hash(&table) != header.entries_crc {
        return Err(ScanError::Corrupt("GPT entry table CRC mismatch".into()));
    }

    let mut warnings = Vec::new();
    let mut partitions = Vec::new();
    let mut index = 0u32;
    for i in 0..header.num_entries as usize {
        let entry_size = header.entry_size as usize;
        let base = i
            .checked_mul(entry_size)
            .ok_or_else(|| ScanError::Corrupt("GPT entry offset overflow".into()))?;
        let end = base
            .checked_add(entry_size)
            .ok_or_else(|| ScanError::Corrupt("GPT entry end overflow".into()))?;
        let entry = table
            .get(base..end)
            .ok_or_else(|| ScanError::Corrupt("GPT entry outside entry table".into()))?;
        let type_guid_raw = entry
            .get(0..16)
            .ok_or_else(|| ScanError::Corrupt("truncated GPT type GUID".into()))?;
        if type_guid_raw.iter().all(|&b| b == 0) {
            continue;
        }
        let first_lba = le::u64_at(entry, 32)
            .ok_or_else(|| ScanError::Corrupt("truncated GPT first LBA".into()))?;
        let last = le::u64_at(entry, 40)
            .ok_or_else(|| ScanError::Corrupt("truncated GPT last LBA".into()))?;
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
            None => {
                return Err(ScanError::Corrupt(format!(
                    "GPT entry {i} byte offset overflows"
                )))
            }
        };
        let len = match last
            .checked_sub(first_lba)
            .and_then(|span| span.checked_add(1))
            .and_then(|sectors| sectors.checked_mul(sector))
        {
            Some(v) => v,
            None => {
                return Err(ScanError::Corrupt(format!(
                    "GPT entry {i} byte length overflows"
                )))
            }
        };
        let Some(region) = Region::new(offset, len).filter(|r| r.end() <= source_len) else {
            warnings.push(format!("GPT entry {i} exceeds source bounds; ignored"));
            continue;
        };
        // Name: UTF-16LE, 36 code units at offset 56.
        let name_bytes = entry
            .get(56..128)
            .ok_or_else(|| ScanError::Corrupt("truncated GPT partition name".into()))?;
        let name_units: Vec<u16> = name_bytes
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
        index = index
            .checked_add(1)
            .ok_or_else(|| ScanError::Corrupt("GPT partition index overflow".into()))?;
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
