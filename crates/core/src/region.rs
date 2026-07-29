use serde::{Deserialize, Serialize};

/// A byte range inside a source, expressed as absolute offset and length.
///
/// All arithmetic is checked; a `Region` can never describe a range that
/// wraps around `u64::MAX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Region {
    pub offset: u64,
    pub len: u64,
}

impl Region {
    /// Creates a region, returning `None` if `offset + len` overflows.
    pub fn new(offset: u64, len: u64) -> Option<Self> {
        offset.checked_add(len)?;
        Some(Self { offset, len })
    }

    pub fn end(&self) -> u64 {
        // Safe by construction (checked in `new`).
        self.offset + self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns true when `other` lies fully inside `self`.
    pub fn contains(&self, other: &Region) -> bool {
        other.offset >= self.offset && other.end() <= self.end()
    }

    pub fn contains_offset(&self, offset: u64) -> bool {
        offset >= self.offset && offset < self.end()
    }

    /// Intersection of two regions, if any.
    pub fn intersect(&self, other: &Region) -> Option<Region> {
        let start = self.offset.max(other.offset);
        let end = self.end().min(other.end());
        if start < end {
            Some(Region {
                offset: start,
                len: end - start,
            })
        } else {
            None
        }
    }

    /// A sub-region expressed relative to this region's start.
    /// Returns `None` when the requested slice does not fit.
    pub fn sub(&self, rel_offset: u64, len: u64) -> Option<Region> {
        let abs = self.offset.checked_add(rel_offset)?;
        let candidate = Region::new(abs, len)?;
        if self.contains(&candidate) {
            Some(candidate)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn new_rejects_overflow() {
        assert!(Region::new(u64::MAX, 1).is_none());
        assert!(Region::new(u64::MAX - 10, 10).is_some());
    }

    #[test]
    fn intersect_basics() {
        let a = Region::new(0, 100).unwrap();
        let b = Region::new(50, 100).unwrap();
        assert_eq!(a.intersect(&b), Some(Region::new(50, 50).unwrap()));
        let c = Region::new(100, 1).unwrap();
        assert_eq!(a.intersect(&c), None);
    }

    proptest! {
        #[test]
        fn sub_always_contained(off in 0u64..1_000_000, len in 0u64..1_000_000,
                                s in 0u64..2_000_000, l in 0u64..2_000_000) {
            if let Some(r) = Region::new(off, len) {
                if let Some(subr) = r.sub(s, l) {
                    prop_assert!(r.contains(&subr));
                }
            }
        }
    }
}
