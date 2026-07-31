# ADR-0027 — Destination Capability and Physical-disk Separation

Status: Accepted
Decision date: 2026-07-30

Implementation status: `Implemented-unverified`. Tasks 3 and 4 implement
source physical-disk propagation, destination-root admission, pure separation
policy, handle-relative path containment, transactional no-clobber
publication, the durable job journal, partial sidecars, and the final manifest.
Tasks 5–7 still own coordinator integration, desktop destination maps, the
user workflow, vertical fixture acceptance, packaging, and release evidence.

## Context

Recovery must not write to the same physical disk as the scan source because a
destination write could overwrite bytes that remain recoverable. Drive letters,
mount labels, picker paths, and unelevated display inventory do not prove a
physical-disk relationship. Unknown and composite mappings must therefore fail
closed.

Task 4 also needs an authority that survives validation without reopening a
mutable picker path. The retained object must be usable by capability-relative
code while native paths, volume GUIDs, physical disk numbers, extents, and
handles remain outside the WebView.

Task 3 performs query-only admission. Destination creation, publication, and
all mutation remain outside `crates/io-windows` and the elevated broker.

## Decision

### Authoritative source identity

The elevated broker already opens a source by opaque volume identity, queries
the live handle with the fixed
`IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`, rejects zero and multi-disk mappings,
and retains the complete extent vector for due revalidation. Disk number alone
is insufficient for an attached VHD/VHDX because Windows assigns that
file-backed device its own number. The broker therefore also issues the fixed
`StorageDeviceProperty` query and admits only the reviewed direct ATA, SATA,
USB, or NVMe bus classes. Virtual, file-backed virtual, Storage Spaces, SCSI,
RAID, network/array, unknown, and future bus values fail closed. Task 3 binds
the resulting sole authoritative `u32` disk number to `RawVolume`, propagates
it through `BrokerSourceGeometry`, and returns it in `Message::Opened`.

Adding this fixed field changes the wire shape. The current broker protocol is
version 3. A v2 header is rejected as unsupported; mixed v2/v3 desktop and
broker binaries never reinterpret the expanded `Opened` payload. The protocol
still has the same ten-message allowlist, 20-byte frame header, one-megabyte
payload cap, monotonic sequences, and no path, access-mask, generic control, or
mutation field.

`BrokerSourceReader` exposes the disk number only to native Rust. The scan
session atomically retains it beside the opaque volume ID, inventory
generation, canonical source length, and filesystem. Before a later restore,
the source must be reopened through the same broker path and the new
authoritative number must equal the scan-bound number. Missing or changed
identity rejects the operation.

### Opaque destination-root authority

`open_destination_root_binding(&Path)` accepts only a Rust-owned native picker
selection. It executes this fixed sequence:

1. classify the drive root and reject remote, optical, RAM-backed, unknown, and
   unsupported roots;
2. open the selected directory with exactly
   `FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY`, read/write sharing without
   delete sharing, `OPEN_EXISTING`, and
   `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT`;
3. query that exact handle and reject a non-directory or reparse root;
4. derive the final volume GUID root from that same handle;
5. open only that internally derived volume GUID with desired access `0`,
   read/write/delete sharing, and `OPEN_EXISTING`;
6. query volume serial, sanitized label/filesystem, free bytes, the bounded
   fixed disk-extents IOCTL, and the fixed storage-device bus property;
7. require the directory and volume serials to agree, require one known disk
   with a reviewed direct bus class, and accept only NTFS; and
8. retain the original directory as an owned `std::fs::File`.

Omitting delete sharing from the retained root handle prevents a competing
rename/delete open from substituting that root during the authority lifetime.
Task 4 consumes this exact `std::fs::File` into `cap_std::fs::Dir`; it never
reopens the picker path.

The public binding has private fields and implements neither serialization nor
cloning. It exposes only sanitized native metadata, the validated single disk
number for native policy, and a consuming transfer of the retained
`std::fs::File`. It exposes no picker path, final volume GUID, extent list, or
raw handle.

No destination file or directory is created, removed, renamed, truncated, or
written by Task 3. There is no path-based, overwrite-capable, or unsupported
filesystem fallback.

### Capability-relative restore and no-clobber publication

Task 4 consumes the exact retained `std::fs::File` into
`cap_std::fs::Dir`. There is no ambient-path constructor in
`crates/restore`. Construction captures the root `(dev, ino)` identity through
safe `cap_fs_ext::MetadataExt` access and revalidates that same retained handle
before starting a job.

Recovered relative paths are typed and immutable after validation. Empty,
absolute/prefixed, separator-bearing, traversal, control-character,
alternate-data-stream, trailing-dot/space, overlong, excessive-depth, and
Windows reserved-device components are rejected. Unsafe recovered names use a
deterministic SHA-256 fallback; the original untrusted spelling appears only
as JSON evidence.

The job creates one deterministic, no-clobber job directory and a
`create_new` JSON Lines journal. Descendant directories are walked one
component at a time from the currently retained capability. Existing
directories use `open_dir_nofollow`; newly created directories are immediately
rebound through the same no-follow operation. Final data, sidecar, journal,
and manifest operations never reopen an ambient descendant path.

Each selected file is streamed from `SourceReader` into a unique
`.umrecovering` entry with a fixed 1 MiB scratch buffer. On Windows the
capability open combines `create_new`, no-follow, and `maybe_dir(true)`, then
requires a regular file. This retains the temporary handle without delete
sharing. After flush, `sync_all`, length verification, and expected/actual
SHA-256 validation, the still-open handle identity must match a new
capability-relative no-follow name lookup.

Publication uses only a capability-relative hard link. Partial sidecars are
keyed by immutable plan index plus candidate ID, prepared and linked once, and
durably journaled before any data-name attempt. Data collisions never retract
or republish that sidecar. A final-name race fails atomically; the deterministic
`name (recovered N).ext` policy retries at most 10,000 times. Unsupported hard
links fail the item. There is no rename, overwrite, copy-to-final, or ambient
fallback. The final manifest uses the same temporary-file and hard-link
protocol and never replaces an existing entry.

Directory `sync_all` evidence is reported as `Synced`, `Unsupported`, or
`Failed`; `Unconfirmed` is used only when a visible directory has not reached
that sync boundary. On Windows, only access-denied on the expected read/query
directory handle and the two documented unsupported-function codes are
classified as `Unsupported`; invalid handle, I/O/device, parameter, disk-full,
and other errors are `Failed`. Unsupported or failed namespace durability
retains temporary links and marks the result `NeedsReconciliation`.

Protected temporary links are also retained after a synchronized publication
and reported as `RetainedBySafeCleanupPolicy`. Closing the no-delete-share
handle and then deleting by path would introduce a substitution race, while
the current safe capability API has no identity-atomic unlink operation.
Consequently Task 4 performs no production path-based temporary cleanup. A
later cleanup design must supply and audit an identity-atomic primitive before
this retention policy may change.

The version-1 append-only journal has bounded canonical JSON payloads,
bounded record count, monotonic sequence numbers, one fixed hashed job
identity, previous-record and record SHA-256 values, and exact canonical-byte
audit checks. Audit rejects alternate whitespace/key ordering, unknown
envelope fields, torn tails, and oversized or cross-job records. Its chain
advances only after write, flush, and `sync_all`. Any durability failure
poisons the journal. `ItemPrepared` must be durable before linking; a journal
failure before that point prevents publication. The sidecar/data link through
its durable publication record is non-cancellable. A journal failure after a
link preserves final and temporary evidence and returns an explicit
reconciliation-needed result.

The manifest is prepared from a frozen journal outcomes-prefix hash, written
last, and returned with its literal SHA-256. With a healthy journal, ordinary
item completion, failure, or cancellation proceeds to manifest publication;
every successfully published final manifest contains exactly the immutable
plan item count, using `Published`, `Failed`, `Cancelled`, `NotAttempted`,
`DirectoryCreated`, or `DirectoryNeedsReconciliation` dispositions. Manifest
preparation or no-clobber publication may itself fail explicitly and never
replaces an existing manifest. A poisoned journal suppresses the manifest
rather than inventing outcomes beyond the durable prefix. The manifest records requested/safe paths,
output and expected hashes, readable and zero-filled ranges with reasons,
conflicts, read failures, warnings, sidecar item key/path and SHA-256,
temporary disposition, namespace durability, completion status, and
path-substitution evidence.

Original untrusted path evidence is bounded before cloning or serialization:
256 KiB per item and 8 MiB in aggregate per restore job. The immutable job
constructor also enforces one million sanitized path components using checked
addition.

A directory-only plan item carries no content plan and validates its path
evidence before any create. Because directories have no safe no-clobber
hard-link publication primitive, direct `create_dir` is their publication
boundary. Before each collision candidate the journal durably records
`directoryPublicationPlanned`, binding the item key/kind and exact
collision-resolved path/name. This record precedes the direct create, so a
poisoned post-create journal still leaves a deterministic reconciliation
candidate without claiming the create completed.

After create, Task 4 immediately no-follow binds the visible name, compares
the bound handle identity with a capability-relative no-follow name lookup,
and keeps that bound capability alive through parent sync and durable
`ItemPublished`. Any transition, bind, or identity failure after create
preserves the namespace entry and, while the journal remains healthy, records
`directoryReconciliationRequired`. Its manifest outcome is
`DirectoryNeedsReconciliation`, not plain `Failed`, and binds the actual path
and name, item key/kind, no-follow-bind flag, validation state, namespace
durability, and terminal reconciliation status. A bound directory with
`Unsupported` or `Failed` namespace sync has the same reconciliation
disposition. Only a bound, identity-validated, synchronized directory is
`DirectoryCreated`.

There is no directory path rollback or cleanup after direct create because a
substituted entry could otherwise be deleted. A poisoned reconciliation or
publication record stops the job, suppresses the final manifest, and preserves
the durable pre-create candidate prefix. Directory manifest/result evidence
omits file-only length, SHA-256, and temporary-file disposition fields. The
operation creates no historical child/sibling and performs no source read,
data temporary publication, or sidecar creation.

Cross-platform scripted transition injection replaces every newly created
component between create and rebind. Windows acceptance also races real
disposable NTFS junctions and competing temporary-file rename/delete attempts.
All such tests use `TempDir` destinations and synthetic in-memory sources; no
scan source or real disk namespace is mutated.

### Separation and expiry policy

The source-at-scan, source-at-restore, and destination identities must all be
known single disks. The source identities must be equal and the destination
disk must differ. The following are structured failures:

- source identity missing;
- source identity changed since scan;
- destination disk mapping missing;
- destination mapping contains more than one distinct disk;
- destination backing is virtual, file-backed, Storage Spaces, array/network,
  unknown, future, or otherwise unproven;
- destination disk equals the source disk; or
- destination filesystem is ReFS, FAT, exFAT, empty, or otherwise not NTFS.

Disk number zero is valid and is never used as an unknown sentinel.

The retained handle is the authority. Any later desktop destination ID is only
an opaque lookup key and must expire with its bounded native entry. Task 5 must
bound retained authorities to 32, bind them to the scan/source generation, and
reject eviction, expiry, or revalidation mismatch. Task 4 must revalidate the
retained root identity before work and use only capability-relative,
no-follow operations below it. Those later tasks may strengthen revalidation;
they may not reopen an ambient picker path.

## Audited Windows API surface

This decision adds no Windows function, IOCTL, process API, registry API, or
network API to ADR-0023's closed allowlist. It authorizes new fixed uses of the
existing query-only surface:

- `CreateFileW` for the destination root with the exact query/list access and
  no delete sharing stated above;
- `GetFileInformationByHandle` and `GetFinalPathNameByHandleW` on that retained
  root;
- `CreateFileW` with desired access `0` for only the derived volume GUID;
- `GetVolumeInformationByHandleW` for serial, label, and filesystem;
- `GetDiskFreeSpaceExW` for available/total bytes; and
- `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` plus
  `IOCTL_STORAGE_QUERY_PROPERTY` with exactly `StorageDeviceProperty` and
  `PropertyStandardQuery`, through private bounded wrappers.

The real-only desktop validator recognizes only these exact new call shapes
and retains rejection of every other access expression, share mode, create
disposition, control code, path, and Windows-system feature.

Unsafe code remains confined to `crates/io-windows/src/windows.rs`. The pure
policy, protocol, client, broker, desktop binding, and tests contain no unsafe
code. Unit and pull-request tests use value fixtures and an inert query double;
they do not open, write, or destructively query a real volume.

## Consequences

### Positive

- the source disk number comes from the broker's bounded extent query plus
  direct-bus admission rather than display inventory;
- validation and later capability use share one retained directory handle;
- same-disk, unknown, composite, changed-source, and unsupported-filesystem
  cases fail closed;
- protocol v3 makes the incompatible `Opened` expansion explicit;
- native handles and physical identities do not cross the Tauri serialization
  boundary; and
- Task 4 receives the exact capability it needs without adding destination
  authority to the elevated broker.

### Negative and residual risks

- Task 3 admits NTFS destinations only;
- a mounted destination remains a live filesystem, not a snapshot;
- disk numbers can be reused after removal, so later work must combine the
  retained handle, serial/file identity, generation, and revalidation rather
  than trusting the number alone;
- bus type is defensive admission evidence, not cryptographic proof of backing:
  a hypervisor or third-party filter can emulate a reviewed direct bus, so
  native VHD/Storage Spaces regressions and packaged-host acceptance remain
  required and such environments are not claimed safe by Task 3;
- the retained no-delete-share handle may block legitimate rename/delete until
  authority release;
- synchronized jobs retain protected `.umrecovering` links until an audited
  identity-atomic cleanup primitive exists, consuming destination space but
  avoiding path-substitution deletion;
- a directory becomes visible at its direct no-clobber create boundary; any
  later bind, validation, sync, or journal failure requires reconciliation and
  intentionally preserves that entry rather than attempting path rollback;
- Storage Spaces/dynamic/composite mappings are rejected even when a user might
  consider them acceptable; and
- no product restore exists until Tasks 5–6 integrate the Task 4 transaction
  into the coordinator and UI behavior.

ADR-0023's unsigned-development, hotplug, active-volume, exact
GUID-plus-serial-clone, and protected-installation residual risks remain.

## Rejected alternatives

- **Compare drive letters or display groups:** mutable and not physical
  identity.
- **Trust unelevated inventory disk numbers:** inventory intentionally has no
  physical authority.
- **Trust a distinct `DiskNumber` without device classification:** attached
  VHD/VHDX media receive their own number even though their backing file can be
  on the source disk.
- **Reopen the picker path after validation:** retains a validation/open race.
- **Allow unknown or multi-disk mappings:** cannot prove separation.
- **Allow same-disk with an override in this increment:** conflicts with the
  approved SDD-020 scope.
- **Allow ReFS/FAT/exFAT with rename/copy fallback:** Task 4 depends on
  capability-relative atomic hard-link publication and forbids overwrite.
- **Send disk numbers or paths to JavaScript:** makes untrusted WebView data an
  authority input.

## Related decisions

- [ADR-0002](0002-read-only-source-invariant.md): source never-write
  invariant.
- [ADR-0015](0015-same-physical-disk-policy.md): earlier proposed separation
  direction, now concretized for SDD-020.
- [ADR-0023](0023-windows-read-only-broker-and-folder-scope.md): audited
  Windows boundary and broker lifecycle.
- [ADR-0025](0025-native-result-query-and-selection-authority.md): native scan
  and selection binding.
- [ADR-0026](0026-bounded-content-plan-and-partial-recovery.md): bounded
  extraction consumed by later transaction work.

## Revisit triggers

Revisit before adding another destination filesystem, an override, an ambient
path constructor, a new Windows function or IOCTL, a caller-selected access
mask/share mode/control code, cloning destination authority, broker-side
destination access, or any destination mutation inside `crates/io-windows`.
