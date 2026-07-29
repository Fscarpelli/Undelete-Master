//! Read-only source readers backed by ordinary files (raw disk images).
//!
//! The image file is opened strictly read-only; this crate contains no code
//! path that can write to a source.

#![forbid(unsafe_code)]

use std::fs::File;
use std::io;
use std::path::Path;

use um_core::{
    ReadError, ReadOutcome, Region, SectorLayout, SourceIdentity, SourceKind, SourceReader,
};

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
        let file = File::options().read(true).write(false).open(path)?;
        let len = file.metadata()?.len();
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image".to_string());
        let identity = SourceIdentity {
            id: format!("image:{}:{}", path.display(), len),
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
                    let abs = offset + pos as u64;
                    let ok = abs
                        .checked_add(chunk as u64)
                        .map(|end| end <= self.len)
                        .unwrap_or(false)
                        && self
                            .read_at_impl(abs, &mut buffer[pos..pos + chunk])
                            .map(|n| n == chunk)
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
}
