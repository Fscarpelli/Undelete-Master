//! Safe, non-elevated entry point for scanning ordinary disk-image files.
//!
//! This crate never opens physical devices, extracts content, or writes to a
//! scan source. Reports contain metadata only and redact the supplied path.

#![forbid(unsafe_code)]

mod report;

use std::fs;
use std::io;
use std::path::Path;

pub use report::{ImageScanReport, SourceReport, VolumeReport};
use thiserror::Error;
use um_core::{Region, SourceReader};
use um_fs_common::ScanError;
use um_fs_fat::{scan_fat, FatVariant};
use um_fs_ntfs::scan_ntfs;
use um_io_common::{FileImageReader, RegionReader};
use um_partition::{discover, PartitionTableKind};

const SUPPORTED_EXTENSIONS: &[&str] = &["img", "dd", "raw", "bin"];

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
}

/// Scans one ordinary image file and returns a sanitized metadata report.
pub fn scan_image_path(path: &Path) -> Result<ImageScanReport, CliError> {
    if path_is_forbidden_source(path) {
        return Err(CliError::ForbiddenSourcePath);
    }
    let metadata = fs::symlink_metadata(path).map_err(CliError::Metadata)?;
    if !metadata.file_type().is_file() {
        return Err(CliError::NotRegularFile);
    }
    validate_extension(path)?;

    let reader = FileImageReader::open(path).map_err(CliError::Open)?;
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
            Ok(scan_volume(index, region, &bounded))
        })
        .collect::<Result<Vec<_>, CliError>>()?;

    Ok(ImageScanReport {
        schema_version: 1,
        source,
        partition_table,
        volumes,
        warnings,
    })
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

fn scan_volume(index: u32, region: Region, reader: &dyn SourceReader) -> VolumeReport {
    match scan_ntfs(reader) {
        Ok(output) => VolumeReport {
            index,
            offset_bytes: region.offset,
            length_bytes: region.len,
            file_system: "ntfs".to_string(),
            candidate_count: output.candidates.len(),
            warnings: output.warnings,
        },
        Err(ntfs_error) => match scan_fat(reader) {
            Ok(output) => {
                let file_system = match output.boot.variant {
                    FatVariant::Fat12 => "fat12",
                    FatVariant::Fat16 => "fat16",
                    FatVariant::Fat32 => "fat32",
                };
                let mut warnings = output.warnings;
                if !matches!(ntfs_error, ScanError::NotRecognized(_)) {
                    warnings.push(format!("NTFS probe failed safely: {ntfs_error}"));
                }
                VolumeReport {
                    index,
                    offset_bytes: region.offset,
                    length_bytes: region.len,
                    file_system: file_system.to_string(),
                    candidate_count: output.candidates.len(),
                    warnings,
                }
            }
            Err(fat_error) => {
                let mut warnings = Vec::new();
                if !matches!(ntfs_error, ScanError::NotRecognized(_)) {
                    warnings.push(format!("NTFS probe failed safely: {ntfs_error}"));
                }
                if !matches!(fat_error, ScanError::NotRecognized(_)) {
                    warnings.push(format!("FAT probe failed safely: {fat_error}"));
                }
                unrecognized_volume(index, region, warnings)
            }
        },
    }
}

fn unrecognized_volume(index: u32, region: Region, warnings: Vec<String>) -> VolumeReport {
    VolumeReport {
        index,
        offset_bytes: region.offset,
        length_bytes: region.len,
        file_system: "unrecognized".to_string(),
        candidate_count: 0,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::path_is_forbidden_source;
    use std::path::Path;

    #[test]
    fn cli_image_device_path_001_rejects_windows_device_spellings_without_opening() {
        for path in [
            r"NUL.img",
            r"C:\images\COM1.raw",
            r"C:\images\lpt9.bin",
            r"C:\images\CON.txt.img",
            r"C:\images\NUL .img",
            r"C:\images\COM¹.dd",
            r"\\.\PhysicalDrive0.img",
            r"\\?\GLOBALROOT\Device\Harddisk0\Partition0.img",
            r"\??\PhysicalDrive0.img",
            r"\GLOBALROOT\Device\Harddisk0\Partition0.img",
            r"\\server\pipe\source.img",
        ] {
            assert!(
                path_is_forbidden_source(Path::new(path)),
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
}
