use crate::RestoreError;
use cap_std::fs::File;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;

pub(crate) const JOURNAL_VERSION: u32 = 1;
pub(crate) const JOURNAL_NAME: &str = "recovery-journal.jsonl";
const MAX_JOURNAL_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_JOURNAL_RECORD_BYTES: usize = MAX_JOURNAL_PAYLOAD_BYTES + 4096;
const MAX_JOURNAL_RECORDS: u64 = 1_000_000;
const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const HASH_DOMAIN: &[u8] = b"undelete-master-journal-record-v1\0";

pub(crate) trait DurableJournalSink: Write {
    fn sync_all(&self) -> std::io::Result<()>;
}

impl DurableJournalSink for File {
    fn sync_all(&self) -> std::io::Result<()> {
        File::sync_all(self)
    }
}

pub(crate) struct JobJournal {
    sink: Box<dyn DurableJournalSink>,
    job_id_sha256: [u8; 32],
    next_sequence: u64,
    previous_hash: String,
    poisoned: bool,
}

impl JobJournal {
    pub(crate) fn new(file: File, job_id: &str) -> Self {
        Self::with_sink(Box::new(file), Sha256::digest(job_id.as_bytes()).into())
    }

    pub(crate) fn with_sink(sink: Box<dyn DurableJournalSink>, job_id_sha256: [u8; 32]) -> Self {
        Self {
            sink,
            job_id_sha256,
            next_sequence: 0,
            previous_hash: ZERO_HASH.to_owned(),
            poisoned: false,
        }
    }

    pub(crate) fn append<T: Serialize>(
        &mut self,
        kind: &'static str,
        payload: &T,
    ) -> Result<(), RestoreError> {
        if self.poisoned {
            return Err(RestoreError::JournalPoisoned);
        }
        if self.next_sequence >= MAX_JOURNAL_RECORDS {
            return Err(RestoreError::JournalRecordLimit {
                maximum: MAX_JOURNAL_RECORDS,
            });
        }
        let payload =
            serde_json::to_value(payload).map_err(|error| RestoreError::JournalSerialization {
                message: error.to_string(),
            })?;
        let payload_len = serde_json::to_vec(&payload)
            .map_err(|error| RestoreError::JournalSerialization {
                message: error.to_string(),
            })?
            .len();
        if payload_len > MAX_JOURNAL_PAYLOAD_BYTES {
            return Err(RestoreError::JournalPayloadTooLarge {
                actual: payload_len,
                maximum: MAX_JOURNAL_PAYLOAD_BYTES,
            });
        }

        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|error| RestoreError::JournalSerialization {
                message: error.to_string(),
            })?;
        let record_hash = record_hash(
            &self.job_id_sha256,
            self.next_sequence,
            &self.previous_hash,
            kind,
            &payload_bytes,
        )?;
        let record = JournalRecord {
            version: JOURNAL_VERSION,
            job_id_sha256: hex_lower(&self.job_id_sha256),
            sequence: self.next_sequence,
            previous_hash: self.previous_hash.clone(),
            kind: kind.to_owned(),
            payload_length: payload_bytes.len(),
            payload,
            record_hash: record_hash.clone(),
        };
        let mut bytes =
            serde_json::to_vec(&record).map_err(|error| RestoreError::JournalSerialization {
                message: error.to_string(),
            })?;
        bytes.push(b'\n');
        if bytes.len() > MAX_JOURNAL_RECORD_BYTES {
            return Err(RestoreError::JournalRecordTooLarge {
                actual: bytes.len(),
                maximum: MAX_JOURNAL_RECORD_BYTES,
            });
        }

        if let Err(error) = self.sink.write_all(&bytes) {
            self.poisoned = true;
            return Err(journal_io("write", error));
        }
        if let Err(error) = self.sink.flush() {
            self.poisoned = true;
            return Err(journal_io("flush", error));
        }
        if let Err(error) = self.sink.sync_all() {
            self.poisoned = true;
            return Err(journal_io("sync", error));
        }

        self.next_sequence =
            self.next_sequence
                .checked_add(1)
                .ok_or(RestoreError::JournalSequenceOverflow {
                    sequence: self.next_sequence,
                })?;
        self.previous_hash = record_hash;
        Ok(())
    }

    pub(crate) fn head_hash(&self) -> &str {
        &self.previous_hash
    }

    pub(crate) fn is_poisoned(&self) -> bool {
        self.poisoned
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JournalRecord {
    version: u32,
    job_id_sha256: String,
    sequence: u64,
    previous_hash: String,
    kind: String,
    payload_length: usize,
    payload: Value,
    record_hash: String,
}

#[derive(Debug, Clone)]
pub struct JournalAudit {
    records: Vec<JournalRecord>,
}

impl JournalAudit {
    pub fn is_hash_chain_valid(&self) -> bool {
        true
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn kinds(&self) -> Vec<&str> {
        self.records
            .iter()
            .map(|record| record.kind.as_str())
            .collect()
    }

    pub fn sequences(&self) -> Vec<u64> {
        self.records.iter().map(|record| record.sequence).collect()
    }
}

pub fn audit_journal(bytes: &[u8]) -> Result<JournalAudit, RestoreError> {
    audit_journal_with_record_limit(bytes, MAX_JOURNAL_RECORDS)
}

fn audit_journal_with_record_limit(
    bytes: &[u8],
    maximum_records: u64,
) -> Result<JournalAudit, RestoreError> {
    let text = std::str::from_utf8(bytes).map_err(|error| RestoreError::JournalAudit {
        message: error.to_string(),
    })?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(RestoreError::JournalAudit {
            message: "journal does not end at a complete JSONL record".into(),
        });
    }

    let mut records = Vec::new();
    let mut expected_sequence = 0u64;
    let mut expected_previous = ZERO_HASH.to_owned();
    let mut expected_job_id_sha256 = None;
    for terminated_line in bytes.split_inclusive(|byte| *byte == b'\n') {
        if expected_sequence >= maximum_records {
            return Err(RestoreError::JournalRecordLimit {
                maximum: maximum_records,
            });
        }
        let record_len = terminated_line.len();
        if record_len > MAX_JOURNAL_RECORD_BYTES {
            return Err(RestoreError::JournalRecordTooLarge {
                actual: record_len,
                maximum: MAX_JOURNAL_RECORD_BYTES,
            });
        }
        let line =
            terminated_line
                .strip_suffix(b"\n")
                .ok_or_else(|| RestoreError::JournalAudit {
                    message: "journal does not end at a complete JSONL record".into(),
                })?;
        if line.is_empty() {
            return Err(RestoreError::JournalAudit {
                message: "journal contains an empty JSONL record".into(),
            });
        }
        let record: JournalRecord =
            serde_json::from_slice(line).map_err(|error| RestoreError::JournalAudit {
                message: error.to_string(),
            })?;
        let canonical =
            serde_json::to_vec(&record).map_err(|error| RestoreError::JournalAudit {
                message: error.to_string(),
            })?;
        if canonical != line {
            return Err(RestoreError::JournalAudit {
                message: "journal record bytes are not canonical".into(),
            });
        }
        if record.version != JOURNAL_VERSION
            || record.sequence != expected_sequence
            || record.previous_hash != expected_previous
        {
            return Err(RestoreError::JournalAudit {
                message: "journal version, sequence, or previous hash is invalid".into(),
            });
        }
        let payload_bytes =
            serde_json::to_vec(&record.payload).map_err(|error| RestoreError::JournalAudit {
                message: error.to_string(),
            })?;
        if payload_bytes.len() > MAX_JOURNAL_PAYLOAD_BYTES {
            return Err(RestoreError::JournalPayloadTooLarge {
                actual: payload_bytes.len(),
                maximum: MAX_JOURNAL_PAYLOAD_BYTES,
            });
        }
        if payload_bytes.len() != record.payload_length {
            return Err(RestoreError::JournalAudit {
                message: "journal payload length is invalid".into(),
            });
        }
        let job_id_sha256 = decode_hash(&record.job_id_sha256)?;
        match expected_job_id_sha256 {
            Some(expected) if expected != job_id_sha256 => {
                return Err(RestoreError::JournalAudit {
                    message: "journal job identity changed inside one hash chain".into(),
                });
            }
            None => expected_job_id_sha256 = Some(job_id_sha256),
            Some(_) => {}
        }
        let actual_hash = record_hash(
            &job_id_sha256,
            record.sequence,
            &record.previous_hash,
            &record.kind,
            &payload_bytes,
        )?;
        if actual_hash != record.record_hash {
            return Err(RestoreError::JournalAudit {
                message: "journal record hash is invalid".into(),
            });
        }
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(RestoreError::JournalAudit {
                message: "journal sequence overflow".into(),
            })?;
        expected_previous = record.record_hash.clone();
        records.push(record);
    }
    Ok(JournalAudit { records })
}

fn journal_io(operation: &'static str, error: std::io::Error) -> RestoreError {
    RestoreError::JournalIo {
        operation,
        kind: error.kind(),
        message: error.to_string(),
    }
}

fn record_hash(
    job_id_sha256: &[u8; 32],
    sequence: u64,
    previous_hash: &str,
    kind: &str,
    payload: &[u8],
) -> Result<String, RestoreError> {
    let previous = decode_hash(previous_hash)?;
    let kind_len = u32::try_from(kind.len()).map_err(|_| RestoreError::JournalSerialization {
        message: "journal kind is too long".into(),
    })?;
    let payload_len =
        u64::try_from(payload.len()).map_err(|_| RestoreError::JournalSerialization {
            message: "journal payload is too long".into(),
        })?;
    let mut hasher = Sha256::new();
    hasher.update(HASH_DOMAIN);
    hasher.update(job_id_sha256);
    hasher.update(sequence.to_le_bytes());
    hasher.update(previous);
    hasher.update(kind_len.to_le_bytes());
    hasher.update(kind.as_bytes());
    hasher.update(payload_len.to_le_bytes());
    hasher.update(payload);
    Ok(hex_lower(&hasher.finalize()))
}

fn decode_hash(encoded: &str) -> Result<[u8; 32], RestoreError> {
    if encoded.len() != 64 {
        return Err(RestoreError::JournalAudit {
            message: "journal hash length is invalid".into(),
        });
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let pair = &encoded[index * 2..index * 2 + 2];
        *byte = u8::from_str_radix(pair, 16).map_err(|_| RestoreError::JournalAudit {
            message: "journal hash encoding is invalid".into(),
        })?;
    }
    Ok(bytes)
}

pub(crate) fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Copy)]
    enum FailAt {
        Write,
        Flush,
        Sync,
    }

    struct FaultSink {
        bytes: Arc<Mutex<Vec<u8>>>,
        fail_at: FailAt,
    }

    impl Write for FaultSink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if matches!(self.fail_at, FailAt::Write) {
                return Err(io::Error::other("injected write"));
            }
            self.bytes.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if matches!(self.fail_at, FailAt::Flush) {
                return Err(io::Error::other("injected flush"));
            }
            Ok(())
        }
    }

    impl DurableJournalSink for FaultSink {
        fn sync_all(&self) -> io::Result<()> {
            if matches!(self.fail_at, FailAt::Sync) {
                return Err(io::Error::other("injected sync"));
            }
            Ok(())
        }
    }

    #[test]
    fn journal_failure_at_each_durability_boundary_does_not_advance_sequence() {
        for fail_at in [FailAt::Write, FailAt::Flush, FailAt::Sync] {
            let mut journal = JobJournal::with_sink(
                Box::new(FaultSink {
                    bytes: Arc::new(Mutex::new(Vec::new())),
                    fail_at,
                }),
                [7; 32],
            );
            assert!(journal
                .append("itemPrepared", &serde_json::json!({}))
                .is_err());
            assert_eq!(journal.next_sequence, 0);
            assert_eq!(journal.previous_hash, ZERO_HASH);
            assert!(journal.poisoned);
            assert!(matches!(
                journal.append("itemFailed", &serde_json::json!({})),
                Err(RestoreError::JournalPoisoned)
            ));
        }
    }

    #[test]
    fn journal_audit_rejects_tampering_and_truncated_records() {
        let truncated = br#"{"version":1}"#;
        assert!(audit_journal(truncated).is_err());

        let job_id_sha256 = [9u8; 32];
        let payload = serde_json::json!({});
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let hash = record_hash(&job_id_sha256, 0, ZERO_HASH, "jobStarted", &payload_bytes).unwrap();
        let record = JournalRecord {
            version: JOURNAL_VERSION,
            job_id_sha256: hex_lower(&job_id_sha256),
            sequence: 0,
            previous_hash: ZERO_HASH.into(),
            kind: "jobStarted".into(),
            payload_length: payload_bytes.len(),
            payload,
            record_hash: hash,
        };
        let mut valid = serde_json::to_vec(&record).unwrap();
        valid.push(b'\n');
        assert!(audit_journal(&valid).is_ok());
        let kind_position = valid
            .windows(b"jobStarted".len())
            .position(|window| window == b"jobStarted")
            .unwrap();
        valid[kind_position + 5] = b'X';
        assert!(matches!(
            audit_journal(&valid),
            Err(RestoreError::JournalAudit { message })
                if message.contains("record hash")
        ));
    }

    #[test]
    fn journal_audit_rejects_a_complete_record_over_the_append_bound() {
        let mut oversized = vec![b' '; MAX_JOURNAL_RECORD_BYTES];
        oversized.push(b'\n');

        assert!(matches!(
            audit_journal(&oversized),
            Err(RestoreError::JournalRecordTooLarge {
                actual,
                maximum: MAX_JOURNAL_RECORD_BYTES,
            }) if actual == oversized.len()
        ));
    }

    #[test]
    fn journal_audit_rejects_a_payload_over_the_append_bound() {
        let job_id_sha256 = [5u8; 32];
        let payload = Value::String("x".repeat(MAX_JOURNAL_PAYLOAD_BYTES));
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        assert!(payload_bytes.len() > MAX_JOURNAL_PAYLOAD_BYTES);
        let hash =
            record_hash(&job_id_sha256, 0, ZERO_HASH, "itemPrepared", &payload_bytes).unwrap();
        let record = JournalRecord {
            version: JOURNAL_VERSION,
            job_id_sha256: hex_lower(&job_id_sha256),
            sequence: 0,
            previous_hash: ZERO_HASH.into(),
            kind: "itemPrepared".into(),
            payload_length: payload_bytes.len(),
            payload,
            record_hash: hash,
        };
        let mut bytes = serde_json::to_vec(&record).unwrap();
        bytes.push(b'\n');
        assert!(bytes.len() <= MAX_JOURNAL_RECORD_BYTES);

        assert!(matches!(
            audit_journal(&bytes),
            Err(RestoreError::JournalPayloadTooLarge {
                actual,
                maximum: MAX_JOURNAL_PAYLOAD_BYTES,
            }) if actual == payload_bytes.len()
        ));
    }

    #[test]
    fn journal_audit_rejects_a_job_identity_change_inside_one_chain() {
        let first_job = [1u8; 32];
        let second_job = [2u8; 32];
        let payload = serde_json::json!({});
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let first_hash =
            record_hash(&first_job, 0, ZERO_HASH, "jobStarted", &payload_bytes).unwrap();
        let first = JournalRecord {
            version: JOURNAL_VERSION,
            job_id_sha256: hex_lower(&first_job),
            sequence: 0,
            previous_hash: ZERO_HASH.into(),
            kind: "jobStarted".into(),
            payload_length: payload_bytes.len(),
            payload: payload.clone(),
            record_hash: first_hash.clone(),
        };
        let second_hash =
            record_hash(&second_job, 1, &first_hash, "jobCompleted", &payload_bytes).unwrap();
        let second = JournalRecord {
            version: JOURNAL_VERSION,
            job_id_sha256: hex_lower(&second_job),
            sequence: 1,
            previous_hash: first_hash,
            kind: "jobCompleted".into(),
            payload_length: payload_bytes.len(),
            payload,
            record_hash: second_hash,
        };
        let bytes = [
            serde_json::to_vec(&first).unwrap(),
            b"\n".to_vec(),
            serde_json::to_vec(&second).unwrap(),
            b"\n".to_vec(),
        ]
        .concat();

        assert!(matches!(
            audit_journal(&bytes),
            Err(RestoreError::JournalAudit { message })
                if message.contains("job identity")
        ));
    }

    #[test]
    fn journal_audit_rejects_noncanonical_and_unknown_envelope_bytes() {
        let job_id_sha256 = [6u8; 32];
        let payload = serde_json::json!({"alpha": 1, "beta": 2});
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let hash = record_hash(&job_id_sha256, 0, ZERO_HASH, "jobStarted", &payload_bytes).unwrap();
        let record = JournalRecord {
            version: JOURNAL_VERSION,
            job_id_sha256: hex_lower(&job_id_sha256),
            sequence: 0,
            previous_hash: ZERO_HASH.into(),
            kind: "jobStarted".into(),
            payload_length: payload_bytes.len(),
            payload,
            record_hash: hash,
        };
        let canonical = serde_json::to_vec(&record).unwrap();

        let mut leading_whitespace = vec![b' '];
        leading_whitespace.extend_from_slice(&canonical);
        leading_whitespace.push(b'\n');
        assert!(matches!(
            audit_journal(&leading_whitespace),
            Err(RestoreError::JournalAudit { message })
                if message.contains("canonical")
        ));

        let mut crlf = canonical.clone();
        crlf.extend_from_slice(b"\r\n");
        assert!(matches!(
            audit_journal(&crlf),
            Err(RestoreError::JournalAudit { message })
                if message.contains("canonical")
        ));

        let mut reordered: Value = serde_json::from_slice(&canonical).unwrap();
        let map = reordered.as_object_mut().unwrap();
        let version = map.remove("version").unwrap();
        map.insert("version".into(), version);
        let mut reordered_bytes = serde_json::to_vec(&reordered).unwrap();
        reordered_bytes.push(b'\n');
        assert_ne!(&reordered_bytes[..reordered_bytes.len() - 1], canonical);
        assert!(matches!(
            audit_journal(&reordered_bytes),
            Err(RestoreError::JournalAudit { message })
                if message.contains("canonical")
        ));

        let mut unknown: Value = serde_json::from_slice(&canonical).unwrap();
        unknown
            .as_object_mut()
            .unwrap()
            .insert("unknownEnvelope".into(), Value::Bool(true));
        let mut unknown_bytes = serde_json::to_vec(&unknown).unwrap();
        unknown_bytes.push(b'\n');
        assert!(matches!(
            audit_journal(&unknown_bytes),
            Err(RestoreError::JournalAudit { message })
                if message.contains("canonical")
        ));
    }

    #[test]
    fn journal_audit_enforces_the_record_count_bound_before_retention() {
        let job = [8u8; 32];
        let payload = serde_json::json!({});
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let first_hash = record_hash(&job, 0, ZERO_HASH, "jobStarted", &payload_bytes).unwrap();
        let first = JournalRecord {
            version: JOURNAL_VERSION,
            job_id_sha256: hex_lower(&job),
            sequence: 0,
            previous_hash: ZERO_HASH.into(),
            kind: "jobStarted".into(),
            payload_length: payload_bytes.len(),
            payload: payload.clone(),
            record_hash: first_hash.clone(),
        };
        let second_hash =
            record_hash(&job, 1, &first_hash, "jobCompleted", &payload_bytes).unwrap();
        let second = JournalRecord {
            version: JOURNAL_VERSION,
            job_id_sha256: hex_lower(&job),
            sequence: 1,
            previous_hash: first_hash,
            kind: "jobCompleted".into(),
            payload_length: payload_bytes.len(),
            payload,
            record_hash: second_hash,
        };
        let bytes = [
            serde_json::to_vec(&first).unwrap(),
            b"\n".to_vec(),
            serde_json::to_vec(&second).unwrap(),
            b"\n".to_vec(),
        ]
        .concat();

        assert!(matches!(
            audit_journal_with_record_limit(&bytes, 1),
            Err(RestoreError::JournalRecordLimit { maximum: 1 })
        ));
    }

    #[test]
    fn journal_bounds_records_and_payloads_before_touching_the_sink() {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let mut record_limited = JobJournal::with_sink(
            Box::new(FaultSink {
                bytes: Arc::clone(&bytes),
                fail_at: FailAt::Write,
            }),
            [3; 32],
        );
        record_limited.next_sequence = MAX_JOURNAL_RECORDS;
        assert!(matches!(
            record_limited.append("jobCompleted", &serde_json::json!({})),
            Err(RestoreError::JournalRecordLimit {
                maximum: MAX_JOURNAL_RECORDS
            })
        ));
        assert!(!record_limited.poisoned);
        assert!(bytes.lock().unwrap().is_empty());

        let mut payload_limited = JobJournal::with_sink(
            Box::new(FaultSink {
                bytes: Arc::clone(&bytes),
                fail_at: FailAt::Write,
            }),
            [4; 32],
        );
        let oversized = "x".repeat(MAX_JOURNAL_PAYLOAD_BYTES + 1);
        assert!(matches!(
            payload_limited.append("itemFailed", &serde_json::json!({"message": oversized})),
            Err(RestoreError::JournalPayloadTooLarge {
                maximum: MAX_JOURNAL_PAYLOAD_BYTES,
                ..
            })
        ));
        assert!(!payload_limited.poisoned);
        assert_eq!(payload_limited.next_sequence, 0);
        assert!(bytes.lock().unwrap().is_empty());
    }
}
