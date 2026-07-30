#![cfg(windows)]

use cap_std::{ambient_authority, fs::Dir};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use um_core::{
    Candidate, CandidateKind, CandidateState, DiscoveryMethod, ExtentAvailability, ExtentRun,
    MetadataConfidence, ReadError, ReadOutcome, SectorLayout, SourceIdentity, SourceKind,
    SourceReader, Timestamps,
};
use um_restore::{
    job_directory_component, plan_candidate, CancellationProbe, DestinationRoot, FileRestorePlan,
    PartialPolicy, PlanLimits, ProgressSink, RestoreJobPlan, SafeRelativePath, StreamProgress,
};

struct MemoryReader {
    bytes: Vec<u8>,
    identity: SourceIdentity,
}

impl MemoryReader {
    fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.to_vec(),
            identity: SourceIdentity {
                id: "windows-transaction-fixture".into(),
                kind: SourceKind::ImageFile,
                label: "synthetic".into(),
                size: bytes.len() as u64,
            },
        }
    }
}

impl SourceReader for MemoryReader {
    fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn sector_layout(&self) -> SectorLayout {
        SectorLayout::DEFAULT_512
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        let start = usize::try_from(offset).map_err(|_| ReadError::OutOfBounds {
            offset,
            len: buffer.len() as u64,
            source_len: self.len(),
        })?;
        let end = start
            .checked_add(buffer.len())
            .filter(|end| *end <= self.bytes.len())
            .ok_or(ReadError::OutOfBounds {
                offset,
                len: buffer.len() as u64,
                source_len: self.len(),
            })?;
        buffer.copy_from_slice(&self.bytes[start..end]);
        Ok(())
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        match self.read_exact_at(offset, buffer) {
            Ok(()) => ReadOutcome::complete(buffer.len() as u64),
            Err(_) => ReadOutcome {
                bytes_valid: 0,
                bad_ranges: vec![(0, buffer.len() as u64)],
            },
        }
    }
}

struct NeverCancel;

impl CancellationProbe for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}

struct NoProgress;

impl ProgressSink for NoProgress {
    fn advanced(&mut self, _progress: StreamProgress) {}
}

fn destination(path: &Path) -> DestinationRoot {
    let retained = Dir::open_ambient_dir(path, ambient_authority())
        .unwrap()
        .into_std_file();
    DestinationRoot::from_retained_file(retained).unwrap()
}

fn job(job_id: &str, parents: &[String], file_name: &str, len: u64) -> RestoreJobPlan {
    let candidate = Candidate {
        id: 81,
        kind: CandidateKind::File,
        method: DiscoveryMethod::NtfsMetadata,
        state: CandidateState::CompleteUnvalidated,
        name: file_name.into(),
        name_certain: true,
        parent_path: Vec::new(),
        metadata_confidence: MetadataConfidence::High,
        size: len,
        timestamps: Timestamps::default(),
        extents: vec![ExtentRun {
            logical_offset: 0,
            physical_offset: Some(0),
            len,
            availability: ExtentAvailability::FreeInSnapshot,
        }],
        record_ref: 81,
        sequence: Some(1),
        warnings: Vec::new(),
    };
    let content = plan_candidate(
        &candidate,
        len,
        None,
        PartialPolicy::CompleteOnly,
        PlanLimits::default(),
    )
    .unwrap()
    .unwrap();
    let path = SafeRelativePath::derive_for_recovery(parents, file_name).unwrap();
    RestoreJobPlan::new(job_id, vec![FileRestorePlan::file(path, content).unwrap()]).unwrap()
}

struct CompetingTempMutation {
    job_dir: PathBuf,
    attempted: bool,
    rename_error: Option<std::io::ErrorKind>,
    delete_error: Option<std::io::ErrorKind>,
}

impl ProgressSink for CompetingTempMutation {
    fn advanced(&mut self, _progress: StreamProgress) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        let temporary = fs::read_dir(&self.job_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with(".umrecovering"))
            })
            .expect("stream progress occurs with the temporary link present");
        let moved = self.job_dir.join("attacker-moved.tmp");
        let competing_temporary = temporary.clone();
        let competing = thread::spawn(move || {
            (
                fs::rename(&competing_temporary, &moved),
                fs::remove_file(&competing_temporary),
            )
        });
        let (rename, delete) = competing.join().unwrap();
        self.rename_error = rename.err().map(|error| error.kind());
        self.delete_error = delete.err().map(|error| error.kind());
    }
}

#[test]
fn transaction_windows_open_temporary_handle_denies_competing_rename_and_delete() {
    let temp = tempfile::tempdir().unwrap();
    let job_id = "windows-open-temp";
    let job_dir = temp.path().join(job_directory_component(job_id));
    let bytes = b"handle identity remains stable";
    let mut progress = CompetingTempMutation {
        job_dir: job_dir.clone(),
        attempted: false,
        rename_error: None,
        delete_error: None,
    };

    let summary = destination(temp.path())
        .restore_job(
            &MemoryReader::new(bytes),
            &job(job_id, &[], "stable.bin", bytes.len() as u64),
            &NeverCancel,
            &mut progress,
        )
        .unwrap();

    assert!(progress.attempted);
    assert!(
        progress.rename_error.is_some(),
        "a competing rename unexpectedly replaced the open temporary name"
    );
    assert!(
        progress.delete_error.is_some(),
        "a competing delete unexpectedly removed the open temporary name"
    );
    assert_eq!(
        fs::read(
            temp.path()
                .join(summary.job_directory_name())
                .join("stable.bin")
        )
        .unwrap(),
        bytes
    );
    assert!(!job_dir.join("attacker-moved.tmp").exists());
}

#[test]
fn transaction_windows_repeated_real_junction_swaps_never_escape_the_retained_root() {
    let mut successful_swaps = 0usize;
    for iteration in 0..8 {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("sentinel.bin"), b"unchanged").unwrap();
        let parents = (0..96)
            .map(|index| format!("level-{index:03}"))
            .collect::<Vec<_>>();
        let job_id = format!("junction-race-{iteration}");
        let job_dir = temp.path().join(job_directory_component(&job_id));
        let in_root_decoy = job_dir.join(".attacker-decoy");
        let stop = Arc::new(AtomicBool::new(false));
        let swaps = Arc::new(AtomicUsize::new(0));
        let attacker_stop = Arc::clone(&stop);
        let attacker_swaps = Arc::clone(&swaps);
        let attacker_job_dir = job_dir.clone();
        let attacker_outside = outside.path().to_owned();
        let attacker_decoy = in_root_decoy.clone();
        let attacker_parents = parents.clone();

        let attacker = thread::spawn(move || {
            while !attacker_stop.load(Ordering::SeqCst) {
                let redirect_target = if iteration % 2 == 0 {
                    attacker_outside.clone()
                } else {
                    if fs::create_dir(&attacker_decoy).is_err() && !attacker_decoy.is_dir() {
                        thread::yield_now();
                        continue;
                    }
                    attacker_decoy.clone()
                };
                let mut current = attacker_job_dir.clone();
                for (index, component) in attacker_parents.iter().enumerate() {
                    current.push(component);
                    if !current.exists() {
                        if junction::create(&redirect_target, &current).is_ok() {
                            attacker_swaps.fetch_add(1, Ordering::SeqCst);
                            return;
                        }
                        continue;
                    }
                    if junction::exists(&current).unwrap_or(false) {
                        break;
                    }
                    let held = current.with_extension(format!("held-{iteration}-{index}"));
                    if fs::rename(&current, &held).is_err() {
                        continue;
                    }
                    match junction::create(&redirect_target, &current) {
                        Ok(()) => {
                            attacker_swaps.fetch_add(1, Ordering::SeqCst);
                            return;
                        }
                        Err(_) => {
                            let _ = fs::rename(&held, &current);
                        }
                    }
                }
                thread::yield_now();
            }
        });

        let bytes = b"junction race payload";
        let _result = destination(temp.path()).restore_job(
            &MemoryReader::new(bytes),
            &job(&job_id, &parents, "outside.bin", bytes.len() as u64),
            &NeverCancel,
            &mut NoProgress,
        );
        stop.store(true, Ordering::SeqCst);
        attacker.join().unwrap();
        successful_swaps += swaps.load(Ordering::SeqCst);

        assert_eq!(
            fs::read(outside.path().join("sentinel.bin")).unwrap(),
            b"unchanged"
        );
        assert_eq!(
            fs::read_dir(outside.path()).unwrap().count(),
            1,
            "iteration {iteration} created an entry through a junction"
        );
        assert!(
            !in_root_decoy.join("outside.bin").exists(),
            "iteration {iteration} followed an in-root junction instead of rejecting it"
        );
        remove_junctions_below(&job_dir);
    }

    assert!(
        successful_swaps > 0,
        "the real NTFS junction attacker never won a disposable transition race"
    );
}

fn remove_junctions_below(root: &Path) {
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if junction::exists(&path).unwrap_or(false) {
                junction::delete(&path).unwrap();
            } else if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push(path);
            }
        }
    }
}
