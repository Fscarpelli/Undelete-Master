# SDD-009 — Restore Semantics

Status: `Not started`

Restore is a destination write operation, never a source mutation.

## Required behavior

- Only explicitly selected candidates are planned.
- Required ancestors may be created, but historical siblings/descendants are not
  implicitly restored.
- A selected directory alone produces an empty directory.
- Paths reject traversal, absolute injection, unexpected UNC, unauthorized ADS,
  reserved device names, trailing-dot/space hazards, and reparse escape.
- Default collision policy renames; active files are never silently replaced.
- Each file uses a destination `.umrecovering` temporary, streaming SHA-256,
  flush/verification, and atomic rename where possible.
- Partial recovery is explicit and records gaps in a sidecar/manifest.
- Same-physical-disk restore is blocked in guided mode and requires post-scan
  advanced confirmation and audit evidence.

No restore screen, progress simulation, or restore command is present in the
real-only desktop. Restore remains entirely unimplemented.
