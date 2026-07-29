# SDD-005 — Windows I/O and Privilege Model

Status: Target design; physical-device implementation `Not started`

## Regular image files

The current safe path opens existing regular `.img`, `.dd`, `.raw`, or approved
image files through `FileImageReader` with read enabled and write disabled.
Image scanning must not require UAC.

## Future physical sources

Physical disks and mounted volumes require a separate audited Windows boundary:

- the desktop remains unelevated;
- UAC is requested only for the read broker;
- sources are selected from broker-generated inventory, never arbitrary UI
  device paths;
- handles request `GENERIC_READ` or zero access for queries, never
  `GENERIC_WRITE`;
- IPC commands are allowlisted and contain no `WriteAt`, trim, format, lock,
  dismount, delete, or generic `DeviceIoControl`;
- caller SID, parent/session, nonce, size, timeout, and source identity are
  validated.

## Test prohibition

Pull-request CI and ordinary local tests use only memory images, deterministic
regular files, and temporary directories. Device/VHD tests require a future
isolated workflow, explicit allowlist, marker, size limit, and fail-closed guard.
No such test is part of the current increment.

