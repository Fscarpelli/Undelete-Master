//! Bounded content planning and streaming extraction.

#![forbid(unsafe_code)]

mod error;
mod plan;
mod stream;

pub use error::RestoreError;
pub use plan::{
    plan_candidate, ContentPlan, ContentSegment, PartialPolicy, PlanLimits, ZeroFillReason,
    MAX_CONTENT_PLAN_SEGMENTS, MAX_STREAM_BUFFER_BYTES,
};
pub use stream::{
    stream_candidate, CancellationProbe, ProgressSink, StreamOutcome, StreamProgress,
    ZeroFilledRange, MAX_ZERO_FILLED_RANGES,
};
