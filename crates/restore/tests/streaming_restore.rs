use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use um_core::{
    Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
    MetadataConfidence, ReadError, ReadOutcome, SectorLayout, SourceIdentity, SourceKind,
    SourceReader, Timestamps,
};
use um_restore::{
    plan_candidate, stream_candidate, CancellationProbe, ContentSegment, PartialPolicy, PlanLimits,
    ProgressSink, RestoreError, StreamProgress, ZeroFillReason, ZeroFilledRange,
    MAX_STREAM_BUFFER_BYTES, MAX_ZERO_FILLED_RANGES,
};

const SOURCE_LEN: u64 = 64 * 1024;

fn candidate(size: u64, extents: Vec<ExtentRun>) -> Candidate {
    Candidate {
        id: 42,
        kind: CandidateKind::File,
        method: DiscoveryMethod::NtfsMetadata,
        state: CandidateState::CompleteUnvalidated,
        name: "fixture.bin".into(),
        name_certain: true,
        parent_path: vec!["deleted".into()],
        metadata_confidence: MetadataConfidence::High,
        size,
        timestamps: Timestamps::default(),
        extents,
        record_ref: 7,
        sequence: Some(1),
        warnings: Vec::new(),
    }
}

fn extent(
    logical_offset: u64,
    physical_offset: Option<u64>,
    len: u64,
    availability: ExtentAvailability,
) -> ExtentRun {
    ExtentRun {
        logical_offset,
        physical_offset,
        len,
        availability,
    }
}

fn plan(
    candidate: &Candidate,
    policy: PartialPolicy,
) -> Result<Option<um_restore::ContentPlan>, RestoreError> {
    plan_candidate(candidate, SOURCE_LEN, None, policy, PlanLimits::default())
}

#[test]
fn plan_contiguous_fragmented_and_resident_ranges_are_exact() {
    let contiguous = candidate(
        8,
        vec![extent(0, Some(100), 8, ExtentAvailability::FreeInSnapshot)],
    );
    let contiguous_plan = plan(&contiguous, PartialPolicy::CompleteOnly)
        .unwrap()
        .unwrap();
    assert_eq!(
        contiguous_plan.segments,
        vec![ContentSegment::Read {
            logical_offset: 0,
            physical_offset: 100,
            len: 8,
        }]
    );
    assert!(!contiguous_plan.requires_best_effort);

    let fragmented_unordered = candidate(
        12,
        vec![
            extent(8, Some(800), 4, ExtentAvailability::FreeInSnapshot),
            extent(0, Some(200), 4, ExtentAvailability::FreeInSnapshot),
            extent(4, Some(500), 4, ExtentAvailability::Resident),
        ],
    );
    let fragmented_plan = plan(&fragmented_unordered, PartialPolicy::CompleteOnly)
        .unwrap()
        .unwrap();
    assert_eq!(
        fragmented_plan.segments,
        vec![
            ContentSegment::Read {
                logical_offset: 0,
                physical_offset: 200,
                len: 4,
            },
            ContentSegment::Read {
                logical_offset: 4,
                physical_offset: 500,
                len: 4,
            },
            ContentSegment::Read {
                logical_offset: 8,
                physical_offset: 800,
                len: 4,
            },
        ]
    );
}

#[test]
fn plan_sparse_ranges_are_intentional_zeros_not_partial() {
    let sparse = candidate(
        12,
        vec![
            extent(0, Some(100), 4, ExtentAvailability::FreeInSnapshot),
            extent(4, None, 4, ExtentAvailability::Sparse),
            extent(8, Some(108), 4, ExtentAvailability::FreeInSnapshot),
        ],
    );

    let content_plan = plan(&sparse, PartialPolicy::CompleteOnly).unwrap().unwrap();
    assert_eq!(
        content_plan.segments,
        vec![
            ContentSegment::Read {
                logical_offset: 0,
                physical_offset: 100,
                len: 4,
            },
            ContentSegment::Zero {
                logical_offset: 4,
                len: 4,
                reason: ZeroFillReason::Sparse,
            },
            ContentSegment::Read {
                logical_offset: 8,
                physical_offset: 108,
                len: 4,
            },
        ]
    );
    assert!(!content_plan.requires_best_effort);
}

#[test]
fn plan_gaps_require_explicit_zero_fill_policy() {
    let gapped = candidate(
        12,
        vec![
            extent(0, Some(100), 4, ExtentAvailability::FreeInSnapshot),
            extent(8, Some(108), 4, ExtentAvailability::FreeInSnapshot),
        ],
    );

    assert!(matches!(
        plan(&gapped, PartialPolicy::CompleteOnly),
        Err(RestoreError::UnavailableContent {
            logical_offset: 4,
            len: 4,
            reason: ZeroFillReason::MissingExtent,
        })
    ));

    let best_effort = plan(&gapped, PartialPolicy::ZeroFillAndMap)
        .unwrap()
        .unwrap();
    assert_eq!(
        best_effort.segments,
        vec![
            ContentSegment::Read {
                logical_offset: 0,
                physical_offset: 100,
                len: 4,
            },
            ContentSegment::Zero {
                logical_offset: 4,
                len: 4,
                reason: ZeroFillReason::MissingExtent,
            },
            ContentSegment::Read {
                logical_offset: 8,
                physical_offset: 108,
                len: 4,
            },
        ]
    );
    assert!(best_effort.requires_best_effort);
}

#[test]
fn plan_unavailable_ranges_preserve_the_exact_reason() {
    let cases = [
        (
            ExtentAvailability::CurrentlyAllocated,
            Some(200),
            ZeroFillReason::CurrentlyAllocated,
        ),
        (
            ExtentAvailability::OutOfVolume,
            Some(SOURCE_LEN + 1),
            ZeroFillReason::OutOfVolume,
        ),
        (
            ExtentAvailability::ReadFailed,
            Some(300),
            ZeroFillReason::PreviouslyReadFailed,
        ),
        (
            ExtentAvailability::Zeroed,
            Some(400),
            ZeroFillReason::Zeroed,
        ),
        (
            ExtentAvailability::Unknown,
            None,
            ZeroFillReason::UnknownAvailability,
        ),
    ];

    for (availability, physical_offset, reason) in cases {
        let unavailable = candidate(4, vec![extent(0, physical_offset, 4, availability)]);
        assert!(matches!(
            plan(&unavailable, PartialPolicy::CompleteOnly),
            Err(RestoreError::UnavailableContent {
                logical_offset: 0,
                len: 4,
                reason: actual,
            }) if actual == reason
        ));

        let best_effort = plan(&unavailable, PartialPolicy::ZeroFillAndMap)
            .unwrap()
            .unwrap();
        assert_eq!(
            best_effort.segments,
            vec![ContentSegment::Zero {
                logical_offset: 0,
                len: 4,
                reason,
            }]
        );
        assert!(best_effort.requires_best_effort);
    }
}

#[test]
fn plan_rejects_duplicate_and_overlapping_extents() {
    let first = extent(0, Some(100), 8, ExtentAvailability::FreeInSnapshot);
    let duplicate = candidate(8, vec![first.clone(), first.clone()]);
    assert!(matches!(
        plan(&duplicate, PartialPolicy::ZeroFillAndMap),
        Err(RestoreError::OverlappingExtents {
            previous_end: 8,
            next_start: 0,
        })
    ));

    let overlap = candidate(
        12,
        vec![
            first,
            extent(4, Some(500), 8, ExtentAvailability::FreeInSnapshot),
        ],
    );
    assert!(matches!(
        plan(&overlap, PartialPolicy::ZeroFillAndMap),
        Err(RestoreError::OverlappingExtents {
            previous_end: 8,
            next_start: 4,
        })
    ));
}

#[test]
fn plan_rejects_overflow_invalid_coverage_and_out_of_source_reads() {
    let logical_overflow = candidate(
        u64::MAX,
        vec![extent(
            u64::MAX - 1,
            Some(0),
            2,
            ExtentAvailability::FreeInSnapshot,
        )],
    );
    assert!(matches!(
        plan(&logical_overflow, PartialPolicy::ZeroFillAndMap),
        Err(RestoreError::LogicalRangeOverflow { .. })
    ));

    let beyond_logical_size = candidate(
        4,
        vec![extent(2, Some(0), 4, ExtentAvailability::FreeInSnapshot)],
    );
    assert!(matches!(
        plan(&beyond_logical_size, PartialPolicy::ZeroFillAndMap),
        Err(RestoreError::LogicalRangeOutOfBounds {
            logical_offset: 2,
            len: 4,
            logical_size: 4,
        })
    ));

    let physical_overflow = candidate(
        2,
        vec![extent(
            0,
            Some(u64::MAX),
            2,
            ExtentAvailability::FreeInSnapshot,
        )],
    );
    assert!(matches!(
        plan(&physical_overflow, PartialPolicy::CompleteOnly),
        Err(RestoreError::PhysicalRangeOverflow { .. })
    ));

    let outside_source = candidate(
        8,
        vec![extent(
            0,
            Some(SOURCE_LEN - 4),
            8,
            ExtentAvailability::FreeInSnapshot,
        )],
    );
    assert!(matches!(
        plan(&outside_source, PartialPolicy::CompleteOnly),
        Err(RestoreError::PhysicalRangeOutOfBounds {
            physical_offset,
            len: 8,
            source_len: SOURCE_LEN,
        }) if physical_offset == SOURCE_LEN - 4
    ));
}

#[test]
fn plan_rejects_zero_length_and_excessive_segments() {
    let zero_length = candidate(
        1,
        vec![extent(0, Some(100), 0, ExtentAvailability::FreeInSnapshot)],
    );
    assert!(matches!(
        plan(&zero_length, PartialPolicy::ZeroFillAndMap),
        Err(RestoreError::ZeroLengthExtent { index: 0 })
    ));

    let fragmented = candidate(
        3,
        vec![
            extent(0, Some(100), 1, ExtentAvailability::FreeInSnapshot),
            extent(1, None, 1, ExtentAvailability::Sparse),
            extent(2, Some(102), 1, ExtentAvailability::FreeInSnapshot),
        ],
    );
    assert!(matches!(
        plan_candidate(
            &fragmented,
            SOURCE_LEN,
            None,
            PartialPolicy::CompleteOnly,
            PlanLimits { max_segments: 2 },
        ),
        Err(RestoreError::SegmentLimitExceeded { limit: 2 })
    ));
}

#[test]
fn plan_coalesces_only_semantically_compatible_ranges() {
    let runs = candidate(
        16,
        vec![
            extent(0, Some(100), 4, ExtentAvailability::FreeInSnapshot),
            extent(4, Some(104), 4, ExtentAvailability::FreeInSnapshot),
            extent(8, None, 4, ExtentAvailability::Sparse),
            extent(12, None, 4, ExtentAvailability::Sparse),
        ],
    );
    let content_plan = plan(&runs, PartialPolicy::CompleteOnly).unwrap().unwrap();
    assert_eq!(
        content_plan.segments,
        vec![
            ContentSegment::Read {
                logical_offset: 0,
                physical_offset: 100,
                len: 8,
            },
            ContentSegment::Zero {
                logical_offset: 8,
                len: 8,
                reason: ZeroFillReason::Sparse,
            },
        ]
    );
}

#[test]
fn plan_directories_have_no_content_plan() {
    let mut directory = candidate(0, Vec::new());
    directory.kind = CandidateKind::Directory;

    assert_eq!(
        plan(&directory, PartialPolicy::ZeroFillAndMap).unwrap(),
        None
    );
}

const ABC_SHA256: [u8; 32] = [
    0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
    0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
];

#[derive(Debug, Clone, Copy)]
struct InjectedFailure {
    offset: u64,
    len: u64,
}

struct ObservedReader {
    data: Vec<u8>,
    identity: SourceIdentity,
    failure: Option<InjectedFailure>,
    reads: AtomicUsize,
    max_requested: AtomicUsize,
}

impl ObservedReader {
    fn new(data: Vec<u8>) -> Self {
        let len = data.len() as u64;
        Self {
            data,
            identity: SourceIdentity {
                id: "restore-test-source".into(),
                kind: SourceKind::ImageFile,
                label: "synthetic".into(),
                size: len,
            },
            failure: None,
            reads: AtomicUsize::new(0),
            max_requested: AtomicUsize::new(0),
        }
    }

    fn with_failure(mut self, offset: u64, len: u64) -> Self {
        self.failure = Some(InjectedFailure { offset, len });
        self
    }

    fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    fn max_requested(&self) -> usize {
        self.max_requested.load(Ordering::SeqCst)
    }
}

impl SourceReader for ObservedReader {
    fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn sector_layout(&self) -> SectorLayout {
        SectorLayout::DEFAULT_512
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        let end = offset
            .checked_add(buffer.len() as u64)
            .ok_or(ReadError::OutOfBounds {
                offset,
                len: buffer.len() as u64,
                source_len: self.len(),
            })?;
        if end > self.len() {
            return Err(ReadError::OutOfBounds {
                offset,
                len: buffer.len() as u64,
                source_len: self.len(),
            });
        }
        buffer.copy_from_slice(&self.data[offset as usize..end as usize]);
        Ok(())
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.max_requested.fetch_max(buffer.len(), Ordering::SeqCst);
        if self.read_exact_at(offset, buffer).is_err() {
            buffer.fill(0);
            return ReadOutcome {
                bytes_valid: 0,
                bad_ranges: vec![(0, buffer.len() as u64)],
            };
        }

        let Some(failure) = self.failure else {
            return ReadOutcome::complete(buffer.len() as u64);
        };
        let request_end = offset + buffer.len() as u64;
        let failure_end = failure.offset.saturating_add(failure.len);
        let bad_start = offset.max(failure.offset);
        let bad_end = request_end.min(failure_end);
        if bad_start >= bad_end {
            return ReadOutcome::complete(buffer.len() as u64);
        }

        let relative_start = (bad_start - offset) as usize;
        let relative_end = (bad_end - offset) as usize;
        buffer[relative_start..relative_end].fill(0);
        let bad_len = (relative_end - relative_start) as u64;
        ReadOutcome {
            bytes_valid: buffer.len() as u64 - bad_len,
            bad_ranges: vec![(relative_start as u64, bad_len)],
        }
    }
}

struct ScriptedOutcomeReader {
    inner: ObservedReader,
    outcome: ReadOutcome,
}

impl ScriptedOutcomeReader {
    fn new(data: Vec<u8>, outcome: ReadOutcome) -> Self {
        Self {
            inner: ObservedReader::new(data),
            outcome,
        }
    }
}

impl SourceReader for ScriptedOutcomeReader {
    fn identity(&self) -> &SourceIdentity {
        self.inner.identity()
    }

    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn sector_layout(&self) -> SectorLayout {
        self.inner.sector_layout()
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        self.inner.read_exact_at(offset, buffer)
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        self.inner.reads.fetch_add(1, Ordering::SeqCst);
        self.inner
            .max_requested
            .fetch_max(buffer.len(), Ordering::SeqCst);
        self.inner.read_exact_at(offset, buffer).unwrap();
        self.outcome.clone()
    }
}

struct NeverCancelled;

impl CancellationProbe for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct AtomicCancellation {
    cancelled: Arc<AtomicBool>,
}

impl CancellationProbe for AtomicCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct RecordingProgress {
    updates: Vec<StreamProgress>,
}

impl ProgressSink for RecordingProgress {
    fn advanced(&mut self, progress: StreamProgress) {
        self.updates.push(progress);
    }
}

struct CancellingProgress {
    updates: Vec<StreamProgress>,
    cancelled: Arc<AtomicBool>,
}

impl ProgressSink for CancellingProgress {
    fn advanced(&mut self, progress: StreamProgress) {
        self.updates.push(progress);
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct CountingWriter {
    bytes_written: u64,
}

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.bytes_written += buffer.len() as u64;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct FailAfterWriter {
    bytes: Vec<u8>,
    fail_after: usize,
    max_per_write: usize,
}

impl Write for FailAfterWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.bytes.len() >= self.fail_after {
            return Err(io::Error::other("injected destination failure"));
        }
        let remaining = self.fail_after - self.bytes.len();
        let accepted = remaining.min(self.max_per_write).min(buffer.len());
        self.bytes.extend_from_slice(&buffer[..accepted]);
        Ok(accepted)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn source_with_bytes(writes: &[(usize, &[u8])], len: usize) -> Vec<u8> {
    let mut source = vec![0u8; len];
    for (offset, bytes) in writes {
        source[*offset..*offset + bytes.len()].copy_from_slice(bytes);
    }
    source
}

fn stream_to_vec(
    source: &dyn SourceReader,
    content_plan: &um_restore::ContentPlan,
    scratch_len: usize,
) -> Result<(Vec<u8>, um_restore::StreamOutcome, Vec<StreamProgress>), RestoreError> {
    let mut output = Vec::new();
    let mut scratch = vec![0u8; scratch_len];
    let mut progress = RecordingProgress::default();
    let outcome = stream_candidate(
        source,
        content_plan,
        &mut output,
        &mut scratch,
        &NeverCancelled,
        &mut progress,
    )?;
    Ok((output, outcome, progress.updates))
}

#[test]
fn stream_contiguous_fragmented_and_resident_content_has_exact_sha256() {
    let source = ObservedReader::new(source_with_bytes(
        &[(100, b"abc"), (200, b"a"), (300, b"b"), (400, b"c")],
        512,
    ));
    let cases = [
        candidate(
            3,
            vec![extent(0, Some(100), 3, ExtentAvailability::FreeInSnapshot)],
        ),
        candidate(
            3,
            vec![
                extent(0, Some(200), 1, ExtentAvailability::FreeInSnapshot),
                extent(1, Some(300), 1, ExtentAvailability::FreeInSnapshot),
                extent(2, Some(400), 1, ExtentAvailability::FreeInSnapshot),
            ],
        ),
        candidate(
            3,
            vec![extent(0, Some(100), 3, ExtentAvailability::Resident)],
        ),
    ];

    for candidate in cases {
        let content_plan = plan_candidate(
            &candidate,
            source.len(),
            None,
            PartialPolicy::CompleteOnly,
            PlanLimits::default(),
        )
        .unwrap()
        .unwrap();
        let (output, outcome, progress) = stream_to_vec(&source, &content_plan, 2).unwrap();
        assert_eq!(output, b"abc");
        assert_eq!(outcome.sha256, ABC_SHA256);
        assert_eq!(outcome.bytes_written, 3);
        assert!(outcome.zero_filled_ranges.is_empty());
        assert!(!outcome.requires_best_effort);
        assert_eq!(progress.last().unwrap().bytes_written, 3);
        assert!(progress
            .windows(2)
            .all(|pair| pair[0].bytes_written < pair[1].bytes_written));
    }
}

#[test]
fn stream_sparse_and_planned_unavailable_ranges_emit_exact_zero_map() {
    let source = ObservedReader::new(source_with_bytes(&[(100, b"ab"), (200, b"ij")], 256));
    let sparse_and_gap = candidate(
        10,
        vec![
            extent(0, Some(100), 2, ExtentAvailability::FreeInSnapshot),
            extent(2, None, 2, ExtentAvailability::Sparse),
            extent(6, Some(200), 2, ExtentAvailability::CurrentlyAllocated),
            extent(8, Some(200), 2, ExtentAvailability::FreeInSnapshot),
        ],
    );
    let content_plan = plan_candidate(
        &sparse_and_gap,
        source.len(),
        None,
        PartialPolicy::ZeroFillAndMap,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();

    let (output, outcome, _) = stream_to_vec(&source, &content_plan, 3).unwrap();
    assert_eq!(output, [b'a', b'b', 0, 0, 0, 0, 0, 0, b'i', b'j']);
    assert_eq!(
        outcome.zero_filled_ranges,
        vec![
            ZeroFilledRange {
                logical_offset: 2,
                len: 2,
                reason: ZeroFillReason::Sparse,
            },
            ZeroFilledRange {
                logical_offset: 4,
                len: 2,
                reason: ZeroFillReason::MissingExtent,
            },
            ZeroFilledRange {
                logical_offset: 6,
                len: 2,
                reason: ZeroFillReason::CurrentlyAllocated,
            },
        ]
    );
    assert!(outcome.requires_best_effort);
    assert_eq!(outcome.bytes_written, 10);
}

#[test]
fn stream_read_faults_zero_exact_ranges_only_with_explicit_policy() {
    let source =
        ObservedReader::new(source_with_bytes(&[(100, b"abcdefgh")], 256)).with_failure(102, 2);
    let readable = candidate(
        8,
        vec![extent(0, Some(100), 8, ExtentAvailability::FreeInSnapshot)],
    );

    let complete_only = plan_candidate(
        &readable,
        source.len(),
        None,
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let mut rejected_output = Vec::new();
    let mut scratch = [0u8; 8];
    let mut rejected_progress = RecordingProgress::default();
    assert!(matches!(
        stream_candidate(
            &source,
            &complete_only,
            &mut rejected_output,
            &mut scratch,
            &NeverCancelled,
            &mut rejected_progress,
        ),
        Err(RestoreError::ReadFailure {
            logical_offset: 2,
            len: 2,
        })
    ));
    assert!(rejected_output.is_empty());
    assert!(rejected_progress.updates.is_empty());

    let zero_fill = plan_candidate(
        &readable,
        source.len(),
        None,
        PartialPolicy::ZeroFillAndMap,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let (output, outcome, _) = stream_to_vec(&source, &zero_fill, 8).unwrap();
    assert_eq!(output, [b'a', b'b', 0, 0, b'e', b'f', b'g', b'h']);
    assert_eq!(
        outcome.zero_filled_ranges,
        vec![ZeroFilledRange {
            logical_offset: 2,
            len: 2,
            reason: ZeroFillReason::PreviouslyReadFailed,
        }]
    );
    assert!(outcome.requires_best_effort);
}

#[test]
fn stream_combines_planned_and_runtime_zero_ranges_in_logical_order() {
    let source = ObservedReader::new(source_with_bytes(&[(100, b"ab")], 256)).with_failure(101, 1);
    let partially_unavailable = candidate(
        4,
        vec![
            extent(0, Some(100), 2, ExtentAvailability::FreeInSnapshot),
            extent(2, Some(200), 2, ExtentAvailability::ReadFailed),
        ],
    );
    let content_plan = plan_candidate(
        &partially_unavailable,
        source.len(),
        None,
        PartialPolicy::ZeroFillAndMap,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();

    let (output, outcome, _) = stream_to_vec(&source, &content_plan, 2).unwrap();
    assert_eq!(output, [b'a', 0, 0, 0]);
    assert_eq!(
        outcome.zero_filled_ranges,
        vec![ZeroFilledRange {
            logical_offset: 1,
            len: 3,
            reason: ZeroFillReason::PreviouslyReadFailed,
        }]
    );
}

#[test]
fn stream_explicitly_zeros_reported_bad_slices_before_writing() {
    let source = ScriptedOutcomeReader::new(
        source_with_bytes(&[(100, b"abcdefgh")], 256),
        ReadOutcome {
            bytes_valid: 6,
            bad_ranges: vec![(2, 2)],
        },
    );
    let readable = candidate(
        8,
        vec![extent(0, Some(100), 8, ExtentAvailability::FreeInSnapshot)],
    );
    let content_plan = plan_candidate(
        &readable,
        source.len(),
        None,
        PartialPolicy::ZeroFillAndMap,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();

    let (output, outcome, _) = stream_to_vec(&source, &content_plan, 8).unwrap();
    assert_eq!(output, [b'a', b'b', 0, 0, b'e', b'f', b'g', b'h']);
    assert_eq!(
        outcome.zero_filled_ranges,
        vec![ZeroFilledRange {
            logical_offset: 2,
            len: 2,
            reason: ZeroFillReason::PreviouslyReadFailed,
        }]
    );
}

#[test]
fn stream_rejects_malformed_read_outcomes_before_writing_the_chunk() {
    let source = ScriptedOutcomeReader::new(
        source_with_bytes(&[(100, b"abcdefgh")], 256),
        ReadOutcome {
            bytes_valid: 8,
            bad_ranges: vec![(7, 2)],
        },
    );
    let readable = candidate(
        8,
        vec![extent(0, Some(100), 8, ExtentAvailability::FreeInSnapshot)],
    );
    let content_plan = plan_candidate(
        &readable,
        source.len(),
        None,
        PartialPolicy::ZeroFillAndMap,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let mut output = Vec::new();
    let mut scratch = [0u8; 8];
    let mut progress = RecordingProgress::default();

    assert!(matches!(
        stream_candidate(
            &source,
            &content_plan,
            &mut output,
            &mut scratch,
            &NeverCancelled,
            &mut progress,
        ),
        Err(RestoreError::InvalidReadOutcome { requested_len: 8 })
    ));
    assert!(output.is_empty());
    assert!(progress.updates.is_empty());
}

#[test]
fn stream_bounds_runtime_zero_fill_evidence_before_writing_the_chunk() {
    let requested_len = MAX_ZERO_FILLED_RANGES * 2 + 1;
    assert!(requested_len <= MAX_STREAM_BUFFER_BYTES);
    let bad_ranges = (0..=MAX_ZERO_FILLED_RANGES)
        .map(|index| ((index * 2) as u64, 1))
        .collect::<Vec<_>>();
    let source = ScriptedOutcomeReader::new(
        vec![0xff; requested_len],
        ReadOutcome {
            bytes_valid: (requested_len - bad_ranges.len()) as u64,
            bad_ranges,
        },
    );
    let readable = candidate(
        requested_len as u64,
        vec![extent(
            0,
            Some(0),
            requested_len as u64,
            ExtentAvailability::FreeInSnapshot,
        )],
    );
    let content_plan = plan_candidate(
        &readable,
        source.len(),
        None,
        PartialPolicy::ZeroFillAndMap,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let mut output = Vec::new();
    let mut scratch = vec![0u8; requested_len];
    let mut progress = RecordingProgress::default();

    assert!(matches!(
        stream_candidate(
            &source,
            &content_plan,
            &mut output,
            &mut scratch,
            &NeverCancelled,
            &mut progress,
        ),
        Err(RestoreError::ZeroFillRangeLimitExceeded {
            limit: MAX_ZERO_FILLED_RANGES,
        })
    ));
    assert!(output.is_empty());
    assert!(progress.updates.is_empty());
}

#[test]
fn stream_cancellation_is_observed_between_bounded_chunks() {
    let source = ObservedReader::new(vec![0x5a; 128 * 1024]);
    let readable = candidate(
        source.len(),
        vec![extent(
            0,
            Some(0),
            source.len(),
            ExtentAvailability::FreeInSnapshot,
        )],
    );
    let content_plan = plan_candidate(
        &readable,
        source.len(),
        None,
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancellation = AtomicCancellation {
        cancelled: Arc::clone(&cancelled),
    };
    let mut progress = CancellingProgress {
        updates: Vec::new(),
        cancelled,
    };
    let mut output = Vec::new();
    let mut scratch = vec![0u8; 64 * 1024];

    assert!(matches!(
        stream_candidate(
            &source,
            &content_plan,
            &mut output,
            &mut scratch,
            &cancellation,
            &mut progress,
        ),
        Err(RestoreError::Cancelled {
            bytes_written: 65_536,
        })
    ));
    assert_eq!(output.len(), 64 * 1024);
    assert_eq!(source.read_count(), 1);
    assert_eq!(progress.updates.last().unwrap().bytes_written, 65_536);
}

#[test]
fn stream_multi_gigabyte_logical_file_keeps_reads_and_scratch_bounded() {
    const FIRST_CHUNK: u64 = MAX_STREAM_BUFFER_BYTES as u64;
    const LOGICAL_SIZE: u64 = 5 * 1024 * 1024 * 1024;
    let source = ObservedReader::new(vec![0x2a; MAX_STREAM_BUFFER_BYTES]);
    let huge = candidate(
        LOGICAL_SIZE,
        vec![extent(
            0,
            Some(0),
            FIRST_CHUNK,
            ExtentAvailability::FreeInSnapshot,
        )],
    );
    let content_plan = plan_candidate(
        &huge,
        source.len(),
        None,
        PartialPolicy::ZeroFillAndMap,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancellation = AtomicCancellation {
        cancelled: Arc::clone(&cancelled),
    };
    let mut progress = CancellingProgress {
        updates: Vec::new(),
        cancelled,
    };
    let mut output = CountingWriter::default();
    let mut scratch = vec![0u8; MAX_STREAM_BUFFER_BYTES];

    assert!(matches!(
        stream_candidate(
            &source,
            &content_plan,
            &mut output,
            &mut scratch,
            &cancellation,
            &mut progress,
        ),
        Err(RestoreError::Cancelled {
            bytes_written: FIRST_CHUNK,
        })
    ));
    assert_eq!(source.max_requested(), MAX_STREAM_BUFFER_BYTES);
    assert_eq!(source.read_count(), 1);
    assert_eq!(scratch.len(), MAX_STREAM_BUFFER_BYTES);
    assert_eq!(output.bytes_written, FIRST_CHUNK);
}

#[test]
fn stream_expected_hash_match_passes_and_mismatch_fails() {
    let source = ObservedReader::new(source_with_bytes(&[(100, b"abc")], 256));
    let readable = candidate(
        3,
        vec![extent(0, Some(100), 3, ExtentAvailability::FreeInSnapshot)],
    );
    let matching = plan_candidate(
        &readable,
        source.len(),
        Some(ABC_SHA256),
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        stream_to_vec(&source, &matching, 2).unwrap().1.sha256,
        ABC_SHA256
    );

    let mismatching = plan_candidate(
        &readable,
        source.len(),
        Some([0u8; 32]),
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let mut output = Vec::new();
    let mut scratch = [0u8; 2];
    let mut progress = RecordingProgress::default();
    assert!(matches!(
        stream_candidate(
            &source,
            &mismatching,
            &mut output,
            &mut scratch,
            &NeverCancelled,
            &mut progress,
        ),
        Err(RestoreError::HashMismatch {
            expected,
            actual: ABC_SHA256,
        }) if expected == [0u8; 32]
    ));
    assert_eq!(output, b"abc");
    assert_eq!(progress.updates.last().unwrap().bytes_written, 3);
}

#[test]
fn stream_rejects_malformed_plan_coverage_before_first_read_or_write() {
    let source = ObservedReader::new(source_with_bytes(&[(100, b"abcd"), (200, b"efgh")], 256));
    let fragmented = candidate(
        8,
        vec![
            extent(0, Some(100), 4, ExtentAvailability::FreeInSnapshot),
            extent(4, Some(200), 4, ExtentAvailability::FreeInSnapshot),
        ],
    );
    let valid = plan_candidate(
        &fragmented,
        source.len(),
        None,
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let malformed_segments = [
        vec![valid.segments[1].clone(), valid.segments[0].clone()],
        vec![valid.segments[0].clone(), valid.segments[0].clone()],
        vec![
            valid.segments[0].clone(),
            ContentSegment::Read {
                logical_offset: 2,
                physical_offset: 200,
                len: 4,
            },
        ],
        vec![
            valid.segments[0].clone(),
            ContentSegment::Read {
                logical_offset: 5,
                physical_offset: 200,
                len: 3,
            },
        ],
    ];

    for segments in malformed_segments {
        let mut malformed = valid.clone();
        malformed.segments = segments;
        let mut output = Vec::new();
        let mut scratch = [0u8; 8];
        let mut progress = RecordingProgress::default();
        assert!(matches!(
            stream_candidate(
                &source,
                &malformed,
                &mut output,
                &mut scratch,
                &NeverCancelled,
                &mut progress,
            ),
            Err(RestoreError::InvalidPlanCoverage { .. })
        ));
        assert!(output.is_empty());
        assert!(progress.updates.is_empty());
    }
    assert_eq!(source.read_count(), 0);
}

#[test]
fn stream_rejects_reads_outside_actual_source_before_first_read() {
    let source = ObservedReader::new(vec![0u8; 64]);
    let planned_against_larger_source = candidate(
        8,
        vec![extent(0, Some(60), 8, ExtentAvailability::FreeInSnapshot)],
    );
    let content_plan = plan_candidate(
        &planned_against_larger_source,
        128,
        None,
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let mut output = Vec::new();
    let mut scratch = [0u8; 8];
    let mut progress = RecordingProgress::default();

    assert!(matches!(
        stream_candidate(
            &source,
            &content_plan,
            &mut output,
            &mut scratch,
            &NeverCancelled,
            &mut progress,
        ),
        Err(RestoreError::PhysicalRangeOutOfBounds {
            physical_offset: 60,
            len: 8,
            source_len: 64,
        })
    ));
    assert_eq!(source.read_count(), 0);
    assert!(output.is_empty());
}

#[test]
fn stream_progress_reports_only_bytes_accepted_by_the_writer() {
    let source = ObservedReader::new(source_with_bytes(&[(100, b"abcdefgh")], 256));
    let readable = candidate(
        8,
        vec![extent(0, Some(100), 8, ExtentAvailability::FreeInSnapshot)],
    );
    let content_plan = plan_candidate(
        &readable,
        source.len(),
        None,
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let mut output = FailAfterWriter {
        bytes: Vec::new(),
        fail_after: 5,
        max_per_write: 3,
    };
    let mut scratch = [0u8; 8];
    let mut progress = RecordingProgress::default();

    assert!(matches!(
        stream_candidate(
            &source,
            &content_plan,
            &mut output,
            &mut scratch,
            &NeverCancelled,
            &mut progress,
        ),
        Err(RestoreError::OutputWrite {
            bytes_written: 5,
            ..
        })
    ));
    assert_eq!(output.bytes, b"abcde");
    assert_eq!(
        progress
            .updates
            .iter()
            .map(|update| update.bytes_written)
            .collect::<Vec<_>>(),
        vec![3, 5]
    );
}

#[test]
fn stream_rejects_empty_or_oversized_scratch_before_io() {
    let source = ObservedReader::new(source_with_bytes(&[(100, b"a")], 128));
    let readable = candidate(
        1,
        vec![extent(0, Some(100), 1, ExtentAvailability::FreeInSnapshot)],
    );
    let content_plan = plan_candidate(
        &readable,
        source.len(),
        None,
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();

    for scratch_len in [0, MAX_STREAM_BUFFER_BYTES + 1] {
        let mut output = Vec::new();
        let mut scratch = vec![0u8; scratch_len];
        let mut progress = RecordingProgress::default();
        assert!(matches!(
            stream_candidate(
                &source,
                &content_plan,
                &mut output,
                &mut scratch,
                &NeverCancelled,
                &mut progress,
            ),
            Err(RestoreError::InvalidScratchLength { len, .. }) if len == scratch_len
        ));
        assert!(output.is_empty());
    }
    assert_eq!(source.read_count(), 0);
}

#[test]
fn stream_zero_length_file_produces_the_empty_sha256_without_io() {
    const EMPTY_SHA256: [u8; 32] = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];
    let source = ObservedReader::new(Vec::new());
    let empty = candidate(0, Vec::new());
    let content_plan = plan_candidate(
        &empty,
        source.len(),
        Some(EMPTY_SHA256),
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();

    let (output, outcome, progress) = stream_to_vec(&source, &content_plan, 1).unwrap();
    assert!(output.is_empty());
    assert_eq!(outcome.bytes_written, 0);
    assert_eq!(outcome.sha256, EMPTY_SHA256);
    assert!(outcome.zero_filled_ranges.is_empty());
    assert!(progress.is_empty());
    assert_eq!(source.read_count(), 0);
}
