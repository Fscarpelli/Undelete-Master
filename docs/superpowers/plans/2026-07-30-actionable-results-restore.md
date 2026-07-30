# Actionable Results and Transactional Restore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` to execute this plan task by task.

**Goal:** Turn the real connected-volume scan into an end-to-end recovery
workflow that can select, plan, and safely restore metadata-backed files of any
extension, including explicit best-effort output when parts are unavailable.

**Architecture:** Native Rust state remains authoritative for candidates,
queries, selection, source identity, destination capabilities, plans, and
jobs. A new `um-restore` crate builds bounded logical content plans and streams
them from the existing read-only `SourceReader` into an unprivileged,
transactional destination writer. The React WebView receives only opaque IDs
and sanitized summaries; it renders one bounded backend page and never receives
source paths, destination paths, extents, offsets, handles, or recovered bytes.

**Tech Stack:** Rust 1.88 workspace, Tauri 2, Windows read/query-only FFI in
`um-io-windows`, React 18, TypeScript, Vitest, Testing Library, deterministic
NTFS/FAT fixtures, SHA-256 verification.

## Global constraints

- Never open, mutate, lock, dismount, trim, format, or otherwise write to a
  scan source.
- Never run a destructive test against a real disk. Tests use deterministic
  in-repository images and temporary destination directories only.
- Keep the desktop unelevated and all source reads behind `SourceReader`.
- Destination writes must stay outside the elevated broker.
- No production mock providers, fake candidates, simulated progress, TODOs,
  stubs, or placeholder controls.
- All parser, range, count, and byte arithmetic is checked and bounded.
- Never overwrite an existing destination entry.
- Recovered files are untrusted. Never preview or execute them.
- Update `docs/traceability-matrix.md` in every task that changes behavior.
- Add or amend an ADR for every architecture/security decision.
- Every bug fix and requirement begins with a failing regression test.
- Do not stage or modify the unrelated, user-owned `crates/fs-exfat/` work in
  the primary checkout.

---

## Task 1: Backend-owned candidate query and durable selection

**Requirements:** `RESULT-QUERY-FILTER-001`,
`RESULT-QUERY-SORT-002`, `RESULT-CURSOR-BINDING-003`,
`RESULT-SELECTION-PERSIST-004`, `RESULT-SELECT-ALL-005`,
`RESULT-SELECTION-STALE-006`, `RESULT-PAGE-BOUND-007`.

**Files:**

- Create: `apps/desktop/src-tauri/src/results.rs`
- Modify: `apps/desktop/src-tauri/src/storage.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src/api/storage.ts`
- Modify: `apps/desktop/src/api/storage.test.ts`
- Modify: `apps/desktop/src/api/storageDesktop.ts`
- Modify: `apps/desktop/src/api/storageDesktop.test.ts`
- Create: `docs/adr/0025-native-result-query-and-selection-authority.md`
- Modify: `docs/adr/README.md`
- Modify: `docs/traceability-matrix.md`

### Step 1: Write the failing Rust query tests

Add tests beside `results.rs` that construct at least 12 real
`StoredCandidate` values covering mixed names, extensions, sizes, methods,
states, confidence, scores, eligibility, and selection.

The native model must expose these closed types:

```rust
pub const CANDIDATE_PAGE_LIMIT: usize = 100;

pub struct StoredCandidate {
    pub candidate: um_core::Candidate,
    pub row: CandidateRowDto,
    pub expected_sha256: Option<[u8; 32]>,
}

pub struct ScanSourceBinding {
    pub inventory_generation: String,
    pub volume_id: String,
    pub source_len: u64,
    pub file_system: String,
}

pub enum RecoveryEligibility {
    Complete,
    BestEffort,
    Ineligible,
}
```

Tests must prove:

- every filter is applied to the complete retained scan, not the visible page;
- extension matching is case-insensitive and uses `""` for no extension;
- extension facet counts are exact for the active scan;
- every confidence, method, state, kind, score, and eligibility filter is
  exercised independently and in a combined query;
- each supported sort has `candidate.id` as the final tie-breaker;
- null scores have deterministic placement;
- no page can contain more than exactly 100 rows;
- a cursor is rejected after query, sort, scan, or query-revision changes;
- a cursor cannot be replayed against a foreign scan;
- select-all operates on the canonical filtered set without accepting a list of
  IDs;
- selection survives query, sort, and page changes;
- stale selection revision is rejected without mutation;
- `matching_selected_candidates <= filtered_total`.

Run:

```powershell
cargo test -p um-desktop result_
```

Expected: FAIL because the result authority and commands do not exist.

### Step 2: Implement the native authority minimally

Move candidate storage from presentation-only rows to a bounded structure:

```rust
pub struct ScanSession {
    // Existing scan metadata remains.
    source: ScanSourceBinding,
    candidates: Vec<StoredCandidate>,
    candidate_index: HashMap<um_core::CandidateId, usize>,
    selected: HashSet<um_core::CandidateId>,
    selection_revision: u64,
}
```

Implement canonical `CandidateQuery`, `CandidateSort`, stable query IDs,
opaque bound cursors, page DTOs, facets, `query_candidate_page`, and
`update_candidate_selection`. Direct ID mutations accept `1..=100` unique IDs.
Selection operations are a tagged enum:

```rust
pub enum SelectionOperationDto {
    SetIds {
        candidate_ids: Vec<String>,
        selected: bool,
    },
    SelectAllMatching,
    ClearMatching,
    ClearAll,
}
```

The selection summary includes global selected counts/bytes plus
`matchingSelectedCandidates`. Keep IDs decimal/opaque in the WebView contract.
Do not serialize `Candidate`, extents, source bindings, hashes, or paths that
represent native authority.

Physical-disk identity is deliberately not part of this task because the
current inventory has display evidence only. Task 3 atomically extends
`ScanSourceBinding` after the authoritative broker-open response carries that
identity; no sentinel, guessed disk number, or placeholder is permitted.

### Step 3: Add failing TypeScript contract tests

In `storage.test.ts`, add exact-schema parser tests that:

- accept one complete schema-v1 query page;
- reject 101 rows, duplicate candidate IDs, bad decimal `u64` strings,
  `matchingSelectedCandidates > filteredTotal`, and unknown fields;
- reject any destination/source path, extent, offset, handle, or recovered-byte
  field added to a response.

In `storageDesktop.test.ts`, verify wrappers send only:

```ts
queryCandidatePage(requestId, scanId, query, sort, cursor)
updateCandidateSelection(
  requestId,
  scanId,
  queryId,
  operation,
  selectionRevision,
)
```

Run:

```powershell
pnpm --dir apps/desktop test -- storage.test.ts storageDesktop.test.ts
```

Expected: FAIL because schema-v1 parsers and wrappers are absent.

### Step 4: Implement the TypeScript contract

Add `CandidateQuery`, `CandidateSort`, `RecoveryEligibility`,
`ActionableCandidateRow`, `CandidateSelectionSummary`,
`CandidateQueryPage`, `CandidateSelectionOperation`, and exact parsers.
The page parser enforces 100 rows, unique IDs, closed keys, and decimal bounds.

Add wrappers for the two native commands. Remove no legacy scan behavior yet;
Task 6 replaces the old page-append consumer.

### Step 5: Document and verify

Record native query/selection authority and cursor/revision binding in
ADR-0025. Mark the eight result requirements as implemented/tested in the
traceability matrix only when their tests pass.

Run:

```powershell
cargo fmt --all
cargo test -p um-desktop result_
pnpm --dir apps/desktop test -- storage.test.ts storageDesktop.test.ts
```

Expected: PASS.

### Step 6: Commit

```powershell
git add apps/desktop/src-tauri/src/results.rs apps/desktop/src-tauri/src/storage.rs apps/desktop/src-tauri/src/lib.rs apps/desktop/src/api/storage.ts apps/desktop/src/api/storage.test.ts apps/desktop/src/api/storageDesktop.ts apps/desktop/src/api/storageDesktop.test.ts docs/adr/0025-native-result-query-and-selection-authority.md docs/adr/README.md docs/traceability-matrix.md
git commit -m "feat: add native result query and selection"
```

---

## Task 2: Bounded logical content planning and streaming extraction

**Requirements:** `RESTORE-STREAM-CONTIGUOUS-001`,
`RESTORE-STREAM-FRAGMENTED-002`, `RESTORE-STREAM-RESIDENT-003`,
`RESTORE-STREAM-SPARSE-004`, `RESTORE-PARTIAL-ZERO-FILL-005`,
`RESTORE-READ-FAILURE-006`, `RESTORE-CANCEL-007`,
`RESTORE-MEMORY-BOUND-008`, `RESTORE-HASH-MATCH-009`,
`RESTORE-HASH-MISMATCH-010`.

**Files:**

- Create: `crates/restore/Cargo.toml`
- Create: `crates/restore/src/lib.rs`
- Create: `crates/restore/src/error.rs`
- Create: `crates/restore/src/plan.rs`
- Create: `crates/restore/src/stream.rs`
- Create: `crates/restore/tests/streaming_restore.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `docs/adr/0026-bounded-content-plan-and-partial-recovery.md`
- Modify: `docs/adr/README.md`
- Modify: `docs/traceability-matrix.md`

### Step 1: Write failing planner tests

Create deterministic candidates for contiguous, fragmented, resident, sparse,
gapped, currently allocated, out-of-volume, read-failed, zeroed, overlapping,
and unordered extents.

The public plan contract is:

```rust
pub const MAX_STREAM_BUFFER_BYTES: usize = 1024 * 1024;

pub enum PartialPolicy {
    CompleteOnly,
    ZeroFillAndMap,
}

pub enum ZeroFillReason {
    Sparse,
    MissingExtent,
    CurrentlyAllocated,
    OutOfVolume,
    PreviouslyReadFailed,
    Zeroed,
    UnknownAvailability,
}

pub enum ContentSegment {
    Read {
        logical_offset: u64,
        physical_offset: u64,
        len: u64,
    },
    Zero {
        logical_offset: u64,
        len: u64,
        reason: ZeroFillReason,
    },
}

pub struct ContentPlan {
    pub candidate_id: um_core::CandidateId,
    pub logical_size: u64,
    pub segments: Vec<ContentSegment>,
    pub expected_sha256: Option<[u8; 32]>,
    pub requires_best_effort: bool,
}
```

`CompleteOnly` rejects any non-sparse unavailable range. `ZeroFillAndMap`
preserves logical length and emits exact reasons. Sparse ranges are intentional
zeros and do not make the result partial. Overlaps, overflow, out-of-order
coverage, excessive segment counts, and physical reads outside source length
must fail closed.

Run:

```powershell
cargo test -p um-restore plan_
```

Expected: FAIL because `um-restore` does not exist.

### Step 2: Implement the bounded planner

Implement `plan_candidate(candidate, source_len, expected_sha256, policy,
limits)`. Sort by logical offset, reject every overlap or duplicate
deterministically, create explicit logical gaps, coalesce adjacent segments
only when semantics and physical continuity match, and enforce a fixed maximum
segment count. Directories produce no content plan.

Never allocate a buffer proportional to file size.

### Step 3: Write failing streaming tests

Use an in-memory `SourceReader` double whose reads, injected failures, and
maximum requested length are observable. Define:

```rust
pub trait CancellationProbe: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

pub trait ProgressSink {
    fn advanced(&mut self, progress: StreamProgress);
}
```

Test:

- exact SHA-256 for contiguous, fragmented, and resident content;
- sparse output contains zeros at the correct logical offsets;
- injected short/unreadable subranges become exact zero-fill evidence only
  under `ZeroFillAndMap`;
- cancellation is observed between bounded chunks;
- the largest source read and scratch buffer are at most 1 MiB for a synthetic
  multi-gigabyte logical file;
- expected carved hash match passes and mismatch fails;
- any logical overlap, including duplicated extents, fails closed before the
  first read;
- progress is based only on committed streamed bytes and never simulated.

Run:

```powershell
cargo test -p um-restore stream_
```

Expected: FAIL because streaming is absent.

### Step 4: Implement streaming

Implement:

```rust
pub fn stream_candidate<W: std::io::Write>(
    source: &dyn um_core::SourceReader,
    plan: &ContentPlan,
    output: &mut W,
    scratch: &mut [u8],
    cancel: &dyn CancellationProbe,
    progress: &mut dyn ProgressSink,
) -> Result<StreamOutcome, RestoreError>;
```

Require `1..=MAX_STREAM_BUFFER_BYTES` scratch, split reads/writes into scratch
chunks, use `read_best_effort_at` for recoverable read faults, hash exactly the
bytes written, preserve logical length, and report all zero-filled ranges.
Expected content hash mismatch returns an error before any caller may publish
the output.

### Step 5: Document and verify

ADR-0026 records the logical segment model, memory bound, partial policy, and
hash semantics. Update traceability only for passing tests.

Run:

```powershell
cargo fmt --all
cargo clippy -p um-restore --all-targets -- -D warnings
cargo test -p um-restore
```

Expected: PASS.

### Step 6: Commit

```powershell
git add Cargo.toml Cargo.lock crates/restore docs/adr/0026-bounded-content-plan-and-partial-recovery.md docs/adr/README.md docs/traceability-matrix.md
git commit -m "feat: add bounded restore streaming"
```

---

## Task 3: Physical-disk identity and opaque destination authority

**Requirements:** `RESTORE-DIFFERENT-DISK-018`,
`RESTORE-SAME-DISK-REJECT-019`, `RESTORE-UNKNOWN-DISK-REJECT-020`,
`RESTORE-SOURCE-CHANGED-021`.

**Files:**

- Modify: `crates/io-windows/src/model.rs`
- Modify: `crates/io-windows/src/windows.rs`
- Modify: `crates/io-windows/src/unsupported.rs`
- Modify: `crates/io-windows/src/lib.rs`
- Modify: `crates/broker-protocol/src/message.rs`
- Modify: `crates/broker-protocol/tests/schema.rs`
- Modify: `crates/broker-protocol/tests/protocol.rs`
- Modify: `crates/broker-client/src/lib.rs`
- Modify: `crates/broker-client/tests/protocol_session.rs`
- Modify: `crates/broker-client/tests/source_reader.rs`
- Modify: `crates/elevated-broker/src/lib.rs`
- Modify: `crates/elevated-broker/tests/session.rs`
- Modify: `apps/desktop/src-tauri/src/storage.rs`
- Modify: `docs/adr/0023-windows-read-only-broker-and-folder-scope.md`
- Create: `docs/adr/0027-destination-capability-and-disk-separation.md`
- Modify: `docs/adr/README.md`
- Modify: `docs/traceability-matrix.md`

### Step 1: Write failing protocol and pure-policy tests

Amend the broker `Opened` response with a Rust/native-only
`physical_disk_number: u32`. Add schema round-trip and malformed-message tests.
Add pure policy tests for:

- known single source disk and different known single destination disk: allow;
- same disk: reject;
- either identity unknown: reject;
- multi-disk destination: reject;
- ReFS, FAT, exFAT, or unknown destination filesystem: reject;
- source identity changed since scan: reject.

The WebView DTO tests must prove no physical disk number is serialized across
the Tauri boundary.

Run:

```powershell
cargo test -p um-broker-protocol -p um-broker-client -p um-elevated-broker
cargo test -p um-desktop disk_
```

Expected: FAIL because the open response and comparison policy lack identity.

### Step 2: Propagate source disk identity

Populate the hardcoded single-disk identity already established by the broker's
bounded `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` query. Propagate it through
`OpenedSource`/`BrokerSourceReader` as a Rust-only accessor and atomically
extend `ScanSourceBinding` with `physical_disk_number: u32`.

Do not add a caller-chosen device path, desired access, IOCTL, or arbitrary
process API.

### Step 3: Write failing destination binding tests

Add tests around a private `PhysicalDiskSet` and a native destination-root
opener. Use pure parser/policy inputs and filesystem doubles only; no test
writes a real volume.

The Windows opener must return a private, non-serializable binding with:

- an owned query-only directory handle opened without delete sharing;
- final volume GUID identity;
- sanitized label/filesystem/free bytes;
- known physical-disk set;
- reparse safety evidence;
- no serialization.

### Step 4: Implement query-only destination-root authority

Implement `open_destination_root_binding(&Path)` inside `um-io-windows`.
This API only opens and queries the selected root; it never creates, removes,
renames, truncates, or writes a destination entry. Reuse the existing
folder-scope primitives in this order:

1. reject remote/unsupported drive types;
2. open the selected folder with `FILE_READ_ATTRIBUTES |
   FILE_LIST_DIRECTORY`, share read/write but deliberately not delete,
   `OPEN_EXISTING`, `FILE_FLAG_BACKUP_SEMANTICS`, and
   `FILE_FLAG_OPEN_REPARSE_POINT`;
3. query the opened object and reject a reparse root or non-directory;
4. retain that exact owned `std::fs::File` handle for the authority lifetime;
5. derive the final volume GUID from the same retained handle;
6. open that fixed derived GUID with desired access `0`, share
   read/write/delete, `OPEN_EXISTING`;
7. use existing query-only volume information and bounded disk-extent parsing.

The absence of delete sharing prevents substitution/rename of the retained
root on Windows. Task 4 consumes this exact handle as a capability; it never
reopens the picker path. This removes the validation/open race.

For this increment, destination filesystem authority is accepted only when the
queried filesystem is NTFS. ReFS/FAT/exFAT/unknown destinations fail with a
structured unsupported-destination error because Task 4's atomic no-clobber
publication depends on same-filesystem hard links. There is no path-based or
overwrite-capable fallback.

All `unsafe` remains in `windows.rs`. The non-Windows implementation returns a
structured unsupported error.

### Step 5: Document and verify

Amend ADR-0023's closed API inventory and add ADR-0027 for opaque destination
authority, same-disk rejection, expiry/revalidation, and failure on unknown or
multi-disk identity.

Run:

```powershell
cargo fmt --all
cargo clippy -p um-io-windows -p um-broker-protocol -p um-broker-client -p um-elevated-broker --all-targets -- -D warnings
cargo test -p um-io-windows -p um-broker-protocol -p um-broker-client -p um-elevated-broker -p um-desktop
```

Expected: PASS.

### Step 6: Commit

```powershell
git add crates/io-windows crates/broker-protocol crates/broker-client crates/elevated-broker apps/desktop/src-tauri/src/storage.rs docs/adr/0023-windows-read-only-broker-and-folder-scope.md docs/adr/0027-destination-capability-and-disk-separation.md docs/adr/README.md docs/traceability-matrix.md
git commit -m "feat: bind source and destination disk identity"
```

---

## Task 4: Safe paths, transactional publication, and recovery manifest

**Requirements:** `RESTORE-PATH-TRAVERSAL-011`,
`RESTORE-PATH-WINDOWS-RESERVED-012`, `RESTORE-PATH-REPARSE-013`,
`RESTORE-COLLISION-RENAME-014`, `RESTORE-NO-CLOBBER-015`,
`RESTORE-TEMP-PUBLICATION-016`, `RESTORE-MANIFEST-017`,
`RESTORE-JOURNAL-DURABILITY-027`, `RESTORE-REPARSE-RACE-028`.

**Files:**

- Create: `crates/restore/src/path.rs`
- Create: `crates/restore/src/transaction.rs`
- Create: `crates/restore/src/journal.rs`
- Create: `crates/restore/src/manifest.rs`
- Modify: `crates/restore/src/lib.rs`
- Modify: `crates/restore/Cargo.toml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `crates/restore/tests/path_safety.rs`
- Create: `crates/restore/tests/transactional_restore.rs`
- Create: `crates/restore/tests/reparse_race_windows.rs`
- Modify: `docs/adr/0027-destination-capability-and-disk-separation.md`
- Modify: `docs/traceability-matrix.md`

### Step 1: Write failing path-security tests

Create table-driven tests for empty components, `.`, `..`, absolute paths,
drive prefixes, UNC paths, slash/backslash injection, NUL/control characters,
alternate data streams, trailing dot/space, reserved Windows names with and
without extensions, overlong components, excessive depth, and excessive total
path length.

Test that selected files create only sanitized ancestors; selecting a directory
does not imply historical descendants.

Run:

```powershell
cargo test -p um-restore path_
```

Expected: FAIL because safe path derivation is absent.

### Step 2: Implement typed relative paths

Create private-field `SafeRelativePath` and `SafeComponent` types. Construction
performs every validation once; transaction code accepts only those types.
Use a deterministic escaped fallback for unsafe recovered names while
preserving the original untrusted name only in a JSON string field in the
manifest.

### Step 3: Write failing transaction tests

Against `tempfile::TempDir`, prove:

- output starts under a unique `.umrecovering` name;
- failure or cancellation never exposes an incomplete final file;
- successful output is flushed, `sync_all`ed, length/hash verified, then
  published;
- an existing destination file remains byte-for-byte unchanged;
- collisions produce deterministic `name (recovered N).ext` variants within a
  10,000-attempt bound;
- a concurrent creator racing every final-name attempt wins or loses atomically
  without either file being replaced;
- on Windows, the open temporary file handle prevents rename/delete
  substitution until hard-link publication completes; a regression attempts
  both operations from a competing thread and must fail safely;
- a destination that rejects hard links fails before a final name is
  published and does not fall back to replacement rename or copy;
- a reparse/symlink encountered below the authority root is rejected atomically
  by a no-follow component open;
- replacing a checked component with a symlink/junction between operations
  cannot redirect a create/open outside the retained root;
- a partial file emits an exact sidecar range map;
- a versioned append-only job journal records prepared, published, failed,
  cancelled, and terminal records with monotonic sequence and hash chaining;
- each journal record is flushed and `sync_all`ed at its durability boundary;
- injected journal write/flush/sync failures prevent subsequent publication,
  or, if publication already happened, leave a durable prepared record that
  makes the outcome explicitly reconcilable;
- the final manifest is versioned, internally consistent, and its SHA-256
  matches the returned summary;
- no allocation is proportional to candidate size.

Run:

```powershell
cargo test -p um-restore transaction_
```

Expected: FAIL because transactional publication is absent.

### Step 4: Implement capability-relative transaction, journal, and manifest

Add reviewed, pinned `cap-std = 4.0.2` and `cap-fs-ext = 4.0.2` workspace
dependencies. Add Windows-only `junction = 2.0.0` as a dev dependency for a
real temporary NTFS junction race regression that requires neither elevation
nor Developer Mode.

Implement private `DestinationRoot`, `FileRestorePlan`, `JobJournal`, and
`RestoreItemResult`. `DestinationRoot` is constructed only by consuming the
retained `std::fs::File` from Task 3 into `cap_std::fs::Dir`; production code
has no ambient-path constructor. Walk/create one sanitized component at a time
relative to the currently retained directory capability. Existing directories
are opened with `cap_fs_ext::DirExt::open_dir_nofollow`; newly created
directories are immediately rebound through the same no-follow open. Final
files use capability-relative `OpenOptions` with `create_new(true)` and
`OpenOptionsFollowExt::follow(FollowSymlinks::No)`.

The Windows race test repeatedly swaps a child directory for a real junction
to an outside temp directory while recovery attempts to descend. Recovery may
fail safely, but the outside sentinel and directory must remain byte-for-byte
unchanged and receive no created entry. A cross-platform scripted capability
double deterministically injects replacement at each transition so the
security test never depends only on timing.

Revalidate root identity through the retained handle before the job. Stream
with the Task 2 engine and a fixed 1 MiB scratch buffer. Publish with a
capability-relative atomic hard link:

```rust
parent_dir.hard_link(&temporary_name, &parent_dir, &final_name)
```

`Dir::hard_link`/the operating-system link primitive fails atomically when the
final name already exists. On collision, derive the next bounded name and
retry; never pre-check then rename. After a successful link, sync the parent
directory where supported, append/sync `ItemPublished`, then remove only the
temporary link created by this job. A cleanup failure is recorded but never
causes the final link to be replaced or removed.

Keep the capability-created temporary file handle open through publication.
The pinned capability implementation opens Windows path components without
delete sharing to prevent namespace substitution; the Windows regression above
is a release-blocking guard on that property. Immediately before linking,
compare the still-open temporary handle's file identity with the
capability-relative no-follow name lookup; any mismatch fails closed.

Only NTFS destination authorities from Task 3 reach this primitive.
`ERROR_NOT_SUPPORTED`, cross-device, cloud-placeholder, or other hard-link
failures fail the item/job explicitly. There is no `Dir::rename`,
`std::fs::rename`, overwrite, copy-to-final, or ambient-path fallback.

Create a versioned append-only JSON Lines journal with `create_new(true)`.
Records contain a monotonic sequence, previous-record SHA-256, record SHA-256,
and bounded payload. Append/sync `ItemPrepared` before publication,
`ItemPublished` immediately after publication, and explicit item/job
failure/cancellation/terminal records. A journal durability failure is a job
failure and never silently advances to another publication.

Write per-partial-file sidecars transactionally. Write the job manifest last,
also through a temporary file and no-clobber publication. Record all item
dispositions, output hashes, expected hashes, readable ranges, zero-filled
ranges/reasons, conflicts, read errors, warnings, and temporary-file
disposition.

### Step 5: Document and verify

Amend ADR-0027 with the final handle-relative/reparse-containment and
no-clobber decision. Update traceability only for passing tests.

Run:

```powershell
cargo fmt --all
cargo clippy -p um-restore --all-targets -- -D warnings
cargo test -p um-restore
```

Expected: PASS.

### Step 6: Commit

```powershell
git add Cargo.toml Cargo.lock crates/restore docs/adr/0027-destination-capability-and-disk-separation.md docs/traceability-matrix.md
git commit -m "feat: publish recovered files transactionally"
```

---

## Task 5: Tauri restore plans, jobs, cancellation, and vertical fixture

**Requirements:** `DESKTOP-RESTORE-CONTRACT-023` and the native half of
`DESKTOP-RESTORE-FLOW-024`,
`DESKTOP-RESTORE-OPEN-DESTINATION-026`.

**Files:**

- Create: `apps/desktop/src-tauri/src/restore.rs`
- Modify: `apps/desktop/src-tauri/src/storage.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Create: `apps/desktop/src-tauri/tests/restore_fixture.rs`
- Modify: `apps/desktop/src/api/storage.ts`
- Modify: `apps/desktop/src/api/storage.test.ts`
- Modify: `apps/desktop/src/api/storageDesktop.ts`
- Modify: `apps/desktop/src/api/storageDesktop.test.ts`
- Create: `docs/adr/0028-restore-plan-job-and-manifest-lifecycle.md`
- Modify: `docs/adr/README.md`
- Modify: `docs/traceability-matrix.md`

### Step 1: Write failing native coordinator tests

Test bounded native maps and immutable binding:

- at most 32 destination authorities and eight retained plans/jobs;
- destination cancellation creates no authority;
- plan is bound to scan/source, destination, selection revision, ordered item
  descriptors, collision policy, partial policy, and SHA-256 plan digest;
- stale selection or changed source/destination identity invalidates planning
  or start;
- an ineligible item blocks plan creation with a structured error;
- best-effort items require `zeroFillAndMap`;
- `start_restore` returns a job immediately and work uses real counters;
- one atomic cancellation token is idempotent and cooperatively observed;
- only terminal jobs expose a manifest summary;
- `open_restore_destination` accepts only request/job IDs, requires a completed
  job, revalidates its native authority, and opens only the job directory.

Run:

```powershell
cargo test -p um-desktop restore_
```

Expected: FAIL because coordinator commands are absent.

### Step 2: Implement opaque native state and commands

Add bounded stores for `DestinationAuthority`, `RestorePlan`, and
`RestoreJob`. Register:

```text
select_restore_destination
create_restore_plan
start_restore
get_restore_job
cancel_restore
open_restore_destination
```

Only the first command may invoke the native folder picker. No command accepts
or returns a path, extent, offset, disk number, handle, executable, or recovered
bytes. `open_restore_destination` uses the fixed Windows shell operation on the
already job-bound directory, never a caller-selected executable/path.

Worker execution reopens/revalidates the source through the broker, revalidates
destination authority/disk separation, uses `um-restore`, and publishes status
snapshots from actual item/byte work.

### Step 3: Add failing TypeScript contract tests

Add exact parsers/wrappers for:

```ts
selectRestoreDestination(requestId, scanId)
createRestorePlan(
  requestId,
  scanId,
  selectionRevision,
  destinationId,
  "rename",
  partialFilePolicy,
)
startRestore(requestId, planId)
getRestoreJob(requestId, jobId)
cancelRestore(requestId, jobId)
openRestoreDestination(requestId, jobId)
```

Reject unknown keys, invalid status transitions, non-decimal counts, paths,
extents, offsets, handles, executables, and recovered bytes. Enforce that a
destination relation is only `"different"`.

Run:

```powershell
pnpm --dir apps/desktop test -- storage.test.ts storageDesktop.test.ts
```

Expected: FAIL before implementation, then PASS.

### Step 4: Add the failing vertical deterministic test

Use `um-fixture-builder` to create an NTFS image containing:

- one intact deleted file with known content/hash;
- one fragmented deleted file with known content/hash;
- one incomplete/conflicted file with a known zero-fill expected output and
  exact range map.

The test must pass through the same retained scan candidate, native selection,
plan, job, and restore engine state used by Tauri commands, with a temp
destination and disk-identity doubles. Assert output SHA-256, no-clobber
behavior, sidecar ranges, and manifest SHA-256. Do not open a real volume.

Run:

```powershell
cargo test -p um-desktop --test restore_fixture
```

Expected: FAIL until the vertical path is wired; then PASS.

### Step 5: Document and verify

ADR-0028 records plan immutability, job bounds, progress/cancellation,
manifest durability, and safe shell-open behavior. Update traceability.

Run:

```powershell
cargo fmt --all
cargo clippy -p um-desktop --all-targets -- -D warnings
cargo test -p um-desktop
pnpm --dir apps/desktop test -- storage.test.ts storageDesktop.test.ts
```

Expected: PASS.

### Step 6: Commit

```powershell
git add apps/desktop/src-tauri apps/desktop/src/api docs/adr/0028-restore-plan-job-and-manifest-lifecycle.md docs/adr/README.md docs/traceability-matrix.md
git commit -m "feat: add native restore plans and jobs"
```

---

## Task 6: Actionable results workspace and real restore UI

**Requirements:** `RESULT-LATE-RESPONSE-008`,
`DESKTOP-RESULT-CONTROLS-022`, `DESKTOP-RESTORE-FLOW-024`,
`DESKTOP-RESTORE-A11Y-025`,
`DESKTOP-RESTORE-OPEN-DESTINATION-026`.

**Files:**

- Create: `apps/desktop/src/state/resultsWorkspace.ts`
- Create: `apps/desktop/src/state/resultsWorkspace.test.ts`
- Create: `apps/desktop/src/state/restoreWorkflow.ts`
- Create: `apps/desktop/src/state/restoreWorkflow.test.ts`
- Create: `apps/desktop/src/components/results/ResultsWorkspace.tsx`
- Create: `apps/desktop/src/components/results/ResultsFilters.tsx`
- Create: `apps/desktop/src/components/results/CandidateResultsTable.tsx`
- Create: `apps/desktop/src/components/results/SelectionBar.tsx`
- Create: `apps/desktop/src/components/results/RestoreWorkflowDialog.tsx`
- Modify: `apps/desktop/src/state/storageScan.ts`
- Modify: `apps/desktop/src/views/AnalysisView.tsx`
- Modify: `apps/desktop/src/views/AnalysisView.test.tsx`
- Modify: `apps/desktop/src/App.tsx`
- Modify: `apps/desktop/src/App.test.tsx`
- Modify: `apps/desktop/src/i18n/messages.ts`
- Modify: `apps/desktop/src/views/HelpView.tsx`
- Modify: `apps/desktop/src/styles/global.css`
- Modify: `apps/desktop/src/csp-styles.test.js`
- Modify: `apps/desktop/src/production-surface.test.ts`
- Modify: `apps/desktop/package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `docs/traceability-matrix.md`

### Step 1: Write failing results-state tests

`useStorageScan` must stop appending candidate pages. Add reducer/hook tests
for a separate results workspace:

```ts
interface ResultsWorkspaceState {
  phase: "idle" | "loading" | "ready" | "error";
  scanId: string | null;
  query: CandidateQuery;
  sort: CandidateSort;
  requestRevision: number;
  currentPageIndex: number;
  cursorHistory: Array<string | null>;
  pageCache: CachedCandidatePage[]; // maximum 3
  page: CandidateQueryPage | null;  // only rendered page
  selectionPhase: "idle" | "updating";
  error: StorageErrorCode | null;
}
```

Prove query/sort reset to cursor `null`, late responses are ignored, next and
previous preserve cursor identity, the fourth cached page evicts the LRU page,
only one page is exposed to rendering, and selection is accepted only from
native responses.

Run:

```powershell
pnpm --dir apps/desktop test -- resultsWorkspace.test.ts
```

Expected: FAIL because the hook is absent.

### Step 2: Implement bounded results state

Create `useResultsWorkspace(runtimeAvailable, scanId)`. It exposes explicit
search submission, facet/filter toggles, score range, selected-only, stable
sort toggles, page navigation, retry, row selection, select-all-matching,
clear-matching, and clear-all. Keep at most three cached pages and render one
page of at most 100 rows. Never optimistically invent selection authority.

### Step 3: Write failing component and accessibility tests

With complete native DTOs mocked only at the Tauri `invoke` boundary, prove:

- search is an explicitly submitted `role="search"` form;
- extension facets/counts come from the backend;
- filter controls have programmatic labels;
- sortable headers expose/update `aria-sort`;
- paths remain in `<bdi dir="auto">`;
- previous/next disabled state matches real cursors;
- header checkbox represents the whole filtered result using
  `matchingSelectedCandidates` and `filteredTotal`;
- select-all sends one native operation, never all candidate IDs;
- selection persists through query, sort, and page changes;
- sticky summary shows global counts/bytes/best-effort/conflicts/ineligible;
- the DOM never contains more than 100 candidate rows;
- keyboard focus remains predictable when sorting/filtering changes rows.

Run:

```powershell
pnpm --dir apps/desktop test -- AnalysisView.test.tsx
```

Expected: FAIL because controls and selection do not exist.

### Step 4: Implement the real results UI

Extract the current results block into the listed components. Preserve the
existing design tokens and visual language. Add no mockup, sample row, fake
facet, simulated progress, disabled placeholder, or browser fallback.

Use semantic tables and inputs. Render `aria-rowcount` from the backend total.
An ineligible candidate remains inspectable/selectable, but disables recovery
until cleared. A page is bounded enough that no virtualization dependency is
needed.

### Step 5: Write failing restore-workflow tests

Model a discriminated workflow:

```ts
type RestoreWorkflowPhase =
  | "idle"
  | "selectingDestination"
  | "setup"
  | "planning"
  | "review"
  | "starting"
  | "active"
  | "finished"
  | "error";
```

Prove:

- picker cancellation returns to results without an error or plan;
- best-effort cannot proceed without `zeroFillAndMap` and explicit consent;
- destination -> plan review -> start uses exact opaque IDs/revisions;
- a changed selection revision invalidates plan review;
- displayed item/byte progress equals native snapshots and uses `BigInt`
  basis-point calculation, never local timers;
- polling stops at terminal state/unmount;
- cancel sends one idempotent command and displays `cancelling`;
- completed/partial/failed counts and manifest hash are visible;
- opening the destination sends only `requestId` and `jobId`;
- no source/destination path, extent, offset, handle, executable, recovered
  byte, or recovered-file execution enters the WebView.

Run:

```powershell
pnpm --dir apps/desktop test -- restoreWorkflow.test.ts App.test.tsx production-surface.test.ts
```

Expected: FAIL before implementation.

### Step 6: Implement restore workflow and bounded styling

Implement `useRestoreWorkflow` and the four dialog steps: destination setup,
plan review, real progress/cancel, and completion. Use native snapshots as the
only progress source. Offer only `Open destination folder` on completion.

Add responsive filter/sidebar, scroll-bounded table, sticky headers/selection
column/selection bar, focus-visible treatment, forced-colors support, and
reduced-motion behavior. Use `<progress>` rather than dynamic inline widths so
the existing CSP remains closed.

Update Help content to describe real restore boundaries and remove the obsolete
statement that recovery is unavailable.

Update `production-surface.test.ts` with an exact allowlist of the new commands
while continuing to reject mock providers, simulated progress, paths, extents,
offsets, handles, executables, and recovered bytes.

### Step 7: Verify

Run:

```powershell
pnpm --dir apps/desktop lint
pnpm --dir apps/desktop typecheck
pnpm --dir apps/desktop test
```

Expected: PASS.

### Step 8: Commit

```powershell
git add apps/desktop/src apps/desktop/package.json pnpm-lock.yaml docs/traceability-matrix.md
git commit -m "feat: add actionable recovery workspace"
```

---

## Task 7: Whole-product verification, evidence, and release artifact

**Files:**

- Modify: `docs/specs/020-actionable-results-and-transactional-restore.md`
- Modify: `docs/specs/009-restore-semantics.md`
- Modify: `docs/specs/010-ux-ui-and-accessibility.md`
- Modify: `docs/specs/012-test-and-validation-plan.md`
- Modify: `docs/specs/015-known-limitations.md`
- Modify: `docs/traceability-matrix.md`
- Create: `docs/evidence/actionable-restore-2026-07-30.md`
- Modify: `README.md`

### Step 1: Run focused regression gates

```powershell
cargo test -p um-restore
cargo test -p um-desktop --test restore_fixture
pnpm --dir apps/desktop test
```

Expected: PASS. If any fail, return to the owning task with a new failing
regression test; do not weaken an assertion or mark a test skipped.

### Step 2: Run every repository-required gate

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir apps/desktop lint
pnpm --dir apps/desktop typecheck
pnpm --dir apps/desktop test
```

Expected: PASS with no skipped critical tests.

### Step 3: Build the release application

```powershell
pnpm --dir apps/desktop desktop:build
```

Expected: PASS. This repository currently sets `bundle.active = false`, so the
release artifact for this increment is the verified sibling executable pair,
not an installer:

```text
target/release/undelete-master-desktop.exe
target/release/undelete-master-broker.exe
```

The command must run `broker:release` before `tauri build`. Verify both files
exist in the same directory, the desktop and broker manifests have their
intended execution levels, and the broker path check accepts exactly this
layout. Record exact paths, sizes, and SHA-256. Do not claim an installer,
portable package, or Authenticode signing unless it is actually produced and
verified.

### Step 4: Perform bounded real-app acceptance

Run the release app unelevated and verify without destructive disk activity:

- connected local volumes are discovered by the native runtime;
- an allowed source can be scanned through the read-only broker;
- the results page can search/filter/sort/page/select real findings;
- destination selection rejects the same or unknown physical disk;
- a different-disk destination produces a real plan;
- recovery writes only below the chosen destination;
- progress/cancellation come from the native job;
- recovered outputs, partial sidecar, and final manifest match the job summary;
- opening the destination never executes a recovered file.

If no suitable different physical test disk is available, record this
real-device acceptance as unverified rather than substituting a mock. The
deterministic fixture vertical test remains mandatory and must pass.

### Step 5: Reconcile SDD and evidence

Update all listed specifications, traceability, known limitations, README, and
the evidence report with:

- requirement -> implementation -> test mappings;
- exact commands and outcomes;
- fixture names and expected/actual SHA-256;
- release artifact SHA-256;
- supported metadata-backed file coverage;
- explicit limitations: NTFS-only restore destinations,
  overwritten/TRIMed/encrypted/compressed content,
  unsupported deep-carving formats, live-volume consistency, and any
  unperformed real-device acceptance.

Never describe best-effort output as intact and never claim “all file types”
for deep carving. State precisely that metadata-backed restore is extension
agnostic while deep carving remains format-plugin based.

### Step 6: Commit

```powershell
git add README.md docs
git commit -m "docs: record actionable restore evidence"
```

### Step 7: Final review and publication

Run a fresh whole-branch specification/security/code-quality review. Resolve
all findings with focused regression tests and rerun the full gate. Then use
the repository's configured GitHub remote to push
`codex/actionable-restore`. Do not force-push. Create or update a pull request
only after the push succeeds.
