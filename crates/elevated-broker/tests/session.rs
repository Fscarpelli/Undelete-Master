use std::io::{self, Cursor, Read, Write};
use std::sync::{Arc, Mutex};

use um_broker_protocol::{BrokerErrorCode, Message, SessionCodec};
use um_elevated_broker::{
    serve_session, BrokerSourceGeometry, ReadOnlyBrokerSource, ReadOnlySourceFactory, SessionError,
};

struct MemoryTransport {
    inbound: Cursor<Vec<u8>>,
    outbound: Vec<u8>,
}

impl MemoryTransport {
    fn new(inbound: Vec<u8>) -> Self {
        Self {
            inbound: Cursor::new(inbound),
            outbound: Vec::new(),
        }
    }
}

impl Read for MemoryTransport {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inbound.read(buffer)
    }
}

impl Write for MemoryTransport {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.outbound.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct SourceState {
    reads: Vec<(u64, usize)>,
    revalidations: usize,
    drops: usize,
}

struct ScriptedSource {
    bytes: Vec<u8>,
    state: Arc<Mutex<SourceState>>,
    revalidation_error: Option<BrokerErrorCode>,
}

impl Drop for ScriptedSource {
    fn drop(&mut self) {
        self.state.lock().expect("source state").drops += 1;
    }
}

impl ReadOnlyBrokerSource for ScriptedSource {
    fn geometry(&self) -> BrokerSourceGeometry {
        BrokerSourceGeometry {
            size: self.bytes.len() as u64,
            logical_sector: 512,
            physical_sector: 4096,
            physical_disk_number: 7,
        }
    }

    fn revalidate(&mut self) -> Result<(), BrokerErrorCode> {
        let mut state = self.state.lock().expect("source state");
        state.revalidations += 1;
        self.revalidation_error.map_or(Ok(()), Err)
    }

    fn read_exact_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<(), BrokerErrorCode> {
        self.state
            .lock()
            .expect("source state")
            .reads
            .push((offset, buffer.len()));
        let start = usize::try_from(offset).map_err(|_| BrokerErrorCode::ReadOutOfRange)?;
        let end = start
            .checked_add(buffer.len())
            .ok_or(BrokerErrorCode::ReadOutOfRange)?;
        buffer.copy_from_slice(
            self.bytes
                .get(start..end)
                .ok_or(BrokerErrorCode::ReadOutOfRange)?,
        );
        Ok(())
    }
}

struct ScriptedFactory {
    bytes: Vec<u8>,
    state: Arc<Mutex<SourceState>>,
    opened: Arc<Mutex<Vec<String>>>,
    revalidation_error: Option<BrokerErrorCode>,
}

impl ReadOnlySourceFactory for ScriptedFactory {
    type Source = ScriptedSource;

    fn open_source(&mut self, source_id: &str) -> Result<Self::Source, BrokerErrorCode> {
        self.opened
            .lock()
            .expect("opened sources")
            .push(source_id.to_owned());
        Ok(ScriptedSource {
            bytes: self.bytes.clone(),
            state: Arc::clone(&self.state),
            revalidation_error: self.revalidation_error,
        })
    }
}

fn encode_client_messages(messages: &[Message]) -> Vec<u8> {
    let mut codec = SessionCodec::new();
    let mut bytes = Vec::new();
    for message in messages {
        codec
            .write_message(&mut bytes, message)
            .expect("encode client message");
    }
    bytes
}

fn decode_server_messages(bytes: &[u8]) -> Vec<Message> {
    let mut codec = SessionCodec::new();
    let mut cursor = Cursor::new(bytes);
    let mut messages = Vec::new();
    while (cursor.position() as usize) < bytes.len() {
        messages.push(
            codec
                .read_message(&mut cursor)
                .expect("decode server message")
                .message,
        );
    }
    messages
}

fn factory(bytes: Vec<u8>) -> (ScriptedFactory, Arc<Mutex<SourceState>>) {
    let state = Arc::new(Mutex::new(SourceState::default()));
    (
        ScriptedFactory {
            bytes,
            state: Arc::clone(&state),
            opened: Arc::new(Mutex::new(Vec::new())),
            revalidation_error: None,
        },
        state,
    )
}

#[test]
fn elevated_broker_session_001_handshakes_opens_reads_closes_and_shuts_down() {
    let source_bytes = (0_u8..=255).cycle().take(8192).collect::<Vec<_>>();
    let input = encode_client_messages(&[
        Message::Hello {
            challenge: [0xA7; 32],
        },
        Message::OpenSource {
            source_id: "inventory-source-17".to_owned(),
        },
        Message::ReadAt {
            handle_id: 71,
            offset: 4093,
            length: 19,
        },
        Message::CloseSource { handle_id: 71 },
        Message::Shutdown,
    ]);
    let mut transport = MemoryTransport::new(input);
    let (mut factory, state) = factory(source_bytes.clone());

    serve_session(&mut transport, &mut factory, 71).expect("broker session");

    let messages = decode_server_messages(&transport.outbound);
    assert_eq!(
        messages,
        [
            Message::HelloAck {
                challenge: [0xA7; 32],
            },
            Message::Opened {
                handle_id: 71,
                size: 8192,
                logical_sector: 512,
                physical_sector: 4096,
                physical_disk_number: 7,
            },
            Message::ReadData {
                bytes: source_bytes[4093..4112].to_vec(),
            },
            Message::Closed { handle_id: 71 },
        ]
    );
    let state = state.lock().expect("source state");
    assert_eq!(state.revalidations, 1);
    assert_eq!(state.reads, [(4093, 19)]);
    assert_eq!(state.drops, 1);
    assert_eq!(
        *factory.opened.lock().expect("opened sources"),
        ["inventory-source-17"]
    );
}

#[test]
fn elevated_broker_session_002_rejects_out_of_range_before_source_read() {
    let input = encode_client_messages(&[
        Message::Hello {
            challenge: [0xA7; 32],
        },
        Message::OpenSource {
            source_id: "inventory-source-17".to_owned(),
        },
        Message::ReadAt {
            handle_id: 71,
            offset: 4095,
            length: 2,
        },
        Message::Shutdown,
    ]);
    let mut transport = MemoryTransport::new(input);
    let (mut factory, state) = factory(vec![0x5A; 4096]);

    serve_session(&mut transport, &mut factory, 71).expect("broker session");

    assert_eq!(
        decode_server_messages(&transport.outbound),
        [
            Message::HelloAck {
                challenge: [0xA7; 32],
            },
            Message::Opened {
                handle_id: 71,
                size: 4096,
                logical_sector: 512,
                physical_sector: 4096,
                physical_disk_number: 7,
            },
            Message::Error {
                code: BrokerErrorCode::ReadOutOfRange,
            },
        ]
    );
    assert!(state.lock().expect("source state").reads.is_empty());
}

#[test]
fn elevated_broker_session_003_identity_change_closes_the_handle() {
    let input = encode_client_messages(&[
        Message::Hello {
            challenge: [0xA7; 32],
        },
        Message::OpenSource {
            source_id: "inventory-source-17".to_owned(),
        },
        Message::ReadAt {
            handle_id: 71,
            offset: 0,
            length: 8,
        },
        Message::CloseSource { handle_id: 71 },
        Message::Shutdown,
    ]);
    let mut transport = MemoryTransport::new(input);
    let (mut factory, state) = factory(vec![0x5A; 4096]);
    factory.revalidation_error = Some(BrokerErrorCode::SourceChanged);

    serve_session(&mut transport, &mut factory, 71).expect("broker session");

    assert_eq!(
        decode_server_messages(&transport.outbound),
        [
            Message::HelloAck {
                challenge: [0xA7; 32],
            },
            Message::Opened {
                handle_id: 71,
                size: 4096,
                logical_sector: 512,
                physical_sector: 4096,
                physical_disk_number: 7,
            },
            Message::Error {
                code: BrokerErrorCode::SourceChanged,
            },
            Message::Error {
                code: BrokerErrorCode::InvalidHandle,
            },
        ]
    );
    let state = state.lock().expect("source state");
    assert!(state.reads.is_empty());
    assert_eq!(state.drops, 1);
}

#[test]
fn elevated_broker_session_004_requires_hello_before_any_source_command() {
    let input = encode_client_messages(&[Message::OpenSource {
        source_id: "inventory-source-17".to_owned(),
    }]);
    let mut transport = MemoryTransport::new(input);
    let (mut factory, _) = factory(vec![0; 4096]);

    assert_eq!(
        serve_session(&mut transport, &mut factory, 71),
        Err(SessionError::AuthenticationFailed)
    );
    assert_eq!(
        decode_server_messages(&transport.outbound),
        [Message::Error {
            code: BrokerErrorCode::AuthenticationFailed,
        }]
    );
}

#[test]
fn elevated_broker_session_005_allows_only_one_open_source() {
    let input = encode_client_messages(&[
        Message::Hello {
            challenge: [0xA7; 32],
        },
        Message::OpenSource {
            source_id: "inventory-source-17".to_owned(),
        },
        Message::OpenSource {
            source_id: "inventory-source-18".to_owned(),
        },
        Message::Shutdown,
    ]);
    let mut transport = MemoryTransport::new(input);
    let (mut factory, _) = factory(vec![0; 4096]);

    serve_session(&mut transport, &mut factory, 71).expect("broker session");
    assert_eq!(
        decode_server_messages(&transport.outbound),
        [
            Message::HelloAck {
                challenge: [0xA7; 32],
            },
            Message::Opened {
                handle_id: 71,
                size: 4096,
                logical_sector: 512,
                physical_sector: 4096,
                physical_disk_number: 7,
            },
            Message::Error {
                code: BrokerErrorCode::InvalidRequest,
            },
        ]
    );
}

#[test]
fn elevated_broker_session_006_rejects_an_empty_challenge() {
    let input = encode_client_messages(&[Message::Hello { challenge: [0; 32] }]);
    let mut transport = MemoryTransport::new(input);
    let (mut factory, _) = factory(vec![0; 4096]);

    assert_eq!(
        serve_session(&mut transport, &mut factory, 71),
        Err(SessionError::AuthenticationFailed)
    );
    assert_eq!(
        decode_server_messages(&transport.outbound),
        [Message::Error {
            code: BrokerErrorCode::AuthenticationFailed,
        }]
    );
}
