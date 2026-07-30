//! Deterministic minimal-but-valid NTFS volume image builder.
//!
//! Default geometry: 512-byte sectors, 4096-byte clusters, 1024 clusters
//! (4 MiB), 64 MFT records of 1024 bytes at LCN 4, `$Bitmap` data at LCN 20
//! and user data from LCN 24 upward. Tests that exercise scanner coverage may
//! request a larger, still deterministic MFT layout.

use crate::manifest::{ExpectedCandidate, FixtureManifest};
use crate::sha256_hex;

const SECTOR: usize = 512;
const CLUSTER: usize = 4096;
const DEFAULT_TOTAL_CLUSTERS: u64 = 1024;
const MFT_LCN: u64 = 4;
const DEFAULT_MFT_RECORDS: u64 = 64;
const RECORD_SIZE: usize = 1024;
const ROOT_RECORD: u64 = 5;
const DEFAULT_FIRST_USER_RECORD: u64 = 16;

/// Fixed FILETIME for deterministic fixtures: 2024-01-15 12:00:00 UTC.
const FIXED_UNIX_MS: i64 = 1_705_320_000_000;
fn fixed_filetime() -> u64 {
    ((FIXED_UNIX_MS + 11_644_473_600_000) * 10_000) as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeParent {
    Root,
    Node(usize),
}

#[derive(Debug, Clone, Default)]
pub struct FileOptions {
    /// Force resident/non-resident storage; `None` = auto by size.
    pub force_resident: Option<bool>,
    /// Store the content in two fragmented runs with a gap between them.
    pub fragmented: bool,
    /// Logical byte ranges overwritten after deletion (clusters become
    /// allocated to "other data" and their bytes are scrambled).
    pub overwrite_ranges: Vec<(u64, u64)>,
}

struct Node {
    name: String,
    parent: NodeParent,
    is_dir: bool,
    deleted: bool,
    content: Vec<u8>,
    options: FileOptions,
}

/// Builder for a synthetic NTFS volume.
pub struct NtfsImageBuilder {
    fixture_id: String,
    nodes: Vec<Node>,
    orphan_contents: Vec<Vec<u8>>,
    mft_records: u64,
    first_user_record: u64,
}

impl NtfsImageBuilder {
    pub fn new(fixture_id: &str) -> Self {
        Self {
            fixture_id: fixture_id.to_string(),
            nodes: Vec::new(),
            orphan_contents: Vec::new(),
            mft_records: DEFAULT_MFT_RECORDS,
            first_user_record: DEFAULT_FIRST_USER_RECORD,
        }
    }

    /// Places fixture nodes at a caller-selected MFT record boundary.
    ///
    /// This is intended for deterministic scanner coverage tests. It does not
    /// model MFT growth and never touches a host disk.
    pub fn with_mft_layout(mut self, mft_records: u64, first_user_record: u64) -> Self {
        assert!(
            first_user_record >= DEFAULT_FIRST_USER_RECORD,
            "user records overlap reserved NTFS records"
        );
        assert!(
            first_user_record < mft_records,
            "first user record must be inside the MFT"
        );
        self.mft_records = mft_records;
        self.first_user_record = first_user_record;
        self
    }

    pub fn add_dir(&mut self, parent: NodeParent, name: &str, deleted: bool) -> usize {
        self.nodes.push(Node {
            name: name.to_string(),
            parent,
            is_dir: true,
            deleted,
            content: Vec::new(),
            options: FileOptions::default(),
        });
        self.nodes.len() - 1
    }

    pub fn add_file(
        &mut self,
        parent: NodeParent,
        name: &str,
        content: Vec<u8>,
        deleted: bool,
        options: FileOptions,
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

    /// Places unreferenced bytes in free clusters without creating an MFT
    /// record. This models content that only a signature carver can discover.
    pub fn add_orphan_content(&mut self, content: Vec<u8>) {
        assert!(
            !content.is_empty(),
            "orphan fixture content cannot be empty"
        );
        self.orphan_contents.push(content);
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

    /// Assembles the image and the truth manifest.
    pub fn build(&self) -> (Vec<u8>, FixtureManifest) {
        assert!(
            self.nodes.len() as u64 <= self.mft_records - self.first_user_record,
            "too many fixture nodes"
        );
        let mft_bytes = self
            .mft_records
            .checked_mul(RECORD_SIZE as u64)
            .expect("fixture MFT size overflow");
        let mft_clusters = mft_bytes.div_ceil(CLUSTER as u64);
        let bitmap_lcn = MFT_LCN + mft_clusters;
        let first_data_lcn = bitmap_lcn + 4;
        let total_clusters = DEFAULT_TOTAL_CLUSTERS.max(first_data_lcn + 256);
        let total_sectors = total_clusters * (CLUSTER / SECTOR) as u64;
        let image_len = total_clusters as usize * CLUSTER + SECTOR; // + backup boot
        let mut image = vec![0u8; image_len];

        // --- boot sector (and backup copy at the end) ---
        let boot = build_boot_sector(total_sectors);
        image[..SECTOR].copy_from_slice(&boot);
        let backup_off = total_clusters as usize * CLUSTER;
        image[backup_off..backup_off + SECTOR].copy_from_slice(&boot);

        // --- allocation bitmap ---
        let mut bitmap = vec![0u8; total_clusters.div_ceil(8) as usize];
        let mut mark = |cluster: u64| {
            bitmap[(cluster / 8) as usize] |= 1 << (cluster % 8);
        };
        for c in 0..first_data_lcn {
            mark(c);
        }

        // --- allocate content clusters and write file data ---
        let mut next_lcn = first_data_lcn;
        struct Placement {
            runs: Vec<(u64, u64)>, // (lcn, cluster_count)
            resident: bool,
        }
        let mut placements: Vec<Placement> = Vec::new();
        for node in &self.nodes {
            if node.is_dir {
                placements.push(Placement {
                    runs: Vec::new(),
                    resident: false,
                });
                continue;
            }
            let resident = node
                .options
                .force_resident
                .unwrap_or(node.content.len() <= 700);
            if resident {
                assert!(node.content.len() <= 700, "resident content too large");
                placements.push(Placement {
                    runs: Vec::new(),
                    resident: true,
                });
                continue;
            }
            let clusters_needed = (node.content.len() as u64).div_ceil(CLUSTER as u64).max(1);
            let runs: Vec<(u64, u64)> = if node.options.fragmented && clusters_needed >= 2 {
                let first = clusters_needed / 2;
                let second = clusters_needed - first;
                let r1 = (next_lcn, first);
                next_lcn += first + 2; // 2-cluster gap
                let r2 = (next_lcn, second);
                next_lcn += second;
                vec![r1, r2]
            } else {
                let r = (next_lcn, clusters_needed);
                next_lcn += clusters_needed;
                vec![r]
            };
            assert!(next_lcn < total_clusters, "fixture volume out of space");
            // Write content into the image following the runs.
            let mut written = 0usize;
            for (lcn, count) in &runs {
                let mut off = *lcn as usize * CLUSTER;
                let mut room = *count as usize * CLUSTER;
                while written < node.content.len() && room > 0 {
                    let take = room.min(node.content.len() - written);
                    image[off..off + take].copy_from_slice(&node.content[written..written + take]);
                    written += take;
                    off += take;
                    room -= take;
                }
            }
            // Allocation: in-use files keep clusters allocated; deleted files
            // leave them free.
            if !node.deleted {
                for (lcn, count) in &runs {
                    for c in *lcn..*lcn + *count {
                        mark(c);
                    }
                }
            }
            placements.push(Placement { runs, resident });
        }

        // Unreferenced carving fixtures occupy physically free clusters and
        // deliberately have no MFT metadata.
        for content in &self.orphan_contents {
            let clusters_needed = (content.len() as u64).div_ceil(CLUSTER as u64);
            let start_lcn = next_lcn;
            next_lcn = next_lcn
                .checked_add(clusters_needed)
                .expect("orphan fixture placement overflow");
            assert!(next_lcn < total_clusters, "fixture volume out of space");
            let offset = start_lcn as usize * CLUSTER;
            image[offset..offset + content.len()].copy_from_slice(content);
        }

        // Post-delete overwrites: scramble bytes and mark clusters allocated.
        for (node, placement) in self.nodes.iter().zip(&placements) {
            for (ov_off, ov_len) in &node.options.overwrite_ranges {
                let first_cluster = ov_off / CLUSTER as u64;
                let last_cluster = (ov_off + ov_len - 1) / CLUSTER as u64;
                let mut logical_cluster = 0u64;
                for (lcn, count) in &placement.runs {
                    for i in 0..*count {
                        if (first_cluster..=last_cluster).contains(&logical_cluster) {
                            let abs = (*lcn + i) as usize * CLUSTER;
                            image[abs..abs + CLUSTER].fill(0xCC);
                            bitmap[((*lcn + i) / 8) as usize] |= 1 << ((*lcn + i) % 8);
                        }
                        logical_cluster += 1;
                    }
                }
            }
        }

        // Write the bitmap data cluster.
        let bm_off = bitmap_lcn as usize * CLUSTER;
        image[bm_off..bm_off + bitmap.len()].copy_from_slice(&bitmap);

        // --- MFT records ---
        let mft_off = MFT_LCN as usize * CLUSTER;
        let mut write_record = |record_no: u64, rec: [u8; RECORD_SIZE]| {
            let off = mft_off + record_no as usize * RECORD_SIZE;
            image[off..off + RECORD_SIZE].copy_from_slice(&rec);
        };

        // Record 0: $MFT.
        write_record(
            0,
            build_record(
                1,
                true,
                false,
                &[
                    attr_std_info(),
                    attr_file_name(ROOT_RECORD, 5, "$MFT", false, mft_bytes),
                    attr_data_nonresident(&[(MFT_LCN, mft_clusters)], mft_bytes),
                ],
            ),
        );
        // Records 1-4: minimal in-use system records.
        for (no, name) in [
            (1u64, "$MFTMirr"),
            (2, "$LogFile"),
            (3, "$Volume"),
            (4, "$AttrDef"),
        ] {
            write_record(
                no,
                build_record(
                    1,
                    true,
                    false,
                    &[
                        attr_std_info(),
                        attr_file_name(ROOT_RECORD, 5, name, false, 0),
                        attr_data_resident(&[]),
                    ],
                ),
            );
        }
        // Record 5: root directory.
        write_record(
            5,
            build_record(
                5,
                true,
                true,
                &[
                    attr_std_info(),
                    attr_file_name(ROOT_RECORD, 5, ".", true, 0),
                ],
            ),
        );
        // Record 6: $Bitmap.
        write_record(
            6,
            build_record(
                1,
                true,
                false,
                &[
                    attr_std_info(),
                    attr_file_name(ROOT_RECORD, 5, "$Bitmap", false, bitmap.len() as u64),
                    attr_data_nonresident(
                        &[(bitmap_lcn, bitmap.len().div_ceil(CLUSTER) as u64)],
                        bitmap.len() as u64,
                    ),
                ],
            ),
        );

        // User records.
        for (idx, (node, placement)) in self.nodes.iter().zip(&placements).enumerate() {
            let record_no = self.first_user_record + idx as u64;
            let (parent_record, parent_seq_at_creation) = match node.parent {
                NodeParent::Root => (ROOT_RECORD, 5u16),
                NodeParent::Node(p) => (self.first_user_record + p as u64, 1u16),
            };
            // Sequence: 1 while alive; bumped to 2 when the record is freed.
            let seq = if node.deleted { 2 } else { 1 };
            let mut attrs = vec![
                attr_std_info(),
                attr_file_name(
                    parent_record,
                    parent_seq_at_creation,
                    &node.name,
                    node.is_dir,
                    node.content.len() as u64,
                ),
            ];
            if !node.is_dir {
                if placement.resident {
                    attrs.push(attr_data_resident(&node.content));
                } else {
                    attrs.push(attr_data_nonresident(
                        &placement.runs,
                        node.content.len() as u64,
                    ));
                }
            }
            write_record(
                record_no,
                build_record(seq, !node.deleted, node.is_dir, &attrs),
            );
        }

        // --- truth manifest ---
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
                filesystem: "NTFS".into(),
                sector_size: SECTOR as u32,
                cluster_size: CLUSTER as u32,
                expected_candidates,
            },
        )
    }
}

fn build_boot_sector(total_sectors: u64) -> [u8; SECTOR] {
    let mut s = [0u8; SECTOR];
    s[0] = 0xEB;
    s[1] = 0x52;
    s[2] = 0x90;
    s[3..11].copy_from_slice(b"NTFS    ");
    s[11..13].copy_from_slice(&(SECTOR as u16).to_le_bytes());
    s[13] = (CLUSTER / SECTOR) as u8;
    s[21] = 0xF8; // media descriptor
    s[40..48].copy_from_slice(&total_sectors.to_le_bytes());
    s[48..56].copy_from_slice(&MFT_LCN.to_le_bytes());
    s[56..64].copy_from_slice(&2u64.to_le_bytes()); // $MFTMirr LCN (unused)
    s[64] = 0xF6; // -10 => 1024-byte file records
    s[68] = 0xF4; // -12 => 4096-byte index records
    s[72..80].copy_from_slice(&0xDEAD_BEEF_CAFE_F00Du64.to_le_bytes());
    s[510..512].copy_from_slice(&0xAA55u16.to_le_bytes());
    s
}

/// Builds a 1024-byte FILE record with proper USA fixups.
fn build_record(seq: u16, in_use: bool, is_dir: bool, attrs: &[Vec<u8>]) -> [u8; RECORD_SIZE] {
    let mut r = [0u8; RECORD_SIZE];
    r[0..4].copy_from_slice(b"FILE");
    r[4..6].copy_from_slice(&48u16.to_le_bytes()); // USA offset
    r[6..8].copy_from_slice(&3u16.to_le_bytes()); // USA count (USN + 2 sectors)
    r[16..18].copy_from_slice(&seq.to_le_bytes());
    r[18..20].copy_from_slice(&1u16.to_le_bytes()); // hard links
    r[20..22].copy_from_slice(&56u16.to_le_bytes()); // attrs offset
    let mut flags = 0u16;
    if in_use {
        flags |= 0x1;
    }
    if is_dir {
        flags |= 0x2;
    }
    r[22..24].copy_from_slice(&flags.to_le_bytes());
    r[28..32].copy_from_slice(&(RECORD_SIZE as u32).to_le_bytes()); // allocated

    let mut pos = 56usize;
    for attr in attrs {
        assert!(pos + attr.len() + 8 <= RECORD_SIZE, "record overflow");
        r[pos..pos + attr.len()].copy_from_slice(attr);
        pos += attr.len();
    }
    r[pos..pos + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    pos += 8;
    r[24..28].copy_from_slice(&(pos as u32).to_le_bytes()); // bytes used

    // Fixups: stash original sector-end bytes in the USA, then write the USN.
    let usn = seq.max(1).to_le_bytes();
    r[48..50].copy_from_slice(&usn);
    for i in 1..3usize {
        let end = i * SECTOR;
        let orig = [r[end - 2], r[end - 1]];
        r[48 + i * 2..50 + i * 2].copy_from_slice(&orig);
        r[end - 2..end].copy_from_slice(&usn);
    }
    r
}

fn align8(v: usize) -> usize {
    v.div_ceil(8) * 8
}

/// Encodes a resident attribute.
fn resident_attr(type_id: u32, value: &[u8]) -> Vec<u8> {
    let total = align8(24 + value.len());
    let mut a = vec![0u8; total];
    a[0..4].copy_from_slice(&type_id.to_le_bytes());
    a[4..8].copy_from_slice(&(total as u32).to_le_bytes());
    a[8] = 0; // resident
    a[16..20].copy_from_slice(&(value.len() as u32).to_le_bytes());
    a[20..22].copy_from_slice(&24u16.to_le_bytes());
    a[24..24 + value.len()].copy_from_slice(value);
    a
}

fn attr_std_info() -> Vec<u8> {
    let mut v = vec![0u8; 48];
    let ft = fixed_filetime().to_le_bytes();
    v[0..8].copy_from_slice(&ft); // created
    v[8..16].copy_from_slice(&ft); // modified
    v[16..24].copy_from_slice(&ft); // MFT modified
    v[24..32].copy_from_slice(&ft); // accessed
    v[32..36].copy_from_slice(&0x20u32.to_le_bytes()); // FILE_ATTRIBUTE_ARCHIVE
    resident_attr(0x10, &v)
}

fn attr_file_name(
    parent_record: u64,
    parent_seq: u16,
    name: &str,
    is_dir: bool,
    logical_size: u64,
) -> Vec<u8> {
    let units: Vec<u16> = name.encode_utf16().collect();
    assert!(units.len() <= 255, "name too long");
    let mut v = vec![0u8; 66 + units.len() * 2];
    let parent_ref = (parent_record & 0x0000_FFFF_FFFF_FFFF) | ((parent_seq as u64) << 48);
    v[0..8].copy_from_slice(&parent_ref.to_le_bytes());
    let ft = fixed_filetime().to_le_bytes();
    v[8..16].copy_from_slice(&ft);
    v[16..24].copy_from_slice(&ft);
    v[24..32].copy_from_slice(&ft);
    v[32..40].copy_from_slice(&ft);
    let alloc = logical_size.div_ceil(CLUSTER as u64) * CLUSTER as u64;
    v[40..48].copy_from_slice(&alloc.to_le_bytes());
    v[48..56].copy_from_slice(&logical_size.to_le_bytes());
    let file_flags: u32 = if is_dir { 0x1000_0000 } else { 0x20 };
    v[56..60].copy_from_slice(&file_flags.to_le_bytes());
    v[64] = units.len() as u8;
    v[65] = 3; // Win32 & DOS namespace
    for (i, u) in units.iter().enumerate() {
        v[66 + i * 2..68 + i * 2].copy_from_slice(&u.to_le_bytes());
    }
    resident_attr(0x30, &v)
}

fn attr_data_resident(content: &[u8]) -> Vec<u8> {
    resident_attr(0x80, content)
}

/// Encodes a runlist for `(lcn, cluster_count)` runs.
fn encode_runlist(runs: &[(u64, u64)]) -> Vec<u8> {
    fn min_len_bytes(v: u64) -> usize {
        for n in 1..=8 {
            if v < (1u64 << (8 * n)) {
                return n;
            }
        }
        8
    }
    fn min_off_bytes(v: i64) -> usize {
        for n in 1..=8usize {
            let shift = 64 - n * 8;
            if ((v << shift) >> shift) == v {
                return n;
            }
        }
        8
    }
    let mut out = Vec::new();
    let mut prev: i64 = 0;
    for (lcn, count) in runs {
        let delta = *lcn as i64 - prev;
        let lb = min_len_bytes(*count);
        let ob = min_off_bytes(delta);
        out.push(((ob as u8) << 4) | lb as u8);
        out.extend_from_slice(&count.to_le_bytes()[..lb]);
        out.extend_from_slice(&delta.to_le_bytes()[..ob]);
        prev = *lcn as i64;
    }
    out.push(0);
    out
}

fn attr_data_nonresident(runs: &[(u64, u64)], data_size: u64) -> Vec<u8> {
    let runlist = encode_runlist(runs);
    let total = align8(64 + runlist.len());
    let mut a = vec![0u8; total];
    a[0..4].copy_from_slice(&0x80u32.to_le_bytes());
    a[4..8].copy_from_slice(&(total as u32).to_le_bytes());
    a[8] = 1; // non-resident
    let total_clusters: u64 = runs.iter().map(|(_, c)| c).sum();
    let last_vcn = total_clusters.saturating_sub(1);
    a[16..24].copy_from_slice(&0u64.to_le_bytes()); // starting VCN
    a[24..32].copy_from_slice(&last_vcn.to_le_bytes());
    a[32..34].copy_from_slice(&64u16.to_le_bytes()); // runlist offset
    let alloc = total_clusters * CLUSTER as u64;
    a[40..48].copy_from_slice(&alloc.to_le_bytes());
    a[48..56].copy_from_slice(&data_size.to_le_bytes());
    a[56..64].copy_from_slice(&data_size.to_le_bytes()); // initialized
    a[64..64 + runlist.len()].copy_from_slice(&runlist);
    a
}
