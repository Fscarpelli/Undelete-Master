# Task 6 report: actionable results workspace and real restore UI

Date: 2026-07-30

## Outcome

The passive appended candidate list was replaced by a bounded,
native-authority results workspace. A completed scan now exposes real search,
facet filtering, sort, cursor pagination, native selection, restore planning,
restore execution, cancellation, terminal evidence, and the fixed opaque
open-destination action.

No production candidate, progress value, selection state, restore plan, job,
manifest, path, or recovered byte is mocked or invented in the WebView.

## Requirement trace

| Requirement | Implementation | Regression evidence |
| --- | --- | --- |
| `RESULT-LATE-RESPONSE-008` | One serialized results queue, latest-intent coalescing, generation/query/cursor guards, three-page LRU | Late response, stale cursor, scan replacement, selection/pagination race, and 101-row rejection tests |
| `DESKTOP-RESULT-CONTROLS-022` | Native facets/totals, explicit search, sortable semantic table, global tri-state selection, selected-only mode, pagination and sticky selection summary | Results controller and component suites, including filtered-empty truth and selection-update pagination lock |
| `DESKTOP-RESTORE-FLOW-024` | Opaque destination, immutable plan review, explicit partial consent, serialized native job operations, `BigInt` progress, native cancellation and terminal manifest | Picker cancellation, policy, selection revision, poll/cancel serialization, bounded polling failure and terminal-status tests |
| `DESKTOP-RESTORE-A11Y-025` | Semantic table/dialog/progress, programmatic names, focus recovery/return, `aria-sort`, `<bdi>`, forced colors, reduced motion and responsive containment | Focused row-removal/table-unmount, dialog, CSP and full frontend suites |
| `DESKTOP-RESTORE-OPEN-DESTINATION-026` | Completed opaque job ID is sent only through `open_restore_destination`; failed open retains completed evidence and remains retryable | Terminal-only open, failed-open retry, and adversarial production-surface tests |

## Review findings closed

Independent review found and regression-locked the following before commit:

1. Cached pagination could move while a native selection update refreshed its
   originating page. Pagination is now disabled at both controller and UI
   boundaries while selection authority is updating.
2. Permanent job-tracking failures could poll forever. Job-not-found and
   incompatible snapshots now fail closed immediately; internal polling gets
   at most three consecutive attempts and resets after a valid snapshot.
   Exhaustion reports an unknown outcome, retains the last native evidence,
   stops polling, and permits closing without claiming cancellation or
   completion.
3. An open-destination failure discarded the only retry path. The completed
   job and manifest now remain visible and the opaque action is retryable.
4. Filtered-empty, initial-load error, terminal status/count, complete-only
   policy, and row-focus copy/behavior were corrected to match actual native
   behavior.
5. The production guard now detects generic and non-generic invokes,
   aliases/namespace/dynamic dispatch, browser/Tauri/process launch surfaces,
   and caller-controlled path/program authority through adversarial fixtures.

Final independent state and UI reviews reported no remaining Critical or
Important finding.

## Verification

Fresh local results after all review fixes:

```text
pnpm --dir apps/desktop lint
PASS

pnpm --dir apps/desktop typecheck
PASS

pnpm --dir apps/desktop test
10 files, 146 tests passed

python .github/scripts/validate_docs.py
PASS: 58/58 FR catalog/matrix entries, 8 NFR, 11 foundation,
12 real-only, 13 connected-volume requirements, 17 ADR topics,
28 formal justifications

python .github/scripts/tests/test_validate_real_only_desktop.py
98 tests passed

python .github/scripts/validate_real_only_desktop.py
PASS: 117 production-boundary files inspected

git diff --check
PASS (line-ending conversion notices only)
```

## Honest residuals

- Packaged Tauri, Explorer opening, live different-physical-disk restore,
  screen-reader, zoom, and Windows high-contrast acceptance remain Task 7.
- Durable job resume after process restart remains unsupported.
- Metadata-backed recovery is extension agnostic. Deep carving is still
  format-plugin based and currently limited to the implemented JPEG path.
- The scan screen still lacks determinate progress, heartbeat, ETA, and
  cancellation. A long scan therefore still needs separate product work; this
  Task 6 does not claim to solve the earlier one-hour-scan complaint.
- No destructive test was run against a real disk.
