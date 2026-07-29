use crate::record::FileRecord;
use crate::runs::{decode_runlist, RunElement};
use um_fs_common::{filetime_to_unix_ms, le};

pub const ATTR_STANDARD_INFORMATION: u32 = 0x10;
pub const ATTR_ATTRIBUTE_LIST: u32 = 0x20;
pub const ATTR_FILE_NAME: u32 = 0x30;
pub const ATTR_DATA: u32 = 0x80;
pub const ATTR_INDEX_ROOT: u32 = 0x90;
pub const ATTR_INDEX_ALLOCATION: u32 = 0xA0;
pub const ATTR_BITMAP: u32 = 0xB0;
pub const ATTR_END: u32 = 0xFFFF_FFFF;

/// Attribute flags (attribute header, offset 12).
pub const ATTR_FLAG_COMPRESSED: u16 = 0x0001;
pub const ATTR_FLAG_ENCRYPTED: u16 = 0x4000;
pub const ATTR_FLAG_SPARSE: u16 = 0x8000;

/// One attribute located inside a fixed-up record.
#[derive(Debug, Clone)]
pub struct Attribute<'a> {
    pub type_id: u32,
    pub name: Option<String>,
    pub flags: u16,
    pub body: AttrBody<'a>,
}

#[derive(Debug, Clone)]
pub enum AttrBody<'a> {
    Resident {
        value: &'a [u8],
        /// Offset of the value inside the record (for physical addressing).
        value_offset_in_record: usize,
    },
    NonResident {
        starting_vcn: u64,
        data_size: u64,
        initialized_size: u64,
        allocated_size: u64,
        runlist: &'a [u8],
    },
}

/// Parses the attributes of a fixed-up record, defensively.
///
/// The boolean is true only when a valid end marker was reached without
/// skipping any malformed attribute header, name, or body.
pub fn iter_attributes(record: &FileRecord) -> (Vec<Attribute<'_>>, bool) {
    let data = &record.data;
    let mut out = Vec::new();
    let mut pos = record.attrs_offset as usize;
    let mut structurally_complete = true;
    // A record can hold at most a few dozen attributes; guard against loops.
    for _ in 0..256 {
        let Some(type_id) = le::u32_at(data, pos) else {
            return (out, false);
        };
        if type_id == ATTR_END {
            return (out, structurally_complete);
        }
        let Some(attr_len) = le::u32_at(data, pos + 4) else {
            return (out, false);
        };
        let attr_len = attr_len as usize;
        let Some(attr_end) = pos.checked_add(attr_len) else {
            return (out, false);
        };
        if attr_len < 24 || attr_end > data.len() {
            return (out, false);
        }
        let attr = &data[pos..attr_end];
        let non_resident = le::u8_at(attr, 8).unwrap_or(0) != 0;
        let name_len = le::u8_at(attr, 9).unwrap_or(0) as usize;
        let name_off = le::u16_at(attr, 10).unwrap_or(0) as usize;
        let flags = le::u16_at(attr, 12).unwrap_or(0);
        let name = if name_len > 0 {
            let name_end = name_len
                .checked_mul(2)
                .and_then(|bytes| name_off.checked_add(bytes));
            if let Some(name_end) = name_end.filter(|end| *end <= attr.len()) {
                let units: Vec<u16> = attr[name_off..name_end]
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                Some(String::from_utf16_lossy(&units))
            } else {
                structurally_complete = false;
                None
            }
        } else {
            None
        };

        let body = if non_resident {
            if attr.len() < 64 {
                structurally_complete = false;
                pos = attr_end;
                continue;
            }
            let starting_vcn = le::u64_at(attr, 16).unwrap_or(0);
            let run_off = le::u16_at(attr, 32).unwrap_or(0) as usize;
            let allocated_size = le::u64_at(attr, 40).unwrap_or(0);
            let data_size = le::u64_at(attr, 48).unwrap_or(0);
            let initialized_size = le::u64_at(attr, 56).unwrap_or(data_size);
            if !(64..attr.len()).contains(&run_off) || initialized_size > data_size {
                structurally_complete = false;
                pos = attr_end;
                continue;
            }
            AttrBody::NonResident {
                starting_vcn,
                data_size,
                initialized_size,
                allocated_size,
                runlist: &data[pos + run_off..pos + attr_len],
            }
        } else {
            let value_len = le::u32_at(attr, 16).unwrap_or(0) as usize;
            let value_off = le::u16_at(attr, 20).unwrap_or(0) as usize;
            let value_end = value_off.checked_add(value_len);
            if value_off < 24 || value_end.is_none_or(|end| end > attr.len()) {
                structurally_complete = false;
                pos = attr_end;
                continue;
            }
            let value_end = value_end.unwrap_or(value_off);
            AttrBody::Resident {
                value: &data[pos + value_off..pos + value_end],
                value_offset_in_record: pos + value_off,
            }
        };

        out.push(Attribute {
            type_id,
            name,
            flags,
            body,
        });
        pos = attr_end;
    }
    (out, false)
}

/// Parsed `$STANDARD_INFORMATION`.
#[derive(Debug, Clone, Copy, Default)]
pub struct StandardInformation {
    pub created_ms: Option<i64>,
    pub modified_ms: Option<i64>,
    pub accessed_ms: Option<i64>,
    pub dos_attributes: u32,
}

pub fn parse_standard_information(value: &[u8]) -> Option<StandardInformation> {
    if value.len() < 48 {
        return None;
    }
    Some(StandardInformation {
        created_ms: filetime_to_unix_ms(le::u64_at(value, 0)?),
        modified_ms: filetime_to_unix_ms(le::u64_at(value, 8)?),
        accessed_ms: filetime_to_unix_ms(le::u64_at(value, 24)?),
        dos_attributes: le::u32_at(value, 32)?,
    })
}

/// NTFS `$FILE_NAME` namespaces.
pub const NS_POSIX: u8 = 0;
pub const NS_WIN32: u8 = 1;
pub const NS_DOS: u8 = 2;
pub const NS_WIN32_AND_DOS: u8 = 3;

/// Parsed `$FILE_NAME` attribute value.
#[derive(Debug, Clone)]
pub struct FileNameAttr {
    pub parent_record: u64,
    pub parent_sequence: u16,
    pub namespace: u8,
    pub name: String,
    pub logical_size: u64,
    pub allocated_size: u64,
}

pub fn parse_file_name(value: &[u8]) -> Option<FileNameAttr> {
    if value.len() < 66 {
        return None;
    }
    let parent_ref = le::u64_at(value, 0)?;
    let allocated_size = le::u64_at(value, 40)?;
    let logical_size = le::u64_at(value, 48)?;
    let name_len = le::u8_at(value, 64)? as usize;
    let namespace = le::u8_at(value, 65)?;
    let name_bytes = value.get(66..66 + name_len * 2)?;
    let units: Vec<u16> = name_bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some(FileNameAttr {
        parent_record: parent_ref & 0x0000_FFFF_FFFF_FFFF,
        parent_sequence: (parent_ref >> 48) as u16,
        namespace,
        name: String::from_utf16_lossy(&units),
        logical_size,
        allocated_size,
    })
}

/// A `$DATA` stream extracted from a record.
#[derive(Debug, Clone)]
pub struct DataStream {
    /// `None` for the unnamed (main) stream, `Some(name)` for ADS.
    pub name: Option<String>,
    pub flags: u16,
    pub kind: DataStreamKind,
}

#[derive(Debug, Clone)]
pub enum DataStreamKind {
    Resident {
        /// Absolute offset of the value inside the MFT record.
        value_offset_in_record: usize,
        len: u64,
    },
    NonResident {
        data_size: u64,
        initialized_size: u64,
        runs: Vec<RunElement>,
    },
}

/// Extracts all `$DATA` streams from a record's attributes.
pub fn extract_data_streams(
    attrs: &[Attribute<'_>],
    total_clusters: u64,
    warnings: &mut Vec<String>,
) -> Vec<DataStream> {
    let mut out = Vec::new();
    for a in attrs {
        if a.type_id != ATTR_DATA {
            continue;
        }
        match &a.body {
            AttrBody::Resident {
                value,
                value_offset_in_record,
            } => out.push(DataStream {
                name: a.name.clone(),
                flags: a.flags,
                kind: DataStreamKind::Resident {
                    value_offset_in_record: *value_offset_in_record,
                    len: value.len() as u64,
                },
            }),
            AttrBody::NonResident {
                data_size,
                initialized_size,
                runlist,
                ..
            } => match decode_runlist(runlist, total_clusters) {
                Ok(runs) => out.push(DataStream {
                    name: a.name.clone(),
                    flags: a.flags,
                    kind: DataStreamKind::NonResident {
                        data_size: *data_size,
                        initialized_size: *initialized_size,
                        runs,
                    },
                }),
                Err(e) => warnings.push(format!("malformed runlist ignored: {e}")),
            },
        }
    }
    out
}
