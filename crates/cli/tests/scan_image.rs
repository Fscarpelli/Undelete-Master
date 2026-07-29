use std::fs;
use std::path::Path;
use std::process::Command;

use um_cli::{scan_image_path, CliError};
use um_fixture_builder::fat::{FatFileOptions, FatImageBuilder, FatKind, NodeParent as FatParent};
use um_fixture_builder::ntfs::{FileOptions, NodeParent as NtfsParent, NtfsImageBuilder};

fn write_fixture(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes).expect("write deterministic fixture");
    path
}

#[test]
fn cli_image_ntfs_001_reports_deterministic_candidates_without_full_path() {
    let mut builder = NtfsImageBuilder::new("cli-ntfs");
    builder.add_file(
        NtfsParent::Root,
        "deleted-note.txt",
        b"deterministic NTFS content".to_vec(),
        true,
        FileOptions::default(),
    );
    let (image, manifest) = builder.build();
    let temp = tempfile::tempdir().unwrap();
    let path = write_fixture(temp.path(), "fixture.img", &image);

    let report = scan_image_path(&path).expect("scan NTFS fixture");
    assert_eq!(report.schema_version, 1);
    assert_eq!(report.source.label, "fixture.img");
    assert_eq!(report.source.size_bytes, image.len() as u64);
    assert_eq!(report.volumes.len(), 1);
    assert_eq!(report.volumes[0].file_system, "ntfs");
    assert_eq!(
        report.volumes[0].candidate_count,
        manifest.expected_candidates.len()
    );

    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains(&temp.path().display().to_string()));
    assert!(!report
        .source
        .id
        .contains(&temp.path().display().to_string()));
}

#[test]
fn cli_image_fat_001_reports_deterministic_candidates() {
    let mut builder = FatImageBuilder::new("cli-fat16", FatKind::Fat16);
    builder.add_file(
        FatParent::Root,
        "deleted.csv",
        b"one,two\n1,2\n".to_vec(),
        true,
        FatFileOptions::default(),
    );
    let (image, manifest) = builder.build();
    let temp = tempfile::tempdir().unwrap();
    let path = write_fixture(temp.path(), "fixture.DD", &image);

    let report = scan_image_path(&path).expect("scan FAT fixture");
    assert_eq!(report.volumes.len(), 1);
    assert_eq!(report.volumes[0].file_system, "fat16");
    assert_eq!(
        report.volumes[0].candidate_count,
        manifest.expected_candidates.len()
    );
}

#[test]
fn cli_image_partition_001_scans_only_the_discovered_mbr_region() {
    let mut builder = FatImageBuilder::new("cli-mbr-fat16", FatKind::Fat16);
    builder.add_file(
        FatParent::Root,
        "inside.txt",
        b"bounded partition scan".to_vec(),
        true,
        FatFileOptions::default(),
    );
    let (volume, manifest) = builder.build();
    let start_lba = 2048u32;
    let sector_count = u32::try_from(volume.len() / 512).unwrap();
    let mut disk = vec![0u8; (start_lba as usize + sector_count as usize) * 512];
    let entry = 446usize;
    disk[entry + 4] = 0x06;
    disk[entry + 8..entry + 12].copy_from_slice(&start_lba.to_le_bytes());
    disk[entry + 12..entry + 16].copy_from_slice(&sector_count.to_le_bytes());
    disk[510..512].copy_from_slice(&0xAA55u16.to_le_bytes());
    disk[start_lba as usize * 512..].copy_from_slice(&volume);

    let temp = tempfile::tempdir().unwrap();
    let path = write_fixture(temp.path(), "partitioned.bin", &disk);
    let report = scan_image_path(&path).expect("scan bounded MBR partition");

    assert_eq!(report.partition_table.as_deref(), Some("mbr"));
    assert_eq!(report.volumes.len(), 1);
    assert_eq!(report.volumes[0].offset_bytes, start_lba as u64 * 512);
    assert_eq!(report.volumes[0].length_bytes, volume.len() as u64);
    assert_eq!(report.volumes[0].file_system, "fat16");
    assert_eq!(
        report.volumes[0].candidate_count,
        manifest.expected_candidates.len()
    );
}

#[test]
fn cli_image_ext_001_rejects_unsupported_extension_before_scanning() {
    let temp = tempfile::tempdir().unwrap();
    let path = write_fixture(temp.path(), "not-an-image.txt", b"not a disk image");

    assert!(matches!(
        scan_image_path(&path),
        Err(CliError::UnsupportedExtension { .. })
    ));
}

#[test]
fn cli_image_regular_001_rejects_a_directory() {
    let temp = tempfile::tempdir().unwrap();

    assert!(matches!(
        scan_image_path(temp.path()),
        Err(CliError::NotRegularFile)
    ));
}

#[test]
fn cli_process_json_001_emits_machine_readable_json() {
    let mut builder = FatImageBuilder::new("cli-process", FatKind::Fat16);
    builder.add_file(
        FatParent::Root,
        "report.txt",
        b"process smoke".to_vec(),
        true,
        FatFileOptions::default(),
    );
    let (image, _) = builder.build();
    let temp = tempfile::tempdir().unwrap();
    let path = write_fixture(temp.path(), "process.raw", &image);

    let output = Command::new(env!("CARGO_BIN_EXE_undelete-master"))
        .args(["scan-image", path.to_str().unwrap(), "--pretty"])
        .output()
        .expect("run CLI");

    assert!(output.status.success(), "{output:?}");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["schemaVersion"], 1);
    assert_eq!(json["volumes"][0]["fileSystem"], "fat16");
    assert!(output.stderr.is_empty());
}

#[test]
fn cli_process_help_001_prints_usage_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_undelete-master"))
        .arg("--help")
        .output()
        .expect("run CLI help");

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("scan-image"));
    assert!(stdout.contains(".img"));
    assert!(output.stderr.is_empty());
}
