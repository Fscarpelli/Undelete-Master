use cap_std::ambient_authority;
use cap_std::fs::Dir;
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use um_core::{
    Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
    MetadataConfidence, ReadError, ReadOutcome, SectorLayout, SourceIdentity, SourceKind,
    SourceReader, Timestamps,
};
use um_restore::{
    audit_journal, job_directory_component, plan_candidate, CancellationProbe, ContentPlan,
    DestinationRoot, FileRestorePlan, PartialPolicy, PlanLimits, ProgressSink,
    RestoreCompletionStatus, RestoreError, RestoreJobPlan, SafeRelativePath, StreamProgress,
    RESTORE_SCRATCH_BYTES,
};

struct MemoryReader {
    data: Vec<u8>,
    identity: SourceIdentity,
    max_read: AtomicUsize,
}

impl MemoryReader {
    fn new(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            identity: SourceIdentity {
                id: "transaction-fixture".into(),
                kind: SourceKind::ImageFile,
                label: "synthetic".into(),
                size: data.len() as u64,
            },
            max_read: AtomicUsize::new(0),
        }
    }
}

impl SourceReader for MemoryReader {
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
        self.max_read.fetch_max(buffer.len(), Ordering::SeqCst);
        match self.read_exact_at(offset, buffer) {
            Ok(()) => ReadOutcome::complete(buffer.len() as u64),
            Err(_) => {
                buffer.fill(0);
                ReadOutcome {
                    bytes_valid: 0,
                    bad_ranges: vec![(0, buffer.len() as u64)],
                }
            }
        }
    }
}

struct NeverCancel;

impl CancellationProbe for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct Cancelled(AtomicBool);

impl CancellationProbe for Cancelled {
    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Default)]
struct Progress(Vec<StreamProgress>);

impl ProgressSink for Progress {
    fn advanced(&mut self, progress: StreamProgress) {
        self.0.push(progress);
    }
}

fn retained_directory(path: &std::path::Path) -> std::fs::File {
    Dir::open_ambient_dir(path, ambient_authority())
        .unwrap()
        .into_std_file()
}

fn root(temp: &tempfile::TempDir) -> DestinationRoot {
    DestinationRoot::from_retained_file(retained_directory(temp.path())).unwrap()
}

fn safe(name: &str) -> um_restore::DerivedSafePath {
    SafeRelativePath::derive_for_recovery(&[] as &[&str], name).unwrap()
}

fn read_plan(candidate_id: u64, source_offset: u64, len: u64) -> ContentPlan {
    plan_candidate(
        &candidate(
            candidate_id,
            len,
            vec![ExtentRun {
                logical_offset: 0,
                physical_offset: Some(source_offset),
                len,
                availability: ExtentAvailability::FreeInSnapshot,
            }],
        ),
        source_offset + len,
        None,
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap()
}

fn zero_fill_plan(candidate_id: u64) -> ContentPlan {
    plan_candidate(
        &candidate(
            candidate_id,
            8,
            vec![ExtentRun {
                logical_offset: 0,
                physical_offset: Some(0),
                len: 4,
                availability: ExtentAvailability::FreeInSnapshot,
            }],
        ),
        4,
        None,
        PartialPolicy::ZeroFillAndMap,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap()
}

fn candidate(candidate_id: u64, size: u64, extents: Vec<ExtentRun>) -> Candidate {
    Candidate {
        id: candidate_id,
        kind: CandidateKind::File,
        method: DiscoveryMethod::NtfsMetadata,
        state: CandidateState::CompleteUnvalidated,
        name: "fixture.bin".into(),
        name_certain: true,
        parent_path: Vec::new(),
        metadata_confidence: MetadataConfidence::High,
        size,
        timestamps: Timestamps::default(),
        extents,
        record_ref: candidate_id,
        sequence: Some(1),
        warnings: Vec::new(),
    }
}

fn single_job(job_id: &str, item: FileRestorePlan) -> RestoreJobPlan {
    RestoreJobPlan::new(job_id, vec![item]).unwrap()
}

#[test]
fn transaction_success_prepares_then_publishes_and_hashes_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let destination = root(&temp);
    let source = MemoryReader::new(b"recover me");
    let item = FileRestorePlan::file(safe("report.bin"), read_plan(1, 0, 10))
        .unwrap()
        .with_warnings(vec!["fixture warning".into()])
        .unwrap();
    let mut progress = Progress::default();

    let summary = destination
        .restore_job(
            &source,
            &single_job("success", item),
            &NeverCancel,
            &mut progress,
        )
        .unwrap();

    let job_dir = temp.path().join(summary.job_directory_name());
    assert_eq!(fs::read(job_dir.join("report.bin")).unwrap(), b"recover me");
    let has_temporary = fs::read_dir(&job_dir).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".umrecovering")
    });
    match summary.completion_status() {
        RestoreCompletionStatus::CompletedDurable => assert!(!has_temporary),
        RestoreCompletionStatus::NeedsReconciliation => assert!(has_temporary),
    }
    assert_eq!(source.max_read.load(Ordering::SeqCst), 10);
    assert!(source.max_read.load(Ordering::SeqCst) <= RESTORE_SCRATCH_BYTES);

    let manifest = fs::read(job_dir.join(summary.manifest_name())).unwrap();
    let manifest_hash: [u8; 32] = Sha256::digest(&manifest).into();
    assert_eq!(manifest_hash, summary.manifest_sha256());
    let manifest_json: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(manifest_json["version"], 1);
    assert_eq!(manifest_json["jobId"], "success");
    assert_eq!(
        manifest_json["journalHeadSha256"].as_str().unwrap().len(),
        64
    );
    assert_eq!(manifest_json["items"].as_array().unwrap().len(), 1);
    let manifest_item = &manifest_json["items"][0];
    assert_eq!(manifest_item["candidateId"], 1);
    assert_eq!(manifest_item["disposition"], "published");
    assert_eq!(manifest_item["requestedPath"], "report.bin");
    assert_eq!(manifest_item["publishedPath"], "report.bin");
    assert_eq!(manifest_item["outputLength"], 10);
    assert_eq!(
        manifest_item["outputSha256"],
        "bc54d1d8c0a99336ea2c89cccee81d1545b9e5c10791b3e5a7140803035213fb"
    );
    assert_eq!(
        manifest_item["readableRanges"],
        serde_json::json!([{"logicalOffset": 0, "length": 10}])
    );
    assert_eq!(manifest_item["zeroFilledRanges"], serde_json::json!([]));
    assert_eq!(manifest_item["conflicts"], serde_json::json!([]));
    assert_eq!(manifest_item["readErrors"], serde_json::json!([]));
    assert_eq!(
        manifest_item["warnings"],
        serde_json::json!(["fixture warning"])
    );
    assert_eq!(manifest_item["pathEvidence"]["version"], 1);

    let journal = fs::read(job_dir.join(summary.journal_name())).unwrap();
    let audit = audit_journal(&journal).unwrap();
    assert!(audit.is_hash_chain_valid());
    assert!(audit
        .kinds()
        .windows(2)
        .any(|pair| { pair == ["itemPrepared", "itemPublished"] }));
    assert_eq!(
        audit.sequences(),
        (0..audit.record_count() as u64).collect::<Vec<_>>()
    );
    assert!(progress
        .0
        .last()
        .is_some_and(|value| value.bytes_written == 10));
}

#[test]
fn transaction_collisions_are_atomic_and_never_replace_the_first_file() {
    let temp = tempfile::tempdir().unwrap();
    let destination = root(&temp);
    let source = MemoryReader::new(b"firstsecond");
    let first = FileRestorePlan::file(safe("same.txt"), read_plan(1, 0, 5)).unwrap();
    let second = FileRestorePlan::file(safe("same.txt"), read_plan(2, 5, 6)).unwrap();
    let job = RestoreJobPlan::new("collisions", vec![first, second]).unwrap();

    let summary = destination
        .restore_job(&source, &job, &NeverCancel, &mut Progress::default())
        .unwrap();
    let job_dir = temp.path().join(summary.job_directory_name());

    assert_eq!(fs::read(job_dir.join("same.txt")).unwrap(), b"first");
    assert_eq!(
        fs::read(job_dir.join("same (recovered 1).txt")).unwrap(),
        b"second"
    );
    assert_eq!(
        summary
            .items()
            .iter()
            .map(|item| item.published_name())
            .collect::<Vec<_>>(),
        ["same.txt", "same (recovered 1).txt"]
    );
}

#[test]
fn transaction_cancel_never_exposes_an_incomplete_final_file() {
    let temp = tempfile::tempdir().unwrap();
    let destination = root(&temp);
    let source = MemoryReader::new(&vec![0x5a; RESTORE_SCRATCH_BYTES + 1]);
    let item = FileRestorePlan::file(safe("cancelled.bin"), read_plan(1, 0, source.len())).unwrap();
    let cancelled = Cancelled(AtomicBool::new(true));

    let error = destination
        .restore_job(
            &source,
            &single_job("cancelled", item),
            &cancelled,
            &mut Progress::default(),
        )
        .unwrap_err();

    assert!(matches!(error, RestoreError::Cancelled { .. }));
    let exposed = walk_files(temp.path())
        .into_iter()
        .any(|path| path.file_name().is_some_and(|name| name == "cancelled.bin"));
    assert!(!exposed);
}

#[test]
fn transaction_failure_manifest_preserves_the_immutable_plan_item_count() {
    let temp = tempfile::tempdir().unwrap();
    let destination = root(&temp);
    let source = MemoryReader::new(b"firstfailthird");
    let first = FileRestorePlan::file(safe("first.bin"), read_plan(1, 0, 5)).unwrap();
    let mut failing_content = read_plan(2, 5, 4);
    failing_content.expected_sha256 = Some([0; 32]);
    let failing = FileRestorePlan::file(safe("failing.bin"), failing_content).unwrap();
    let third = FileRestorePlan::file(safe("third.bin"), read_plan(3, 9, 5)).unwrap();
    let job_id = "failure-manifest";
    let job = RestoreJobPlan::new(job_id, vec![first, failing, third]).unwrap();

    let error = destination
        .restore_job(&source, &job, &NeverCancel, &mut Progress::default())
        .unwrap_err();

    assert!(matches!(error, RestoreError::HashMismatch { .. }));
    let job_dir = temp.path().join(job_directory_component(job_id));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(job_dir.join("recovery-manifest.json")).unwrap()).unwrap();
    let items = manifest["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["candidateId"], 1);
    assert_eq!(items[0]["disposition"], "published");
    assert_eq!(items[1]["candidateId"], 2);
    assert_eq!(items[1]["disposition"], "failed");
    assert_eq!(items[1]["publishedPath"], serde_json::Value::Null);
    assert_eq!(items[2]["candidateId"], 3);
    assert_eq!(items[2]["disposition"], "notAttempted");
    assert_eq!(items[2]["publishedPath"], serde_json::Value::Null);
    assert!(!job_dir.join("third.bin").exists());
}

#[test]
fn transaction_cancellation_manifest_marks_current_and_unattempted_items() {
    let temp = tempfile::tempdir().unwrap();
    let destination = root(&temp);
    let source = MemoryReader::new(b"firstsecond");
    let first = FileRestorePlan::file(safe("first.bin"), read_plan(1, 0, 5)).unwrap();
    let second = FileRestorePlan::file(safe("second.bin"), read_plan(2, 5, 6)).unwrap();
    let job_id = "cancel-manifest";
    let job = RestoreJobPlan::new(job_id, vec![first, second]).unwrap();
    let cancelled = Cancelled(AtomicBool::new(true));

    let error = destination
        .restore_job(&source, &job, &cancelled, &mut Progress::default())
        .unwrap_err();

    assert!(matches!(error, RestoreError::Cancelled { .. }));
    let job_dir = temp.path().join(job_directory_component(job_id));
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(job_dir.join("recovery-manifest.json")).unwrap()).unwrap();
    let items = manifest["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["candidateId"], 1);
    assert_eq!(items[0]["disposition"], "cancelled");
    assert_eq!(items[1]["candidateId"], 2);
    assert_eq!(items[1]["disposition"], "notAttempted");
}

#[test]
fn transaction_partial_file_emits_exact_versioned_range_sidecar() {
    let temp = tempfile::tempdir().unwrap();
    let destination = root(&temp);
    let source = MemoryReader::new(b"read");
    let mut content = zero_fill_plan(7);
    content.expected_sha256 = Some([
        0xab, 0xf5, 0x5a, 0xdc, 0x5a, 0xfc, 0xfa, 0x05, 0xc3, 0xc6, 0xf2, 0xd0, 0x44, 0xc0, 0x7f,
        0xaa, 0xf1, 0xd7, 0xda, 0x50, 0x28, 0x01, 0xd4, 0x3a, 0xfc, 0xbd, 0xbb, 0x3a, 0x07, 0xbd,
        0xb2, 0xc0,
    ]);
    let item = FileRestorePlan::file(safe("partial.bin"), content)
        .unwrap()
        .with_warnings(vec!["best-effort recovery confirmed".into()])
        .unwrap();

    let summary = destination
        .restore_job(
            &source,
            &single_job("partial", item),
            &NeverCancel,
            &mut Progress::default(),
        )
        .unwrap();
    let job_dir = temp.path().join(summary.job_directory_name());

    assert_eq!(
        fs::read(job_dir.join("partial.bin")).unwrap(),
        b"read\0\0\0\0"
    );
    let sidecar_name = ".um-partial-000000-0000000000000007.json";
    let sidecar = fs::read_to_string(job_dir.join(sidecar_name)).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&sidecar).unwrap(),
        serde_json::json!({
            "version": 1,
            "itemKey": "000000-0000000000000007",
            "candidateId": 7,
            "logicalSize": 8,
            "readableRanges": [{"logicalOffset": 0, "length": 4}],
            "zeroFilledRanges": [{
                "logicalOffset": 4,
                "length": 4,
                "reason": "missingExtent"
            }],
            "zeroFilledMissingRanges": [{
                "logicalOffset": 4,
                "length": 4,
                "reason": "missingExtent"
            }],
            "conflictRanges": [],
            "sourceReadErrors": [],
            "outputSha256":
                "abf55adc5afcfa05c3c6f2d044c07faaf1d7da502801d43afcbdbb3a07bdb2c0",
            "expectedContentSha256":
                "abf55adc5afcfa05c3c6f2d044c07faaf1d7da502801d43afcbdbb3a07bdb2c0",
            "validation": {
                "expectedSha256Present": true,
                "expectedSha256Matched": true
            },
            "warnings": ["best-effort recovery confirmed"]
        })
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(job_dir.join(summary.manifest_name())).unwrap()).unwrap();
    let item = &manifest["items"][0];
    assert_eq!(
        item["readableRanges"],
        serde_json::json!([{"logicalOffset": 0, "length": 4}])
    );
    assert_eq!(
        item["zeroFilledRanges"],
        serde_json::json!([{
            "logicalOffset": 4,
            "length": 4,
            "reason": "missingExtent"
        }])
    );
    assert_eq!(item["conflicts"], serde_json::json!([]));
    assert_eq!(item["readErrors"], serde_json::json!([]));
    assert_eq!(item["itemKey"], "000000-0000000000000007");
    assert_eq!(item["partialSidecar"], sidecar_name);
    assert_eq!(
        item["partialSidecarSha256"],
        format!("{:x}", Sha256::digest(sidecar.as_bytes()))
    );
}

#[test]
fn transaction_directory_only_creates_exactly_the_selected_sanitized_directory() {
    let temp = tempfile::tempdir().unwrap();
    let destination = root(&temp);
    let source = MemoryReader::new(b"");
    let selected = SafeRelativePath::derive_for_recovery(&["selected"], "only-directory").unwrap();
    let item = FileRestorePlan::directory(91, selected).unwrap();
    let job = single_job("directory-only", item);

    let summary = destination
        .restore_job(&source, &job, &NeverCancel, &mut Progress::default())
        .unwrap();
    let job_dir = temp.path().join(summary.job_directory_name());

    assert!(job_dir.join("selected").join("only-directory").is_dir());
    assert!(!job_dir
        .join("selected")
        .join("only-directory")
        .join("historical-child")
        .exists());
    assert!(!job_dir.join("historical-sibling").exists());
    assert_eq!(source.max_read.load(Ordering::SeqCst), 0);
    assert_eq!(summary.items()[0].output_len(), None);
    assert_eq!(summary.items()[0].output_sha256(), None);
    assert_eq!(summary.items()[0].temporary_disposition(), None);
    assert!(!walk_files(&job_dir).iter().any(|path| {
        let name = path.file_name().unwrap().to_string_lossy();
        name.contains("-data-") || name.contains("-partial-") || name.ends_with(".um-partial.json")
    }));

    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(job_dir.join(summary.manifest_name())).unwrap()).unwrap();
    let items = manifest["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["candidateId"], 91);
    assert_eq!(items[0]["itemKind"], "directory");
    assert_eq!(items[0]["requestedPath"], "selected/only-directory");
    assert_eq!(items[0]["publishedPath"], "selected/only-directory");
    assert_eq!(items[0]["outputLength"], serde_json::Value::Null);
    assert_eq!(items[0]["outputSha256"], serde_json::Value::Null);
    assert_eq!(
        items[0]["temporaryFileDisposition"],
        serde_json::Value::Null
    );
    assert_eq!(items[0]["partialSidecar"], serde_json::Value::Null);
    assert_eq!(items[0]["directoryNoFollowBindCompleted"], true);
    assert_eq!(items[0]["directoryValidation"], "boundNoFollow");
    match items[0]["namespaceDurability"]["state"].as_str().unwrap() {
        "synced" => {
            assert_eq!(items[0]["disposition"], "directoryCreated");
            assert_eq!(items[0]["completionStatus"], "completedDurable");
            assert_eq!(items[0]["failureKind"], serde_json::Value::Null);
        }
        "unconfirmed" | "unsupported" | "failed" => {
            assert_eq!(items[0]["disposition"], "directoryNeedsReconciliation");
            assert_eq!(items[0]["completionStatus"], "needsReconciliation");
            assert_eq!(items[0]["failureKind"], "needsReconciliation");
        }
        state => panic!("unexpected namespace durability state {state:?}"),
    }
}

#[cfg(unix)]
#[test]
fn transaction_rejects_a_symlink_below_the_retained_root() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("sentinel"), b"unchanged").unwrap();
    let predictable_job_dir = um_restore::job_directory_component("symlink");
    fs::create_dir(temp.path().join(&predictable_job_dir)).unwrap();
    symlink(
        outside.path(),
        temp.path().join(&predictable_job_dir).join("escape"),
    )
    .unwrap();

    let destination = root(&temp);
    let source = MemoryReader::new(b"payload");
    let derived = SafeRelativePath::derive_for_recovery(&["escape"], "outside.bin").unwrap();
    let item = FileRestorePlan::file(derived, read_plan(1, 0, 7)).unwrap();

    assert!(destination
        .restore_job(
            &source,
            &single_job("symlink", item),
            &NeverCancel,
            &mut Progress::default(),
        )
        .is_err());
    assert_eq!(
        fs::read(outside.path().join("sentinel")).unwrap(),
        b"unchanged"
    );
    assert!(!outside.path().join("outside.bin").exists());
}

fn walk_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            } else {
                files.push(entry.path());
            }
        }
    }
    files
}
