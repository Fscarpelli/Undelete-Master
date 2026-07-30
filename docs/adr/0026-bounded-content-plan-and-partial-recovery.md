# ADR-0026 — Bounded Content Plan and Partial Recovery

Status: Accepted
Decision date: 2026-07-30

Implementation status: `Implemented-unverified` for the SDD-020 Task 2 slice.
Portable logical planning and caller-owned streaming extraction are implemented
and pass focused deterministic tests. Destination authority, filesystem writes,
transactional publication, journals, manifests, Tauri jobs, the React workflow,
real-device acceptance, and final same-revision evidence remain later SDD-020
tasks.

## Context

The existing `um_core::extract_candidate` helper materializes the complete file
and a byte-per-byte coverage map in memory. It also treats any extent with a
physical offset as readable regardless of allocation evidence. That behavior is
useful only for small fixture inspection; it cannot safely recover a
multi-gigabyte file, enforce explicit partial-recovery consent, or preserve an
auditable reason for every zero-filled range.

Recovery therefore needs a portable contract between retained scan evidence and
later destination publication. The contract must reject malformed extent
authority before source I/O, distinguish intentional sparse zeros from degraded
content, stream through the existing read-only `SourceReader`, and never make
memory scale with logical file size.

## Decision

### Logical content plans

`um-restore` converts one native `Candidate` into a checked `ContentPlan`.
Directories return no content plan. A zero-length file returns an empty plan and
can be hashed as an empty stream.

Candidate extents are validated before sorting. Zero-length extents, checked
logical or physical overflow, logical coverage beyond the declared file size,
and readable physical ranges beyond the supplied source length fail closed.
Candidate extents may arrive unordered; the planner sorts them by logical
offset and then rejects every duplicate or overlap deterministically.

The resulting segments cover exactly `[0, logical_size)`:

- `Read` carries a logical offset, physical offset, and length for
  `FreeInSnapshot` or physically mapped `Resident` evidence;
- `Zero(Sparse)` represents intentional filesystem zeros and is not partial;
- a logical gap becomes `Zero(MissingExtent)`;
- `CurrentlyAllocated`, `OutOfVolume`, prior `ReadFailed`, `Zeroed`, and
  `Unknown` evidence retain their corresponding closed zero-fill reason; and
- readable/resident evidence without a physical mapping is
  `UnknownAvailability`.

Adjacent reads coalesce only when their logical and physical ranges are
contiguous and their source-availability semantics match. Adjacent zero ranges
coalesce only when their reasons match. Plans contain at most 1,000,000
segments; caller-supplied lower limits are enforced during construction.

### Partial policy

`CompleteOnly` accepts readable and sparse ranges only. Any other unavailable
range rejects planning with its exact logical offset, length, and reason.

`ZeroFillAndMap` explicitly authorizes unavailable ranges to be materialized as
zeros and recorded. The plan retains this permission privately because
streaming-time read faults cannot otherwise be distinguished from a
`CompleteOnly` request. Sparse-only plans do not set `requires_best_effort`;
any non-sparse planned or streaming-time zero fill does.

### Defensive streaming preflight

Although plans are produced natively, their listed fields remain inspectable
and may be changed by a future caller. `stream_candidate` therefore validates
the complete plan before the first source read or destination write:

- scratch length is `1..=1 MiB`;
- segment count, nonzero lengths, and all additions are bounded;
- segment coverage is ordered, contiguous, non-overlapping, and exactly equal
  to the logical size;
- every read remains within the current `SourceReader::len()`;
- zero reasons remain compatible with the retained partial policy; and
- the public best-effort summary agrees with the actual segment reasons.

An unordered, duplicated, overlapping, gapped, overflowing, truncated, or
out-of-source plan produces no I/O.

### Read faults and evidence

Every source request is at most the caller's scratch length and therefore at
most 1 MiB. Streaming uses only `read_best_effort_at`; it never opens a path or
source itself.

`ReadOutcome` is treated as untrusted boundary data. Bad ranges must be
nonempty, checked, source-chunk-relative, and within the requested buffer.
Their normalized union must agree exactly with `bytes_valid`. Malformed
outcomes fail the item before that chunk is written. Valid bad ranges are
explicitly zeroed in scratch; the implementation does not rely on a reader
having zeroed them.

Under `CompleteOnly`, the first runtime bad range fails extraction. Under
`ZeroFillAndMap`, readable bytes around each fault are preserved and the exact
bad ranges are recorded as `PreviouslyReadFailed`. That closed reason covers
both prior scan evidence and a fault observed during extraction because the
public reason enum intentionally has no separate runtime variant.

Zero-fill evidence coalesces only across adjacent ranges with the same reason
and is capped at 65,536 ranges. A range explosion fails the item instead of
omitting evidence or growing memory with file size. One normalized
`ReadOutcome` is also bounded by the 1 MiB request.

### Writes, progress, cancellation, and hashes

Read and zero segments are written in scratch-sized chunks. SHA-256 is updated
only for byte counts actually accepted by `Write`. Progress reports cumulative
accepted bytes and the logical total after each successful write; it is never
advanced from requested, read, or simulated work.

Cancellation is cooperative and checked before each bounded read and each
bounded write. A cancellation or output error reports the exact accepted byte
count. The caller owns the incomplete output and later transactional code must
ensure it is never published as final.

Successful extraction requires the accepted byte count to equal logical size.
The returned SHA-256 covers exactly the complete output, including sparse and
policy zeros. If an expected SHA-256 is present, mismatch returns an error
after streaming and before any later caller may publish the output.

## Security consequences

- No scan source is opened or written by `um-restore`; authority remains the
  read-only `SourceReader`.
- The crate has no Windows, broker, desktop, path, destination, preview,
  execution, registry, device-control, or network dependency.
- Preflight rejects every malformed plan before source or destination I/O.
- Source reads and scratch are at most 1 MiB.
- Segment and zero-evidence collections have fixed ceilings; no allocation is
  proportional to logical file size.
- Recovered bytes remain untrusted caller-owned output and are never executed
  or previewed.
- Tests use only deterministic in-memory candidates, readers, and writers.

## Consequences and limits

- Contiguous, fragmented, resident, sparse, gapped, unavailable, and
  streaming-read-failed logical content now has one closed portable model.
- Best-effort output is explicitly distinguishable and retains exact zero-fill
  evidence; it is never described as intact.
- A malformed `SourceReader` outcome fails rather than guessing which bytes are
  valid.
- Cancellation and output/hash errors can occur after bytes have reached the
  caller-owned writer. Task 4 must keep those bytes under a temporary,
  non-published capability until all checks and journal steps succeed.
- Expected original hashes will ordinarily reject intentionally zero-filled
  output. Callers must not discard or weaken the mismatch merely to publish a
  partial file.
- This task does not add destination writes, collision handling, path
  sanitation, transactional publication, a manifest, UI commands, or physical
  disk identity.

## Verification

Focused integration tests construct real `Candidate` and `ExtentRun` values
and observable in-memory `SourceReader`/`Write` doubles. They cover:

- contiguous, fragmented, and resident output with a literal known SHA-256;
- sparse, missing, allocated, out-of-volume, previously failed, zeroed, and
  unknown range planning;
- `CompleteOnly` rejection and explicit `ZeroFillAndMap`;
- exact runtime read-fault zeroing, malformed `ReadOutcome` rejection, and the
  zero-evidence ceiling;
- canonical logical ordering/coalescing across adjacent runtime and
  previously planned read-failure evidence;
- cancellation between bounded chunks and a cancelled 5 GiB logical plan
  without allocating or hashing 5 GiB;
- progress for only bytes accepted by a partial-failure writer;
- scratch/source-read bounds, empty-file hashing, hash match, and hash mismatch;
  and
- malformed, duplicate, overlapping, out-of-order, gapped, overflowing, and
  out-of-source plans rejected before the first read or write.

No test opens or writes a real disk.

## Relationship to earlier decisions

- [ADR-0002](0002-read-only-source-invariant.md) remains authoritative for
  source access.
- [ADR-0004](0004-deterministic-synthetic-fixtures.md) governs test media.
- [ADR-0024](0024-streaming-mft-and-bounded-content-carving.md) remains
  authoritative for retained candidate and carving hash evidence.
- [ADR-0025](0025-native-result-query-and-selection-authority.md) remains
  authoritative for native candidate retention and selection.

## Revisit triggers

Revisit before changing the scratch, segment, or evidence ceilings; adding a
new availability/zero reason; accepting compressed or encrypted logical
streams; retaining a different runtime partial-policy representation; changing
hash semantics; or allowing any component to publish output before length and
expected-hash validation succeed.
