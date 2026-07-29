use um_fs_common::{le, ScanError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatVariant {
    Fat12,
    Fat16,
    Fat32,
}

/// Validated FAT boot sector / BPB parameters and derived layout.
#[derive(Debug, Clone)]
pub struct FatBoot {
    pub variant: FatVariant,
    pub bytes_per_sector: u32,
    pub sectors_per_cluster: u32,
    pub cluster_size: u64,
    pub reserved_sectors: u32,
    pub num_fats: u32,
    pub fat_size_sectors: u32,
    pub total_sectors: u64,
    pub root_entries: u32,
    /// FAT32 only: first cluster of the root directory.
    pub root_cluster: u32,
    /// Byte offset of the first FAT.
    pub fat_offset: u64,
    /// Byte offset of the fixed root directory (FAT12/16).
    pub root_dir_offset: u64,
    pub root_dir_bytes: u64,
    /// Byte offset of cluster 2.
    pub data_offset: u64,
    /// Number of data clusters (clusters are numbered from 2).
    pub cluster_count: u64,
}

impl FatBoot {
    pub fn parse(sector0: &[u8], volume_len: u64) -> Result<Self, ScanError> {
        if sector0.len() < 512 {
            return Err(ScanError::NotRecognized("boot sector too small".into()));
        }
        if le::u16_at(sector0, 510) != Some(0xAA55) {
            return Err(ScanError::NotRecognized("missing boot signature".into()));
        }
        let bytes_per_sector = le::u16_at(sector0, 11).unwrap_or(0) as u32;
        if !bytes_per_sector.is_power_of_two() || !(512..=4096).contains(&bytes_per_sector) {
            return Err(ScanError::NotRecognized(format!(
                "implausible bytes per sector {bytes_per_sector}"
            )));
        }
        let sectors_per_cluster = le::u8_at(sector0, 13).unwrap_or(0) as u32;
        if !sectors_per_cluster.is_power_of_two() || sectors_per_cluster > 128 {
            return Err(ScanError::NotRecognized(
                "implausible sectors per cluster".into(),
            ));
        }
        let reserved_sectors = le::u16_at(sector0, 14).unwrap_or(0) as u32;
        let num_fats = le::u8_at(sector0, 16).unwrap_or(0) as u32;
        if reserved_sectors == 0 || num_fats == 0 || num_fats > 4 {
            return Err(ScanError::NotRecognized("implausible reserved/FAT count".into()));
        }
        let root_entries = le::u16_at(sector0, 17).unwrap_or(0) as u32;
        let total16 = le::u16_at(sector0, 19).unwrap_or(0) as u64;
        let fat_size16 = le::u16_at(sector0, 22).unwrap_or(0) as u32;
        let total32 = le::u32_at(sector0, 32).unwrap_or(0) as u64;
        let total_sectors = if total16 != 0 { total16 } else { total32 };
        if total_sectors == 0 {
            return Err(ScanError::NotRecognized("zero total sectors".into()));
        }
        let fs_bytes = total_sectors
            .checked_mul(bytes_per_sector as u64)
            .ok_or_else(|| ScanError::Corrupt("size overflow".into()))?;
        if fs_bytes > volume_len {
            return Err(ScanError::Corrupt(format!(
                "BPB claims {fs_bytes} bytes but region has {volume_len}"
            )));
        }
        let fat_size32 = le::u32_at(sector0, 36).unwrap_or(0);
        let fat_size_sectors = if fat_size16 != 0 { fat_size16 } else { fat_size32 };
        if fat_size_sectors == 0 {
            return Err(ScanError::NotRecognized("zero FAT size".into()));
        }

        let fat_offset = reserved_sectors as u64 * bytes_per_sector as u64;
        let fats_bytes = num_fats as u64 * fat_size_sectors as u64 * bytes_per_sector as u64;
        let root_dir_offset = fat_offset + fats_bytes;
        let root_dir_bytes = (root_entries as u64 * 32).div_ceil(bytes_per_sector as u64)
            * bytes_per_sector as u64;
        let data_offset = root_dir_offset + root_dir_bytes;
        if data_offset >= fs_bytes {
            return Err(ScanError::Corrupt("data region starts past volume end".into()));
        }
        let cluster_size = bytes_per_sector as u64 * sectors_per_cluster as u64;
        let cluster_count = (fs_bytes - data_offset) / cluster_size;
        if cluster_count == 0 {
            return Err(ScanError::Corrupt("no data clusters".into()));
        }

        let variant = if cluster_count < 4085 {
            FatVariant::Fat12
        } else if cluster_count < 65525 {
            FatVariant::Fat16
        } else {
            FatVariant::Fat32
        };
        // Consistency: FAT32 has no fixed root dir and uses fat_size32.
        let root_cluster = le::u32_at(sector0, 44).unwrap_or(0);
        if variant == FatVariant::Fat32 {
            if root_entries != 0 || fat_size16 != 0 {
                return Err(ScanError::Corrupt(
                    "FAT32 volume with FAT12/16-style root directory fields".into(),
                ));
            }
            if root_cluster < 2 || (root_cluster as u64) >= cluster_count + 2 {
                return Err(ScanError::Corrupt("FAT32 root cluster out of range".into()));
            }
        } else if root_entries == 0 {
            return Err(ScanError::Corrupt("FAT12/16 volume without root entries".into()));
        }

        Ok(FatBoot {
            variant,
            bytes_per_sector,
            sectors_per_cluster,
            cluster_size,
            reserved_sectors,
            num_fats,
            fat_size_sectors,
            total_sectors,
            root_entries,
            root_cluster,
            fat_offset,
            root_dir_offset,
            root_dir_bytes,
            data_offset,
            cluster_count,
        })
    }

    /// Byte offset of a data cluster (numbered from 2).
    pub fn cluster_offset(&self, cluster: u64) -> Option<u64> {
        if cluster < 2 || cluster >= self.cluster_count + 2 {
            return None;
        }
        Some(self.data_offset + (cluster - 2) * self.cluster_size)
    }
}
