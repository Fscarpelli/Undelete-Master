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
sharing and `OPEN_EXISTING`.

Every valid read is range-checked and reaches `ReadFile` unless identity
revalidation first fails. Expensive identity enumeration occurs only after
both 256 valid reads and one elapsed second. The source stays live and mutable;
no snapshot, lock or dismount occurs.

## Audited native allowlist

`crates/io-windows` is the sole Windows FFI boundary for:

- mounted-volume discovery, identity, length and sector geometry;
- the query-only IOCTLs for volume extents, disk length, storage alignment, and
  the fixed `StorageDeviceProperty` classification;
- read-only NTFS directory identity;
- local named-pipe creation/connect and peer PID/liveness;
- fixed sibling elevation;
- read-only mounted-volume open, seek and read.

`GENERIC_WRITE` is permitted only on the duplex named-pipe transport. It is
forbidden for a scan source. No write, trim, format, delete, repair, lock,
dismount, mount or arbitrary `DeviceIoControl` capability is allowed.

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
