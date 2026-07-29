use um_fs_common::{le, ScanError};

/// Validated NTFS boot sector parameters.
#[derive(Debug, Clone)]
pub struct NtfsBoot {
    pub bytes_per_sector: u32,
    pub sectors_per_cluster: u32,
    pub cluster_size: u64,
    pub total_sectors: u64,
    pub total_clusters: u64,
    pub mft_lcn: u64,
    pub mftmirr_lcn: u64,
    pub file_record_size: u32,
    pub index_record_size: u32,
    pub volume_serial: u64,
}

fn size_from_clusters_field(raw: i8, cluster_size: u64) -> Option<u64> {
    if raw > 0 {
        (raw as u64).checked_mul(cluster_size)
    } else if raw < 0 {
        let exp = raw.checked_neg()? as u32;
        if exp > 31 {
            return None;
        }
        Some(1u64 << exp)
    } else {
        None
    }
}

impl NtfsBoot {
    /// Parses and cross-validates an NTFS boot sector.
    pub fn parse(sector0: &[u8], volume_len: u64) -> Result<Self, ScanError> {
        if sector0.len() < 512 {
            return Err(ScanError::NotRecognized("boot sector too small".into()));
        }
        if &sector0[3..11] != b"NTFS    " {
            return Err(ScanError::NotRecognized("missing NTFS OEM ID".into()));
        }
        if le::u16_at(sector0, 510) != Some(0xAA55) {
            return Err(ScanError::NotRecognized("missing boot signature".into()));
        }
        let bytes_per_sector = le::u16_at(sector0, 11).unwrap_or(0) as u32;
        if !bytes_per_sector.is_power_of_two() || !(256..=4096).contains(&bytes_per_sector) {
            return Err(ScanError::Corrupt(format!(
                "implausible bytes per sector: {bytes_per_sector}"
            )));
        }
        let spc_raw = le::u8_at(sector0, 13).unwrap_or(0);
        // Values above 0x80 encode 2^(256 - value) clusters (large clusters).
        let sectors_per_cluster: u32 = if spc_raw == 0 {
            return Err(ScanError::Corrupt("zero sectors per cluster".into()));
        } else if spc_raw <= 0x80 {
            spc_raw as u32
        } else {
            let exp = 256u32 - spc_raw as u32;
            if exp > 16 {
                return Err(ScanError::Corrupt("implausible cluster exponent".into()));
            }
            1u32 << exp
        };
        if !sectors_per_cluster.is_power_of_two() {
            return Err(ScanError::Corrupt(format!(
                "sectors per cluster not a power of two: {sectors_per_cluster}"
            )));
        }
        let cluster_size = bytes_per_sector as u64 * sectors_per_cluster as u64;
        let total_sectors = le::u64_at(sector0, 40).unwrap_or(0);
        if total_sectors == 0 {
            return Err(ScanError::Corrupt("zero total sectors".into()));
        }
        // NTFS keeps a backup boot sector just past the counted sectors, so
        // the volume must hold at least total_sectors of data.
        let fs_bytes = total_sectors
            .checked_mul(bytes_per_sector as u64)
            .ok_or_else(|| ScanError::Corrupt("total size overflow".into()))?;
        if fs_bytes > volume_len {
            return Err(ScanError::Corrupt(format!(
                "boot sector claims {fs_bytes} bytes but volume region has {volume_len}"
            )));
        }
        let total_clusters = total_sectors / sectors_per_cluster as u64;
        let mft_lcn = le::u64_at(sector0, 48).unwrap_or(0);
        let mftmirr_lcn = le::u64_at(sector0, 56).unwrap_or(0);
        if mft_lcn == 0 || mft_lcn >= total_clusters {
            return Err(ScanError::Corrupt(format!(
                "MFT LCN {mft_lcn} outside volume ({total_clusters} clusters)"
            )));
        }
        let file_record_size =
            size_from_clusters_field(le::i8_at(sector0, 64).unwrap_or(0), cluster_size)
                .filter(|&s| (256..=65536).contains(&s) && s.is_power_of_two())
                .ok_or_else(|| ScanError::Corrupt("implausible file record size".into()))?
                as u32;
        let index_record_size =
            size_from_clusters_field(le::i8_at(sector0, 68).unwrap_or(0), cluster_size)
                .filter(|&s| (256..=65536).contains(&s) && s.is_power_of_two())
                .unwrap_or(4096) as u32;
        let volume_serial = le::u64_at(sector0, 72).unwrap_or(0);

        Ok(NtfsBoot {
            bytes_per_sector,
            sectors_per_cluster,
            cluster_size,
            total_sectors,
            total_clusters,
            mft_lcn,
            mftmirr_lcn,
            file_record_size,
            index_record_size,
            volume_serial,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_boot() -> Vec<u8> {
        let mut s = vec![0u8; 512];
        s[3..11].copy_from_slice(b"NTFS    ");
        s[11..13].copy_from_slice(&512u16.to_le_bytes());
        s[13] = 8; // 4096-byte clusters
        s[40..48].copy_from_slice(&20480u64.to_le_bytes()); // 10 MiB
        s[48..56].copy_from_slice(&4u64.to_le_bytes()); // MFT at cluster 4
        s[56..64].copy_from_slice(&2u64.to_le_bytes());
        s[64] = 0xF6; // -10 => 1024-byte file records
        s[68] = 0xF4; // -12 => 4096-byte index records
        s[510..512].copy_from_slice(&0xAA55u16.to_le_bytes());
        s
    }

    #[test]
    fn parses_valid_boot() {
        let b = NtfsBoot::parse(&valid_boot(), 20480 * 512 + 512).unwrap();
        assert_eq!(b.cluster_size, 4096);
        assert_eq!(b.file_record_size, 1024);
        assert_eq!(b.mft_lcn, 4);
        assert_eq!(b.total_clusters, 2560);
    }

    #[test]
    fn rejects_oversized_claim() {
        assert!(NtfsBoot::parse(&valid_boot(), 1024 * 1024).is_err());
    }

    #[test]
    fn rejects_bad_oem() {
        let mut s = valid_boot();
        s[3] = b'X';
        assert!(NtfsBoot::parse(&s, u64::MAX).is_err());
    }

    #[test]
    fn rejects_mft_outside_volume() {
        let mut s = valid_boot();
        s[48..56].copy_from_slice(&99999u64.to_le_bytes());
        assert!(NtfsBoot::parse(&s, 20480 * 512 + 512).is_err());
    }
}
