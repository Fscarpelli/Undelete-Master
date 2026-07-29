# S0 Environment Inventory

Captured: 2026-07-29  
Scope: discovery evidence plus current pre-release implementation inventory

## Repository

- Branch: `codex/foundation-hardening`.
- Design baseline: `f9befd5`.
- Local historical branch `master`: `127c2af`.
- No remote or upstream was configured during the historical baseline audit.
- `origin` now targets `Fscarpelli/Undelete-Master`; publication remains gated
  on final verification and a non-force branch/PR that preserves remote
  `main`.

## Toolchains observed

- Cargo 1.88.0.
- rustc 1.88.0.
- rustfmt 1.8.0-stable.
- Node 22.17.1.
- npm 11.6.2.
- pnpm 10.12.1.
- Git for Windows.

Tool presence is not test evidence. Task 8 records final command output on the
final tree.

## Historical repository capabilities at discovery

- Read-only image reader, checked regions, partition parsing, NTFS/FAT metadata
  scanners, deterministic fixture builder, and component tests.
- React/Vite desktop demonstration backed by synthetic data. That implementation
  has since been removed from the production tree.
- No Tauri shell, Windows broker, restore engine, carving/validation worker,
  session database, production exFAT, installer, signing, or release evidence.

## Baseline failures and gaps

- `cargo fmt --all -- --check` failed on formatting drift.
- The baseline recorded Clippy failure in FAT code.
- Frontend typecheck/tests passed in the baseline record, but the required lint
  command did not exist.
- No PR workflow, complete SDD family, traceability matrix, or risk register
  existed.
- Device and VHD tests were not run.

These are historical baseline observations captured in SDD-016. They must not
be cited as final results after concurrent implementation changes.

## Current implementation before the final gate

- The production desktop is a Tauri 2 application with one Rust-owned
  image-selection and scan command.
- The selected path remains inside Rust; the WebView receives only a bounded,
  path-free report with large integers represented as decimal strings.
- Browser-only execution fails closed and presents no substitute data.
- Restore, preview, sessions, carving, raw-device access, the elevated broker,
  and exFAT remain absent from the production command and route inventories.
- Frontend preferences are limited to locally effective language, theme, and
  reduced-motion settings. They store no source or scan report.

These statements describe the implemented tree as
`Implemented-unverified`. The same-revision final commands and artifact hashes
belong in the final evidence record.

## Assumptions

| ID | Assumption | Verification or disposition |
| --- | --- | --- |
| A-S0-001 | Concurrent workers share the worktree and may have uncommitted files outside this documentation scope. | Preserve unrelated changes; review the final path-scoped diff and full `git status` before integration. |
| A-S0-002 | Foundation CLI validation uses regular fixture or image files, never a physical disk or mounted volume. | Enforced by ADR-0003, CI policy, negative path tests, and the static CI guard; this is not proof of future broker behavior. |
| A-S0-003 | All automated storage tests in this increment use repository fixtures or temporary files. | Review test inputs and CI commands; no real-device or VHD attachment command is authorized. |
| A-S0-004 | The baseline desktop demonstration cannot remain in the product. | Disposed: production mock/demo modules and screenshots were removed; SDD-017 now governs a real-only Tauri image workflow. The future raw-device broker remains separate and absent. |
| A-S0-005 | Baseline tool versions are observations, not a supported-platform declaration. | Task 8 must capture the actual final command versions and results; ADR-0019 remains Proposed. |
| A-S0-006 | Remote history must be integrated without force-push or deletion. | Publish only the reviewed branch/PR after Task 8; preserve unrelated remote history. |

## Agents, skills, plugins, and MCP inventory

| Category | Used capability | Purpose / boundary |
| --- | --- | --- |
| Agent | Coordinating Codex agent | Scoped planning, integration, and final evidence; workers received non-overlapping ownership. |
| Agent | Rust/core worker | Parser, reader, CLI, and regression-test work; no real-device operations. |
| Agent | Desktop UX worker | Versioned frontend implementation and component checks; no privileged preview or recovered-file execution. |
| Agent | Documentation/CI worker and independent reviewer | SDD, ADR, traceability, CI-policy validators, and adversarial consistency review. |
| Skill/plugin | Superpowers | Specification planning, test-first changes, parallel dispatch, and verification discipline. |
| Skill/plugin | Build Web Apps | Frontend implementation guidance within `apps/desktop`; no deployment authority. |
| Skill/plugin | Product Design | UX/accessibility review and evidence framing; visual review is not functional verification. |
| Skill/plugin | GitHub | Repository/remote inspection and the user-authorized later branch publication boundary; no Task 7 remote mutation. |
| Skill/plugin | Computer Use | Requested by the user but no callable native Computer Use surface was available in this run; it supplied no evidence. |
| MCP/tool | In-app browser automation | Local Vite interaction and screenshots only; no recovered content and no privileged process. |
| MCP/tool | Local shell and patch tools | Read-only inspection, scoped file edits, and deterministic validators in this worktree. |
| MCP/tool | GitHub connector | Read-only repository context during implementation; publication remains a separate post-verification action. |
| Plugin | Atlassian, Box, Notion, SharePoint, Slack, and Teams | Available recommendations were not installed or used because they were outside the repository task. |

This inventory records capabilities actually used or explicitly requested. It
does not imply that a plugin result, screenshot, or agent statement is test
evidence.

## Preliminary risks

| Risk | S0 observation | Initial control / next evidence |
| --- | --- | --- |
| [R-001](../risk-register.md) | Any source write would be irreversible. | Read-only trait and regular-image boundary now; future broker requires separate controlled evidence. |
| [R-002](../risk-register.md) | Hostile metadata can overflow or escape a region. | Checked parser work and regression tests; Task 8 runs the final workspace gates. |
| [R-003](../risk-register.md) | Synthetic UI state can be mistaken for verified live recovery state. | ADR-0021, browser fail-closed behavior, route/command inventory tests, and the production-surface guard remove that state; final package inspection remains required. |
| [R-004](../risk-register.md) | A destination on the source disk could overwrite recoverable bytes. | Restore remains unimplemented; ADR-0015 defines the future fail-closed policy. |
| [R-007](../risk-register.md) | CI commands and manifest hooks can introduce device/destructive behavior. | Hosted fixture-only jobs, lifecycle scripts disabled, and enumerated static scanning; residual static-analysis limits remain explicit. |
| [R-008](../risk-register.md) | Dependencies can add supply-chain or license risk. | Locked installs and proposed dependency policy; full audit/deny/SBOM evidence is still open. |
| [R-009](../risk-register.md) | Remote integration could overwrite unrelated history. | Use a non-force branch/PR only after final verification. |
| [R-010](../risk-register.md) | Requirement drift can create unsupported completion claims. | Exact set/status/test/path/ADR validators plus independent review; semantic review remains required. |
| [R-011](../risk-register.md) | Sensitive paths/content can leak through diagnostics. | Default CLI redaction tests; broader logging/session evidence remains open. |
| [R-012](../risk-register.md) | Unsupported filesystem/platform claims can mislead users. | Honest compatibility statuses, proposed support ADRs, and explicit known limitations. |
