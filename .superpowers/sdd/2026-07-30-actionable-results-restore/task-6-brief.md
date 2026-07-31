## Task 6: Actionable results workspace and real restore UI

**Requirements:** `RESULT-LATE-RESPONSE-008`,
`DESKTOP-RESULT-CONTROLS-022`, `DESKTOP-RESTORE-FLOW-024`,
`DESKTOP-RESTORE-A11Y-025`, and
`DESKTOP-RESTORE-OPEN-DESTINATION-026`.

### Product outcome

Replace the passive, appended candidate list with a bounded native-authority
workspace in which the user can search, filter, sort, page, select, review, and
recover real scan candidates. Recovery must call the six native Task 5
commands and display only native job snapshots. There is no production mock,
sample candidate, browser fallback, simulated progress, placeholder recovery
button, or locally invented selection authority.

### Non-negotiable behavior

- The initial sort is `recoverabilityScore` descending. Native null-last
  ordering is preserved.
- Query, page, and selection commands share one serialized frontend queue
  because the native scan retains one `active_query`. Only one result command
  may be in flight. New query/sort/filter intent while it is running is
  coalesced to the latest intent and dispatched after the current invocation
  settles. Late responses never replace newer intent.
- Query, sort, and filters reset to cursor `null`. Previous/next reuse only
  native cursor identity. At most three pages are cached and only one page of
  at most 100 rows reaches rendering.
- Keep the authoritative selection summary and extension facets at workspace
  state level rather than duplicating them into three cached pages. Facet
  search may render at most 100 native facet options at once while preserving
  selected facet chips; the native query accepts at most 128 extensions.
- The native selected-ID set and its decimal revision are authoritative.
  Checkboxes are projections of native responses; direct updates contain at
  most 100 IDs. Select-all-matching and clear-matching each send one tagged
  native operation and never enumerate the complete result set in the WebView.
- Add the exact storage error codes `RESULT_QUERY_INVALID`,
  `RESULT_CURSOR_STALE`, `RESULT_SELECTION_STALE`, and
  `RESULT_SELECTION_INVALID` to the closed TypeScript error contract.
- Selection survives query, sort, filter, and page changes. Stale cursor or
  selection errors are recoverable through a fresh native query; no optimistic
  mutation is shown.
- Candidate rows are semantic table rows. Sortable headers expose `aria-sort`;
  paths stay inside `<bdi dir="auto">`; controls have programmatic names;
  `aria-rowcount` uses the backend total; the header checkbox represents all
  filtered matches, not only the current page.
- After a control changes the row set, focus stays on the initiating control
  when it remains mounted. If a focused row disappears, focus moves to the
  results summary/status, never to an arbitrary row.
- Facets and totals come only from the native response. The complete retained
  set may contain 110,000 candidates, but the DOM contains no more than 100
  candidate rows.
- An ineligible candidate remains inspectable/selectable and is reported in
  the global selection summary, but plan creation is blocked until it is
  cleared.
- Best-effort recovery requires the native `zeroFillAndMap` policy and an
  explicit user consent recorded in UI state. `completeOnly` is the default.
- Destination cancellation returns to results without an error or retained
  plan. Plan review uses only opaque IDs and the exact selection revision.
  Any changed revision invalidates review and requires replanning.
- Progress, item counts, byte counts, partial count, failures, cancellation,
  reconciliation state, and manifest SHA-256 are copied from native snapshots.
  Percentage uses `BigInt` basis points. Polling is the sole permitted timer;
  it stops at terminal state and unmount. Cancel invokes the native idempotent
  command once. Poll, cancel, and open-destination commands are serialized in
  a restore-operation queue so a late poll cannot overwrite a newer snapshot.
- Completion offers only the fixed native `open_restore_destination` action.
  The WebView never receives source/destination paths, extents, offsets, disk
  numbers, handles, access masks, control codes, executables, or recovered
  bytes, and never previews or executes recovered content.
- Keep the established desktop design tokens, components, Lucide icon library,
  density, and dark visual language. Add no dependency unless an existing
  primitive cannot satisfy an evidenced requirement.
- Use `<progress>` rather than dynamic inline widths. Preserve CSP, add
  `:focus-visible`, forced-colors, reduced-motion, responsive filter/table
  containment, sticky headers, and a sticky selection bar.
- Update Help to describe the real recovery flow and its honest boundaries.
  Remove the obsolete claim that the app cannot restore files.
- The production-surface test must recurse through nested result components,
  exactly allow the real command/timer surfaces, and continue rejecting mock
  providers, fake/simulated progress, paths, low-level authority, executable
  launching, and recovered bytes.

### Files and conflict-free ownership

State/native-contract owner:

- Create `apps/desktop/src/state/resultsWorkspace.ts`
- Create `apps/desktop/src/state/resultsWorkspace.test.ts`
- Create `apps/desktop/src/state/restoreWorkflow.ts`
- Create `apps/desktop/src/state/restoreWorkflow.test.ts`
- Modify `apps/desktop/src/api/storageDesktop.ts`
- Modify `apps/desktop/src/api/storageDesktop.test.ts`

Results-view owner:

- Create `apps/desktop/src/components/results/ResultsWorkspace.tsx`
- Create `apps/desktop/src/components/results/ResultsFilters.tsx`
- Create `apps/desktop/src/components/results/CandidateResultsTable.tsx`
- Create `apps/desktop/src/components/results/SelectionBar.tsx`
- Create `apps/desktop/src/components/results/RestoreWorkflowDialog.tsx`

Restore/integration owner:

- Modify `apps/desktop/src/state/storageScan.ts`
- Modify `apps/desktop/src/views/AnalysisView.tsx`
- Modify `apps/desktop/src/views/AnalysisView.test.tsx`
- Modify `apps/desktop/src/App.tsx`
- Modify `apps/desktop/src/App.test.tsx`
- Modify `apps/desktop/src/i18n/messages.ts`
- Modify `apps/desktop/src/views/HelpView.tsx`
- Modify `apps/desktop/src/styles/global.css`
- Modify `apps/desktop/src/csp-styles.test.js`
- Modify `apps/desktop/src/production-surface.test.ts`
- Modify `docs/traceability-matrix.md`

The state owner exports `ResultsWorkspaceController` and
`RestoreWorkflowController`. The results-view owner accepts those controllers
plus `t`/locale as props and owns no side-effecting hook. The integration owner
instantiates the hooks, replaces the legacy appended-page view, and consumes
the new components without editing the state or component owners' files. It
owns one recovery-button ref and passes it through the selection bar/dialog so
closing or completing the modal returns focus predictably. No package or
lockfile change is expected.

### Explicit residuals

- `ActionableCandidateRow` intentionally has no content hash or validator
  payload. Task 6 must not recreate the old validation column with invented
  evidence or claim complete master-spec validation presentation.
- SDD-020 intentionally bounds the DOM to one 100-row page instead of adding a
  virtualization dependency. The broader FR-060–067 result-suite status
  remains `Partial`.
- Alternate destination layouts, optional destination metadata/CSV, durable
  restart resume, external corpora, and packaged real-device evidence remain
  outside Task 6 and must not be implied by the UI.

### TDD and verification

Write failing tests before production code. Required focused gates:

```powershell
pnpm --dir apps/desktop test -- resultsWorkspace.test.ts
pnpm --dir apps/desktop test -- restoreWorkflow.test.ts
pnpm --dir apps/desktop test -- AnalysisView.test.tsx
pnpm --dir apps/desktop test -- App.test.tsx production-surface.test.ts
pnpm --dir apps/desktop lint
pnpm --dir apps/desktop typecheck
pnpm --dir apps/desktop test
python .github/scripts/tests/test_validate_real_only_desktop.py
python .github/scripts/validate_real_only_desktop.py
```

The final Task 6 commit is permitted only after independent review reports no
Critical or Important finding and the worktree contains no unrelated changes.
