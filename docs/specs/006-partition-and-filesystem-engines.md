# SDD-006 — Partition and Filesystem Engines

Status: Normative behavior with component-level partial implementation

| Engine | Required scope | Current honest status |
| --- | --- | --- |
| MBR | Primary entries, EBR loop protection, bounds, overlap warnings. | Verified for the implemented deterministic component scope; external-corpus acceptance remains open. |
| GPT | Canonical primary/backup headers and entry tables, reciprocal metadata, reserved areas, CRCs, names, bounds, overlap, atomic-copy validation and corruption handling. | Verified for the implemented deterministic component scope, including fallback, conflicting-valid-copy, and protective-MBR regressions; external-corpus acceptance remains open. |
| NTFS | Boot/MFT, deleted records, paths, resident/nonresident extents, bitmap conflicts, bounded corruption handling and structural completeness. | Partial; logical MFT-size/signature checks and deterministic coverage exist, while compression, full attribute lists, ADS workflow, MFT mirror and enrichment remain incomplete. |
| FAT12/16/32 | BPB, FAT copies/chains, deleted entries/directories, LFN, bounded inference, loop detection and structural completeness. | Partial; deterministic completeness coverage exists, while the production matrix remains incomplete. |
| exFAT | Boot regions, checksums, bitmap, upcase, entry sets, NoFatChain, fragmented files and directories. | Not started for production; incomplete files are excluded from the workspace. |
| ReFS/unknown | Detect and offer explicitly limited RAW/carving behavior only. | Not started. |

## Common parser requirements

- Every range is bounded to the supplied source region.
- Arithmetic is checked before allocation or slicing.
- Corruption produces typed errors or provenance warnings, never invented data.
- Inference reduces confidence and score.
- Tests use deterministic fixtures; byte recovery is asserted with SHA-256.
- A passing synthetic case does not establish production filesystem support.

## GPT copy selection

Each GPT candidate is a complete unit: its header, header CRC, entry-array
range, entry-array CRC, and bounded entries must validate before any partition
is returned. Both canonical copies are evaluated even when the primary is
usable. The backup header is read only from the final logical block, must point
back to primary LBA 1, and must place its entry array between the usable range
and its own header. A valid primary header must point to that canonical final
LBA; when both headers validate, usable range, disk GUID, entry count/size, and
entry-array CRC must agree reciprocally or discovery fails closed.

A primary header read failure, invalid primary header, or unusable primary
entry table causes the canonical backup copy to be evaluated. If neither copy
is usable, a protective MBR entry with type `0xEE` is not surfaced as a data
partition or as evidence that GPT succeeded.

Focused coverage: `PART-GPT-BACKUP-001` through
`PART-GPT-BACKUP-005` and `PART-GPT-PROTECTIVE-001`.

## NTFS bounded completeness

`NtfsScanOutput::is_complete` is false when a known safety or evidence boundary
prevents exhaustive MFT enumeration: the MFT work cap, a trusted physical
prefix shorter than the declared stream, skipped unreadable/torn/corrupt
records, a nonblank record without the `FILE` signature, or extension-record
merge failures. Malformed attribute headers, names, bodies, end markers, or
`$ATTRIBUTE_LIST` content in an ordinary or extension record also make the scan
incomplete. The current merge pass does not prove complete resolution of every
reference and candidate-defining attribute, so any `$ATTRIBUTE_LIST`—including
a well-formed resident value—also keeps `is_complete` false. The unnamed `$MFT`
stream is rejected as corrupt unless its logical `data_size` is record-aligned
and covers all
`FIRST_USER_RECORD` (16) reserved record slots; it cannot become an empty or
reserved-only complete scan. Malformed record-0 attributes are likewise fatal
because that record bootstraps the MFT stream. The CLI preserves recoverable
incompleteness as `scanStatus: "partial"`; candidate counts are observations
within the scanned prefix, not totals.

The `$Bitmap` read and backing allocation are capped at 64 MiB. Bits beyond the
retained prefix are `Unknown` rather than inferred free or allocated, and a
bounded warning explains the cap. Focused coverage includes
`NTFS-BITMAP-BOUND-004`, `NTFS-MFT-BOUND-003`, `NTFS-MFT-SIZE-001`,
`NTFS-RECORD-SIGNATURE-001`, `NTFS-ATTRIBUTE-BOUNDS-001`, and
`NTFS-ATTRIBUTE-LIST-PARTIAL-001`. Full filesystem compatibility, fuzz, and
performance evidence remain open.

## FAT bounded completeness

The BPB permits one through four total FAT copies. The first copy remains
authoritative. `FatScanOutput::is_complete` is false when any declared
secondary copy diverges from it or is unreadable; a reachable directory chain
is broken, cyclic, or starts at an unusable cluster; a descended directory
cannot be read; or enumeration stops at `MAX_DIRS` (10,000 directories),
`MAX_DIR_BYTES` (8 MiB per directory stream), or `MAX_DIR_DEPTH` (256 levels).
The maximum directory-chain cluster count is derived from
`MAX_DIR_BYTES / cluster_size` and passed into `FatTable::chain`, bounding the
chain vector before cluster reads; reaching that bound makes the scan partial.
Warnings preserve copy disagreement/read failure and the boundary reached.

The declared size of one FAT copy above `MAX_FAT_READ_BYTES` (64 MiB) is
rejected as `ScanError::Corrupt` before allocating or reading the table. A
fatal first-copy/root read likewise returns a typed error rather than a partial
report.

The CLI maps a returned incomplete FAT scan to `scanStatus: "partial"` without
changing its filesystem kind. Focused coverage is
`FAT-COMPLETENESS-001` through `FAT-COMPLETENESS-004`,
`FAT-TABLE-BOUND-001`, `FAT-DEPTH-BOUND-001`,
`FAT-DIRECTORY-CHAIN-BOUND-001`, and the cross-layer
`CLI-IMAGE-FAT-PARTIAL-001`. Production compatibility and exhaustive
cycle/read/budget corpora remain open.
