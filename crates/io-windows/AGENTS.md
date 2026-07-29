# AGENTS.md — `crates/io-windows`

These instructions extend the repository root `AGENTS.md`.

## Boundary

- This is the only crate currently permitted to contain `unsafe` code.
- The permitted FFI surface is read-only locality classification through
  `GetDriveTypeW` using a fixed-size, NUL-terminated drive-root buffer.
- The crate must not open files, devices, volumes, processes, registry keys, or
  network resources. It must not expose write, trim, format, dismount,
  filesystem-control, device-control, or privilege APIs.
- Unknown roots, remote roots, and classification failures fail closed at the
  caller. Classification is not authorization and does not prove stable file
  identity.

## Change discipline

- Every `unsafe` block requires a local `SAFETY` justification that states the
  exact pointer, buffer, lifetime, and API contract.
- Adding or changing a Windows API requires an architecture/security review,
  an ADR update, and regression tests before use by the scan path.
- Keep all safe path traversal, metadata checks, and read-only file opening in
  `crates/io-common`; do not duplicate them here.
- Tests use drive-type values, ordinary temporary files, or disposable
  allowlisted fixtures. They never open or mutate a real disk or volume.
- Preserve the residual-risk statement in ADR-0022: drive classification,
  ancestor inspection, native selection, and final open are not one race-free
  retained-handle operation.
