# SDD-012 — Test and Validation Plan

Status: Normative; local deterministic gates implemented, external acceptance partial

## Pull-request gates

- Rust format, Clippy with warnings denied, workspace tests and release build.
- Frontend lockfile install with lifecycle scripts disabled, lint, strict
  TypeScript, Vitest and Vite build.
- Unelevated Tauri `--no-bundle` executable build on the Windows runner.
- Documentation IDs, statuses, implementation paths, tests and local links.
- Static destructive-storage and real-only desktop boundary validators.
- No real disk, raw device, VHD attach, format, trim, lock or dismount command.

The canonical frontend lockfile is `apps/desktop/pnpm-lock.yaml`; CI uses
`pnpm install --frozen-lockfile --ignore-scripts`. The static storage guard
reviews workflows and enumerated first-party executable surfaces, including
local helpers and package scripts. The real-only guard rejects production mock
providers, simulation timers/randomness, unapproved invokes/capabilities,
privileged plugins, remote CSP sources and elevation.

## Current evidence levels

| Level | Implemented evidence | Remaining evidence |
| --- | --- | --- |
| Unit/property | Focused parser bounds, canonical/reciprocal GPT copy selection, NTFS logical-MFT/signature/attribute completeness, FAT table-allocation and directory traversal/chain/depth completeness, DTO/schema, error privacy, preferences, focus and CSP regressions; the frozen-tree full workspace/frontend suites pass. | Broader fuzz/property corpora. |
| Integration | Focused deterministic NTFS/FAT/partition scans and Tauri-to-CLI parity paths; the frozen-tree workspace/frontend gates pass. | External forensic corpora and packaged/native integration. |
| Frontend | Focused runtime, concurrency, schema/status, partial-caveat, privacy, preferences, navigation-focus, scroll-table accessibility, forced-colors and development-CSP tests; lint, typecheck, 28 tests, and production build pass on the frozen code tree. | Native assistive-technology, 200% zoom and visual acceptance. |
| Native build | Source/config tests cover CSP, capability and `asInvoker` declarations. | Final same-revision native executable build, extracted-manifest inspection, packaged smoke, signing, clean-machine, update and uninstall. |
| Security | CI static guards and hostile path regression tests. | Broker, sandbox and restore boundary tests. |
| Performance | Parser work limits only. | Benchmarks, cancellation and large real corpora. |

Only deterministic synthetic images or temporary regular files may be used in
ordinary tests. A passing local component suite is not proof of complete
real-world recovery support.

The focused regression set for the current behavior includes
`NTFS-BITMAP-BOUND-004`, `CLI-PROBE-ERROR-PRIVACY-001`,
`NTFS-MFT-SIZE-001`, `NTFS-RECORD-SIGNATURE-001`,
`NTFS-ATTRIBUTE-BOUNDS-001`, `NTFS-ATTRIBUTE-LIST-PARTIAL-001`,
`FAT-COMPLETENESS-001` through `FAT-COMPLETENESS-004`,
`FAT-TABLE-BOUND-001`, `FAT-DEPTH-BOUND-001`,
`FAT-DIRECTORY-CHAIN-BOUND-001`,
`CLI-IMAGE-FAT-PARTIAL-001`, `PART-GPT-BACKUP-001` through
`PART-GPT-BACKUP-005`, `PART-GPT-PROTECTIVE-001`,
`DESKTOP-VOLUME-SCAN-ERROR-001`, `DESKTOP-SCAN-STATUS-001`,
`DESKTOP-PARTIAL-SCAN-STATUS-001`, `DESKTOP-FAT-PARTIAL-001`,
`DESKTOP-NAVIGATION-FOCUS-001`,
`DESKTOP-FORCED-COLORS-FOCUS-001`, and `DESKTOP-DEV-CSP-001`.
The UI/CSP subset also includes `DESKTOP-VOLUME-TABLE-A11Y-001` and
`DESKTOP-DEV-CSP-002`.
This list records targeted evidence; it does not replace any required command
or native acceptance gate.

## Evidence rule

`Verified` requires a command, source identity, result and retained artifact.
The frozen-code local command results are retained in the
[2026-07-29 evidence record](../evidence/real-only-desktop-2026-07-29.md).
Until remote CI and native/package acceptance evidence are attached, the
desktop slice as a whole remains `Implemented-unverified`; independently
testable component requirements may be `Verified`.
