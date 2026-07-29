# SDD-010 — UX, UI, and Accessibility

Status: `Partial` demonstration

## Information architecture

Sources, Scan setup, Live scan, Results, Restore, Sessions, Settings, and Help
are the required primary screens. The current React/Vite implementation is a
deterministic demonstration and must show that state persistently.

## Trust presentation

- Synthetic sources/results are labeled as demonstration data.
- The verified read-only seal is shown only after provider/runtime verification.
- “Found” is not synonymous with “recoverable.”
- Recoverability, metadata confidence, and structural validation remain
  separate.
- No implicit session fallback may fabricate results.

## Accessibility

Primary flows require keyboard operation, visible focus, native interactive
semantics, named progress/status announcements, AA contrast, reduced motion,
high-contrast compatibility, and usable 1100×700 and 200% zoom layouts.

## Evidence

Component tests establish semantics; screenshots under `docs/evidence/ui-audit`
show visual state only. Neither mocks nor screenshots prove Tauri integration,
real scanning, restore, scale, or accessibility conformance.

