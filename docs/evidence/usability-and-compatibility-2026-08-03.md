# Usability and Compatibility Corrections Evidence — 2026-08-03

## Evidence boundary

This focused record reconciles the product-code corrections from commit
`da57f7e827749f9b347a7199c6f249f2e858b647` through
`a426b1c` (inclusive) on `codex/actionable-restore`. The inspected product-code
HEAD was `a426b1c`.

This is source inspection plus focused deterministic test evidence. It is not a
new full-gate record, release build, signed artifact, clean-machine result,
hardware compatibility certification or real deleted-file recovery. Historical
screenshots and user-reported behavior remain diagnostic inputs, not acceptance
evidence.

## Corrections reconciled

| Area | Product correction | Evidence available now | Evidence still required |
| --- | --- | --- | --- |
| Scan feedback | `da57f7e` carries real native phase events into the WebView, starts elapsed time with the scan, and renders measured MFT percentage plus a rate-based ETA when the native total is trustworthy. Later phases remain indeterminate. | `SCAN-PROGRESS-001`; focused `AnalysisView` test passed. | Cooperative scan cancellation, deep-phase totals, real long-running scan and packaged timing/ETA acceptance. |
| Responsive workspace | `58146dc` expands scan/report shells; `cbd12b5` removes the candidate table's fixed 1,180-pixel minimum, uses fixed layout/wrapping, and lets the non-input portion of a row dispatch the same single-candidate toggle as its checkbox. | Current CSS and React event-handler inspection; bounded table/component tests passed. | A dedicated CSS-width assertion, a dedicated row-click regression, packaged multi-resolution/200% zoom and assistive-technology acceptance. |
| Individual selection | `bf64b31` stops treating a displayed structured query error as selection authority loss; `9fb4b31` retains `ready` when a refresh fails after a valid page exists; `636a890` applies camelCase renaming to tagged-operation fields so `candidateIds` deserializes exactly. | `RESULT-SELECTION-CONTRACT-030`; controller/component focused suites include checkbox payload and native refresh projection and passed. | Dedicated retained-page-after-refresh-error regression and governed packaged single-item selection/recovery. |
| Restore destination admission | `636a890` queries filesystem, label and serial with `GetVolumeInformationW` against the volume-GUID root derived from the retained directory handle. The desired-access-zero volume handle remains limited to bus/extents queries. | All 15 `um-io-windows` tests matching `destination` passed; source inspection confirms the GUID-root call and retained-authority flow. | Real picker/admission/revalidation and deleted-file restore to a governed different physical NTFS disk. |
| Legacy direct USB source | `a426b1c` permits a logical-sector fallback only for Windows `ERROR_INVALID_FUNCTION` and `ERROR_NOT_SUPPORTED`, using the enumerated logical sector size. The fallback mode is retained and revalidated; other failures still fail closed. `SourceUnavailable` is reported as `SOURCE_IO`, not as proof that the drive was removed. | `WINDOWS-LEGACY-ALIGNMENT-003` passed and checks both allowed codes, rejection of an unrelated error, and rejection of invalid fallback geometry. | A governed rerun on the reported G: device and broader legacy-controller compatibility evidence. |

## Focused commands executed

All commands below were run from the actionable-restore worktree on 2026-08-03.

| Command | Observed result |
| --- | --- |
| `pnpm --dir apps/desktop test -- src/views/AnalysisView.test.tsx src/state/resultsWorkspace.test.ts src/components/results/ResultsWorkspace.test.tsx` | Exit 0; 3 files and 36 tests passed. |
| `cargo test -p um-desktop result_selection_contract_030_accepts_camel_case_candidate_ids -- --nocapture` | Exit 0; the matching test passed in both desktop unit-test targets; 0 failed. |
| `cargo test -p um-io-windows windows_legacy_alignment_003_uses_only_the_vetted_logical_fallback -- --nocapture` | Exit 0; 1 passed, 0 failed. |
| `cargo test -p um-io-windows destination -- --nocapture` | Exit 0; 15 passed, 0 failed. |

These focused commands do not replace `cargo fmt --all -- --check`, workspace
Clippy with warnings denied, the complete Rust workspace suite, the complete
frontend lint/typecheck/test suite, documentation validators, a fresh release
build or remote CI for the final documentation revision.

## Deliberately unclaimed acceptance

No command in this record:

- scanned C:, E:, F:, G: or another real storage source;
- opened an elevated broker session against the reported legacy USB device;
- selected or wrote to a real restore destination;
- recovered a deleted file or verified recovered bytes on physical media;
- built, signed, launched or visually accepted a new release executable;
- proved row-click, full-width layout or refresh-failure interaction in the
  packaged Tauri application.

The corrections are therefore documented as implemented with focused local
evidence, while product/hardware acceptance remains `Implemented-unverified` or
`Partial` according to the governing specification row.
