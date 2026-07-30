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
    audit_journal, plan_candidate, CancellationProbe, ContentPlan, DestinationRoot,
    FileRestorePlan, PartialPolicy, PlanLimits, ProgressSink, RestoreCompletionStatus,
    RestoreError, RestoreJobPlan, SafeRelativePath, StreamProgress, RESTORE_SCRATCH_BYTES,
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
fn transaction_partial_file_emits_exact_versioned_range_sidecar() {
    let temp = tempfile::tempdir().unwrap();
    let destination = root(&temp);
    let source = MemoryReader::new(b"read");
    let item = FileRestorePlan::file(safe("partial.bin"), zero_fill_plan(7)).unwrap();

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
    let sidecar = fs::read_to_string(job_dir.join("partial.bin.um-partial.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&sidecar).unwrap(),
        serde_json::json!({
            "version": 1,
            "logicalSize": 8,
            "readableRanges": [{"logicalOffset": 0, "length": 4}],
            "zeroFilledRanges": [{
                "logicalOffset": 4,
                "length": 4,
                "reason": "missingExtent"
            }]
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
    assert_eq!(item["partialSidecar"], "partial.bin.um-partial.json");
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
