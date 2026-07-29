//! Domain types and read-only source abstractions for Undelete Master.
//!
//! This crate is OS-independent and performs no I/O of its own. Every other
//! engine crate builds on the types defined here. The central architectural
//! invariant is that [`SourceReader`] exposes **no write operations**: scan
//! sources can only ever be read.

#![forbid(unsafe_code)]

pub mod candidate;
pub mod extract;
pub mod read;
pub mod region;
pub mod score;
pub mod source;

pub use candidate::{
    Candidate, CandidateId, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability,
    ExtentRun, MetadataConfidence, Timestamps,
};
pub use extract::{extract_candidate, Extraction};
pub use read::{ReadError, ReadOutcome};
pub use region::Region;
pub use score::{score_label, RecoverabilityInputs, RecoverabilityScore, ScoreLabel};
pub use source::{SectorLayout, SourceIdentity, SourceKind, SourceReader};
