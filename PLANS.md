# Foundation Hardening, Safe Image CLI, and Real-only Desktop Plan

> **Historical plan:** Tasks below describe the completed SDD-017 image-only
> increment. The desktop product surface has since been superseded by
> [SDD-018](docs/specs/018-windows-volume-and-folder-scan.md): it inventories
> connected local storage, selects a mounted volume and optional NTFS folder,
> and performs bounded reads through the elevated read-only broker. The CLI
> remains image-only. This history is retained as implementation evidence and
> must not be read as the current desktop contract.

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` or `superpowers:executing-plans` to
> implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Remove known parser correctness hazards, provide a real read-only
image scan entry point, replace the desktop demonstration with a narrow
real-only Tauri application, and establish repeatable SDD/CI evidence.

**Architecture:** Parser fixes remain inside their owning crates. The
image-only CLI composes the existing read-only reader, partition discovery, and
NTFS/FAT scanners without privilege or write behavior. The unelevated Tauri 2
desktop calls that real library through `spawn_blocking`, exposes only its
bounded report, and fails closed outside Tauri. Production contains no
synthetic provider, data, progress, restore, session, or fallback path.

**Tech Stack:** Rust 1.88, Cargo workspace, Tauri 2, React 18, TypeScript 5.7,
Vite 8, Vitest, Testing Library, ESLint, GitHub Actions.

## Global constraints

- Never write to a scan source.
- Never run destructive tests against real disks.
- The desktop UI remains unelevated.
- Recovered files are untrusted and are never executed.
- All parser arithmetic is checked and bounded.
- Every behavior fix starts with a failing regression test.
- Fixtures are deterministic and byte recovery assertions use SHA-256.
- No production TODOs, stubs, fake data, skipped critical tests, or unsupported
  completion claims.
- No mock/demo provider or fabricated domain behavior in production; browser
  runtime fails closed.
- No restore, preview, sessions, raw devices, broker, carving, or exFAT desktop
  surface until a real safe backend exists.
- Update `docs/traceability-matrix.md` for every behavior change.

---

## Task 0 — Real-only specification gate (must precede desktop code)

**Files**

- Create: `docs/specs/017-real-only-image-desktop.md`
- Create: `docs/adr/0021-real-only-image-desktop.md`
- Supersede: `docs/adr/0007-demo-provider-trust-state.md`
- Update traceability, risks, limitations, ADR index, and this plan.

- [x] Define the runtime, IPC, privacy, limits, exclusions, acceptance IDs,
  phases, and revisit triggers.
- [x] Record SDD-REAL requirements as `Not started`.
- [x] Prohibit synthetic fallback and candidate rows not emitted by the real
  scanner.
- [ ] Pass documentation validation and independent R0 review before desktop
  implementation begins.

---

## Task 1 — GPT and sector-layout hardening

**Files**

- Modify: `crates/core/src/source.rs`
- Modify: `crates/io-common/src/lib.rs`
- Modify: `crates/partition/src/lib.rs`
- Modify: `crates/partition/src/gpt.rs`
- Modify: `crates/partition/tests/partition_tables.rs`

**Interfaces**

- Produces: `SectorLayout::new(logical: u32, physical: u32) -> Option<SectorLayout>`
- Preserves: `SourceReader::sector_layout() -> SectorLayout`
- Preserves: `partition::discover(&dyn SourceReader) -> Result<PartitionTable, ScanError>`

- [ ] Write `PART-GPT-OVERFLOW-001`, `PART-GPT-RANGE-001`,
  `CORE-SECTOR-VALID-001`, and `PART-SECTOR-ZERO-001`.
- [ ] Run the focused tests and verify the current panic/invalid acceptance.
- [ ] Add checked range construction and fail-closed sector guards.
- [ ] Run focused tests and the affected crate tests.

## Task 2 — Allocation map and NTFS initialized tail

**Files**

- Modify: `crates/fs-common/src/lib.rs`
- Modify: `crates/fs-ntfs/src/scan.rs`
- Modify only if needed for integration evidence:
  `crates/fixture-builder/src/ntfs.rs`

**Interfaces**

- Preserves: `AllocationMap::from_raw(Vec<u8>, u64) -> AllocationMap`
- Preserves: `AllocationMap::is_allocated(u64) -> Option<bool>`
- Produces internal helper:
  `split_at_initialized_size(extents: Vec<ExtentRun>, initialized_size: u64, data_size: u64) -> Vec<ExtentRun>`

- [ ] Write `FS-MAP-TRUNC-001` and `NTFS-INIT-TAIL-001`.
- [ ] Run focused tests and verify out-of-range indexing/incorrect tail behavior.
- [ ] Return unknown for missing bitmap bits and split physical extents at the
  initialized boundary.
- [ ] Run focused tests, extraction checks, and NTFS integration tests.

## Task 3 — Union-based candidate coverage and score

**Files**

- Modify: `crates/core/src/candidate.rs`
- Modify: `crates/core/src/score.rs`

**Interfaces**

- Produces internal normalized interval helper over candidate logical extents.
- Preserves: `Candidate::covered_len() -> u64`
- Preserves: `Candidate::has_missing_ranges() -> bool`
- Preserves: `RecoverabilityInputs::from_candidate(&Candidate) -> Self`

- [ ] Write `CORE-COVERAGE-UNION-001` and `CORE-SCORE-OVERLAP-001`.
- [ ] Verify both tests fail because duplicate extents hide a logical gap.
- [ ] Normalize clipped intervals and use saturating class accounting.
- [ ] Run `cargo test -p um-core`.

## Task 4 — Safe regular-image CLI

**Files**

- Create: `crates/cli/Cargo.toml`
- Create: `crates/cli/src/lib.rs`
- Create: `crates/cli/src/report.rs`
- Create: `crates/cli/src/main.rs`
- Create: `crates/cli/tests/scan_image.rs`
- Modify: `Cargo.toml`

**Interfaces**

- Produces:
  `scan_image_path(path: &Path) -> Result<ImageScanReport, CliError>`
- Produces:
  `ImageScanReport { schema_version, source, partition_table, volumes, warnings }`
- Consumes: `FileImageReader`, `RegionReader`, `partition::discover`,
  `scan_ntfs`, and `scan_fat`.

- [ ] Write failing tests for deterministic NTFS/FAT reports, unsupported
  extensions, non-regular paths, and path redaction.
- [ ] Run `cargo test -p um-cli` and verify failures precede implementation.
- [ ] Implement regular-file validation, bounded volume discovery/scanning, and
  stable serde output.
- [ ] Add a minimal argument parser for `scan-image <path> [--pretty]`.
- [ ] Run focused tests and one process-level JSON smoke test.

## Task 5 — Real-only Tauri desktop runtime

**Files**

- Create: `apps/desktop/src-tauri/` Tauri 2 shell/configuration.
- Create: the real Tauri command and TypeScript boundary.
- Delete: production mock/demo provider and fabricated datasets.
- Reduce production views/routes to image selection and real report.
- Add focused Rust and component tests.

**Interfaces**

- Produces:
  `select_and_scan_image(requestId) ->
  Result<Option<DesktopScanReport>, DesktopCommandError>`.
- Consumes: `um_cli::scan_image_path` through `spawn_blocking`.

- [ ] Write failing tests for Tauri-only fail-closed runtime, Rust-owned native
  dialog filters/cancel, path-free IPC, backend path rejection, real CLI
  parity, sanitized errors, exact `u64` transport, and report limits.
- [ ] Add the unelevated shell, minimal CSP/capabilities, and one allowlisted
  command.
- [ ] Remove every production mock/demo import, dataset, timer, unsupported
  route, and fallback.
- [ ] Render only real MBR/GPT, NTFS/FAT, candidate-count, and warning values.
- [ ] Prove browser-only runtime produces no data and run focused gates.

## Task 6 — Real image workflow robustness and accessibility

**Files**

- Modify real image selection/report views and state.
- Modify: `apps/desktop/src/styles/global.css`
- Modify: `apps/desktop/src/styles/tokens.css`
- Modify: `apps/desktop/package.json`
- Modify lockfile selected by the repository
- Add focused component tests.

**Interfaces**

- Preserves the SDD-017 real-only command/report contract.
- Produces a truthful pending state, duplicate-submit guard, and monotonic
  stale-response guard.

- [ ] Write failing duplicate-submit, stale-response, runtime-loss, keyboard,
  error, preference, and pending-status tests.
- [ ] Keep image selection and report navigation keyboard operable.
- [ ] Expose a named busy status without invented percentage or scan phase.
- [ ] Retain only preferences with tested immediate effect.
- [ ] Keep primary controls reachable at the 1100 px desktop minimum.
- [ ] Run zero-warning lint, typecheck, tests, build, and production bundle
  inspection.

## Task 7 — SDD, CI, traceability, and evidence

**Current state (2026-07-29):** The documents, ADRs, matrices, risk records,
workflow/validator definitions, and bounded security-review record exist.
This is documentation/implementation completion, not final acceptance. The
documentation and safety validators passed after the parallel code changes
settled; remote CI remains a Task 8 gate.

**Files**

- Create the required `docs/specs/000` through `015` set.
- Create applicable ADRs under `docs/adr/`.
- Create: `docs/traceability-matrix.md`
- Create: `docs/risk-register.md`
- Create/update: `README.md`, `README.pt-BR.md`, `SECURITY.md`,
  `CONTRIBUTING.md`
- Create: `.github/workflows/quality.yml`
- Create/update evidence under `docs/evidence/`.
- Update: `docs/ui-data-contract.md`

**Interfaces**

- Defines requirement status vocabulary:
  `Not started`, `Partial`, `Implemented-unverified`, `Verified`.
- Maps each delivered requirement to design, code, tests, and evidence.

- [x] Record the S0 discovery inventory and baseline failures.
- [x] Create concise normative specs that explicitly distinguish implemented,
  partial, and excluded behavior.
- [x] Record architecture/security decisions without weakening the master spec,
  including the audited `io-windows` boundary and residual pathname TOCTOU.
- [x] Add deterministic PR quality-gate definitions that cannot access real
  devices. Their final local execution and GitHub run remain pending.
- [x] Record the sealed Codex Security snapshot with `34/34` unique receipts,
  three technically valid candidates decided `ignore`, and zero reportable
  findings without treating that result as proof of absence.
- [x] Update traceability and risk status to `Implemented-unverified` or lower
  for the currently visible implementation.
- [x] Re-run documentation and safety validators on the final locked tree and
  retain the exact results before Task 7 is treated as fully closed.

## Task 8 — Final verification and GitHub integration

**Current state (2026-07-29):** In progress. The frozen-code Rust/frontend
gates, final local validators, deterministic CLI process smoke, and an unsigned
local native-launch smoke passed. Packaged fixture acceptance, native visual
and assistive-technology review, remote CI, signing, clean-machine release
acceptance, and the unresolved Norton classification remain open.

**Files**

- No new behavior files; update evidence documents with final command results.

- [x] Run `cargo fmt --all -- --check`.
- [x] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Run `cargo test --workspace`.
- [x] Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, and `pnpm build` in
  `apps/desktop`.
- [x] Run the documentation, CI-safety, and real-only validators on the same
  final revision.
- [x] Re-run the image-only CLI smoke against deterministic fixtures.
- [ ] Re-capture the desktop flow in the in-app browser, inspect screenshots,
  test 1440 × 900 and 1100 × 700, and check console errors.
- [x] Review `git diff`, confirm no secrets/generated caches/real-disk artifacts,
  and update evidence.
- [x] Record the final development artifact hash and Authenticode state;
  do not redistribute the unsigned build or classify the Norton event without
  vendor/security evidence.
- [x] Preserve the GitHub `main` commit through a non-force integration, commit
  the verified tree, push, and report the exact branch/commit.
