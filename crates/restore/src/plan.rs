use crate::RestoreError;
use um_core::{Candidate, CandidateId, CandidateKind, ExtentAvailability, ExtentRun};

pub const MAX_STREAM_BUFFER_BYTES: usize = 1024 * 1024;
pub const MAX_CONTENT_PLAN_SEGMENTS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartialPolicy {
    CompleteOnly,
    ZeroFillAndMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZeroFillReason {
    Sparse,
    MissingExtent,
    CurrentlyAllocated,
    OutOfVolume,
    PreviouslyReadFailed,
    Zeroed,
    UnknownAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentSegment {
    Read {
        logical_offset: u64,
        physical_offset: u64,
        len: u64,
    },
    Zero {
        logical_offset: u64,
        len: u64,
        reason: ZeroFillReason,
    },
}

impl ContentSegment {
    pub(crate) fn logical_offset(&self) -> u64 {
        match self {
            Self::Read { logical_offset, .. } | Self::Zero { logical_offset, .. } => {
                *logical_offset
            }
        }
    }

    pub(crate) fn len(&self) -> u64 {
        match self {
            Self::Read { len, .. } | Self::Zero { len, .. } => *len,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPlan {
    pub candidate_id: CandidateId,
    pub logical_size: u64,
    pub segments: Vec<ContentSegment>,
    pub expected_sha256: Option<[u8; 32]>,
    pub requires_best_effort: bool,
    pub(crate) policy: PartialPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanLimits {
    pub max_segments: usize,
}

impl Default for PlanLimits {
    fn default() -> Self {
        Self {
            max_segments: MAX_CONTENT_PLAN_SEGMENTS,
        }
    }
}

#[derive(Debug)]
struct BuiltSegment {
    segment: ContentSegment,
    read_availability: Option<ExtentAvailability>,
}

struct PlanBuilder {
    segments: Vec<BuiltSegment>,
    policy: PartialPolicy,
    max_segments: usize,
    requires_best_effort: bool,
}

impl PlanBuilder {
    fn new(policy: PartialPolicy, max_segments: usize) -> Self {
        Self {
            segments: Vec::new(),
            policy,
            max_segments,
            requires_best_effort: false,
        }
    }

    fn push_read(
        &mut self,
        logical_offset: u64,
        physical_offset: u64,
        len: u64,
        availability: ExtentAvailability,
    ) -> Result<(), RestoreError> {
        if let Some(last) = self.segments.last_mut() {
            if last.read_availability == Some(availability) {
                if let ContentSegment::Read {
                    logical_offset: last_logical,
                    physical_offset: last_physical,
                    len: last_len,
                } = &mut last.segment
                {
                    let last_logical_end = last_logical
                        .checked_add(*last_len)
                        .expect("validated logical segment");
                    let last_physical_end = last_physical
                        .checked_add(*last_len)
                        .expect("validated physical segment");
                    if last_logical_end == logical_offset && last_physical_end == physical_offset {
                        *last_len = last_len
                            .checked_add(len)
                            .expect("validated coalesced segment");
                        return Ok(());
                    }
                }
            }
        }

        self.push(BuiltSegment {
            segment: ContentSegment::Read {
                logical_offset,
                physical_offset,
                len,
            },
            read_availability: Some(availability),
        })
    }

    fn push_zero(
        &mut self,
        logical_offset: u64,
        len: u64,
        reason: ZeroFillReason,
    ) -> Result<(), RestoreError> {
        if reason != ZeroFillReason::Sparse {
            if self.policy == PartialPolicy::CompleteOnly {
                return Err(RestoreError::UnavailableContent {
                    logical_offset,
                    len,
                    reason,
                });
            }
            self.requires_best_effort = true;
        }

        if let Some(last) = self.segments.last_mut() {
            if let ContentSegment::Zero {
                logical_offset: last_logical,
                len: last_len,
                reason: last_reason,
            } = &mut last.segment
            {
                let last_end = last_logical
                    .checked_add(*last_len)
                    .expect("validated logical segment");
                if last_end == logical_offset && *last_reason == reason {
                    *last_len = last_len
                        .checked_add(len)
                        .expect("validated coalesced segment");
                    return Ok(());
                }
            }
        }

        self.push(BuiltSegment {
            segment: ContentSegment::Zero {
                logical_offset,
                len,
                reason,
            },
            read_availability: None,
        })
    }

    fn push(&mut self, segment: BuiltSegment) -> Result<(), RestoreError> {
        if self.segments.len() >= self.max_segments {
            return Err(RestoreError::SegmentLimitExceeded {
                limit: self.max_segments,
            });
        }
        self.segments.push(segment);
        Ok(())
    }

    fn finish(self) -> (Vec<ContentSegment>, bool) {
        (
            self.segments
                .into_iter()
                .map(|built| built.segment)
                .collect(),
            self.requires_best_effort,
        )
    }
}

pub fn plan_candidate(
    candidate: &Candidate,
    source_len: u64,
    expected_sha256: Option<[u8; 32]>,
    policy: PartialPolicy,
    limits: PlanLimits,
) -> Result<Option<ContentPlan>, RestoreError> {
    if candidate.kind == CandidateKind::Directory {
        return Ok(None);
    }
    if limits.max_segments == 0 || limits.max_segments > MAX_CONTENT_PLAN_SEGMENTS {
        return Err(RestoreError::InvalidPlanLimits {
            requested: limits.max_segments,
            maximum: MAX_CONTENT_PLAN_SEGMENTS,
        });
    }
    if candidate.extents.len() > MAX_CONTENT_PLAN_SEGMENTS {
        return Err(RestoreError::SegmentLimitExceeded {
            limit: limits.max_segments,
        });
    }

    let mut ordered: Vec<(usize, &ExtentRun)> = Vec::with_capacity(candidate.extents.len());
    for (index, extent) in candidate.extents.iter().enumerate() {
        if extent.len == 0 {
            return Err(RestoreError::ZeroLengthExtent { index });
        }
        let logical_end = extent.logical_offset.checked_add(extent.len).ok_or(
            RestoreError::LogicalRangeOverflow {
                logical_offset: extent.logical_offset,
                len: extent.len,
            },
        )?;
        if logical_end > candidate.size {
            return Err(RestoreError::LogicalRangeOutOfBounds {
                logical_offset: extent.logical_offset,
                len: extent.len,
                logical_size: candidate.size,
            });
        }
        ordered.push((index, extent));
    }
    ordered.sort_unstable_by_key(|(index, extent)| (extent.logical_offset, *index));

    let mut builder = PlanBuilder::new(policy, limits.max_segments);
    let mut logical_cursor = 0u64;
    for (_, extent) in ordered {
        if extent.logical_offset < logical_cursor {
            return Err(RestoreError::OverlappingExtents {
                previous_end: logical_cursor,
                next_start: extent.logical_offset,
            });
        }
        if extent.logical_offset > logical_cursor {
            builder.push_zero(
                logical_cursor,
                extent.logical_offset - logical_cursor,
                ZeroFillReason::MissingExtent,
            )?;
        }

        match readable_mapping(extent, source_len)? {
            ExtentMapping::Read { physical_offset } => builder.push_read(
                extent.logical_offset,
                physical_offset,
                extent.len,
                extent.availability,
            )?,
            ExtentMapping::Zero(reason) => {
                builder.push_zero(extent.logical_offset, extent.len, reason)?;
            }
        }
        logical_cursor = extent
            .logical_offset
            .checked_add(extent.len)
            .expect("validated logical extent");
    }

    if logical_cursor < candidate.size {
        builder.push_zero(
            logical_cursor,
            candidate.size - logical_cursor,
            ZeroFillReason::MissingExtent,
        )?;
    }

    let (segments, requires_best_effort) = builder.finish();
    Ok(Some(ContentPlan {
        candidate_id: candidate.id,
        logical_size: candidate.size,
        segments,
        expected_sha256,
        requires_best_effort,
        policy,
    }))
}

enum ExtentMapping {
    Read { physical_offset: u64 },
    Zero(ZeroFillReason),
}

fn readable_mapping(extent: &ExtentRun, source_len: u64) -> Result<ExtentMapping, RestoreError> {
    match extent.availability {
        ExtentAvailability::FreeInSnapshot | ExtentAvailability::Resident => {
            let Some(physical_offset) = extent.physical_offset else {
                return Ok(ExtentMapping::Zero(ZeroFillReason::UnknownAvailability));
            };
            let physical_end = physical_offset.checked_add(extent.len).ok_or(
                RestoreError::PhysicalRangeOverflow {
                    physical_offset,
                    len: extent.len,
                },
            )?;
            if physical_end > source_len {
                return Err(RestoreError::PhysicalRangeOutOfBounds {
                    physical_offset,
                    len: extent.len,
                    source_len,
                });
            }
            Ok(ExtentMapping::Read { physical_offset })
        }
        ExtentAvailability::Sparse => Ok(ExtentMapping::Zero(ZeroFillReason::Sparse)),
        ExtentAvailability::CurrentlyAllocated => {
            Ok(ExtentMapping::Zero(ZeroFillReason::CurrentlyAllocated))
        }
        ExtentAvailability::OutOfVolume => Ok(ExtentMapping::Zero(ZeroFillReason::OutOfVolume)),
        ExtentAvailability::ReadFailed => {
            Ok(ExtentMapping::Zero(ZeroFillReason::PreviouslyReadFailed))
        }
        ExtentAvailability::Zeroed => Ok(ExtentMapping::Zero(ZeroFillReason::Zeroed)),
        ExtentAvailability::Unknown => Ok(ExtentMapping::Zero(ZeroFillReason::UnknownAvailability)),
    }
}
