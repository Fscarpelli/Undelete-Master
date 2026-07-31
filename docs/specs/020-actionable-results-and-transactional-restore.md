# SDD-020 — Actionable Results and Transactional Restore

Status: Approved design; implementation in progress
Decision date: 2026-07-30

This increment turns the connected-volume desktop from a discovery-only report
into a usable recovery workflow. It implements backend-owned query, selection,
destination authority, restore planning, bounded extraction, transactional
destination writes, partial-file policies, progress, cancellation, and a
recovery manifest.

The master specification remains authoritative. This increment implements the
first production slice of FR-060 through FR-067 and FR-080 through FR-088
without weakening the read-only source invariant.

## 1. Confirmed problem

The current desktop can discover and display real candidates, but it cannot
act on them:

- candidate pages accept only `scanId`, cursor, and a fixed limit;
- the frontend appends every loaded page to one growing array;
- there is no global query, filter, sort, or bounded page cache;
- there is no selection model in the frontend or native state;
- `ScanSession` retains presentation rows, not the candidate extents required
  for extraction;
- there is no destination picker, restore plan, restore engine, progress,
  cancellation, or recovery manifest;
- the production-surface test explicitly rejects restore commands.

This is not a hidden-control defect. Restore and results-workspace behavior are
absent from the implemented product surface.

## 2. Approved outcome

The increment is complete only when a user can:

1. scan a supported source without writing to it;
2. search, filter, and sort the entire result set rather than only visible
   rows;
3. select individual candidates or all candidates matching a query;
4. keep selection while changing page, query, filters, or sort order;
5. inspect an exact selection summary before recovery;
6. choose a real destination folder through a native picker;
7. recover only explicitly selected candidates to a different physical disk;
8. choose an explicit best-effort policy for incomplete candidates;
9. observe real item/byte progress and cancel cooperatively;
10. receive recovered files plus a versioned manifest containing hashes,
    missing/conflicting ranges, warnings, and final disposition.

All visible controls operate on real native state. No sample candidate,
simulated progress, placeholder restore, or browser fallback is permitted.

## 3. Scope boundaries

### 3.1 In scope

- metadata-backed files of every extension when usable content extents remain;
- carved candidates already emitted with bounded physical ranges;
- NTFS resident, contiguous, fragmented, and sparse streams represented by
  the candidate model;
- best-effort recovery of readable parts with explicit gap/conflict evidence;
- backend-owned query, stable sort, pagination, and selection;
- a native destination capability that never exposes a path to the WebView;
- same-physical-disk rejection;
- NTFS restore destinations for capability-relative atomic hard-link
  publication;
- destination collision policy `rename`, with no silent replacement;
- bounded streaming extraction and SHA-256;
- per-file `.umrecovering` temporary state and no-clobber publication;
- JSON recovery manifest and per-item partial sidecars when required;
- bounded in-memory scan/session state for this increment.

### 3.2 Explicitly outside this increment

- restoring to the original path or the same physical disk;
- ReFS/FAT/exFAT/unknown restore destinations until a capability-relative atomic
  no-replace publication primitive is implemented for them;
- overwriting an existing destination file;
- restoring NTFS ACLs, EFS keys, alternate data streams, reparse points, or
  executable state;
- transparent NTFS compression decompression;
- previewing or executing recovered content;
- claiming a snapshot of a live mounted source;
- persistent resume after closing the desktop;
- expanding deep carving beyond the formats implemented by a separately
  accepted multiformat increment;
- reversing TRIM, overwrite, secure erase, unavailable encryption keys, or
  physical media loss.

These exclusions are fail-closed product boundaries, not silent fallbacks.

## 4. Trust and authority model

### 4.1 Source authority

The desktop remains unelevated. Privileged source reads remain confined to the
read-only broker and expose only `SourceReader`.

Native scan state retains:

- opaque source volume identity and the inventory generation used to bind it;
- canonical source length and filesystem evidence;
- the full bounded `Candidate` objects needed for extraction;
- opaque row IDs mapped to candidate indexes;
- carving evidence needed to verify an expected content hash;
- the sanitized presentation rows used by candidate queries.

The WebView never receives a native source path, volume GUID, physical disk
number, handle, extent, offset, or recovered byte.

Before restore, the source is reopened through the same broker admission path.
The native coordinator revalidates source identity, length, candidate
descriptor bounds, and any expected carved-content hash. Identity change fails
closed.

### 4.2 Destination authority

The destination is selected only through a native folder picker. Native code
resolves and retains a bounded `DestinationAuthority` containing:

- opaque destination ID;
- a query-only directory handle opened without delete sharing and unavailable
  to the WebView;
- sanitized label and filesystem;
- observed free bytes;
- physical-disk identity set;
- reparse/symlink safety evidence;
- creation time and expiry generation.

The WebView receives only the opaque destination ID and sanitized summary.
Caller-provided destination paths are forbidden.

The source and destination physical-disk identity sets must both be known,
single-disk, unchanged, and disjoint. Unknown, multi-disk, substituted, or
same-disk destinations are rejected.

The authority is derived from the same retained directory handle used for
final-volume and disk-identity queries. Production restore code never reopens
the picker path. On Windows the root is opened without delete sharing, so it
cannot be renamed or substituted while the authority exists.

The queried destination filesystem must be NTFS. ReFS, FAT, exFAT, unknown, and
filesystems that reject hard links fail closed with an explicit
unsupported-destination result. No overwrite-capable or path-based fallback is
allowed.

### 4.3 Destination writes

Destination writes occur in the unprivileged restore component. The elevated
broker never receives a destination, opens a destination, or writes recovered
content.

Every recovery job creates one uniquely named job directory beneath the
authorized destination. All descendant operations are capability-relative to
the retained root handle. Existing directories are opened one component at a
time with no-follow semantics; newly created directories are immediately
rebound through the same no-follow open. Final files are created without
following the last component. No ambient descendant path is reopened.

All derived relative paths are sanitized before use. No absolute path, UNC
component, drive prefix, `..`, alternate data stream, reserved device name,
trailing-dot/space component, or reparse escape is accepted. Deterministic
tests must race a real temporary Windows junction plus a scripted replacement
at every component transition and prove that no outside entry is created.

## 5. Candidate query and selection

### 5.1 Query

The backend query supports:

- case-insensitive name/path search;
- one or more extensions;
- candidate kind;
- metadata confidence;
- discovery method;
- candidate state;
- minimum and maximum recoverability score;
- complete/best-effort eligibility;
- selected-only mode.

The backend returns dynamic extension facets with counts for the active scan.
An empty extension represents files with no declared extension.

### 5.2 Stable sort

Supported sort fields are:

- path/name;
- extension;
- size;
- state;
- confidence;
- recoverability score;
- discovery method.

Every sort uses the opaque candidate ID as a final deterministic tie-breaker.
Null scores have an explicit stable placement. Cursor validity is bound to the
scan, canonical query, sort, and query revision.

### 5.3 Pagination

The native API returns at most 100 rows. The frontend keeps only a bounded
window of pages and never appends the complete result set to the DOM.

Foreign, stale, malformed, duplicated, or query-mismatched cursors fail closed.
Late responses from an older query revision are ignored.

### 5.4 Selection

Selection is canonical in native state and addressed by opaque candidate ID.
The frontend checkbox is a view of native selection, not the authority.

Supported operations are:

- set one or more candidate IDs selected/unselected;
- select every candidate matching the canonical query;
- clear every candidate matching the canonical query;
- clear the complete selection.

Every mutation carries a selection revision. Stale revisions are rejected.
Selecting all matching candidates is evaluated natively and does not send all
IDs through the WebView.

The selection summary reports:

- selected files and directories;
- logical bytes;
- candidates requiring best-effort consent;
- conflicted candidates;
- candidates with no readable content plan.

Each query-page response additionally reports `matchingSelectedCandidates` so
the filtered-set checkbox can distinguish none, some, and all without
enumerating selected IDs in the WebView.

## 6. Restore planning

A restore plan is immutable and bound to:

- scan/source identity;
- selection revision;
- destination authority;
- collision policy;
- partial-file policy;
- ordered candidate descriptors;
- required logical bytes;
- a plan digest.

Only explicitly selected candidates enter a plan. A selected directory creates
only that directory. Selecting a file creates only its necessary sanitized
ancestors. Historical siblings and descendants are never implied.

The first increment supports:

- collision policy `rename`;
- complete candidates;
- best-effort policy `zeroFillAndMap`, which preserves logical length, writes
  readable ranges, writes zeros for gaps, and emits a sidecar range map;
- directory-only items.

Candidates without any bounded extraction plan remain selectable for
inspection but are marked ineligible and cannot enter a restore plan.

## 7. Streaming restore algorithm

For each planned file:

1. revalidate the immutable plan, source identity, destination authority, and
   available space;
2. derive and validate the destination-relative path;
3. create a unique temporary file ending in `.umrecovering` in the final
   destination directory;
4. iterate logical extents in checked order using a bounded buffer;
5. read only through `SourceReader`;
6. stream readable bytes to the destination and SHA-256;
7. materialize sparse regions as zeros;
8. under `zeroFillAndMap`, materialize unreadable, unavailable, or conflicting
   logical ranges as zeros and record exact range evidence;
9. honor cooperative cancellation between bounded reads and writes;
10. flush and `sync_all` the temporary file;
11. verify length and any expected carving hash;
12. append and sync an `ItemPrepared` record to the versioned hash-chained job
    journal;
13. for a partial item, prepare and publish one immutable sidecar keyed by the
    ordered plan index and candidate ID, then durably journal that sidecar
    before attempting any data name;
14. atomically create the final data name as a capability-relative hard link to
    the temporary file; if the name exists, derive a deterministic renamed
    destination and retry within a bounded limit without retracting or
    republishing the sidecar;
15. append and sync `ItemPublished`, or an explicit failure/cancellation
    record, to the journal;
16. include exactly one truthful disposition for every immutable plan item in
    the final manifest.

An interrupted or failed item never appears under its final name unless the
publication step completed. Temporary-file disposition is recorded.

The journal is append-only canonical JSON Lines with a monotonic sequence,
previous record hash, record hash, and bounded record count. Audit rejects
noncanonical encodings, unknown envelope fields, oversized records, torn
tails, and chain or job-identity changes. Publication never begins unless
`ItemPrepared` is durable. A journal write/flush/sync failure poisons the
journal, fails the job, forbids further publication, and suppresses a final
manifest that could otherwise overstate the uncertain journal prefix.

With a healthy journal, ordinary item completion, failure, or cancellation
proceeds to final-manifest publication. Every final manifest that is
successfully published contains exactly the immutable plan item count. Each
item is marked `published`, `failed`, `cancelled`, `notAttempted`, or
`directoryCreated`; an item failure or cancellation never silently truncates a
published manifest. Manifest preparation or no-clobber publication can itself
fail explicitly, in which case no existing manifest is replaced and the job
must not claim that a final manifest exists.

Hard-link creation is the no-clobber commit primitive: it fails atomically when
the destination name already exists and never replaces that entry. Hard-link
failure is explicit; it never falls back to rename-overwrite or copy-to-final.
The temporary handle remains open without delete sharing through publication,
and its file identity must match a capability-relative no-follow lookup
immediately before linking. Namespace substitution or identity mismatch fails
closed.

Task 4 deliberately retains the protected temporary link after publication.
Releasing its no-delete-share handle and then deleting that path would create a
time-of-check/time-of-use substitution window; the current safe
capability-relative API has no identity-atomic unlink primitive. The manifest
therefore reports `retainedBySafeCleanupPolicy` after a synchronized
publication and `retainedForReconciliation` when namespace durability is not
proven. A later cleanup implementation may remove the link only through a
reviewed identity-atomic primitive.

A selected directory follows the same capability-relative no-follow ancestor
walk, creates and immediately rebinds only that directory, syncs its parent,
and receives the `directoryCreated` disposition. It has no content plan,
source read, data temporary file, or partial sidecar.

The implementation must not allocate memory proportional to candidate size.

## 8. Partial and conflicted content

The UI must distinguish:

- complete evidence;
- complete but unvalidated;
- best-effort due to gaps/read failures;
- conflicted because currently allocated bytes may belong to another file;
- zeroed/TRIM evidence;
- metadata-only with no extraction plan.

Best-effort recovery requires an explicit confirmation in the restore plan.
The output sidecar is keyed by ordered plan index plus candidate ID, is
published once before data-name retries, and is bound back into the manifest
by item key, sidecar path, and sidecar SHA-256. It records:

- item key and candidate ID;
- logical size;
- readable ranges;
- all zero-filled ranges;
- zero-filled missing ranges;
- conflicting ranges;
- source read errors;
- output SHA-256;
- expected content SHA-256 when available;
- validation and warning evidence.

Score and confidence are prioritization aids. They never block a user from
requesting an eligible best-effort recovery and never imply a guarantee.

## 9. Native command contract

Schema version 1 adds the following bounded commands:

### `query_candidate_page`

Consumes opaque `scanId`, canonical query, canonical sort, cursor, and
`limit = 100`. Returns query ID/revision, filtered total, facets, selection
summary, and at most 100 rows with selected/eligible flags.

### `update_candidate_selection`

Consumes opaque scan/query IDs, operation, bounded candidate IDs when required,
and selection revision. Returns the new revision and summary.

### `select_restore_destination`

Consumes only request and scan IDs. Returns `null` for native-picker
cancellation or an opaque destination summary.

### `create_restore_plan`

Consumes scan, selection revision, destination ID, `rename`, and the chosen
partial policy. Returns an immutable opaque plan and exact summary.

### `start_restore`

Consumes only the opaque plan ID. Returns an opaque job ID immediately and
starts bounded native work.

### `get_restore_job`

Returns real item/byte counters, current item, status, warnings, and final
manifest summary.

### `cancel_restore`

Consumes only the job ID and requests cooperative cancellation.

### `open_restore_destination`

Consumes only request and completed job IDs. Native code revalidates the
job-bound destination authority and asks the Windows shell to open the recovery
job directory. No path or caller-selected executable crosses the WebView
boundary.

No command accepts a native source/destination path, extent, offset, handle,
access mask, device-control code, executable, or recovered bytes.

## 10. UI workflow

The results screen retains the current visual language and adds:

- search;
- extension multi-select facets with counts;
- confidence, score, method, state, and eligibility filters;
- sortable column headers with `aria-sort`;
- one checkbox per row;
- an accessible tri-state checkbox for the current filtered set;
- bounded previous/next page navigation;
- selected-only filter;
- a sticky selection bar with count, bytes, partial/conflict count, clear, and
  `Recover selected`;
- a native destination step;
- a restore-plan review step;
- explicit best-effort consent;
- real progress, cancel, completion, partial, and failure states;
- a final `Open destination folder` action only; no recovered file is executed.

The table renders a bounded page, preserves keyboard focus, exposes row counts,
and keeps recovered text directionally isolated.

## 11. Performance and bounds

Initial hard limits:

- 100 rows per page;
- 100 candidate IDs per direct selection mutation;
- 110,000 retained candidates per scan, covering the scanner's 100,000
  metadata-candidate budget plus the deep scanner's 10,000 carved-candidate
  budget;
- four retained scans;
- 32 destination authorities;
- eight active/planned restore jobs;
- 1 MiB source read/write buffer;
- 256 KiB serialized original-path evidence per item;
- 8 MiB aggregate serialized original-path evidence per restore job;
- 1,000,000 sanitized path components per job;
- 1,000,000 journal records per restore job;
- 10,000 collision-renaming attempts per job;
- manifest entries equal to the immutable plan item count.

All additions and multiplications use checked arithmetic. Bound saturation is a
structured partial/error state and is never described as complete.

## 12. Required tests and evidence

### Query and selection

- `RESULT-QUERY-FILTER-001`
- `RESULT-QUERY-SORT-002`
- `RESULT-CURSOR-BINDING-003`
- `RESULT-SELECTION-PERSIST-004`
- `RESULT-SELECT-ALL-005`
- `RESULT-SELECTION-STALE-006`
- `RESULT-PAGE-BOUND-007`
- `RESULT-LATE-RESPONSE-008`

### Restore engine

- `RESTORE-STREAM-CONTIGUOUS-001`
- `RESTORE-STREAM-FRAGMENTED-002`
- `RESTORE-STREAM-RESIDENT-003`
- `RESTORE-STREAM-SPARSE-004`
- `RESTORE-PARTIAL-ZERO-FILL-005`
- `RESTORE-READ-FAILURE-006`
- `RESTORE-CANCEL-007`
- `RESTORE-MEMORY-BOUND-008`
- `RESTORE-HASH-MATCH-009`
- `RESTORE-HASH-MISMATCH-010`

### Path and transaction safety

- `RESTORE-PATH-TRAVERSAL-011`
- `RESTORE-PATH-WINDOWS-RESERVED-012`
- `RESTORE-PATH-REPARSE-013`
- `RESTORE-COLLISION-RENAME-014`
- `RESTORE-NO-CLOBBER-015`
- `RESTORE-TEMP-PUBLICATION-016`
- `RESTORE-MANIFEST-017`
- `RESTORE-JOURNAL-DURABILITY-027`
- `RESTORE-REPARSE-RACE-028`

### Authority and desktop

- `RESTORE-DIFFERENT-DISK-018`
- `RESTORE-SAME-DISK-REJECT-019`
- `RESTORE-UNKNOWN-DISK-REJECT-020`
- `RESTORE-SOURCE-CHANGED-021`
- `DESKTOP-RESULT-CONTROLS-022`
- `DESKTOP-RESTORE-CONTRACT-023`
- `DESKTOP-RESTORE-FLOW-024`
- `DESKTOP-RESTORE-A11Y-025`
- `DESKTOP-RESTORE-OPEN-DESTINATION-026`

Tests use deterministic in-repository images and temporary destination
directories only. No test writes to a real scan source or performs destructive
work on a real disk.

Expected recovered results are asserted by SHA-256.

## 13. Architecture decisions required by implementation

Implementation must add or amend ADRs for:

1. backend candidate-query and native-selection authority;
2. candidate/source binding across scan and restore;
3. destination capability, physical-disk comparison, reparse containment, and
   no-clobber publication;
4. bounded restore progress/cancellation and manifest durability.

Any new Windows API remains inside the audited `crates/io-windows` FFI boundary
and must fit its closed read-only/query-only source surface. Destination writes
belong to `crates/restore`, not the broker or source-I/O layer.

## 14. Completion gate

This increment is not complete merely because controls render or a temporary
file is created. Completion requires:

- all commands operate on real native state;
- a deterministic NTFS fixture can be scanned, selected through the same
  native selection model, restored, and verified by SHA-256;
- a deterministic partial fixture produces the expected zero-filled output and
  exact missing-range sidecar;
- same-disk/unknown identity, path traversal, reparse, hash mismatch, stale
  selection, and collision cases fail as specified;
- journal durability/failure injection and a real temporary Windows junction
  race pass without any write outside the retained destination authority;
- required Rust and frontend gates pass;
- the traceability matrix and known limitations match the implemented
  boundary;
- a release build, not only a debug build, is produced for user validation.

