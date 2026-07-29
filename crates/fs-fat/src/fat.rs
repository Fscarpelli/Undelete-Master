use crate::boot::{FatBoot, FatVariant};

/// An in-memory copy of the file allocation table.
pub struct FatTable {
    variant: FatVariant,
    raw: Vec<u8>,
    cluster_count: u64,
}

/// Interpretation of one FAT entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatEntry {
    Free,
    Next(u32),
    EndOfChain,
    Bad,
    Reserved,
}

impl FatTable {
    pub fn new(variant: FatVariant, raw: Vec<u8>, cluster_count: u64) -> Self {
        Self {
            variant,
            raw,
            cluster_count,
        }
    }

    pub fn from_boot(boot: &FatBoot, raw: Vec<u8>) -> Self {
        Self::new(boot.variant, raw, boot.cluster_count)
    }

    /// Raw entry value for a cluster, `None` when out of table bounds.
    pub fn raw_entry(&self, cluster: u32) -> Option<u32> {
        let c = cluster as usize;
        match self.variant {
            FatVariant::Fat12 => {
                let off = c + c / 2; // 1.5 bytes per entry
                let lo = *self.raw.get(off)? as u32;
                let hi = *self.raw.get(off + 1)? as u32;
                let v = lo | (hi << 8);
                Some(if c % 2 == 0 { v & 0x0FFF } else { v >> 4 })
            }
            FatVariant::Fat16 => {
                let off = c * 2;
                Some(u32::from(*self.raw.get(off)?) | (u32::from(*self.raw.get(off + 1)?) << 8))
            }
            FatVariant::Fat32 => {
                let off = c * 4;
                let b = self.raw.get(off..off + 4)?;
                Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) & 0x0FFF_FFFF)
            }
        }
    }

    pub fn entry(&self, cluster: u32) -> Option<FatEntry> {
        if (cluster as u64) >= self.cluster_count + 2 {
            return None;
        }
        let v = self.raw_entry(cluster)?;
        let (bad, eoc_min) = match self.variant {
            FatVariant::Fat12 => (0x0FF7, 0x0FF8),
            FatVariant::Fat16 => (0xFFF7, 0xFFF8),
            FatVariant::Fat32 => (0x0FFF_FFF7, 0x0FFF_FFF8),
        };
        Some(if v == 0 {
            FatEntry::Free
        } else if v == bad {
            FatEntry::Bad
        } else if v >= eoc_min {
            FatEntry::EndOfChain
        } else if v == 1 {
            FatEntry::Reserved
        } else {
            FatEntry::Next(v)
        })
    }

    pub fn is_free(&self, cluster: u32) -> Option<bool> {
        self.entry(cluster).map(|e| e == FatEntry::Free)
    }

    /// Follows a cluster chain with loop protection.
    ///
    /// Returns `(clusters, complete)`: `complete` is false when the chain hit
    /// a loop, a bad/free entry or the volume bounds before end-of-chain.
    pub fn chain(&self, start: u32, max_clusters: u64) -> (Vec<u32>, bool) {
        let mut out = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut current = start;
        let limit = max_clusters.min(self.cluster_count);
        for _ in 0..limit {
            if current < 2 || (current as u64) >= self.cluster_count + 2 {
                return (out, false);
            }
            if !visited.insert(current) {
                return (out, false); // loop
            }
            out.push(current);
            match self.entry(current) {
                Some(FatEntry::Next(n)) => current = n,
                Some(FatEntry::EndOfChain) => return (out, true),
                _ => return (out, false),
            }
        }
        (out, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fat32_raw(entries: &[u32]) -> Vec<u8> {
        entries.iter().flat_map(|e| e.to_le_bytes()).collect()
    }

    #[test]
    fn fat32_chain_follows_to_eoc() {
        // clusters: 2 -> 3 -> 5 -> EOC
        let raw = fat32_raw(&[0x0FFFFFF8, 0x0FFFFFFF, 3, 5, 0, 0x0FFFFFFF, 0]);
        let t = FatTable::new(FatVariant::Fat32, raw, 5);
        let (chain, complete) = t.chain(2, 100);
        assert_eq!(chain, vec![2, 3, 5]);
        assert!(complete);
    }

    #[test]
    fn detects_loop() {
        // 2 -> 3 -> 2 loop
        let raw = fat32_raw(&[0x0FFFFFF8, 0x0FFFFFFF, 3, 2, 0]);
        let t = FatTable::new(FatVariant::Fat32, raw, 3);
        let (chain, complete) = t.chain(2, 100);
        assert_eq!(chain, vec![2, 3]);
        assert!(!complete);
    }

    #[test]
    fn fat12_packed_entries() {
        // FAT12: entries [0]=0xFF8, [1]=0xFFF, [2]=0x003, [3]=0xFFF
        // packed: bytes = F8 FF FF | 03 F0 FF
        let raw = vec![0xF8, 0xFF, 0xFF, 0x03, 0xF0, 0xFF];
        let t = FatTable::new(FatVariant::Fat12, raw, 2);
        assert_eq!(t.raw_entry(2), Some(0x003));
        assert_eq!(t.raw_entry(3), Some(0xFFF));
        let (chain, complete) = t.chain(2, 10);
        assert_eq!(chain, vec![2, 3]);
        assert!(complete);
    }

    #[test]
    fn free_and_bad_entries() {
        let raw = fat32_raw(&[0x0FFFFFF8, 0x0FFFFFFF, 0, 0x0FFFFFF7]);
        let t = FatTable::new(FatVariant::Fat32, raw, 2);
        assert_eq!(t.entry(2), Some(FatEntry::Free));
        assert_eq!(t.entry(3), Some(FatEntry::Bad));
    }
}
