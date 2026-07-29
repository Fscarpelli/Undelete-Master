use crate::{check_overlaps, PartitionEntry, PartitionTable, PartitionTableKind};
use um_core::{Region, SourceReader};
use um_fs_common::{le, ScanError};

const MBR_SIGNATURE_OFFSET: usize = 510;
const ENTRY_TABLE_OFFSET: usize = 446;
const ENTRY_SIZE: usize = 16;
const MAX_EBR_CHAIN: usize = 128;

pub(crate) fn mbr_type_description(t: u8) -> &'static str {
    match t {
        0x01 => "FAT12",
        0x04 | 0x06 | 0x0E => "FAT16",
        0x05 | 0x0F => "Extended",
        0x07 => "NTFS/exFAT (IFS)",
        0x0B | 0x0C => "FAT32",
        0x82 => "Linux swap",
        0x83 => "Linux",
        0xEE => "GPT protective",
        0xEF => "EFI system",
        _ => "Unknown",
    }
}

struct RawEntry {
    part_type: u8,
    lba_start: u64,
    sectors: u64,
}

fn parse_entries(sector_buf: &[u8]) -> Option<Vec<RawEntry>> {
    if le::u16_at(sector_buf, MBR_SIGNATURE_OFFSET)? != 0xAA55 {
        return None;
    }
    let mut out = Vec::with_capacity(4);
    for i in 0..4 {
        let base = ENTRY_TABLE_OFFSET + i * ENTRY_SIZE;
        let part_type = le::u8_at(sector_buf, base + 4)?;
        let lba_start = le::u32_at(sector_buf, base + 8)? as u64;
        let sectors = le::u32_at(sector_buf, base + 12)? as u64;
        out.push(RawEntry {
            part_type,
            lba_start,
            sectors,
        });
    }
    Some(out)
}

pub(crate) fn parse_mbr(reader: &dyn SourceReader) -> Result<PartitionTable, ScanError> {
    let sector = reader.sector_layout().logical as u64;
    let buf = reader.read_vec_at(0, sector.max(512) as usize)?;
    let entries =
        parse_entries(&buf).ok_or_else(|| ScanError::NotRecognized("no MBR signature".into()))?;

    let mut warnings = Vec::new();
    let mut partitions = Vec::new();
    let mut index = 0u32;
    let source_len = reader.len();

    let mut push_partition =
        |lba: u64, sectors: u64, part_type: u8, warnings: &mut Vec<String>, index: &mut u32| {
            let offset = match lba.checked_mul(sector) {
                Some(v) => v,
                None => {
                    warnings.push(format!("partition LBA {lba} overflows"));
                    return;
                }
            };
            let len = match sectors.checked_mul(sector) {
                Some(v) => v,
                None => {
                    warnings.push(format!("partition size at LBA {lba} overflows"));
                    return;
                }
            };
            match Region::new(offset, len) {
                Some(r) if r.end() <= source_len && !r.is_empty() => {
                    partitions.push(PartitionEntry {
                        index: *index,
                        region: r,
                        mbr_type: Some(part_type),
                        type_guid: None,
                        name: None,
                        type_description: mbr_type_description(part_type).to_string(),
                    });
                    *index += 1;
                }
                _ => warnings.push(format!(
                    "partition at LBA {lba} ({sectors} sectors) exceeds source bounds; ignored"
                )),
            }
        };

    // Primary entries; follow extended partitions through EBR chains.
    let mut extended_chains: Vec<u64> = Vec::new();
    for e in &entries {
        if e.part_type == 0 || e.sectors == 0 {
            continue;
        }
        if e.part_type == 0x05 || e.part_type == 0x0F {
            extended_chains.push(e.lba_start);
        } else {
            push_partition(e.lba_start, e.sectors, e.part_type, &mut warnings, &mut index);
        }
    }

    for ext_base in extended_chains {
        let mut visited = std::collections::HashSet::new();
        let mut current = ext_base;
        for _ in 0..MAX_EBR_CHAIN {
            if !visited.insert(current) {
                warnings.push("EBR chain loop detected; chain truncated".into());
                break;
            }
            let ebr_off = match current.checked_mul(sector) {
                Some(v) if v + 512 <= source_len => v,
                _ => {
                    warnings.push("EBR outside source bounds; chain truncated".into());
                    break;
                }
            };
            let ebr = match reader.read_vec_at(ebr_off, 512) {
                Ok(b) => b,
                Err(_) => {
                    warnings.push("EBR unreadable; chain truncated".into());
                    break;
                }
            };
            let Some(ebr_entries) = parse_entries(&ebr) else {
                warnings.push("EBR missing signature; chain truncated".into());
                break;
            };
            // Entry 0: logical partition relative to this EBR.
            let e0 = &ebr_entries[0];
            if e0.part_type != 0 && e0.sectors != 0 {
                if let Some(lba) = current.checked_add(e0.lba_start) {
                    push_partition(lba, e0.sectors, e0.part_type, &mut warnings, &mut index);
                }
            }
            // Entry 1: next EBR relative to extended base.
            let e1 = &ebr_entries[1];
            if (e1.part_type == 0x05 || e1.part_type == 0x0F) && e1.sectors != 0 {
                match ext_base.checked_add(e1.lba_start) {
                    Some(next) => current = next,
                    None => break,
                }
            } else {
                break;
            }
        }
    }

    if partitions.is_empty() {
        return Err(ScanError::NotRecognized(
            "MBR signature present but no usable partitions".into(),
        ));
    }
    check_overlaps(&partitions, &mut warnings);
    Ok(PartitionTable {
        kind: PartitionTableKind::Mbr,
        partitions,
        warnings,
    })
}
