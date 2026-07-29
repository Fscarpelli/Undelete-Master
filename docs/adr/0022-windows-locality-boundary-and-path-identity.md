# ADR-0022 — Windows Locality Boundary and Path Identity

Status: Accepted

## Context

The real-only desktop accepts ordinary local image files. Lexical namespace and
extension checks alone cannot distinguish a local drive letter from a mapped
remote drive, and checking only the final path component does not reject a file
reached through a junction or another reparse-point ancestor.

The repository otherwise forbids `unsafe` code. A narrowly bounded Windows FFI
call is needed to classify a drive root without adding file, device, volume, or
write capability to the scan path.

## Decision

`crates/io-windows` is the sole audited Windows FFI boundary. Its current
Windows surface:

- extracts only an ordinary drive-letter root from an absolute path;
- calls `GetDriveTypeW` with a fixed-size, NUL-terminated UTF-16 root buffer;
- returns local or remote classification;
- fails closed for unsupported roots, unknown drive types, or missing roots;
- exposes no file-open, write, device-control, volume-control, process, shell,
  privilege, or network API.

The call is observational: it does not open or mutate the path. Its one
`unsafe` block documents the buffer and pointer contract. All other crates,
including `crates/io-common`, continue to forbid unsafe code.

`crates/io-common` remains responsible for safe path validation and source
opening. It:

1. forms an absolute path and rejects mapped-remote or unclassifiable roots;
2. inspects every normal path component with no symlink following and rejects
   symlink/reparse ancestors;
3. requires the selected object to be a regular file;
4. opens the source with read access and write access disabled;
5. on Windows, opens the final reparse point itself and validates metadata from
   the resulting handle;
6. exposes only a basename label and a redacted source identity outside the
   reader.

The Tauri WebView never supplies or receives the path. The Rust-owned picker,
CLI validation, locality boundary, read-only open, and scanner remain in one
command path.

## Residual path-identity risk

This decision does not claim a race-free path-identity guarantee.
`GetDriveTypeW`, ancestor metadata inspection, native picker selection, and the
final open are separate pathname-based operations. A drive mapping or ancestor
could change between them. Opening the final component without following a
reparse point binds the final check to that handle, but it does not retain
handles for every ancestor or bind the picker selection to a stable file
identity.

The residual is accepted only for the current unelevated, single-operator,
regular-image analyzer. The application does not claim forensic chain of
custody, privileged isolation from the same user, or support for hostile
concurrent namespace mutation. A future multi-command session, privileged
broker, shared-user boundary, or chain-of-custody claim requires retained
handles, stable file/volume identity comparison, and deterministic race tests.

## Consequences

### Positive

- mapped remote drives and reparse ancestors fail closed before scanning;
- the unsafe surface is isolated from file opening and source reads;
- the source remains read-only and no device capability is introduced;
- path and parent-directory identity remain outside reports and IPC.

### Negative

- locality classification is Windows-specific;
- pathname checks reduce exposure but do not eliminate TOCTOU;
- stronger identity binding will require additional audited Windows design.

## Verification

The current regression surface includes:

- `WINDOWS-DRIVE-TYPE-001` for remote and local drive-type classification;
- `WINDOWS-DRIVE-ROOT-001` for unsupported roots;
- `IO-REMOTE-PATH-001` for fail-closed mapped-remote classification;
- `IO-ANCESTOR-REPARSE-001` for final and ancestor junction rejection.

Tests use classification values and disposable temporary junctions, never a
real disk. Frozen-code local commands pass; native namespace acceptance and
remote CI evidence remain separate release gates.

## Revisit triggers

Revisit this ADR before adding another unsafe call, physical-device access,
volume handles, a privileged broker, persistent source sessions, forensic
chain-of-custody claims, or adversarial multi-user namespace assumptions.
