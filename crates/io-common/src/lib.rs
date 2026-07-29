//! Read-only source readers backed by ordinary files (raw disk images).
//!
//! The image file is opened strictly read-only; this crate contains no code
//! path that can write to a source.

#![forbid(unsafe_code)]

use std::fs::File;
use std::io;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;
use um_core::{
    ReadError, ReadOutcome, Region, SectorLayout, SourceIdentity, SourceKind, SourceReader,
};
use um_io_windows::StorageLocation;

#[derive(Debug, Error)]
pub enum SourcePathError {
    #[error("source path is remote, redirected, or otherwise forbidden")]
    Forbidden,
    #[error("source path is not an ordinary regular file")]
    NotRegular,
    #[error("source path metadata could not be read")]
    Metadata(#[source] io::Error),
}

/// Resolves a path lexically and rejects remote storage plus every reparse or
/// symlink component before a source handle is opened.
pub fn validate_local_regular_file(path: &Path) -> Result<PathBuf, SourcePathError> {
    validate_local_regular_file_with(path, um_io_windows::classify_path)
}

fn validate_local_regular_file_with<F>(path: &Path, classify: F) -> Result<PathBuf, SourcePathError>
where
    F: FnOnce(&Path) -> Result<StorageLocation, um_io_windows::LocationError>,
{
    let absolute = std::path::absolute(path).map_err(SourcePathError::Metadata)?;
    match classify(&absolute) {
        Ok(StorageLocation::Local) => {}
        Ok(StorageLocation::Remote) | Err(_) => return Err(SourcePathError::Forbidden),
    }

    let mut current = PathBuf::new();
    let mut final_metadata = None;
    for component in absolute.components() {
        current.push(component.as_os_str());
        match component {
            Component::Prefix(_) | Component::RootDir => continue,
            Component::CurDir | Component::ParentDir => return Err(SourcePathError::Forbidden),
            Component::Normal(_) => {}
        }
        let metadata = std::fs::symlink_metadata(&current).map_err(SourcePathError::Metadata)?;
        if metadata_is_redirect(&metadata) {
            return Err(SourcePathError::Forbidden);
        }
        final_metadata = Some(metadata);
    }

    let metadata = final_metadata.ok_or(SourcePathError::NotRegular)?;
    if !metadata.file_type().is_file() {
        return Err(SourcePathError::NotRegular);
    }
    Ok(absolute)
}

fn metadata_is_redirect(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Read-only reader over a raw image file (`.img`, `.dd`, `.raw`).
pub struct FileImageReader {
    file: File,
    identity: SourceIdentity,
    len: u64,
    sector_layout: SectorLayout,
}

impl FileImageReader {
    /// Opens the image strictly read-only.
    pub fn open(path: &Path) -> io::Result<Self> {
        let path = validate_local_regular_file(path).map_err(|error| match error {
            SourcePathError::Forbidden => io::Error::new(
                io::ErrorKind::InvalidInput,
                "source path is remote or redirected",
            ),
            SourcePathError::NotRegular => {
                io::Error::new(io::ErrorKind::InvalidInput, "source is not a regular file")
            }
            SourcePathError::Metadata(error) => error,
        })?;
        let mut options = File::options();
        options.read(true).write(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Open the reparse point itself instead of following it. This
            // binds the final file-type check below to the opened handle and
            // closes the validation/open race for symlinks and junctions.
            const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
            options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(&path)?;
        let metadata = file.metadata()?;
        if metadata_is_redirect(&metadata) || !metadata.file_type().is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "source handle is redirected or is not a regular file",
            ));
        }
        let len = metadata.len();
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_string());
        let identity = SourceIdentity {
            id: format!("image:<redacted-path>:{len}"),
            kind: SourceKind::ImageFile,
            label,
            size: len,
        };
        Ok(Self {
            file,
            identity,
            len,
            sector_layout: SectorLayout::DEFAULT_512,
        })
    }

    /// Overrides the assumed sector layout (e.g. 4Kn images).
    pub fn with_sector_layout(mut self, layout: SectorLayout) -> Self {
        self.sector_layout = layout;
        self
    }

    fn read_at_impl(&self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(windows)]
        {
            use std::os::windows::fs::FileExt;
            self.file.seek_read(buf, offset)
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;
            self.file.read_at(buf, offset)
        }
    }
}

impl SourceReader for FileImageReader {
    fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    fn len(&self) -> u64 {
        self.len
    }

    fn sector_layout(&self) -> SectorLayout {
        self.sector_layout
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        let len = buffer.len() as u64;
        let region = Region::new(offset, len).ok_or(ReadError::OutOfBounds {
            offset,
            len,
            source_len: self.len,
        })?;
        if region.end() > self.len {
            return Err(ReadError::OutOfBounds {
                offset,
                len,
                source_len: self.len,
            });
        }
        let mut done = 0usize;
        while done < buffer.len() {
            match self.read_at_impl(offset + done as u64, &mut buffer[done..]) {
                Ok(0) => {
                    return Err(ReadError::Io {
                        offset: offset + done as u64,
                        message: "unexpected end of file".into(),
                    })
                }
                Ok(n) => done += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    return Err(ReadError::Io {
                        offset: offset + done as u64,
                        message: e.to_string(),
                    })
                }
            }
        }
        Ok(())
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        match self.read_exact_at(offset, buffer) {
            Ok(()) => ReadOutcome::complete(buffer.len() as u64),
            Err(_) => {
                // Retry sector by sector, zero-filling failures.
                let sector = self.sector_layout.logical as usize;
                if sector == 0 {
                    buffer.fill(0);
                    return ReadOutcome {
                        bytes_valid: 0,
                        bad_ranges: vec![(0, buffer.len() as u64)],
                    };
                }
                let mut bad = Vec::new();
                let mut valid = 0u64;
                let mut pos = 0usize;
                while pos < buffer.len() {
                    let chunk = sector.min(buffer.len() - pos);
                    let abs = offset.checked_add(pos as u64);
                    let ok = abs
                        .and_then(|absolute| {
                            absolute
                                .checked_add(chunk as u64)
                                .map(|end| (absolute, end))
                        })
                        .filter(|(_, end)| *end <= self.len)
                        .map(|(absolute, _)| {
                            self.read_at_impl(absolute, &mut buffer[pos..pos + chunk])
                                .map(|n| n == chunk)
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    if ok {
                        valid += chunk as u64;
                    } else {
                        buffer[pos..pos + chunk].fill(0);
                        bad.push((pos as u64, chunk as u64));
                    }
                    pos += chunk;
                }
                ReadOutcome {
                    bytes_valid: valid,
                    bad_ranges: bad,
                }
            }
        }
    }
}

/// In-memory read-only source, used by tests and fixtures.
pub struct MemImageReader {
    data: Vec<u8>,
    identity: SourceIdentity,
    sector_layout: SectorLayout,
}

impl MemImageReader {
    pub fn new(label: &str, data: Vec<u8>) -> Self {
        let identity = SourceIdentity {
            id: format!("mem:{label}:{}", data.len()),
            kind: SourceKind::ImageFile,
            label: label.to_string(),
            size: data.len() as u64,
        };
        Self {
            data,
            identity,
            sector_layout: SectorLayout::DEFAULT_512,
        }
    }
}

impl SourceReader for MemImageReader {
    fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn sector_layout(&self) -> SectorLayout {
        self.sector_layout
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        let len = buffer.len() as u64;
        let end = offset.checked_add(len).ok_or(ReadError::OutOfBounds {
            offset,
            len,
            source_len: self.len(),
        })?;
        if end > self.len() {
            return Err(ReadError::OutOfBounds {
                offset,
                len,
                source_len: self.len(),
            });
        }
        buffer.copy_from_slice(&self.data[offset as usize..end as usize]);
        Ok(())
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
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

/// A reader restricted to a sub-region of another reader (e.g. a partition
/// within a disk image). Offsets are relative to the region start.
pub struct RegionReader<'a> {
    inner: &'a dyn SourceReader,
    region: Region,
    identity: SourceIdentity,
}

impl<'a> RegionReader<'a> {
    pub fn new(inner: &'a dyn SourceReader, region: Region) -> Option<Self> {
        let whole = Region::new(0, inner.len())?;
        if !whole.contains(&region) {
            return None;
        }
        let base = inner.identity();
        let identity = SourceIdentity {
            id: format!("{}@{}+{}", base.id, region.offset, region.len),
            kind: base.kind,
            label: base.label.clone(),
            size: region.len,
        };
        Some(Self {
            inner,
            region,
            identity,
        })
    }
}

impl SourceReader for RegionReader<'_> {
    fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    fn len(&self) -> u64 {
        self.region.len
    }

    fn sector_layout(&self) -> SectorLayout {
        self.inner.sector_layout()
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        let len = buffer.len() as u64;
        let sub = self.region.sub(offset, len).ok_or(ReadError::OutOfBounds {
            offset,
            len,
            source_len: self.region.len,
        })?;
        self.inner.read_exact_at(sub.offset, buffer)
    }

    fn read_best_effort_at(&self, offset: u64, buffer: &mut [u8]) -> ReadOutcome {
        let len = buffer.len() as u64;
        match self.region.sub(offset, len) {
            Some(sub) => self.inner.read_best_effort_at(sub.offset, buffer),
            None => {
                buffer.fill(0);
                ReadOutcome {
                    bytes_valid: 0,
                    bad_ranges: vec![(0, len)],
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_image(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.img");
        let mut f = File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        (dir, path)
    }

    #[test]
    fn reads_exact() {
        let data: Vec<u8> = (0..=255u8).collect();
        let (_d, path) = temp_image(&data);
        let r = FileImageReader::open(&path).unwrap();
        assert_eq!(r.len(), 256);
        let got = r.read_vec_at(10, 16).unwrap();
        assert_eq!(got, &data[10..26]);
    }

    #[test]
    fn rejects_out_of_bounds() {
        let (_d, path) = temp_image(&[0u8; 100]);
        let r = FileImageReader::open(&path).unwrap();
        assert!(matches!(
            r.read_vec_at(90, 20),
            Err(ReadError::OutOfBounds { .. })
        ));
        assert!(matches!(
            r.read_vec_at(u64::MAX, 2),
            Err(ReadError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn region_reader_translates_and_bounds() {
        let data: Vec<u8> = (0..=255u8).collect();
        let (_d, path) = temp_image(&data);
        let r = FileImageReader::open(&path).unwrap();
        let region = Region::new(100, 50).unwrap();
        let rr = RegionReader::new(&r, region).unwrap();
        assert_eq!(rr.len(), 50);
        assert_eq!(rr.read_vec_at(0, 4).unwrap(), &data[100..104]);
        assert!(rr.read_vec_at(48, 4).is_err());
    }

    #[test]
    fn io_sector_zero_001_fails_best_effort_closed() {
        let (_d, path) = temp_image(&[0xAA; 16]);
        let reader = FileImageReader::open(&path)
            .unwrap()
            .with_sector_layout(SectorLayout {
                logical: 0,
                physical: 512,
            });
        let mut buffer = [0xFF; 4];

        let outcome = reader.read_best_effort_at(32, &mut buffer);

        assert_eq!(buffer, [0; 4]);
        assert_eq!(outcome.bytes_valid, 0);
        assert_eq!(outcome.bad_ranges, vec![(0, 4)]);
    }

    #[test]
    fn io_best_effort_overflow_002_fails_closed_without_wrapping() {
        let (_d, path) = temp_image(&[0xAA; 2048]);
        let reader = FileImageReader::open(&path).unwrap();
        let mut buffer = [0xFF; 1024];

        let outcome = reader.read_best_effort_at(u64::MAX, &mut buffer);

        assert_eq!(buffer, [0; 1024]);
        assert_eq!(outcome.bytes_valid, 0);
        assert_eq!(outcome.bad_ranges, vec![(0, 512), (512, 512)]);
    }

    #[test]
    fn rejects_directory_as_an_image_handle() {
        let dir = tempfile::tempdir().unwrap();
        assert!(FileImageReader::open(dir.path()).is_err());
    }

    #[test]
    fn io_source_identity_privacy_001_redacts_the_parent_path() {
        let dir = tempfile::tempdir().unwrap();
        let secret_dir = dir.path().join("UNIQUE-SECRET-PARENT-MARKER");
        std::fs::create_dir(&secret_dir).unwrap();
        let path = secret_dir.join("source.img");
        std::fs::write(&path, [1_u8, 2, 3]).unwrap();

        let reader = FileImageReader::open(&path).unwrap();

        assert_eq!(reader.identity().label, "source.img");
        assert_eq!(reader.identity().id, "image:<redacted-path>:3");
        assert!(!reader.identity().id.contains("UNIQUE-SECRET-PARENT-MARKER"));
    }

    #[test]
    fn io_remote_path_001_rejects_a_mapped_remote_classification_before_open() {
        let (_dir, path) = temp_image(&[1, 2, 3]);

        let error = validate_local_regular_file_with(&path, |_| Ok(StorageLocation::Remote))
            .expect_err("remote path must be rejected");

        assert!(matches!(error, SourcePathError::Forbidden));
    }

    #[cfg(windows)]
    #[test]
    fn io_ancestor_reparse_001_rejects_a_junction_before_open() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        let junction = dir.path().join("junction");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("source.img"), [1_u8, 2, 3]).unwrap();
        let output = std::process::Command::new("cmd")
            .args([
                "/d",
                "/c",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                target.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "failed to create disposable test junction: {output:?}"
        );

        let junction_metadata = std::fs::symlink_metadata(&junction).unwrap();
        assert!(metadata_is_redirect(&junction_metadata));
        let final_error =
            validate_local_regular_file(&junction).expect_err("final reparse point must fail");
        assert!(matches!(final_error, SourcePathError::Forbidden));

        let error = validate_local_regular_file(&junction.join("source.img"))
            .expect_err("junction ancestor must be rejected");

        assert!(matches!(error, SourcePathError::Forbidden));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_without_following_its_regular_target() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.img");
        let link = dir.path().join("link.img");
        File::create(&target).unwrap();
        symlink(&target, &link).unwrap();

        assert!(FileImageReader::open(&link).is_err());
    }
}
