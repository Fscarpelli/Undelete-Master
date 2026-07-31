//! Vertical desktop recovery proof over a deterministic NTFS image.
//!
//! The production source modules are included as white-box modules so this
//! integration target exercises the same storage authority and restore
//! coordinator implementation without exporting test-only production APIs.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "white-box source modules include command surfaces not invoked by this vertical test"
)]

#[path = "../src/restore.rs"]
mod restore;
#[path = "../src/results.rs"]
mod results;
#[path = "../src/storage.rs"]
mod storage;

use std::{
    fs,
    sync::Arc,
    time::{Duration, Instant},
};

use cap_fs_ext::DirExt;
use cap_std::{ambient_authority, fs::Dir};
use restore::{
    AdmittedDestination, CollisionPolicyDto, DesktopRestoreError, DestinationCapability,
    DestinationRevalidation, OpenedRestoreSource, PartialFilePolicyDto, RestoreCoordinator,
    RestoreJobSnapshotDto, RestoreJobStatusDto, RestoreRuntime,
};
use results::{
    CandidateQuery, CandidateSort, CandidateSortDirection, CandidateSortField,
    SelectionOperationDto,
};
use sha2::{Digest, Sha256};
use storage::{DesktopStorageState, SessionScope};
use um_fixture_builder::{
    deterministic_bytes,
    ntfs::{FileOptions, NodeParent, NtfsImageBuilder},
};
use um_io_common::MemImageReader;

struct FixtureRuntime {
    image: Arc<Vec<u8>>,
}

impl RestoreRuntime for FixtureRuntime {
    fn open_source(
        &self,
        binding: &results::ScanSourceBinding,
    ) -> Result<OpenedRestoreSource, DesktopRestoreError> {
        Ok(OpenedRestoreSource {
            reader: Box::new(MemImageReader::new(
                &binding.volume_id,
                self.image.as_ref().clone(),
            )),
            physical_disk_number: 7,
        })
    }
}

struct FixtureDestination {
    root: fs::File,
}

impl DestinationCapability for FixtureDestination {
    fn revalidate(&self) -> Result<DestinationRevalidation, DesktopRestoreError> {
        Ok(DestinationRevalidation {
            file_system: "NTFS".into(),
            free_bytes: 8 * 1024 * 1024,
            physical_disk_number: 9,
            reparse_safe: true,
        })
    }

    fn try_clone_root(&self) -> Result<fs::File, DesktopRestoreError> {
        self.root.try_clone().map_err(|_| destination_error())
    }

    fn open_job_directory(&self, job_component: &str) -> Result<(), DesktopRestoreError> {
        let root = Dir::from_std_file(self.try_clone_root()?);
        let job = root
            .open_dir_nofollow(job_component)
            .map_err(|_| destination_error())?;
        if !job
            .dir_metadata()
            .map_err(|_| destination_error())?
            .is_dir()
        {
            return Err(destination_error());
        }
        Ok(())
    }
}

fn destination_error() -> DesktopRestoreError {
    DesktopRestoreError {
        code: "RESTORE_DESTINATION_EXPIRED",
        message: "The deterministic destination authority is unavailable.",
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn wait_terminal(coordinator: &RestoreCoordinator, job_id: &str) -> RestoreJobSnapshotDto {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let snapshot = coordinator.get_restore_job(job_id).unwrap();
        if snapshot.status.is_terminal() {
            return snapshot;
        }
        assert!(Instant::now() < deadline, "restore fixture timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn restore_fixture_recovers_intact_fragmented_and_zero_filled_files_end_to_end() {
    let intact = deterministic_bytes(0xA11C_E001, 6_000);
    let colliding = deterministic_bytes(0xA11C_E002, 7_000);
    let fragmented = deterministic_bytes(0xF12A_6001, 20_000);
    let conflicted = deterministic_bytes(0xC0F1_1C71, 16_384);

    let mut builder = NtfsImageBuilder::new("desktop-restore-vertical");
    builder.add_file(
        NodeParent::Root,
        "intact.bin",
        intact.clone(),
        true,
        FileOptions::default(),
    );
    builder.add_file(
        NodeParent::Root,
        "intact.bin",
        colliding.clone(),
        true,
        FileOptions::default(),
    );
    builder.add_file(
        NodeParent::Root,
        "fragmented.bin",
        fragmented.clone(),
        true,
        FileOptions {
            fragmented: true,
            ..Default::default()
        },
    );
    builder.add_file(
        NodeParent::Root,
        "conflicted.bin",
        conflicted.clone(),
        true,
        FileOptions {
            overwrite_ranges: vec![(4096, 4096)],
            ..Default::default()
        },
    );
    let (image, truth) = builder.build();
    assert_eq!(truth.expected_candidates.len(), 4);

    let reader = MemImageReader::new("desktop-restore-vertical", image.clone());
    let details = um_cli::scan_volume_reader(&reader).expect("fixture scan");
    let session = storage::build_scan_session(
        "scan-desktop-restore-vertical",
        "Deterministic NTFS fixture",
        details,
        SessionScope::Volume,
    )
    .expect("desktop scan authority");
    let storage = DesktopStorageState::default();
    storage.store_session(session).unwrap();

    let page = storage
        .query_candidate_page(
            "scan-desktop-restore-vertical",
            CandidateQuery {
                revision: "1".into(),
                search: String::new(),
                extensions: Vec::new(),
                kinds: Vec::new(),
                metadata_confidences: Vec::new(),
                methods: Vec::new(),
                states: Vec::new(),
                min_recoverability_score: None,
                max_recoverability_score: None,
                eligibilities: Vec::new(),
                selected_only: false,
            },
            CandidateSort {
                field: CandidateSortField::Path,
                direction: CandidateSortDirection::Ascending,
            },
            None,
        )
        .expect("real candidate query");
    assert_eq!(page.filtered_total, "4");
    let selection = storage
        .update_candidate_selection(
            "scan-desktop-restore-vertical",
            &page.query_id,
            SelectionOperationDto::SelectAllMatching,
            0,
        )
        .expect("real candidate selection");
    assert_eq!(selection.selection_revision, "1");
    let scan = storage
        .restore_snapshot("scan-desktop-restore-vertical", Some(1))
        .expect("immutable selected scan snapshot");

    let destination = tempfile::tempdir().unwrap();
    fs::write(destination.path().join("intact.bin"), b"existing sentinel").unwrap();
    let destination_file = Dir::open_ambient_dir(destination.path(), ambient_authority())
        .unwrap()
        .into_std_file();
    let coordinator = RestoreCoordinator::with_runtime(Arc::new(FixtureRuntime {
        image: Arc::new(image),
    }));
    let destination_summary = coordinator
        .admit_destination(
            &scan,
            Some(AdmittedDestination {
                label: "Deterministic destination".into(),
                volume_label: "Fixture NTFS".into(),
                capability: Arc::new(FixtureDestination {
                    root: destination_file,
                }),
            }),
        )
        .unwrap()
        .unwrap();
    let plan = coordinator
        .create_restore_plan(
            &scan,
            &destination_summary.destination_id,
            CollisionPolicyDto::Rename,
            PartialFilePolicyDto::ZeroFillAndMap,
        )
        .expect("real desktop restore plan");
    assert_eq!(plan.items_total, "4");
    assert_eq!(plan.best_effort_items, "1");

    let started = coordinator
        .start_restore(&scan, &plan.plan_id)
        .expect("real background restore job");
    let completed = wait_terminal(&coordinator, &started.job_id);
    assert_eq!(completed.status, RestoreJobStatusDto::Completed);
    assert_eq!(completed.items_completed, "4");
    assert_eq!(completed.items_failed, "0");
    assert_eq!(completed.items_cancelled, "0");

    let job_dir = destination
        .path()
        .join(um_restore::job_directory_component(&started.job_id));
    assert_eq!(
        fs::read(destination.path().join("intact.bin")).unwrap(),
        b"existing sentinel",
        "restore isolation must never clobber pre-existing destination content"
    );
    assert_eq!(
        sha256_hex(&fs::read(job_dir.join("intact.bin")).unwrap()),
        sha256_hex(&intact)
    );
    assert_eq!(
        sha256_hex(&fs::read(job_dir.join("intact (recovered 1).bin")).unwrap()),
        sha256_hex(&colliding),
        "same-path candidates must use deterministic no-clobber renaming"
    );
    assert_eq!(
        sha256_hex(&fs::read(job_dir.join("fragmented.bin")).unwrap()),
        sha256_hex(&fragmented)
    );

    let mut expected_partial = conflicted;
    expected_partial[4096..8192].fill(0);
    let partial_output = fs::read(job_dir.join("conflicted.bin")).unwrap();
    assert_eq!(sha256_hex(&partial_output), sha256_hex(&expected_partial));

    let manifest_bytes = fs::read(job_dir.join("recovery-manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest["items"].as_array().unwrap().len(), 4);
    let partial_item = manifest["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["requestedPath"] == "conflicted.bin")
        .expect("conflicted manifest item");
    assert_eq!(
        partial_item["zeroFilledRanges"],
        serde_json::json!([{
            "logicalOffset": 4096,
            "length": 4096,
            "reason": "currentlyAllocated"
        }])
    );
    let sidecar_name = partial_item["partialSidecar"]
        .as_str()
        .expect("partial sidecar name");
    let sidecar_bytes = fs::read(job_dir.join(sidecar_name)).unwrap();
    assert_eq!(
        partial_item["partialSidecarSha256"],
        sha256_hex(&sidecar_bytes)
    );
    let sidecar: serde_json::Value = serde_json::from_slice(&sidecar_bytes).unwrap();
    assert_eq!(sidecar["outputSha256"], sha256_hex(&expected_partial));
    assert_eq!(
        sidecar["conflictRanges"],
        serde_json::json!([{"logicalOffset": 4096, "length": 4096}])
    );

    let completed_json = serde_json::to_value(&completed).unwrap();
    assert_eq!(
        completed_json["manifest"]["manifestSha256"],
        sha256_hex(&manifest_bytes)
    );
    assert_eq!(completed_json["manifest"]["partialItems"], "1");
    let public_contract = completed_json.to_string();
    assert!(!public_contract.contains(&destination.path().display().to_string()));
    assert!(!public_contract.contains("physicalDisk"));
}
