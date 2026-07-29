# SDD-002 — Non-functional Requirements

Status: Normative

Each record uses the requirement fields mandated by master spec §23.2.

## NFR-001 — Source preservation

- **Rationale / priority / source:** Data loss is irreversible; Must; master
  §§1, 8, 21 and `AGENTS.md`.
- **Preconditions / behavior / errors:** For every scan source, expose read-only
  operations only; reject unsafe or out-of-range access without mutation.
- **Security / observability:** Audit effective read-only state without logging
  content or secrets.
- **Acceptance / tests / implementation / status:** AC-002 plus architecture and
  runtime tests; `crates/core`, `crates/io-common`; `Partial`.

## NFR-002 — Bounded hostile-input parsing

- **Rationale / priority / source:** Images and filesystems are untrusted; Must;
  master §§8.3, 21 and 26.
- **Preconditions / behavior / errors:** All offsets, lengths, loops, and slices
  use checked bounds; malformed data returns a typed error or bounded warning.
- **Security / observability:** No panic, out-of-region read, or unbounded work;
  errors identify the parser stage without dumping source bytes.
- **Acceptance / tests / implementation / status:** parser regression,
  property, and fuzz tests; parser crates; `Partial`.

## NFR-003 — Deterministic recovery evidence

- **Rationale / priority / source:** Recovery claims must be reproducible; Must;
  master §§26.2 and 30.
- **Preconditions / behavior / errors:** Synthetic fixtures have stable truth
  manifests and expected SHA-256; mismatches fail tests.
- **Security / observability:** Fixtures never address real devices; CI retains
  only non-sensitive logs.
- **Acceptance / tests / implementation / status:** NTFS/FAT integration tests;
  `crates/fixture-builder`; `Partial`.

## NFR-004 — Local privacy

- **Rationale / priority / source:** Names, paths, hashes, and recovered content
  are sensitive; Must; FR-100/101 and master §21.5.
- **Preconditions / behavior / errors:** Core workflows require no network and
  redact paths from default diagnostics.
- **Security / observability:** No telemetry, uploads, keys, or file content in
  normal logs.
- **Acceptance / tests / implementation / status:** AC-026 and redaction tests;
  future logging/session layers; `Partial`.

## NFR-005 — Accessibility

- **Rationale / priority / source:** Primary workflows must be operable without
  a mouse; High; master §§19.9, 27 and AC-022.
- **Preconditions / behavior / errors:** WCAG 2.2 AA where applicable, keyboard
  operation, visible focus, named progress, reduced motion, and 200% zoom.
- **Security / observability:** Risk actions remain explicit and labeled.
- **Acceptance / tests / implementation / status:** component, axe, keyboard,
  and real-shell E2E tests; `apps/desktop`; `Partial`.

## NFR-006 — Scale and cancellation

- **Rationale / priority / source:** Scans may produce millions of candidates;
  High; master §§20 and 26.7.
- **Preconditions / behavior / errors:** Bounded queues, pagination, cooperative
  cancellation, and measured budgets; no unbounded frontend dataset.
- **Security / observability:** Resource limits prevent denial of service;
  throughput and cancellation are measured.
- **Acceptance / tests / implementation / status:** million-row, memory, and
  cancellation benchmarks; `Not started`.

## NFR-007 — Reproducible quality and supply chain

- **Rationale / priority / source:** Unsupported completion claims are unsafe;
  Must; master §§21.3, 28 and 30.
- **Preconditions / behavior / errors:** Lockfiles, warnings-as-errors, tests,
  dependency/license scans, and release evidence gate delivery.
- **Security / observability:** CI uses no real disks or release secrets on pull
  requests.
- **Acceptance / tests / implementation / status:** `.github/workflows/quality.yml`;
  `Implemented-unverified`.

## NFR-008 — Honest compatibility

- **Rationale / priority / source:** Platform and filesystem labels must reflect
  tested behavior; Must; master §§5 and 30.6.
- **Preconditions / behavior / errors:** Unsupported layouts and formats fail
  clearly or use explicitly limited RAW mode.
- **Security / observability:** No heuristic raw writes or silent fallback.
- **Acceptance / tests / implementation / status:** Windows/version/filesystem
  matrix evidence; `Partial`.

