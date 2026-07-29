use crate::candidate::{normalized_logical_intervals, Candidate, ExtentAvailability};
use serde::{Deserialize, Serialize};

/// Inputs to the explainable content-recoverability score (§16.2).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RecoverabilityInputs {
    pub total_len: u64,
    pub free_bytes: u64,
    pub allocated_conflict_bytes: u64,
    pub missing_bytes: u64,
    pub read_error_bytes: u64,
    /// Cleanly readable bytes observed as zeroed.
    pub zeroed_bytes: u64,
    /// Union of all zeroed evidence, including overlaps with worse evidence.
    ///
    /// This is separate from `zeroed_bytes` so contradictory extents cannot
    /// make unreadable/conflicted bytes look readable while still preserving
    /// the mandatory majority-zero risk cap.
    #[serde(default)]
    pub zeroed_risk_bytes: u64,
    pub resident_bytes: u64,
    pub sparse_bytes: u64,
    /// The extent chain was inferred (e.g. FAT chain cleared, contiguity assumed).
    pub chain_inferred: bool,
    /// A structural validator confirmed the content.
    pub validated: bool,
    /// A validator exists for the detected type but was not run / failed to confirm.
    pub validator_available: bool,
}

/// Explainable score: final value plus the components that produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverabilityScore {
    pub value: u8,
    pub explanations: Vec<String>,
}

/// Human labels per §16.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoreLabel {
    Excellent,
    Good,
    Partial,
    Low,
    Unrecoverable,
}

pub fn score_label(value: u8) -> ScoreLabel {
    match value {
        85..=100 => ScoreLabel::Excellent,
        70..=84 => ScoreLabel::Good,
        40..=69 => ScoreLabel::Partial,
        11..=39 => ScoreLabel::Low,
        _ => ScoreLabel::Unrecoverable,
    }
}

fn availability_index(availability: ExtentAvailability) -> usize {
    match availability {
        ExtentAvailability::Sparse => 0,
        ExtentAvailability::Resident => 1,
        ExtentAvailability::FreeInSnapshot => 2,
        ExtentAvailability::Zeroed => 3,
        ExtentAvailability::Unknown => 4,
        ExtentAvailability::CurrentlyAllocated => 5,
        ExtentAvailability::ReadFailed => 6,
        ExtentAvailability::OutOfVolume => 7,
    }
}

fn classified_logical_segments(candidate: &Candidate) -> Vec<(u64, u64, [bool; 8])> {
    let mut events = Vec::with_capacity(candidate.extents.len().saturating_mul(2));
    for extent in &candidate.extents {
        let start = extent.logical_offset.min(candidate.size);
        let end = extent
            .logical_offset
            .saturating_add(extent.len)
            .min(candidate.size);
        if start < end {
            let class = availability_index(extent.availability);
            events.push((start, true, class));
            events.push((end, false, class));
        }
    }
    events.sort_unstable_by_key(|event| event.0);

    let Some(first) = events.first() else {
        return Vec::new();
    };
    let mut active = [0usize; 8];
    let mut previous = first.0;
    let mut segments = Vec::with_capacity(events.len() / 2);
    let mut event_index = 0usize;

    while event_index < events.len() {
        let position = events[event_index].0;
        if previous < position && active.iter().any(|count| *count > 0) {
            segments.push((
                previous,
                position,
                active.map(|active_count| active_count > 0),
            ));
        }
        while event_index < events.len() && events[event_index].0 == position {
            let (_, starts, class) = events[event_index];
            if starts {
                active[class] = active[class].saturating_add(1);
            } else {
                active[class] = active[class].saturating_sub(1);
            }
            event_index += 1;
        }
        previous = position;
    }

    segments
}

impl RecoverabilityInputs {
    /// Derives the byte-availability components from a candidate's extents.
    pub fn from_candidate(c: &Candidate) -> Self {
        let mut inp = RecoverabilityInputs {
            total_len: c.size,
            ..Default::default()
        };
        for (start, end, active) in classified_logical_segments(c) {
            let len = end - start;
            let has = |availability| active[availability_index(availability)];
            let out_of_volume = has(ExtentAvailability::OutOfVolume);
            let read_failed = has(ExtentAvailability::ReadFailed);
            let allocated_conflict = has(ExtentAvailability::CurrentlyAllocated);
            let unknown = has(ExtentAvailability::Unknown);
            let zeroed = has(ExtentAvailability::Zeroed);

            // Adverse evidence is cumulative: contradictory extents must never
            // erase a stricter score cap.
            if out_of_volume {
                inp.missing_bytes = inp.missing_bytes.saturating_add(len);
            }
            if read_failed {
                inp.read_error_bytes = inp.read_error_bytes.saturating_add(len);
            }
            if allocated_conflict {
                inp.allocated_conflict_bytes = inp.allocated_conflict_bytes.saturating_add(len);
            }
            if zeroed {
                inp.zeroed_risk_bytes = inp.zeroed_risk_bytes.saturating_add(len);
            }

            // Only evidence without a conflicting/unreadable/unknown class
            // contributes to clean readable-byte accounting.
            if out_of_volume || read_failed || allocated_conflict || unknown {
                continue;
            }
            if zeroed {
                inp.zeroed_bytes = inp.zeroed_bytes.saturating_add(len);
            } else if has(ExtentAvailability::FreeInSnapshot) {
                inp.free_bytes = inp.free_bytes.saturating_add(len);
            } else if has(ExtentAvailability::Resident) {
                inp.resident_bytes = inp.resident_bytes.saturating_add(len);
            } else if has(ExtentAvailability::Sparse) {
                inp.sparse_bytes = inp.sparse_bytes.saturating_add(len);
            }
        }
        let covered = normalized_logical_intervals(&c.extents, c.size)
            .into_iter()
            .fold(0u64, |total, (start, end)| {
                total.saturating_add(end - start)
            });
        if covered < c.size {
            inp.missing_bytes = inp.missing_bytes.saturating_add(c.size - covered);
        }
        inp.chain_inferred = c
            .warnings
            .iter()
            .any(|w| w.contains("inferred") || w.contains("assumed"));
        inp
    }

    /// Computes the content score honoring the mandatory caps of §16.3.
    ///
    /// The score never reaches 100 on real data: without a pre-existing
    /// reference hash the maximum is 99 ("strong evidence of complete content").
    pub fn score(&self) -> RecoverabilityScore {
        let mut expl = Vec::new();
        if self.total_len == 0 {
            // Zero-length file: trivially recoverable if metadata exists.
            return RecoverabilityScore {
                value: 99,
                explanations: vec!["zero-length file; nothing to recover beyond metadata".into()],
            };
        }

        let readable = self
            .free_bytes
            .saturating_add(self.resident_bytes)
            .saturating_add(self.sparse_bytes)
            .saturating_add(self.zeroed_bytes)
            .min(self.total_len);
        let usable = readable
            .saturating_add(self.allocated_conflict_bytes)
            .min(self.total_len);

        if usable == 0 {
            return RecoverabilityScore {
                value: 0,
                explanations: vec!["metadata only: no content bytes are available".into()],
            };
        }

        // Base score proportional to cleanly available bytes.
        let frac_ok = readable as f64 / self.total_len as f64;
        let mut score = (frac_ok * 99.0).floor() as i32;
        expl.push(format!(
            "{:.1}% of content bytes available without conflict",
            frac_ok * 100.0
        ));

        // Caps, most restrictive last-applied via min().
        let mut cap = 99;
        if !self.validated {
            if self.validator_available {
                cap = cap.min(84);
                expl.push("structural validation not confirmed: capped at 84".into());
            } else {
                cap = cap.min(84);
                expl.push("no validator available for this type: capped at 84".into());
            }
        }
        if self.chain_inferred {
            cap = cap.min(74);
            expl.push("extent chain was inferred, not proven: capped at 74".into());
        }
        if self.missing_bytes > 0 || self.read_error_bytes > 0 {
            cap = cap.min(69);
            let unavailable = self.missing_bytes.saturating_add(self.read_error_bytes);
            expl.push(format!(
                "{unavailable} bytes missing or unreadable: capped at 69"
            ));
        }
        if self.allocated_conflict_bytes > 0 {
            cap = cap.min(49);
            expl.push(format!(
                "{} bytes conflict with currently allocated data: capped at 49",
                self.allocated_conflict_bytes
            ));
        }
        let zeroed_risk_bytes = self.zeroed_risk_bytes.max(self.zeroed_bytes);
        if zeroed_risk_bytes > self.total_len / 2 {
            cap = cap.min(10);
            expl.push("majority of content observed zeroed: capped at 10".into());
        }
        if self.validated && self.missing_bytes == 0 && self.allocated_conflict_bytes == 0 {
            score = score.max(85);
            expl.push("fully readable and structurally validated: floor 85".into());
        }

        let value = score.clamp(0, cap.min(99)) as u8;
        RecoverabilityScore {
            value,
            explanations: expl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn base(total: u64) -> RecoverabilityInputs {
        RecoverabilityInputs {
            total_len: total,
            ..Default::default()
        }
    }

    #[test]
    fn fully_available_validated_is_excellent() {
        let mut i = base(1000);
        i.free_bytes = 1000;
        i.validated = true;
        let s = i.score();
        assert!((85..=99).contains(&s.value), "got {}", s.value);
    }

    #[test]
    fn fully_available_unvalidated_capped_84() {
        let mut i = base(1000);
        i.free_bytes = 1000;
        let s = i.score();
        assert!(s.value <= 84, "got {}", s.value);
        assert!(s.value >= 70, "got {}", s.value);
    }

    #[test]
    fn inferred_chain_capped_74() {
        let mut i = base(1000);
        i.free_bytes = 1000;
        i.chain_inferred = true;
        assert!(i.score().value <= 74);
    }

    #[test]
    fn missing_range_capped_69() {
        let mut i = base(1000);
        i.free_bytes = 900;
        i.missing_bytes = 100;
        i.validated = true;
        assert!(i.score().value <= 69);
    }

    #[test]
    fn conflict_capped_49() {
        let mut i = base(1000);
        i.free_bytes = 500;
        i.allocated_conflict_bytes = 500;
        assert!(i.score().value <= 49);
    }

    #[test]
    fn mostly_zeroed_capped_10() {
        let mut i = base(1000);
        i.zeroed_bytes = 900;
        i.free_bytes = 100;
        assert!(i.score().value <= 10);
    }

    #[test]
    fn metadata_only_is_zero() {
        let mut i = base(1000);
        i.missing_bytes = 1000;
        assert_eq!(i.score().value, 0);
    }

    #[test]
    fn never_100() {
        let mut i = base(8);
        i.free_bytes = 8;
        i.validated = true;
        assert!(i.score().value < 100);
    }

    #[test]
    fn core_score_overlap_001_duplicate_extents_do_not_inflate_score() {
        let duplicate = crate::ExtentRun {
            logical_offset: 0,
            physical_offset: Some(4096),
            len: 50,
            availability: ExtentAvailability::FreeInSnapshot,
        };
        let candidate = Candidate {
            id: 1,
            kind: crate::CandidateKind::File,
            method: crate::DiscoveryMethod::NtfsMetadata,
            state: crate::CandidateState::CompleteUnvalidated,
            name: "overlap.bin".into(),
            name_certain: true,
            parent_path: Vec::new(),
            metadata_confidence: crate::MetadataConfidence::High,
            size: 100,
            timestamps: crate::Timestamps::default(),
            extents: vec![duplicate.clone(), duplicate],
            record_ref: 1,
            sequence: Some(1),
            warnings: Vec::new(),
        };

        let inputs = RecoverabilityInputs::from_candidate(&candidate);

        assert_eq!(inputs.free_bytes, 50);
        assert_eq!(inputs.missing_bytes, 50);
        assert!(inputs.score().value <= 69);
    }

    #[test]
    fn core_score_overlap_conflict_002_preserves_strictest_cap() {
        let candidate = Candidate {
            id: 2,
            kind: crate::CandidateKind::File,
            method: crate::DiscoveryMethod::NtfsMetadata,
            state: crate::CandidateState::Conflicted,
            name: "conflicting-overlap.bin".into(),
            name_certain: true,
            parent_path: Vec::new(),
            metadata_confidence: crate::MetadataConfidence::High,
            size: 100,
            timestamps: crate::Timestamps::default(),
            extents: vec![
                crate::ExtentRun {
                    logical_offset: 0,
                    physical_offset: Some(4096),
                    len: 90,
                    availability: ExtentAvailability::FreeInSnapshot,
                },
                crate::ExtentRun {
                    logical_offset: 90,
                    physical_offset: Some(8192),
                    len: 10,
                    availability: ExtentAvailability::CurrentlyAllocated,
                },
                crate::ExtentRun {
                    logical_offset: 90,
                    physical_offset: Some(8192),
                    len: 10,
                    availability: ExtentAvailability::ReadFailed,
                },
            ],
            record_ref: 2,
            sequence: Some(1),
            warnings: Vec::new(),
        };

        let inputs = RecoverabilityInputs::from_candidate(&candidate);

        assert_eq!(inputs.allocated_conflict_bytes, 10);
        assert_eq!(inputs.read_error_bytes, 10);
        assert!(inputs.score().value <= 49);
    }

    #[test]
    fn core_score_overlap_zeroed_003_preserves_majority_zero_cap() {
        let candidate = Candidate {
            id: 3,
            kind: crate::CandidateKind::File,
            method: crate::DiscoveryMethod::NtfsMetadata,
            state: crate::CandidateState::ZeroedOrTrimmed,
            name: "zeroed-unknown-overlap.bin".into(),
            name_certain: true,
            parent_path: Vec::new(),
            metadata_confidence: crate::MetadataConfidence::High,
            size: 100,
            timestamps: crate::Timestamps::default(),
            extents: vec![
                crate::ExtentRun {
                    logical_offset: 0,
                    physical_offset: Some(4096),
                    len: 40,
                    availability: ExtentAvailability::FreeInSnapshot,
                },
                crate::ExtentRun {
                    logical_offset: 40,
                    physical_offset: Some(8192),
                    len: 60,
                    availability: ExtentAvailability::Zeroed,
                },
                crate::ExtentRun {
                    logical_offset: 40,
                    physical_offset: Some(8192),
                    len: 60,
                    availability: ExtentAvailability::Unknown,
                },
            ],
            record_ref: 3,
            sequence: Some(1),
            warnings: Vec::new(),
        };

        let mut inputs = RecoverabilityInputs::from_candidate(&candidate);
        inputs.validated = true;

        assert_eq!(inputs.free_bytes, 40);
        assert_eq!(inputs.zeroed_bytes, 0);
        assert_eq!(inputs.zeroed_risk_bytes, 60);
        assert_eq!(inputs.missing_bytes, 0);
        assert!(inputs.score().value <= 10);
    }

    #[test]
    fn core_score_extreme_class_totals_saturate_without_panicking() {
        let inputs = RecoverabilityInputs {
            total_len: u64::MAX,
            free_bytes: u64::MAX,
            allocated_conflict_bytes: u64::MAX,
            missing_bytes: u64::MAX,
            read_error_bytes: u64::MAX,
            zeroed_bytes: u64::MAX,
            resident_bytes: u64::MAX,
            sparse_bytes: u64::MAX,
            ..Default::default()
        };

        assert!(inputs.score().value <= 10);
    }

    proptest! {
        #[test]
        fn score_in_bounds(total in 1u64..1_000_000,
                           free in 0u64..1_000_000,
                           conflict in 0u64..1_000_000,
                           missing in 0u64..1_000_000,
                           zeroed in 0u64..1_000_000,
                           validated: bool, inferred: bool) {
            let i = RecoverabilityInputs {
                total_len: total,
                free_bytes: free.min(total),
                allocated_conflict_bytes: conflict.min(total),
                missing_bytes: missing.min(total),
                zeroed_bytes: zeroed.min(total),
                validated,
                chain_inferred: inferred,
                ..Default::default()
            };
            let s = i.score();
            prop_assert!(s.value <= 99);
            if i.allocated_conflict_bytes > 0 { prop_assert!(s.value <= 49); }
            if i.missing_bytes > 0 { prop_assert!(s.value <= 69); }
            if i.chain_inferred { prop_assert!(s.value <= 74); }
        }
    }
}
