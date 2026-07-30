use std::io::{self, Cursor, Read, Write};

use crate::message::{BrokerErrorCode, Message, Opcode};
use crate::{FramePart, ProtocolError, SequenceDirection, MAX_READ_LEN};

pub const PROTOCOL_MAGIC: [u8; 4] = *b"UMBP";
pub const PROTOCOL_VERSION: u16 = 3;
pub const HEADER_LEN: usize = 20;
pub const MAX_PAYLOAD_LEN: usize = MAX_READ_LEN;
pub const MAX_FRAME_LEN: usize = HEADER_LEN + MAX_PAYLOAD_LEN;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub sequence: u64,
    pub message: Message,
}

pub fn encode_frame(sequence: u64, message: &Message) -> Result<Vec<u8>, ProtocolError> {
    validate_sequence(sequence)?;
    let payload = encode_payload(message)?;
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(ProtocolError::PayloadTooLarge {
            length: payload.len(),
            maximum: MAX_PAYLOAD_LEN,
        });
    }
    let frame_length = HEADER_LEN
        .checked_add(payload.len())
        .ok_or(ProtocolError::ArithmeticOverflow)?;
    if frame_length > MAX_FRAME_LEN {
        return Err(ProtocolError::PayloadTooLarge {
            length: payload.len(),
            maximum: MAX_PAYLOAD_LEN,
        });
    }
    let payload_length =
        u32::try_from(payload.len()).map_err(|_| ProtocolError::ArithmeticOverflow)?;

    let mut frame = Vec::with_capacity(frame_length);
    frame.extend_from_slice(&PROTOCOL_MAGIC);
    frame.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    frame.extend_from_slice(&(message.opcode() as u16).to_le_bytes());
    frame.extend_from_slice(&sequence.to_le_bytes());
    frame.extend_from_slice(&payload_length.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn write_frame<W: Write>(
    writer: &mut W,
    sequence: u64,
    message: &Message,
) -> Result<(), ProtocolError> {
    let frame = encode_frame(sequence, message)?;
    writer.write_all(&frame).map_err(ProtocolError::Io)
}

pub fn read_frame<R: Read>(reader: &mut R) -> Result<Frame, ProtocolError> {
    let mut header = [0_u8; HEADER_LEN];
    read_exact_bounded(reader, &mut header, FramePart::Header)?;

    let mut magic = [0_u8; 4];
    magic.copy_from_slice(&header[0..4]);
    if magic != PROTOCOL_MAGIC {
        return Err(ProtocolError::InvalidMagic { actual: magic });
    }

    let version = u16::from_le_bytes([header[4], header[5]]);
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion { actual: version });
    }

    let opcode_raw = u16::from_le_bytes([header[6], header[7]]);
    let opcode = Opcode::try_from(opcode_raw)?;

    let sequence = u64::from_le_bytes([
        header[8], header[9], header[10], header[11], header[12], header[13], header[14],
        header[15],
    ]);
    validate_sequence(sequence)?;

    let payload_u32 = u32::from_le_bytes([header[16], header[17], header[18], header[19]]);
    let payload_length =
        usize::try_from(payload_u32).map_err(|_| ProtocolError::ArithmeticOverflow)?;
    if payload_length > MAX_PAYLOAD_LEN {
        return Err(ProtocolError::PayloadTooLarge {
            length: payload_length,
            maximum: MAX_PAYLOAD_LEN,
        });
    }
    HEADER_LEN
        .checked_add(payload_length)
        .filter(|length| *length <= MAX_FRAME_LEN)
        .ok_or(ProtocolError::ArithmeticOverflow)?;

    let mut payload = vec![0_u8; payload_length];
    read_exact_bounded(reader, &mut payload, FramePart::Payload)?;
    let message = decode_payload(opcode, &payload)?;

    Ok(Frame { sequence, message })
}

pub fn decode_frame(bytes: &[u8]) -> Result<Frame, ProtocolError> {
    let mut cursor = Cursor::new(bytes);
    let frame = read_frame(&mut cursor)?;
    let consumed =
        usize::try_from(cursor.position()).map_err(|_| ProtocolError::ArithmeticOverflow)?;
    let trailing = bytes
        .len()
        .checked_sub(consumed)
        .ok_or(ProtocolError::ArithmeticOverflow)?;
    if trailing != 0 {
        return Err(ProtocolError::TrailingFrameBytes { count: trailing });
    }
    Ok(frame)
}

/// Stateful bidirectional codec with independent, contiguous sequence spaces.
///
/// Both directions begin at sequence one. A repeated or older inbound value is
/// a replay; a gap is a protocol error. Sequence state advances only after a
/// complete valid frame has been transferred.
#[derive(Debug, Clone)]
pub struct SessionCodec {
    next_outbound: Option<u64>,
    next_inbound: Option<u64>,
}

impl SessionCodec {
    pub const fn new() -> Self {
        Self {
            next_outbound: Some(1),
            next_inbound: Some(1),
        }
    }

    pub fn with_sequences(next_outbound: u64, next_inbound: u64) -> Result<Self, ProtocolError> {
        validate_sequence(next_outbound)?;
        validate_sequence(next_inbound)?;
        Ok(Self {
            next_outbound: Some(next_outbound),
            next_inbound: Some(next_inbound),
        })
    }

    pub const fn next_outbound_sequence(&self) -> Option<u64> {
        self.next_outbound
    }

    pub const fn next_inbound_sequence(&self) -> Option<u64> {
        self.next_inbound
    }

    pub fn write_message<W: Write>(
        &mut self,
        writer: &mut W,
        message: &Message,
    ) -> Result<u64, ProtocolError> {
        let sequence = self.next_outbound.ok_or(ProtocolError::SequenceExhausted {
            direction: SequenceDirection::Outbound,
        })?;
        write_frame(writer, sequence, message)?;
        self.next_outbound = sequence.checked_add(1);
        Ok(sequence)
    }

    pub fn read_message<R: Read>(&mut self, reader: &mut R) -> Result<Frame, ProtocolError> {
        let expected = self.next_inbound.ok_or(ProtocolError::SequenceExhausted {
            direction: SequenceDirection::Inbound,
        })?;
        let frame = read_frame(reader)?;
        if frame.sequence < expected {
            let last_accepted = expected
                .checked_sub(1)
                .ok_or(ProtocolError::ArithmeticOverflow)?;
            return Err(ProtocolError::Replay {
                sequence: frame.sequence,
                last_accepted,
            });
        }
        if frame.sequence > expected {
            return Err(ProtocolError::OutOfSequence {
                expected,
                actual: frame.sequence,
            });
        }
        self.next_inbound = expected.checked_add(1);
        Ok(frame)
    }
}

impl Default for SessionCodec {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_sequence(sequence: u64) -> Result<(), ProtocolError> {
    if sequence == 0 {
        return Err(ProtocolError::InvalidSequence { actual: sequence });
    }
    Ok(())
}

fn read_exact_bounded<R: Read>(
    reader: &mut R,
    mut destination: &mut [u8],
    part: FramePart,
) -> Result<(), ProtocolError> {
    let expected = destination.len();
    let mut actual = 0_usize;

    while !destination.is_empty() {
        match reader.read(destination) {
            Ok(0) => {
                return Err(ProtocolError::Truncated {
                    part,
                    expected,
                    actual,
                });
            }
            Ok(read) => {
                if read > destination.len() {
                    return Err(ProtocolError::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "reader returned more bytes than requested",
                    )));
                }
                actual = actual
                    .checked_add(read)
                    .ok_or(ProtocolError::ArithmeticOverflow)?;
                let (_, remaining) = destination.split_at_mut(read);
                destination = remaining;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(ProtocolError::Truncated {
                    part,
                    expected,
                    actual,
                });
            }
            Err(error) => return Err(ProtocolError::Io(error)),
        }
    }
    Ok(())
}

fn encode_payload(message: &Message) -> Result<Vec<u8>, ProtocolError> {
    message.validate()?;
    let mut payload = PayloadWriter::new();
    match message {
        Message::Hello { challenge } | Message::HelloAck { challenge } => {
            payload.bytes(challenge)?;
        }
        Message::Shutdown => {}
        Message::OpenSource { source_id } => {
            payload.identifier(source_id)?;
        }
        Message::Opened {
            handle_id,
            size,
            logical_sector,
            physical_sector,
            physical_disk_number,
        } => {
            payload.u64(*handle_id)?;
            payload.u64(*size)?;
            payload.u32(*logical_sector)?;
            payload.u32(*physical_sector)?;
            payload.u32(*physical_disk_number)?;
        }
        Message::ReadAt {
            handle_id,
            offset,
            length,
        } => {
            payload.u64(*handle_id)?;
            payload.u64(*offset)?;
            payload.u32(*length)?;
        }
        Message::ReadData { bytes } => payload.bytes(bytes)?,
        Message::CloseSource { handle_id } | Message::Closed { handle_id } => {
            payload.u64(*handle_id)?;
        }
        Message::Error { code } => payload.u16(*code as u16)?,
    }
    Ok(payload.finish())
}

fn decode_payload(opcode: Opcode, payload: &[u8]) -> Result<Message, ProtocolError> {
    if opcode == Opcode::ReadData {
        let message = Message::ReadData {
            bytes: payload.to_vec(),
        };
        message.validate()?;
        return Ok(message);
    }

    let mut reader = PayloadReader::new(opcode, payload);
    let message = match opcode {
        Opcode::Hello => Message::Hello {
            challenge: reader.array_32()?,
        },
        Opcode::HelloAck => Message::HelloAck {
            challenge: reader.array_32()?,
        },
        Opcode::OpenSource => Message::OpenSource {
            source_id: reader.identifier("source_id", crate::MAX_SOURCE_ID_LEN)?,
        },
        Opcode::Opened => Message::Opened {
            handle_id: reader.u64()?,
            size: reader.u64()?,
            logical_sector: reader.u32()?,
            physical_sector: reader.u32()?,
            physical_disk_number: reader.u32()?,
        },
        Opcode::ReadAt => Message::ReadAt {
            handle_id: reader.u64()?,
            offset: reader.u64()?,
            length: reader.u32()?,
        },
        Opcode::CloseSource => Message::CloseSource {
            handle_id: reader.u64()?,
        },
        Opcode::Closed => Message::Closed {
            handle_id: reader.u64()?,
        },
        Opcode::Shutdown => Message::Shutdown,
        Opcode::Error => Message::Error {
            code: BrokerErrorCode::try_from(reader.u16()?)?,
        },
        Opcode::ReadData => unreachable!("handled before payload cursor creation"),
    };
    reader.finish()?;
    message.validate()?;
    Ok(message)
}

struct PayloadWriter {
    bytes: Vec<u8>,
}

impl PayloadWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u16(&mut self, value: u16) -> Result<(), ProtocolError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), ProtocolError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), ProtocolError> {
        self.bytes(&value.to_le_bytes())
    }

    fn identifier(&mut self, value: &str) -> Result<(), ProtocolError> {
        let length = u16::try_from(value.len()).map_err(|_| ProtocolError::ArithmeticOverflow)?;
        self.u16(length)?;
        self.bytes(value.as_bytes())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), ProtocolError> {
        let new_length = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or(ProtocolError::ArithmeticOverflow)?;
        if new_length > MAX_PAYLOAD_LEN {
            return Err(ProtocolError::PayloadTooLarge {
                length: new_length,
                maximum: MAX_PAYLOAD_LEN,
            });
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct PayloadReader<'a> {
    opcode: Opcode,
    bytes: &'a [u8],
    position: usize,
}

impl<'a> PayloadReader<'a> {
    fn new(opcode: Opcode, bytes: &'a [u8]) -> Self {
        Self {
            opcode,
            bytes,
            position: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ProtocolError::ArithmeticOverflow)?;
        if end > self.bytes.len() {
            let remaining = self
                .bytes
                .len()
                .checked_sub(self.position)
                .ok_or(ProtocolError::ArithmeticOverflow)?;
            return Err(ProtocolError::PayloadUnderflow {
                opcode: self.opcode as u16,
                needed: length,
                remaining,
            });
        }
        let result = &self.bytes[self.position..end];
        self.position = end;
        Ok(result)
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> Result<u64, ProtocolError> {
        let bytes = self.take(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn array_32(&mut self) -> Result<[u8; 32], ProtocolError> {
        let bytes = self.take(32)?;
        let mut result = [0_u8; 32];
        result.copy_from_slice(bytes);
        Ok(result)
    }

    fn identifier(&mut self, field: &'static str, maximum: usize) -> Result<String, ProtocolError> {
        let length = usize::from(self.u16()?);
        if length > maximum {
            return Err(ProtocolError::InvalidFieldLength {
                field,
                length,
                maximum,
            });
        }
        let bytes = self.take(length)?;
        let value = std::str::from_utf8(bytes)
            .map_err(|_| ProtocolError::InvalidUtf8 { field })?
            .to_owned();
        Ok(value)
    }

    fn finish(self) -> Result<(), ProtocolError> {
        let remaining = self
            .bytes
            .len()
            .checked_sub(self.position)
            .ok_or(ProtocolError::ArithmeticOverflow)?;
        if remaining != 0 {
            return Err(ProtocolError::TrailingPayload {
                opcode: self.opcode as u16,
                remaining,
            });
        }
        Ok(())
    }
}
