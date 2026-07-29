# ADR-0011 — NTFS Raw Parsing and Active-Record APIs

Master specification topic: 6 — NTFS raw parsing versus active-record APIs

Status: Proposed

## Context

Raw NTFS parsing is required for deleted metadata, while supported Windows APIs
may provide more reliable information for active records.

## Proposed decision

Use the bounded, buffer-based NTFS engine for deleted records and on-image
evidence. A future Windows adapter may enrich active-record evidence through
documented read-only APIs, but never replace raw deleted-record parsing or
silently override conflicting evidence. Preserve provenance for every source.

## Current raw-parser completeness boundary

The implemented raw parser exposes `NtfsScanOutput::is_complete`. It is false
when any known boundary prevents exhaustive MFT candidate enumeration,
including:

- the configured MFT byte/record work cap;
- a trusted physical MFT prefix shorter than the declared data stream;
- a nonblank record inside the trusted prefix that lacks the `FILE` signature;
- unreadable, torn, or corrupt records that are skipped;
- malformed attribute headers, names, bodies, end markers, or
  `$ATTRIBUTE_LIST` content in an ordinary or extension record;
- any `$ATTRIBUTE_LIST`, including a well-formed resident value, until every
  reference and candidate-defining attribute has been completely resolved;
- an extension-record offset/read/parse failure during stream merge.

The unnamed `$MFT` stream's logical `data_size` is a structural prerequisite,
not a completeness hint. It must be record-aligned and span at least
`FIRST_USER_RECORD` (16) reserved record slots. Zero, an aligned value below 16
records, or any unaligned value returns `ScanError::Corrupt` before
enumeration; it must never produce an empty or reserved-only
`is_complete=true` result.

Record 0 is the bootstrap authority for the MFT stream. Malformed record-0
attributes therefore return `ScanError::Corrupt`; the same structural
attribute failure in an ordinary or extension record makes the returned scan
incomplete and emits a bounded warning.

The allocation `$Bitmap` read and backing allocation are capped at 64 MiB.
Cluster states whose bits lie beyond the retained prefix remain `Unknown`;
they are never inferred allocated or free. The cap emits a bounded warning.

This structural completeness value and its warnings must survive through the
CLI schema and desktop DTO. A candidate count from an incomplete scan is a
bounded observed count, not an exhaustive total. Focused regressions are
`NTFS-MFT-SIZE-001`, `NTFS-RECORD-SIGNATURE-001`,
`NTFS-ATTRIBUTE-BOUNDS-001`, and `NTFS-ATTRIBUTE-LIST-PARTIAL-001`; they do not
replace the full filesystem compatibility and hostile-corpus gates.

## Security and verification consequences

All raw arithmetic remains checked and region bounded. API enrichment runs
outside parsers and cannot issue mutations. Cross-source conflict, active versus
deleted record, hostile image, and Windows-version tests are required before
the hybrid design is accepted. Only the raw image parser is partial today. The
frozen-tree workspace and frontend suites pass; external-corpus, performance,
and native acceptance evidence remain pending.
