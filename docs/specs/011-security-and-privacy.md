# SDD-011 — Security and Privacy

Status: Normative; implementation `Partial`

## Protected assets

Source integrity, recoverable bytes, recovered content, administrative
privilege, source/session identity, local paths, manifests, and result
credibility.

## Mandatory controls

- Read-only source interfaces and no source-write command.
- Checked, bounded Rust parsing; `unsafe` only in a future audited Windows FFI
  boundary.
- Unelevated UI and separate least-privilege broker/worker boundaries.
- No network, shell, remote content, arbitrary URL/path opening, or privileged
  preview.
- Destination path containment, no reparse following, collision policy, and
  source/destination physical-disk validation.
- Lockfiles, dependency/license review, secret scanning, SBOM, and signed release
  evidence before production distribution.

## Privacy

No account, cloud, or telemetry is required. Default logs and JSON reports omit
content, secrets, recovery keys, and full local paths. Diagnostic export is
manual, previewed, local, and redacted.

## Current boundary

Only regular image readers and OS-independent parsers exist. The broker, IPC,
worker sandbox, production Tauri CSP/capabilities, restore boundary, and release
signing are not implemented.

