# AGENTS.md — Undelete Master

## Non-negotiable invariants

- **Never write to a scan source.** All `SourceReader` implementations are read-only; no crate in the
  scan path may open a source with write access or issue write/trim/format/dismount commands.
- **Never run destructive tests against real disks.** Tests use only synthetic in-repo images produced
  by `crates/fixture-builder` or disposable, explicitly allowlisted test VHDs.
- The desktop UI stays unelevated; elevation is restricted to the (future) read-only broker.
- Recovered files are untrusted content: never execute, never preview in a privileged process.
- No production TODOs, stubs, fake data, skipped critical tests, or unsupported completion claims.
- All parser arithmetic is checked/bounded; parsers never read outside the provided buffer or region.

## Required commands before completion

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# frontend (apps/desktop): pnpm lint && pnpm typecheck && pnpm test
```

## Change discipline

- Update `docs/traceability-matrix.md` when behavior changes.
- Add an ADR under `docs/adr/` for architecture/security deviations from the master spec
  (`UNDELETE_MASTER_CODEX_MASTER_SPEC.md`).
- Every bug fix needs a regression test.
- `unsafe` code is forbidden except in an audited Windows FFI boundary crate (not yet present);
  every other crate declares `#![forbid(unsafe_code)]`.
- Fixtures are deterministic; expected recovery results are asserted by SHA-256.

## Directory ownership

- `crates/core` — domain types and traits; no I/O, no OS deps.
- `crates/io-common` — read-only file/image readers.
- `crates/partition` — MBR/GPT parsing.
- `crates/fs-*` — filesystem scanners (buffer/image based, OS independent).
- `crates/carving` — signature carving plugins.
- `crates/restore` — restore planning/execution (path sanitation is security-critical).
- `crates/fixture-builder` — deterministic synthetic images + truth manifests.
- `crates/cli` — headless scan/extract front end.
- `apps/desktop` — Tauri 2 + React UI.
