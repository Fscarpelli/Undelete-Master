# AGENTS.md — `crates/io-windows`

These instructions extend the repository root `AGENTS.md`.

## Boundary

- This is the only crate currently permitted to contain `unsafe` code.
- The accepted FFI surface is the closed read-only/query-only Windows adapter
  in ADR-0023 and SDD-018:
  - local drive, disk, mounted-volume, label, filesystem, size, sector,
    extent, and stable-identity queries, including
    `GetVolumeInformationByHandleW` solely to bind the serial of the live
    read-only volume handle during open and revalidation;
  - read-only folder-handle identity/final-volume queries for a Rust-owned
    native selection;
  - one-instance local named-pipe creation/peer-PID/liveness queries with a
    restrictive current-user DACL, plus `QueryFullProcessImageNameW` on the
    already-bound peer process handle solely to require the canonical fixed
    `undelete-master-desktop.exe` sibling of the running broker before serving;
  - fixed-path `runas` launch of the packaged read-only broker;
  - broker-internal volume open with `GENERIC_READ`, `OPEN_EXISTING`, and
    read/write/delete sharing, followed only by bounded aligned reads.
- Every `DeviceIoControl` wrapper is private and hard-codes one reviewed query
  control code. No caller may supply an IOCTL, desired-access mask, device path,
  pipe path, security descriptor, or shell verb.
- The crate must not expose or import write, trim, format, lock, dismount,
  offline, eject, mount, filesystem-mutation, arbitrary process, registry, or
  network capabilities. Source handles are never write-capable.
- Unknown roots, remote roots, and classification failures fail closed at the
  caller. Unelevated inventory is display metadata, not authorization; the
  broker independently re-enumerates and validates the expected source
  identity before and after opening.

## Change discipline

- Every `unsafe` block requires a local `SAFETY` justification that states the
  exact pointer, buffer, lifetime, and API contract.
- Adding or changing a Windows API requires an architecture/security review,
  an ADR update, and regression tests before use by the scan path.
- Keep all safe path traversal, metadata checks, and read-only file opening in
  `crates/io-common`; do not duplicate them here.
- Tests use pure DTOs, protocol doubles, ordinary temporary files/pipes, or
  deterministic image readers. Ordinary unit/PR tests never open or mutate a
  real disk or volume. A future device test requires an isolated, disposable,
  explicitly allowlisted and marked VHD plus the SDD-018 fail-closed guard.
- Preserve the residual-risk statements in ADR-0022 for ordinary image paths
  and ADR-0023 for hotplug, active-volume consistency, unsigned development
  builds, an exact GUID-plus-serial volume clone, and incomplete filesystem
  ancestry. The fixed peer-image path is not publisher verification and does
  not make a user-writable package directory trusted.
