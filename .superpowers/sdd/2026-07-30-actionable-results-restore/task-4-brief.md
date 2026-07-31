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
directory where supported and append/sync `ItemPublished`. Security review
supersedes the original path-cleanup step: protected `.umrecovering` links are
retained and truthfully manifested because the current safe capability API has
no identity-atomic unlink primitive. There is no close-then-delete-by-path
fallback, which would introduce a substitution race.

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
