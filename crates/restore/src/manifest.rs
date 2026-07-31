use crate::{journal::hex_lower, StreamOutcome, ZeroFillReason, ZeroFilledRange};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub(crate) const MANIFEST_VERSION: u32 = 1;
pub(crate) const MANIFEST_NAME: &str = "recovery-manifest.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum NamespaceDurability {
    Synced,
    Unsupported { kind: String, message: String },
    Failed { kind: String, message: String },
}

impl NamespaceDurability {
    pub fn is_synced(&self) -> bool {
        matches!(self, Self::Synced)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RestoreCompletionStatus {
    CompletedDurable,
    NeedsReconciliation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TemporaryFileDisposition {
    Removed,
    RetainedForReconciliation,
    RetainedAfterCleanupFailure,
    RetainedBySafeCleanupPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManifestItemDisposition {
    Published,
    Failed,
    Cancelled,
    NotAttempted,
    DirectoryCreated,
}

impl ManifestItemDisposition {
    fn as_str(self) -> &'static str {
        match self {
            Self::Published => "published",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::NotAttempted => "notAttempted",
            Self::DirectoryCreated => "directoryCreated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreItemResult {
    pub(crate) candidate_id: u64,
    pub(crate) item_key: String,
    pub(crate) requested_path: String,
    pub(crate) published_path: String,
    pub(crate) published_name: String,
    pub(crate) output_len: u64,
    pub(crate) output_sha256: [u8; 32],
    pub(crate) expected_sha256: Option<[u8; 32]>,
    pub(crate) zero_filled_ranges: Vec<ZeroFilledRange>,
    pub(crate) sidecar_name: Option<String>,
    pub(crate) sidecar_sha256: Option<[u8; 32]>,
    pub(crate) temporary_disposition: TemporaryFileDisposition,
    pub(crate) path_evidence: serde_json::Value,
    pub(crate) warnings: Vec<String>,
    pub(crate) namespace_durability: NamespaceDurability,
    pub(crate) completion_status: RestoreCompletionStatus,
}

impl RestoreItemResult {
    pub fn candidate_id(&self) -> u64 {
        self.candidate_id
    }

    pub fn published_path(&self) -> &str {
        &self.published_path
    }

    pub fn published_name(&self) -> &str {
        &self.published_name
    }

    pub fn output_len(&self) -> u64 {
        self.output_len
    }

    pub fn output_sha256(&self) -> [u8; 32] {
        self.output_sha256
    }

    pub fn temporary_disposition(&self) -> TemporaryFileDisposition {
        self.temporary_disposition
    }

    pub fn namespace_durability(&self) -> &NamespaceDurability {
        &self.namespace_durability
    }

    pub fn completion_status(&self) -> RestoreCompletionStatus {
        self.completion_status
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreSummary {
    pub(crate) job_directory_name: String,
    pub(crate) manifest_name: String,
    pub(crate) journal_name: String,
    pub(crate) manifest_sha256: [u8; 32],
    pub(crate) items: Vec<RestoreItemResult>,
    pub(crate) completion_status: RestoreCompletionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestItemOutcome {
    pub(crate) candidate_id: u64,
    pub(crate) item_key: String,
    pub(crate) item_kind: &'static str,
    pub(crate) disposition: ManifestItemDisposition,
    pub(crate) requested_path: String,
    pub(crate) expected_sha256: Option<[u8; 32]>,
    pub(crate) warnings: Vec<String>,
    pub(crate) path_evidence: serde_json::Value,
    pub(crate) failure_kind: Option<&'static str>,
    pub(crate) published_sidecar: Option<PublishedSidecarEvidence>,
    pub(crate) published: Option<RestoreItemResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PublishedSidecarEvidence {
    pub(crate) item_key: String,
    pub(crate) name: String,
    pub(crate) sha256: [u8; 32],
}

impl RestoreSummary {
    pub fn job_directory_name(&self) -> &str {
        &self.job_directory_name
    }

    pub fn manifest_name(&self) -> &str {
        &self.manifest_name
    }

    pub fn journal_name(&self) -> &str {
        &self.journal_name
    }

    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }

    pub fn items(&self) -> &[RestoreItemResult] {
        &self.items
    }

    pub fn completion_status(&self) -> RestoreCompletionStatus {
        self.completion_status
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryManifest<'a> {
    version: u32,
    job_id: &'a str,
    journal_head_sha256: &'a str,
    items: Vec<ManifestItem<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestItem<'a> {
    candidate_id: u64,
    item_key: &'a str,
    item_kind: &'static str,
    disposition: &'static str,
    requested_path: &'a str,
    published_path: Option<&'a str>,
    output_length: Option<u64>,
    output_sha256: Option<String>,
    expected_sha256: Option<String>,
    readable_ranges: Vec<ManifestRange>,
    zero_filled_ranges: Vec<ManifestZeroRange>,
    conflicts: Vec<ManifestRange>,
    read_errors: Vec<ManifestRange>,
    warnings: &'a [String],
    partial_sidecar: Option<&'a str>,
    partial_sidecar_sha256: Option<String>,
    temporary_file_disposition: Option<TemporaryFileDisposition>,
    namespace_durability: Option<&'a NamespaceDurability>,
    completion_status: Option<RestoreCompletionStatus>,
    failure_kind: Option<&'static str>,
    path_evidence: &'a serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestRange {
    logical_offset: u64,
    length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestZeroRange {
    logical_offset: u64,
    length: u64,
    reason: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PartialSidecar {
    version: u32,
    item_key: String,
    candidate_id: u64,
    logical_size: u64,
    readable_ranges: Vec<ManifestRange>,
    zero_filled_ranges: Vec<ManifestZeroRange>,
    zero_filled_missing_ranges: Vec<ManifestZeroRange>,
    conflict_ranges: Vec<ManifestRange>,
    source_read_errors: Vec<ManifestRange>,
    output_sha256: String,
    expected_content_sha256: Option<String>,
    validation: SidecarValidation,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SidecarValidation {
    expected_sha256_present: bool,
    expected_sha256_matched: bool,
}

pub(crate) fn build_manifest(
    job_id: &str,
    journal_head: &str,
    items: &[ManifestItemOutcome],
) -> Result<(Vec<u8>, [u8; 32]), serde_json::Error> {
    let entries = items.iter().map(manifest_item).collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&RecoveryManifest {
        version: MANIFEST_VERSION,
        job_id,
        journal_head_sha256: journal_head,
        items: entries,
    })?;
    let sha256 = Sha256::digest(&bytes).into();
    Ok((bytes, sha256))
}

pub(crate) fn build_partial_sidecar(
    item_key: &str,
    candidate_id: u64,
    logical_size: u64,
    outcome: &StreamOutcome,
    expected_sha256: Option<[u8; 32]>,
    warnings: &[String],
) -> Result<Option<Vec<u8>>, serde_json::Error> {
    if !outcome.requires_best_effort {
        return Ok(None);
    }
    let zero_filled_missing_ranges = outcome
        .zero_filled_ranges
        .iter()
        .filter(|range| {
            !matches!(
                range.reason,
                ZeroFillReason::CurrentlyAllocated | ZeroFillReason::PreviouslyReadFailed
            )
        })
        .map(manifest_zero_range)
        .collect();
    let bytes = serde_json::to_vec(&PartialSidecar {
        version: MANIFEST_VERSION,
        item_key: item_key.to_owned(),
        candidate_id,
        logical_size,
        readable_ranges: readable_ranges(logical_size, &outcome.zero_filled_ranges),
        zero_filled_ranges: outcome
            .zero_filled_ranges
            .iter()
            .map(manifest_zero_range)
            .collect(),
        zero_filled_missing_ranges,
        conflict_ranges: filtered_ranges(
            &outcome.zero_filled_ranges,
            ZeroFillReason::CurrentlyAllocated,
        ),
        source_read_errors: filtered_ranges(
            &outcome.zero_filled_ranges,
            ZeroFillReason::PreviouslyReadFailed,
        ),
        output_sha256: hex_lower(&outcome.sha256),
        expected_content_sha256: expected_sha256.map(|hash| hex_lower(&hash)),
        validation: SidecarValidation {
            expected_sha256_present: expected_sha256.is_some(),
            expected_sha256_matched: expected_sha256.is_some_and(|hash| hash == outcome.sha256),
        },
        warnings: warnings.to_vec(),
    })?;
    Ok(Some(bytes))
}

fn manifest_item(outcome: &ManifestItemOutcome) -> ManifestItem<'_> {
    let published = outcome.published.as_ref();
    let file_published = (outcome.disposition == ManifestItemDisposition::Published)
        .then_some(published)
        .flatten();
    let zero_filled_ranges = file_published
        .map(|result| result.zero_filled_ranges.as_slice())
        .unwrap_or_default();
    let output_len = file_published.map(|result| result.output_len);
    let conflicts = filtered_ranges(zero_filled_ranges, ZeroFillReason::CurrentlyAllocated);
    let read_errors = filtered_ranges(zero_filled_ranges, ZeroFillReason::PreviouslyReadFailed);
    ManifestItem {
        candidate_id: outcome.candidate_id,
        item_key: &outcome.item_key,
        item_kind: outcome.item_kind,
        disposition: outcome.disposition.as_str(),
        requested_path: &outcome.requested_path,
        published_path: published.map(|result| result.published_path.as_str()),
        output_length: output_len,
        output_sha256: file_published.map(|result| hex_lower(&result.output_sha256)),
        expected_sha256: outcome.expected_sha256.map(|hash| hex_lower(&hash)),
        readable_ranges: output_len
            .map(|len| readable_ranges(len, zero_filled_ranges))
            .unwrap_or_default(),
        zero_filled_ranges: zero_filled_ranges.iter().map(manifest_zero_range).collect(),
        conflicts,
        read_errors,
        warnings: &outcome.warnings,
        partial_sidecar: outcome
            .published_sidecar
            .as_ref()
            .map(|sidecar| sidecar.name.as_str()),
        partial_sidecar_sha256: outcome
            .published_sidecar
            .as_ref()
            .map(|sidecar| hex_lower(&sidecar.sha256)),
        temporary_file_disposition: file_published.map(|result| result.temporary_disposition),
        namespace_durability: published.map(|result| &result.namespace_durability),
        completion_status: published.map(|result| result.completion_status),
        failure_kind: outcome.failure_kind,
        path_evidence: &outcome.path_evidence,
    }
}

fn filtered_ranges(ranges: &[ZeroFilledRange], reason: ZeroFillReason) -> Vec<ManifestRange> {
    ranges
        .iter()
        .filter(|range| range.reason == reason)
        .map(|range| ManifestRange {
            logical_offset: range.logical_offset,
            length: range.len,
        })
        .collect()
}

fn readable_ranges(logical_size: u64, zero_ranges: &[ZeroFilledRange]) -> Vec<ManifestRange> {
    let mut readable = Vec::with_capacity(zero_ranges.len().saturating_add(1));
    let mut cursor = 0u64;
    for range in zero_ranges {
        if cursor < range.logical_offset {
            readable.push(ManifestRange {
                logical_offset: cursor,
                length: range.logical_offset - cursor,
            });
        }
        cursor = range
            .logical_offset
            .checked_add(range.len)
            .unwrap_or(logical_size)
            .min(logical_size);
    }
    if cursor < logical_size {
        readable.push(ManifestRange {
            logical_offset: cursor,
            length: logical_size - cursor,
        });
    }
    readable
}

fn manifest_zero_range(range: &ZeroFilledRange) -> ManifestZeroRange {
    ManifestZeroRange {
        logical_offset: range.logical_offset,
        length: range.len,
        reason: reason_name(range.reason),
    }
}

fn reason_name(reason: ZeroFillReason) -> &'static str {
    match reason {
        ZeroFillReason::Sparse => "sparse",
        ZeroFillReason::MissingExtent => "missingExtent",
        ZeroFillReason::CurrentlyAllocated => "currentlyAllocated",
        ZeroFillReason::OutOfVolume => "outOfVolume",
        ZeroFillReason::PreviouslyReadFailed => "previouslyReadFailed",
        ZeroFillReason::Zeroed => "zeroed",
        ZeroFillReason::UnknownAvailability => "unknownAvailability",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_records_expected_hash_conflicts_read_errors_and_temp_disposition() {
        let result = RestoreItemResult {
            candidate_id: 73,
            item_key: "000000-0000000000000049".into(),
            requested_path: "unsafe-source-name".into(),
            published_path: "safe/output.bin".into(),
            published_name: "output.bin".into(),
            output_len: 12,
            output_sha256: [0x11; 32],
            expected_sha256: Some([0x22; 32]),
            zero_filled_ranges: vec![
                ZeroFilledRange {
                    logical_offset: 2,
                    len: 3,
                    reason: ZeroFillReason::CurrentlyAllocated,
                },
                ZeroFilledRange {
                    logical_offset: 8,
                    len: 2,
                    reason: ZeroFillReason::PreviouslyReadFailed,
                },
            ],
            sidecar_name: Some("output.bin.um-partial.json".into()),
            sidecar_sha256: Some([0x33; 32]),
            temporary_disposition: TemporaryFileDisposition::RetainedForReconciliation,
            path_evidence: serde_json::json!({
                "version": 1,
                "substitutions": [{"original": "unsafe-source-name"}]
            }),
            warnings: vec!["review required".into()],
            namespace_durability: NamespaceDurability::Unsupported {
                kind: "Unsupported".into(),
                message: "directory sync unavailable".into(),
            },
            completion_status: RestoreCompletionStatus::NeedsReconciliation,
        };

        let outcome = ManifestItemOutcome {
            candidate_id: 73,
            item_key: "000000-0000000000000049".into(),
            item_kind: "file",
            disposition: ManifestItemDisposition::Published,
            requested_path: "unsafe-source-name".into(),
            expected_sha256: Some([0x22; 32]),
            warnings: vec!["review required".into()],
            path_evidence: serde_json::json!({
                "version": 1,
                "substitutions": [{"original": "unsafe-source-name"}]
            }),
            failure_kind: None,
            published_sidecar: Some(PublishedSidecarEvidence {
                item_key: "000000-0000000000000049".into(),
                name: "output.bin.um-partial.json".into(),
                sha256: [0x33; 32],
            }),
            published: Some(result),
        };
        let (bytes, sha256) =
            build_manifest("manifest-evidence", &"ab".repeat(32), &[outcome]).unwrap();
        assert_eq!(<[u8; 32]>::from(Sha256::digest(&bytes)), sha256);
        let manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let item = &manifest["items"][0];
        assert_eq!(
            item["expectedSha256"],
            "2222222222222222222222222222222222222222222222222222222222222222"
        );
        assert_eq!(
            item["readableRanges"],
            serde_json::json!([
                {"logicalOffset": 0, "length": 2},
                {"logicalOffset": 5, "length": 3},
                {"logicalOffset": 10, "length": 2}
            ])
        );
        assert_eq!(
            item["conflicts"],
            serde_json::json!([{"logicalOffset": 2, "length": 3}])
        );
        assert_eq!(
            item["readErrors"],
            serde_json::json!([{"logicalOffset": 8, "length": 2}])
        );
        assert_eq!(
            item["temporaryFileDisposition"],
            "retainedForReconciliation"
        );
        assert_eq!(item["warnings"], serde_json::json!(["review required"]));
        assert_eq!(
            item["pathEvidence"]["substitutions"][0]["original"],
            "unsafe-source-name"
        );
    }
}
