# ADR-0024 — Streaming MFT Enumeration and Bounded Content Carving

Status: Accepted
Decision date: 2026-07-30

Implementation status: `Partial`. The bounded MFT path, hardened NTFS
allocation authority, allocation-aware JPEG orchestration, schema 3 desktop
coverage, schema 2 candidate evidence, and explicit whole-NTFS UI mode are
implemented but not finally verified. Machine-readable MFT partial reasons,
real-time progress, cancellation, restore, broader formats, and fragmented
recovery remain absent.

## Context

A scan of a large mounted `C:` volume returned zero candidates. The actual run
was not persisted, so its exact deleted-record distribution cannot be
reconstructed. Source inspection nevertheless confirmed two product gaps:

1. NTFS enumeration was capped to the first 64 MiB of physically backed MFT
   records. At the ordinary 1 KiB record size, it inspected no more than 65,536
   records, even when the trusted MFT contained more.
2. The product had no content carving. `crates/carving` was only a future
   workspace path, so content without recoverable filesystem metadata could not
   be discovered.

Simply deleting every bound is unsafe. A large or hostile MFT can exhaust
memory or cause millions of one-record broker round trips. Scanning all bytes
for JPEG signatures also finds active files and embedded thumbnails, inflates
false positives, and can turn an unallocated deep scan into an undeclared RAW
whole-volume scan.

## Decision drivers

- preserve the never-write source invariant;
- make zero results measurable and honest;
- remove the ordinary 64 MiB false-negative prefix;
- keep parser memory and read sizes bounded;
- avoid one broker request per normal MFT record;
- do not retain every active regular file merely to find deleted records;
- restrict the first carving slice to structurally recognizable contiguous
  JPEG data;
- separate a read-only carving engine from product scope authority;
- never treat a folder path or carved bytes as historical ancestry;
- reject unknown/allocated ranges rather than silently expanding scope;
- keep recovered bytes out of privileged and UI processes;
- use deterministic synthetic fixtures with SHA-256 truth;
- leave progress, cancellation, restore, preview, and broader formats honestly
  pending.

## Options considered

| Option | Benefit | Defect | Decision |
| --- | --- | --- | --- |
| Keep the 64 MiB MFT prefix | Small bounded work | Systematically hides later deleted records and makes zero results misleading | Rejected |
| Remove all MFT limits and retain every parsed record | Maximum nominal coverage | Hostile/unusually large MFT can exhaust memory and time | Rejected |
| Read every MFT record individually | Simple failure isolation | Excessive IPC/syscall overhead on ordinary large volumes | Rejected |
| Read the whole MFT into memory | Sequential I/O | Unbounded allocation and long-lived sensitive data | Rejected |
| Bounded batches plus compact retained evidence | Sequential bounded I/O and broad ordinary coverage | More complex error isolation and multi-pass dependencies | Accepted |
| Carve every byte of a mounted volume automatically | Can find content without allocation metadata | Returns active/embedded content, increases false positives and cost, violates scope expectations | Rejected |
| Let the carving crate decide volume/folder/free authority | Convenient API | Couples portable parser code to Windows/filesystem policy and risks authority expansion | Rejected |
| Caller supplies explicit regions; product integrator admits only trusted free whole-volume ranges | Portable bounded component and fail-closed policy | Requires strict allocation-map and scope integration | Accepted |
| Port `undelete_jpg` C code | Fast reuse | POSIX/device/output assumptions, shallow validation, unsafe output policy, and needless third-party code incorporation | Rejected |
| Clean-room Rust JPEG component informed only by public concepts | Fits repository boundaries and tests | Requires independent implementation and validation | Accepted |

## Decision

### 1. MFT enumeration

1. Remove the 64 MiB byte prefix from ordinary MFT record enumeration.
2. Retain an explicit high record work ceiling. Reaching it is `partial`, never
   complete.
3. Compute every declared/available/examined record and byte count with checked
   arithmetic.
4. Read at most one bounded batch at a time. The current target is 1 MiB,
   compatible with the broker's maximum read payload; a reader may split it
   further without changing scanner semantics.
5. Parse records inside the batch and then release the batch.
6. Retain deleted base records, required deleted-stream extension evidence, and
   directories needed for namespace reconstruction. Discard active ordinary
   files after classification. Cap deleted entries, directory entries,
   extension references, and extension streams merged into base records
   independently at 100,000. Across the retained base/extension evidence, also
   cap names at 400,000, streams at 200,000, and run elements at 1,000,000.
   An over-budget base record or extension merge is rejected atomically and
   saturation is partial.
7. If a batch read fails, retry only its bounded records individually to
   isolate readable evidence. Cap warnings and mark unreadable records partial.
8. Preserve every existing corruption, attribute-dependency, trusted-prefix,
   namespace, and work-bound reason for incompleteness.

### 2. Coverage

The NTFS component exposes declared, physically/trustfully available, and
examined records/bytes. Those counters now pass through CLI, Tauri, TypeScript,
and UI with decimal-string protection at the JavaScript boundary. Product
reports must still add bounded machine-readable partial reasons and carry those
reasons unchanged through the same layers.

Candidate count is never a coverage proxy. In particular, zero candidates plus
partial coverage cannot be described as no recoverable files on the source.

### 3. JPEG component

1. `crates/carving` depends on `SourceReader` and core domain types only.
2. Its caller supplies source-bounded regions and all resource limits.
3. The component performs sequential bounded reads with enough carry to detect
   an SOI split across chunks.
4. Candidate validation is incremental and uses a chunk-sized buffer rather
   than allocating the maximum candidate size. Signature-attempt and aggregate
   validation-byte budgets bound hostile false-positive fanout.
5. A JPEG candidate requires bounded SOI, marker length, SOF dimension, SOS,
   entropy stuffing/restart, and EOI validation.
6. The candidate is contiguous. No generic fragmentation inference is allowed.
7. Results preserve physical range, method, uncertain generated metadata, and
   structural state. `CarveReport.evidence` links candidate ID to physical
   range, exact-range SHA-256, and the `jpeg-structural-v1` validator version.
8. The component performs no extraction, destination write, preview, pixel
   decode, repair, execution, network access, native device open, or scope
   selection.

### 4. Product admission

The portable component accepts explicit regions so it can be tested and reused;
that API is not product scan authority.

The NTFS scanner may expose coalesced regions explicitly proven free by its
retained `$Bitmap` snapshot. `$Bitmap` becomes allocation authority only when
record 6 is active, a base record, not a directory, named `$Bitmap` under the
root, has exactly one unnamed data stream, no `$ATTRIBUTE_LIST`, no
compressed/encrypted/sparse attribute flag, starting VCN zero, and coherent
initialized/logical/allocated sizes. That evidence remains partial when the
bitmap is capped or incomplete and never classifies its unknown suffix as
free.

The implemented CLI-library/Tauri integrator invokes the JPEG component only
when:

- the selected scope is the whole mounted NTFS volume;
- every submitted range has trustworthy `FreeInSnapshot` allocation evidence;
- capped/unknown/conflicting/allocated/unreadable regions are excluded and
  reported;
- folder scope is absent;
- no automatic fallback broadens the scan to allocated or whole RAW content.

`MetadataOnly` is the library default and the legacy `scan_volume_reader`
behavior. The desktop accepts only explicit `metadata` or `deepJpeg`; native
validation rejects `deepJpeg` with a folder scope or non-NTFS volume before
broker use. The `scan-image` process command remains metadata-only.

The product deep profile uses 1 MiB chunks, 128 MiB per candidate, 10,000
candidates, 65,536 coalesced regions, 16 TiB of submitted free space,
10,000,000 validation attempts, and 8 GiB of aggregate validation reads.
Coverage exports every relevant counter/limit flag and becomes partial on
saturation.

Exact-range corroboration keeps one unambiguous metadata identity and attaches
the JPEG hash/validator evidence. Ambiguous multiple metadata owners retain a
separate carving candidate with an explicit warning. Duplicate carves of one
range coalesce only when SHA-256 and validator agree. This is not a general
evidence-merging policy.

## Security invariants

- no scan-source write, trim, format, delete, lock, dismount, mount, or repair;
- no caller-selected desired access, device-control code, raw device path, or
  destination in the carving API;
- all offsets, lengths, marker segments, chunks, overlaps, counters, and
  allocations use checked source/region bounds;
- only deterministic synthetic images or explicitly allowlisted disposable
  test VHDs may be used in tests;
- recovered bytes are never executed or previewed by the broker, desktop, or
  carving component;
- carving does not grant filesystem ancestry or an original name/path/date;
- unknown allocation is not treated as free;
- a work/region/candidate/signature/validation limit produces bounded
  partial/rejected evidence, not a completion claim.

## Consequences

### Positive

- deleted records after the legacy MFT prefix can be discovered;
- 1 MiB-scale reads substantially reduce ordinary broker round trips;
- memory tracks bounded useful retained evidence and one batch rather than the
  whole MFT;
- zero results gain quantitative interpretation;
- the first content carver is real, portable, read-only, bounded, and
  deterministic;
- allocation authority remains outside the portable carving component;
- linked physical range, validator version and exact-range SHA-256 reach the
  product DTO without sending recovered bytes to JavaScript.

### Negative

- the explicit high MFT record ceiling can still make an extraordinary scan
  partial;
- each of the four retained MFT evidence classes can saturate at 100,000 and
  then makes the result partial;
- the aggregate retained-evidence ceilings of 400,000 names, 200,000 streams,
  or 1,000,000 run elements can omit a later record/merge and make the result
  partial; hostile external-corpus and long-running memory evidence is still
  incomplete;
- per-record fallback after a failed batch can be slow in damaged regions;
- numeric MFT coverage is visible, but machine-readable partial reasons and a
  full byte/reason breakdown are absent;
- the JPEG mode has no real-time progress, cooperative cancellation,
  checkpoint, or `scan-image` command-line option;
- contiguous JPEG carving cannot reconstruct generic fragmentation;
- structural marker validation is not a safe preview or proof of visual
  integrity;
- a live system volume remains mutable and SSD TRIM may have removed deleted
  content before the scan.

## Clean-room and license record

The public
[`saintmarina/undelete_jpg`](https://github.com/saintmarina/undelete_jpg/tree/e134c2078d5b1198d020521b91893f01f1cd4049)
repository was reviewed conceptually. It is distributed under the
[`MIT License`](https://github.com/saintmarina/undelete_jpg/blob/e134c2078d5b1198d020521b91893f01f1cd4049/LICENSE).
No code, constants, output naming, API surface, or implementation is
incorporated. The local Rust component is independently implemented against the
master specification and deterministic local fixtures.

The reference confirms that sequential read-only signature search followed by
bounded structural checks is a useful starting idea. Its whole-device scan,
POSIX I/O, current-directory writes, overwrite-prone output, fixed-size
assumptions, and limited validation are explicitly not adopted.

## Verification and evidence boundary

Component acceptance requires:

- a deleted NTFS fixture record beyond the former 64 MiB boundary and exact
  truth SHA-256;
- a bounded 1 MiB MFT batch-read regression;
- an NTFS allocation snapshot regression that exposes only bitmap-proven free
  regions and rejects untrusted `$Bitmap` record/stream semantics;
- existing NTFS corruption, bounds, namespace, and extraction regressions;
- JPEG signature-at-boundary, marker-validation, incremental-state,
  signature/validation-budget, truncation, region-bound, candidate-size/count,
  partial-read, and SHA-256 fixtures;
- allocation-aware CLI integration for free, allocated, unknown, non-NTFS,
  region-limit, ID/deduplication, and ambiguity cases;
- Tauri/TypeScript/UI tests for explicit mode admission, schema 3 coverage,
  schema 2 evidence, fail-closed provenance, and non-exhaustive zero messaging;
- format, Clippy, complete Rust workspace tests, and frontend gates on the same
  revision.

No ordinary acceptance scan may open the user's `C:` volume or another real
disk. Synthetic success does not establish external-corpus or arbitrary-media
compatibility.

Production-readiness additionally requires:

- real byte progress and cooperative cancellation;
- performance/memory/false-positive evaluation and external forensic corpora;
- native accessibility and long-running mutable-volume evidence;
- explicit release evidence.

## Relationship to earlier decisions

- [ADR-0002](0002-read-only-source-invariant.md) remains authoritative.
- [ADR-0004](0004-deterministic-synthetic-fixtures.md) and
  [ADR-0005](0005-pull-request-ci-excludes-devices.md) govern tests.
- [ADR-0011](0011-ntfs-raw-parsing-and-active-record-apis.md) remains
  authoritative for NTFS structural incompleteness.
- This ADR accepts only the first built-in bounded JPEG slice of
  [ADR-0013](0013-carving-plugin-architecture.md). It does not accept a general
  plugin ABI, custom signatures, preview, repair, or the complete baseline
  format set.
- [ADR-0023](0023-windows-read-only-broker-and-folder-scope.md) remains
  authoritative for mounted-volume authority. This decision does not add a
  broker opcode, physical-disk scan, path-bearing IPC, or folder carving.

## Revisit triggers

Revisit before changing the record/batch/retention/carving ceilings, retaining
broad active-file state, weakening `$Bitmap` authority, scanning
allocated/unknown ranges, adding damaged/RAW whole-region mode, carving a
selected folder, inferring fragmented JPEGs, adding another format, loading
external code/signatures, adding preview/repair/extraction, or calling the
current indeterminate/non-cancellable slice production-ready.
