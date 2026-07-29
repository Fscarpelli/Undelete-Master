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
- **Acceptance / tests / implementation:** AC-002 plus
  `JUST-NFR-001-BROKER-PENDING`; `crates/core`, `crates/io-common`.
- **Status:** `Partial`.

## NFR-002 — Bounded hostile-input parsing

- **Rationale / priority / source:** Images and filesystems are untrusted; Must;
  master §§8.3, 21 and 26.
- **Preconditions / behavior / errors:** All offsets, lengths, loops, slices,
  and allocations use checked bounds. A GPT copy is accepted only after its
  header and entry table validate together at a canonical location with valid
  reciprocal metadata. Malformed `$MFT` logical sizes and record-0 attributes
  fail closed; malformed candidate attributes, unresolved `$ATTRIBUTE_LIST`
  dependency, and bounded NTFS/FAT directory work expose incompleteness instead
  of claiming exhaustive enumeration. A declared FAT table larger than 64 MiB
  is rejected before table allocation. Malformed data returns a typed error or
  bounded warning.
- **Security / observability:** No panic, out-of-region read, or unbounded work;
  errors identify the parser stage without dumping source bytes.
- **Acceptance / tests / implementation:** `PART-GPT-OVERFLOW-001`,
  `PART-GPT-RANGE-001`, `PART-GPT-BACKUP-001`,
  `PART-GPT-BACKUP-002`, `PART-GPT-BACKUP-003`,
  `PART-GPT-BACKUP-004`, `PART-GPT-BACKUP-005`,
  `PART-GPT-PROTECTIVE-001`,
  `FS-MAP-TRUNC-001`, `NTFS-BITMAP-BOUND-004`,
  `NTFS-MFT-BOUND-003`, `NTFS-MFT-SIZE-001`,
  `NTFS-RECORD-SIGNATURE-001`, `NTFS-ATTRIBUTE-BOUNDS-001`,
  `NTFS-ATTRIBUTE-LIST-PARTIAL-001`, `NTFS-INIT-TAIL-001`,
  `FAT-COMPLETENESS-001` through
  `FAT-COMPLETENESS-004`, `FAT-TABLE-BOUND-001`,
  `FAT-DEPTH-BOUND-001`, and `FAT-DIRECTORY-CHAIN-BOUND-001`; parser crates.
- **Status:** `Partial`.

## NFR-003 — Deterministic recovery evidence

- **Rationale / priority / source:** Recovery claims must be reproducible; Must;
  master §§26.2 and 30.
- **Preconditions / behavior / errors:** Synthetic fixtures have stable truth
  manifests and expected SHA-256; mismatches fail tests.
- **Security / observability:** Fixtures never address real devices; CI retains
  only non-sensitive logs.
- **Acceptance / tests / implementation:** deterministic NTFS/FAT tests plus
  `JUST-NFR-003-EXTERNAL-EVIDENCE`; `crates/fixture-builder`.
- **Status:** `Partial`.

## NFR-004 — Local privacy

- **Rationale / priority / source:** Names, paths, hashes, and recovered content
  are sensitive; Must; FR-100/101 and master §21.5.
- **Preconditions / behavior / errors:** Core workflows require no network and
  redact paths from default diagnostics.
- **Security / observability:** No telemetry, uploads, keys, or file content in
  normal logs.
- **Acceptance / tests / implementation:** AC-026,
  `CLI-JSON-PRIVACY-001`, `CLI-PROCESS-ERROR-PRIVACY-001`,
  `CLI-PROBE-ERROR-PRIVACY-001`, and
  `DESKTOP-VOLUME-SCAN-ERROR-001`; future logging/session layers.
- **Status:** `Partial`.

## NFR-005 — Accessibility

- **Rationale / priority / source:** Primary workflows must be operable without
  a mouse; High; master §§19.9, 27 and AC-022.
- **Preconditions / behavior / errors:** WCAG 2.2 AA where applicable, keyboard
  operation, visible focus, named progress, reduced motion, and 200% zoom.
- **Security / observability:** Risk actions remain explicit and labeled.
- **Acceptance / tests / implementation:** focused keyboard, heading-focus,
  forced-colors, and effective reduced-motion coverage is implemented in
  `apps/desktop/src/App.tsx`, `apps/desktop/src/styles/global.css`, and
  component/CSS
  tests `DESKTOP-NAVIGATION-FOCUS-001`,
  `DESKTOP-FORCED-COLORS-FOCUS-001`, and
  `DESKTOP-EFFECTIVE-PREFS-001`, plus the named focusable table-region test
  `DESKTOP-VOLUME-TABLE-A11Y-001`. Automated accessibility, 200% scaling,
  assistive-technology, and real-shell acceptance remain covered by
  `JUST-NFR-005-UNVERSIONED-UI`.
- **Status:** `Partial`.

## NFR-006 — Scale and cancellation

- **Rationale / priority / source:** Scans may produce millions of candidates;
  High; master §§20 and 26.7.
- **Preconditions / behavior / errors:** Bounded queues, pagination, cooperative
  cancellation, and measured budgets; no unbounded frontend dataset.
- **Security / observability:** Resource limits prevent denial of service;
  throughput and cancellation are measured.
- **Acceptance / tests / implementation:** million-row, memory, and
  cancellation benchmarks remain covered by `JUST-NFR-006-PERFORMANCE`.
- **Status:** `Not started`.

## NFR-007 — Reproducible quality and supply chain

- **Rationale / priority / source:** Unsupported completion claims are unsafe;
  Must; master §§21.3, 28 and 30.
- **Preconditions / behavior / errors:** Lockfiles, warnings-as-errors, tests,
  dependency/license scans, and release evidence gate delivery.
- **Security / observability:** CI uses no real disks or release secrets on pull
  requests.
- **Acceptance / tests / implementation:** `CI-SAFETY-INLINE-001`,
  `CI-SAFETY-PACKAGE-LIFECYCLE-001`, `CI-SAFETY-SHEBANG-001`,
  `CI-SAFETY-INVOKED-HELPER-001`, `CI-SAFETY-INVOKED-HELPER-002`,
  `CI-SAFETY-INVOKED-HELPER-003`, `CI-SAFETY-INVOKED-HELPER-004`,
  `CI-SAFETY-INVOKED-HELPER-005`, `CI-SAFETY-INVOKED-HELPER-006`,
  `CI-SAFETY-INVOKED-HELPER-007`, `CI-SAFETY-INVOKED-HELPER-008`,
  `CI-SAFETY-INVOKED-HELPER-009`, `CI-SAFETY-INVOKED-HELPER-010`,
  `CI-SAFETY-INVOKED-HELPER-011`, `CI-SAFETY-INVOKED-HELPER-012`,
  `CI-SAFETY-WORKFLOWS-001`,
  `DOCS-REQUIREMENT-SETS-001`, `DOCS-STATUS-SYNC-001`,
  `DOCS-ACTUAL-TEST-001`, and `DOCS-ACTUAL-TEST-003`;
  `.github/workflows/quality.yml` and `.github/scripts`.
- **Status:** `Partial`.

## NFR-008 — Honest compatibility

- **Rationale / priority / source:** Platform and filesystem labels must reflect
  tested behavior; Must; master §§5 and 30.6.
- **Preconditions / behavior / errors:** Unsupported layouts and formats fail
  clearly or use explicitly limited RAW mode.
- **Security / observability:** No heuristic raw writes or silent fallback.
- **Acceptance / tests / implementation:** focused honest-state coverage
  includes `FAT-COMPLETENESS-001` through `FAT-COMPLETENESS-004`,
  `FAT-TABLE-BOUND-001`, `FAT-DEPTH-BOUND-001`,
  `FAT-DIRECTORY-CHAIN-BOUND-001`, `CLI-IMAGE-FAT-PARTIAL-001`,
  `NTFS-MFT-SIZE-001`, `NTFS-ATTRIBUTE-BOUNDS-001`,
  `NTFS-ATTRIBUTE-LIST-PARTIAL-001`, and
  `PART-GPT-BACKUP-002` through `PART-GPT-BACKUP-005`; the remaining
  Windows/version/filesystem matrix is covered by
  `JUST-NFR-008-SUPPORT-MATRIX`.
- **Status:** `Partial`.
