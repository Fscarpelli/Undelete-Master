use crate::candidate::{Candidate, ExtentAvailability};
use crate::read::ReadError;
use crate::source::SourceReader;

/// Result of materializing a candidate's content from a source region.
#[derive(Debug)]
pub struct Extraction {
    /// The materialized bytes (gaps zero-filled), truncated to logical size.
    pub bytes: Vec<u8>,
    /// Logical ranges `(offset, len)` that could not be read from the source.
    pub missing_ranges: Vec<(u64, u64)>,
}

/// Materializes a candidate's bytes by walking its extents in logical order.
///
/// - Sparse extents are zero-filled by design.
/// - Extents without a physical offset (other than sparse) are reported missing.
/// - Read failures are zero-filled and reported missing.
/// - Logical ranges not covered by any extent are zero-filled and reported missing.
///
/// The `reader` must be scoped to the same region the extents refer to
/// (e.g. a `RegionReader` over the partition).
pub fn extract_candidate(
    reader: &dyn SourceReader,
    candidate: &Candidate,
) -> Result<Extraction, ReadError> {
    let size = usize::try_from(candidate.size).map_err(|_| ReadError::OutOfBounds {
        offset: 0,
        len: candidate.size,
        source_len: reader.len(),
    })?;
    let mut bytes = vec![0u8; size];
    let mut covered = vec![false; size];
    let mut missing: Vec<(u64, u64)> = Vec::new();

    for extent in &candidate.extents {
        let lo = extent.logical_offset.min(candidate.size);
        let hi = extent
            .logical_offset
            .saturating_add(extent.len)
            .min(candidate.size);
        if lo >= hi {
            continue;
        }
        let (lo_u, hi_u) = (lo as usize, hi as usize);
        match extent.availability {
            ExtentAvailability::Sparse => {
                // Logically zero: already zero-filled, counts as covered.
                covered[lo_u..hi_u].iter_mut().for_each(|c| *c = true);
            }
            _ => match extent.physical_offset {
                Some(phys) => {
                    let read_len = hi - lo;
                    let outcome = reader.read_best_effort_at(phys, &mut bytes[lo_u..hi_u]);
                    if outcome.is_complete() {
                        covered[lo_u..hi_u].iter_mut().for_each(|c| *c = true);
                    } else {
                        let mut bad = vec![false; read_len as usize];
                        for (roff, rlen) in &outcome.bad_ranges {
                            let s = (*roff).min(read_len) as usize;
                            let e = roff.saturating_add(*rlen).min(read_len) as usize;
                            bad[s..e].iter_mut().for_each(|b| *b = true);
                        }
                        for (i, is_bad) in bad.iter().enumerate() {
                            if !is_bad {
                                covered[lo_u + i] = true;
                            }
                        }
                    }
                }
                None => {
                    // No physical location and not sparse: nothing to read.
                }
            },
        }
    }

    // Collect uncovered logical ranges.
    let mut i = 0usize;
    while i < size {
        if !covered[i] {
            let start = i;
            while i < size && !covered[i] {
                i += 1;
            }
            missing.push((start as u64, (i - start) as u64));
        } else {
            i += 1;
        }
    }

    Ok(Extraction {
        bytes,
        missing_ranges: missing,
    })
}
