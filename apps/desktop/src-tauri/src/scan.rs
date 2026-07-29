use std::path::PathBuf;

use serde::Serialize;
use tauri_plugin_dialog::DialogExt;
use um_cli::{CliError, ImageScanReport, VolumeReport, VolumeScanStatus};
use um_fs_common::ScanError;

const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &["img", "dd", "raw", "bin"];
const MAX_REQUEST_ID_SCALARS: usize = 128;
const MAX_VOLUMES: usize = 1_024;
const MAX_WARNINGS: usize = 512;
const MAX_WARNING_SCALARS: usize = 512;
const MAX_LABEL_SCALARS: usize = 255;
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const REDACTED_WARNING_TEXT: &str = "Warning text removed by the safety policy.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopScanReport {
    schema_version: u32,
    source: DesktopSourceReport,
    partition_table: String,
    volumes: Vec<DesktopVolumeReport>,
    warnings: Vec<String>,
    warning_count: String,
    warnings_omitted: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSourceReport {
    label: String,
    size_bytes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopVolumeReport {
    index: u32,
    offset_bytes: String,
    length_bytes: String,
    file_system: String,
    scan_status: String,
    candidate_count: String,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DesktopCommandError {
    code: &'static str,
    message: &'static str,
}

impl DesktopCommandError {
    const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }

    const fn internal() -> Self {
        Self::new(
            "SCAN_INTERNAL",
            "The desktop scanner could not complete the request.",
        )
    }

    const fn report_too_large() -> Self {
        Self::new(
            "SCAN_REPORT_TOO_LARGE",
            "The real scan report is too large for the desktop interface.",
        )
    }
}

/// Opens the native picker in Rust and scans the selected regular image.
///
/// A filesystem path is neither accepted from nor returned to the webview.
/// Canceling the picker is a successful `None` response.
#[tauri::command]
pub(crate) async fn select_and_scan_image(
    app: tauri::AppHandle,
    request_id: String,
) -> Result<Option<DesktopScanReport>, DesktopCommandError> {
    validate_request_id(&request_id)?;

    let selected = app
        .dialog()
        .file()
        .set_title("Select a disk image")
        .add_filter("Disk images", SUPPORTED_IMAGE_EXTENSIONS)
        .blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|_| {
        DesktopCommandError::new(
            "SOURCE_UNSUPPORTED",
            "The selected item is not a supported local image file.",
        )
    })?;

    scan_selected_path(path).await.map(Some)
}

async fn scan_selected_path(path: PathBuf) -> Result<DesktopScanReport, DesktopCommandError> {
    let result = tauri::async_runtime::spawn_blocking(move || um_cli::scan_image_path(&path))
        .await
        .map_err(|_| DesktopCommandError::internal())?;
    let report = result.map_err(map_cli_error)?;
    adapt_report(report)
}

fn validate_request_id(request_id: &str) -> Result<(), DesktopCommandError> {
    let valid = !request_id.is_empty()
        && request_id.chars().count() <= MAX_REQUEST_ID_SCALARS
        && request_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(DesktopCommandError::internal())
    }
}

fn map_cli_error(error: CliError) -> DesktopCommandError {
    match error {
        CliError::ForbiddenSourcePath => DesktopCommandError::new(
            "SOURCE_FORBIDDEN",
            "The selected source is forbidden by the read-only safety policy.",
        ),
        CliError::UnsupportedExtension { .. } => DesktopCommandError::new(
            "SOURCE_UNSUPPORTED",
            "Choose an IMG, DD, RAW, or BIN image file.",
        ),
        CliError::NotRegularFile => DesktopCommandError::new(
            "SOURCE_NOT_REGULAR",
            "The selected source must be an ordinary local file.",
        ),
        CliError::EmptyImage => {
            DesktopCommandError::new("SOURCE_EMPTY", "The selected image file is empty.")
        }
        CliError::Metadata(_)
        | CliError::Open(_)
        | CliError::Partition(ScanError::Read(_))
        | CliError::VolumeScan {
            source: ScanError::Read(_),
            ..
        } => DesktopCommandError::new(
            "SOURCE_IO",
            "The selected image could not be opened read-only.",
        ),
        CliError::Partition(_) | CliError::InvalidRegion { .. } | CliError::VolumeScan { .. } => {
            DesktopCommandError::new(
                "SCAN_CORRUPT",
                "The image contains invalid or unsupported disk structures.",
            )
        }
    }
}

fn adapt_report(report: ImageScanReport) -> Result<DesktopScanReport, DesktopCommandError> {
    if report.schema_version != 2 {
        return Err(DesktopCommandError::internal());
    }
    if report.volumes.len() > MAX_VOLUMES {
        return Err(DesktopCommandError::report_too_large());
    }

    let partition_table = match report.partition_table.as_deref() {
        Some("mbr") => "mbr",
        Some("gpt") => "gpt",
        None => "none",
        Some(_) => return Err(DesktopCommandError::internal()),
    }
    .to_string();

    let warning_count = report
        .warnings
        .len()
        .checked_add(
            report
                .volumes
                .iter()
                .try_fold(0usize, |total, volume| {
                    total.checked_add(volume.warnings.len())
                })
                .ok_or_else(DesktopCommandError::report_too_large)?,
        )
        .ok_or_else(DesktopCommandError::report_too_large)?;

    let mut warning_budget = MAX_WARNINGS;
    let warnings = take_sanitized_warnings(report.warnings, &mut warning_budget);
    let volumes = report
        .volumes
        .into_iter()
        .map(|volume| adapt_volume(volume, &mut warning_budget))
        .collect::<Result<Vec<_>, _>>()?;
    let returned_warning_count = MAX_WARNINGS - warning_budget;
    let warnings_omitted = warning_count
        .checked_sub(returned_warning_count)
        .ok_or_else(DesktopCommandError::internal)?;

    let desktop_report = DesktopScanReport {
        schema_version: 2,
        source: DesktopSourceReport {
            label: sanitize_label(&report.source.label),
            size_bytes: report.source.size_bytes.to_string(),
        },
        partition_table,
        volumes,
        warnings,
        warning_count: warning_count.to_string(),
        warnings_omitted: warnings_omitted.to_string(),
    };

    ensure_payload_within_limit(&desktop_report)?;
    Ok(desktop_report)
}

fn ensure_payload_within_limit<T: Serialize>(payload: &T) -> Result<(), DesktopCommandError> {
    let serialized = serde_json::to_vec(payload).map_err(|_| DesktopCommandError::internal())?;
    if serialized.len() > MAX_PAYLOAD_BYTES {
        return Err(DesktopCommandError::report_too_large());
    }
    Ok(())
}

fn adapt_volume(
    volume: VolumeReport,
    warning_budget: &mut usize,
) -> Result<DesktopVolumeReport, DesktopCommandError> {
    let file_system = match volume.file_system.as_str() {
        "ntfs" | "fat12" | "fat16" | "fat32" | "unrecognized" => volume.file_system,
        _ => return Err(DesktopCommandError::internal()),
    };
    let scan_status = match (file_system.as_str(), volume.scan_status) {
        ("unrecognized", VolumeScanStatus::Unrecognized) => "unrecognized",
        ("ntfs" | "fat12" | "fat16" | "fat32", VolumeScanStatus::Partial) => "partial",
        ("ntfs" | "fat12" | "fat16" | "fat32", VolumeScanStatus::Complete) => "complete",
        _ => return Err(DesktopCommandError::internal()),
    }
    .to_string();
    Ok(DesktopVolumeReport {
        index: volume.index,
        offset_bytes: volume.offset_bytes.to_string(),
        length_bytes: volume.length_bytes.to_string(),
        file_system,
        scan_status,
        candidate_count: volume.candidate_count.to_string(),
        warnings: take_sanitized_warnings(volume.warnings, warning_budget),
    })
}

fn take_sanitized_warnings(warnings: Vec<String>, budget: &mut usize) -> Vec<String> {
    let take = warnings.len().min(*budget);
    *budget -= take;
    warnings
        .into_iter()
        .take(take)
        .map(|warning| {
            let sanitized = sanitize_untrusted_text(&warning, MAX_WARNING_SCALARS);
            if sanitized.trim().is_empty() {
                REDACTED_WARNING_TEXT.to_string()
            } else {
                sanitized
            }
        })
        .collect()
}

fn sanitize_label(label: &str) -> String {
    let sanitized = sanitize_untrusted_text(label, MAX_LABEL_SCALARS);
    if sanitized.trim().is_empty() {
        "image".to_string()
    } else {
        sanitized
    }
}

fn sanitize_untrusted_text(value: &str, max_scalars: usize) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_control() && !is_bidirectional_formatting_control(*character)
        })
        .take(max_scalars)
        .collect()
}

fn is_bidirectional_formatting_control(character: char) -> bool {
    matches!(
        character,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'
            | '\u{202B}'
            | '\u{202C}'
            | '\u{202D}'
            | '\u{202E}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use super::*;
    use sha2::{Digest, Sha256};
    use um_cli::{SourceReport, VolumeReport};
    use um_core::ReadError;
    use um_fixture_builder::fat::{
        FatFileOptions, FatImageBuilder, FatKind, NodeParent as FatParent,
    };
    use um_fixture_builder::ntfs::{
        FileOptions as NtfsFileOptions, NodeParent as NtfsParent, NtfsImageBuilder,
    };
    use um_fs_common::ScanError;

    fn report_with_volume(volume: VolumeReport) -> ImageScanReport {
        ImageScanReport {
            schema_version: 2,
            source: SourceReport {
                id: "image:<redacted-path>:18446744073709551615".to_string(),
                label: "real-image.raw".to_string(),
                size_bytes: u64::MAX,
            },
            partition_table: Some("gpt".to_string()),
            volumes: vec![volume],
            warnings: Vec::new(),
        }
    }

    fn build_gpt_disk(volume: &[u8], start_lba: u64, volume_name: &str) -> Vec<u8> {
        const SECTOR_SIZE: usize = 512;
        const ENTRY_COUNT: usize = 128;
        const ENTRY_SIZE: usize = 128;
        const PRIMARY_ENTRY_LBA: usize = 2;

        assert!(!volume.is_empty());
        assert_eq!(volume.len() % SECTOR_SIZE, 0);
        assert!(start_lba >= 34);

        let volume_sectors =
            u64::try_from(volume.len() / SECTOR_SIZE).expect("fixture volume sector count");
        let last_lba = start_lba
            .checked_add(volume_sectors)
            .and_then(|value| value.checked_sub(1))
            .expect("fixture partition last LBA");
        let total_sectors = last_lba
            .checked_add(2_048)
            .expect("fixture disk sector count");
        let disk_len = total_sectors
            .checked_mul(SECTOR_SIZE as u64)
            .and_then(|value| usize::try_from(value).ok())
            .expect("fixture disk byte length");
        let mut disk = vec![0_u8; disk_len];

        let protective_sectors =
            u32::try_from(total_sectors - 1).expect("fixture protective MBR sector count");
        let mbr_entry_offset = 446;
        disk[mbr_entry_offset + 4] = 0xEE;
        disk[mbr_entry_offset + 8..mbr_entry_offset + 12].copy_from_slice(&1_u32.to_le_bytes());
        disk[mbr_entry_offset + 12..mbr_entry_offset + 16]
            .copy_from_slice(&protective_sectors.to_le_bytes());
        disk[510..512].copy_from_slice(&0xAA55_u16.to_le_bytes());

        let mut entries = vec![0_u8; ENTRY_COUNT * ENTRY_SIZE];
        let basic_data_guid = [
            0xA2, 0xA0, 0xD0, 0xEB, 0xE5, 0xB9, 0x33, 0x44, 0x87, 0xC0, 0x68, 0xB6, 0xB7, 0x26,
            0x99, 0xC7,
        ];
        entries[0..16].copy_from_slice(&basic_data_guid);
        entries[16..32].copy_from_slice(&[0x5A; 16]);
        entries[32..40].copy_from_slice(&start_lba.to_le_bytes());
        entries[40..48].copy_from_slice(&last_lba.to_le_bytes());
        for (index, code_unit) in volume_name.encode_utf16().enumerate().take(36) {
            let offset = 56 + index * 2;
            entries[offset..offset + 2].copy_from_slice(&code_unit.to_le_bytes());
        }
        let entries_crc = crc32fast::hash(&entries);

        let mut header = [0_u8; 92];
        header[0..8].copy_from_slice(b"EFI PART");
        header[8..12].copy_from_slice(&0x0001_0000_u32.to_le_bytes());
        header[12..16].copy_from_slice(&92_u32.to_le_bytes());
        header[24..32].copy_from_slice(&1_u64.to_le_bytes());
        header[32..40].copy_from_slice(&(total_sectors - 1).to_le_bytes());
        header[40..48].copy_from_slice(&34_u64.to_le_bytes());
        header[48..56].copy_from_slice(&(total_sectors - 34).to_le_bytes());
        header[56..72].copy_from_slice(&[0xA5; 16]);
        header[72..80].copy_from_slice(&(PRIMARY_ENTRY_LBA as u64).to_le_bytes());
        header[80..84].copy_from_slice(&(ENTRY_COUNT as u32).to_le_bytes());
        header[84..88].copy_from_slice(&(ENTRY_SIZE as u32).to_le_bytes());
        header[88..92].copy_from_slice(&entries_crc.to_le_bytes());
        let header_crc = crc32fast::hash(&header);
        header[16..20].copy_from_slice(&header_crc.to_le_bytes());

        disk[SECTOR_SIZE..SECTOR_SIZE + header.len()].copy_from_slice(&header);
        let entries_offset = PRIMARY_ENTRY_LBA * SECTOR_SIZE;
        disk[entries_offset..entries_offset + entries.len()].copy_from_slice(&entries);
        let volume_offset = usize::try_from(start_lba).expect("fixture start LBA") * SECTOR_SIZE;
        disk[volume_offset..volume_offset + volume.len()].copy_from_slice(volume);
        disk
    }

    fn windows_raw_device_test_path() -> PathBuf {
        let namespace = ['\\', '\\', '.', '\\'].into_iter().collect::<String>();
        let selector = ["Physical", "Drive", "0.img"].concat();
        PathBuf::from(namespace + &selector)
    }

    #[test]
    fn desktop_u64_ipc_001_maps_large_integers_to_exact_decimal_strings() {
        let report = adapt_report(report_with_volume(VolumeReport {
            index: 7,
            offset_bytes: (1_u64 << 53) - 1,
            length_bytes: u64::MAX,
            file_system: "ntfs".to_string(),
            scan_status: VolumeScanStatus::Complete,
            candidate_count: usize::MAX,
            warnings: Vec::new(),
        }))
        .expect("adapt real report");

        assert_eq!(report.source.size_bytes, u64::MAX.to_string());
        assert_eq!(report.volumes[0].offset_bytes, "9007199254740991");
        assert_eq!(report.volumes[0].length_bytes, u64::MAX.to_string());
        assert_eq!(report.volumes[0].candidate_count, usize::MAX.to_string());
        assert_eq!(report.volumes[0].scan_status, "complete");

        let json = serde_json::to_value(report).expect("serialize report");
        assert!(json["source"]["sizeBytes"].is_string());
        assert!(json["volumes"][0]["offsetBytes"].is_string());
        assert!(json["volumes"][0]["candidateCount"].is_string());
    }

    #[test]
    fn desktop_u64_ipc_002_maps_two_to_the_53_exactly() {
        let report = adapt_report(report_with_volume(VolumeReport {
            index: 0,
            offset_bytes: 1_u64 << 53,
            length_bytes: 1,
            file_system: "unrecognized".to_string(),
            scan_status: VolumeScanStatus::Unrecognized,
            candidate_count: 0,
            warnings: Vec::new(),
        }))
        .expect("adapt report");

        assert_eq!(report.volumes[0].offset_bytes, "9007199254740992");
    }

    #[test]
    fn desktop_warning_text_001_strips_controls_bounds_text_and_reports_omissions() {
        let volume_warnings = (0..MAX_WARNINGS)
            .map(|index| format!("volume-{index}"))
            .collect();
        let mut report = report_with_volume(VolumeReport {
            index: 0,
            offset_bytes: 0,
            length_bytes: 1,
            file_system: "fat32".to_string(),
            scan_status: VolumeScanStatus::Complete,
            candidate_count: 0,
            warnings: volume_warnings,
        });
        report.warnings = vec![format!("root\u{0000}\n{}", "x".repeat(700))];

        let desktop = adapt_report(report).expect("adapt bounded warnings");

        assert_eq!(desktop.warning_count, "513");
        assert_eq!(desktop.warnings_omitted, "1");
        assert_eq!(desktop.warnings.len(), 1);
        assert_eq!(desktop.warnings[0].chars().count(), MAX_WARNING_SCALARS);
        assert!(!desktop.warnings[0].chars().any(char::is_control));
        assert_eq!(desktop.volumes[0].warnings.len(), MAX_WARNINGS - 1);
    }

    #[test]
    fn desktop_warning_text_002_replaces_an_empty_sanitized_warning() {
        let mut report = report_with_volume(VolumeReport {
            index: 0,
            offset_bytes: 0,
            length_bytes: 1,
            file_system: "ntfs".to_string(),
            scan_status: VolumeScanStatus::Complete,
            candidate_count: 0,
            warnings: Vec::new(),
        });
        report.warnings = vec!["\u{0000}\n\t".to_string()];

        let desktop = adapt_report(report).expect("adapt warning");

        assert_eq!(desktop.warning_count, "1");
        assert_eq!(desktop.warnings_omitted, "0");
        assert_eq!(desktop.warnings, [REDACTED_WARNING_TEXT]);
    }

    #[test]
    fn desktop_error_privacy_001_never_serializes_raw_io_details_or_path() {
        let secret = r"C:\private\UNIQUE-SECRET-MARKER\source.img";
        let cli_error = CliError::Open(io::Error::other(format!(
            "access denied while opening {secret}"
        )));

        let json = serde_json::to_string(&map_cli_error(cli_error)).expect("serialize error");

        assert_eq!(
            json,
            r#"{"code":"SOURCE_IO","message":"The selected image could not be opened read-only."}"#
        );
        assert!(!json.contains("UNIQUE-SECRET-MARKER"));
        assert!(!json.contains("access denied"));
    }

    #[test]
    fn desktop_partition_read_error_001_maps_to_source_io_without_details() {
        let cli_error = CliError::Partition(ScanError::Read(ReadError::Io {
            offset: 4_096,
            message: "UNIQUE-PARTITION-READ-SECRET".to_string(),
        }));

        let json = serde_json::to_string(&map_cli_error(cli_error)).expect("serialize error");

        assert_eq!(
            json,
            r#"{"code":"SOURCE_IO","message":"The selected image could not be opened read-only."}"#
        );
        assert!(!json.contains("UNIQUE-PARTITION-READ-SECRET"));
        assert!(!json.contains("4096"));
    }

    #[test]
    fn desktop_volume_scan_error_001_maps_typed_failures_without_details() {
        let read_error = CliError::VolumeScan {
            index: 3,
            file_system: "NTFS",
            source: ScanError::Read(ReadError::Io {
                offset: 8_192,
                message: "UNIQUE-VOLUME-READ-SECRET".to_string(),
            }),
        };
        let corrupt_error = CliError::VolumeScan {
            index: 4,
            file_system: "NTFS",
            source: ScanError::Corrupt("UNIQUE-VOLUME-CORRUPT-SECRET".to_string()),
        };

        let read_json =
            serde_json::to_string(&map_cli_error(read_error)).expect("serialize read error");
        let corrupt_json =
            serde_json::to_string(&map_cli_error(corrupt_error)).expect("serialize corrupt error");

        assert!(read_json.contains(r#""code":"SOURCE_IO""#));
        assert!(corrupt_json.contains(r#""code":"SCAN_CORRUPT""#));
        assert!(!read_json.contains("UNIQUE-VOLUME-READ-SECRET"));
        assert!(!corrupt_json.contains("UNIQUE-VOLUME-CORRUPT-SECRET"));
    }

    #[test]
    fn desktop_bidi_text_001_strips_unicode_bidirectional_formatting_controls() {
        let controls = [
            '\u{061C}', '\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}',
            '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ];
        let hostile = format!(
            "invoice{}cod.exe{}",
            '\u{202E}',
            controls.iter().collect::<String>()
        );

        let sanitized = sanitize_untrusted_text(&hostile, MAX_LABEL_SCALARS);

        assert_eq!(sanitized, "invoicecod.exe");
        assert!(controls.iter().all(|control| !sanitized.contains(*control)));
    }

    #[test]
    fn desktop_request_id_001_accepts_only_a_bounded_opaque_token() {
        for request_id in ["1", "request-42", "5e223419_0f63"] {
            assert!(validate_request_id(request_id).is_ok());
        }
        for request_id in ["", "../image.img", "contains space", "é", &"x".repeat(129)] {
            let error = validate_request_id(request_id).expect_err("reject request id");
            assert_eq!(error.code, "SCAN_INTERNAL");
        }
    }

    #[test]
    fn desktop_report_schema_001_rejects_unknown_backend_values() {
        let mut report = report_with_volume(VolumeReport {
            index: 0,
            offset_bytes: 0,
            length_bytes: 1,
            file_system: "ext4".to_string(),
            scan_status: VolumeScanStatus::Complete,
            candidate_count: 0,
            warnings: Vec::new(),
        });
        assert_eq!(
            adapt_report(report.clone()).expect_err("reject filesystem"),
            DesktopCommandError::internal()
        );

        report.volumes[0].file_system = "ntfs".to_string();
        report.partition_table = Some("apm".to_string());
        assert_eq!(
            adapt_report(report).expect_err("reject partition table"),
            DesktopCommandError::internal()
        );

        let mut report = report_with_volume(VolumeReport {
            index: 0,
            offset_bytes: 0,
            length_bytes: 1,
            file_system: "ntfs".to_string(),
            scan_status: VolumeScanStatus::Complete,
            candidate_count: 0,
            warnings: Vec::new(),
        });
        report.schema_version = 3;
        assert_eq!(
            adapt_report(report).expect_err("reject incompatible schema"),
            DesktopCommandError::internal()
        );
    }

    #[test]
    fn desktop_scan_status_001_preserves_ntfs_and_fat_partial_status() {
        let partial = adapt_report(report_with_volume(VolumeReport {
            index: 0,
            offset_bytes: 0,
            length_bytes: 1,
            file_system: "ntfs".to_string(),
            scan_status: VolumeScanStatus::Partial,
            candidate_count: 0,
            warnings: vec!["MFT scan stopped at the configured work budget".to_string()],
        }))
        .expect("adapt partial NTFS report");
        assert_eq!(partial.volumes[0].scan_status, "partial");

        let partial_fat = adapt_report(report_with_volume(VolumeReport {
            index: 0,
            offset_bytes: 0,
            length_bytes: 1,
            file_system: "fat32".to_string(),
            scan_status: VolumeScanStatus::Partial,
            candidate_count: 0,
            warnings: vec!["directory chain incomplete".to_string()],
        }))
        .expect("adapt partial FAT report");
        assert_eq!(partial_fat.volumes[0].scan_status, "partial");
    }

    #[test]
    fn desktop_report_bound_001_rejects_too_many_structural_volume_records() {
        let volume = VolumeReport {
            index: 0,
            offset_bytes: 0,
            length_bytes: 1,
            file_system: "unrecognized".to_string(),
            scan_status: VolumeScanStatus::Unrecognized,
            candidate_count: 0,
            warnings: Vec::new(),
        };
        let mut report = report_with_volume(volume.clone());
        report.volumes = vec![volume; MAX_VOLUMES + 1];

        assert_eq!(
            adapt_report(report).expect_err("reject oversized report"),
            DesktopCommandError::report_too_large()
        );
    }

    #[test]
    fn desktop_payload_limit_001_enforces_the_real_one_mib_serialized_limit() {
        #[derive(Serialize)]
        struct PayloadProbe {
            value: String,
        }

        let below_limit = PayloadProbe {
            value: "x".repeat(MAX_PAYLOAD_BYTES - 32),
        };
        let above_limit = PayloadProbe {
            value: "x".repeat(MAX_PAYLOAD_BYTES),
        };

        ensure_payload_within_limit(&below_limit).expect("payload below one MiB");
        assert_eq!(
            ensure_payload_within_limit(&above_limit).expect_err("payload above one MiB"),
            DesktopCommandError::report_too_large()
        );
    }

    #[test]
    fn desktop_drag_drop_off_001_configures_one_real_application_window() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let windows = config["app"]["windows"].as_array().expect("windows array");
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0]["label"], "main");
        assert_eq!(windows[0]["dragDropEnabled"], false);
        assert_eq!(config["bundle"]["active"], false);
    }

    #[test]
    fn desktop_csp_001_allows_only_local_assets_and_tauri_ipc() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let csp = config["app"]["security"]["csp"]
            .as_str()
            .expect("CSP string");
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("connect-src ipc: http://ipc.localhost"));
        assert!(csp.contains("object-src 'none'"));
        assert!(!csp.contains("https:"));
        assert!(!csp.contains("http: "));
    }

    #[test]
    fn desktop_capability_min_001_grants_no_plugin_or_host_permissions() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json"))
                .expect("valid capability");
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert_eq!(capability["permissions"], serde_json::json!([]));
    }

    #[test]
    fn desktop_as_invoker_001_embeds_an_unelevated_windows_manifest() {
        let manifest = include_str!("../windows-app-manifest.xml");
        assert!(manifest.contains(r#"requestedExecutionLevel level="asInvoker""#));
        assert!(!manifest.contains("requireAdministrator"));
        assert!(!manifest.contains("highestAvailable"));
    }

    #[test]
    fn desktop_picker_filter_001_lists_only_supported_regular_image_extensions() {
        assert_eq!(SUPPORTED_IMAGE_EXTENSIONS, ["img", "dd", "raw", "bin"]);
    }

    #[test]
    fn desktop_cli_parity_001_scans_a_synthetic_fixture_without_modifying_it() {
        let mut builder = FatImageBuilder::new("desktop-real-fat16", FatKind::Fat16);
        builder.add_file(
            FatParent::Root,
            "deleted.txt",
            b"real deterministic desktop scan".to_vec(),
            true,
            FatFileOptions::default(),
        );
        let (image, manifest) = builder.build();
        let temp = tempfile::tempdir().expect("temporary fixture directory");
        let path = temp.path().join("desktop-fixture.img");
        fs::write(&path, &image).expect("write synthetic image");

        let before_bytes = fs::read(&path).expect("read fixture before scan");
        let before_sha256 = Sha256::digest(&before_bytes);
        let cli = um_cli::scan_image_path(&path).expect("direct CLI library scan");
        let desktop = tauri::async_runtime::block_on(scan_selected_path(path.clone()))
            .expect("desktop blocking-worker scan");
        let after_bytes = fs::read(&path).expect("read fixture after scan");
        let after_sha256 = Sha256::digest(&after_bytes);

        assert_eq!(after_bytes, before_bytes);
        assert_eq!(after_sha256, before_sha256);
        assert_eq!(desktop.source.label, cli.source.label);
        assert_eq!(desktop.source.size_bytes, cli.source.size_bytes.to_string());
        assert_eq!(desktop.partition_table, "none");
        assert_eq!(desktop.volumes.len(), cli.volumes.len());
        assert_eq!(desktop.volumes[0].index, cli.volumes[0].index);
        assert_eq!(
            desktop.volumes[0].offset_bytes,
            cli.volumes[0].offset_bytes.to_string()
        );
        assert_eq!(
            desktop.volumes[0].length_bytes,
            cli.volumes[0].length_bytes.to_string()
        );
        assert_eq!(desktop.volumes[0].file_system, "fat16");
        assert_eq!(
            desktop.volumes[0].candidate_count,
            manifest.expected_candidates.len().to_string()
        );
        assert_eq!(
            desktop.volumes[0].candidate_count,
            cli.volumes[0].candidate_count.to_string()
        );
    }

    #[test]
    fn desktop_backend_path_001_revalidates_bypassed_picker_inputs_in_rust() {
        let temp = tempfile::tempdir().expect("temporary test directory");
        let unsupported = temp.path().join("source.txt");
        fs::write(&unsupported, b"not an image").expect("write unsupported fixture");
        let empty = temp.path().join("empty.img");
        fs::write(&empty, []).expect("write empty fixture");

        let cases = [
            (unsupported, "SOURCE_UNSUPPORTED"),
            (temp.path().to_path_buf(), "SOURCE_NOT_REGULAR"),
            (empty, "SOURCE_EMPTY"),
            (windows_raw_device_test_path(), "SOURCE_FORBIDDEN"),
            (
                PathBuf::from(r"\\server\share\source.img"),
                "SOURCE_FORBIDDEN",
            ),
            (
                PathBuf::from(r"C:\images\source.img:stream"),
                "SOURCE_FORBIDDEN",
            ),
        ];

        for (path, expected_code) in cases {
            let error = tauri::async_runtime::block_on(scan_selected_path(path))
                .expect_err("unsafe or unsupported path must fail");
            assert_eq!(error.code, expected_code);
        }
    }

    #[test]
    fn desktop_cli_parity_002_scans_a_partitioned_ntfs_fixture_without_modifying_it() {
        let mut builder = NtfsImageBuilder::new("desktop-mbr-ntfs");
        builder.add_file(
            NtfsParent::Root,
            "deleted-note.txt",
            b"deterministic partitioned NTFS desktop scan".to_vec(),
            true,
            NtfsFileOptions::default(),
        );
        let (volume, manifest) = builder.build();
        assert_eq!(volume.len() % 512, 0);
        let start_lba = 2_048_u32;
        let sector_count = u32::try_from(volume.len() / 512).expect("fixture sector count");
        let mut disk = vec![0_u8; (start_lba as usize + sector_count as usize) * 512];
        let entry = 446_usize;
        disk[entry + 4] = 0x07;
        disk[entry + 8..entry + 12].copy_from_slice(&start_lba.to_le_bytes());
        disk[entry + 12..entry + 16].copy_from_slice(&sector_count.to_le_bytes());
        disk[510..512].copy_from_slice(&0xAA55_u16.to_le_bytes());
        disk[start_lba as usize * 512..].copy_from_slice(&volume);

        let temp = tempfile::tempdir().expect("temporary fixture directory");
        let path = temp.path().join("partitioned-ntfs.raw");
        fs::write(&path, &disk).expect("write synthetic partitioned image");
        let before = fs::read(&path).expect("read image before scan");
        let before_sha256 = Sha256::digest(&before);

        let cli = um_cli::scan_image_path(&path).expect("direct CLI partitioned scan");
        let desktop = tauri::async_runtime::block_on(scan_selected_path(path.clone()))
            .expect("desktop partitioned scan");

        let after = fs::read(&path).expect("read image after scan");
        assert_eq!(after, before);
        assert_eq!(Sha256::digest(&after), before_sha256);
        assert_eq!(desktop.partition_table, "mbr");
        assert_eq!(desktop.volumes.len(), 1);
        assert_eq!(desktop.volumes[0].file_system, "ntfs");
        assert_eq!(
            desktop.volumes[0].offset_bytes,
            (u64::from(start_lba) * 512).to_string()
        );
        assert_eq!(
            desktop.volumes[0].candidate_count,
            manifest.expected_candidates.len().to_string()
        );
        assert_eq!(
            desktop.volumes[0].candidate_count,
            cli.volumes[0].candidate_count.to_string()
        );
        assert_eq!(
            desktop.volumes[0].length_bytes,
            cli.volumes[0].length_bytes.to_string()
        );
    }

    #[test]
    fn desktop_cli_parity_003_scans_a_deterministic_gpt_fixture_without_modifying_it() {
        let mut builder = NtfsImageBuilder::new("desktop-gpt-ntfs");
        builder.add_file(
            NtfsParent::Root,
            "deleted-gpt-note.txt",
            b"deterministic GPT Tauri parity fixture".to_vec(),
            true,
            NtfsFileOptions::default(),
        );
        let (volume, manifest) = builder.build();
        let start_lba = 2_048_u64;
        let disk = build_gpt_disk(&volume, start_lba, "Evidence");
        let temp = tempfile::tempdir().expect("temporary GPT fixture directory");
        let path = temp.path().join("partitioned-gpt-ntfs.img");
        fs::write(&path, &disk).expect("write synthetic GPT image");
        let before = fs::read(&path).expect("read GPT image before scan");
        let before_sha256 = Sha256::digest(&before);

        let cli = um_cli::scan_image_path(&path).expect("direct CLI GPT scan");
        let expected_desktop = adapt_report(cli).expect("adapt direct CLI GPT report");
        let desktop = tauri::async_runtime::block_on(scan_selected_path(path.clone()))
            .expect("desktop GPT scan");

        let after = fs::read(&path).expect("read GPT image after scan");
        assert_eq!(after, before);
        assert_eq!(Sha256::digest(&after), before_sha256);
        assert_eq!(desktop, expected_desktop);
        assert_eq!(desktop.partition_table, "gpt");
        assert_eq!(desktop.volumes.len(), 1);
        assert_eq!(desktop.volumes[0].file_system, "ntfs");
        assert_eq!(
            desktop.volumes[0].offset_bytes,
            (start_lba * 512).to_string()
        );
        assert_eq!(
            desktop.volumes[0].candidate_count,
            manifest.expected_candidates.len().to_string()
        );
    }

    #[cfg(windows)]
    #[test]
    fn desktop_reparse_guard_001_rejects_a_temporary_junction_before_scan() {
        let mut builder = FatImageBuilder::new("desktop-junction-fat16", FatKind::Fat16);
        builder.add_file(
            FatParent::Root,
            "deleted.txt",
            b"junction guard fixture".to_vec(),
            true,
            FatFileOptions::default(),
        );
        let (image, _) = builder.build();
        let temp = tempfile::tempdir().expect("temporary junction fixture directory");
        let target = temp.path().join("target");
        let junction = temp.path().join("junction");
        fs::create_dir(&target).expect("create junction target directory");
        let target_image = target.join("source.img");
        fs::write(&target_image, image).expect("write synthetic image behind junction");
        let before_sha256 =
            Sha256::digest(fs::read(&target_image).expect("read junction fixture before scan"));

        let output = std::process::Command::new("cmd")
            .args([
                "/d",
                "/c",
                "mklink",
                "/J",
                junction.to_str().expect("junction path is UTF-8"),
                target.to_str().expect("target path is UTF-8"),
            ])
            .output()
            .expect("create disposable test junction");
        assert!(
            output.status.success(),
            "failed to create disposable test junction: {output:?}"
        );

        let error = tauri::async_runtime::block_on(scan_selected_path(junction.join("source.img")))
            .expect_err("junction source must fail closed");

        assert_eq!(error.code, "SOURCE_FORBIDDEN");
        assert_eq!(
            Sha256::digest(
                fs::read(&target_image).expect("read junction fixture after rejected scan")
            ),
            before_sha256
        );
    }
}
