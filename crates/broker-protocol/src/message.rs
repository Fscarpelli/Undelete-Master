use crate::ProtocolError;

/// Maximum encoded read payload and maximum requested read length.
pub const MAX_READ_LEN: usize = 1024 * 1024;
pub const MAX_SOURCE_ID_LEN: usize = 128;
pub const MIN_SECTOR_SIZE: u32 = 512;
pub const MAX_SECTOR_SIZE: u32 = 1024 * 1024;

/// The complete protocol operation allowlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Opcode {
    Hello = 1,
    HelloAck = 2,
    OpenSource = 3,
    Opened = 4,
    ReadAt = 5,
    ReadData = 6,
    CloseSource = 7,
    Closed = 8,
    Shutdown = 9,
    Error = 10,
}

pub const ALL_OPCODES: [Opcode; 10] = [
    Opcode::Hello,
    Opcode::HelloAck,
    Opcode::OpenSource,
    Opcode::Opened,
    Opcode::ReadAt,
    Opcode::ReadData,
    Opcode::CloseSource,
    Opcode::Closed,
    Opcode::Shutdown,
    Opcode::Error,
];

impl TryFrom<u16> for Opcode {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, ProtocolError> {
        match value {
            1 => Ok(Self::Hello),
            2 => Ok(Self::HelloAck),
            3 => Ok(Self::OpenSource),
            4 => Ok(Self::Opened),
            5 => Ok(Self::ReadAt),
            6 => Ok(Self::ReadData),
            7 => Ok(Self::CloseSource),
            8 => Ok(Self::Closed),
            9 => Ok(Self::Shutdown),
            10 => Ok(Self::Error),
            actual => Err(ProtocolError::UnknownOpcode { actual }),
        }
    }
}

/// Stable broker-side failure codes. There is intentionally no free-form
/// broker diagnostic in a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum BrokerErrorCode {
    InvalidRequest = 1,
    AuthenticationFailed = 2,
    SourceNotFound = 3,
    SourceIdentityMismatch = 4,
    SourceUnavailable = 5,
    InvalidHandle = 6,
    ReadOutOfRange = 7,
    ReadFailed = 8,
    SourceChanged = 9,
    InternalFailure = 10,
}

impl TryFrom<u16> for BrokerErrorCode {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::InvalidRequest),
            2 => Ok(Self::AuthenticationFailed),
            3 => Ok(Self::SourceNotFound),
            4 => Ok(Self::SourceIdentityMismatch),
            5 => Ok(Self::SourceUnavailable),
            6 => Ok(Self::InvalidHandle),
            7 => Ok(Self::ReadOutOfRange),
            8 => Ok(Self::ReadFailed),
            9 => Ok(Self::SourceChanged),
            10 => Ok(Self::InternalFailure),
            actual => Err(ProtocolError::UnknownErrorCode { actual }),
        }
    }
}

/// Allowlisted protocol messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Hello {
        challenge: [u8; 32],
    },
    HelloAck {
        challenge: [u8; 32],
    },
    OpenSource {
        source_id: String,
    },
    Opened {
        handle_id: u64,
        size: u64,
        logical_sector: u32,
        physical_sector: u32,
    },
    ReadAt {
        handle_id: u64,
        offset: u64,
        length: u32,
    },
    ReadData {
        bytes: Vec<u8>,
    },
    CloseSource {
        handle_id: u64,
    },
    Closed {
        handle_id: u64,
    },
    Shutdown,
    Error {
        code: BrokerErrorCode,
    },
}

impl Message {
    pub const fn opcode(&self) -> Opcode {
        match self {
            Self::Hello { .. } => Opcode::Hello,
            Self::HelloAck { .. } => Opcode::HelloAck,
            Self::OpenSource { .. } => Opcode::OpenSource,
            Self::Opened { .. } => Opcode::Opened,
            Self::ReadAt { .. } => Opcode::ReadAt,
            Self::ReadData { .. } => Opcode::ReadData,
            Self::CloseSource { .. } => Opcode::CloseSource,
            Self::Closed { .. } => Opcode::Closed,
            Self::Shutdown => Opcode::Shutdown,
            Self::Error { .. } => Opcode::Error,
        }
    }

    /// Validates all semantic bounds before a message crosses the boundary.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Hello { .. } | Self::HelloAck { .. } | Self::Shutdown | Self::Error { .. } => {
                Ok(())
            }
            Self::OpenSource { source_id } => {
                validate_identifier("source_id", source_id, MAX_SOURCE_ID_LEN)
            }
            Self::Opened {
                handle_id,
                size,
                logical_sector,
                physical_sector,
            } => {
                validate_nonzero("handle_id", *handle_id)?;
                validate_nonzero("size", *size)?;
                validate_sector_layout(*logical_sector, *physical_sector)
            }
            Self::ReadAt {
                handle_id,
                offset,
                length,
            } => {
                validate_nonzero("handle_id", *handle_id)?;
                let length_usize =
                    usize::try_from(*length).map_err(|_| ProtocolError::ArithmeticOverflow)?;
                if length_usize == 0 || length_usize > MAX_READ_LEN {
                    return Err(ProtocolError::InvalidNumericValue {
                        field: "length",
                        value: u64::from(*length),
                    });
                }
                offset
                    .checked_add(u64::from(*length))
                    .ok_or(ProtocolError::RangeOverflow)?;
                Ok(())
            }
            Self::ReadData { bytes } => {
                if bytes.len() > MAX_READ_LEN {
                    return Err(ProtocolError::InvalidFieldLength {
                        field: "bytes",
                        length: bytes.len(),
                        maximum: MAX_READ_LEN,
                    });
                }
                Ok(())
            }
            Self::CloseSource { handle_id } | Self::Closed { handle_id } => {
                validate_nonzero("handle_id", *handle_id)
            }
        }
    }
}

fn validate_nonzero(field: &'static str, value: u64) -> Result<(), ProtocolError> {
    if value == 0 {
        return Err(ProtocolError::InvalidNumericValue { field, value });
    }
    Ok(())
}

fn validate_identifier(
    field: &'static str,
    value: &str,
    maximum: usize,
) -> Result<(), ProtocolError> {
    if value.is_empty() {
        return Err(ProtocolError::EmptyField { field });
    }
    if value.len() > maximum {
        return Err(ProtocolError::InvalidFieldLength {
            field,
            length: value.len(),
            maximum,
        });
    }
    if let Some((index, _)) = value
        .bytes()
        .enumerate()
        .find(|(_, byte)| !is_identifier_byte(*byte))
    {
        return Err(ProtocolError::InvalidIdentifier { field, index });
    }
    Ok(())
}

const fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || byte == b'-'
        || byte == b'_'
        || byte == b'.'
        || byte == b'{'
        || byte == b'}'
}

fn validate_sector_layout(logical: u32, physical: u32) -> Result<(), ProtocolError> {
    if !valid_sector_size(logical) {
        return Err(ProtocolError::InvalidNumericValue {
            field: "logical_sector",
            value: u64::from(logical),
        });
    }
    if !valid_sector_size(physical) || physical < logical || physical % logical != 0 {
        return Err(ProtocolError::InvalidNumericValue {
            field: "physical_sector",
            value: u64::from(physical),
        });
    }
    Ok(())
}

const fn valid_sector_size(value: u32) -> bool {
    value >= MIN_SECTOR_SIZE && value <= MAX_SECTOR_SIZE && value.is_power_of_two()
}

/// Machine-readable schema manifest used by compatibility and architecture
/// checks. The field list is deliberately exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageSchema {
    pub opcode: Opcode,
    pub name: &'static str,
    pub fields: &'static [&'static str],
}

pub const MESSAGE_SCHEMA: [MessageSchema; 10] = [
    MessageSchema {
        opcode: Opcode::Hello,
        name: "Hello",
        fields: &["challenge"],
    },
    MessageSchema {
        opcode: Opcode::HelloAck,
        name: "HelloAck",
        fields: &["challenge"],
    },
    MessageSchema {
        opcode: Opcode::OpenSource,
        name: "OpenSource",
        fields: &["source_id"],
    },
    MessageSchema {
        opcode: Opcode::Opened,
        name: "Opened",
        fields: &["handle_id", "size", "logical_sector", "physical_sector"],
    },
    MessageSchema {
        opcode: Opcode::ReadAt,
        name: "ReadAt",
        fields: &["handle_id", "offset", "length"],
    },
    MessageSchema {
        opcode: Opcode::ReadData,
        name: "ReadData",
        fields: &["bytes"],
    },
    MessageSchema {
        opcode: Opcode::CloseSource,
        name: "CloseSource",
        fields: &["handle_id"],
    },
    MessageSchema {
        opcode: Opcode::Closed,
        name: "Closed",
        fields: &["handle_id"],
    },
    MessageSchema {
        opcode: Opcode::Shutdown,
        name: "Shutdown",
        fields: &[],
    },
    MessageSchema {
        opcode: Opcode::Error,
        name: "Error",
        fields: &["code"],
    },
];
