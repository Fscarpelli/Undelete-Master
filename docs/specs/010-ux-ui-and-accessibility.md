# SDD-010 — UX, UI, and Accessibility

Status: connected-volume desktop `Implemented-unverified`

## Current information architecture

The production desktop contains:

- **Analysis:** load observed mounted volumes into honest logical groups,
  select one eligible volume, optionally select one NTFS folder, start the real
  scan and browse real candidate pages.
- **Settings:** language, theme and reduced motion, each with immediate effect
  and local persistence.
- **Help:** supported scope, read-only boundary, UAC explanation, result
  interpretation and absent features.

The browser build fails closed with a desktop-runtime-required state and invokes
no storage command. It never substitutes sample disks or candidates.

## Analysis flow

1. Inventory loads without UAC.
2. Each real mounted volume is a labelled radio in a logical group that does
   not claim a physical disk number.
3. Unsupported volumes remain non-selectable with their real warning.
4. Choosing another volume clears the previous folder authority and scan.
5. An eligible NTFS volume enables the native folder picker. Canceling the
   dialog changes no scope.
6. Starting a scan may open UAC for the read-only broker. Inventory and folder
   selection do not claim to require elevation.
7. The pending state is indeterminate; there is no invented percentage or ETA.
8. A completed scan shows real filesystem/status/counts/warnings and the first
   page of at most 100 real candidates.
9. Search, native facets and sortable headers re-query the complete retained
   result set. Previous/next navigation keeps a bounded three-page cache and
   renders no more than 100 rows at once.
10. Native selection persists across pages, filters and sorting for the current
    process. The sticky selection summary exposes exact native counts and
    enables recovery only for an eligible explicit selection.
11. Recovery uses a native destination picker, immutable plan review,
    explicit best-effort consent, native item/byte progress, cooperative
    cancellation, a terminal manifest summary and an opaque
    `Open destination folder` action.

Logical group cards are presentation only, not physical-disk maps or
whole-disk controls. The UI never claims to scan unmounted partitions or all
bytes of a physical disk.

## Trust presentation

- The results collection uses a neutral “Discovered candidates” label and an
  always-visible note that state, confidence and score estimate metadata
  quality; they do not prove intact or recoverable content.
- A live-volume result is not described as a snapshot.
- Recognized NTFS/FAT `partial` status remains visible.
- In folder mode, `unknownCandidates` is separate from matched results; unknown
  ancestry is not represented as outside.
- Candidate state, confidence and score are scanner evidence, not certainty.
- Directories display no recoverability score.
- Parser warnings and recovered paths render only as text. A path that exceeds
  the 512-scalar IPC display bound is visibly elided and receives a bounded
  candidate-derived reference so silent common-prefix collisions do not make
  distinct rows appear identical.
- Recovered content is never previewed, executed or privileged-opened. The
  Windows shell may open only the completed job directory after native
  destination-authority revalidation; the WebView cannot supply a path or
  executable.
- UAC denial, source disappearance, unsupported scope, broker failure and
  incompatible report have stable localized states.

## Accessibility contract

The interface uses native buttons, radio groups, selects, checkbox and table
semantics; visible focus; named status regions; keyboard navigation;
forced-color rules; light/dark themes; and reduced-motion behavior.

The Windows application manifest declares both the `true/pm` fallback and
`PerMonitorV2, PerMonitor`. At 150% scaling, the native WebView client and UI
Automation root have identical physical dimensions, so Windows DPI
virtualization does not conceal controls or warnings.

Navigation moves focus to the new level-one heading. Every volume radio has an
accessible label. The candidate table is inside a keyboard-focusable named
region, includes an accessible caption and directionally isolates recovered
paths. Focus does not depend only on shadow or color.

Focused component and native-adapter tests cover volume-radio labels,
candidate-table naming and the metadata-quality caveat, long-path
disambiguation, unknown/partial truth, browser fail-closed behavior, real
inventory, empty inventory, native folder cancellation, bounded
query/page/selection flow, duplicate submission, sanitized errors, late
unmount responses, restore review/progress/cancellation/tracking loss/open
retry, heading/dialog/table focus and effective preferences.

Native 150% bounds and clipping acceptance is verified. Native 200% zoom,
forced-colors on Windows and assistive-technology acceptance remain pending. A
browser component render or screenshot does not replace actual Tauri
acceptance.

## Explicitly absent interactions

No image picker, physical-disk scan control, scan pause/resume/cancellation,
scan progress percentage or scan ETA is presented as functional. There is no
persistent session history, recovery-job resume after restart, preview, repair,
content execution, production exFAT workflow or multi-format deep carving.
Whole-volume NTFS contiguous-JPEG carving and process-lifetime restore
progress/cancellation are real, separately bounded features.
