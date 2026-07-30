# SDD-005 — Windows I/O and Privilege Model

Status: Normative; mounted-volume implementation `Implemented-unverified`

## Unelevated inventory

The desktop discovers mounted drive-letter volumes without UAC. This step uses
mounted-volume metadata only: no DASD open, IOCTL, disk extent, canonical
length or physical-disk mapping. Remote, CD-ROM, RAM-disk and unknown roots are
not scan eligible.

Logical groups have no physical disk number. The stable opaque volume ID uses
volume GUID plus serial and excludes mount, filesystem, quota-visible
size/free values, extents and disk number. Values returned by
`GetDiskFreeSpaceExW` are display/quota-visible only and never authority.

## Native folder authority

The native folder picker is available only for an eligible NTFS volume. Rust
rejects non-local, reparse and cross-volume selections and retains volume
serial plus the NTFS file reference (MFT record and sequence). Only an opaque
scope ID and sanitized label cross IPC. FAT has no folder scope.

## Query-only destination authority

The SDD-020 Task 3 native API admits a Rust-owned destination selection without
creating or modifying an entry. It opens the exact selected directory with
`FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY`, read/write sharing without
delete sharing, `OPEN_EXISTING`, and
`FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT`. The directory
must be non-reparse and its volume serial must agree with the volume reached
through `GetFinalPathNameByHandleW` on that same retained handle.

Only the volume GUID derived from that handle may then be opened, with desired
access `0`, read/write/delete sharing, `OPEN_EXISTING`, and
`FILE_ATTRIBUTE_NORMAL`. The query requires NTFS, one distinct physical disk
(which may have multiple extents), and a reviewed direct ATA, SATA, USB, or
NVMe bus value. Missing, composite, virtual, file-backed, Storage Spaces,
array/network, unknown, and future values fail closed. The retained directory
handle and physical-disk number remain native; this capability is not a Tauri
command and no destination path or disk number crosses the WebView boundary.

The restore-admission policy requires the source disk captured with the scan,
the freshly reopened source disk, and the destination disk all to be known
single-disk identities. The two source values must agree and the destination
must differ. Disk number `0` is valid. This is a fail-closed OS identity check,
not hardware attestation: a hypervisor or malicious storage stack can emulate
a reviewed direct bus.

## Elevated read-only broker

The main manifest is `asInvoker`; the fixed sibling broker manifest is
`requireAdministrator`. UAC occurs only after explicit scan activation.

The client creates a current-user, one-instance local named pipe and accepts
only the launched broker PID. Protocol v3 then uses exact nonce echo, monotonic
sequences, ten messages and a maximum 1 MiB payload. Peer PID remains the
primary identity check; nonce echo is not a MAC.

The broker independently resolves the opaque volume ID, then queries canonical
length, sector geometry, disk extents, and the fixed storage-device bus
property. Missing, multi-disk, virtual/file-backed, Storage Spaces,
array/network, unknown, and future mappings fail closed. Only then does it open
the internal mounted-volume selector with exactly `GENERIC_READ`, compatible
sharing and `OPEN_EXISTING`. Production derives the authoritative single
physical-disk number from the mounted-volume handle; it does not open
`PhysicalDriveN`.

Every valid read is range-checked and reaches `ReadFile` unless identity
revalidation first fails. Expensive identity enumeration occurs only after
both 256 valid reads and one elapsed second. The source stays live and mutable;
no snapshot, lock or dismount occurs.

## Audited native allowlist

`crates/io-windows` is the sole Windows FFI boundary for:

- mounted-volume discovery, identity, length and sector geometry;
- the query-only IOCTLs for volume extents, disk length, storage alignment, and
  the fixed `StorageDeviceProperty` classification;
- read-only NTFS directory identity, including the exact
  `FILE_READ_ATTRIBUTES` folder open;
- query-only destination admission, including the retained destination-root
  open with exact query/list access and no delete sharing, and the fixed
  desired-access-zero open of only its handle-derived volume GUID;
- local named-pipe creation/connect and peer PID/liveness, including the sole
  duplex `CreateFileW` call;
- fixed sibling elevation;
- read-only mounted-volume `CreateFileW` open, seek and read.

`GENERIC_WRITE` is permitted only on the duplex named-pipe transport. It is
forbidden for a scan source. No write, trim, format, delete, repair, lock,
dismount, mount or arbitrary `DeviceIoControl` capability is allowed. Every
`CreateFileW` call is restricted to exactly one of the five audited functions
above, with all seven arguments fixed by that function's policy. Static
validation permits exactly one canonical, unaliased
private top-level `windows_sys::Win32::Storage::FileSystem::CreateFileW`
import and five bare, unqualified direct call references outside macro token
trees. The inventory uses comment/string-aware Rust significant tokens,
rejects every raw-identifier spelling such as `r#CreateFileW`, and cannot be
bypassed with comments or whitespace around `::` or `.`. Address-taking,
function-pointer binding, public/imported/module aliases, qualified calls,
macro-wrapped imports or calls, macro references and every other symbol use
fail closed. Foreign linkage is separately pinned before ABI/link literals are
masked: the sole allowed declaration is the exact `shell32`
`ShellExecuteExW` block. `link_name`, any other `extern` declaration, and
dynamic symbol resolution APIs are forbidden. The `io-windows` general and
Windows-target dependency inventories and the desktop Tauri build/runtime/test
dependency inventories are exact. The root workspace dependency inventory is
also exact, and every dependency in every other first-party manifest must be
either a `workspace = true` entry from that inventory or one of the
path-specific pinned Windows/Tauri entries; first-party proc-macro crate forms
are rejected. Cargo patch/replace tables and repository Cargo source
configuration are forbidden, so the pinned package sources cannot be locally
substituted. Within all `io-windows` sources, only the reviewed compiler/std
macro names may be invoked, qualified or nested unreviewed macros are
rejected, and approved names cannot be rebound through macro definitions,
attributes, imports, or external globs. This closes both direct and transitive
proc-macro token-synthesis routes. First-party Cargo manifests are
additionally checked by resolved package identity, including `package =`
aliases, for dynamic-loader families, so a renamed safe-loader dependency
cannot bypass literal-source checks.

## Regular image CLI

The CLI continues to open approved regular image files through
`FileImageReader` with write disabled and without UAC. The desktop no longer
uses an image picker.

## Test prohibition

Pull-request CI and ordinary local tests use only deterministic in-repository
images, synthetic readers and temporary ordinary files/directories. They do not
open a real disk or volume, attach a VHD, format, trim, lock or dismount media.
Read-only live inventory may be observed manually, but it is not evidence of a
successful real-volume scan.
