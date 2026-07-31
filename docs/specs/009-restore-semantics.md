# SDD-009 — Restore Semantics

Status: `Partial` — the native transactional engine, destination authority,
plans, jobs, cancellation, manifest publication, and safe open-destination
operation are implemented and locally verified. The actionable desktop
workflow is implemented. Packaged-host discovery was observed from the release
binary, while a governed deleted-file recovery to a separate physical NTFS
disk remains unverified.

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
  flush/verification, and capability-relative no-clobber hard-link
  publication. There is no replacement rename or copy fallback.
- Partial recovery is explicit and records gaps in a sidecar/manifest.
- Same-, unknown-, multi-, virtual-, composite-, and otherwise unproven
  physical-disk destinations are rejected. There is no advanced override.

## Implementation status

| Requirement | Current status | Evidence |
| --- | --- | --- |
| FR-080 destination | Partial | The native picker retains an opaque NTFS directory authority and requires one different proven physical disk. Preserve-tree recovery is implemented; flatten/by-type layouts are not. |
| FR-081 minimal ancestors | Implemented-unverified | Typed sanitized paths create only ancestors required by explicitly selected items. |
| FR-082 selected folder alone | Implemented-unverified | A selected directory publishes only that empty directory. Historical descendants are never implicit. |
| FR-083 collisions | Implemented-unverified | Deterministic rename variants and capability-relative no-clobber publication preserve existing files. |
| FR-084 transaction | Implemented-unverified | Bounded streaming, hash verification, durability journal, protected temporary links, atomic no-clobber publication, and terminal manifest are implemented. Packaged-host acceptance is pending. |
| FR-085 partial files | Partial | Exact zero-filled ranges, sidecars, policy binding, manifests, and explicit desktop consent are implemented. Truncate/separate-segment policies and validator-based policy advice are absent. |
| FR-086 destination metadata | Not started | ACL, EFS, ADS, and optional metadata preservation are not implemented or claimed. |
| FR-087 manifest | Partial | Versioned JSON manifest with hashes, ranges, errors, warnings, and reconciliation status is implemented. Optional CSV is absent and unclaimed. |
| FR-088 resume | Not started | Jobs are bounded and observable during one process lifetime; durable resume after restart is not implemented. |

The six native restore commands, together with six storage/query commands, are
registered without exposing paths, extents, offsets, handles, disk identities,
executables, or recovered bytes to the WebView. The desktop controls render
only native snapshots for selection, planning, progress, cancellation,
completion and destination opening. The release pair, embedded elevation
levels and hashes are recorded in the Task 7 evidence; safe real-device
deleted-file recovery remains a separate acceptance gate.
