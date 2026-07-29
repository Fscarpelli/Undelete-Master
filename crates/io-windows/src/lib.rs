//! Minimal audited Windows FFI boundary for classifying a filesystem path.
//!
//! This crate exposes no file-open, write, device, volume-control, or process
//! APIs. Its only Windows call is `GetDriveTypeW` with a fixed-size,
//! NUL-terminated drive-root buffer.

#![deny(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageLocation {
    Local,
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LocationError {
    #[error("path does not have a supported local drive root")]
    UnsupportedRoot,
    #[error("Windows could not classify the drive root")]
    DriveTypeUnavailable,
}

/// Classifies an absolute path without opening the path or any source file.
///
/// On Windows, mapped SMB/WebDAV drive letters are reported as `Remote`.
/// Callers must reject both `Remote` and errors. Other platforms do not have
/// Windows mapped drives and return `Local`; their own path policy remains the
/// caller's responsibility.
pub fn classify_path(path: &Path) -> Result<StorageLocation, LocationError> {
    platform::classify_path(path)
}

#[cfg(windows)]
mod platform {
    use std::path::{Component, Path, Prefix};

    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    use super::{LocationError, StorageLocation};

    const DRIVE_UNKNOWN: u32 = 0;
    const DRIVE_NO_ROOT_DIR: u32 = 1;
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOTE: u32 = 4;
    const DRIVE_CDROM: u32 = 5;
    const DRIVE_RAMDISK: u32 = 6;

    pub(super) fn classify_path(path: &Path) -> Result<StorageLocation, LocationError> {
        let drive_letter = match path.components().next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::Disk(letter) => letter,
                _ => return Err(LocationError::UnsupportedRoot),
            },
            _ => return Err(LocationError::UnsupportedRoot),
        };
        let root = [
            u16::from(drive_letter.to_ascii_uppercase()),
            u16::from(b':'),
            u16::from(b'\\'),
            0,
        ];

        // SAFETY: `root` is a live, fixed-size, NUL-terminated UTF-16 buffer.
        // GetDriveTypeW only reads that buffer and performs no file mutation.
        let drive_type = unsafe { GetDriveTypeW(root.as_ptr()) };
        classify_drive_type(drive_type)
    }

    fn classify_drive_type(drive_type: u32) -> Result<StorageLocation, LocationError> {
        match drive_type {
            DRIVE_REMOTE => Ok(StorageLocation::Remote),
            DRIVE_REMOVABLE | DRIVE_FIXED | DRIVE_CDROM | DRIVE_RAMDISK => {
                Ok(StorageLocation::Local)
            }
            DRIVE_UNKNOWN | DRIVE_NO_ROOT_DIR | 7..=u32::MAX => {
                Err(LocationError::DriveTypeUnavailable)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn windows_drive_type_001_classifies_mapped_remote_drives() {
            assert_eq!(
                classify_drive_type(DRIVE_REMOTE),
                Ok(StorageLocation::Remote)
            );
            for drive_type in [DRIVE_REMOVABLE, DRIVE_FIXED, DRIVE_CDROM, DRIVE_RAMDISK] {
                assert_eq!(classify_drive_type(drive_type), Ok(StorageLocation::Local));
            }
            for drive_type in [DRIVE_UNKNOWN, DRIVE_NO_ROOT_DIR, 99] {
                assert_eq!(
                    classify_drive_type(drive_type),
                    Err(LocationError::DriveTypeUnavailable)
                );
            }
        }

        #[test]
        fn windows_drive_root_001_rejects_non_drive_prefixes() {
            assert_eq!(
                classify_path(Path::new(r"\\server\share\image.img")),
                Err(LocationError::UnsupportedRoot)
            );
            assert_eq!(
                classify_path(Path::new(r"relative\image.img")),
                Err(LocationError::UnsupportedRoot)
            );
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::path::Path;

    use super::{LocationError, StorageLocation};

    pub(super) fn classify_path(_path: &Path) -> Result<StorageLocation, LocationError> {
        Ok(StorageLocation::Local)
    }
}
