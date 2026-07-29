# SDD-004 — Architecture

Status: Normative target; image-only desktop slice `Implemented-unverified`

## Implemented boundary

1. **Desktop:** unelevated Tauri 2 + React process with one command,
   `select_and_scan_image(requestId)`.
2. **Scanner composition:** `um-cli` library validates and scans an ordinary
   local image on `spawn_blocking`.
3. **Readers/parsers:** OS-independent, bounded Rust modules; the source trait
   has no write operation.

The native picker runs inside Rust. The selected path never crosses IPC. The
WebView receives only a bounded sanitized report. Browser-only execution fails
closed.

```text
React requestId
  -> Rust-owned native picker
  -> um_cli::scan_image_path
 -> FileImageReader (read-only)
  -> atomic GPT-copy / bounded MBR partition discovery
  -> bounded RegionReader
  -> NTFS metadata scanner; FAT only when NTFS is not recognized
  -> ImageScanReport schema 2 with per-volume scanStatus
  -> bounded DesktopScanReport adapter
```

## Scan truth and failure semantics

Partition-table discovery falls back to a whole-image volume only when no
partition format is recognized. A corrupt GPT copy is not accepted
header-only: its header and entry table validate atomically. Both canonical
copies are evaluated even when the primary is usable. The backup header is read
only at the final logical block, points reciprocally to LBA 1, and keeps its
entry array in reserved metadata space. If both headers are valid, shared
metadata must agree or discovery fails closed. An independently valid backup
may be used after a primary read, header, or table failure. A protective `0xEE`
MBR is never exposed as an ordinary partition when both GPT copies are
unusable.

Filesystem probing is likewise classification, not error recovery. FAT is
tried only after NTFS returns `NotRecognized`; a recognized parser's `Read` or
`Corrupt` error remains typed and fails the volume scan. Schema-version-2
reports require `complete`, `partial`, or `unrecognized` per volume.
Recognized NTFS and FAT volumes may be `partial`; the desktop preserves that
state, warnings, and the coverage caveat.

## Future boundaries

The following are architectural targets, not current code:

1. **Read broker:** minimal elevated process for inventory and bounded physical
   reads, without restore or generic device control.
2. **Sandbox worker:** restricted unelevated validator/preview process without
   network.
3. **Restore helper:** separate destination-only process with handle-based
   containment.

## Module boundaries

- `crates/core`: domain types and traits; no I/O or OS dependency.
- `crates/io-common`: read-only regular image readers.
- `crates/partition`: bounded MBR/GPT parsing.
- `crates/fs-*`: OS-independent filesystem scanners.
- `crates/fixture-builder`: deterministic synthetic test images and truth
  manifests.
- `crates/cli`: shared image-scan composition and headless entry point.
- `apps/desktop/src-tauri`: unelevated native boundary and DTO adapter.
- `apps/desktop/src`: real-only presentation, report validation and effective
  local preferences.

No implemented path accepts a WebView-supplied source path, device selector,
write-capable handle, restore content, or production exFAT claim.
