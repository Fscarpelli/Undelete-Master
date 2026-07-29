# Architecture Decision Records

ADRs capture durable architecture and security decisions. `Accepted` means the
decision is adopted; it is not proof that every consequence is implemented.
`Proposed` records the required direction before implementation and remains
subject to independent review.

## Master specification §23.3 coverage

| Topic | ADR | Decision | Status |
| --- | --- | --- | --- |
| 1 | [0001](0001-tauri-react-rust-target.md) | Tauri, React, TypeScript, and Rust target | Accepted |
| 2 | [0008](0008-process-separation-and-uac.md) | Process separation and UAC boundary | Proposed |
| 3 | [0002](0002-read-only-source-invariant.md) | Read-only source invariant | Accepted |
| 4 | [0009](0009-ipc-protocol.md) | Broker IPC protocol | Proposed |
| 5 | [0010](0010-sqlite-session-schema.md) | SQLite session schema | Proposed |
| 6 | [0011](0011-ntfs-raw-parsing-and-active-record-apis.md) | NTFS raw parsing versus active-record APIs | Proposed |
| 7 | [0012](0012-filesystem-plugin-architecture.md) | Filesystem plugin architecture | Proposed |
| 8 | [0013](0013-carving-plugin-architecture.md) | Carving plugin architecture | Proposed |
| 9 | [0014](0014-sandbox-strategy.md) | Validation and preview sandbox | Proposed |
| 10 | [0006](0006-explainable-recoverability-score.md) | Explainable bounded recoverability score | Accepted |
| 11 | [0015](0015-same-physical-disk-policy.md) | Same-physical-disk destination policy | Proposed |
| 12 | [0016](0016-dependency-and-license-policy.md) | Dependency and license policy | Proposed |
| 13 | [0017](0017-installer-and-update-strategy.md) | Installer and update strategy | Proposed |
| 14 | [0018](0018-refs-scope.md) | ReFS scope | Proposed |
| 15 | [0019](0019-windows-support-matrix.md) | Windows 10/11 support matrix | Proposed |
| 16 | [0020](0020-image-format-architecture.md) | RAW, segmented RAW, VHD, and VHDX image architecture | Proposed |
| 17 | [0004](0004-deterministic-synthetic-fixtures.md) | Deterministic fixtures and external forensic corpora | Accepted |

## Increment-specific decisions

| ADR | Decision | Status |
| --- | --- | --- |
| [0003](0003-regular-image-only-cli.md) | Current CLI accepts one regular contiguous image file only | Accepted |
| [0005](0005-pull-request-ci-excludes-devices.md) | Pull-request CI excludes devices and elevation | Accepted |
| [0007](0007-demo-provider-trust-state.md) | Demonstration-provider proposal | Superseded by [0021](0021-real-only-image-desktop.md) |
| [0021](0021-real-only-image-desktop.md) | Unelevated Tauri image desktop uses only the real scanner and fails closed without it | Accepted |
| [0022](0022-windows-locality-boundary-and-path-identity.md) | Minimal read-only Windows locality FFI with explicit residual path-identity races | Accepted |

A new ADR supersedes an accepted decision; accepted history is never silently
rewritten. Proposed records must be reviewed and moved to Accepted before their
architecture is implemented.
