# SDD-004 — Architecture

Status: Normative target; current implementation is partial

## Required process boundaries

1. **Desktop:** unelevated Tauri/React coordinator; no raw writable handle and
   no hostile preview decoding.
2. **Read broker:** future minimal elevated process; inventory and bounded reads
   only, with no restore or generic device-control command.
3. **Sandbox worker:** future unelevated restricted validator/preview process
   without network.
4. **Restore helper:** optional future process, separate from the read broker,
   limited to approved destination paths.

Only the OS-independent Rust libraries and Vite demonstration exist today. A
browser demonstration is not the desktop production boundary.

## Module boundaries

- `crates/core`: domain types and traits; no I/O or OS dependency.
- `crates/io-common`: read-only regular image readers.
- `crates/partition`: bounded MBR/GPT parsing.
- `crates/fs-*`: OS-independent filesystem scanners.
- `crates/fixture-builder`: deterministic synthetic images and truth manifests.
- `crates/cli`: current-increment image-only composition; no devices or restore.
- `apps/desktop`: demonstration until the Tauri shell and real provider exist.

## Current-increment data flow

```text
regular image file
  -> FileImageReader (read-only)
  -> partition discovery
  -> bounded RegionReader
  -> NTFS or FAT metadata scanner
  -> sanitized JSON report
```

No current-increment path may accept a device selector, create a write-capable
source handle, restore content, or claim production exFAT support.

