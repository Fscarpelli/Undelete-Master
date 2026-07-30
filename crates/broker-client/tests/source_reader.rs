use std::sync::{Arc, Mutex};

use um_broker_client::{BrokerClientError, BrokerSourceReader, OpenedSource, ReadSession};
use um_broker_protocol::MAX_READ_LEN;
use um_core::{ReadError, SourceReader};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Open(String),
    Read {
        handle_id: u64,
        offset: u64,
        length: u32,
    },
    Close(u64),
    Shutdown,
}

struct ScriptedSession {
    bytes: Vec<u8>,
    calls: Arc<Mutex<Vec<Call>>>,
    fail_at: Option<u64>,
    open_error: Option<BrokerClientError>,
    panic_at: Option<u64>,
}

impl ScriptedSession {
    fn new(bytes: Vec<u8>, calls: Arc<Mutex<Vec<Call>>>) -> Self {
        Self {
            bytes,
            calls,
            fail_at: None,
            open_error: None,
            panic_at: None,
        }
    }

    fn with_failure(mut self, offset: u64) -> Self {
        self.fail_at = Some(offset);
        self
    }

    fn with_open_failure(mut self, error: BrokerClientError) -> Self {
        self.open_error = Some(error);
        self
    }

    fn with_read_panic(mut self, offset: u64) -> Self {
        self.panic_at = Some(offset);
        self
    }
}

impl ReadSession for ScriptedSession {
    fn open_source(&mut self, source_id: &str) -> Result<OpenedSource, BrokerClientError> {
        self.calls
            .lock()
            .expect("call log")
            .push(Call::Open(source_id.to_owned()));
        if let Some(error) = self.open_error {
            return Err(error);
        }
        Ok(OpenedSource {
            handle_id: 41,
            size: self.bytes.len() as u64,
            logical_sector: 512,
            physical_sector: 4096,
        })
    }

    fn read_at(
        &mut self,
        handle_id: u64,
        offset: u64,
        length: u32,
    ) -> Result<Vec<u8>, BrokerClientError> {
        self.calls.lock().expect("call log").push(Call::Read {
            handle_id,
            offset,
            length,
        });
        assert_ne!(self.panic_at, Some(offset), "scripted session panic");
        if self.fail_at == Some(offset) {
            return Err(BrokerClientError::ReadFailed);
        }
        let start = usize::try_from(offset).expect("fixture offset");
        let end = start
            .checked_add(usize::try_from(length).expect("fixture length"))
            .expect("fixture range");
        Ok(self.bytes[start..end].to_vec())
    }

    fn close_source(&mut self, handle_id: u64) -> Result<(), BrokerClientError> {
        self.calls
            .lock()
            .expect("call log")
            .push(Call::Close(handle_id));
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), BrokerClientError> {
        self.calls.lock().expect("call log").push(Call::Shutdown);
        Ok(())
    }
}

#[test]
fn broker_client_reader_001_returns_exact_source_bytes() {
    let source = (0_u8..=251).cycle().take(32 * 1024).collect::<Vec<_>>();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = BrokerSourceReader::open(
        ScriptedSession::new(source.clone(), Arc::clone(&calls)),
        "inventory-source-17",
    )
    .expect("open scripted broker source");

    let mut actual = vec![0_u8; 4097];
    reader
        .read_exact_at(777, &mut actual)
        .expect("read exact source range");

    assert_eq!(actual, source[777..777 + 4097]);
    assert_eq!(reader.len(), source.len() as u64);
    assert_eq!(reader.sector_layout().logical, 512);
    assert_eq!(reader.sector_layout().physical, 4096);
    assert_eq!(reader.identity().id, "inventory-source-17");
}

#[test]
fn broker_client_reader_002_chunks_large_reads_at_protocol_limit() {
    let total = MAX_READ_LEN
        .checked_mul(2)
        .and_then(|value| value.checked_add(17))
        .expect("test size");
    let source = (0_u8..=255).cycle().take(total).collect::<Vec<_>>();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = BrokerSourceReader::open(
        ScriptedSession::new(source.clone(), Arc::clone(&calls)),
        "inventory-source-17",
    )
    .expect("open scripted broker source");

    let mut actual = vec![0_u8; total];
    reader
        .read_exact_at(0, &mut actual)
        .expect("chunked broker read");

    assert_eq!(actual, source);
    let reads = calls
        .lock()
        .expect("call log")
        .iter()
        .filter_map(|call| match call {
            Call::Read { offset, length, .. } => Some((*offset, *length)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        reads,
        [
            (0, MAX_READ_LEN as u32),
            (MAX_READ_LEN as u64, MAX_READ_LEN as u32),
            ((MAX_READ_LEN * 2) as u64, 17),
        ]
    );
}

#[test]
fn broker_client_reader_003_rejects_out_of_range_without_transport_read() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = BrokerSourceReader::open(
        ScriptedSession::new(vec![0xA5; 4096], Arc::clone(&calls)),
        "inventory-source-17",
    )
    .expect("open scripted broker source");

    let mut bytes = [0_u8; 2];
    assert_eq!(
        reader.read_exact_at(4095, &mut bytes),
        Err(ReadError::OutOfBounds {
            offset: 4095,
            len: 2,
            source_len: 4096,
        })
    );
    assert_eq!(
        reader.read_exact_at(u64::MAX, &mut bytes),
        Err(ReadError::OutOfBounds {
            offset: u64::MAX,
            len: 2,
            source_len: 4096,
        })
    );
    assert!(!calls
        .lock()
        .expect("call log")
        .iter()
        .any(|call| matches!(call, Call::Read { .. })));
}

#[test]
fn broker_client_reader_004_maps_failures_to_stable_sanitized_errors() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = BrokerSourceReader::open(
        ScriptedSession::new(vec![0x5A; 4096], calls).with_failure(0),
        "inventory-source-17",
    )
    .expect("open scripted broker source");
    let mut bytes = [0xCC; 32];

    let error = reader
        .read_exact_at(0, &mut bytes)
        .expect_err("scripted read failure");
    assert_eq!(
        error,
        ReadError::Io {
            offset: 0,
            message: "broker source read failed".to_owned(),
        }
    );
    assert_eq!(bytes, [0_u8; 32]);
    assert_eq!(
        BrokerClientError::TransportFailure.to_string(),
        "broker transport unavailable"
    );
}

#[test]
fn broker_client_reader_005_drop_closes_source_then_shuts_down_session() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    {
        let _reader = BrokerSourceReader::open(
            ScriptedSession::new(vec![0; 4096], Arc::clone(&calls)),
            "inventory-source-17",
        )
        .expect("open scripted broker source");
    }

    assert_eq!(
        *calls.lock().expect("call log"),
        [
            Call::Open("inventory-source-17".to_owned()),
            Call::Close(41),
            Call::Shutdown,
        ]
    );
}

#[test]
fn broker_client_reader_006_failed_open_still_shuts_down_session() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let error = match BrokerSourceReader::open(
        ScriptedSession::new(vec![0; 4096], Arc::clone(&calls))
            .with_open_failure(BrokerClientError::SourceIdentityMismatch),
        "inventory-source-17",
    ) {
        Ok(_) => panic!("identity mismatch must reject open"),
        Err(error) => error,
    };

    assert_eq!(error, BrokerClientError::SourceIdentityMismatch);
    assert_eq!(
        *calls.lock().expect("call log"),
        [Call::Open("inventory-source-17".to_owned()), Call::Shutdown,]
    );
}

#[test]
fn broker_client_reader_007_poisoned_session_still_closes_on_drop() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let reader = BrokerSourceReader::open(
        ScriptedSession::new(vec![0; 4096], Arc::clone(&calls)).with_read_panic(0),
        "inventory-source-17",
    )
    .expect("open scripted broker source");
    let mut bytes = [0_u8; 8];

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = reader.read_exact_at(0, &mut bytes);
    }));
    assert!(panic.is_err());
    drop(reader);

    assert_eq!(
        *calls.lock().expect("call log"),
        [
            Call::Open("inventory-source-17".to_owned()),
            Call::Read {
                handle_id: 41,
                offset: 0,
                length: 8,
            },
            Call::Close(41),
            Call::Shutdown,
        ]
    );
}

#[test]
fn broker_client_arch_001_read_session_exposes_no_mutating_source_api() {
    let crate_source = include_str!("../src/lib.rs").to_ascii_lowercase();
    let trait_start = crate_source
        .find("pub trait readsession")
        .expect("ReadSession trait");
    let trait_tail = &crate_source[trait_start..];
    let trait_end = trait_tail.find("\n}").expect("ReadSession trait end");
    let trait_source = &trait_tail[..trait_end];
    let method_names = trait_source
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("fn ")
                .and_then(|signature| signature.split('(').next())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        method_names,
        ["open_source", "read_at", "close_source", "shutdown"]
    );

    for forbidden in [
        "fn write",
        "fn trim",
        "fn format",
        "fn delete",
        "fn lock",
        "fn dismount",
        "fn ioctl",
        "fn device_control",
        "destination",
        "access_mask",
    ] {
        assert!(
            !trait_source.contains(forbidden),
            "ReadSession contains forbidden capability {forbidden}"
        );
    }
}

#[test]
fn broker_client_arch_002_windows_entry_accepts_only_opaque_volume_id() {
    let _entry: fn(&str) -> Result<um_broker_client::WindowsBrokerSourceReader, BrokerClientError> =
        um_broker_client::open_windows_source;
    let crate_source = include_str!("../src/lib.rs").to_ascii_lowercase();
    assert!(!crate_source.contains("std::path"));
    assert!(!crate_source.contains("device_path"));
    assert!(!crate_source.contains("volume_path"));
}
