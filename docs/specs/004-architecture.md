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
  -> unelevated Rust coordinator
  -> broker-backed SourceReader
  -> elevated fixed sibling broker
  -> selected mounted volume, read-only
```

The WebView supplies bounded request IDs and opaque volume, folder-scope, scan
and cursor IDs. It never supplies or receives a native path, device name,
volume GUID, pipe name, source handle, raw offset or source bytes.

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
   namespace filtering, sanitation and presentation.
2. `undelete-master-broker.exe`: `requireAdministrator`; fixed sibling,
   short-lived, local pipe, one identity-bound mounted-volume source and bounded
   reads only.
3. Future preview/validation and restore boundaries remain absent.

The protocol has exactly ten messages and no write, arbitrary path,
destination or generic control field. Peer PID is primary identity. The
32-byte nonce echo is checked in constant time but is not a MAC or secret.

## Module boundaries

- `crates/core`: domain types and `SourceReader`; no I/O or OS dependency.
- `crates/io-common`: regular image readers used by the CLI.
- `crates/io-windows`: single audited Windows FFI boundary with closed
  query/identity/pipe/process/read-only-volume allowlists.
- `crates/broker-protocol`: safe protocol-v3 framing, schema and bounds.
- `crates/broker-client`: unelevated broker session and `SourceReader` adapter.
- `crates/elevated-broker`: privileged protocol server; no filesystem parser.
- `crates/partition`: bounded MBR/GPT parsing.
- `crates/fs-*`: OS-independent filesystem scanners and NTFS namespace
  evidence.
- `crates/fixture-builder`: deterministic synthetic test images and truth
  manifests.
- `crates/cli`: shared image and already-open-volume scan composition.
- `apps/desktop/src-tauri`: four-command native facade, bounded in-memory
  authorities and candidate pages.
- `apps/desktop/src`: typed real-only presentation and effective local
  preferences.

Restore, preview, carving, exFAT, persistent sessions, cancellation, hotplug
guarantees and whole-physical-disk authority are not implemented.
