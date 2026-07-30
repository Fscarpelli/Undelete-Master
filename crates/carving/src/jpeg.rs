#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JpegStatus {
    NeedMore,
    Valid,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JpegFeedResult {
    pub consumed: usize,
    pub status: JpegStatus,
}

#[derive(Debug, Clone, Copy)]
enum ValidatorState {
    SoiPrefix,
    SoiCode,
    MarkerPrefix,
    MarkerCode,
    LengthHigh { marker: u8 },
    LengthLow { marker: u8, high: u8 },
    Segment(SegmentState),
    Entropy,
    EntropyMarker,
    Finished,
    Rejected,
}

#[derive(Debug, Clone, Copy)]
struct SegmentState {
    marker: u8,
    declared_len: u16,
    remaining: u16,
    prefix: [u8; 6],
    captured: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Continue,
    Valid,
    Invalid,
}

/// Incremental, fixed-memory JPEG marker validator.
///
/// Every supplied byte is consumed at most once. Segment payloads are skipped
/// in place while only the six bytes needed for SOF/SOS checks are retained.
pub(crate) struct JpegValidator {
    state: ValidatorState,
    saw_sof: bool,
    saw_sos: bool,
    processed_bytes: u64,
}

impl JpegValidator {
    pub(crate) fn new() -> Self {
        Self {
            state: ValidatorState::SoiPrefix,
            saw_sof: false,
            saw_sos: false,
            processed_bytes: 0,
        }
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) -> JpegFeedResult {
        if matches!(self.state, ValidatorState::Finished) {
            return JpegFeedResult {
                consumed: 0,
                status: JpegStatus::Valid,
            };
        }
        if matches!(self.state, ValidatorState::Rejected) {
            return JpegFeedResult {
                consumed: 0,
                status: JpegStatus::Invalid,
            };
        }

        for (index, byte) in bytes.iter().copied().enumerate() {
            let step = self.consume(byte);
            let Some(processed) = self.processed_bytes.checked_add(1) else {
                self.state = ValidatorState::Rejected;
                return JpegFeedResult {
                    consumed: index + 1,
                    status: JpegStatus::Invalid,
                };
            };
            self.processed_bytes = processed;
            match step {
                Step::Continue => {}
                Step::Valid => {
                    self.state = ValidatorState::Finished;
                    return JpegFeedResult {
                        consumed: index + 1,
                        status: JpegStatus::Valid,
                    };
                }
                Step::Invalid => {
                    self.state = ValidatorState::Rejected;
                    return JpegFeedResult {
                        consumed: index + 1,
                        status: JpegStatus::Invalid,
                    };
                }
            }
        }
        JpegFeedResult {
            consumed: bytes.len(),
            status: JpegStatus::NeedMore,
        }
    }

    pub(crate) fn processed_bytes(&self) -> u64 {
        self.processed_bytes
    }

    fn consume(&mut self, byte: u8) -> Step {
        match self.state {
            ValidatorState::SoiPrefix => {
                if byte != 0xFF {
                    return Step::Invalid;
                }
                self.state = ValidatorState::SoiCode;
                Step::Continue
            }
            ValidatorState::SoiCode => {
                if byte != 0xD8 {
                    return Step::Invalid;
                }
                self.state = ValidatorState::MarkerPrefix;
                Step::Continue
            }
            ValidatorState::MarkerPrefix => {
                if byte != 0xFF {
                    return Step::Invalid;
                }
                self.state = ValidatorState::MarkerCode;
                Step::Continue
            }
            ValidatorState::MarkerCode => {
                if byte == 0xFF {
                    return Step::Continue;
                }
                self.handle_marker(byte)
            }
            ValidatorState::LengthHigh { marker } => {
                self.state = ValidatorState::LengthLow { marker, high: byte };
                Step::Continue
            }
            ValidatorState::LengthLow { marker, high } => {
                let declared_len = u16::from_be_bytes([high, byte]);
                if declared_len < 2 {
                    return Step::Invalid;
                }
                let segment = SegmentState {
                    marker,
                    declared_len,
                    remaining: declared_len - 2,
                    prefix: [0; 6],
                    captured: 0,
                };
                if segment.remaining == 0 {
                    self.finish_segment(segment)
                } else {
                    self.state = ValidatorState::Segment(segment);
                    Step::Continue
                }
            }
            ValidatorState::Segment(mut segment) => {
                if usize::from(segment.captured) < segment.prefix.len() {
                    segment.prefix[usize::from(segment.captured)] = byte;
                    segment.captured += 1;
                }
                segment.remaining -= 1;
                if segment.remaining == 0 {
                    self.finish_segment(segment)
                } else {
                    self.state = ValidatorState::Segment(segment);
                    Step::Continue
                }
            }
            ValidatorState::Entropy => {
                if byte == 0xFF {
                    self.state = ValidatorState::EntropyMarker;
                }
                Step::Continue
            }
            ValidatorState::EntropyMarker => match byte {
                0xFF => Step::Continue,
                0x00 | 0xD0..=0xD7 => {
                    self.state = ValidatorState::Entropy;
                    Step::Continue
                }
                _ => self.handle_marker(byte),
            },
            ValidatorState::Finished => Step::Valid,
            ValidatorState::Rejected => Step::Invalid,
        }
    }

    fn handle_marker(&mut self, marker: u8) -> Step {
        match marker {
            // TEM is the only legal standalone marker below C0.
            0x01 => {
                self.state = ValidatorState::MarkerPrefix;
                Step::Continue
            }
            0xD8 | 0xD0..=0xD7 | 0x00 | 0x02..=0xBF => Step::Invalid,
            0xD9 => {
                if self.saw_sof && self.saw_sos {
                    Step::Valid
                } else {
                    Step::Invalid
                }
            }
            0xC0..=0xFE => {
                self.state = ValidatorState::LengthHigh { marker };
                Step::Continue
            }
            0xFF => Step::Invalid,
        }
    }

    fn finish_segment(&mut self, segment: SegmentState) -> Step {
        if is_sof(segment.marker) {
            if !valid_sof(segment) {
                return Step::Invalid;
            }
            self.saw_sof = true;
            self.state = ValidatorState::MarkerPrefix;
            return Step::Continue;
        }
        if segment.marker == 0xDA {
            if !self.saw_sof || !valid_sos(segment) {
                return Step::Invalid;
            }
            self.saw_sos = true;
            self.state = ValidatorState::Entropy;
            return Step::Continue;
        }
        self.state = ValidatorState::MarkerPrefix;
        Step::Continue
    }
}

fn is_sof(marker: u8) -> bool {
    matches!(
        marker,
        0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF
    )
}

fn valid_sof(segment: SegmentState) -> bool {
    if segment.captured < 6 {
        return false;
    }
    let height = u16::from_be_bytes([segment.prefix[1], segment.prefix[2]]);
    let width = u16::from_be_bytes([segment.prefix[3], segment.prefix[4]]);
    let components = u16::from(segment.prefix[5]);
    let expected_len = 8u16.checked_add(components.saturating_mul(3));
    components > 0 && height > 0 && width > 0 && expected_len == Some(segment.declared_len)
}

fn valid_sos(segment: SegmentState) -> bool {
    if segment.captured < 1 {
        return false;
    }
    let components = u16::from(segment.prefix[0]);
    let expected_len = 6u16.checked_add(components.saturating_mul(2));
    components > 0 && expected_len == Some(segment.declared_len)
}

#[cfg(test)]
mod tests {
    use super::{JpegStatus, JpegValidator};
    use um_fixture_builder::carving::structurally_valid_jpeg;

    #[test]
    fn carve_jpeg_incremental_005_processes_each_byte_once_across_tiny_chunks() {
        let jpeg = structurally_valid_jpeg();
        let mut validator = JpegValidator::new();
        let mut status = JpegStatus::NeedMore;

        for byte in &jpeg {
            let result = validator.feed(&[*byte]);
            assert_eq!(result.consumed, 1);
            status = result.status;
        }

        assert_eq!(status, JpegStatus::Valid);
        assert_eq!(validator.processed_bytes(), jpeg.len() as u64);
    }
}
