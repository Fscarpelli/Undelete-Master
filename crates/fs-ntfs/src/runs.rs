/// One element of a decoded NTFS runlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunElement {
    /// Length in clusters.
    pub cluster_count: u64,
    /// Starting LCN; `None` for sparse runs.
    pub lcn: Option<u64>,
}

/// Decodes an NTFS runlist with checked arithmetic and hard bounds.
///
/// Returns the decoded runs; stops cleanly at the terminator. Malformed
/// input (overflow, negative absolute LCN, out-of-buffer header) yields an
/// error rather than a panic or bogus runs.
pub fn decode_runlist(buf: &[u8], total_clusters: u64) -> Result<Vec<RunElement>, String> {
    let mut runs = Vec::new();
    let mut pos = 0usize;
    let mut current_lcn: i128 = 0;
    let mut total_len: u64 = 0;
    const MAX_RUNS: usize = 100_000;

    loop {
        let header = *buf.get(pos).ok_or("runlist ended without terminator")?;
        if header == 0 {
            break;
        }
        pos += 1;
        let len_size = (header & 0x0F) as usize;
        let off_size = ((header >> 4) & 0x0F) as usize;
        if len_size == 0 || len_size > 8 || off_size > 8 {
            return Err(format!("invalid runlist header 0x{header:02X}"));
        }
        let len_bytes = buf
            .get(pos..pos + len_size)
            .ok_or("runlist length field out of bounds")?;
        pos += len_size;
        let mut cluster_count: u64 = 0;
        for (i, b) in len_bytes.iter().enumerate() {
            cluster_count |= (*b as u64) << (8 * i);
        }
        if cluster_count == 0 {
            return Err("zero-length run".into());
        }
        total_len = total_len
            .checked_add(cluster_count)
            .ok_or("runlist total length overflow")?;
        if total_len > total_clusters {
            return Err("runlist longer than volume".into());
        }

        if off_size == 0 {
            // Sparse run.
            runs.push(RunElement {
                cluster_count,
                lcn: None,
            });
        } else {
            let off_bytes = buf
                .get(pos..pos + off_size)
                .ok_or("runlist offset field out of bounds")?;
            pos += off_size;
            // Sign-extended little-endian relative offset.
            let mut delta: i64 = 0;
            for (i, b) in off_bytes.iter().enumerate() {
                delta |= (*b as i64) << (8 * i);
            }
            let shift = 64 - off_size * 8;
            delta = (delta << shift) >> shift;
            current_lcn += delta as i128;
            if current_lcn < 0 || current_lcn as u64 >= total_clusters {
                return Err(format!("run LCN {current_lcn} outside volume"));
            }
            let lcn = current_lcn as u64;
            if lcn
                .checked_add(cluster_count)
                .map(|end| end > total_clusters)
                .unwrap_or(true)
            {
                return Err("run extends past volume end".into());
            }
            runs.push(RunElement {
                cluster_count,
                lcn: Some(lcn),
            });
        }
        if runs.len() > MAX_RUNS {
            return Err("too many runs".into());
        }
    }
    Ok(runs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn decodes_single_run() {
        // header 0x11: 1 len byte, 1 offset byte; len=4 clusters, lcn=+16
        let buf = [0x11, 0x04, 0x10, 0x00];
        let runs = decode_runlist(&buf, 1000).unwrap();
        assert_eq!(
            runs,
            vec![RunElement {
                cluster_count: 4,
                lcn: Some(16)
            }]
        );
    }

    #[test]
    fn decodes_fragmented_with_negative_offset() {
        // run1: len=2 at lcn 100; run2: len=3 at lcn 100-60=40
        let buf = [0x11, 0x02, 0x64, 0x11, 0x03, 0xC4, 0x00];
        let runs = decode_runlist(&buf, 1000).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].lcn, Some(100));
        assert_eq!(runs[1].lcn, Some(40));
    }

    #[test]
    fn decodes_sparse_run() {
        // header 0x01: 1 len byte, 0 offset bytes => sparse
        let buf = [0x01, 0x08, 0x00];
        let runs = decode_runlist(&buf, 1000).unwrap();
        assert_eq!(runs[0].lcn, None);
        assert_eq!(runs[0].cluster_count, 8);
    }

    #[test]
    fn rejects_negative_absolute_lcn() {
        // First run with negative relative offset from 0.
        let buf = [0x11, 0x02, 0xFF, 0x00];
        assert!(decode_runlist(&buf, 1000).is_err());
    }

    #[test]
    fn rejects_run_past_volume() {
        let buf = [0x11, 0xFF, 0x10, 0x00];
        assert!(decode_runlist(&buf, 100).is_err());
    }

    #[test]
    fn rejects_missing_terminator() {
        let buf = [0x11, 0x02];
        assert!(decode_runlist(&buf, 1000).is_err());
    }

    proptest! {
        /// The decoder must never panic and never return out-of-volume runs.
        #[test]
        fn never_panics_and_stays_bounded(data in proptest::collection::vec(any::<u8>(), 0..256),
                                          clusters in 1u64..1_000_000) {
            if let Ok(runs) = decode_runlist(&data, clusters) {
                for r in runs {
                    if let Some(lcn) = r.lcn {
                        prop_assert!(lcn + r.cluster_count <= clusters);
                    }
                }
            }
        }
    }
}
