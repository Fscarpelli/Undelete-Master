# SDD-012 — Test and Validation Plan

Status: Normative; coverage `Partial`

## Pull-request gates

- Rust format, Clippy with warnings denied, workspace unit/property/integration
  tests, and debug/release smoke where practical.
- Frontend deterministic install, lint, TypeScript strict, Vitest, and build.
- Documentation required-file, ID, status, and local-link validation.
- No real disk, raw device, VHD attach, format, trim, lock, or dismount command.

## Test levels

| Level | Required evidence |
| --- | --- |
| Unit/property | Parser primitives, bounds, runlists, score, path and protocol invariants. |
| Integration | Deterministic image scan/extraction and SHA-256 truth comparison. |
| Fuzz | MBR/GPT, filesystem records, signatures, validators, IPC, imports, and restore paths. |
| Frontend | Contract, state, keyboard, i18n, errors, and accessibility. |
| Tauri E2E | Real shell commands, permissions, logs, screenshots, and offline flow. |
| Security | Broker spoofing, oversized frames, traversal, reparse races, bombs, and source-write prohibition. |
| Performance | Sequential scan, cancellation, DB/filter latency, memory, and one-million-row UI. |

Only the first two categories and limited frontend tests currently have partial
implementation. Fuzz, real Tauri E2E, security boundary, external corpora, and
performance evidence remain open.

## Evidence rule

`Verified` requires a command, revision, result, and artifact link. Historical
or locally claimed success without retained evidence is
`Implemented-unverified` at most.

