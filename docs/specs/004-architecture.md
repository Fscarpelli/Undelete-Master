# SDD-004 — Architecture

Status: Normative; connected-volume desktop `Implemented-unverified`

## Current desktop boundary

The Windows desktop is a real mounted-volume scanner:

```text
React WebView
  -> list_storage_sources
  -> select_scan_folder
  -> scan_storage_volume
  -> get_candidate_page
  -> query_candidate_page / update_candidate_selection
  -> select_restore_destination / create_restore_plan
  -> start_restore / get_restore_job / cancel_restore
  -> open_restore_destination
  -> unelevated Rust scan/results/restore coordinators
     source-read branch:
       -> broker-backed SourceReader
       -> elevated fixed sibling broker
       -> selected mounted volume, read-only
     destination-write branch:
       -> crates/restore
       -> capability-relative destination writes
       -> retained NTFS folder on one proven different physical disk
```

The WebView supplies bounded request IDs and opaque volume, folder-scope, scan
cursor, destination, plan and job IDs. It never supplies or receives a native
path, device name, volume GUID, physical-disk number, pipe name, source or
destination handle, raw extent, offset, executable, source byte or recovered
byte.

Inventory is unelevated and presents mounted local volumes in logical groups.
It neither maps nor exposes a physical disk number and performs no DASD/IOCTL
query. Scanning opens the selected mounted volume; a logical group does not
authorize `PhysicalDriveN`, an unmounted partition or a whole-disk scan.

Starting a scan launches the fixed sibling elevated broker. The broker speaks
the closed protocol-v3 read lifecycle and performs no partition/filesystem
parsing. The broker-backed reader is passed to
`um_cli::scan_volume_reader` on `spawn_blocking`; partition, NTFS and FAT
parsing therefore remains unelevated.

The optional folder picker is native. NTFS volume serial, MFT record and
sequence bind the folder authority. Candidate ancestry is classified as
`Match`, `NoMatch` or `Unknown`; only proven matches become result rows and the
unknown count stays explicit.

Completed scans retain bounded native candidate authority. Search, dynamic
extension facets, evidence filters, stable sorting, cursor pagination and
selection execute against that native state; the WebView renders at most one
100-row page and never expands selected candidate IDs into caller authority.

Restore destination selection is also native. `io-windows` retains a query-only
NTFS directory authority with final-volume, reparse and physical-backing
evidence. Planning and start revalidate a single proven physical disk different
from the source. `crates/restore` performs bounded source reads through
`SourceReader` and capability-relative destination writes outside the broker,
preserving the tree and publishing with deterministic rename/no-clobber
semantics. Progress, cooperative cancellation, partial sidecars, terminal
counts and manifest identity come from the native job. Opening a completed
destination uses only the retained authority and never executes a recovered
file.

## CLI boundary

The headless CLI remains a separate regular-image workflow:

```text
regular image path
  -> FileImageReader (read-only)
  -> um_cli::scan_image_path
  -> partition + filesystem scanners
  -> bounded machine-readable report
```

SDD-018 supersedes SDD-017 only for the desktop. ADR-0003 continues to govern
the image CLI.

## Scan truth and failure semantics

Partition and filesystem classification never treats a recognized parser error
as permission to relabel a volume unrecognized. NTFS and FAT may report
`complete` or `partial`; an unrecognized filesystem remains `unrecognized`.
Warnings and coverage limits survive the native adapter.

A folder result is not a directory walk and does not use display-path prefix
matching. Missing, corrupt, reused or ambiguous ancestry is unknown, not
outside. A live mounted volume is mutable, so the application does not claim a
forensic snapshot.

## Process and privilege boundaries

1. `undelete-master-desktop.exe`: `asInvoker`; WebView, coordination, parsers,
   namespace filtering, sanitation, native result/selection authority,
   destination capability retention, restore planning, bounded destination
   writes and presentation.
2. `undelete-master-broker.exe`: `requireAdministrator`; fixed sibling,
   short-lived, local pipe, one identity-bound mounted-volume source and bounded
   reads only.
3. Preview, recovered-content validation and execution boundaries remain
   absent. Restore is implemented in the unelevated desktop/`crates/restore`
   boundary and does not add a write operation to the broker.

The protocol has exactly ten messages and no write, arbitrary path,
destination or generic control field. Peer PID is primary identity. The
32-byte nonce echo is checked in constant time but is not a MAC or secret.

## Module boundaries

- `crates/core`: domain types and `SourceReader`; no I/O or OS dependency.
- `crates/io-common`: regular image readers used by the CLI.
- `crates/io-windows`: single audited Windows FFI boundary with closed
  query/identity/pipe/process/read-only-volume, query-only destination and fixed
  folder-open allowlists.
- `crates/broker-protocol`: safe protocol-v3 framing, schema and bounds.
- `crates/broker-client`: unelevated broker session and `SourceReader` adapter.
- `crates/elevated-broker`: privileged protocol server; no filesystem parser.
- `crates/partition`: bounded MBR/GPT parsing.
- `crates/fs-*`: OS-independent filesystem scanners and NTFS namespace
  evidence.
- `crates/fixture-builder`: deterministic synthetic test images and truth
  manifests.
- `crates/cli`: shared image and already-open-volume scan composition.
- `crates/restore`: typed restore plans, bounded stream/hash, path sanitation,
  capability-relative transaction publication, journals, sidecars and
  manifests; no source-open or privileged dependency.
- `apps/desktop/src-tauri`: exact 12-command native facade, bounded in-memory
  scan/result/selection/destination/plan/job authorities and background restore
  coordination.
- `apps/desktop/src`: typed real-only presentation, actionable results
  workspace, restore workflow and effective local preferences.

Preview, recovered-content execution, exFAT, persistent sessions, restart
resume, scan progress/cancellation, hotplug guarantees and whole-physical-disk
authority are not implemented. Deep carving is plugin based and currently
limited to contiguous JPEG on whole NTFS volumes with trusted allocation
evidence. Packaged real-device restore acceptance remains unverified.
