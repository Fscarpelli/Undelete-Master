# SDD-010 — UX, UI, and Accessibility

Status: image-only desktop slice `Implemented-unverified`

## Current information architecture

The production application contains only:

- **Analysis:** choose and scan one regular image in Tauri, then render its real
  aggregate report.
- **Settings:** language, theme and reduced motion; every control has immediate
  effect and local persistence.
- **Help:** implemented scope, safety boundary, absent features and correct
  interpretation of candidate counts.

Physical sources, scan modes, percentage/ETA, pause/resume/cancel, individual
candidate rows, restore, preview and sessions are absent until real backends
exist.

## Trust presentation

- Browser-only execution displays a runtime-required state and invokes no data
  command.
- The verified read-only seal appears only on a successfully returned report.
- “Metadata candidate” is not synonymous with “recoverable file.”
- Every volume displays its required scan status. When any recognized NTFS or
  FAT volume is `partial`, the analysis view displays a localized coverage
  caveat and does not present the observed candidate count as exhaustive.
- Parser warnings are untrusted bounded text and render only as React text.
- No implicit fallback or generated state may fabricate a result.

## Accessibility contract

The current flow uses native buttons, selects, checkbox and table semantics;
visible focus; named status/progress states; keyboard navigation; forced-color
styles; light/dark themes; and reduced-motion behavior. Navigation moves
programmatic focus to the newly selected view's level-one heading. In Windows
forced-colors mode, focusable controls use an explicit `Highlight` outline and
do not depend on a box shadow. The horizontally scrollable volume-table
container is a keyboard-focusable named region, and the table has a visually
hidden localized caption with the same accessible name. The layout remains
usable at the Tauri minimum window and reflows below it for browser inspection.

Focused tests cover the fail-closed state, native cancellation,
duplicate/stale responses, real and partial report presentation, localized
errors, effective preferences, navigation heading focus, and forced-colors
focus rules. Relevant IDs are `DESKTOP-PARTIAL-SCAN-STATUS-001`,
`DESKTOP-FAT-PARTIAL-001`,
`DESKTOP-NAVIGATION-FOCUS-001`,
`DESKTOP-FORCED-COLORS-FOCUS-001`, and
`DESKTOP-EFFECTIVE-PREFS-001`, and
`DESKTOP-VOLUME-TABLE-A11Y-001`. Native visual, 200% zoom, and
assistive-technology acceptance remain open and are not replaced by
source/CSS/component inspection or screenshots.
