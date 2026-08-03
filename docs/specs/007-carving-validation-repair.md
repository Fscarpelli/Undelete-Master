# SDD-007 — Carving, Validation, and Repair

Status: `Partial`

## Current implemented boundary

The first bounded, product-integrated slice is specified by
[SDD-019](019-ntfs-coverage-and-jpeg-deep-scan.md) and
[ADR-0024](../adr/0024-streaming-mft-and-bounded-content-carving.md).
`crates/carving` is a real read-only, bounded contiguous-JPEG engine over
caller-supplied `SourceReader` regions. It validates incrementally with bounded
buffers, applies signature-attempt and aggregate-validation-byte budgets, and
links each candidate to physical range, exact-range SHA-256, and validator
version.

The CLI library, Tauri adapter and desktop now integrate one narrow product
slice: an explicit `deepJpeg` mode for a whole mounted NTFS volume, using only
coalesced `$Bitmap`-proven `FreeInSnapshot` ranges. Metadata remains the safe
default. The desktop receives real scan phases and elapsed time; MFT
enumeration has measured percentage and a rate-based ETA, while carving phases
remain indeterminate because they do not expose a trustworthy total. There is
still no deep option in the `scan-image` process command, no folder deep scan,
persistent result store, cooperative scan cancellation, or separate
validation/repair pipeline. Eligible retained candidates can be extracted only
through the transactional restore workflow specified by SDD-020.

## Carving

Product carving must scan bounded unallocated regions only when allocation
evidence is reliable. The first NTFS integration is restricted to a
whole-volume selection. Folder-scoped carving is forbidden because a signature
hit has no proven historical folder ancestry.

The portable component accepts explicit regions for deterministic testing and
reuse; that API is not product authority. The current integrator submits only
ranges proven `FreeInSnapshot`, excludes unknown/allocated/conflicted ranges,
and never silently expands a free-space scan into a whole-volume RAW scan.
Untrusted or unavailable `$Bitmap` semantics produce zero submitted regions
and partial coverage, not a RAW fallback.

An explicit damaged/unknown-filesystem RAW mode remains future work. Built-in
carvers are compiled with the product. User extensions remain future
declarative, schema-limited signatures; arbitrary DLLs and scripts are
forbidden.

The current JPEG slice is contiguous only. It does not claim generic
fragmentation recovery, perfect visual decode, or coverage of PNG, PDF, ZIP,
Office, audio, video, executable, or arbitrary formats.

## Validation

The JPEG component's bounded marker parser is discovery-time structural
validation, not a sandboxed preview:

- SOI, bounded marker/segment lengths, non-zero SOF dimensions, SOS and EOI are
  required;
- entropy byte stuffing and restart markers are handled within explicit
  candidate/region/read budgets;
- malformed or truncated structures are rejected without writing partial
  output;
- the component does not decode pixels.

Broader recovered-content validators remain future restricted, networkless
worker functionality with strict file-size, CPU, memory, recursion, expansion,
and time limits. A validator crash must not terminate the scan.

## Preview

Preview is not implemented. Future preview must escape text/HTML, forbid
external navigation, limit decoded dimensions and byte windows, and never
execute macros, scripts, DLL entry points, or recovered applications. No
preview runs in the broker or carving component.

## Repair

Repair is not implemented. Repairs must be deterministic derivatives. The
original extraction remains unchanged; both files receive hashes and a
provenance report listing every transformation. Repair must never fabricate
original bytes.

## Status matrix

| Capability | Status |
| --- | --- |
| Bounded contiguous-JPEG component | Implemented-unverified |
| Incremental JPEG validation plus signature/validation budgets | Implemented-unverified |
| Physical range, SHA-256 and validator version through candidate-page schema 2 | Implemented-unverified |
| Hardened NTFS `$Bitmap` authority and proven-free region iterator | Implemented-unverified |
| Whole-volume admission and submission of free ranges to the carver | Implemented-unverified |
| Explicit whole-NTFS deep-scan Tauri/UI path | Implemented-unverified |
| Deep mode in the `scan-image` process command | Not started |
| Folder carving | Intentionally unsupported |
| Real-time scan progress | Partial — native phase/elapsed feedback and measured MFT percentage/ETA exist; later carving phases remain indeterminate |
| Cooperative scan cancellation | Not started |
| Fragmented JPEG reconstruction | Not started |
| Additional baseline formats | Not started |
| Sandboxed validators/preview | Not started |
| Repair derivatives | Not started |

No screen, candidate count, signature match, or focused test is evidence that
the unimplemented behaviors above or arbitrary real-media recovery are
complete.
