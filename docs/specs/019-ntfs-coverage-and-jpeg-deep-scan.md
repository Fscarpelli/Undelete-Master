# SDD-019 — NTFS Coverage and Bounded JPEG Deep Scan

Status: Active increment with partial implementation
Decision date: 2026-07-30

This increment corrects a real metadata-coverage defect and implements the
first bounded, allocation-gated content-carving slice. The desktop exposes that
slice as an explicit whole-volume NTFS mode. It remains a narrow JPEG discovery
capability, not a complete data-recovery product.

## 1. Problem statement and confirmed root cause

The NTFS scanner previously bounded enumeration with both a record cap and
`MAX_MFT_SCAN_BYTES = 64 MiB`. The effective record count was the minimum of
the trusted MFT prefix, the record cap, and `64 MiB / record_size`. With the
usual 1 KiB NTFS record, only the first 65,536 MFT records were examined.

That code-level defect is confirmed. On a large system volume, early MFT
records can all be active or reserved while deleted records exist later. A
zero-candidate result therefore described only the inspected prefix, not the
absence of recoverable files on the volume. Although the scanner marked such a
run partial and emitted a warning, the desktop did not expose quantitative
records/bytes coverage. The result was technically bounded but easy to
interpret as exhaustive.

The exact reason for the reported zero-candidate `C:` run is not proven because
that run did not leave a retained report with its coverage and warnings. Other
possible causes remain:

- a file still present in the Recycle Bin has an active MFT record;
- a deleted record was reused, zeroed, torn, unnamed, or beyond another
  explicit work/evidence bound;
- file content remains but its metadata no longer does, which requires
  carving;
- SSD TRIM, overwrite, secure erase, encryption without keys, or an unreadable
  region made the original bytes unavailable.

This distinction is normative: the confirmed 64 MiB prefix defect is a credible
cause of false-negative metadata coverage, but it is not retrospective proof of
what existed on the user's live volume.

## 2. Outcome and non-goals

### In scope

- enumerate the trusted NTFS MFT beyond the former 64 MiB prefix;
- read MFT data in bounded batches instead of one ordinary record per broker
  round trip;
- retain a compact subset needed for deleted-candidate and namespace work;
- expose structured MFT coverage from the filesystem scanner;
- provide a read-only, bounded JPEG carving engine over caller-supplied
  regions;
- integrate an explicit `metadata`/`deepJpeg` choice while keeping metadata as
  the safe default;
- admit whole-volume NTFS deep scan only over ranges proven free by trustworthy
  allocation evidence;
- preserve physical offset, size, format validation result, SHA-256, validator
  version, discovery method, structured coverage, and bounded merge evidence;
- state physical and product limits without recovery guarantees.

### Explicitly not completed by this increment

- a `scan-image` command-line flag for deep scan; that process command remains
  metadata-only even though the CLI library has the integrated mode;
- deep scan for a selected folder;
- RAW scan of allocated regions or an entire damaged/unknown filesystem;
- fragmented-JPEG reconstruction;
- generalized metadata/carving merge beyond exact unambiguous physical-range
  corroboration;
- persistent sessions, pause, resume, cooperative cancellation, live
  throughput, percentage, or ETA;
- extraction, restore, preview, repair, or execution of recovered content;
- PNG, PDF, ZIP, Office, audio, video, executable, or arbitrary-format carving;
- reversal of TRIM, overwrite, secure erase, BitLocker, EFS, or physical media
  damage.

## 3. Architecture and trust boundaries

The decision is recorded in
[ADR-0024](../adr/0024-streaming-mft-and-bounded-content-carving.md).

### 3.1 Metadata quick scan

The NTFS engine:

1. validates record 0 and the unnamed `$MFT::$DATA` stream;
2. computes declared, trusted available, and work-bounded record counts with
   checked arithmetic;
3. reads at most a bounded batch at a time;
4. parses records inside that batch without retaining the batch afterward;
5. retains only records needed for deleted candidates, deleted-stream
   extension resolution, and directory namespace reconstruction, with four
   separate ceilings: 100,000 deleted entries, 100,000 directory entries,
   100,000 extension references, and 100,000 extension streams merged into
   base records; retained nested evidence is additionally bounded across those
   records to 400,000 names, 200,000 streams, and 1,000,000 run elements;
6. falls back to bounded per-record reads if a batch read fails, so one bad
   range does not hide an otherwise readable batch;
7. marks coverage partial for any work or retention cap, short
   initialized/physical prefix, unreadable or malformed record, unresolved
   attribute dependency, or namespace saturation.

The work ceiling remains a safety control, not an exhaustion claim. Reaching it
must be structured partial evidence.

### 3.2 Content carving component

`crates/carving` consumes only `SourceReader` and an explicit list of bounded
regions. It never opens a path or device, discovers Windows volumes, chooses a
scan scope, writes extracted content, or generates a preview.

The component can reject regions that exceed its configured count/byte limits.
It reads one bounded discovery buffer and one bounded incremental validation
buffer rather than allocating a maximum-candidate-sized buffer. It preserves
the minimal overlap needed to detect a JPEG SOI signature across a chunk
boundary and charges every validation read to aggregate signature-attempt and
validation-byte budgets. A candidate is emitted only after bounded structural
parsing confirms:

- SOI;
- legal bounded marker/segment lengths before scan data;
- a non-zero SOF dimension record;
- SOS;
- entropy-coded byte stuffing and restart-marker handling;
- EOI within the configured maximum candidate size.

This is a contiguous JPEG carver. It does not infer generic fragmentation and
does not equate a valid marker envelope with perfect pixel decode.

### 3.3 Product admission policy

The engine's ability to accept an explicit region is not authority to scan that
region in the product. The implemented CLI-library/Tauri integrator enforces
all of the following:

- the user selected the whole mounted NTFS volume, not a folder;
- the default/legacy entry point remains `MetadataOnly`; `DeepJpeg` is explicit;
- the allocation authority is the active, base, non-directory root
  `$Bitmap` record with exactly one unnamed data stream, no attribute list,
  compressed/encrypted/sparse flags, nonzero starting VCN, or incoherent
  initialized/logical/allocated sizes;
- every submitted byte is classified `FreeInSnapshot`;
- unknown, allocated, conflicting, out-of-volume, sparse, resident, failed, or
  unproven ranges are excluded;
- a missing, truncated, capped, inconsistent, or unreadable `$Bitmap` disables
  carving for the affected range and marks the deep scan partial;
- the product never silently expands an unallocated scan into a whole-volume
  RAW scan.

The current product profile uses 1 MiB chunks, a 128 MiB maximum contiguous
candidate, 10,000 output candidates, 65,536 coalesced free regions, 16 TiB of
submitted free space, 10,000,000 signature-validation attempts, and 8 GiB of
aggregate validation reads. Reaching a budget sets a structured flag and
partial status; it does not silently continue or claim exhaustive coverage.

A carved candidate has no reliable historical folder ancestry. Folder-scoped
carving is therefore forbidden. A future explicit damaged/RAW whole-region mode
requires another accepted increment with separate UX, limits, provenance, and
false-positive controls.

## 4. Structured coverage contract

The NTFS engine records at least:

| Field | Meaning |
| --- | --- |
| `recordsDeclared` | Whole records implied by trusted logical `$MFT` size. |
| `recordsAvailable` | Whole records inside the initialized, source-bounded physical prefix. |
| `recordsExamined` | Records attempted within the explicit work ceiling. |
| `bytesDeclared` | Trusted logical `$MFT` byte count. |
| `bytesAvailable` | Whole-record bytes available in the trusted physical prefix. |
| `bytesExamined` | Whole-record bytes attempted within the work ceiling. |
| `isComplete` | True only when no known evidence or work boundary prevented complete metadata enumeration. |
| `partialReasons[]` | Bounded machine-readable reasons for incomplete coverage. |

The numeric counters now propagate through CLI, Tauri and the typed desktop
contract; the UI shows examined versus declared MFT records and uses separate
complete/partial zero-result messages. Existing `is_complete` and bounded prose
warnings still carry the limiting context. Machine-readable partial reasons and
a complete user-facing byte/reason breakdown remain incomplete.

An explicit deep scan additionally reports:

| Field | Meaning |
| --- | --- |
| `bytesRequested` / `bytesScanned` | Proven-free bytes submitted and uniquely visited by signature discovery. |
| `signaturesAttempted` | SOI hits admitted to bounded structural validation. |
| `validationBytesRead` | Aggregate bytes read by incremental candidate validation. |
| `readErrorCount` | Bounded count of validation/discovery read failures. |
| `candidateLimitReached` / `candidateByteLimitHits` | Output or per-candidate work saturation. |
| `signatureAttemptLimitReached` / `validationByteLimitReached` | Hostile-fanout/aggregate-validation saturation. |
| `rejectedSignatures` / `truncatedSignatures` | Structurally rejected or region-ended signatures. |
| `regionsSubmitted` / `regionLimitReached` | Coalesced proven-free region admission and saturation. |
| `partial` | True when allocation evidence, reads, or any work/output budget prevents exhaustive coverage of the requested eligible evidence. |

The product presentation must distinguish:

- `0 candidates, complete metadata coverage`;
- `0 candidates in partial metadata coverage`;
- `0 metadata candidates; content carving not executed`;
- metadata candidates, carved candidates, and metadata candidates corroborated
  by JPEG validation without discarding the original discovery method.

For a partial zero result, the primary message is equivalent to:

> No candidates were found in the coverage examined. This scan did not prove
> that the volume contains no recoverable data.

The current UI shows examined versus declared MFT records and
scanned/requested JPEG bytes plus deep-limit flags. A complete MFT
byte/reason presentation remains pending. It must never describe a
metadata-only scan as reading the entire volume in RAW mode.

## 5. Carved evidence contract

Each accepted JPEG carving result must carry:

- `DiscoveryMethod::Carving`;
- a deterministic candidate identity within the scan;
- generated name and low metadata confidence, never a fabricated original
  name/path/date;
- absolute physical offset and bounded length;
- the submitted source-region availability/provenance;
- structural status and validator version;
- SHA-256 of the exact accepted byte range;
- warnings for any bounded limitation;
- no preview, decoded pixels, executable content, destination path, or source
  mutation capability.

The current `CarveReport` links each candidate ID to `CarveEvidence` carrying
its physical range, exact-range SHA-256, and `jpeg-structural-v1` validator
version. The Tauri adapter validates that link and exposes the lowercase
SHA-256 plus validator string in candidate-page schema 2; no recovered bytes
enter JavaScript.

When exactly one NTFS metadata candidate owns the same exact contiguous
physical range, the integration retains that metadata candidate and attaches
the JPEG evidence to it. Multiple metadata records for the same range are
ambiguous: the carved candidate is retained separately with a warning rather
than assigning the hash to an arbitrary record. Repeated identical carves of
one range coalesce only when range, SHA-256, and validator agree. This narrow
corroboration does not prove an original path, semantic image correctness, or
generalized evidence merging.

## 6. Requirements

### SDD-REC-001 — Remove the legacy MFT prefix defect

- **Rationale:** a fixed 64 MiB prefix can hide deleted records on an ordinary
  large NTFS volume and make a zero result misleading.
- **Priority:** Must.
- **Source:** FR-030; reported zero-candidate system-volume scan; risk R-026.
- **Preconditions:** record 0 and the unnamed MFT stream are structurally
  trusted and record-aligned.
- **Behavior:** enumerate the trusted initialized MFT up to the explicit record
  work ceiling without applying the former 64 MiB scan-byte ceiling.
- **Error behavior:** any ceiling or short/unreadable trusted region yields
  partial coverage; it never yields an exhaustive zero claim.
- **Security implications:** all reads remain range-checked and read-only.
- **Observability:** declared, available, and examined record/byte counts are
  retained by the NTFS scanner.
- **Acceptance criteria:** a deterministic deleted record placed beyond the
  former 64 MiB boundary is found byte-exact, and its content matches the truth
  manifest SHA-256.
- **Test IDs:** `NTFS-MFT-COVERAGE-001`.
- **Implementation links:** `crates/fs-ntfs/src/scan.rs`,
  `crates/fs-ntfs/tests/ntfs_recovery.rs`,
  `crates/fixture-builder/src/ntfs.rs`.
- **Status:** Implemented-unverified.

### SDD-REC-002 — Batch MFT reads and compact retention

- **Rationale:** removing a prefix cap must not replace false negatives with
  millions of IPC calls or unbounded memory.
- **Priority:** Must.
- **Source:** FR-030; NFR-002; ADR-0024.
- **Preconditions:** the trusted MFT runlist and record size are available.
- **Behavior:** process bounded aligned batches, discard each batch after
  parsing, and retain only deleted-candidate, required extension, and directory
  namespace evidence rather than every active regular file. Bound the four
  retained classes independently at 100,000 deleted entries, directory
  entries, extension references, and merged extension streams. Across retained
  base and extension evidence, also bound nested names to 400,000, streams to
  200,000, and run elements to 1,000,000. Admit or reject an over-budget base
  record/extension merge as a unit so partial evidence is not half-accounted.
- **Error behavior:** a failed batch is isolated by bounded per-record reads;
  unreadable records or any retention ceiling remain partial evidence and
  warning output is capped.
- **Security implications:** batch offset/length/slice arithmetic is checked;
  no batch can exceed the scanner/broker limit.
- **Observability:** coverage counts describe attempted records independently
  of candidate count; capped work produces an explicit warning.
- **Acceptance criteria:** the beyond-64-MiB fixture succeeds without allocating
  the whole MFT in the scanner, while existing corruption/namespace regressions
  remain green.
- **Test IDs:** `NTFS-MFT-COVERAGE-001`, `NTFS-MFT-BATCH-001`,
  `NTFS-MFT-BOUND-003`, `NTFS-MFT-RETENTION-004`,
  `NTFS-MFT-RETENTION-005`, `NTFS-MFT-RETENTION-006`,
  `NTFS-NAMESPACE-WORK-BOUND-001`.
- **Implementation links:** `crates/fs-ntfs/src/scan.rs`.
- **Status:** Implemented-unverified.

### SDD-REC-003 — Propagate quantitative coverage to the user

- **Rationale:** `partial` plus a prose warning is insufficient to interpret a
  zero-candidate result.
- **Priority:** Must.
- **Source:** FR-040; R-019; R-026.
- **Preconditions:** an NTFS metadata scan returned or stopped with bounded
  evidence.
- **Behavior:** carry numeric coverage and bounded reasons/flags through CLI,
  Tauri, TypeScript, and UI; separate metadata coverage from content-carving
  coverage.
- **Error behavior:** schema mismatch fails closed; missing coverage cannot be
  displayed as complete.
- **Security implications:** coverage contains counts/reasons only, never
  recovered names, content, native paths, or device selectors.
- **Observability:** the UI and report show examined/declared MFT counts,
  requested/scanned JPEG bytes, validation work, and deep-limit flags next to
  candidate totals. Machine-readable MFT partial reasons remain pending.
- **Acceptance criteria:** a partial zero fixture is rendered as observed
  non-exhaustive coverage, while a complete metadata zero is separately
  identifiable and still does not deny other recovery techniques.
- **Test IDs:** `CLI-JSON-PRIVACY-001`, `DESKTOP-MFT-COVERAGE-001`,
  `WIN-MFT-COVERAGE-001`, `WIN-ZERO-PARTIAL-001`,
  `WIN-ZERO-COMPLETE-001`, `DESKTOP-DEEP-EVIDENCE-001`,
  `WIN-DEEP-PROVENANCE-001`, `WIN-DEEP-ZERO-001`.
- **Implementation links:** `crates/fs-ntfs/src/scan.rs`,
  `crates/cli/src/report.rs`, `crates/cli/src/lib.rs`,
  `apps/desktop/src-tauri/src/storage.rs`,
  `apps/desktop/src/api/storage.ts`,
  `apps/desktop/src/views/AnalysisView.tsx`.
- **Status:** Partial.

### SDD-REC-004 — Bounded read-only JPEG carver component

- **Rationale:** metadata can disappear while contiguous JPEG bytes remain.
- **Priority:** Must.
- **Source:** FR-031; FR-051; AC-010 and AC-011.
- **Preconditions:** the caller supplies nonempty, source-bounded regions and
  explicit resource limits.
- **Behavior:** detect JPEG across chunk boundaries, validate incrementally
  with fixed-size bounded buffers, and emit only structurally accepted
  contiguous candidates within region, scan-byte, chunk, candidate,
  candidate-count, signature-attempt, and aggregate-validation-byte budgets.
- **Error behavior:** malformed/truncated input is rejected, read failure is
  bounded, and limit exhaustion is explicit; no partial bytes are written.
- **Security implications:** the component consumes only `SourceReader`,
  forbids unsafe code, writes nothing, performs no preview/decode, and opens no
  path/device/network resource.
- **Observability:** candidates include method, physical range, structural
  status, and warnings; linked evidence includes exact-range SHA-256 and
  validator version; coverage includes signature/validation counters and
  saturation flags.
- **Acceptance criteria:** deterministic fixtures cover split signatures,
  marker validation, truncation, region bounds, candidate size/count limits,
  partial reads, and SHA-256 provenance.
- **Test IDs:** `CARVE-JPEG-BOUNDARY-001`,
  `CARVE-JPEG-VALIDATION-001` (including truncated/no-EOI evidence),
  `CARVE-REGION-BOUNDS-001`, `CARVE-LIMIT-CANDIDATE-BYTES-001`,
  `CARVE-LIMIT-CANDIDATE-COUNT-001`, `CARVE-READ-PARTIAL-001`,
  `CARVE-SIGNATURE-BUDGET-002`, `CARVE-VALIDATION-BUDGET-003`,
  `CARVE-JPEG-TEM-004`, `CARVE-JPEG-INCREMENTAL-005`, and
  `CARVE-PROVENANCE-SHA256-001`.
- **Implementation links:** `crates/carving`,
  `crates/fixture-builder/src/carving.rs`.
- **Status:** Implemented-unverified.

### SDD-REC-005 — Admit carving only for trusted free whole-volume ranges

- **Rationale:** scanning allocated data returns active files/thumbnails and
  false “undelete” candidates; carved bytes cannot prove selected-folder
  ancestry.
- **Priority:** Must.
- **Source:** FR-031; FR-034; ADR-0024; risk R-027.
- **Preconditions:** the user selected a whole NTFS volume and its allocation
  evidence is trustworthy for each proposed range.
- **Behavior:** the integrator submits only `FreeInSnapshot` regions; folder
  scope, unknown allocation, partial bitmap suffixes, and silent RAW expansion
  are rejected.
- **Error behavior:** omit the affected range, mark carving partial/not run,
  and explain why; never broaden authority.
- **Security implications:** scope selection remains native and opaque; the
  engine cannot grant itself source authority. `$Bitmap` authority additionally
  requires the active/base/non-directory root identity and one coherent,
  unflagged unnamed data stream.
- **Observability:** report requested scope, eligible/free bytes, examined
  bytes, skipped bytes by reason, and whether carving ran.
- **Acceptance criteria:** an allocated JPEG is excluded, a free-range JPEG is
  eligible, an unknown range is skipped, and a folder-scoped request cannot
  invoke carving.
- **Test IDs:** `NTFS-ALLOCATION-SNAPSHOT-001`,
  `NTFS-BITMAP-AUTHORITY-002`, `CLI-DEEP-JPEG-001`,
  `CLI-DEEP-FREE-ONLY-003`, `CLI-DEEP-UNKNOWN-004`,
  `CLI-DEEP-FAT-008`, `CLI-DEEP-REGION-COALESCE-009`,
  `CLI-DEEP-LIMIT-010`, `DESKTOP-DEEP-MODE-001`,
  `WIN-DEEP-MODE-001`, `WIN-DEEP-MODE-002`, `WIN-DEEP-MODE-003`.
- **Implementation links:** `crates/fs-ntfs/src/scan.rs`,
  `crates/fs-ntfs/tests/ntfs_internal_bounds.rs`,
  `crates/cli/src/deep.rs`, `crates/cli/tests/deep_volume_scan.rs`,
  `apps/desktop/src-tauri/src/storage.rs`,
  `apps/desktop/src/state/storageScan.ts`,
  `apps/desktop/src/views/AnalysisView.tsx`.
- **Status:** Partial.

### SDD-REC-006 — Preserve carving provenance without fabricating metadata

- **Rationale:** a carved header does not reveal the original name, path, date,
  or filesystem record.
- **Priority:** Must.
- **Source:** FR-051; FR-054; FR-055.
- **Preconditions:** the bounded carver accepted a JPEG byte range.
- **Behavior:** preserve method, physical range, structural status, generated
  name, low metadata confidence, exact-range SHA-256, and validator version;
  never invent historical metadata. Corroborate only one unambiguous metadata
  owner of the same exact contiguous range; retain an ambiguous carving
  separately. Duplicate carving evidence coalesces only when range, SHA-256,
  and validator agree.
- **Error behavior:** missing mandatory provenance rejects the result; hash
  failure cannot silently downgrade into an ordinary accepted candidate.
- **Security implications:** hash bytes stay local and recovered content does
  not cross into JavaScript.
- **Observability:** the component links candidate ID to physical range,
  SHA-256, and validator version. Tauri validates the linkage and candidate
  page schema 2 exposes method, lowercase SHA-256, and validator without
  exposing content bytes.
- **Acceptance criteria:** the candidate is explicitly `Carving` with uncertain
  generated metadata, and its linked evidence carries the truth-manifest
  SHA-256, physical range, and validator version.
- **Test IDs:** `CARVE-PROVENANCE-SHA256-001`,
  `CLI-DEEP-DEDUPE-005`, `CLI-DEEP-ID-006`,
  `CLI-DEEP-DEDUPE-011`, `CLI-DEEP-DEDUPE-012`,
  `DESKTOP-DEEP-EVIDENCE-001`, `DESKTOP-DEEP-EVIDENCE-002`,
  `WIN-DEEP-PROVENANCE-001`.
- **Implementation links:** `crates/carving`, `crates/cli/src/deep.rs`,
  `apps/desktop/src-tauri/src/storage.rs`,
  `apps/desktop/src/api/storage.ts`,
  `apps/desktop/src/views/AnalysisView.tsx`.
- **Status:** Implemented-unverified.

### SDD-REC-007 — Preserve the never-write and no-preview boundary

- **Rationale:** discovery must never destroy source evidence or parse hostile
  pixels in a privileged process.
- **Priority:** Must.
- **Source:** FR-003; FR-071; FR-072; ADR-0002.
- **Preconditions:** metadata or carving discovery runs.
- **Behavior:** use existing read-only `SourceReader`; expose no destination,
  write, trim, lock, dismount, repair, execution, decode, or preview operation.
- **Error behavior:** discovery stops or skips bounded ranges; it never writes
  a fallback artifact.
- **Security implications:** the elevated broker remains a byte reader only;
  carving stays unelevated and parser-only.
- **Observability:** static dependency/API review and tests show no write or
  preview surface.
- **Acceptance criteria:** the carving crate has no output path/device-open
  API, and deterministic tests operate only on synthetic sources.
- **Test IDs:** `CARVE-REGION-BOUNDS-001`, `CARVE-READ-PARTIAL-001`.
- **Implementation links:** `crates/carving`, `crates/core/src/source.rs`.
- **Status:** Implemented-unverified.

### SDD-REC-008 — State physical and cryptographic limits honestly

- **Rationale:** a stronger scanner still cannot reconstruct unavailable
  information.
- **Priority:** Must.
- **Source:** master specification §2.2; AC-012.
- **Preconditions:** every scan result, especially zero, partial, system-volume,
  SSD, encrypted, or carved output.
- **Behavior:** explain that overwrite, TRIM/garbage collection, secure erase,
  unreadable sectors, absent encryption keys, and unsupported fragmentation can
  prevent recovery; a live `C:` scan is not a snapshot.
- **Error behavior:** unavailable bytes remain unavailable/unknown; never
  synthesize original bytes or claim a successful decrypt.
- **Security implications:** do not collect BitLocker/EFS keys and do not
  execute or preview untrusted results.
- **Observability:** the report records the applicable limitation without
  claiming that zeros alone prove TRIM.
- **Acceptance criteria:** documentation and future UI never promise recovery
  of overwritten, trimmed, securely erased, EFS-encrypted, or physically
  unreadable content.
- **Test IDs or formal justification:** `WIN-DEEP-ZERO-001` and the localized
  deep-coverage text cover the initial JPEG slice; full physical/cryptographic
  acceptance remains absent.
- **Implementation links:** `docs/specs/015-known-limitations.md`,
  `apps/desktop/src/i18n/messages.ts`.
- **Status:** Partial.

### SDD-REC-009 — Deep-scan progress and cooperative cancellation

- **Rationale:** a sequential content scan of a large system volume can take
  hours and must not look hung or be impossible to stop safely.
- **Priority:** Must before production readiness.
- **Source:** FR-040; FR-041; R-015.
- **Preconditions:** a product command exposes deep scan.
- **Behavior:** report real examined/eligible bytes and phase, and honor
  cooperative cancellation at bounded intervals without fabricated timers.
- **Error behavior:** cancellation produces a truthful terminal/partial state
  and no duplicate or invented result.
- **Security implications:** cancellation cannot mutate or dismount the source.
- **Observability:** byte counters, elapsed time, cancellation latency, and
  terminal reason are testable.
- **Acceptance criteria:** the integrated bounded slice is not marked
  production-ready until a large synthetic scan reports real progress and
  cancels within its measured budget.
- **Test IDs or formal justification:** no cancellation/progress implementation
  or executable acceptance test exists.
- **Implementation links:** none.
- **Status:** Not started.

### SDD-REC-010 — Validate only with deterministic sources

- **Rationale:** fixing a real-volume symptom does not authorize destructive or
  unbounded tests on a real disk.
- **Priority:** Must.
- **Source:** ADR-0004; ADR-0005; repository invariants.
- **Preconditions:** metadata/carving regression or acceptance test.
- **Behavior:** use deterministic fixture-builder images and exact SHA-256 truth
  manifests; no ordinary test opens `C:`, `PhysicalDriveN`, or another real
  source.
- **Error behavior:** a test requiring real media is excluded from ordinary
  gates until an isolated, allowlisted disposable-VHD process exists.
- **Security implications:** no source writes, device controls, elevation, or
  recovered-content execution.
- **Observability:** fixture ID, expected range/hash, and bounded limit are
  asserted.
- **Acceptance criteria:** the former-prefix regression and JPEG component
  cases pass on synthetic inputs; external forensic corpora and live
  compatibility remain separate open evidence.
- **Test IDs:** `NTFS-MFT-COVERAGE-001`,
  `CARVE-PROVENANCE-SHA256-001`, `CARVE-REGION-BOUNDS-001`.
- **Implementation links:** `crates/fixture-builder/src/ntfs.rs`,
  `crates/fixture-builder/src/carving.rs`.
- **Status:** Implemented-unverified.

## 7. Capability/status matrix

| Capability | Current status | Evidence boundary |
| --- | --- | --- |
| Deleted NTFS metadata beyond the former 64 MiB prefix | Implemented-unverified | Synthetic regression and final same-revision local gates passed; exact commit, external-corpus and native evidence remain pending. |
| Bounded batched MFT reads and compact record retention | Implemented-unverified | One 1 MiB batch; four explicit 100,000-entry class ceilings; aggregate ceilings of 400,000 names, 200,000 streams and 1,000,000 run elements; large external-corpus/performance evidence remains residual. |
| Numeric MFT coverage inside `NtfsScanOutput` | Implemented-unverified | Core counters exist. |
| Numeric MFT coverage through CLI/Tauri/TypeScript/UI | Implemented-unverified | Focused contract/UI tests and final same-revision local gates passed; native acceptance remains. |
| Machine-readable partial reasons and full byte/reason presentation | Partial | Deep scan has structured counters/limit flags; MFT still relies on status plus bounded warnings for the specific reason. |
| Bounded contiguous JPEG carving over supplied regions | Implemented-unverified | Incremental fixed-buffer component, deterministic fixtures and final same-revision local gates passed; external corpus remains. |
| JPEG SOF/SOS/EOI marker validation, physical-range provenance, SHA-256 and validator version | Implemented-unverified | Linked `CarveEvidence` reaches validated candidate-page schema 2; no decode/preview. |
| Trusted NTFS `$Bitmap` snapshot and proven-free region iterator | Implemented-unverified | Authority checks active/base/root identity, stream uniqueness/flags/VCN/sizes; retained bitmap can still be partial/capped. |
| Whole-volume admission and submission of free regions to JPEG carving | Implemented-unverified | CLI library and Tauri adapter submit only coalesced `FreeInSnapshot` NTFS regions and fail closed for folder/non-NTFS/unknown authority. |
| Folder-scoped JPEG carving | Intentionally unsupported | Carved bytes do not prove folder ancestry. |
| Deep-scan CLI library/Tauri/UI | Implemented-unverified | Explicit `deepJpeg` whole-NTFS option, schema 3 coverage and schema 2 evidence; `scan-image` process command remains metadata-only. |
| Deep-scan progress/cancellation/checkpointing | Not started | UI is honestly indeterminate and states that percentage, ETA and cancellation are absent; required before production readiness. |
| Fragmented JPEG reconstruction | Not started | Contiguous carving only. |
| Exact-range metadata corroboration and carving deduplication | Implemented-unverified | Unique exact range may retain metadata identity; ambiguous ownership preserves a separate carving; generalized merge remains absent. |
| Extraction/restore | Implemented-unverified | SDD-020 adds real native extraction and transactional publication for candidates with usable content plans; governed real-media acceptance remains pending. |
| Preview/repair | Not started | No recovered-content preview, execution, or repair surface is exposed. |
| External-corpus and arbitrary real-volume compatibility | Not started | Synthetic component evidence must not be generalized. |

## 8. Clean-room reference note

The public
[`saintmarina/undelete_jpg`](https://github.com/saintmarina/undelete_jpg)
repository, licensed under the
[`MIT License`](https://github.com/saintmarina/undelete_jpg/blob/e134c2078d5b1198d020521b91893f01f1cd4049/LICENSE),
was reviewed as an old conceptual reference. It demonstrates a useful
two-stage idea: cheap SOI discovery followed by bounded marker parsing over
sequential read-only input.

No source from that project is incorporated or ported. The Rust implementation
is a clean-room design against this repository's own interfaces, safety
invariants, fixture truth, limits, and master specification. In particular, it
does not inherit the reference project's whole-device scan, current-directory
output writes, overwrite-prone filenames, 50 MiB assumption, POSIX APIs, or
limited validation behavior.

## 9. Release and claim boundary

Passing focused component/integration tests does not prove that scanning `C:`
will find every deleted file. The implemented UI may be described only as a
bounded whole-NTFS contiguous-JPEG deep mode. Before any production-ready or
general deep-recovery claim:

- SDD-REC-003 and SDD-REC-005 must close their remaining observability gaps,
  and SDD-REC-009 must be implemented and tested;
- the current CLI/Tauri/UI mode, coverage, evidence, and fail-closed scope
  contracts passed the final same-revision local gates recorded in
  [2026-07-30 recovery evidence](../evidence/recovery-hardening-2026-07-30.md),
  but still require exact-commit, native and remote evidence;
- long-running memory, throughput, cancellation, mutable-volume, and hostile
  input tests must pass;
- external forensic corpora must be evaluated with license/provenance records;
- the final committed revision and remote gates must pass.

Until then, the desktop path is an implemented-unverified, explicitly bounded
discovery slice. It is not evidence of fragmented recovery, broad format
coverage, extraction/restore, arbitrary real-media compatibility, or recovery
of overwritten/TRIM-discarded bytes.
