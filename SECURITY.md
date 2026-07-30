# Security Policy

## Supported status

Undelete Master is pre-release foundation software. The development desktop has
a narrow privileged read-only path for supported mounted Windows volumes, but
no current build is supported for production recovery.

## Reporting a vulnerability

Do not post source-write, privilege-boundary, parser-memory-safety, path
traversal, recovered-content execution, or secret-exposure reports in a public
issue. Use the repository host's private security advisory/reporting channel
when it is configured. If no private channel is available, contact the
repository owner privately before sharing proof-of-concept details.

Include:

- affected revision and component;
- impact and safe reproduction steps;
- whether only a deterministic image/temporary directory was used;
- relevant logs with paths, file names, content, keys, and secrets redacted.

Never attach recovered private content, real disk images, credentials, or
BitLocker recovery material.

## Safety invariants

- Scan sources are read-only by construction.
- Tests never target real disks or ordinary mounted volumes.
- The desktop process remains unelevated.
- Mounted-volume access is confined to the audited minimal broker: fixed sibling
  executable, elevation manifest, current-user local pipe, opaque source
  identity, bounded reads, and no source-write or generic IOCTL operation.
- Inventory alone is unelevated. Only an explicit user scan launches the broker
  and may show a Windows UAC prompt.
- Recovered files are untrusted and never executed or privileged-previewed.
- Parser arithmetic and reads remain bounded to the supplied region.
- Destination restore paths must remain contained and must not follow reparse
  points.

## Security release gates

Production distribution requires a reviewed threat model, broker/IPC tests,
source-write invariant proof, path-containment review, dependency and license
audit, secret scan, SBOM, Tauri CSP/capabilities audit, signed artifacts with
reputation evidence, and retained evidence. The local unsigned development
binary is not a production release.

Current gaps are listed in
[SDD-015](docs/specs/015-known-limitations.md) and the
[risk register](docs/risk-register.md).
