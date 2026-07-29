//! Deterministic FAT16/FAT32 volume image builder.
//!
//! Geometry (both variants use 512-byte sectors, 1 sector per cluster):
//! - FAT16: 4 reserved sectors, 2 FATs, 512 root entries, 8000 clusters.
//! - FAT32: 32 reserved sectors, 2 FATs, root at cluster 2, 66000 clusters.

use crate::manifest::{ExpectedCandidate, FixtureManifest};
use crate::sha256_hex;

const SECTOR: usize = 512;
const CLUSTER: usize = 512;

/// Fixed DOS timestamp: 2024-01-15 12:00:00.
const DOS_DATE: u16 = ((2024 - 1980) << 9) | (1 << 5) | 15;
const DOS_TIME: u16 = 12 << 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatKind {
    Fat16,
    Fat32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeParent {
    Root,
    Node(usize),
}

#[derive(Debug, Clone, Default)]
pub struct FatFileOptions {
    /// Keep the FAT chain intact after deletion (tests "chain retained").
    pub keep_chain: bool,
    /// Write only the 8.3 entry, no LFN (the deleted first char is then lost).
    pub no_lfn: bool,
    /// Fragment the file into two runs with a gap.
    pub fragmented: bool,
    /// Logical ranges overwritten post-delete (clusters reallocated+scrambled).
    pub overwrite_ranges: Vec<(u64, u64)>,
}

struct Node {
    name: String,
    parent: NodeParent,
    is_dir: bool,
    deleted: bool,
    content: Vec<u8>,
    options: FatFileOptions,
}

pub struct FatImageBuilder {
    fixture_id: String,
    kind: FatKind,
    nodes: Vec<Node>,
}

struct Geometry {
    reserved: u32,
    num_fats: u32,
    fat_sectors: u32,
    root_entries: u32,
    clusters: u32,
    total_sectors: u32,
}

impl FatImageBuilder {
    pub fn new(fixture_id: &str, kind: FatKind) -> Self {
        Self {
            fixture_id: fixture_id.to_string(),
            kind,
            nodes: Vec::new(),
        }
    }

    pub fn add_dir(&mut self, parent: NodeParent, name: &str, deleted: bool) -> usize {
        self.nodes.push(Node {
            name: name.to_string(),
            parent,
            is_dir: true,
            deleted,
            content: Vec::new(),
            options: FatFileOptions::default(),
        });
        self.nodes.len() - 1
    }

    pub fn add_file(
        &mut self,
        parent: NodeParent,
        name: &str,
        content: Vec<u8>,
        deleted: bool,
        options: FatFileOptions,
    ) -> usize {
        self.nodes.push(Node {
            name: name.to_string(),
            parent,
            is_dir: false,
            deleted,
            content,
            options,
        });
        self.nodes.len() - 1
    }

    fn geometry(&self) -> Geometry {
        match self.kind {
            FatKind::Fat16 => {
                let clusters = 8000u32;
                let fat_sectors = ((clusters + 2) * 2).div_ceil(SECTOR as u32);
                Geometry {
                    reserved: 4,
                    num_fats: 2,
                    fat_sectors,
                    root_entries: 512,
                    clusters,
                    total_sectors: 4
                        + 2 * fat_sectors
                        + (512u32 * 32).div_ceil(SECTOR as u32)
                        + clusters,
                }
            }
            FatKind::Fat32 => {
                let clusters = 66_000u32;
                let fat_sectors = ((clusters + 2) * 4).div_ceil(SECTOR as u32);
                Geometry {
                    reserved: 32,
                    num_fats: 2,
                    fat_sectors,
                    root_entries: 0,
                    clusters,
                    total_sectors: 32 + 2 * fat_sectors + clusters,
                }
            }
        }
    }

    fn parent_path(&self, node: &Node) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = node.parent;
        while let NodeParent::Node(idx) = current {
            parts.push(self.nodes[idx].name.clone());
            current = self.nodes[idx].parent;
        }
        parts.reverse();
        parts
    }

    pub fn build(&self) -> (Vec<u8>, FixtureManifest) {
        let geo = self.geometry();
        let image_len = geo.total_sectors as usize * SECTOR;
        let mut image = vec![0u8; image_len];

        let fat_offset = geo.reserved as usize * SECTOR;
        let root_dir_offset =
            fat_offset + (geo.num_fats * geo.fat_sectors) as usize * SECTOR;
        let root_dir_bytes = geo.root_entries as usize * 32;
        let data_offset = root_dir_offset + root_dir_bytes.div_ceil(SECTOR) * SECTOR;

        // FAT as u32 values (written packed per variant at the end).
        let mut fat = vec![0u32; (geo.clusters + 2) as usize];
        fat[0] = 0x0FFF_FFF8;
        fat[1] = 0x0FFF_FFFF;
        let eoc: u32 = match self.kind {
            FatKind::Fat16 => 0xFFFF,
            FatKind::Fat32 => 0x0FFF_FFFF,
        };

        let mut next_free: u32 = match self.kind {
            FatKind::Fat16 => 2,
            FatKind::Fat32 => 3, // cluster 2 = root
        };
        if self.kind == FatKind::Fat32 {
            fat[2] = eoc; // root directory: single cluster
        }
        let cluster_off =
            |c: u32| -> usize { data_offset + (c as usize - 2) * CLUSTER };

        // --- allocate content clusters ---
        struct Placement {
            clusters: Vec<u32>,
        }
        let mut placements: Vec<Placement> = Vec::new();
        for node in &self.nodes {
            if node.is_dir {
                // One cluster per directory to hold its entries.
                let c = next_free;
                next_free += 1;
                placements.push(Placement { clusters: vec![c] });
                continue;
            }
            let needed = node.content.len().div_ceil(CLUSTER) as u32;
            let mut clusters = Vec::new();
            if needed > 0 {
                if node.options.fragmented && needed >= 2 {
                    let first = needed / 2;
                    for i in 0..first {
                        clusters.push(next_free + i);
                    }
                    next_free += first + 3; // gap
                    for i in 0..(needed - first) {
                        clusters.push(next_free + i);
                    }
                    next_free += needed - first;
                } else {
                    for i in 0..needed {
                        clusters.push(next_free + i);
                    }
                    next_free += needed;
                }
            }
            assert!(next_free < geo.clusters + 2, "fixture volume out of space");
            // Write content.
            let mut written = 0usize;
            for &c in &clusters {
                let off = cluster_off(c);
                let take = CLUSTER.min(node.content.len() - written);
                image[off..off + take].copy_from_slice(&node.content[written..written + take]);
                written += take;
            }
            placements.push(Placement { clusters });
        }

        // --- FAT chains ---
        for (node, placement) in self.nodes.iter().zip(&placements) {
            let keep = !node.deleted || node.options.keep_chain;
            if !keep {
                continue;
            }
            let cl = &placement.clusters;
            for (i, &c) in cl.iter().enumerate() {
                fat[c as usize] = if i + 1 < cl.len() { cl[i + 1] } else { eoc };
            }
        }

        // --- post-delete overwrites ---
        for (node, placement) in self.nodes.iter().zip(&placements) {
            for (ov_off, ov_len) in &node.options.overwrite_ranges {
                let first = (ov_off / CLUSTER as u64) as usize;
                let last = ((ov_off + ov_len - 1) / CLUSTER as u64) as usize;
                for (i, &c) in placement.clusters.iter().enumerate() {
                    if i >= first && i <= last {
                        let off = cluster_off(c);
                        image[off..off + CLUSTER].fill(0xCC);
                        fat[c as usize] = eoc; // now allocated to "other data"
                    }
                }
            }
        }

        // --- directory streams ---
        // children[parent_index_or_root] -> node indices
        let mut root_children = Vec::new();
        let mut children: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate() {
            match node.parent {
                NodeParent::Root => root_children.push(i),
                NodeParent::Node(p) => children[p].push(i),
            }
        }

        let build_dir_stream = |child_idxs: &[usize], placements: &[Placement]| -> Vec<u8> {
            let mut stream = Vec::new();
            for (ordinal, &ci) in child_idxs.iter().enumerate() {
                let node = &self.nodes[ci];
                let first_cluster = if node.is_dir {
                    placements[ci].clusters[0]
                } else {
                    placements[ci].clusters.first().copied().unwrap_or(0)
                };
                let entries = encode_entry(
                    &node.name,
                    ordinal as u32,
                    node.is_dir,
                    node.deleted,
                    first_cluster,
                    node.content.len() as u32,
                    node.options.no_lfn,
                );
                stream.extend(entries);
            }
            stream
        };

        // Root directory.
        let root_stream = build_dir_stream(&root_children, &placements);
        match self.kind {
            FatKind::Fat16 => {
                assert!(root_stream.len() <= root_dir_bytes, "root directory full");
                image[root_dir_offset..root_dir_offset + root_stream.len()]
                    .copy_from_slice(&root_stream);
            }
            FatKind::Fat32 => {
                assert!(root_stream.len() <= CLUSTER, "root directory full");
                let off = cluster_off(2);
                image[off..off + root_stream.len()].copy_from_slice(&root_stream);
            }
        }
        // Subdirectories.
        for (i, node) in self.nodes.iter().enumerate() {
            if !node.is_dir {
                continue;
            }
            let stream = build_dir_stream(&children[i], &placements);
            assert!(stream.len() <= CLUSTER, "fixture subdirectory full");
            let off = cluster_off(placements[i].clusters[0]);
            image[off..off + stream.len()].copy_from_slice(&stream);
        }

        // --- boot sector ---
        let boot = self.build_boot(&geo);
        image[..SECTOR].copy_from_slice(&boot);

        // --- write FAT copies ---
        let mut fat_bytes = vec![0u8; geo.fat_sectors as usize * SECTOR];
        match self.kind {
            FatKind::Fat16 => {
                for (i, &v) in fat.iter().enumerate() {
                    fat_bytes[i * 2..i * 2 + 2].copy_from_slice(&(v as u16).to_le_bytes());
                }
            }
            FatKind::Fat32 => {
                for (i, &v) in fat.iter().enumerate() {
                    fat_bytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
        for copy in 0..geo.num_fats as usize {
            let off = fat_offset + copy * geo.fat_sectors as usize * SECTOR;
            image[off..off + fat_bytes.len()].copy_from_slice(&fat_bytes);
        }

        // --- manifest ---
        let expected_candidates = self
            .nodes
            .iter()
            .filter(|n| n.deleted)
            .map(|n| ExpectedCandidate {
                name: n.name.clone(),
                parent_path: self.parent_path(n),
                is_directory: n.is_dir,
                size: n.content.len() as u64,
                content_sha256: if n.is_dir {
                    String::new()
                } else {
                    sha256_hex(&n.content)
                },
                fully_recoverable: n.options.overwrite_ranges.is_empty(),
                damaged_ranges: n.options.overwrite_ranges.clone(),
            })
            .collect();

        (
            image,
            FixtureManifest {
                fixture_id: self.fixture_id.clone(),
                filesystem: match self.kind {
                    FatKind::Fat16 => "FAT16".into(),
                    FatKind::Fat32 => "FAT32".into(),
                },
                sector_size: SECTOR as u32,
                cluster_size: CLUSTER as u32,
                expected_candidates,
            },
        )
    }

    fn build_boot(&self, geo: &Geometry) -> [u8; SECTOR] {
        let mut s = [0u8; SECTOR];
        s[0] = 0xEB;
        s[1] = 0x58;
        s[2] = 0x90;
        s[3..11].copy_from_slice(b"MSWIN4.1");
        s[11..13].copy_from_slice(&(SECTOR as u16).to_le_bytes());
        s[13] = 1; // sectors per cluster
        s[14..16].copy_from_slice(&(geo.reserved as u16).to_le_bytes());
        s[16] = geo.num_fats as u8;
        s[17..19].copy_from_slice(&(geo.root_entries as u16).to_le_bytes());
        s[21] = 0xF8;
        match self.kind {
            FatKind::Fat16 => {
                if geo.total_sectors <= 0xFFFF {
                    s[19..21].copy_from_slice(&(geo.total_sectors as u16).to_le_bytes());
                } else {
                    s[32..36].copy_from_slice(&geo.total_sectors.to_le_bytes());
                }
                s[22..24].copy_from_slice(&(geo.fat_sectors as u16).to_le_bytes());
            }
            FatKind::Fat32 => {
                s[32..36].copy_from_slice(&geo.total_sectors.to_le_bytes());
                s[36..40].copy_from_slice(&geo.fat_sectors.to_le_bytes());
                s[44..48].copy_from_slice(&2u32.to_le_bytes()); // root cluster
            }
        }
        s[510..512].copy_from_slice(&0xAA55u16.to_le_bytes());
        s
    }
}

fn lfn_checksum(short: &[u8; 11]) -> u8 {
    let mut sum: u8 = 0;
    for b in short {
        sum = sum.rotate_right(1).wrapping_add(*b);
    }
    sum
}

/// Encodes one directory entry (LFN chain + 8.3), applying deletion markers.
#[allow(clippy::too_many_arguments)]
fn encode_entry(
    name: &str,
    ordinal: u32,
    is_dir: bool,
    deleted: bool,
    first_cluster: u32,
    size: u32,
    no_lfn: bool,
) -> Vec<u8> {
    // Deterministic unique 8.3 name per directory slot.
    let ext: String = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_uppercase())
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(3)
        .collect();
    let mut short = [b' '; 11];
    let base = if no_lfn {
        // 8.3-compatible name required for the no-LFN case.
        name.split('.').next().unwrap_or(name).to_ascii_uppercase()
    } else {
        format!("F{ordinal:05}~1")
    };
    for (i, b) in base.bytes().take(8).enumerate() {
        short[i] = b;
    }
    for (i, b) in ext.bytes().take(3).enumerate() {
        short[8 + i] = b;
    }

    let mut out = Vec::new();
    if !no_lfn {
        let ck = lfn_checksum(&short);
        let units: Vec<u16> = name.encode_utf16().collect();
        let parts = units.chunks(13).collect::<Vec<_>>();
        for (i, part) in parts.iter().enumerate().rev() {
            let seq_no = (i + 1) as u8;
            let seq = if i + 1 == parts.len() {
                seq_no | 0x40
            } else {
                seq_no
            };
            let mut e = [0u8; 32];
            e[0] = if deleted { 0xE5 } else { seq };
            e[11] = 0x0F;
            e[13] = ck;
            let mut padded = part.to_vec();
            if padded.len() < 13 {
                padded.push(0);
                while padded.len() < 13 {
                    padded.push(0xFFFF);
                }
            }
            let ranges = [(1usize, 11usize), (14, 26), (28, 32)];
            let mut u = 0;
            for (start, end) in ranges {
                for pos in (start..end).step_by(2) {
                    e[pos..pos + 2].copy_from_slice(&padded[u].to_le_bytes());
                    u += 1;
                }
            }
            out.extend_from_slice(&e);
        }
    }

    let mut e = [0u8; 32];
    e[0..11].copy_from_slice(&short);
    if deleted {
        e[0] = 0xE5;
    }
    e[11] = if is_dir { 0x10 } else { 0x20 };
    e[14..16].copy_from_slice(&DOS_TIME.to_le_bytes());
    e[16..18].copy_from_slice(&DOS_DATE.to_le_bytes());
    e[22..24].copy_from_slice(&DOS_TIME.to_le_bytes());
    e[24..26].copy_from_slice(&DOS_DATE.to_le_bytes());
    e[20..22].copy_from_slice(&((first_cluster >> 16) as u16).to_le_bytes());
    e[26..28].copy_from_slice(&((first_cluster & 0xFFFF) as u16).to_le_bytes());
    if !is_dir {
        e[28..32].copy_from_slice(&size.to_le_bytes());
    }
    out.extend_from_slice(&e);
    out
}
