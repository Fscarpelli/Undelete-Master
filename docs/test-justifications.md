# Formal Test Justifications

## JUST-SDD-REAL-HISTORICAL

- **Requirements:** SDD-REAL-001, SDD-REAL-002, SDD-REAL-003,
  SDD-REAL-004, SDD-REAL-005, SDD-REAL-006, SDD-REAL-007,
  SDD-REAL-008, SDD-REAL-009, SDD-REAL-010, SDD-REAL-011,
  SDD-REAL-012.
- **Formal rationale:** SDD-017 records a superseded image-desktop increment.
  Its removed adapter tests cannot be treated as executable evidence for the
  current mounted-volume desktop; retained evidence is historical only.
- **Exit criterion:** SDD-018 current requirements and same-revision gates
  replace every historical desktop test claim while ADR-0003 retains image-CLI
  coverage.

Status: Active

These records satisfy master specification §23.4 only while the named
requirement lacks a complete executable acceptance scenario. A justification
does not count as implementation or verification. Its exit criterion must be
replaced by linked executable evidence before the requirement can become
`Verified`.

## JUST-SDD-UI-001-UNVERSIONED

- **Requirements:** SDD-UI-001.
- **Formal rationale:** this historical SDD-016 demonstration requirement was
  superseded by the stricter real-only SDD-017 contract; implementing it would
  now violate ADR-0021.
- **Exit criterion:** retire the historical SDD-016 UI row after the normative
  catalog supports superseded requirements; use SDD-REAL-001/002 meanwhile.

## JUST-SDD-UI-002-UNVERSIONED

- **Requirements:** SDD-UI-002.
- **Formal rationale:** the historical provider design was removed. The
  versioned real-only frontend omits sessions and restore entirely, as governed
  by SDD-REAL-006.
- **Exit criterion:** retire the historical SDD-016 UI row after the normative
  catalog supports superseded requirements.

## JUST-SDD-UI-003-UNVERSIONED

- **Requirements:** SDD-UI-003.
- **Formal rationale:** historical demonstration screenshots were deleted
  because they no longer represent the product. SDD-017 owns the current
  pending-state and accessibility contract.
- **Exit criterion:** retire the historical SDD-016 UI row after the normative
  catalog supports superseded requirements; retain only real-only evidence.

## JUST-FR-001-002-UNVERSIONED-UI

- **Requirements:** FR-001, FR-002.
- **Formal rationale:** the versioned desktop UI now exposes real mounted-volume
  selection and scanning, but it does not yet enforce explicit consent or show
  the required preservation warning before a scan starts.
- **Exit criterion:** add versioned UI tests that prove scanning cannot start
  without consent and that the preservation warning is displayed.

## JUST-FR-003-BROKER-PENDING

- **Requirements:** FR-003.
- **Formal rationale:** the read-only broker and mounted-volume boundary are
  implemented, but final same-revision access-mask/import audits, built-binary
  evidence and supported-Windows runtime proof remain pending.
- **Exit criterion:** pass the full broker/static gates, inspect both native
  binaries and retain source-write prohibition evidence.

## JUST-FR-004-UNVERSIONED-UI

- **Requirements:** FR-004.
- **Formal rationale:** guided and advanced product modes have no versioned
  implementation.
- **Exit criterion:** add mode-policy unit and end-to-end tests.

## JUST-FR-010-013-WINDOWS-INVENTORY

- **Requirements:** FR-010, FR-011, FR-012, FR-013.
- **Formal rationale:** mounted local inventory, disk display grouping and
  opaque volume identity are implemented. The master requirement also asks for
  added-image inventory, richer encryption/media metadata, hotplug pause and
  identity-matched resume, which this increment does not implement.
- **Exit criterion:** add the missing source classes/metadata and pass
  drive-letter substitution, hotplug, disconnect, pause and resume acceptance.

## JUST-FR-020-024-SCAN-PREPARATION

- **Requirements:** FR-020, FR-021, FR-022, FR-023, FR-024.
- **Formal rationale:** the versioned product has no working-folder selection or
  physical-disk mapping boundary.
- **Exit criterion:** pass source selection, physical mapping, same-disk block,
  and system-disk end-to-end tests.

## JUST-FR-030-COMPONENT-SCANNERS

- **Requirements:** FR-030.
- **Formal rationale:** deterministic NTFS/FAT component extraction and focused
  completeness-state propagation now cover malformed MFT size/signature and
  candidate attributes, conservative partial state for unresolved
  `$ATTRIBUTE_LIST` records, bounded NTFS enumeration, all declared FAT-copy
  checks, pre-allocation FAT-table rejection, and selected FAT directory
  failure/depth/chain bounds. The real mounted-volume metadata path reaches the
  desktop, but broader filesystem compatibility, external-corpus evidence, and
  the final acceptance artifact still do not exist.
- **Exit criterion:** retain a final-revision quick-scan report with fixture
  SHA-256 assertions, complete/partial/error truth across both filesystems, and
  product-level error handling.

## JUST-FR-031-032-DEEP-IMAGE

- **Requirements:** FR-031, FR-032.
- **Formal rationale:** a bounded allocation-gated whole-NTFS contiguous-JPEG
  slice now reaches the desktop with real coverage/provenance. Progress and
  cooperative cancellation, slack/fragment analysis, broader formats, image
  creation, long-running performance and external-corpus acceptance remain
  absent.
- **Exit criterion:** add real progress/cancellation plus slack/fragment and
  resumable image-creation acceptance scenarios on deterministic images or
  disposable governed media.

## JUST-FR-034-REGION-SELECTION

- **Requirements:** FR-034.
- **Formal rationale:** no product control exposes safe analysis-region
  selection.
- **Exit criterion:** pass region validation, containment, and mode-policy tests.

## JUST-FR-040-043-SCAN-PIPELINE

- **Requirements:** FR-040, FR-041, FR-042, FR-043.
- **Formal rationale:** the versioned live UI invokes real metadata/deep scan
  commands and pages real results, but a persistent scan pipeline,
  checkpoints, real progress, pause/resume/cancel, and selectable resource
  profiles are absent.
- **Exit criterion:** pass progress, pause/resume/cancel, partial-query, and
  resource-profile end-to-end tests.

## JUST-FR-050-052-054-COMPONENT-SCANNERS

- **Requirements:** FR-050, FR-052, FR-054.
- **Formal rationale:** filesystem scanners emit metadata candidates and paths,
  and NTFS malformed/unresolved attribute plus FAT directory incompleteness now
  propagates structurally with bounded FAT table/chain allocation, but
  product-level extension-agnostic, directory, orphan, external-corpus, and
  native acceptance evidence remains incomplete.
- **Exit criterion:** add named cross-filesystem fixture tests and retained
  product evidence for each requirement.

## JUST-FR-051-053-055-CARVING-SELECTION

- **Requirements:** FR-051, FR-053, FR-055.
- **Formal rationale:** the first JPEG slice has allocation/scope admission,
  incremental-budget, provenance, product-DTO, exact-range corroboration, and
  ambiguous-owner regressions. Descendant-selection semantics, generalized
  metadata/content merge, broader formats, false-positive corpora and final
  product acceptance remain incomplete.
- **Exit criterion:** pass generalized selection/overlap/content-agreement,
  false-merge, multi-format and external-corpus product tests.

## JUST-FR-060-067-RESULTS

- **Requirements:** FR-060, FR-061, FR-062, FR-063, FR-064, FR-065, FR-066,
  FR-067.
- **Formal rationale:** there is no versioned results workspace or persistent
  query backend.
- **Exit criterion:** pass scale, filter, search, selection persistence,
  keyboard, and tree/list end-to-end scenarios.

## JUST-FR-070-074-PREVIEW

- **Requirements:** FR-070, FR-071, FR-072, FR-073, FR-074.
- **Formal rationale:** product details, sandboxed preview, validation, and
  repair derivative flows are absent.
- **Exit criterion:** pass provenance/details and restricted-worker security
  acceptance tests.

## JUST-FR-080-088-RESTORE

- **Requirements:** FR-080, FR-081, FR-082, FR-083, FR-084, FR-085, FR-086,
  FR-087, FR-088.
- **Formal rationale:** the restore engine, containment boundary, journal, and
  manifest are absent.
- **Exit criterion:** pass path sanitation, physical destination, transactional
  restore, partial-file, metadata, manifest, and resume tests.

## JUST-FR-090-092-SESSIONS

- **Requirements:** FR-090, FR-091, FR-092.
- **Formal rationale:** SQLite persistence and session import/export are absent.
- **Exit criterion:** pass schema migration, source identity, malformed import,
  traversal, resume, and cleanup tests.

## JUST-FR-093-FINAL-REPORT

- **Requirements:** FR-093.
- **Formal rationale:** the CLI emits a bounded scan report, not the complete
  scan/restore final report required by the product.
- **Exit criterion:** pass versioned report schema, redaction, restore result,
  hash, and limitation tests.

## JUST-FR-100-PRODUCT-OFFLINE

- **Requirements:** FR-100.
- **Formal rationale:** image scanning is local, but the complete preview and
  restore workflows required by the functional requirement do not exist.
- **Exit criterion:** pass the complete product flow with network disabled.

## JUST-FR-102-CLEANUP

- **Requirements:** FR-102.
- **Formal rationale:** persistent sessions and thumbnail stores do not exist,
  so cleanup cannot yet be exercised.
- **Exit criterion:** pass explicit deletion, locked-file, partial failure, and
  SSD wording tests.

## JUST-FR-103-I18N

- **Requirements:** FR-103.
- **Formal rationale:** no versioned desktop catalogs prove complete pt-BR and
  en-US coverage.
- **Exit criterion:** pass typed catalog parity, fallback, hard-coded-string,
  and installer localization tests.

## JUST-NFR-001-BROKER-PENDING

- **Requirements:** NFR-001.
- **Formal rationale:** source traits, image readers and the mounted-volume
  broker are read-only by design, but final same-revision handle-rights,
  native-import and built-binary evidence remains pending.
- **Exit criterion:** pass source-write prohibition and handle-rights audits
  and retain the exact main/broker artifact hashes.

## JUST-NFR-003-EXTERNAL-EVIDENCE

- **Requirements:** NFR-003.
- **Formal rationale:** deterministic fixture and frozen-tree workspace suites
  pass, while external-corpus evidence and separately retained SHA-256 truth
  artifacts remain incomplete.
- **Exit criterion:** retain external-corpus command logs and SHA-256 truth
  artifacts.

## JUST-NFR-005-UNVERSIONED-UI

- **Requirements:** NFR-005.
- **Formal rationale:** focused source/component coverage now exercises
  navigation heading focus, forced-colors focus rules, and effective reduced
  motion, plus a named keyboard-focusable scroll region and accessible caption
  for the volume table. Those tests do not prove complete WCAG behavior, 200%
  scaling, native focus rendering, or assistive-technology interoperability,
  and screenshots alone cannot close that gap.
- **Exit criterion:** retain automated accessibility and 200% scaling checks,
  plus keyboard and assistive-technology acceptance in the real Tauri shell;
  the frozen-code frontend suite already passes.

## JUST-NFR-006-PERFORMANCE

- **Requirements:** NFR-006.
- **Formal rationale:** the persistent pipeline and million-row backend/UI do
  not exist.
- **Exit criterion:** pass measured memory, cancellation, latency, and scale
  budgets.

## JUST-NFR-008-SUPPORT-MATRIX

- **Requirements:** NFR-008.
- **Formal rationale:** current component fixtures cover canonical GPT copy
  selection/conflict, minimum/aligned logical MFT size, non-`FILE` record
  incompleteness, malformed candidate attributes, conservative
  `$ATTRIBUTE_LIST` partial state, and selected FAT
  copy/directory/table/depth/chain bounds. They are not a complete
  Windows/filesystem compatibility matrix.
- **Exit criterion:** publish retained supported-version and unsupported-state
  evidence.
