//! Minimal elevated read-only broker.
//!
//! This crate parses no partition table, filesystem, recovered metadata, or
//! content. It authenticates one local protocol session and owns at most one
//! identity-bound read-only source handle.
//!
//! The Windows source is re-enumerated on a bounded monotonic cadence to avoid
//! turning small scanner reads into full inventory passes. That cache limits
//! identity re-enumeration only: every `ReadAt` still performs a real bounded
//! source read, and any read failure remains fail-closed. Neither behavior
//! claims snapshot consistency for an active volume.

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{Read, Write};
use std::time::Duration;

use um_broker_protocol::{BrokerErrorCode, Message, ProtocolError, SessionCodec};

const PIPE_SUFFIX_LEN: usize = 32;
const REVALIDATION_READ_INTERVAL: u32 = 256;
const REVALIDATION_MIN_INTERVAL: Duration = Duration::from_secs(1);

/// Returns true only after both cadence bounds are met. The source was fully
/// revalidated during open; later calls remain fail-closed but deliberately do
/// not claim snapshot consistency between bounded identity checks.
fn revalidation_due(reads_since_revalidation: u32, elapsed: Duration) -> bool {
    reads_since_revalidation >= REVALIDATION_READ_INTERVAL && elapsed >= REVALIDATION_MIN_INTERVAL
}

/// The complete broker command-line authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerArgs {
    pipe_suffix: String,
    parent_pid: u32,
}

impl BrokerArgs {
    /// Accepts exactly `--pipe <opaque suffix> --parent-pid <u32>`.
    pub fn parse<I, S>(arguments: I) -> Result<Self, BrokerArgumentError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let arguments = arguments.into_iter().map(Into::into).collect::<Vec<_>>();
        if arguments.len() != 4
            || arguments[0] != OsStr::new("--pipe")
            || arguments[2] != OsStr::new("--parent-pid")
        {
            return Err(BrokerArgumentError::InvalidShape);
        }

        let pipe_suffix = arguments[1]
            .clone()
            .into_string()
            .map_err(|_| BrokerArgumentError::InvalidPipeSuffix)?;
        if pipe_suffix.len() != PIPE_SUFFIX_LEN
            || !pipe_suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(BrokerArgumentError::InvalidPipeSuffix);
        }

        let parent_pid_text = arguments[3]
            .to_str()
            .ok_or(BrokerArgumentError::InvalidParentPid)?;
        let parent_pid = parent_pid_text
            .parse::<u32>()
            .ok()
            .filter(|value| *value != 0)
            .ok_or(BrokerArgumentError::InvalidParentPid)?;

        Ok(Self {
            pipe_suffix,
            parent_pid,
        })
    }

    pub fn pipe_suffix(&self) -> &str {
        &self.pipe_suffix
    }

    pub const fn parent_pid(&self) -> u32 {
        self.parent_pid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerArgumentError {
    InvalidShape,
    InvalidPipeSuffix,
    InvalidParentPid,
}

impl fmt::Display for BrokerArgumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidShape => "invalid broker arguments",
            Self::InvalidPipeSuffix => "invalid broker pipe suffix",
            Self::InvalidParentPid => "invalid broker parent process",
        })
    }
}

impl std::error::Error for BrokerArgumentError {}

/// Size and sector geometry visible to the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerSourceGeometry {
    pub size: u64,
    pub logical_sector: u32,
    pub physical_sector: u32,
    pub physical_disk_number: u32,
}

/// One identity-bound read-only source.
pub trait ReadOnlyBrokerSource {
    fn geometry(&self) -> BrokerSourceGeometry;

    /// Re-enumerates and proves that the opened identity is still current.
    fn revalidate(&mut self) -> Result<(), BrokerErrorCode>;

    fn read_exact_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<(), BrokerErrorCode>;
}

/// Authoritative source factory. Implementations independently resolve the
/// opaque inventory ID and derive canonical geometry before returning a handle.
pub trait ReadOnlySourceFactory {
    type Source: ReadOnlyBrokerSource;

    fn open_source(&mut self, source_id: &str) -> Result<Self::Source, BrokerErrorCode>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    AuthenticationFailed,
    InvalidHandleId,
    TransportFailure,
    ProtocolFailure,
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AuthenticationFailed => "broker authentication failed",
            Self::InvalidHandleId => "broker handle initialization failed",
            Self::TransportFailure => "broker transport unavailable",
            Self::ProtocolFailure => "broker protocol failure",
        })
    }
}

impl std::error::Error for SessionError {}

fn protocol_error(error: ProtocolError) -> SessionError {
    if matches!(error, ProtocolError::Io(_)) {
        SessionError::TransportFailure
    } else {
        SessionError::ProtocolFailure
    }
}

struct OpenSource<S> {
    handle_id: u64,
    source: S,
}

/// Serves one authenticated, sequenced protocol session.
///
/// `handle_id` is session-scoped and must be nonzero. The function owns no
/// source before a valid `OpenSource` and drops the source on close, shutdown,
/// disconnect, or identity change.
pub fn serve_session<T, F>(
    transport: &mut T,
    factory: &mut F,
    handle_id: u64,
) -> Result<(), SessionError>
where
    T: Read + Write,
    F: ReadOnlySourceFactory,
{
    if handle_id == 0 {
        return Err(SessionError::InvalidHandleId);
    }

    let mut codec = SessionCodec::new();
    let first = codec
        .read_message(transport)
        .map_err(protocol_error)?
        .message;
    let challenge = match first {
        Message::Hello { challenge } if challenge != [0_u8; 32] => challenge,
        _ => {
            send_message(
                transport,
                &mut codec,
                &Message::Error {
                    code: BrokerErrorCode::AuthenticationFailed,
                },
            )?;
            return Err(SessionError::AuthenticationFailed);
        }
    };
    send_message(transport, &mut codec, &Message::HelloAck { challenge })?;

    let mut opened: Option<OpenSource<F::Source>> = None;
    loop {
        let message = codec
            .read_message(transport)
            .map_err(protocol_error)?
            .message;
        match message {
            Message::OpenSource { source_id } => {
                if opened.is_some() {
                    send_broker_error(transport, &mut codec, BrokerErrorCode::InvalidRequest)?;
                    continue;
                }
                match factory.open_source(&source_id) {
                    Ok(source) => {
                        let geometry = source.geometry();
                        let opened_message = Message::Opened {
                            handle_id,
                            size: geometry.size,
                            logical_sector: geometry.logical_sector,
                            physical_sector: geometry.physical_sector,
                            physical_disk_number: geometry.physical_disk_number,
                        };
                        if opened_message.validate().is_err() {
                            send_broker_error(
                                transport,
                                &mut codec,
                                BrokerErrorCode::SourceIdentityMismatch,
                            )?;
                            continue;
                        }
                        send_message(transport, &mut codec, &opened_message)?;
                        opened = Some(OpenSource { handle_id, source });
                    }
                    Err(code) => send_broker_error(transport, &mut codec, code)?,
                }
            }
            Message::ReadAt {
                handle_id: requested_handle,
                offset,
                length,
            } => {
                let Some(open_source) = opened.as_mut() else {
                    send_broker_error(transport, &mut codec, BrokerErrorCode::InvalidHandle)?;
                    continue;
                };
                if requested_handle != open_source.handle_id {
                    send_broker_error(transport, &mut codec, BrokerErrorCode::InvalidHandle)?;
                    continue;
                }

                let geometry = open_source.source.geometry();
                let Some(end) = offset.checked_add(u64::from(length)) else {
                    send_broker_error(transport, &mut codec, BrokerErrorCode::ReadOutOfRange)?;
                    continue;
                };
                if end > geometry.size {
                    send_broker_error(transport, &mut codec, BrokerErrorCode::ReadOutOfRange)?;
                    continue;
                }

                if let Err(code) = open_source.source.revalidate() {
                    if source_identity_failure(code) {
                        opened = None;
                    }
                    send_broker_error(transport, &mut codec, code)?;
                    continue;
                }

                let length = usize::try_from(length).map_err(|_| SessionError::ProtocolFailure)?;
                let mut bytes = vec![0_u8; length];
                match open_source.source.read_exact_at(offset, &mut bytes) {
                    Ok(()) => {
                        send_message(transport, &mut codec, &Message::ReadData { bytes })?;
                    }
                    Err(code) => {
                        if source_identity_failure(code) {
                            opened = None;
                        }
                        send_broker_error(transport, &mut codec, code)?;
                    }
                }
            }
            Message::CloseSource {
                handle_id: requested_handle,
            } => {
                if opened
                    .as_ref()
                    .is_some_and(|source| source.handle_id == requested_handle)
                {
                    opened = None;
                    send_message(
                        transport,
                        &mut codec,
                        &Message::Closed {
                            handle_id: requested_handle,
                        },
                    )?;
                } else {
                    send_broker_error(transport, &mut codec, BrokerErrorCode::InvalidHandle)?;
                }
            }
            Message::Shutdown => {
                drop(opened.take());
                return Ok(());
            }
            Message::Hello { .. }
            | Message::HelloAck { .. }
            | Message::Opened { .. }
            | Message::ReadData { .. }
            | Message::Closed { .. }
            | Message::Error { .. } => {
                send_broker_error(transport, &mut codec, BrokerErrorCode::InvalidRequest)?;
            }
        }
    }
}

fn source_identity_failure(code: BrokerErrorCode) -> bool {
    matches!(
        code,
        BrokerErrorCode::SourceNotFound
            | BrokerErrorCode::SourceIdentityMismatch
            | BrokerErrorCode::SourceUnavailable
            | BrokerErrorCode::SourceChanged
    )
}

fn send_broker_error<T: Write>(
    transport: &mut T,
    codec: &mut SessionCodec,
    code: BrokerErrorCode,
) -> Result<(), SessionError> {
    send_message(transport, codec, &Message::Error { code })
}

fn send_message<T: Write>(
    transport: &mut T,
    codec: &mut SessionCodec,
    message: &Message,
) -> Result<(), SessionError> {
    codec
        .write_message(transport, message)
        .map_err(protocol_error)?;
    transport
        .flush()
        .map_err(|_| SessionError::TransportFailure)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerRuntimeError {
    UnsupportedPlatform,
    TransportUnavailable,
    PeerVerificationFailed,
    SourceBoundaryUnavailable,
    SessionFailed,
}

impl fmt::Display for BrokerRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => "elevated broker is unsupported on this platform",
            Self::TransportUnavailable => "broker transport unavailable",
            Self::PeerVerificationFailed => "broker peer verification failed",
            Self::SourceBoundaryUnavailable => "broker source boundary unavailable",
            Self::SessionFailed => "broker session failed",
        })
    }
}

impl std::error::Error for BrokerRuntimeError {}

#[cfg(windows)]
mod windows_runtime {
    use std::time::Instant;

    use um_io_windows::{connect_broker_pipe, RawVolume, StorageError};

    use super::{
        serve_session, BrokerArgs, BrokerErrorCode, BrokerRuntimeError, BrokerSourceGeometry,
        ReadOnlyBrokerSource, ReadOnlySourceFactory,
    };

    pub(super) fn run(args: &BrokerArgs) -> Result<(), BrokerRuntimeError> {
        let mut connection = connect_broker_pipe(args.pipe_suffix(), args.parent_pid())
            .map_err(|_| BrokerRuntimeError::TransportUnavailable)?;
        if connection.peer_process_id() != args.parent_pid()
            || !connection
                .peer_is_alive()
                .map_err(|_| BrokerRuntimeError::PeerVerificationFailed)?
            || !connection
                .peer_is_packaged_desktop()
                .map_err(|_| BrokerRuntimeError::PeerVerificationFailed)?
        {
            return Err(BrokerRuntimeError::PeerVerificationFailed);
        }

        let mut factory = WindowsSourceFactory;
        serve_session(&mut connection, &mut factory, session_handle_id(args))
            .map_err(|_| BrokerRuntimeError::SessionFailed)
    }

    fn session_handle_id(args: &BrokerArgs) -> u64 {
        let prefix = args.pipe_suffix().get(..16).unwrap_or_default();
        let random_prefix = u64::from_str_radix(prefix, 16).unwrap_or(1);
        let mixed = random_prefix.rotate_left(17) ^ u64::from(args.parent_pid());
        if mixed == 0 {
            1
        } else {
            mixed
        }
    }

    struct WindowsSourceFactory;

    impl ReadOnlySourceFactory for WindowsSourceFactory {
        type Source = WindowsSource;

        fn open_source(&mut self, source_id: &str) -> Result<Self::Source, BrokerErrorCode> {
            let source = RawVolume::open_by_identity(source_id).map_err(map_open_error)?;
            if source.identity().id != source_id {
                return Err(BrokerErrorCode::SourceIdentityMismatch);
            }
            source
                .revalidate_identity()
                .map_err(map_revalidation_error)?;
            Ok(WindowsSource {
                source,
                reads_since_revalidation: 0,
                last_revalidation: Instant::now(),
            })
        }
    }

    struct WindowsSource {
        source: RawVolume,
        reads_since_revalidation: u32,
        last_revalidation: Instant,
    }

    impl ReadOnlyBrokerSource for WindowsSource {
        fn geometry(&self) -> BrokerSourceGeometry {
            let layout = self.source.sector_layout();
            BrokerSourceGeometry {
                size: self.source.len(),
                logical_sector: layout.logical,
                physical_sector: layout.physical,
                physical_disk_number: self.source.physical_disk_number(),
            }
        }

        fn revalidate(&mut self) -> Result<(), BrokerErrorCode> {
            // `serve_session` calls this before every valid ReadAt. Only the
            // expensive Windows identity re-enumeration is cadence-limited;
            // `read_exact_at` still reaches the source for every request.
            self.reads_since_revalidation = self.reads_since_revalidation.saturating_add(1);
            let now = Instant::now();
            if !super::revalidation_due(
                self.reads_since_revalidation,
                now.duration_since(self.last_revalidation),
            ) {
                return Ok(());
            }

            self.source
                .revalidate_identity()
                .map_err(map_revalidation_error)?;
            self.reads_since_revalidation = 0;
            self.last_revalidation = now;
            Ok(())
        }

        fn read_exact_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<(), BrokerErrorCode> {
            self.source
                .read_exact_at(offset, buffer)
                .map_err(map_read_error)
        }
    }

    fn map_open_error(error: StorageError) -> BrokerErrorCode {
        match error {
            StorageError::SourceIdentityUnavailable => BrokerErrorCode::SourceNotFound,
            StorageError::SourceIdentityChanged => BrokerErrorCode::SourceChanged,
            StorageError::UnsupportedSource(_) => BrokerErrorCode::SourceUnavailable,
            _ => BrokerErrorCode::SourceUnavailable,
        }
    }

    fn map_revalidation_error(error: StorageError) -> BrokerErrorCode {
        match error {
            StorageError::SourceIdentityUnavailable | StorageError::SourceIdentityChanged => {
                BrokerErrorCode::SourceChanged
            }
            _ => BrokerErrorCode::SourceUnavailable,
        }
    }

    fn map_read_error(error: StorageError) -> BrokerErrorCode {
        match error {
            StorageError::ReadPlan(_) => BrokerErrorCode::ReadOutOfRange,
            StorageError::SourceIdentityUnavailable | StorageError::SourceIdentityChanged => {
                BrokerErrorCode::SourceChanged
            }
            _ => BrokerErrorCode::ReadFailed,
        }
    }
}

#[cfg(windows)]
pub fn run_platform(args: &BrokerArgs) -> Result<(), BrokerRuntimeError> {
    windows_runtime::run(args)
}

#[cfg(not(windows))]
pub fn run_platform(_args: &BrokerArgs) -> Result<(), BrokerRuntimeError> {
    Err(BrokerRuntimeError::UnsupportedPlatform)
}

#[cfg(test)]
mod revalidation_policy_tests {
    use std::time::Duration;

    use super::revalidation_due;

    #[test]
    fn elevated_broker_revalidation_001_requires_both_read_and_time_bounds() {
        assert!(!revalidation_due(255, Duration::from_secs(10)));
        assert!(!revalidation_due(256, Duration::from_millis(999)));
        assert!(revalidation_due(256, Duration::from_secs(1)));
    }

    #[test]
    fn elevated_broker_revalidation_002_handles_saturated_read_count() {
        assert!(!revalidation_due(u32::MAX, Duration::from_millis(999)));
        assert!(revalidation_due(u32::MAX, Duration::from_secs(1)));
    }
}
