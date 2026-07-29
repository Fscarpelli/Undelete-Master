# Foundation Hardening and Safe Image CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` or `superpowers:executing-plans` to
> implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Remove the known parser correctness hazards, add a real read-only
image scan entry point, make the desktop demonstration truthful and more
accessible, and establish repeatable SDD/CI evidence.

**Architecture:** Parser fixes remain inside their owning crates. A new
image-only CLI composes the existing read-only reader, partition discovery, and
NTFS/FAT scanners without adding privilege or write behavior. The frontend
retains its current design system while provider runtime metadata controls
truthful demo/verification states.

**Tech Stack:** Rust 1.88, Cargo workspace, React 18, TypeScript 5.7, Vite 6,
Vitest, Testing Library, ESLint, GitHub Actions.

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
- Update `docs/traceability-matrix.md` for every behavior change.

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

## Task 5 — Truthful desktop runtime state

**Files**

- Modify: `apps/desktop/src/api/provider.ts`
- Modify: `apps/desktop/src/api/mock.ts`
- Modify: `apps/desktop/src/api/index.ts`
- Modify: `apps/desktop/src/App.tsx`
- Modify: `apps/desktop/src/state/AppContext.tsx`
- Modify: `apps/desktop/src/views/ResultsView.tsx`
- Modify: `apps/desktop/src/views/RestoreView.tsx`
- Modify: `apps/desktop/src/i18n/pt-BR.ts`
- Modify: `apps/desktop/src/i18n/en-US.ts`
- Add focused component tests under `apps/desktop/src/`.

**Interfaces**

- Produces: `DataProvider.runtime: ProviderRuntime`
- Produces:
  `ProviderRuntime { mode: "demo" | "tauri"; readOnlyVerified: boolean }`

- [ ] Write failing tests for the demo banner, absent verified seal, and
  no-session Results/Restore behavior.
- [ ] Verify provider queries are not invoked without an active session.
- [ ] Add runtime metadata and localized explicit demo state.
- [ ] Remove implicit `sess-0001` fallbacks and add empty-state actions.
- [ ] Run focused Vitest suites.

## Task 6 — Keyboard, status, layout, and lint gates

**Files**

- Modify: `apps/desktop/src/views/ResultsView.tsx`
- Modify: `apps/desktop/src/views/LiveScanView.tsx`
- Modify: `apps/desktop/src/views/RestoreView.tsx`
- Modify: `apps/desktop/src/styles/global.css`
- Modify: `apps/desktop/src/styles/tokens.css`
- Modify: `apps/desktop/package.json`
- Modify lockfile selected by the repository
- Create: `apps/desktop/eslint.config.js`
- Add focused component tests.

**Interfaces**

- Preserves current visible information architecture and data contracts.
- Produces keyboard-operable sort/row actions and named progress semantics.

- [ ] Write failing keyboard and progress-role tests.
- [ ] Convert sort actions to native buttons and add row keyboard activation.
- [ ] Add accessible live/progress semantics and AA small-text token values.
- [ ] Keep primary controls reachable at the 1100 px desktop minimum.
- [ ] Add a zero-warning `pnpm lint` command and run frontend focused checks.

## Task 7 — SDD, CI, traceability, and evidence

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

- [ ] Record the S0 discovery inventory and baseline failures.
- [ ] Create concise normative specs that explicitly distinguish implemented,
  partial, and excluded behavior.
- [ ] Record architecture/security decisions without weakening the master spec.
- [ ] Add deterministic PR quality gates that cannot access real devices.
- [ ] Update traceability and risk status after implementation verification.

## Task 8 — Final verification and GitHub integration

**Files**

- No new behavior files; update evidence documents with final command results.

- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Run `cargo test --workspace`.
- [ ] Run `pnpm lint`, `pnpm typecheck`, `pnpm test`, and `pnpm build` in
  `apps/desktop`.
- [ ] Re-run the image-only CLI smoke against deterministic fixtures.
- [ ] Re-capture the desktop flow in the in-app browser, inspect screenshots,
  test 1440 × 900 and 1100 × 700, and check console errors.
- [ ] Review `git diff`, confirm no secrets/generated caches/real-disk artifacts,
  and update evidence.
- [ ] Preserve the GitHub `main` commit through a non-force integration, commit
  the verified tree, push, and report the exact branch/commit.

