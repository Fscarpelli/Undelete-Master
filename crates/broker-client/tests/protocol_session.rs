use std::io::{self, Cursor, Read, Write};
use std::sync::{Arc, Mutex};

use um_broker_client::{BrokerClientError, ProtocolReadSession, ReadSession};
use um_broker_protocol::{Message, SessionCodec};

#[derive(Debug)]
struct SharedTransport {
    inbound: Cursor<Vec<u8>>,
    outbound: Arc<Mutex<Vec<u8>>>,
}

impl SharedTransport {
    fn new(inbound: Vec<u8>, outbound: Arc<Mutex<Vec<u8>>>) -> Self {
        Self {
            inbound: Cursor::new(inbound),
            outbound,
        }
    }
}

impl Read for SharedTransport {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inbound.read(buffer)
    }
}

impl Write for SharedTransport {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.outbound
            .lock()
            .expect("outbound bytes")
            .extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct FailingFlushTransport {
    inbound: Cursor<Vec<u8>>,
    outbound: Arc<Mutex<Vec<u8>>>,
    flush_count: usize,
    fail_on_flush: usize,
}

impl Read for FailingFlushTransport {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inbound.read(buffer)
    }
}

impl Write for FailingFlushTransport {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.outbound
            .lock()
            .expect("outbound bytes")
            .extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_count += 1;
        if self.flush_count == self.fail_on_flush {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "synthetic flush failure",
            ));
        }
        Ok(())
    }
}

#[test]
fn broker_client_protocol_001_challenges_then_uses_contiguous_sequences() {
    let challenge = [0xC7; 32];
    let mut server_codec = SessionCodec::new();
    let mut inbound = Vec::new();
    server_codec
        .write_message(&mut inbound, &Message::HelloAck { challenge })
        .expect("hello response");
    server_codec
        .write_message(
            &mut inbound,
            &Message::Opened {
                handle_id: 73,
                size: 8192,
                logical_sector: 512,
                physical_sector: 4096,
            },
        )
        .expect("open response");
    server_codec
        .write_message(
            &mut inbound,
            &Message::ReadData {
                bytes: vec![0xAB; 16],
            },
        )
        .expect("read response");
    server_codec
        .write_message(&mut inbound, &Message::Closed { handle_id: 73 })
        .expect("close response");

    let outbound = Arc::new(Mutex::new(Vec::new()));
    let mut session = ProtocolReadSession::connect(
        SharedTransport::new(inbound, Arc::clone(&outbound)),
        challenge,
    )
    .expect("authenticated protocol session");
    let opened = session
        .open_source("inventory-source-17")
        .expect("open source");
    assert_eq!(opened.handle_id, 73);
    assert_eq!(
        session.read_at(opened.handle_id, 4096, 16).expect("read"),
        vec![0xAB; 16]
    );
    session
        .close_source(opened.handle_id)
        .expect("close source");
    session.shutdown().expect("shutdown");

    let written = outbound.lock().expect("outbound bytes").clone();
    let mut cursor = Cursor::new(written);
    let mut client_codec = SessionCodec::new();
    assert_eq!(
        client_codec
            .read_message(&mut cursor)
            .expect("challenge frame")
            .message,
        Message::Hello { challenge }
    );
    assert_eq!(
        client_codec
            .read_message(&mut cursor)
            .expect("open frame")
            .message,
        Message::OpenSource {
            source_id: "inventory-source-17".to_owned(),
        }
    );
    assert_eq!(
        client_codec
            .read_message(&mut cursor)
            .expect("read frame")
            .message,
        Message::ReadAt {
            handle_id: 73,
            offset: 4096,
            length: 16,
        }
    );
    assert_eq!(
        client_codec
            .read_message(&mut cursor)
            .expect("close frame")
            .message,
        Message::CloseSource { handle_id: 73 }
    );
    assert_eq!(
        client_codec
            .read_message(&mut cursor)
            .expect("shutdown frame")
            .message,
        Message::Shutdown
    );
}

#[test]
fn broker_client_protocol_002_rejects_a_non_ack_handshake() {
    let mut server_codec = SessionCodec::new();
    let mut inbound = Vec::new();
    server_codec
        .write_message(
            &mut inbound,
            &Message::Error {
                code: um_broker_protocol::BrokerErrorCode::AuthenticationFailed,
            },
        )
        .expect("authentication failure");
    let outbound = Arc::new(Mutex::new(Vec::new()));

    let error = ProtocolReadSession::connect(SharedTransport::new(inbound, outbound), [0x31; 32])
        .expect_err("authentication must fail closed");
    assert_eq!(error.to_string(), "broker authentication failed");
}

#[test]
fn broker_client_protocol_003_rejects_an_empty_challenge() {
    let outbound = Arc::new(Mutex::new(Vec::new()));
    let error = match ProtocolReadSession::connect(
        SharedTransport::new(Vec::new(), Arc::clone(&outbound)),
        [0_u8; 32],
    ) {
        Ok(_) => panic!("empty challenge must fail closed"),
        Err(error) => error,
    };

    assert_eq!(error.to_string(), "broker authentication failed");
    assert!(outbound.lock().expect("outbound bytes").is_empty());
}

#[test]
fn broker_client_protocol_004_rejects_a_mismatched_challenge_echo() {
    let challenge = [0x44; 32];
    let mut server_codec = SessionCodec::new();
    let mut inbound = Vec::new();
    server_codec
        .write_message(
            &mut inbound,
            &Message::HelloAck {
                challenge: [0x45; 32],
            },
        )
        .expect("mismatched challenge response");

    let error = match ProtocolReadSession::connect(
        SharedTransport::new(inbound, Arc::new(Mutex::new(Vec::new()))),
        challenge,
    ) {
        Ok(_) => panic!("mismatched challenge must fail closed"),
        Err(error) => error,
    };
    assert_eq!(error.to_string(), "broker authentication failed");
}

#[test]
fn broker_client_protocol_005_rejects_an_out_of_sequence_challenge_echo() {
    let challenge = [0x72; 32];
    let inbound = um_broker_protocol::encode_frame(2, &Message::HelloAck { challenge })
        .expect("out-of-sequence response");

    let error = match ProtocolReadSession::connect(
        SharedTransport::new(inbound, Arc::new(Mutex::new(Vec::new()))),
        challenge,
    ) {
        Ok(_) => panic!("out-of-sequence challenge must fail closed"),
        Err(error) => error,
    };
    assert_eq!(error.to_string(), "broker protocol failure");
}

#[test]
fn broker_client_protocol_006_flush_failure_permanently_invalidates_session() {
    let challenge = [0x63; 32];
    let mut server_codec = SessionCodec::new();
    let mut inbound = Vec::new();
    server_codec
        .write_message(&mut inbound, &Message::HelloAck { challenge })
        .expect("challenge response");
    server_codec
        .write_message(
            &mut inbound,
            &Message::ReadData {
                bytes: vec![0xA1; 8],
            },
        )
        .expect("stale read response");
    let outbound = Arc::new(Mutex::new(Vec::new()));
    let transport = FailingFlushTransport {
        inbound: Cursor::new(inbound),
        outbound: Arc::clone(&outbound),
        flush_count: 0,
        fail_on_flush: 2,
    };
    let mut session = ProtocolReadSession::connect(transport, challenge).expect("handshake");

    assert_eq!(
        session.read_at(71, 0, 8),
        Err(BrokerClientError::TransportFailure)
    );
    let bytes_after_failure = outbound.lock().expect("outbound bytes").len();
    assert_eq!(
        session.read_at(71, 4096, 8),
        Err(BrokerClientError::SessionUnavailable)
    );
    assert_eq!(
        outbound.lock().expect("outbound bytes").len(),
        bytes_after_failure,
        "invalidated session must not send another request"
    );
}
