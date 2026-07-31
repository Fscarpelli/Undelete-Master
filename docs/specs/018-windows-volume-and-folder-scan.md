# SDD-018 — Real Windows Mounted-volume and NTFS Folder-scoped Scan

Status: `Implemented-unverified`; this is the mounted-volume scan baseline.
SDD-019 and SDD-020 extend the current product with bounded JPEG carving and
transactional restore. Local gates and the unsigned release pair are recorded;
GitHub Actions `quality` run #9 passed for pushed commit
`41b06d790d47699994a82c41eba44a0a314cc9ab`. Governed real
scanning/recovery and signing gates remain pending.

Decision date: 2026-07-29
Related decision:
[ADR-0023](../adr/0023-windows-read-only-broker-and-folder-scope.md)

## 1. Decision summary

The Windows desktop discovers mounted local volumes, presents them in honest
logical groups, lets the user select one eligible volume and
optionally bind an NTFS folder scope, then scans the selected volume through a
short-lived elevated read-only broker. A logical group is not disk authority:
production never opens `PhysicalDriveN` and never offers a whole-physical-disk
scan.

The desktop remains `asInvoker`. Inventory and folder selection run in the
unelevated native process. Only an explicit scan launches the fixed sibling
`undelete-master-broker.exe`, whose manifest is `requireAdministrator`. The
broker opens the selected mounted volume with `GENERIC_READ` and
`OPEN_EXISTING`, serves bounded reads and exits when the session closes.

The optional folder is not walked to find deleted files. Native code proves the
folder's selected-volume identity using the volume serial and NTFS file
reference, split into MFT record and reuse sequence. The raw NTFS scanner builds
namespace evidence and classifies each candidate as `Match`, `NoMatch` or
`Unknown`. The UI receives only `Match` rows; it receives a separate count of
`Unknown` ancestry and does not mislabel unknown evidence as out of scope.

There is no production image picker in the desktop workflow and no fabricated
inventory, candidate, progress or fallback dataset. The headless CLI continues
to accept regular image files under ADR-0003.

## 2. Scope and explicit limits

### Included

- unelevated inventory of mounted drive-letter volumes;
- logical grouping of observed mounted volumes without claiming a physical disk
  number;
- opaque disk, volume, inventory-generation, folder-scope, scan and cursor IDs;
- unelevated refusal of remote, CD-ROM, RAM-disk and unknown roots, followed by
  authoritative broker refusal of unmapped and multi-disk volumes;
- full-volume metadata scan for eligible mounted NTFS or FAT volumes;
- optional folder scope only for NTFS;
- native folder picker whose absolute path never crosses the WebView boundary;
- short-lived elevated broker and local current-user named pipe;
- protocol version 3 with ten message kinds and a maximum one-megabyte payload;
- bounded candidate pages of exactly at most 100 rows;
- decimal-string transport for every `u64` exposed to JavaScript;
- bounded native state: 32 folder scopes and 4 scan sessions;
- browser fail-closed behavior;
- English and Brazilian Portuguese presentation.

### Excluded from the SDD-018 scan increment

- whole-physical-disk or unmounted-partition scans;
- direct `PhysicalDriveN` open;
- remote, mapped, redirected, CD-ROM, RAM-disk or composite-volume scans;
- hotplug subscription or a guarantee that a mutable mounted volume is a
  snapshot;
- scan pause, resume or cooperative cancellation. The desktop receives real
  phase progress events: MFT enumeration reports measured percentage/ETA, and
  later phases remain indeterminate when no trustworthy total exists;
- preview, content execution or session persistence;
- repair, exFAT, ReFS, locked-BitLocker key handling or image creation;
- arbitrary paths, device paths, pipe names, offsets or access masks supplied
  by JavaScript;
- a persistent service or an elevated WebView;
- signed installer or production-release claim.

Restore and carving were outside SDD-018 itself. They are not absent from the
current product: SDD-019 adds bounded contiguous-JPEG carving for eligible
whole-volume NTFS scans, and SDD-020 adds native query/selection and
transactional restore plus terminal destination-folder opening. Neither
extension changes the scan-source read-only boundary.

## 3. Architecture and trust boundaries

```text
SDD-018 baseline React WebView (opaque IDs and sanitized display data)
  -> four scan/inventory Tauri commands (asInvoker)
  -> native inventory / folder authority / scan coordinator
  -> broker client and local current-user named pipe
     (PID/liveness + fixed sibling desktop image)
  -> fixed sibling broker (requireAdministrator)
  -> identity-bound mounted-volume handle (GENERIC_READ)

broker bytes
  -> SourceReader
  -> partition + NTFS/FAT scanners (unelevated)
  -> namespace classification + candidate adaptation
  -> bounded summary and 100-row pages
```

The current registry is the exact 12-command closed surface documented by
SDD-020: the four SDD-018 baseline operations, two native result
query/selection operations and six restore operations. This diagram preserves
the scan/broker branch; restore destination writes stay unelevated and never
enter the broker.

The broker does not parse partition tables, filesystem metadata, recovered
names or candidate content. Recovered metadata remains in the unelevated
process and is treated as untrusted text.

SDD-018 and ADR-0023 record the deliberate expansion of the audited Windows FFI
boundary from locality classification to the exact query, folder-identity,
local-pipe, fixed-process-launch and read-only mounted-volume calls listed
below. This is a closed allowlist, not authority for general file, process,
volume-control or device-control operations:

- storage discovery and geometry queries;
- query-only `GetVolumeInformationByHandleW` for the live read-only volume
  handle's serial;
- `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`;
- `IOCTL_DISK_GET_LENGTH_INFO`;
- `IOCTL_STORAGE_QUERY_PROPERTY`;
- read-only directory identity queries;
- named-pipe creation/connect, peer PID and liveness checks;
- `QueryFullProcessImageNameW` on the already-bound peer process handle to
  require the canonical fixed desktop sibling before the broker serves;
- fixed sibling elevation through Windows `runas`;
- mounted-volume open with exactly `GENERIC_READ` and `OPEN_EXISTING`;
- `SetFilePointerEx` and `ReadFile`.

There is no write, trim, format, delete, lock, dismount, mount, arbitrary
device-control or destination operation in the scan path. Named-pipe duplex
access uses `GENERIC_READ | GENERIC_WRITE` only for pipe transport; it is not a
scan-source handle.

## 4. Inventory and source authority

Unelevated inventory uses mounted drive roots plus volume GUID and serial
metadata only inside native code. It does not issue DASD opens or IOCTLs, does
not map a physical disk number and does not claim a physical-disk grouping.
The public logical-group `number` is absent/null. Drive letter, filesystem,
label, quota-visible total/free values from `GetDiskFreeSpaceExW` and any
friendly text are display data only.

The stable opaque volume ID is derived from volume GUID plus volume serial; it
does not include mount letter, quota-visible size/free values, extents,
filesystem or disk number. On explicit scan, the elevated broker independently
resolves the same volume, queries canonical length/sector geometry, disk
extents, and the fixed storage-device bus property, then rejects missing,
multi-disk, virtual, file-backed, Storage Spaces, array/network, unknown, or
future mappings before accepting the source or reading any source byte.

The raw open is restricted to the internally resolved mounted-volume selector.
Immediately after the `GENERIC_READ` handle is acquired, query-only
`GetVolumeInformationByHandleW` must return the serial from the selected
inventory identity. Source length, logical/physical sector geometry and
extents and the reviewed direct bus class are then derived authoritatively by
the broker. Serial mismatch, disappearance, changed geometry, changed extents,
changed storage bus, or unresolvable identity fails closed.

For legacy direct USB bridges that return only `ERROR_INVALID_FUNCTION` or
`ERROR_NOT_SUPPORTED` for `StorageAccessAlignmentProperty`, the broker may use
the validated mounted-volume logical sector for buffered read alignment. It
records this fallback mode, revalidates the same mode and geometry, and rejects
all other alignment failures. This does not enable unbuffered I/O or weaken the
read-only source handle.

The pre-UAC identity tuple is exactly volume GUID plus serial. A replacement
that preserves both values exactly is indistinguishable from the selected
volume under the current model. UAC and independent broker enumeration still
occur, but extents, canonical length and sector geometry are derived from the
source mounted when the broker opens it; those post-open values cannot
retroactively distinguish an exact tuple clone. They become the baseline for
later revalidation. This is a documented residual risk, not a snapshot or
forensic identity claim.

The volume remains online and may change while it is scanned. SDD-018 makes no
snapshot or forensic-chain-of-custody claim.

## 5. Broker lifecycle and protocol v3

The client creates a CSPRNG pipe suffix and 32-byte nonce, creates a one-instance
local named pipe with a current-user DACL, launches only the fixed sibling
broker, and accepts only the launched broker PID. The broker binds the connected
server PID and liveness handle, calls `QueryFullProcessImageNameW` on that same
peer process handle, canonicalizes the result, and requires it to equal the
fixed `undelete-master-desktop.exe` sibling derived from the running
`undelete-master-broker.exe`. This check occurs before `serve_session` can open
or read a source.

PID/liveness plus the fixed peer-image path reduces the confused-deputy surface;
it does not verify Authenticode, publisher, package revision, directory ACLs or
that the sibling directory is administrator-protected. An unsigned or
user-replaceable package remains blocked from production distribution.

Protocol v3 has a 20-byte little-endian frame header, monotonic independent
sequence numbers, a maximum payload of 1 MiB and exactly these ten messages:

1. `Hello { challenge }`
2. `HelloAck { challenge }`
3. `OpenSource { source_id }`
4. `Opened { handle_id, size, logical_sector, physical_sector, physical_disk_number }`
5. `ReadAt { handle_id, offset, length }`
6. `ReadData { bytes }`
7. `CloseSource { handle_id }`
8. `Closed { handle_id }`
9. `Shutdown`
10. `Error { code }`

`HelloAck` echoes the exact 32-byte client nonce and the client compares it in
constant time. This echo is not a MAC, shared secret or mutual authentication.
It only binds the response to the connection after native peer PID/liveness and
fixed sibling image-path verification.

The schema contains no arbitrary path, device name, destination, free-form
error, access mask, generic control code or mutation command. Unknown version,
opcode, sequence gap/replay, malformed field, zero handle, invalid sector
geometry, invalid opaque identifier, oversized frame or overflowed range fails
closed.

One broker session opens at most one source. `ReadAt` is checked against source
length before allocation and is at most 1 MiB. Larger scanner requests are
split by the client. Close and shutdown are attempted on normal drop and on
failed opens.

## 6. Source identity during reads

Open performs an authoritative enumeration before and after the read-only
handle is acquired and binds the serial reported by that handle to the selected
identity. During a session, every valid, in-range `ReadAt` invokes the broker
source revalidation hook.

The expensive Windows identity re-enumeration occurs only when both conditions
are true:

- at least 256 valid `ReadAt` requests have occurred since the last
  re-enumeration; and
- at least one second has elapsed since that re-enumeration.

When both bounds are met, revalidation checks current mounted identity and the
live handle's serial, extents, canonical length and sector layout before the
counters reset. This cadence does not cache source bytes and does not make the
volume a snapshot. Unless identity revalidation fails first, every accepted
`ReadAt` still performs a real `ReadFile`; an I/O failure fails closed.

## 7. Folder authority and ancestry semantics

Folder scope is available only when the selected mounted volume reports NTFS.
The native picker returns a path only to Rust. Native validation:

- rejects non-drive roots, remote locations and unsafe components;
- rejects reparse ancestry and a final reparse point;
- proves that the final directory handle belongs to the selected volume;
- records the volume serial and 64-bit NTFS file reference;
- splits the reference into the low 48-bit MFT record and high 16-bit reuse
  sequence;
- returns only an opaque `scopeId`, opaque `volumeId` and sanitized label to
  JavaScript.

Before filtering candidates, the NTFS namespace must resolve the selected
relative components to one active directory whose record and sequence exactly
match the retained folder authority. A textually matching path with the wrong
identity is rejected.

Candidate classification uses the namespace graph:

- `Match`: ancestry proves the selected directory; show the candidate;
- `NoMatch`: ancestry proves a different branch; do not show the candidate;
- `Unknown`: missing, corrupt, reused, cyclic or ambiguous ancestry prevents a
  proof; do not show the candidate and increment `unknownCandidates`.

A folder-scoped result never falls back to a string-prefix comparison. FAT
supports a whole-volume scan but no folder authority in this increment.

Recursive NTFS parent-path expansion applies
`MAX_NAMESPACE_PATHS_PER_NAME` before descending into another sibling branch
once the current name is saturated, as well as while retaining returned paths.
Saturation marks the namespace incomplete, emits bounded partial-coverage
evidence and keeps unproven ancestry `Unknown`; it never promotes omitted work
to `NoMatch`, `Match` or complete coverage. Regression
`NTFS-NAMESPACE-WORK-BOUND-001` captures the former 8,191-call expansion and
requires the corrected synthetic graph to complete in at most 600 recursive
calls while retaining the 256-path cap.

## 8. WebView contract

The SDD-018 baseline registry contained exactly:

- `list_storage_sources(requestId)`;
- `select_scan_folder(requestId, volumeId)`;
- `scan_storage_volume(requestId, volumeId, scopeId?)`;
- `get_candidate_page(requestId, scanId, cursor?, limit)`.

The current registry additionally contains `query_candidate_page`,
`update_candidate_selection` and the six opaque restore lifecycle commands
defined by SDD-020, for exactly 12 commands overall. All remain covered by the
same closed-inventory validators; none accepts source/destination paths,
extents, offsets, handles, executables or recovered bytes from the WebView.

The production WebView represents every native authority with an opaque ID.
Its remaining inputs are bounded request IDs, decimal strings, closed enums
and bounded query/selection objects. It never supplies or receives a native
path, volume GUID, device name, pipe name, raw offset or source handle.

Contract schema version 1 uses decimal strings for disk/volume sizes, free
space and candidate counts. Candidate pages require `limit = 100`, use
scan-bound opaque cursors and contain no more than 100 rows. Directory rows have
no recoverability score. Candidate IDs and display text are sanitized and
bounded; control and bidi-formatting characters are removed.

The native state evicts older authorities after 32 folder scopes and 4 scan
sessions. Inventory is capped at 128 logical groups and 128 volumes per group;
warning and text counts are also bounded. Browser-only execution reports the
unavailable desktop runtime and invokes no native command.

## 9. Requirements

### SDD-WIN-001 — Mounted local inventory

- **Rationale:** users need observed storage choices without fabricated data.
- **Priority:** Must.
- **Source:** product-owner direction; FR-010 through FR-013.
- **Preconditions:** the desktop runs on Windows without elevation.
- **Behavior:** enumerate mounted local volumes into logical display groups
  without a physical disk number and expose opaque IDs plus bounded sanitized
  display metadata.
- **Error behavior:** remote, CD, RAM and unknown roots are ineligible
  unelevated; the broker later rejects unmapped/multi-disk authority.
- **Security implications:** display metadata never authorizes an arbitrary
  path or physical disk.
- **Observability:** deterministic policy and DTO regressions inspect grouping,
  identity and path absence.
- **Acceptance criteria:** real mounted-volume inventory is visible without UAC
  or DASD/IOCTL access and a logical group cannot open `PhysicalDriveN`.
- **Test IDs:** `WINDOWS-INVENTORY-GROUP-001`,
  `WINDOWS-INVENTORY-POLICY-001`, `DESKTOP-INVENTORY-001`,
  `WIN-REAL-INVENTORY-001`.
- **Implementation links:** `crates/io-windows/src/model.rs`,
  `crates/io-windows/src/windows.rs`,
  `apps/desktop/src-tauri/src/storage.rs`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-002 — Least-privilege process split

- **Rationale:** hostile filesystem metadata must not execute in an elevated
  renderer or parser.
- **Priority:** Must.
- **Source:** ADR-0008 and FR-003.
- **Preconditions:** the user explicitly starts a supported volume scan.
- **Behavior:** keep Tauri `asInvoker`; elevate only the fixed sibling broker
  with `requireAdministrator`; before serving, bind peer PID/liveness and
  require `QueryFullProcessImageNameW` on that process handle to resolve to the
  canonical fixed desktop sibling.
- **Error behavior:** denial, missing sibling, wrong peer PID/image, image-query
  failure, early exit or timeout fails closed with a stable code.
- **Security implications:** the broker has no WebView or filesystem parser.
  Image-path verification reduces confused-deputy exposure but is not
  Authenticode or a protected-directory guarantee.
- **Observability:** manifests, arguments, fixed-location resolution, peer PID,
  peer image and bounded accept have focused tests.
- **Acceptance criteria:** inventory requires no UAC; scan elevation cannot
  name another executable or pipe; the broker never serves a peer whose image
  is not the fixed packaged desktop sibling.
- **Test IDs:** `WINDOWS-BROKER-CONFIG-002`,
  `WINDOWS-NAMED-PIPE-001`, `WINDOWS-BROKER-LAUNCH-001`,
  `WINDOWS-BROKER-PEER-001`, `ELEVATED-BROKER-ARCH-005`,
  `ELEVATED-BROKER-ARCH-006`.
- **Implementation links:** `crates/io-windows/src/transport_config.rs`,
  `crates/io-windows/src/windows.rs`,
  `crates/elevated-broker/undelete-master-broker.manifest`,
  `apps/desktop/src-tauri/windows-app-manifest.xml`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-003 — Closed protocol v3

- **Rationale:** the privileged boundary must expose only the minimum read
  lifecycle.
- **Priority:** Must.
- **Source:** ADR-0009 and ADR-0023.
- **Preconditions:** native peer PID/liveness and fixed sibling image-path
  checks accepted the pipe.
- **Behavior:** use exactly the ten v3 messages, the authoritative
  `physical_disk_number` in `Opened`, explicit v2 rejection, one-megabyte
  payload cap, contiguous sequences and exact constant-time nonce echo
  verification.
- **Error behavior:** malformed, replayed, oversized or unknown traffic closes
  or rejects the session without free-form diagnostics.
- **Security implications:** nonce echo is explicitly not a MAC; the native
  PID/liveness/image tuple is primary identity and still depends on trusted
  signed packaging.
- **Observability:** schema inventory and frame/session regressions enumerate
  every opcode and field.
- **Acceptance criteria:** no path, mutation, generic control or destination
  field exists in the schema.
- **Test IDs:** `BROKER-PROTOCOL-ARCH-002`,
  `BROKER-PROTOCOL-MALFORMED-003`,
  `BROKER-PROTOCOL-SESSION-002`,
  `BROKER-CLIENT-PROTOCOL-004`.
- **Implementation links:** `crates/broker-protocol/src/message.rs`,
  `crates/broker-protocol/src/codec.rs`,
  `crates/broker-client/src/lib.rs`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-004 — Identity-bound mounted-volume open

- **Rationale:** a stale display row must not authorize another source.
- **Priority:** Must.
- **Source:** FR-010 through FR-013; risk R-022.
- **Preconditions:** an eligible opaque volume ID exists in a fresh inventory.
- **Behavior:** independently resolve identity, open only that mounted volume
  with `GENERIC_READ | OPEN_EXISTING`, bind the selected serial to the live
  handle, and derive canonical extents, size, sector geometry, and a reviewed
  direct storage-bus classification.
- **Error behavior:** disappearance, replacement, unsupported mapping or
  changed identity fails closed.
- **Security implications:** no UI path, physical-disk selector or access mask
  reaches the open.
- **Observability:** handle-serial, source-reader and session tests cover
  identity, exact geometry, range and cleanup.
- **Acceptance criteria:** production cannot open an unmounted partition or
  `PhysicalDriveN`.
- **Test IDs:** `WINDOWS-RAW-IDENTITY-001`,
  `WINDOWS-RAW-IDENTITY-002`, `BROKER-CLIENT-ARCH-002`,
  `BROKER-CLIENT-READER-003`, `ELEVATED-BROKER-SESSION-003`.
- **Implementation links:** `crates/io-windows/src/windows.rs`,
  `crates/broker-client/src/lib.rs`,
  `crates/elevated-broker/src/lib.rs`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-005 — Bounded read-only source

- **Rationale:** malformed requests must not escape source bounds or allocate
  unbounded memory.
- **Priority:** Must.
- **Source:** ADR-0002; NFR-001 and NFR-002.
- **Preconditions:** the broker has one identity-bound source open.
- **Behavior:** validate checked ranges, cap each protocol read at 1 MiB,
  perform aligned `ReadFile` calls and re-enumerate identity only after both
  256 valid reads and one elapsed second.
- **Error behavior:** short read, overflow, source change and I/O failure fail
  closed and zero incomplete client buffers.
- **Security implications:** no scan-source write or mutation API exists.
- **Observability:** range, chunking, cadence and source-change regressions are
  deterministic and use only synthetic readers.
- **Acceptance criteria:** every accepted request performs real source I/O
  unless identity validation first rejects it; no test opens a real disk.
- **Test IDs:** `WINDOWS-RAW-READ-PLAN-001`,
  `BROKER-CLIENT-READER-002`, `BROKER-PROTOCOL-BOUNDS-001`,
  `ELEVATED-BROKER-REVALIDATION-001`.
- **Implementation links:** `crates/io-windows/src/model.rs`,
  `crates/io-windows/src/windows.rs`,
  `crates/elevated-broker/src/lib.rs`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-006 — Native NTFS folder authority

- **Rationale:** path text cannot prove historical folder identity.
- **Priority:** Must.
- **Source:** product-owner direction; risk R-023.
- **Preconditions:** an eligible NTFS volume is selected.
- **Behavior:** keep the path native, reject redirection/cross-volume input and
  bind scope to volume serial plus MFT record and sequence.
- **Error behavior:** cancel is no change; unavailable or mismatched identity
  yields a stable folder-scope code.
- **Security implications:** no absolute path crosses IPC or is persisted.
- **Observability:** folder model and state tests cover unsafe components,
  record decoding and volume binding.
- **Acceptance criteria:** a path match with a different record or sequence is
  rejected.
- **Test IDs:** `WINDOWS-FOLDER-SCOPE-001`,
  `WINDOWS-FOLDER-SCOPE-003`, `DESKTOP-FOLDER-SCOPE-002`,
  `DESKTOP-STATE-001`.
- **Implementation links:** `crates/io-windows/src/model.rs`,
  `crates/io-windows/src/windows.rs`,
  `apps/desktop/src-tauri/src/storage.rs`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-007 — Real scanner composition

- **Rationale:** desktop output must come from the shared bounded parsers.
- **Priority:** Must.
- **Source:** FR-030 through FR-054.
- **Preconditions:** the broker source opens successfully.
- **Behavior:** call `scan_volume_reader` on `spawn_blocking`, preserve NTFS/FAT
  status, real warnings and candidate metadata, and never synthesize results.
- **Error behavior:** recognized read and corrupt errors remain typed and are
  sanitized at the desktop boundary.
- **Security implications:** parsing remains unelevated; broker returns bytes
  only.
- **Observability:** CLI scanner tests and desktop adapter regressions use
  deterministic fixtures/readers.
- **Acceptance criteria:** browser execution has no fallback data and the
  production desktop has no image-picker command.
- **Test IDs:** `WIN-PRODUCTION-SURFACE-001`,
  `WIN-BROWSER-FAIL-CLOSED-001`, `DESKTOP-CANDIDATE-001`.
- **Implementation links:** `crates/cli/src/lib.rs`,
  `apps/desktop/src-tauri/src/storage.rs`,
  `apps/desktop/src/api/storageDesktop.ts`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-008 — Proven folder containment

- **Rationale:** incomplete or reused NTFS ancestry must not be guessed.
- **Priority:** Must.
- **Source:** ADR-0011; risk R-023.
- **Preconditions:** the NTFS scan produced namespace evidence and a folder
  authority exists.
- **Behavior:** validate the selected active directory identity, classify every
  candidate as `Match`, `NoMatch` or `Unknown`, show only `Match` and report the
  `Unknown` count separately; enforce the per-name namespace path cap before
  recursing into a saturated sibling branch.
- **Error behavior:** unresolved selected directory rejects the folder-scoped
  session; unknown candidate ancestry never becomes `NoMatch`.
- **Security implications:** display-path prefix comparison is never
  authoritative.
- **Observability:** namespace and desktop tests cover links, sequence reuse,
  cycles, missing parents, unknown ancestry and bounded recursive work. The
  work-bound regression records RED at 8,191 calls and GREEN at no more than
  600 calls for the synthetic saturated graph.
- **Acceptance criteria:** the result truthfully distinguishes matched,
  excluded and unprovable ancestry.
- **Test IDs:** `DESKTOP-FOLDER-SCOPE-001`,
  `DESKTOP-FOLDER-SCOPE-002`, `WIN-PARTIAL-UNKNOWN-001`,
  `NTFS-NAMESPACE-WORK-BOUND-001`.
- **Implementation links:** `crates/fs-ntfs/src/lib.rs`,
  `crates/fs-ntfs/src/scan.rs`,
  `apps/desktop/src-tauri/src/storage.rs`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-009 — Historical four-command scan contract

- **Rationale:** a small typed facade prevents unsupported authority from
  entering the WebView.
- **Priority:** Must.
- **Source:** FR-100 through FR-103.
- **Preconditions:** the Tauri runtime is available.
- **Behavior:** this increment exposed exactly inventory, native-folder
  selection, volume scan and candidate-page commands, with schema version 1 and
  decimal-string validation. SDD-020 subsequently extends the current closed
  inventory to exactly 12 commands for native query, selection and restore.
- **Error behavior:** unknown fields, native paths, duplicate IDs, malformed
  decimals or incompatible enums fail closed.
- **Security implications:** native authority is represented only by opaque
  IDs; remaining inputs are bounded text/decimal values, closed enums and
  bounded query/selection objects.
- **Observability:** Rust registration, TypeScript contract tests and the
  real-only validator enumerate the current closed surface.
- **Acceptance criteria:** pages contain at most 100 real rows and cursors are
  bound to their scan.
- **Test IDs:** `DESKTOP-COMMAND-INVENTORY-001`,
  `DESKTOP-PAGINATION-001`, `WIN-CANDIDATE-PAGINATION-001`.
- **Implementation links:** `apps/desktop/src-tauri/src/lib.rs`,
  `apps/desktop/src-tauri/src/storage.rs`,
  `apps/desktop/src/api/storage.ts`,
  `apps/desktop/src/api/storageDesktop.ts`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-010 — Bounded state, privacy and accessible truth

- **Rationale:** recovered metadata is untrusted and large scans must not
  exhaust the UI.
- **Priority:** High.
- **Source:** NFR-004 through NFR-006.
- **Preconditions:** inventory, summary or candidate data crosses IPC.
- **Behavior:** cap native collections, sanitize text, remove control/bidi
  formatting, isolate displayed paths and present real empty, pending, partial,
  unknown and error states.
- **Error behavior:** overflow or incompatible data rejects the response rather
  than truncating structural truth silently.
- **Security implications:** paths, source bytes and raw OS diagnostics remain
  native.
- **Observability:** UI and adapter tests cover accessibility, privacy,
  duplicate submission and stale responses.
- **Acceptance criteria:** volume radios and candidate table have accessible
  names; unknown ancestry and partial coverage remain visible.
- **Test IDs:** `WIN-VOLUME-RADIO-A11Y-001`,
  `WIN-CANDIDATE-TABLE-A11Y-001`, `WIN-ERROR-PRIVACY-001`,
  `WIN-DUPLICATE-SCAN-001`.
- **Implementation links:** `apps/desktop/src/views/AnalysisView.tsx`,
  `apps/desktop/src/state/storageScan.ts`,
  `apps/desktop/src-tauri/src/storage.rs`.
- **Status:** `Implemented-unverified`.

### SDD-WIN-011 — Real-only safety verification

- **Rationale:** component success does not prove a safe packaged workflow.
- **Priority:** Must.
- **Source:** repository safety invariants.
- **Preconditions:** all implementation changes are frozen on one revision.
- **Behavior:** run format, Clippy, Rust tests, frontend lint/typecheck/tests,
  real-only/static-storage validators and production builds.
- **Error behavior:** any fabricated production state, write-capable source
  path, prohibited command, failure or warning blocks completion.
- **Security implications:** ordinary tests use synthetic in-repository images
  or readers only.
- **Observability:** exact commands, revision, artifact hashes and outcomes are
  retained in an evidence record.
- **Acceptance criteria:** all same-revision local and remote gates pass; no
  real source scan is used as a test.
- **Test IDs:** `WIN-PRODUCTION-SURFACE-001`,
  `BROKER-CLIENT-ARCH-001`, `ELEVATED-BROKER-ARCH-003`.
- **Implementation links:** `.github/scripts/validate_real_only_desktop.py`,
  `.github/scripts/validate_ci_safety.py`,
  `.github/workflows/quality.yml`.
- **Status:** `Partial`.

### SDD-WIN-012 — Native package and release evidence

- **Rationale:** an unsigned local executable is not a production release.
- **Priority:** Must.
- **Source:** SDD-013; risks R-017 and R-025.
- **Preconditions:** the same revision passes SDD-WIN-011.
- **Behavior:** build both fixed sibling binaries, inspect manifests, hashes and
  sibling layout, require administrator-protected production placement, run
  native UI acceptance without scanning a real disk, then require signing and
  clean-machine evidence before redistribution.
- **Error behavior:** quarantine, hash drift, publisher mismatch or failed
  native/remote gate blocks release and remains unresolved.
- **Security implications:** disabling endpoint protection is not an acceptance
  mechanism.
- **Observability:** evidence names executable hashes, revision, manifest
  levels, screenshot provenance and remote workflow URL.
- **Acceptance criteria:** signed, administrator-protected, clean-machine and
  vendor disposition evidence exists before any production claim.
- **Test IDs:** `ELEVATED-BROKER-ARCH-005`,
  `WINDOWS-BROKER-LAUNCH-001`; signing and external endpoint-security
  disposition cannot be reduced to a deterministic unit test and remain
  governed by the evidence gates in section 10.
- **Implementation links:** `apps/desktop/src-tauri/tauri.conf.json`,
  `apps/desktop/package.json`,
  `docs/evidence/windows-volume-scan-2026-07-29.md`.
- **Status:** `Partial`.

### SDD-WIN-013 — Per-monitor DPI-correct native desktop

- **Rationale:** the checked-in Windows manifest must preserve Tauri/WebView2
  DPI awareness so source identity, safety warnings and scan controls remain
  fully visible at non-default display scaling.
- **Priority:** Must.
- **Source:** SDD-010 accessibility contract and native 150% acceptance.
- **Preconditions:** the desktop remains `asInvoker` and uses the checked-in
  Tauri Windows application manifest.
- **Behavior:** declare the legacy `true/pm` fallback and modern
  `PerMonitorV2, PerMonitor` awareness; Windows, the WebView and UI Automation
  must describe the same physical client area.
- **Error behavior:** a missing declaration fails the real-only validator;
  mismatched native/UIA bounds or visible clipping blocks native acceptance.
- **Security implications:** DPI virtualization must not hide or visually
  misrepresent source identity, scan scope, UAC expectations or warnings.
- **Observability:** retain window DPI, process awareness, native/WebView
  bounds, UIA root bounds and their dimension ratio without activating a scan.
- **Acceptance criteria:** at 144 DPI (150%), native and UIA client dimensions
  match at `1.0000 × 1.0000` and share the right edge; the validator rejects
  either missing DPI declaration.
- **Test IDs:** `DESKTOP-REAL-ONLY-022`.
- **Implementation links:**
  `apps/desktop/src-tauri/windows-app-manifest.xml`,
  `.github/scripts/validate_real_only_desktop.py`,
  `.github/scripts/tests/test_validate_real_only_desktop.py`.
- **Status:** `Verified`.

## 10. Acceptance and evidence gates

The following evidence may be recorded independently, but release requires one
frozen revision:

| Gate | Required evidence | Current disposition |
| --- | --- | --- |
| Deterministic Rust/frontend | Required commands and focused broker, folder, namespace and DTO tests | Same-revision local gates passed; Task 7 records exact counts |
| Static safety | Exact command/opcode/import/access-mask allowlists and CI real-device prohibition | Static validators and regression suites passed |
| Native build | Main and broker binaries, hashes, sibling layout and extracted manifests | Unsigned sibling release pair and embedded execution levels recorded in Task 7 evidence |
| Native UX | Actual Tauri window at required sizes; no simulated screen | Inventory and Settings keyboard navigation observed; 150% DPI native/UIA bounds previously verified; broader accessibility matrix pending |
| Real storage inventory | Read-only inventory only; observed source labels must be sanitized | Two supported local NTFS volumes observed; no scan activated |
| Real source scan | Not required and not executed as part of this increment | No claim |
| Remote CI | Workflow URL and successful conclusions for pushed revision | Passed for commit `41b06d790d47699994a82c41eba44a0a314cc9ab`: `quality` run #9 on draft PR #2 completed successfully |
| Signing and endpoint security | Authenticode, administrator-protected package placement, clean-machine and vendor disposition for exact artifact | Pending |

Norton deleted an unsigned development executable on 2026-07-29. That event is
unresolved: it is neither proof of malware nor a confirmed false positive.
Temporary local endpoint-protection state is not release evidence.

## 11. Supersession and rollback

Once the implementation revision passes its delivery gates, SDD-018 supersedes
SDD-017 only for desktop source selection, scan execution and result delivery.
SDD-017 remains historical evidence for the earlier image desktop. ADR-0003
continues to govern the image CLI.

Rollback removes the connected-storage desktop artifact while retaining the
read-only image CLI. Rollback never introduces sample disks, fabricated
candidates or browser fallback data.

## 12. Revisit triggers

A new accepted SDD/ADR is required before adding:

- whole-disk, unmounted-volume, VHD/VHDX or composite-volume authority;
- folder scope for FAT or another filesystem;
- scan cancellation, snapshotting or hotplug guarantees. Scan progress is
  reported only from bounded native work already completed; it is not a claim
  of snapshot consistency or exhaustive recovery coverage;
- another protocol opcode, generic device-control or persistent service;
- preview, broader/fragmented carving, exFAT, session persistence or content
  execution;
- a path-bearing WebView contract;
- public distribution without the signing and clean-machine gates.
