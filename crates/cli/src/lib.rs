//! Safe, non-elevated entry point for scanning ordinary disk-image files.
//!
//! This crate never opens physical devices, extracts content, or writes to a
//! scan source. Reports contain metadata only and redact the supplied path.

#![forbid(unsafe_code)]

mod deep;
mod report;

use std::io;
use std::path::Path;

pub use report::{
    ImageScanReport, JpegCarveCoverage, MftScanCoverage, SourceReport, VolumeReport,
    VolumeScanStatus,
};
use thiserror::Error;
pub use um_carving::CarveEvidence;
use um_core::{Candidate, Region, SourceReader};
use um_fs_common::ScanError;
use um_fs_fat::{scan_fat, FatVariant};
use um_fs_ntfs::{
    scan_ntfs_with_progress, NtfsNamespace, NtfsScanCoverage, NtfsScanProgress,
    NtfsScanProgressPhase,
};
use um_io_common::{validate_local_regular_file, FileImageReader, RegionReader, SourcePathError};
use um_partition::{discover, PartitionTableKind};

const SUPPORTED_EXTENSIONS: &[&str] = &["img", "dd", "raw", "bin"];

/// Real scanner details retained by native desktop/session adapters.
///
/// Candidate metadata and NTFS namespace evidence remain in Rust; the image
/// CLI continues to serialize only its existing aggregate report.
#[derive(Debug)]
pub struct VolumeScanDetails {
    pub report: VolumeReport,
    pub candidates: Vec<Candidate>,
    pub ntfs_namespace: Option<NtfsNamespace>,
    pub carve_evidence: Vec<CarveEvidence>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VolumeScanMode {
    #[default]
    MetadataOnly,
    DeepJpeg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeScanProgressPhase {
    Bootstrap,
    MftRecords,
    Namespace,
    Candidates,
    DeepJpeg,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeScanProgress {
    pub phase: VolumeScanProgressPhase,
    pub completed: u64,
    pub total: u64,
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("source path uses a forbidden device, network, or special-file spelling")]
    ForbiddenSourcePath,
    #[error("unsupported image extension '{extension}'")]
    UnsupportedExtension { extension: String },
    #[error("source must be an existing regular file, not a directory, symlink, or device")]
    NotRegularFile,
    #[error("image file is empty")]
    EmptyImage,
    #[error("image metadata could not be read: {0}")]
    Metadata(io::Error),
    #[error("image could not be opened read-only: {0}")]
    Open(io::Error),
    #[error("partition discovery failed: {0}")]
    Partition(ScanError),
    #[error("partition {index} is outside the image bounds")]
    InvalidRegion { index: u32 },
    #[error("{file_system} scan failed safely for volume {index}")]
    VolumeScan {
        index: u32,
        file_system: &'static str,
        #[source]
        source: ScanError,
    },
    #[error("JPEG deep scan failed safely for volume {index}")]
    DeepScan {
        index: u32,
        #[source]
        source: um_carving::CarveError,
    },
    #[error("JPEG deep scan produced invalid bounded evidence for volume {index}")]
    DeepScanResult { index: u32 },
}

/// Scans one ordinary image file and returns a sanitized metadata report.
pub fn scan_image_path(path: &Path) -> Result<ImageScanReport, CliError> {
    if path_is_forbidden_source(path) {
        return Err(CliError::ForbiddenSourcePath);
    }
    let path = validate_local_regular_file(path).map_err(|error| match error {
        SourcePathError::Forbidden => CliError::ForbiddenSourcePath,
        SourcePathError::NotRegular => CliError::NotRegularFile,
        SourcePathError::Metadata(error) => CliError::Metadata(error),
    })?;
    if path_is_forbidden_source(&path) {
        return Err(CliError::ForbiddenSourcePath);
    }
    validate_extension(&path)?;

    let reader = FileImageReader::open(&path).map_err(CliError::Open)?;
    if reader.len() == 0 {
        return Err(CliError::EmptyImage);
    }

    let label = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_string());
    let source = SourceReport {
        id: format!("image:<redacted-path>:{}", reader.len()),
        label,
        size_bytes: reader.len(),
    };

    let (partition_table, regions, warnings) = match discover(&reader) {
        Ok(table) => {
            let kind = match table.kind {
                PartitionTableKind::Mbr => "mbr",
                PartitionTableKind::Gpt => "gpt",
            };
            let regions = table
                .partitions
                .into_iter()
                .map(|partition| (partition.index, partition.region))
                .collect();
            (Some(kind.to_string()), regions, table.warnings)
        }
        Err(ScanError::NotRecognized(_)) => {
            let region = Region::new(0, reader.len()).ok_or(CliError::EmptyImage)?;
            (None, vec![(0, region)], Vec::new())
        }
        Err(error) => return Err(CliError::Partition(error)),
    };

    let volumes = regions
        .into_iter()
        .map(|(index, region)| {
            let bounded =
                RegionReader::new(&reader, region).ok_or(CliError::InvalidRegion { index })?;
            scan_volume(index, region, &bounded)
        })
        .collect::<Result<Vec<_>, CliError>>()?;

    Ok(ImageScanReport {
        schema_version: 2,
        source,
        partition_table,
        volumes,
        warnings,
    })
}

/// Scans one already-open, read-only logical volume.
///
/// This entry point never opens a path or device. The caller owns the
/// `SourceReader` authority and must supply a reader whose length is exactly
/// the selected volume boundary.
pub fn scan_volume_reader(reader: &dyn SourceReader) -> Result<VolumeScanDetails, CliError> {
    scan_volume_reader_with_mode(reader, VolumeScanMode::MetadataOnly)
}

pub fn scan_volume_reader_with_mode(
    reader: &dyn SourceReader,
    mode: VolumeScanMode,
) -> Result<VolumeScanDetails, CliError> {
    scan_volume_reader_with_progress(reader, mode, |_| {})
}

pub fn scan_volume_reader_with_progress<F>(
    reader: &dyn SourceReader,
    mode: VolumeScanMode,
    progress: F,
) -> Result<VolumeScanDetails, CliError>
where
    F: FnMut(VolumeScanProgress),
{
    let region = Region::new(0, reader.len()).ok_or(CliError::EmptyImage)?;
    scan_volume_details_with_progress(0, region, reader, mode, progress)
}

fn validate_extension(path: &Path) -> Result<(), CliError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        Ok(())
    } else {
        Err(CliError::UnsupportedExtension { extension })
    }
}

fn path_is_forbidden_source(path: &Path) -> bool {
    let normalized = path.as_os_str().to_string_lossy().replace('/', "\\");
    let upper_path = normalized.to_uppercase();

    // The current slice deliberately excludes UNC, Win32 device, extended,
    // and NT object-manager namespaces. This is conservative: local
    // drive-backed regular files remain supported, while named pipes, volume
    // handles, GLOBALROOT, and similar selectors cannot reach `open`.
    if upper_path.starts_with(r"\\")
        || upper_path.starts_with(r"\??\")
        || upper_path.starts_with(r"\DEVICE\")
        || upper_path.starts_with(r"\GLOBALROOT\")
    {
        return true;
    }

    normalized
        .split('\\')
        .filter(|component| !component.is_empty())
        .any(component_is_reserved_windows_name)
}

fn component_is_reserved_windows_name(component: &str) -> bool {
    // Ignore the drive designator but reject alternate-stream syntax in every
    // other component.
    if component.len() == 2 && component.as_bytes()[1] == b':' {
        return false;
    }
    if component.contains(':') {
        return true;
    }

    let trimmed = component.trim_end_matches([' ', '.']);
    let basename = trimmed
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.'])
        .to_uppercase();
    if matches!(
        basename.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
    ) {
        return true;
    }

    for prefix in ["COM", "LPT"] {
        if let Some(suffix) = basename.strip_prefix(prefix) {
            if matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
                || matches!(suffix, "¹" | "²" | "³")
            {
                return true;
            }
        }
    }
    false
}

fn scan_volume(
    index: u32,
    region: Region,
    reader: &dyn SourceReader,
) -> Result<VolumeReport, CliError> {
    scan_volume_details(index, region, reader).map(|details| details.report)
}

fn scan_volume_details(
    index: u32,
    region: Region,
    reader: &dyn SourceReader,
) -> Result<VolumeScanDetails, CliError> {
    scan_volume_details_with_mode(index, region, reader, VolumeScanMode::MetadataOnly)
}

fn scan_volume_details_with_mode(
    index: u32,
    region: Region,
    reader: &dyn SourceReader,
    mode: VolumeScanMode,
) -> Result<VolumeScanDetails, CliError> {
    scan_volume_details_with_progress(index, region, reader, mode, |_| {})
}

fn scan_volume_details_with_progress<F>(
    index: u32,
    region: Region,
    reader: &dyn SourceReader,
    mode: VolumeScanMode,
    mut progress: F,
) -> Result<VolumeScanDetails, CliError>
where
    F: FnMut(VolumeScanProgress),
{
    match scan_ntfs_with_progress(reader, |item: NtfsScanProgress| {
        let phase = match item.phase {
            NtfsScanProgressPhase::Bootstrap => VolumeScanProgressPhase::Bootstrap,
            NtfsScanProgressPhase::MftRecords => VolumeScanProgressPhase::MftRecords,
            NtfsScanProgressPhase::Namespace => VolumeScanProgressPhase::Namespace,
            NtfsScanProgressPhase::Candidates => VolumeScanProgressPhase::Candidates,
            NtfsScanProgressPhase::Complete => VolumeScanProgressPhase::Complete,
        };
        progress(VolumeScanProgress {
            phase,
            completed: item.completed,
            total: item.total,
        });
    }) {
        Ok(output) => {
            let metadata_complete = output.is_complete;
            let mut warnings = output.warnings;
            let (candidates, carve_evidence, jpeg_carve_coverage, deep_complete) = match mode {
                VolumeScanMode::MetadataOnly => (output.candidates, Vec::new(), None, true),
                VolumeScanMode::DeepJpeg => {
                    progress(VolumeScanProgress {
                        phase: VolumeScanProgressPhase::DeepJpeg,
                        completed: 0,
                        total: 0,
                    });
                    let deep = deep::scan_ntfs_deep_jpeg(
                        reader,
                        index,
                        output.candidates,
                        output.allocation.as_ref(),
                    )?;
                    warnings.extend(deep.warnings);
                    progress(VolumeScanProgress {
                        phase: VolumeScanProgressPhase::Complete,
                        completed: 1,
                        total: 1,
                    });
                    (
                        deep.candidates,
                        deep.evidence,
                        Some(deep.coverage),
                        deep.is_complete,
                    )
                }
            };
            let report = VolumeReport {
                index,
                offset_bytes: region.offset,
                length_bytes: region.len,
                file_system: "ntfs".to_string(),
                scan_status: if metadata_complete && deep_complete {
                    VolumeScanStatus::Complete
                } else {
                    VolumeScanStatus::Partial
                },
                candidate_count: candidates.len(),
                mft_coverage: Some(output.coverage.into()),
                jpeg_carve_coverage,
                warnings,
            };
            Ok(VolumeScanDetails {
                report,
                candidates,
                ntfs_namespace: Some(output.namespace),
                carve_evidence,
            })
        }
        Err(ScanError::NotRecognized(_)) => match scan_fat(reader) {
            Ok(output) => {
                let file_system = match output.boot.variant {
                    FatVariant::Fat12 => "fat12",
                    FatVariant::Fat16 => "fat16",
                    FatVariant::Fat32 => "fat32",
                };
                let deep_requested = mode == VolumeScanMode::DeepJpeg;
                let mut warnings = output.warnings;
                if deep_requested {
                    warnings.push(
                        "JPEG deep scan was not run because trustworthy NTFS allocation evidence is required."
                            .into(),
                    );
                }
                let report = VolumeReport {
                    index,
                    offset_bytes: region.offset,
                    length_bytes: region.len,
                    file_system: file_system.to_string(),
                    scan_status: if output.is_complete && !deep_requested {
                        VolumeScanStatus::Complete
                    } else {
                        VolumeScanStatus::Partial
                    },
                    candidate_count: output.candidates.len(),
                    mft_coverage: None,
                    jpeg_carve_coverage: deep_requested.then(deep::unsupported_deep_coverage),
                    warnings,
                };
                Ok(VolumeScanDetails {
                    report,
                    candidates: output.candidates,
                    ntfs_namespace: None,
                    carve_evidence: Vec::new(),
                })
            }
            Err(ScanError::NotRecognized(_)) => {
                let mut report = unrecognized_volume(index, region);
                if mode == VolumeScanMode::DeepJpeg {
                    report.jpeg_carve_coverage = Some(deep::unsupported_deep_coverage());
                    report.warnings.push(
                        "JPEG deep scan was not run because trustworthy NTFS allocation evidence is required."
                            .into(),
                    );
                }
                Ok(VolumeScanDetails {
                    report,
                    candidates: Vec::new(),
                    ntfs_namespace: None,
                    carve_evidence: Vec::new(),
                })
            }
            Err(source) => Err(CliError::VolumeScan {
                index,
                file_system: "FAT",
                source,
            }),
        },
        Err(source) => Err(CliError::VolumeScan {
            index,
            file_system: "NTFS",
            source,
        }),
    }
}

fn unrecognized_volume(index: u32, region: Region) -> VolumeReport {
    VolumeReport {
        index,
        offset_bytes: region.offset,
        length_bytes: region.len,
        file_system: "unrecognized".to_string(),
        scan_status: VolumeScanStatus::Unrecognized,
        candidate_count: 0,
        mft_coverage: None,
        jpeg_carve_coverage: None,
        warnings: Vec::new(),
    }
}

impl From<NtfsScanCoverage> for MftScanCoverage {
    fn from(coverage: NtfsScanCoverage) -> Self {
        Self {
            records_declared: coverage.records_declared,
            records_available: coverage.records_available,
            records_examined: coverage.records_examined,
            bytes_declared: coverage.bytes_declared,
            bytes_available: coverage.bytes_available,
            bytes_examined: coverage.bytes_examined,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{path_is_forbidden_source, scan_volume, scan_volume_reader, CliError};
    use std::path::Path;
    use um_core::{
        ReadError, ReadOutcome, Region, SectorLayout, SourceIdentity, SourceKind, SourceReader,
    };
    use um_fixture_builder::ntfs::{
        FileOptions as NtfsFileOptions, NodeParent as NtfsParent, NtfsImageBuilder,
    };
    use um_fs_common::ScanError;
    use um_io_common::MemImageReader;

    struct MarkerErrorReader {
        identity: SourceIdentity,
    }

    impl MarkerErrorReader {
        fn new() -> Self {
            Self {
                identity: SourceIdentity {
                    id: "synthetic-marker-reader".to_string(),
                    kind: SourceKind::ImageFile,
                    label: "synthetic.img".to_string(),
                    size: 4096,
                },
            }
        }
    }

    impl SourceReader for MarkerErrorReader {
        fn identity(&self) -> &SourceIdentity {
            &self.identity
        }

        fn len(&self) -> u64 {
            self.identity.size
        }

        fn sector_layout(&self) -> SectorLayout {
            SectorLayout::DEFAULT_512
        }

        fn read_exact_at(&self, offset: u64, _buffer: &mut [u8]) -> Result<(), ReadError> {
            Err(ReadError::Io {
                offset,
                message: "UNIQUE-SECRET-IO-MARKER".to_string(),
            })
        }

        fn read_best_effort_at(&self, _offset: u64, buffer: &mut [u8]) -> ReadOutcome {
            buffer.fill(0);
            ReadOutcome {
                bytes_valid: 0,
                bad_ranges: vec![(0, buffer.len() as u64)],
            }
        }
    }

    #[test]
    fn cli_image_device_path_001_rejects_windows_device_spellings_without_opening() {
        for path in [
            r"NUL.img",
            r"C:\images\COM1.raw",
            r"C:\images\lpt9.bin",
            r"C:\images\CON.txt.img",
            r"C:\images\NUL .img",
            r"C:\images\COM¹.dd",
        ] {
            assert!(
                path_is_forbidden_source(Path::new(path)),
                "device spelling was accepted: {path}"
            );
        }

        for path in [
            [r"\\", ".", r"\Physical", "Drive0.img"].concat(),
            [r"\\", "?", r"\GLOBALROOT\Device\Harddisk0\Partition0.img"].concat(),
            [r"\?", "?", r"\Physical", "Drive0.img"].concat(),
            [r"\GLOBAL", r"ROOT\Device\Harddisk0\Partition0.img"].concat(),
            [r"\\server", r"\pipe\source.img"].concat(),
        ] {
            assert!(
                path_is_forbidden_source(Path::new(&path)),
                "device spelling was accepted: {path}"
            );
        }
    }

    #[test]
    fn cli_image_device_path_002_accepts_ordinary_image_spellings() {
        for path in [
            r"C:\images\backup.img",
            r".\fixtures\company.raw",
            "/tmp/fixture.dd",
        ] {
            assert!(
                !path_is_forbidden_source(Path::new(path)),
                "ordinary path was rejected: {path}"
            );
        }
    }

    #[test]
    fn cli_probe_error_privacy_001_never_masks_or_interpolates_raw_scan_errors() {
        let reader = MarkerErrorReader::new();
        let region = Region::new(0, reader.len()).unwrap();

        let error = scan_volume(0, region, &reader).expect_err("read failure must not be a report");
        assert!(matches!(
            &error,
            CliError::VolumeScan {
                index: 0,
                file_system: "NTFS",
                source: ScanError::Read(_)
            }
        ));
        assert!(!error.to_string().contains("UNIQUE-SECRET-IO-MARKER"));
    }

    #[test]
    fn cli_volume_reader_001_preserves_real_candidates_and_namespace() {
        let mut builder = NtfsImageBuilder::new("cli-volume-reader");
        builder.add_file(
            NtfsParent::Root,
            "deleted-volume-file.txt",
            b"real candidate metadata".to_vec(),
            true,
            NtfsFileOptions::default(),
        );
        let (volume, manifest) = builder.build();
        let reader = MemImageReader::new("cli-volume-reader", volume);

        let details = scan_volume_reader(&reader).expect("scan mounted-volume reader");

        assert_eq!(details.report.index, 0);
        assert_eq!(details.report.offset_bytes, 0);
        assert_eq!(details.report.length_bytes, reader.len());
        assert_eq!(details.report.file_system, "ntfs");
        assert_eq!(details.candidates.len(), manifest.expected_candidates.len());
        assert_eq!(
            details.candidates[0].name,
            manifest.expected_candidates[0].name
        );
        assert!(
            details.ntfs_namespace.is_some(),
            "folder scoping needs identity evidence, not only a display path"
        );
    }
}
