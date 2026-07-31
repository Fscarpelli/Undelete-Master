# ADR-0023 — Windows Read-only Mounted-volume Broker and NTFS Folder Scope

Status: Accepted
Decision date: 2026-07-29

Implementation status: `Implemented-unverified`. Acceptance of the architecture
does not attest the current binaries, complete the release gates or approve an
unsigned artifact for redistribution.

## Context

ADR-0021 delivered a real but image-only desktop. The product owner now
requires the desktop to discover connected storage, let the user choose a
volume and optionally choose a folder, and show real scanner candidates. No
sample inventory, simulated scan or fabricated fallback is allowed.

Windows ordinary directory enumeration cannot reliably find deleted metadata.
The selected mounted volume must therefore be read as a raw byte source.
Elevating Tauri would also elevate the WebView, parsers, recovered names and UI
dependencies. A smaller privilege boundary is required.

The implementation can prove a folder scope for NTFS using the selected
directory's volume serial and MFT file reference. It cannot prove folder
ancestry for FAT, composite volumes or every damaged NTFS candidate.

## Decision drivers

- never write, lock, dismount, trim, format or repair a scan source;
- keep the desktop, parsers and recovered metadata unelevated;
- select only observed mounted local volumes;
- prevent a disk display card or stale drive letter from becoming authority;
- keep native paths and privileged handles outside JavaScript;
- prevent an arbitrary unelevated process from using the genuine broker as a
  raw-read confused deputy;
- distinguish proven, disproven and unknown NTFS ancestry;
- bound recursive namespace work before expanding another saturated branch;
- bound protocol frames, reads, native state and candidate pages;
- retain the regular-image CLI;
- verify with synthetic sources rather than a real disk.

## Options considered

| Option | Benefit | Defect | Decision |
| --- | --- | --- | --- |
| Walk the selected folder | Simple and usually unelevated | Does not reliably expose deleted metadata | Rejected |
| Elevate Tauri | Avoids IPC | Elevates WebView, parsers and untrusted metadata | Rejected |
| Use drive letters or WMI as authority | Convenient display inventory | Mutable and insufficient for raw-source identity | Rejected as authority |
| Install a service | Avoids repeated UAC | Persistent privileged attack surface and release burden | Rejected |
| Keep the image-only desktop | Existing narrow boundary | Does not meet connected-volume requirement | Rejected for desktop; retained for CLI |
| Trust pipe PID/liveness only | Binds one live peer process | A caller-controlled parent can still be the wrong executable | Rejected as sufficient authentication |
| Also require the fixed sibling desktop image | Narrows the peer to the packaged desktop path | Does not prove publisher or protect a writable package directory | Accepted with signing/protected-package release gates |
| Short-lived read-only broker | Small privileged surface and bounded byte API | Adds UAC, protocol and two-binary packaging | Accepted |

## Decision

Adopt the exact mounted-volume architecture in
[SDD-018](../specs/018-windows-volume-and-folder-scan.md):

1. The Tauri application remains `asInvoker`. Inventory, native folder
   selection, partition/filesystem parsing and all rendering remain
   unelevated.
2. Unelevated inventory contains mounted local volumes in logical display
   groups only. It does not use DASD/IOCTL access, expose a physical disk
   number or claim a physical grouping. A logical group is never scan
   authority; the implementation never opens `PhysicalDriveN`.
3. Remote, CD-ROM, RAM-disk and unknown roots fail closed before scan. The
   elevated broker later rejects unmapped/multi-disk authority.
4. Starting a supported scan launches only the fixed sibling
   `undelete-master-broker.exe`, whose manifest is `requireAdministrator`.
   There is no service and inventory does not request UAC.
5. The stable volume ID uses volume GUID plus serial only, excluding mount,
   quota-visible size/free data, filesystem, extents and disk number. The
   broker independently resolves it, queries canonical length/geometry/extents
   plus the fixed storage-device bus property, opens the mounted volume with
   exactly `GENERIC_READ` and `OPEN_EXISTING`, binds the selected serial to that
   live handle with the query-only `GetVolumeInformationByHandleW`, and rejects
   composite or unproven virtual/array mappings before any source byte is read.
   A replacement preserving exactly the selected GUID plus
   serial remains indistinguishable at first open: UAC and broker enumeration
   still occur, but extents, length and geometry are derived from the source
   currently mounted and become its later revalidation baseline. No snapshot
   or stronger pre-UAC identity is claimed.
6. The local named pipe is restricted to the current user and one instance.
   Both directions retain PID/liveness checks. Before serving, the broker calls
   `QueryFullProcessImageNameW` on the already-bound peer process handle and
   requires the canonical fixed `undelete-master-desktop.exe` sibling derived
   from its own fixed broker path.
7. Protocol version 3 has a 20-byte header, monotonic sequences, at most a
   one-megabyte payload and exactly ten messages: `Hello`, `HelloAck`,
   `OpenSource`, `Opened`, `ReadAt`, `ReadData`, `CloseSource`, `Closed`,
   `Shutdown` and `Error`. Version 3 adds the authoritative single physical
   disk number to `Opened`; version 2 is rejected rather than silently parsing
   the incompatible payload.
8. `HelloAck` echoes the exact CSPRNG 32-byte client nonce and the client checks
   it in constant time. The echo is not a MAC, shared secret or substitute for
   native peer verification.
9. The protocol contains no arbitrary path, device name, destination, access
   mask, generic control code, free-form error or mutation operation.
10. Every accepted `ReadAt` is range-checked and at most 1 MiB. The client
    chunks larger scanner reads. A valid request performs actual `ReadFile`
    unless identity revalidation first fails.
11. Expensive Windows identity re-enumeration occurs only after both 256 valid
    `ReadAt` requests and one elapsed second. This cadence is not a snapshot
    guarantee; the mounted volume remains mutable. Every due revalidation also
    compares the live handle's serial, extents, canonical length, sector
    layout, and reviewed direct storage-bus class with the values bound during
    open.
12. The audited Windows FFI boundary has a closed native-call allowlist for
    inventory, geometry, folder identity, local pipe/PID/liveness, the fixed
    peer-image query, fixed elevation, three query-only IOCTLs and read-only
    mounted-volume I/O. It is not a generic file, process or device-control API.
13. Named-pipe duplex access may use `GENERIC_READ | GENERIC_WRITE` only for
    transport. A scan-source handle never requests write access.
14. The optional folder path exists only in native Rust. NTFS folder authority
    uses selected volume identity, volume serial, low 48-bit MFT record and high
    16-bit reuse sequence. Reparse ancestry, remote roots and cross-volume
    selection are rejected.
15. Folder filtering first resolves the selected active directory to the same
    record and sequence, then classifies candidates as `Match`, `NoMatch` or
    `Unknown`. Only `Match` is returned; `Unknown` is counted separately.
    String-prefix comparison is never authority. Recursive namespace expansion
    checks the 256-path per-name budget before recursing into another saturated
    sibling; saturation marks ancestry incomplete/partial rather than claiming
    unvisited paths.
16. FAT supports a whole-mounted-volume scan only. Folder scope is NTFS-only.
17. JavaScript sees exactly four commands and only opaque IDs, sanitized display
    data, decimal-string `u64` values and pages of at most 100 candidates.
18. The original SDD-018 desktop slice did not implement cancellation, hotplug
    subscription, destination writes, restore publication, preview, exFAT,
    session persistence or a snapshot guarantee. ADR-0027 adds query-only
    destination authority and disk-separation evidence without adding a write
    capability. SDD-020 Task 5 additionally permits native restore
    coordination to duplicate and revalidate only an already-retained
    destination handle and to ask the shell to explore only an already-retained
    completed-job directory. Those operations do not add a source write,
    caller-selected path, executable, verb or argument. The shell request runs
    on one dedicated joined thread whose COM apartment is initialized with
    `COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE`. It uses
    `SEE_MASK_NOASYNC`, the fixed `explore` verb and null parameters, and keeps
    the retained `File` live through the call.
19. Pull-request and ordinary local tests never open a real disk. The image CLI
    remains the read-only fallback and is not superseded.

ADR-0027 authorizes only the fixed source/destination physical-disk evidence and
query-only destination capability described there. Any added broker opcode,
source mutation, generic control path, broader physical-disk authority,
privileged parser or path-bearing WebView contract requires another accepted
ADR.

## Formal audited-boundary expansion

This decision records an explicit architecture change rather than silently
placing native authority in an existing helper. `crates/io-windows` becomes the
single audited Windows FFI boundary for this increment. Its allowed source
authority is narrower than its implementation language surface:

- query mounted-volume identity, extents, length, alignment, and exactly the
  `StorageDeviceProperty` bus classification;
- query the serial of the live read-only volume handle with
  `GetVolumeInformationByHandleW` during open and due revalidation;
- validate one native NTFS directory identity;
- open one Rust-owned destination selection with fixed query/list access,
  read/write sharing without delete sharing, `OPEN_EXISTING`, backup semantics
  and final-reparse no-follow; retain that exact directory handle;
- duplicate that already-authorized directory handle through safe
  `std::fs::File::try_clone` only, preserving the same Windows file object and
  no-delete-share authority without reopening or exposing its picker path;
- revalidate the retained directory itself as a non-reparse directory with the
  same volume serial, file index and final volume GUID, then repeat the
  NTFS/direct/single-disk query and report only bounded current free bytes and
  native policy evidence;
- derive a final volume GUID from that handle, open only that internally
  derived GUID with desired access `0`, and query its serial, sanitized
  label/filesystem, free bytes, bounded physical-disk extents, and the same
  fixed storage-device bus classification;
- create/connect one local broker pipe and verify peer PID/liveness;
- query the already-bound peer image with `QueryFullProcessImageNameW` and
  compare it with the canonical fixed desktop sibling before serving;
- elevate the fixed sibling broker;
- dispatch the existing `ShellExecuteExW` declaration from one dedicated
  joined worker thread only. That thread calls `CoInitializeEx` with exactly
  `COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE`, treats every negative
  HRESULT as failure, and balances both successful results (`S_OK` and
  `S_FALSE`) with exactly one same-thread `CoUninitialize` through a guard;
- construct that shell request only from a bounded normalized path obtained
  from the already-retained completed-job directory handle, set
  `SEE_MASK_NOASYNC`, use the fixed `explore` verb and null parameters, and
  keep the handle live until the shell call has completed. No caller can
  supply a path, executable, verb or argument;
- open only an internally resolved mounted-volume selector for read;
- seek and read bounded bytes.

The only permitted storage control codes remain
`IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`, `IOCTL_DISK_GET_LENGTH_INFO` and
`IOCTL_STORAGE_QUERY_PROPERTY`. None changes storage state. The broker protocol
cannot choose a control code.

This expansion does not weaken ADR-0002: no source write, write-capable access
mask, trim, format, delete, lock, dismount, mount or repair is permitted.

## Consequences

### Positive

- users see real mounted storage without a file-image picker;
- logical presentation cannot be mistaken for a physical-disk map or
  whole-disk scan authority;
- the WebView and filesystem parsers remain unelevated;
- ordinary stale/replaced volume identities with a changed GUID or serial fail
  closed;
- a genuine broker refuses a live peer whose executable is not the fixed
  packaged desktop sibling;
- folder results separate proof from uncertainty;
- protocol and UI surfaces are finite and testable;
- destination validation retains one non-serializable, query-only handle and
  rejects unknown, composite, same-disk, changed-source and non-NTFS evidence;
- restore coordination can derive one worker handle and fresh free-space
  observation without reopening the mutable picker path;
- opening a completed destination is a closed handle-only shell operation
  rather than a WebView-provided path or process request;
- source and destination reject known virtual, file-backed, Storage Spaces,
  array/network, unknown, and future bus classes before trusting a disk number;
- the image CLI remains available.

### Negative

- a scan can display UAC and fail if elevation is denied;
- two binaries, protocol compatibility and sibling packaging add release work;
- fixed image-path verification does not establish Authenticode publisher,
  same-revision package identity or an administrator-protected directory;
- a replacement preserving exactly the selected GUID plus serial cannot be
  distinguished at first open; extents, length and geometry are then derived
  from the currently mounted replacement and become its revalidation baseline;
- storage bus classification catches native VHD/VHDX and Storage Spaces but is
  not proof against a hypervisor or third-party filter that emulates an
  allowlisted direct bus;
- a live mounted volume can change during the scan;
- each safe destination-handle duplicate extends the retained
  no-delete-share lifetime and must remain bounded by the native
  authority/job stores;
- free space is a current observation, not a reservation, and can change after
  revalidation; the restore transaction must still report later allocation or
  write failures explicitly;
- `SEE_MASK_NOASYNC` keeps ShellExecute's potentially asynchronous setup on
  the initialized apartment until the fixed call returns; it does not make
  Explorer itself part of this process. The native handle prevents target
  substitution through the call, but this boundary does not control Explorer
  after return or authenticate shell extensions; it opens only the completed
  job directory and never a recovered file;
- there is no cancellation, snapshot or hotplug guarantee;
- FAT cannot be folder-scoped;
- unmounted and composite sources are unsupported;
- unknown ancestry is omitted from rows and must be interpreted through its
  separate count;
- Authenticode and endpoint-security disposition remain unresolved.

## Security invariants

- every scan-source handle is read-only;
- every range uses checked arithmetic and source-length bounds;
- one session opens at most one source;
- frame, read, identifier, warning, state and page limits are fixed;
- peer PID/liveness and the fixed sibling desktop image are verified before
  protocol trust;
- peer image-path verification is never described as publisher or package
  authentication;
- nonce echo is never described as cryptographic authentication;
- no native path, GUID, handle, extent or source byte reaches JavaScript;
- source and destination physical disk numbers remain native-only and never
  reach JavaScript;
- destination revalidation and shell-open accept only retained native handles;
  they expose no native path, GUID, handle value, executable, verb or argument
  to JavaScript;
- the shell helper verifies directory/non-reparse state and obtains its
  bounded normalized target with `GetFinalPathNameByHandleW` from that same
  live handle before issuing the fixed `explore` request;
- the retained-directory shell worker uses one STA COM initialization and
  balances every nonnegative initialization result with one same-thread
  `CoUninitialize`; a negative HRESULT neither executes the request nor calls
  `CoUninitialize`;
- recovered labels are bounded, sanitized and rendered as text;
- recursive NTFS namespace work is bounded before a saturated sibling descent;
  saturation remains incomplete/partial evidence and unproven ancestry remains
  `Unknown`;
- the broker parses no partition table, filesystem metadata or candidate;
- tests use deterministic synthetic inputs and never a real disk.

## Evidence and release boundary

`WINDOWS-BROKER-PEER-001` fixes the expected desktop sibling derivation and
`ELEVATED-BROKER-ARCH-006` requires peer-image verification to precede
`serve_session`. `NTFS-NAMESPACE-WORK-BOUND-001` records the former 8,191-call
recursive expansion and the corrected bound of at most 600 calls for the
synthetic saturated graph. These source-level regressions do not replace native
package, signature or hostile-corpus evidence.

The retained-directory regression seam executes no Explorer process. Static
mutation tests pin the exact shell call count and callsites, unique fixed
`runas`, fixed `explore`/`SEE_MASK_NOASYNC` request, null parameters, handle-only
public API, dedicated thread and balanced COM lifecycle.

The following remain required before a release claim:

- final same-revision format, Clippy, Rust and frontend gates;
- updated static real-only and destructive-storage validators;
- native main/broker build, hashes, sibling layout and manifest extraction;
- actual Tauri visual/accessibility review without a real source scan;
- remote CI on the pushed revision;
- signed clean-machine artifacts, administrator-protected package placement and
  endpoint-security disposition.

Norton deleted one unsigned development executable on 2026-07-29. That event
remains inconclusive. It is not a confirmed false positive, and disabling
security software is not a release or acceptance gate.

## Supersession

After the same-revision delivery gates pass, this ADR supersedes
[ADR-0021](0021-real-only-image-desktop.md) only for desktop source selection,
scan execution and candidate delivery. ADR-0021 remains historical evidence.
[ADR-0003](0003-regular-image-only-cli.md) continues to govern the image CLI.

[ADR-0022](0022-windows-locality-boundary-and-path-identity.md) remains
historical and applicable to the regular-image path; SDD-018 replaces that
desktop path with opaque volume and NTFS directory identities.

## Related decisions

- [ADR-0002](0002-read-only-source-invariant.md): never-write invariant.
- [ADR-0005](0005-pull-request-ci-excludes-devices.md): no-device PR CI.
- [ADR-0008](0008-process-separation-and-uac.md): least-privilege split.
- [ADR-0009](0009-ipc-protocol.md): broker protocol principles.
- [ADR-0011](0011-ntfs-raw-parsing-and-active-record-apis.md): NTFS namespace
  evidence.
- [ADR-0027](0027-destination-capability-and-disk-separation.md): opaque
  destination authority, protocol v3 source disk identity and fail-closed
  physical-disk separation.

## Platform references

- [ShellExecuteExW function](https://learn.microsoft.com/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw):
  initialize COM before the call; some shell extensions require an STA.
- [SHELLEXECUTEINFOW structure](https://learn.microsoft.com/windows/win32/api/shellapi/ns-shellapi-shellexecuteinfow):
  `SEE_MASK_NOASYNC` is required when the calling thread has no message loop or
  will terminate after `ShellExecuteExW`.
- [CoInitializeEx function](https://learn.microsoft.com/windows/win32/api/combaseapi/nf-combaseapi-coinitializeex):
  both `S_OK` and `S_FALSE` require a matching `CoUninitialize`; failed calls
  do not.

## Revisit triggers

Revisit before adding whole-disk/unmounted authority, another filesystem folder
scope, a protocol message or wire-field change, generic IOCTL, service,
cancellation, progress, snapshot, destination filesystem, same-disk override,
restore write inside this boundary, preview, exFAT, content execution, real-disk
test, another shell verb/executable/argument, a caller-supplied shell path, or a
public artifact without completed release gates.
