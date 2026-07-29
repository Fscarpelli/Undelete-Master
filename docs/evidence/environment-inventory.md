# S0 Environment Inventory

Captured: 2026-07-29  
Scope: discovery evidence, not release verification

## Repository

- Branch: `codex/foundation-hardening`.
- Design baseline: `f9befd5`.
- Local historical branch `master`: `127c2af`.
- No remote or upstream was configured during the baseline audit.
- The integration policy preserves the remote `main` commit through a
  non-force branch/PR; Task 7 does not publish or rewrite history.

## Toolchains observed

- Cargo 1.88.0.
- rustc 1.88.0.
- rustfmt 1.8.0-stable.
- Node 22.17.1.
- npm 11.6.2.
- pnpm 10.12.1.
- Git for Windows.

Tool presence is not test evidence. Task 8 records final command output on the
final tree.

## Repository capabilities at discovery

- Read-only image reader, checked regions, partition parsing, NTFS/FAT metadata
  scanners, deterministic fixture builder, and component tests.
- React/Vite desktop demonstration backed by synthetic data.
- No Tauri shell, Windows broker, restore engine, carving/validation worker,
  session database, production exFAT, installer, signing, or release evidence.

## Baseline failures and gaps

- `cargo fmt --all -- --check` failed on formatting drift.
- The baseline recorded Clippy failure in FAT code.
- Frontend typecheck/tests passed in the baseline record, but the required lint
  command did not exist.
- No PR workflow, complete SDD family, traceability matrix, or risk register
  existed.
- Device and VHD tests were not run.

These are historical baseline observations captured in SDD-016. They must not
be cited as final results after concurrent implementation changes.

## Agents and permissions

The implementation plan is being executed by scoped workers with shared
filesystem access and non-overlapping ownership. Files outside each worker's
ownership are preserved. No plugin, MCP, or external publisher is required for
Task 7, and no remote mutation is authorized.

