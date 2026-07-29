# SDD-006 — Partition and Filesystem Engines

Status: Normative behavior with component-level partial implementation

| Engine | Required scope | Current honest status |
| --- | --- | --- |
| MBR | Primary entries, EBR loop protection, bounds, overlap warnings. | Implemented-unverified. |
| GPT | Primary/backup headers, CRCs, names, bounds, overlap and corruption handling. | Implemented-unverified; hardening is in progress. |
| NTFS | Boot/MFT, deleted records, paths, resident/nonresident extents, bitmap conflicts, bounded corruption handling. | Partial; compression, full attribute lists, ADS workflow, MFT mirror and enrichment are incomplete. |
| FAT12/16/32 | BPB, FAT chains, deleted entries/directories, LFN, bounded inference and loop detection. | Partial; deterministic coverage exists, production matrix is incomplete. |
| exFAT | Boot regions, checksums, bitmap, upcase, entry sets, NoFatChain, fragmented files and directories. | Not started for production; incomplete files are excluded from the workspace. |
| ReFS/unknown | Detect and offer explicitly limited RAW/carving behavior only. | Not started. |

## Common parser requirements

- Every range is bounded to the supplied source region.
- Arithmetic is checked before allocation or slicing.
- Corruption produces typed errors or provenance warnings, never invented data.
- Inference reduces confidence and score.
- Tests use deterministic fixtures; byte recovery is asserted with SHA-256.
- A passing synthetic case does not establish production filesystem support.

