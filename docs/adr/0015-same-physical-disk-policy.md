# ADR-0015 — Same-Physical-Disk Destination Policy

Master specification topic: 11 — same-physical-disk policy

Status: Proposed

## Context

Writing a working database or recovered file to another drive letter on the
same physical disk can overwrite recoverable source bytes.

## Proposed decision

Resolve every source, working path, and restore destination to stable physical
disk identity. Guided mode blocks same-physical-disk destinations without
override. Advanced restore may expose the master-spec post-scan typed
confirmation only after scan completion; scan working data never receives that
override.

## Security and verification consequences

Mappings fail closed when identity is unavailable or changes. Mount-point,
Storage Spaces, removable media, substituted source, drive-letter change, and
race tests are required. The former same-letter demonstration logic was
removed and is not physical-disk evidence.
