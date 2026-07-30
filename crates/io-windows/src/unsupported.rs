use std::path::Path;
use std::time::Duration;
use std::{io, io::Read, io::Write};

use um_core::{SectorLayout, SourceIdentity};

use crate::DestinationError;
use crate::{FolderScope, LocationError, StorageError, StorageInventory, StorageLocation};

pub(super) fn classify_path(_path: &Path) -> Result<StorageLocation, LocationError> {
    Ok(StorageLocation::Local)
}

pub(super) fn enumerate_storage() -> Result<StorageInventory, StorageError> {
    Err(StorageError::UnsupportedPlatform)
}

pub(super) fn validate_folder_scope(
    _volume_id: &str,
    _folder: &Path,
) -> Result<FolderScope, StorageError> {
    Err(StorageError::UnsupportedPlatform)
}

pub(super) fn open_destination_root_binding(
    _destination_root: &Path,
) -> Result<DestinationRootBindingInner, StorageError> {
    Err(DestinationError::UnsupportedPlatform.into())
}

pub(super) enum DestinationRootBindingInner {}

impl DestinationRootBindingInner {
    pub(super) fn display_label(&self) -> &str {
        match *self {}
    }

    pub(super) fn volume_label(&self) -> &str {
        match *self {}
    }

    pub(super) fn file_system(&self) -> &str {
        match *self {}
    }

    pub(super) fn free_bytes(&self) -> u64 {
        match *self {}
    }

    pub(super) fn physical_disk_number(&self) -> u32 {
        match *self {}
    }

    pub(super) fn reparse_safe(&self) -> bool {
        match *self {}
    }

    pub(super) fn into_directory_file(self) -> std::fs::File {
        match self {}
    }
}

pub(super) fn open_raw_volume(_volume_id: &str) -> Result<RawVolumeInner, StorageError> {
    Err(StorageError::UnsupportedPlatform)
}

pub(super) fn create_named_pipe_server(
    _pipe_suffix: &str,
) -> Result<NamedPipeServerInner, StorageError> {
    Err(StorageError::UnsupportedPlatform)
}

pub(super) fn connect_broker_pipe(
    _pipe_suffix: &str,
    _expected_server_pid: u32,
) -> Result<ConnectedPipeInner, StorageError> {
    Err(StorageError::UnsupportedPlatform)
}

pub(super) fn launch_elevated_broker(
    _pipe_suffix: &str,
    _parent_pid: u32,
) -> Result<ElevatedBrokerProcessInner, StorageError> {
    Err(StorageError::UnsupportedPlatform)
}

pub(super) struct RawVolumeInner {
    identity: SourceIdentity,
    length: u64,
    layout: SectorLayout,
    physical_disk_number: u32,
}

impl RawVolumeInner {
    pub(super) fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub(super) fn len(&self) -> u64 {
        self.length
    }

    pub(super) fn sector_layout(&self) -> SectorLayout {
        self.layout
    }

    pub(super) fn physical_disk_number(&self) -> u32 {
        self.physical_disk_number
    }

    pub(super) fn read_exact_at(
        &self,
        _offset: u64,
        _buffer: &mut [u8],
    ) -> Result<(), StorageError> {
        Err(StorageError::UnsupportedPlatform)
    }

    pub(super) fn revalidate_identity(&self) -> Result<(), StorageError> {
        Err(StorageError::UnsupportedPlatform)
    }
}

pub(super) struct NamedPipeServerInner;

impl NamedPipeServerInner {
    pub(super) fn accept_with_timeout(
        self,
        _timeout: Duration,
        _broker: Option<&ElevatedBrokerProcessInner>,
    ) -> Result<ConnectedPipeInner, StorageError> {
        Err(StorageError::UnsupportedPlatform)
    }
}

pub(super) struct ConnectedPipeInner;

impl ConnectedPipeInner {
    pub(super) fn peer_process_id(&self) -> u32 {
        0
    }

    pub(super) fn peer_is_alive(&self) -> Result<bool, StorageError> {
        Err(StorageError::UnsupportedPlatform)
    }

    pub(super) fn peer_is_packaged_desktop(&self) -> Result<bool, StorageError> {
        Err(StorageError::UnsupportedPlatform)
    }
}

impl Read for ConnectedPipeInner {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            StorageError::UnsupportedPlatform,
        ))
    }
}

impl Write for ConnectedPipeInner {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            StorageError::UnsupportedPlatform,
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            StorageError::UnsupportedPlatform,
        ))
    }
}

pub(super) struct ElevatedBrokerProcessInner;

impl ElevatedBrokerProcessInner {
    pub(super) fn pid(&self) -> u32 {
        0
    }

    pub(super) fn is_alive(&self) -> Result<bool, StorageError> {
        Err(StorageError::UnsupportedPlatform)
    }
}
