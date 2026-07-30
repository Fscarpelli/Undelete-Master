//! Bounded, synchronous wire protocol for the read-only elevated broker.
//!
//! The protocol is deliberately narrow. It transports only an inventory-issued
//! source identifier, expected source identity, bounded read requests, read
//! results, lifecycle messages, and stable error codes.
//!
//! Handshake version 2 carries a 32-byte client nonce in `Hello`; `HelloAck`
//! echoes those exact bytes. The echo binds the response to this connection but
//! is not a MAC or mutual secret. Native named-pipe peer-PID verification
//! remains the primary broker identity check.

#![forbid(unsafe_code)]

mod codec;
mod error;
mod message;

pub use codec::{
    decode_frame, encode_frame, read_frame, write_frame, Frame, SessionCodec, HEADER_LEN,
    MAX_FRAME_LEN, MAX_PAYLOAD_LEN, PROTOCOL_MAGIC, PROTOCOL_VERSION,
};
pub use error::{FramePart, ProtocolError, SequenceDirection};
pub use message::{
    BrokerErrorCode, Message, MessageSchema, Opcode, ALL_OPCODES, MAX_READ_LEN, MAX_SECTOR_SIZE,
    MAX_SOURCE_ID_LEN, MESSAGE_SCHEMA, MIN_SECTOR_SIZE,
};
