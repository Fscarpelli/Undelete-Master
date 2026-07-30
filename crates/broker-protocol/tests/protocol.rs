use std::io::{self, Cursor, Write};

use um_broker_protocol::{
    decode_frame, encode_frame, read_frame, BrokerErrorCode, FramePart, Message, ProtocolError,
    SessionCodec, HEADER_LEN, MAX_FRAME_LEN, MAX_PAYLOAD_LEN, MAX_READ_LEN, PROTOCOL_MAGIC,
    PROTOCOL_VERSION,
};

fn all_messages() -> Vec<Message> {
    vec![
        Message::Hello {
            challenge: [0xA5; 32],
        },
        Message::HelloAck {
            challenge: [0x5A; 32],
        },
        Message::OpenSource {
            source_id: "inventory-source-17".to_owned(),
        },
        Message::Opened {
            handle_id: 71,
            size: 8 * 1024 * 1024,
            logical_sector: 512,
            physical_sector: 4096,
        },
        Message::ReadAt {
            handle_id: 71,
            offset: 4096,
            length: 8192,
        },
        Message::ReadData {
            bytes: vec![0x3C; 8192],
        },
        Message::CloseSource { handle_id: 71 },
        Message::Closed { handle_id: 71 },
        Message::Shutdown,
        Message::Error {
            code: BrokerErrorCode::SourceChanged,
        },
    ]
}

fn raw_frame(opcode: u16, sequence: u64, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
    frame.extend_from_slice(&PROTOCOL_MAGIC);
    frame.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    frame.extend_from_slice(&opcode.to_le_bytes());
    frame.extend_from_slice(&sequence.to_le_bytes());
    frame.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("test payload length fits u32")
            .to_le_bytes(),
    );
    frame.extend_from_slice(payload);
    frame
}

#[test]
fn broker_protocol_roundtrip_001_round_trips_every_allowlisted_message() {
    for (index, message) in all_messages().into_iter().enumerate() {
        let sequence = u64::try_from(index + 1).expect("small test sequence");
        let encoded = encode_frame(sequence, &message).expect("message should encode");
        let decoded = decode_frame(&encoded).expect("message should decode");
        assert_eq!(decoded.sequence, sequence);
        assert_eq!(decoded.message, message);
    }
}

#[test]
fn broker_protocol_roundtrip_002_supports_maximum_read_data_frame() {
    let message = Message::ReadData {
        bytes: vec![0xD4; MAX_READ_LEN],
    };
    let encoded = encode_frame(1, &message).expect("maximum data frame should encode");
    assert_eq!(encoded.len(), MAX_FRAME_LEN);
    assert_eq!(encoded.len() - HEADER_LEN, MAX_PAYLOAD_LEN);
    assert_eq!(
        decode_frame(&encoded).expect("frame should decode").message,
        message
    );
}

#[test]
fn broker_protocol_stream_001_reads_two_consecutive_frames() {
    let first = encode_frame(
        1,
        &Message::HelloAck {
            challenge: [0x6D; 32],
        },
    )
    .expect("frame");
    let second = encode_frame(2, &Message::Shutdown).expect("frame");
    let mut stream = first;
    stream.extend_from_slice(&second);
    let mut cursor = Cursor::new(stream);

    assert_eq!(
        read_frame(&mut cursor).expect("first").message,
        Message::HelloAck {
            challenge: [0x6D; 32],
        }
    );
    assert_eq!(
        read_frame(&mut cursor).expect("second").message,
        Message::Shutdown
    );
}

#[test]
fn broker_protocol_malformed_001_rejects_truncated_header_and_payload() {
    let header_error = decode_frame(&[0_u8; HEADER_LEN - 1]).expect_err("truncated header");
    assert!(matches!(
        header_error,
        ProtocolError::Truncated {
            part: FramePart::Header,
            expected: HEADER_LEN,
            actual
        } if actual == HEADER_LEN - 1
    ));

    let mut payload_frame = encode_frame(1, &Message::Hello { challenge: [7; 32] }).expect("frame");
    payload_frame.truncate(payload_frame.len() - 3);
    let payload_error = decode_frame(&payload_frame).expect_err("truncated payload");
    assert!(matches!(
        payload_error,
        ProtocolError::Truncated {
            part: FramePart::Payload,
            expected: 32,
            actual: 29
        }
    ));
}

#[test]
fn broker_protocol_malformed_002_rejects_magic_version_opcode_and_sequence() {
    let baseline = encode_frame(
        1,
        &Message::HelloAck {
            challenge: [0x4C; 32],
        },
    )
    .expect("frame");

    let mut bad_magic = baseline.clone();
    bad_magic[0] ^= 0xFF;
    assert!(matches!(
        decode_frame(&bad_magic),
        Err(ProtocolError::InvalidMagic { .. })
    ));

    let mut bad_version = baseline.clone();
    bad_version[4..6].copy_from_slice(&(PROTOCOL_VERSION + 1).to_le_bytes());
    assert!(matches!(
        decode_frame(&bad_version),
        Err(ProtocolError::UnsupportedVersion { .. })
    ));

    let mut bad_opcode = baseline.clone();
    bad_opcode[6..8].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(matches!(
        decode_frame(&bad_opcode),
        Err(ProtocolError::UnknownOpcode { actual: u16::MAX })
    ));

    let mut zero_sequence = baseline;
    zero_sequence[8..16].copy_from_slice(&0_u64.to_le_bytes());
    assert!(matches!(
        decode_frame(&zero_sequence),
        Err(ProtocolError::InvalidSequence { actual: 0 })
    ));
}

#[test]
fn broker_protocol_malformed_003_rejects_oversized_payload_before_allocation() {
    let mut header = raw_frame(2, 1, &[]);
    header[16..20].copy_from_slice(
        &u32::try_from(MAX_PAYLOAD_LEN + 1)
            .expect("limit fits u32")
            .to_le_bytes(),
    );
    assert!(matches!(
        decode_frame(&header),
        Err(ProtocolError::PayloadTooLarge {
            length,
            maximum: MAX_PAYLOAD_LEN
        }) if length == MAX_PAYLOAD_LEN + 1
    ));

    let too_large = Message::ReadData {
        bytes: vec![0; MAX_READ_LEN + 1],
    };
    assert!(matches!(
        encode_frame(1, &too_large),
        Err(ProtocolError::InvalidFieldLength {
            field: "bytes",
            length,
            maximum: MAX_READ_LEN
        }) if length == MAX_READ_LEN + 1
    ));
}

#[test]
fn broker_protocol_malformed_004_rejects_invalid_utf8_and_internal_lengths() {
    let invalid_utf8 = raw_frame(3, 1, &[1, 0, 0xFF]);
    assert!(matches!(
        decode_frame(&invalid_utf8),
        Err(ProtocolError::InvalidUtf8 { field: "source_id" })
    ));

    let declared_too_long = raw_frame(3, 1, &[129, 0]);
    assert!(matches!(
        decode_frame(&declared_too_long),
        Err(ProtocolError::InvalidFieldLength {
            field: "source_id",
            length: 129,
            maximum: 128
        })
    ));

    let internal_underflow = raw_frame(3, 1, &[4, 0, b'a']);
    assert!(matches!(
        decode_frame(&internal_underflow),
        Err(ProtocolError::PayloadUnderflow {
            needed: 4,
            remaining: 1,
            ..
        })
    ));
}

#[test]
fn broker_protocol_malformed_005_rejects_trailing_payload_and_frame_bytes() {
    let unit_with_payload = raw_frame(9, 1, &[0xCC]);
    assert!(matches!(
        decode_frame(&unit_with_payload),
        Err(ProtocolError::TrailingPayload {
            opcode: 9,
            remaining: 1
        })
    ));

    let mut frame = encode_frame(1, &Message::Shutdown).expect("frame");
    frame.push(0xCC);
    assert!(matches!(
        decode_frame(&frame),
        Err(ProtocolError::TrailingFrameBytes { count: 1 })
    ));
}

#[test]
fn broker_protocol_malformed_006_rejects_unknown_stable_error_code() {
    let frame = raw_frame(10, 1, &99_u16.to_le_bytes());
    assert!(matches!(
        decode_frame(&frame),
        Err(ProtocolError::UnknownErrorCode { actual: 99 })
    ));
}

#[test]
fn broker_protocol_bounds_001_rejects_invalid_read_ranges() {
    for message in [
        Message::ReadAt {
            handle_id: 1,
            offset: 0,
            length: 0,
        },
        Message::ReadAt {
            handle_id: 1,
            offset: 0,
            length: u32::try_from(MAX_READ_LEN + 1).expect("limit fits u32"),
        },
    ] {
        assert!(matches!(
            encode_frame(1, &message),
            Err(ProtocolError::InvalidNumericValue {
                field: "length",
                ..
            })
        ));
    }

    let overflow = Message::ReadAt {
        handle_id: 1,
        offset: u64::MAX,
        length: 1,
    };
    assert!(matches!(
        encode_frame(1, &overflow),
        Err(ProtocolError::RangeOverflow)
    ));
}

#[test]
fn broker_protocol_bounds_002_rejects_invalid_handles_size_and_sector_geometry() {
    for message in [
        Message::Opened {
            handle_id: 0,
            size: 4096,
            logical_sector: 512,
            physical_sector: 4096,
        },
        Message::Opened {
            handle_id: 1,
            size: 0,
            logical_sector: 512,
            physical_sector: 4096,
        },
        Message::Opened {
            handle_id: 1,
            size: 4096,
            logical_sector: 513,
            physical_sector: 4096,
        },
        Message::Opened {
            handle_id: 1,
            size: 4096,
            logical_sector: 4096,
            physical_sector: 512,
        },
        Message::CloseSource { handle_id: 0 },
        Message::Closed { handle_id: 0 },
    ] {
        assert!(encode_frame(1, &message).is_err(), "{message:?}");
    }
}

#[test]
fn broker_protocol_session_001_assigns_independent_monotonic_sequences() {
    let mut sender = SessionCodec::new();
    let mut bytes = Vec::new();
    assert_eq!(
        sender
            .write_message(
                &mut bytes,
                &Message::HelloAck {
                    challenge: [0x31; 32],
                },
            )
            .expect("first"),
        1
    );
    assert_eq!(
        sender
            .write_message(&mut bytes, &Message::Shutdown)
            .expect("second"),
        2
    );
    assert_eq!(sender.next_outbound_sequence(), Some(3));
    assert_eq!(sender.next_inbound_sequence(), Some(1));

    let mut receiver = SessionCodec::new();
    let mut cursor = Cursor::new(bytes);
    assert_eq!(
        receiver.read_message(&mut cursor).expect("first").sequence,
        1
    );
    assert_eq!(
        receiver.read_message(&mut cursor).expect("second").sequence,
        2
    );
    assert_eq!(receiver.next_inbound_sequence(), Some(3));
    assert_eq!(receiver.next_outbound_sequence(), Some(1));
}

#[test]
fn broker_protocol_session_002_rejects_replay_and_gap_without_advancing() {
    let first = encode_frame(
        1,
        &Message::HelloAck {
            challenge: [0x2B; 32],
        },
    )
    .expect("frame");
    let replay = first.clone();
    let gap = encode_frame(3, &Message::Shutdown).expect("frame");
    let second = encode_frame(2, &Message::Shutdown).expect("frame");
    let mut codec = SessionCodec::new();

    codec
        .read_message(&mut Cursor::new(first))
        .expect("sequence one");
    assert!(matches!(
        codec.read_message(&mut Cursor::new(replay)),
        Err(ProtocolError::Replay {
            sequence: 1,
            last_accepted: 1
        })
    ));
    assert_eq!(codec.next_inbound_sequence(), Some(2));
    assert!(matches!(
        codec.read_message(&mut Cursor::new(gap)),
        Err(ProtocolError::OutOfSequence {
            expected: 2,
            actual: 3
        })
    ));
    assert_eq!(codec.next_inbound_sequence(), Some(2));
    codec
        .read_message(&mut Cursor::new(second))
        .expect("sequence two remains acceptable");
}

#[test]
fn broker_protocol_session_003_malformed_frame_does_not_advance_sequence() {
    let mut codec = SessionCodec::new();
    let mut malformed = encode_frame(
        1,
        &Message::HelloAck {
            challenge: [0x17; 32],
        },
    )
    .expect("frame");
    malformed[4..6].copy_from_slice(&(PROTOCOL_VERSION + 1).to_le_bytes());
    assert!(codec.read_message(&mut Cursor::new(malformed)).is_err());
    assert_eq!(codec.next_inbound_sequence(), Some(1));

    let valid = encode_frame(
        1,
        &Message::HelloAck {
            challenge: [0x17; 32],
        },
    )
    .expect("frame");
    codec
        .read_message(&mut Cursor::new(valid))
        .expect("same sequence is valid after malformed frame");
}

#[test]
fn broker_protocol_session_004_sequence_exhaustion_fails_closed() {
    let mut sender = SessionCodec::with_sequences(u64::MAX, 1).expect("valid state");
    let mut output = Vec::new();
    assert_eq!(
        sender
            .write_message(&mut output, &Message::Shutdown)
            .expect("last sequence"),
        u64::MAX
    );
    assert!(matches!(
        sender.write_message(&mut output, &Message::Shutdown),
        Err(ProtocolError::SequenceExhausted { .. })
    ));

    let final_frame = encode_frame(u64::MAX, &Message::Shutdown).expect("frame");
    let mut receiver = SessionCodec::with_sequences(1, u64::MAX).expect("valid state");
    receiver
        .read_message(&mut Cursor::new(final_frame))
        .expect("last sequence");
    assert!(matches!(
        receiver.read_message(&mut Cursor::new(Vec::<u8>::new())),
        Err(ProtocolError::SequenceExhausted { .. })
    ));
}

struct AlwaysFailWriter;

impl Write for AlwaysFailWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "synthetic"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn broker_protocol_session_005_transport_failure_does_not_advance_sequence() {
    let mut codec = SessionCodec::new();
    assert!(matches!(
        codec.write_message(&mut AlwaysFailWriter, &Message::Shutdown),
        Err(ProtocolError::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe
    ));
    assert_eq!(codec.next_outbound_sequence(), Some(1));
}
