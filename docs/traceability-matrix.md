# Traceability Matrix

Status: Active  
Revision under evaluation: `codex/foundation-hardening`

Status meanings are defined in [SDD-000](specs/000-product-vision.md). `Verified`
is reserved for Task 8 evidence on the final revision. A dash means there is no
valid implementation/test evidence yet, not that the requirement is optional.

## Current increment requirements

| Requirement | Design | Code module | Unit tests | Integration/E2E | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| SDD-HARD-001 | [SDD-016 §4.1](specs/016-foundation-hardening-and-safe-image-cli.md) | `crates/partition/src/gpt.rs` | `PART-GPT-OVERFLOW-001`, `PART-GPT-RANGE-001` | `partition_tables` | Task 8 pending | Partial |
| SDD-HARD-002 | [SDD-016 §4.1](specs/016-foundation-hardening-and-safe-image-cli.md) | `crates/fs-common`, `crates/fs-ntfs` | `FS-MAP-TRUNC-001` | `NTFS-BITMAP-TRUNC-001` | Task 8 pending | Partial |
| SDD-HARD-003 | [SDD-016 §4.1](specs/016-foundation-hardening-and-safe-image-cli.md) | `crates/fs-ntfs/src/scan.rs` | `NTFS-INIT-TAIL-001` | NTFS fixture extraction | Task 8 pending | Partial |
| SDD-HARD-004 | [SDD-016 §4.1](specs/016-foundation-hardening-and-safe-image-cli.md) | `crates/core`, `crates/io-common`, `crates/partition` | `CORE-SECTOR-VALID-001`, `IO-SECTOR-ZERO-001`, `PART-SECTOR-ZERO-001` | workspace tests | Task 8 pending | Partial |
| SDD-HARD-005 | [ADR-0006](adr/0006-explainable-recoverability-score.md) | `crates/core/src/candidate.rs`, `score.rs` | `CORE-COVERAGE-UNION-001`, `CORE-SCORE-OVERLAP-001` | core tests | Task 8 pending | Partial |
| SDD-CLI-001 | [ADR-0003](adr/0003-regular-image-only-cli.md) | `crates/cli` | CLI validation tests | `CLI-IMAGE-NTFS-001`, `CLI-IMAGE-FAT-001`, `CLI-IMAGE-EXT-001` | Task 8 pending | Partial |
| SDD-CLI-002 | [SDD-016 §4.2](specs/016-foundation-hardening-and-safe-image-cli.md) | `crates/cli/src/report.rs` | serializer/redaction tests | `CLI-JSON-PRIVACY-001` | Task 8 pending | Partial |
| SDD-UI-001 | [SDD-010](specs/010-ux-ui-and-accessibility.md) | `apps/desktop/src/api`, `App.tsx` | `UI-RUNTIME-DEMO-001`, `UI-SEAL-VERIFY-001` | screenshots are visual only | [UI audit](evidence/ui-audit/) | Partial |
| SDD-UI-002 | [SDD-010](specs/010-ux-ui-and-accessibility.md) | `ResultsView.tsx`, `RestoreView.tsx` | `UI-RESULTS-NOSESSION-001`, `UI-RESTORE-NOSESSION-001` | component flow | Task 8 pending | Partial |
| SDD-UI-003 | [SDD-010](specs/010-ux-ui-and-accessibility.md) | results/live/restore views and styles | `UI-RESULTS-KEYBOARD-001`, `UI-PROGRESS-ARIA-001` | 1100×700 audit pending | [UI audit](evidence/ui-audit/) | Partial |
| SDD-QA-001 | [SDD-012](specs/012-test-and-validation-plan.md) | `.github/workflows/quality.yml` | documentation validator | PR Rust/frontend jobs | [Environment inventory](evidence/environment-inventory.md) | Implemented-unverified |

## Master functional requirements with current evidence

| Requirement | Design | Code module | Unit tests | Integration/E2E | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| FR-001–002 | [SDD-001](specs/001-functional-requirements.md) | desktop scan setup/demo | mock contract only | real scan flow absent | UI audit | Partial |
| FR-003 | [ADR-0002](adr/0002-read-only-source-invariant.md) | `um-core::SourceReader`, `um-io-common::FileImageReader` | read/bounds tests | broker runtime test absent | baseline inventory | Partial |
| FR-004 | [SDD-010](specs/010-ux-ui-and-accessibility.md) | desktop settings/demo | component evidence pending | real mode policy absent | UI audit | Partial |
| FR-010–012 | [SDD-005](specs/005-windows-io-and-privilege-model.md) | image identity plus desktop mock | image reader tests | Windows inventory absent | baseline inventory | Partial |
| FR-013 | [SDD-005](specs/005-windows-io-and-privilege-model.md) | — | — | AC-015 | current increment exclusion | Not started |
| FR-020–021 | [SDD-001](specs/001-functional-requirements.md) | desktop demonstration | mock contract | real provider absent | UI audit | Partial |
| FR-022 | [SDD-005](specs/005-windows-io-and-privilege-model.md) | — | — | AC-003 | current increment exclusion | Not started |
| FR-023–024 | [SDD-001](specs/001-functional-requirements.md) | desktop demonstration | mock same-letter rule only | physical-disk mapping absent | UI audit | Partial |
| FR-030 | [SDD-006](specs/006-partition-and-filesystem-engines.md) | NTFS/FAT scanners | parser tests | synthetic fixture scans | final evidence pending | Partial |
| FR-031–032 | [SDD-007](specs/007-carving-validation-repair.md) | — | — | deep/image-creation E2E | current increment exclusion | Not started |
| FR-033 | [ADR-0003](adr/0003-regular-image-only-cli.md) | `um-io-common`, `um-cli` | reader/CLI tests | AC-027 partial | final evidence pending | Partial |
| FR-034 | [SDD-004](specs/004-architecture.md) | — | — | region-selection E2E | current increment exclusion | Not started |
| FR-040–043 | [SDD-010](specs/010-ux-ui-and-accessibility.md) | desktop demonstration | component evidence pending | persistent pipeline absent | UI audit | Partial |
| FR-050, FR-052, FR-054 | [SDD-006](specs/006-partition-and-filesystem-engines.md) | NTFS/FAT scanners | parser tests | synthetic fixture scans | final evidence pending | Partial |
| FR-051, FR-053, FR-055 | [SDD-007](specs/007-carving-validation-repair.md) | — | — | carving/selection/merge E2E | current increment exclusion | Not started |
| FR-060–067 | [SDD-010](specs/010-ux-ui-and-accessibility.md) | desktop results demo | mock/component tests | million-row/backend E2E absent | UI audit | Partial |
| FR-070 | [SDD-010](specs/010-ux-ui-and-accessibility.md) | desktop details demo | mock/component tests | real evidence provider absent | UI audit | Partial |
| FR-071–074 | [SDD-007](specs/007-carving-validation-repair.md) | — | — | sandbox/validation/repair E2E | current increment exclusion | Not started |
| FR-080, FR-083, FR-085 | [SDD-009](specs/009-restore-semantics.md) | desktop restore demo only | component evidence pending | restore engine absent | UI audit | Partial |
| FR-081–082, FR-084, FR-086–088 | [SDD-009](specs/009-restore-semantics.md) | — | — | restore acceptance tests | current increment exclusion | Not started |
| FR-090, FR-092 | [SDD-008](specs/008-session-data-model.md) | — | — | session persistence/import E2E | current increment exclusion | Not started |
| FR-091, FR-093 | [SDD-008](specs/008-session-data-model.md) | desktop mock only | component evidence pending | SQLite/report backend absent | UI audit | Partial |
| FR-100–101 | [SDD-011](specs/011-security-and-privacy.md) | offline libraries; CLI redaction increment | focused tests pending | network-disabled product E2E absent | final evidence pending | Partial |
| FR-102 | [SDD-008](specs/008-session-data-model.md) | desktop mock only | component evidence pending | real cleanup absent | UI audit | Partial |
| FR-103 | [SDD-014](specs/014-localization.md) | pt-BR/en-US catalogs | catalog parity tests | full shell/installer localization absent | final evidence pending | Partial |

## Acceptance and release gates

AC-001 through AC-030 remain unverified as product acceptance criteria. Existing
component tests may support a row above but do not satisfy the complete
acceptance scenario. Gates S2–S4 remain open; see
[known limitations](specs/015-known-limitations.md).

