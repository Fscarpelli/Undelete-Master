use crate::{
    ContentPlan, ContentSegment, PartialPolicy, RestoreError, ZeroFillReason,
    MAX_CONTENT_PLAN_SEGMENTS, MAX_STREAM_BUFFER_BYTES,
};
use sha2::{Digest, Sha256};
use std::io::Write;
use um_core::{ReadOutcome, SourceReader};

pub const MAX_ZERO_FILLED_RANGES: usize = 65_536;

pub trait CancellationProbe: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

pub trait ProgressSink {
    fn advanced(&mut self, progress: StreamProgress);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamProgress {
    pub bytes_written: u64,
    pub logical_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroFilledRange {
    pub logical_offset: u64,
    pub len: u64,
    pub reason: ZeroFillReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamOutcome {
    pub bytes_written: u64,
    pub sha256: [u8; 32],
    pub zero_filled_ranges: Vec<ZeroFilledRange>,
    pub requires_best_effort: bool,
}

pub fn stream_candidate<W: Write>(
    source: &dyn SourceReader,
    plan: &ContentPlan,
    output: &mut W,
    scratch: &mut [u8],
    cancel: &dyn CancellationProbe,
    progress: &mut dyn ProgressSink,
) -> Result<StreamOutcome, RestoreError> {
    preflight(source.len(), plan, scratch.len())?;
    validate_planned_zero_ranges(plan)?;

    let mut zero_filled_ranges = Vec::new();
    let mut requires_best_effort = plan.requires_best_effort;
    let mut hasher = Sha256::new();
    let mut bytes_written = 0u64;

    for segment in &plan.segments {
        match segment {
            ContentSegment::Read {
                logical_offset,
                physical_offset,
                len,
            } => {
                let mut segment_position = 0u64;
                while segment_position < *len {
                    check_cancel(cancel, bytes_written)?;
                    let remaining = len - segment_position;
                    let chunk_len = usize::try_from(remaining.min(scratch.len() as u64))
                        .expect("chunk length is bounded by scratch");
                    let chunk_logical = logical_offset
                        .checked_add(segment_position)
                        .expect("plan logical range passed preflight");
                    let chunk_physical = physical_offset
                        .checked_add(segment_position)
                        .expect("plan physical range passed preflight");

                    let outcome =
                        source.read_best_effort_at(chunk_physical, &mut scratch[..chunk_len]);
                    let bad_ranges = normalize_bad_ranges(chunk_len, outcome)?;
                    if let Some(&(bad_start, bad_end)) = bad_ranges.first() {
                        let bad_offset = chunk_logical
                            .checked_add(bad_start)
                            .expect("bad range lies within a validated chunk");
                        let bad_len = bad_end - bad_start;
                        if plan.policy == PartialPolicy::CompleteOnly {
                            return Err(RestoreError::ReadFailure {
                                logical_offset: bad_offset,
                                len: bad_len,
                            });
                        }

                        for (bad_start, bad_end) in bad_ranges {
                            let start =
                                usize::try_from(bad_start).expect("bad range lies within scratch");
                            let end =
                                usize::try_from(bad_end).expect("bad range lies within scratch");
                            scratch[start..end].fill(0);
                            append_zero_range(
                                &mut zero_filled_ranges,
                                ZeroFilledRange {
                                    logical_offset: chunk_logical
                                        .checked_add(bad_start)
                                        .expect("bad range lies within logical chunk"),
                                    len: bad_end - bad_start,
                                    reason: ZeroFillReason::PreviouslyReadFailed,
                                },
                            )?;
                        }
                        requires_best_effort = true;
                    }

                    write_committed(
                        output,
                        &scratch[..chunk_len],
                        &mut hasher,
                        cancel,
                        progress,
                        plan.logical_size,
                        &mut bytes_written,
                    )?;
                    segment_position = segment_position
                        .checked_add(chunk_len as u64)
                        .expect("chunk lies within validated segment");
                }
            }
            ContentSegment::Zero {
                logical_offset,
                len,
                reason,
            } => {
                append_zero_range(
                    &mut zero_filled_ranges,
                    ZeroFilledRange {
                        logical_offset: *logical_offset,
                        len: *len,
                        reason: *reason,
                    },
                )?;
                let mut segment_position = 0u64;
                while segment_position < *len {
                    check_cancel(cancel, bytes_written)?;
                    let remaining = len - segment_position;
                    let chunk_len = usize::try_from(remaining.min(scratch.len() as u64))
                        .expect("chunk length is bounded by scratch");
                    scratch[..chunk_len].fill(0);
                    write_committed(
                        output,
                        &scratch[..chunk_len],
                        &mut hasher,
                        cancel,
                        progress,
                        plan.logical_size,
                        &mut bytes_written,
                    )?;
                    segment_position = segment_position
                        .checked_add(chunk_len as u64)
                        .expect("chunk lies within validated segment");
                }
            }
        }
    }

    if bytes_written != plan.logical_size {
        return Err(RestoreError::OutputLengthMismatch {
            bytes_written,
            logical_size: plan.logical_size,
        });
    }
    let actual: [u8; 32] = hasher.finalize().into();
    if let Some(expected) = plan.expected_sha256 {
        if expected != actual {
            return Err(RestoreError::HashMismatch { expected, actual });
        }
    }

    Ok(StreamOutcome {
        bytes_written,
        sha256: actual,
        zero_filled_ranges,
        requires_best_effort,
    })
}

fn preflight(source_len: u64, plan: &ContentPlan, scratch_len: usize) -> Result<(), RestoreError> {
    if scratch_len == 0 || scratch_len > MAX_STREAM_BUFFER_BYTES {
        return Err(RestoreError::InvalidScratchLength {
            len: scratch_len,
            maximum: MAX_STREAM_BUFFER_BYTES,
        });
    }
    if plan.segments.len() > MAX_CONTENT_PLAN_SEGMENTS {
        return Err(RestoreError::SegmentLimitExceeded {
            limit: MAX_CONTENT_PLAN_SEGMENTS,
        });
    }

    let mut expected_offset = 0u64;
    let mut expected_best_effort = false;
    for (index, segment) in plan.segments.iter().enumerate() {
        let logical_offset = segment.logical_offset();
        let len = segment.len();
        if len == 0 {
            return Err(RestoreError::InvalidPlanSegment { index });
        }
        if logical_offset != expected_offset {
            return Err(RestoreError::InvalidPlanCoverage {
                index,
                expected_offset,
                actual_offset: logical_offset,
            });
        }
        let logical_end =
            logical_offset
                .checked_add(len)
                .ok_or(RestoreError::LogicalRangeOverflow {
                    logical_offset,
                    len,
                })?;
        if logical_end > plan.logical_size {
            return Err(RestoreError::LogicalRangeOutOfBounds {
                logical_offset,
                len,
                logical_size: plan.logical_size,
            });
        }

        match segment {
            ContentSegment::Read {
                physical_offset, ..
            } => {
                let physical_end = physical_offset.checked_add(len).ok_or(
                    RestoreError::PhysicalRangeOverflow {
                        physical_offset: *physical_offset,
                        len,
                    },
                )?;
                if physical_end > source_len {
                    return Err(RestoreError::PhysicalRangeOutOfBounds {
                        physical_offset: *physical_offset,
                        len,
                        source_len,
                    });
                }
            }
            ContentSegment::Zero { reason, .. } => {
                if *reason != ZeroFillReason::Sparse {
                    if plan.policy == PartialPolicy::CompleteOnly {
                        return Err(RestoreError::UnavailableContent {
                            logical_offset,
                            len,
                            reason: *reason,
                        });
                    }
                    expected_best_effort = true;
                }
            }
        }
        expected_offset = logical_end;
    }

    if expected_offset != plan.logical_size {
        return Err(RestoreError::InvalidPlanCoverage {
            index: plan.segments.len(),
            expected_offset,
            actual_offset: plan.logical_size,
        });
    }
    if expected_best_effort != plan.requires_best_effort {
        return Err(RestoreError::InvalidPlanBestEffortFlag {
            expected: expected_best_effort,
            actual: plan.requires_best_effort,
        });
    }
    Ok(())
}

fn planned_zero_ranges(plan: &ContentPlan) -> Result<Vec<ZeroFilledRange>, RestoreError> {
    let mut ranges = Vec::new();
    for segment in &plan.segments {
        if let ContentSegment::Zero {
            logical_offset,
            len,
            reason,
        } = segment
        {
            append_zero_range(
                &mut ranges,
                ZeroFilledRange {
                    logical_offset: *logical_offset,
                    len: *len,
                    reason: *reason,
                },
            )?;
        }
    }
    Ok(ranges)
}

fn validate_planned_zero_ranges(plan: &ContentPlan) -> Result<(), RestoreError> {
    drop(planned_zero_ranges(plan)?);
    Ok(())
}

fn normalize_bad_ranges(
    requested_len: usize,
    outcome: ReadOutcome,
) -> Result<Vec<(u64, u64)>, RestoreError> {
    if outcome.bad_ranges.len() > requested_len {
        return Err(RestoreError::InvalidReadOutcome { requested_len });
    }

    let mut ranges = outcome.bad_ranges;
    for &(offset, len) in &ranges {
        let Some(end) = offset.checked_add(len) else {
            return Err(RestoreError::InvalidReadOutcome { requested_len });
        };
        if len == 0 || end > requested_len as u64 {
            return Err(RestoreError::InvalidReadOutcome { requested_len });
        }
    }
    ranges.sort_unstable_by_key(|(offset, len)| (*offset, *len));

    let mut merged_len = 0usize;
    for index in 0..ranges.len() {
        let (start, len) = ranges[index];
        let end = start
            .checked_add(len)
            .ok_or(RestoreError::InvalidReadOutcome { requested_len })?;
        if merged_len > 0 {
            let (last_start, last_end) = ranges[merged_len - 1];
            if start <= last_end {
                ranges[merged_len - 1] = (last_start, last_end.max(end));
                continue;
            }
        }
        ranges[merged_len] = (start, end);
        merged_len += 1;
    }
    ranges.truncate(merged_len);

    let bad_bytes = ranges
        .iter()
        .try_fold(0u64, |total, (start, end)| total.checked_add(end - start));
    let Some(bad_bytes) = bad_bytes else {
        return Err(RestoreError::InvalidReadOutcome { requested_len });
    };
    let expected_valid = requested_len as u64 - bad_bytes;
    if outcome.bytes_valid != expected_valid {
        return Err(RestoreError::InvalidReadOutcome { requested_len });
    }
    Ok(ranges)
}

fn append_zero_range(
    ranges: &mut Vec<ZeroFilledRange>,
    next: ZeroFilledRange,
) -> Result<(), RestoreError> {
    if let Some(last) = ranges.last_mut() {
        let last_end = last
            .logical_offset
            .checked_add(last.len)
            .expect("validated zero-fill evidence");
        if last_end == next.logical_offset && last.reason == next.reason {
            last.len = last
                .len
                .checked_add(next.len)
                .expect("validated adjacent zero-fill evidence");
            return Ok(());
        }
    }
    if ranges.len() >= MAX_ZERO_FILLED_RANGES {
        return Err(RestoreError::ZeroFillRangeLimitExceeded {
            limit: MAX_ZERO_FILLED_RANGES,
        });
    }
    ranges.push(next);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_committed<W: Write>(
    output: &mut W,
    bytes: &[u8],
    hasher: &mut Sha256,
    cancel: &dyn CancellationProbe,
    progress: &mut dyn ProgressSink,
    logical_size: u64,
    bytes_written: &mut u64,
) -> Result<(), RestoreError> {
    let mut position = 0usize;
    while position < bytes.len() {
        check_cancel(cancel, *bytes_written)?;
        let written = match output.write(&bytes[position..]) {
            Ok(0) => {
                return Err(RestoreError::OutputWrite {
                    kind: std::io::ErrorKind::WriteZero,
                    message: "writer accepted zero bytes".into(),
                    bytes_written: *bytes_written,
                });
            }
            Ok(written) if written <= bytes.len() - position => written,
            Ok(_) => {
                return Err(RestoreError::OutputWrite {
                    kind: std::io::ErrorKind::InvalidData,
                    message: "writer reported more bytes than requested".into(),
                    bytes_written: *bytes_written,
                });
            }
            Err(error) => {
                return Err(RestoreError::OutputWrite {
                    kind: error.kind(),
                    message: error.to_string(),
                    bytes_written: *bytes_written,
                });
            }
        };
        hasher.update(&bytes[position..position + written]);
        position += written;
        *bytes_written = bytes_written.checked_add(written as u64).ok_or(
            RestoreError::OutputLengthMismatch {
                bytes_written: *bytes_written,
                logical_size,
            },
        )?;
        progress.advanced(StreamProgress {
            bytes_written: *bytes_written,
            logical_size,
        });
    }
    Ok(())
}

fn check_cancel(cancel: &dyn CancellationProbe, bytes_written: u64) -> Result<(), RestoreError> {
    if cancel.is_cancelled() {
        Err(RestoreError::Cancelled { bytes_written })
    } else {
        Ok(())
    }
}
