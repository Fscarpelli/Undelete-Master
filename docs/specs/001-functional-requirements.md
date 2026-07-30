# SDD-001 — Functional Requirements

Status: Normative catalog

The master specification remains authoritative. This document makes requirement
status explicit without weakening behavior.

## Record fields

Unless a row says otherwise, every requirement below has these inherited §23.2
fields:

- **Rationale:** preserve recoverable data and present technically honest,
  usable results.
- **Priority:** Must.
- **Source:** the identically numbered requirement in
  `UNDELETE_MASTER_CODEX_MASTER_SPEC.md`.
- **Preconditions:** the user is authorized and the referenced source/session
  exists with a valid identity.
- **Error behavior:** fail closed, remain bounded, do not mutate the source, and
  show an actionable localized error.
- **Security implications:** source data is read-only; recovered bytes and
  imported metadata are untrusted; paths and secrets are redacted.
- **Observability:** record phase, session/source/item IDs, result, and bounded
  error codes without file content.
- **Test IDs or formal justification:** the canonical traceability row supplies
  executable test IDs or a `JUST-*` record defined in
  `docs/test-justifications.md`.
- **Implementation links:** the traceability matrix is the canonical code/test
  link and must be updated with behavior changes.

Each row supplies the remaining **ID**, **Title**, **Behavior**, **Acceptance
criteria**, and **Status** fields.

## First use and safety

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-001 | Consent and authorization | Require explicit authorization before the first scan; start is impossible without consent. | Not started |
| FR-002 | Preservation warning | Explain that continued use may overwrite data and recommend another disk/image-first workflow. | Not started |
| FR-003 | No source writes | Expose a verified read-only seal only after proof; architecture and runtime tests show no write operation. | Partial |
| FR-004 | Guided and advanced modes | Guided mode limits risk; advanced mode exposes bounded controls without weakening safeguards. | Not started |

## Source inventory

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-010 | Enumerate sources | Enumerate real disks, volumes, and added images; synthetic cards are not acceptance evidence. Mounted local volume inventory is implemented; added-image inventory is absent. | Partial |
| FR-011 | Device cards | Show identity, capacity, media/bus, volumes, filesystem, encryption, and warnings from real inventory. Disk grouping, capacity, volume, filesystem and warnings are implemented; richer media/encryption metadata is incomplete. | Partial |
| FR-012 | Stable identity | Identity survives drive-letter changes and prevents resume on a substituted source. Opaque multi-property mounted-volume identity and broker revalidation exist; resume is absent. | Partial |
| FR-013 | Refresh and disconnect | Refresh hot-plug state, pause on removal, and resume only after identity match. Explicit refresh and fail-closed source loss exist; subscription, pause and resume are absent. | Partial |

## Scan preparation

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-020 | Select disk/volume | Require a source and permit bounded recognized partition selection. | Not started |
| FR-021 | Working folder | Select a working location for DB, checkpoints, thumbnails, temporary files, and reports. | Not started |
| FR-022 | Physical destination mapping | Resolve working/restore paths to physical disks, not drive letters alone. | Not started |
| FR-023 | Overwrite prevention | Block same-physical-disk work by default and apply the post-scan advanced override policy. | Not started |
| FR-024 | System disk | Detect the active Windows disk, explain inconsistency risk, and require another destination. | Not started |

## Scan modes

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-030 | Quick scan | Scan deleted filesystem metadata, reconstruct extents, and assess allocation evidence. NTFS now enumerates beyond the former 64 MiB MFT prefix in bounded batches, and quantitative MFT counters reach the report and desktop; machine-readable partial reasons and the broader compatibility matrix remain incomplete. | Partial |
| FR-031 | Deep scan | Add bounded raw metadata, unallocated carving, slack, and prioritized validation. An explicit whole-volume NTFS mode now submits only hardened `$Bitmap`-proven free regions to an incremental contiguous-JPEG validator with structured budgets/coverage; slack, fragments, broader formats, progress/cancellation and final validation remain absent. | Partial |
| FR-032 | Image first | Create a resumable, hashed, error-mapped image on another disk without writing the source. | Not started |
| FR-033 | Open existing image | Open regular `.img/.dd/.raw` without UAC and pass it through the same bounded parsers. | Partial |
| FR-034 | Analysis regions | Advanced mode selects valid metadata, unallocated, slack, whole-partition, or explicit ranges. | Not started |

## Progress and control

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-040 | Live dashboard | Show phase, bytes, throughput, candidate quality, errors, elapsed time, and honest ETA. | Not started |
| FR-041 | Pause/resume/cancel | Cooperatively pause, checkpoint, resume without duplication, and cancel safely. | Not started |
| FR-042 | Results during scan | Query already persisted results without blocking the pipeline. | Not started |
| FR-043 | Resource profiles | Provide economy, balanced, maximum, and gentle bounded profiles. | Not started |

## Discovery

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-050 | Any extension via metadata | Recover metadata-backed files regardless of extension when extents are valid. | Partial |
| FR-051 | Known-format carving | Carve only recognized/configured formats with bounded validators and provenance. The allocation-aware whole-NTFS JPEG slice carries structural checks, physical range, SHA-256 and validator evidence through the product DTO; only contiguous JPEG is implemented and final/external-corpus evidence is incomplete. | Partial |
| FR-052 | Deleted folders | Reconstruct deleted folders without implying all descendants are selected. | Partial |
| FR-053 | Descendant selection | Parent checkbox state is explicit and never restores historical descendants silently. | Not started |
| FR-054 | Orphans | Group candidates with uncertain parents under stable orphan groupings. | Partial |
| FR-055 | Evidence merge | Merge metadata/carved evidence only with range/content agreement and preserve provenance. Exact contiguous range corroboration is implemented for one unambiguous NTFS metadata owner; ambiguous owners preserve a separate carving and generalized merge remains absent. | Partial |

## Results workspace

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-060 | Scale | Virtualize and query backend pages; never send millions of rows to the UI. | Not started |
| FR-061 | Columns | Provide configurable evidence, path, size, dates, method, score, validation, and conflict columns. | Not started |
| FR-062 | Extension filter | Build multi-select extension facets dynamically with counts. | Not started |
| FR-063 | Other filters | Combine category, quality, method, size/date/path, validation, partial, preview, and selection filters. | Not started |
| FR-064 | Search | Search name, path, extension, type, record ID, and available hashes. | Not started |
| FR-065 | Persistent selection | Preserve selection across pages, sorting, filters, and session resume. | Not started |
| FR-066 | Selection bar | Show exact count, estimated/readable size, space, partial count, and conflicts. | Not started |
| FR-067 | Tree and list | Offer coherent tree/list navigation without fabricating parent relationships. | Not started |

## Details, validation, and preview

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-070 | Details panel | Explain discovery, extents, allocation, score factors, metadata confidence, and warnings. | Not started |
| FR-071 | Safe preview | Generate bounded preview only in a restricted, networkless, unprivileged worker. | Not started |
| FR-072 | No execution | Never execute, macro-enable, import, or privileged-open recovered content. | Not started |
| FR-073 | Validation | Support explicit single/batch structural validation with bounded reports. | Not started |
| FR-074 | Repair derivatives | Preserve original extraction and create separately hashed, provenance-recorded deterministic derivatives. | Not started |

## Restore

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-080 | Destinations | Recommend another physical disk and offer preserve-tree, flatten, or by-type layouts. | Not started |
| FR-081 | Minimal ancestors | Create only ancestors required by explicitly selected items. | Not started |
| FR-082 | Folder alone | Selecting only a folder restores an empty folder and no historical content. | Not started |
| FR-083 | Name collisions | Default to rename; never silently replace an active destination file. | Not started |
| FR-084 | Transactional operation | Use `.umrecovering`, stream/hash, flush/verify, atomic rename, and journal. | Not started |
| FR-085 | Partial files | Require consent and record zero-fill/truncate/segment policy and missing ranges. | Not started |
| FR-086 | Destination metadata | Preserve safe supported metadata; ACL/EFS/ADS are opt-in. | Not started |
| FR-087 | Recovery manifest | Emit versioned JSON and optional CSV with source, evidence, ranges, hashes, errors, and provenance. | Not started |
| FR-088 | Restore resume | Resume large restores after size/hash verification without duplicating completed files. | Not started |

## Sessions and reports

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-090 | Persistent session | Store versioned SQLite sessions only on a working disk different from the source. | Not started |
| FR-091 | Open session | List real sessions with source identity, status, checkpoint, and availability. | Not started |
| FR-092 | Export/import | Version `.umscan`, exclude recovered content by default, and reject malformed/traversal input. | Not started |
| FR-093 | Final report | Export source, mode, duration, bytes, errors, candidates, quality, restore results, hashes, and limitations. | Partial |

## Privacy and preferences

| ID | Title | Behavior and acceptance criteria | Status |
| --- | --- | --- | --- |
| FR-100 | Local operation | Scan, supported preview, and restore function with network disabled and no required account. | Partial |
| FR-101 | Redacted logs | Normal logs contain no content/secrets and can mask full paths/names. | Partial |
| FR-102 | Session cleanup | Explicitly delete session/thumbnails without claiming guaranteed SSD secure erase. | Not started |
| FR-103 | Languages | Complete typed pt-BR/en-US catalogs with fallback and no principal hard-coded UI strings. | Not started |
