//! Unelevated, read-only client for the elevated storage broker.
//!
//! The scanner-facing type implements [`um_core::SourceReader`]. Its public
//! authority is limited to opaque inventory identity, expected source
//! identity, bounded reads, close, and shutdown.

#![forbid(unsafe_code)]

use std::fmt;
use std::io::{Read, Write};
use std::sync::Mutex;

use subtle::ConstantTimeEq;
use um_broker_protocol::{BrokerErrorCode, Message, ProtocolError, SessionCodec, MAX_READ_LEN};
use um_core::{ReadError, ReadOutcome, SectorLayout, SourceIdentity, SourceKind, SourceReader};

/// Sanitized client failures. No operating-system diagnostic, native object
/// name, pipe name, nonce, or source bytes are retained in these values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerClientError {
    InvalidRequest,
    TransportFailure,
    ProtocolFailure,
    UnexpectedResponse,
    AuthenticationFailed,
    SourceNotFound,
    SourceIdentityMismatch,
    SourceUnavailable,
    InvalidHandle,
    ReadOutOfRange,
    ReadFailed,
    SourceChanged,
    InternalFailure,
    ShortRead,
    SessionUnavailable,
    ElevationDenied,
    BrokerUnavailable,
    PeerVerificationFailed,
    RandomnessUnavailable,
    UnsupportedPlatform,
}

impl BrokerClientError {
    fn from_protocol(error: ProtocolError) -> Self {
        if matches!(error, ProtocolError::Io(_)) {
            Self::TransportFailure
        } else {
            Self::ProtocolFailure
        }
    }

    fn from_broker(code: BrokerErrorCode) -> Self {
        match code {
            BrokerErrorCode::InvalidRequest => Self::InvalidRequest,
            BrokerErrorCode::AuthenticationFailed => Self::AuthenticationFailed,
            BrokerErrorCode::SourceNotFound => Self::SourceNotFound,
            BrokerErrorCode::SourceIdentityMismatch => Self::SourceIdentityMismatch,
            BrokerErrorCode::SourceUnavailable => Self::SourceUnavailable,
            BrokerErrorCode::InvalidHandle => Self::InvalidHandle,
            BrokerErrorCode::ReadOutOfRange => Self::ReadOutOfRange,
            BrokerErrorCode::ReadFailed => Self::ReadFailed,
            BrokerErrorCode::SourceChanged => Self::SourceChanged,
            BrokerErrorCode::InternalFailure => Self::InternalFailure,
        }
    }

    const fn public_message(self) -> &'static str {
        match self {
            Self::InvalidRequest => "broker request rejected",
            Self::TransportFailure => "broker transport unavailable",
            Self::ProtocolFailure => "broker protocol failure",
            Self::UnexpectedResponse => "broker returned an unexpected response",
            Self::AuthenticationFailed => "broker authentication failed",
            Self::SourceNotFound => "broker source is unavailable",
            Self::SourceIdentityMismatch | Self::SourceChanged => "broker source identity changed",
            Self::SourceUnavailable => "broker source is unavailable",
            Self::InvalidHandle => "broker source handle is invalid",
            Self::ReadOutOfRange => "broker source read is out of range",
            Self::ReadFailed | Self::ShortRead => "broker source read failed",
            Self::InternalFailure => "broker internal failure",
            Self::SessionUnavailable => "broker session unavailable",
            Self::ElevationDenied => "broker elevation was not approved",
            Self::BrokerUnavailable => "broker is unavailable",
            Self::PeerVerificationFailed => "broker peer verification failed",
            Self::RandomnessUnavailable => "broker session initialization failed",
            Self::UnsupportedPlatform => "broker source is unsupported on this platform",
        }
    }
}

impl fmt::Display for BrokerClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.public_message())
    }
}

impl std::error::Error for BrokerClientError {}

/// Metadata returned after an identity-bound source is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenedSource {
    pub handle_id: u64,
    pub size: u64,
    pub logical_sector: u32,
    pub physical_sector: u32,
}

/// Testable read-only session boundary used by [`BrokerSourceReader`].
///
/// Implementations may communicate over a protocol transport or provide a
/// deterministic in-memory source in tests. There is deliberately no source
/// mutation operation.
pub trait ReadSession: Send {
    fn open_source(&mut self, source_id: &str) -> Result<OpenedSource, BrokerClientError>;

    fn read_at(
        &mut self,
        handle_id: u64,
        offset: u64,
        length: u32,
    ) -> Result<Vec<u8>, BrokerClientError>;

    fn close_source(&mut self, handle_id: u64) -> Result<(), BrokerClientError>;

    fn shutdown(&mut self) -> Result<(), BrokerClientError>;
}

/// Synchronous protocol session with independent inbound and outbound sequence
/// validation.
#[derive(Debug)]
pub struct ProtocolReadSession<T> {
    transport: T,
    codec: SessionCodec,
    shutdown: bool,
    failed: bool,
}

impl<T> ProtocolReadSession<T>
where
    T: Read + Write,
{
    /// Performs the mandatory post-connect 32-byte nonce exchange and verifies
    /// the broker's exact echo in constant time. The echo is not a MAC or
    /// shared secret; native peer PID, liveness, and packaged-image checks
    /// remain primary.
    pub fn connect(transport: T, challenge: [u8; 32]) -> Result<Self, BrokerClientError> {
        if challenge.ct_eq(&[0_u8; 32]).unwrap_u8() == 1 {
            return Err(BrokerClientError::AuthenticationFailed);
        }
        let mut session = Self {
            transport,
            codec: SessionCodec::new(),
            shutdown: false,
            failed: false,
        };
        let response = session.exchange(&Message::Hello { challenge })?;
        match response {
            Message::HelloAck {
                challenge: echoed_challenge,
            } if challenge.ct_eq(&echoed_challenge).unwrap_u8() == 1 => Ok(session),
            Message::HelloAck { .. } => Err(BrokerClientError::AuthenticationFailed),
            Message::Error { code } => Err(BrokerClientError::from_broker(code)),
            _ => Err(BrokerClientError::UnexpectedResponse),
        }
    }

    fn exchange(&mut self, request: &Message) -> Result<Message, BrokerClientError> {
        if self.shutdown || self.failed {
            return Err(BrokerClientError::SessionUnavailable);
        }
        if let Err(error) = self.codec.write_message(&mut self.transport, request) {
            self.failed = true;
            return Err(BrokerClientError::from_protocol(error));
        }
        if self.transport.flush().is_err() {
            self.failed = true;
            return Err(BrokerClientError::TransportFailure);
        }
        match self.codec.read_message(&mut self.transport) {
            Ok(frame) => Ok(frame.message),
            Err(error) => {
                self.failed = true;
                Err(BrokerClientError::from_protocol(error))
            }
        }
    }

    fn send_without_response(&mut self, request: &Message) -> Result<(), BrokerClientError> {
        if self.shutdown {
            return Ok(());
        }
        if self.failed {
            return Err(BrokerClientError::SessionUnavailable);
        }
        if let Err(error) = self.codec.write_message(&mut self.transport, request) {
            self.failed = true;
            return Err(BrokerClientError::from_protocol(error));
        }
        if self.transport.flush().is_err() {
            self.failed = true;
            return Err(BrokerClientError::TransportFailure);
        }
        Ok(())
    }
}

impl<T> ReadSession for ProtocolReadSession<T>
where
    T: Read + Write + Send,
{
    fn open_source(&mut self, source_id: &str) -> Result<OpenedSource, BrokerClientError> {
        let response = self.exchange(&Message::OpenSource {
            source_id: source_id.to_owned(),
        })?;
        match response {
            Message::Opened {
                handle_id,
                size,
                logical_sector,
                physical_sector,
            } => Ok(OpenedSource {
                handle_id,
                size,
                logical_sector,
                physical_sector,
            }),
            Message::Error { code } => Err(BrokerClientError::from_broker(code)),
            _ => {
                self.failed = true;
                Err(BrokerClientError::UnexpectedResponse)
            }
        }
    }

    fn read_at(
        &mut self,
        handle_id: u64,
        offset: u64,
        length: u32,
    ) -> Result<Vec<u8>, BrokerClientError> {
        let response = self.exchange(&Message::ReadAt {
            handle_id,
            offset,
            length,
        })?;
        match response {
            Message::ReadData { bytes } => {
                if bytes.len()
                    != usize::try_from(length).map_err(|_| BrokerClientError::InvalidRequest)?
                {
                    self.failed = true;
                    return Err(BrokerClientError::ShortRead);
                }
                Ok(bytes)
            }
            Message::Error { code } => Err(BrokerClientError::from_broker(code)),
            _ => {
                self.failed = true;
                Err(BrokerClientError::UnexpectedResponse)
            }
        }
    }

    fn close_source(&mut self, handle_id: u64) -> Result<(), BrokerClientError> {
        let response = self.exchange(&Message::CloseSource { handle_id })?;
        match response {
            Message::Closed {
                handle_id: closed_handle,
            } if closed_handle == handle_id => Ok(()),
            Message::Error { code } => Err(BrokerClientError::from_broker(code)),
            _ => {
                self.failed = true;
                Err(BrokerClientError::UnexpectedResponse)
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), BrokerClientError> {
        if self.shutdown {
            return Ok(());
        }
        self.send_without_response(&Message::Shutdown)?;
        self.shutdown = true;
        Ok(())
    }
}

/// Read-only scanner adapter backed by one identity-bound broker source.
pub struct BrokerSourceReader<S: ReadSession> {
    session: Mutex<Option<S>>,
    handle_id: u64,
    identity: SourceIdentity,
    length: u64,
    sector_layout: SectorLayout,
}

impl<S: ReadSession> BrokerSourceReader<S> {
    /// Opens one opaque inventory source and accepts only bounded, internally
    /// consistent geometry returned by the authoritative elevated broker.
    pub fn open(mut session: S, source_id: &str) -> Result<Self, BrokerClientError> {
        if (Message::OpenSource {
            source_id: source_id.to_owned(),
        })
        .validate()
        .is_err()
        {
            let _ = session.shutdown();
            return Err(BrokerClientError::InvalidRequest);
        }

        let opened = match session.open_source(source_id) {
            Ok(opened) => opened,
            Err(error) => {
                let _ = session.shutdown();
                return Err(error);
            }
        };
        let sector_layout = SectorLayout::new(opened.logical_sector, opened.physical_sector);
        let metadata_matches = opened.handle_id != 0 && opened.size != 0 && sector_layout.is_some();
        if !metadata_matches {
            if opened.handle_id != 0 {
                let _ = session.close_source(opened.handle_id);
            }
            let _ = session.shutdown();
            return Err(BrokerClientError::SourceIdentityMismatch);
        }

        Ok(Self {
            session: Mutex::new(Some(session)),
            handle_id: opened.handle_id,
            identity: SourceIdentity {
                id: source_id.to_owned(),
                kind: SourceKind::Volume,
                label: "Selected broker source".to_owned(),
                size: opened.size,
            },
            length: opened.size,
            sector_layout: sector_layout.expect("sector layout checked above"),
        })
    }

    fn range_is_valid(&self, offset: u64, length: usize) -> bool {
        let Ok(length) = u64::try_from(length) else {
            return false;
        };
        offset
            .checked_add(length)
            .is_some_and(|end| end <= self.length)
    }

    fn read_failure(&self, error: BrokerClientError, offset: u64, length: usize) -> ReadError {
        match error {
            BrokerClientError::SourceNotFound
            | BrokerClientError::SourceIdentityMismatch
            | BrokerClientError::SourceUnavailable
            | BrokerClientError::SourceChanged => ReadError::SourceGone,
            BrokerClientError::ReadOutOfRange => ReadError::OutOfBounds {
                offset,
                len: length as u64,
                source_len: self.length,
            },
            other => ReadError::Io {
                offset,
                message: other.public_message().to_owned(),
            },
        }
    }
}

impl<S: ReadSession> SourceReader for BrokerSourceReader<S> {
    fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    fn len(&self) -> u64 {
        self.length
    }

    fn sector_layout(&self) -> SectorLayout {
        self.sector_layout
    }

    fn read_exact_at(&self, offset: u64, buffer: &mut [u8]) -> Result<(), ReadError> {
        if !self.range_is_valid(offset, buffer.len()) {
            return Err(ReadError::OutOfBounds {
                offset,
                len: buffer.len() as u64,
                source_len: self.length,
            });
        }
        if buffer.is_empty() {
            return Ok(());
        }

        let mut session_guard = self.session.lock().map_err(|_| ReadError::Io {
            offset,
            message: BrokerClientError::SessionUnavailable
                .public_message()
                .to_owned(),
        })?;
        let session = session_guard.as_mut().ok_or_else(|| ReadError::Io {
            offset,
            message: BrokerClientError::SessionUnavailable
                .public_message()
                .to_owned(),
        })?;

        let mut completed = 0_usize;
        while completed < buffer.len() {
            let chunk_length = (buffer.len() - completed).min(MAX_READ_LEN);
            let chunk_offset =
                offset
                    .checked_add(completed as u64)
                    .ok_or(ReadError::OutOfBounds {
                        offset,
                        len: buffer.len() as u64,
                        source_len: self.length,
                    })?;
            let requested_length = u32::try_from(chunk_length).map_err(|_| ReadError::Io {
                offset: chunk_offset,
                message: BrokerClientError::InvalidRequest
                    .public_message()
                    .to_owned(),
            })?;
            let bytes = match session.read_at(self.handle_id, chunk_offset, requested_length) {
                Ok(bytes) if bytes.len() == chunk_length => bytes,
                Ok(_) => {
                    buffer.fill(0);
                    return Err(self.read_failure(
                        BrokerClientError::ShortRead,
                        chunk_offset,
                        chunk_length,
                    ));
                }
                Err(error) => {
                    buffer.fill(0);
                    return Err(self.read_failure(error, chunk_offset, chunk_length));
                }
            };
            let chunk_end = completed
                .checked_add(chunk_length)
                .ok_or_else(|| ReadError::Io {
                    offset: chunk_offset,
                    message: BrokerClientError::InvalidRequest
                        .public_message()
                        .to_owned(),
                })?;
            buffer[completed..chunk_end].copy_from_slice(&bytes);
            completed = chunk_end;
        }
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

impl<S: ReadSession> Drop for BrokerSourceReader<S> {
    fn drop(&mut self) {
        let session_slot = match self.session.get_mut() {
            Ok(session_slot) => session_slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(session) = session_slot.as_mut() {
            let _ = session.close_source(self.handle_id);
            let _ = session.shutdown();
        }
        session_slot.take();
    }
}

#[cfg(windows)]
pub struct WindowsReadSession {
    protocol: ProtocolReadSession<um_io_windows::ConnectedPipe>,
    _process: um_io_windows::ElevatedBrokerProcess,
}

#[cfg(windows)]
impl ReadSession for WindowsReadSession {
    fn open_source(&mut self, source_id: &str) -> Result<OpenedSource, BrokerClientError> {
        self.protocol.open_source(source_id)
    }

    fn read_at(
        &mut self,
        handle_id: u64,
        offset: u64,
        length: u32,
    ) -> Result<Vec<u8>, BrokerClientError> {
        self.protocol.read_at(handle_id, offset, length)
    }

    fn close_source(&mut self, handle_id: u64) -> Result<(), BrokerClientError> {
        self.protocol.close_source(handle_id)
    }

    fn shutdown(&mut self) -> Result<(), BrokerClientError> {
        self.protocol.shutdown()
    }
}

#[cfg(not(windows))]
pub struct WindowsReadSession;

#[cfg(not(windows))]
impl ReadSession for WindowsReadSession {
    fn open_source(&mut self, _source_id: &str) -> Result<OpenedSource, BrokerClientError> {
        Err(BrokerClientError::UnsupportedPlatform)
    }

    fn read_at(
        &mut self,
        _handle_id: u64,
        _offset: u64,
        _length: u32,
    ) -> Result<Vec<u8>, BrokerClientError> {
        Err(BrokerClientError::UnsupportedPlatform)
    }

    fn close_source(&mut self, _handle_id: u64) -> Result<(), BrokerClientError> {
        Err(BrokerClientError::UnsupportedPlatform)
    }

    fn shutdown(&mut self) -> Result<(), BrokerClientError> {
        Err(BrokerClientError::UnsupportedPlatform)
    }
}

pub type WindowsBrokerSourceReader = BrokerSourceReader<WindowsReadSession>;

/// Opens one real Windows volume through the fixed elevated broker.
///
/// The caller supplies only an opaque native inventory ID. Pipe authority,
/// challenge material, authoritative identity resolution, source geometry,
/// fixed-binary launch, and peer verification remain inside native Rust.
#[cfg(windows)]
pub fn open_windows_source(
    volume_id: &str,
) -> Result<WindowsBrokerSourceReader, BrokerClientError> {
    use um_io_windows::{launch_elevated_broker, NamedPipeServer};

    Message::OpenSource {
        source_id: volume_id.to_owned(),
    }
    .validate()
    .map_err(|_| BrokerClientError::InvalidRequest)?;

    let mut random = [0_u8; 48];
    getrandom::fill(&mut random).map_err(|_| BrokerClientError::RandomnessUnavailable)?;
    let pipe_suffix = encode_hex(&random[..16]);
    let mut challenge = [0_u8; 32];
    challenge.copy_from_slice(&random[16..]);

    let server = NamedPipeServer::create(&pipe_suffix).map_err(map_windows_storage_error)?;
    let parent_pid = std::process::id();
    let process =
        launch_elevated_broker(&pipe_suffix, parent_pid).map_err(map_windows_storage_error)?;
    let pipe = server
        .accept_for_process(&process)
        .map_err(map_windows_storage_error)?;
    if pipe.peer_process_id() != process.pid()
        || !pipe.peer_is_alive().map_err(map_windows_storage_error)?
        || !process.is_alive().map_err(map_windows_storage_error)?
    {
        return Err(BrokerClientError::PeerVerificationFailed);
    }

    let protocol = ProtocolReadSession::connect(pipe, challenge)?;
    let session = WindowsReadSession {
        protocol,
        _process: process,
    };
    BrokerSourceReader::open(session, volume_id)
}

#[cfg(windows)]
fn map_windows_storage_error(error: um_io_windows::StorageError) -> BrokerClientError {
    use um_io_windows::StorageError;

    match error {
        StorageError::ElevationCancelled => BrokerClientError::ElevationDenied,
        StorageError::SourceIdentityUnavailable => BrokerClientError::SourceNotFound,
        StorageError::SourceIdentityChanged => BrokerClientError::SourceChanged,
        StorageError::UnsupportedSource(_) => BrokerClientError::SourceUnavailable,
        StorageError::UnexpectedPeerProcess { .. } => BrokerClientError::PeerVerificationFailed,
        StorageError::InvalidPipeSuffix
        | StorageError::InvalidParentProcess
        | StorageError::BrokerExecutableUnavailable
        | StorageError::InvalidUserIdentity
        | StorageError::UnsupportedPlatform => BrokerClientError::BrokerUnavailable,
        _ => BrokerClientError::TransportFailure,
    }
}

#[cfg(windows)]
fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0F)]));
    }
    encoded
}

#[cfg(not(windows))]
pub fn open_windows_source(
    _volume_id: &str,
) -> Result<WindowsBrokerSourceReader, BrokerClientError> {
    Err(BrokerClientError::UnsupportedPlatform)
}
