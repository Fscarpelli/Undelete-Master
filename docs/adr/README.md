# Architecture Decision Records

ADRs capture durable architecture/security decisions. Accepted records are not
proof that every consequence has been implemented.

| ADR | Decision | Status |
| --- | --- | --- |
| [0001](0001-tauri-react-rust-target.md) | Tauri, React, TypeScript, and Rust target | Accepted |
| [0002](0002-read-only-source-invariant.md) | Read-only source invariant | Accepted |
| [0003](0003-regular-image-only-cli.md) | Current CLI accepts regular image files only | Accepted |
| [0004](0004-deterministic-synthetic-fixtures.md) | Deterministic synthetic fixtures and SHA-256 truth | Accepted |
| [0005](0005-pull-request-ci-excludes-devices.md) | Pull-request CI excludes devices and elevation | Accepted |
| [0006](0006-explainable-recoverability-score.md) | Explainable bounded recoverability score | Accepted |
| [0007](0007-demo-provider-trust-state.md) | Synthetic desktop data has an explicit unverified trust state | Accepted |

Future decisions required by master spec §23.3 remain open and must be recorded
before their implementation starts. A new ADR supersedes an accepted decision;
accepted history is not silently rewritten.
