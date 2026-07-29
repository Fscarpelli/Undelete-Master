# SDD-003 — Domain Model

Status: Normative

## Core entities

| Entity | Required meaning | Current state |
| --- | --- | --- |
| `SourceIdentity` | Stable identity, kind, label, size, and sector layout for one source. | Partial; image identity exists, device-grade identity does not. |
| `Region` | Checked byte range entirely contained in a source. | Implemented-unverified. |
| `Partition` | Bounded MBR/GPT region with provenance and warnings. | Implemented-unverified. |
| `Candidate` | Potentially recoverable file, directory, or stream with separate metadata/content evidence. | Implemented-unverified for NTFS/FAT metadata paths. |
| `ExtentRun` | Logical-to-physical mapping or sparse/missing/conflicting range. | Partial. |
| `RecoverabilityScore` | Explainable content assessment, independent of name/path confidence. | Partial. |
| `Session` | Persisted scan configuration, checkpoints, candidates, selections, and events. | Not started. |
| `RestorePlan` | Immutable, validated mapping from selected candidates to safe destination paths. | Not started. |
| `ValidationReport` | Bounded structural validation result from an isolated worker. | Not started. |

## Invariants

- Source offsets and sizes are `u64` and use checked arithmetic.
- Metadata confidence, content recoverability, and structural validation are
  distinct fields.
- A carved candidate does not inherit an original name or path.
- Duplicate evidence may merge only with provenance; name equality is
  insufficient.
- A directory selection never implicitly selects historical descendants.
- Recovered content is untrusted and is never executed.

## Serialization

Externally persisted schemas must carry an explicit version. TypeScript payloads
use `camelCase`; Rust command names use `snake_case`. Default diagnostic output
must not expose full local paths.

