use std::fmt;
use std::io;

/// Fixed frame section in which a transport truncation occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePart {
    Header,
    Payload,
}

/// Direction whose sequence number space was exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceDirection {
    Inbound,
    Outbound,
}

/// Fail-closed protocol and transport errors.
#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    Truncated {
        part: FramePart,
        expected: usize,
        actual: usize,
    },
    InvalidMagic {
        actual: [u8; 4],
    },
    UnsupportedVersion {
        actual: u16,
    },
    UnknownOpcode {
        actual: u16,
    },
    PayloadTooLarge {
        length: usize,
        maximum: usize,
    },
    PayloadUnderflow {
        opcode: u16,
        needed: usize,
        remaining: usize,
    },
    TrailingPayload {
        opcode: u16,
        remaining: usize,
    },
    InvalidUtf8 {
        field: &'static str,
    },
    EmptyField {
        field: &'static str,
    },
    InvalidFieldLength {
        field: &'static str,
        length: usize,
        maximum: usize,
    },
    InvalidIdentifier {
        field: &'static str,
        index: usize,
    },
    InvalidNumericValue {
        field: &'static str,
        value: u64,
    },
    UnknownErrorCode {
        actual: u16,
    },
    RangeOverflow,
    ArithmeticOverflow,
    InvalidSequence {
        actual: u64,
    },
    Replay {
        sequence: u64,
        last_accepted: u64,
    },
    OutOfSequence {
        expected: u64,
        actual: u64,
    },
    SequenceExhausted {
        direction: SequenceDirection,
    },
    TrailingFrameBytes {
        count: usize,
    },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "protocol I/O failed: {error}"),
            Self::Truncated {
                part,
                expected,
                actual,
            } => write!(
                formatter,
                "{part:?} was truncated: expected {expected} bytes, received {actual}"
            ),
            Self::InvalidMagic { actual } => {
                write!(formatter, "invalid frame magic: {actual:02x?}")
            }
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported protocol version {actual}")
            }
            Self::UnknownOpcode { actual } => write!(formatter, "unknown opcode {actual}"),
            Self::PayloadTooLarge { length, maximum } => {
                write!(
                    formatter,
                    "payload length {length} exceeds maximum {maximum}"
                )
            }
            Self::PayloadUnderflow {
                opcode,
                needed,
                remaining,
            } => write!(
                formatter,
                "opcode {opcode} needs {needed} payload bytes but only {remaining} remain"
            ),
            Self::TrailingPayload { opcode, remaining } => {
                write!(
                    formatter,
                    "opcode {opcode} has {remaining} trailing payload bytes"
                )
            }
            Self::InvalidUtf8 { field } => write!(formatter, "{field} is not valid UTF-8"),
            Self::EmptyField { field } => write!(formatter, "{field} must not be empty"),
            Self::InvalidFieldLength {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "{field} length {length} exceeds maximum {maximum}"
            ),
            Self::InvalidIdentifier { field, index } => {
                write!(
                    formatter,
                    "{field} contains an invalid byte at index {index}"
                )
            }
            Self::InvalidNumericValue { field, value } => {
                write!(formatter, "{field} has invalid value {value}")
            }
            Self::UnknownErrorCode { actual } => {
                write!(formatter, "unknown broker error code {actual}")
            }
            Self::RangeOverflow => formatter.write_str("requested byte range overflows"),
            Self::ArithmeticOverflow => formatter.write_str("protocol arithmetic overflow"),
            Self::InvalidSequence { actual } => {
                write!(formatter, "sequence number {actual} is invalid")
            }
            Self::Replay {
                sequence,
                last_accepted,
            } => write!(
                formatter,
                "sequence {sequence} replays or precedes accepted sequence {last_accepted}"
            ),
            Self::OutOfSequence { expected, actual } => {
                write!(formatter, "expected sequence {expected}, received {actual}")
            }
            Self::SequenceExhausted { direction } => {
                write!(formatter, "{direction:?} sequence space is exhausted")
            }
            Self::TrailingFrameBytes { count } => {
                write!(formatter, "frame has {count} trailing bytes")
            }
        }
    }
}

impl std::error::Error for ProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}
