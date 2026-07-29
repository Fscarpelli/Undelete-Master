# SDD-017 — Real-only Image Desktop

Status: Implemented-unverified; frozen-code local gates pass, while
native/package and remote CI acceptance remain open
Decision date: 2026-07-29
Supersedes for desktop delivery: the demonstration-provider direction in
[ADR-0007](../adr/0007-demo-provider-trust-state.md)

Current evidence and pending gates are separated in the
[2026-07-29 evidence record](../evidence/real-only-desktop-2026-07-29.md).

## 1. Decision summary

The next desktop increment is a deliberately narrow, real Tauri 2 application:
the user selects one ordinary local image file, the unelevated Rust process
invokes the existing `um_cli::scan_image_path` implementation on a blocking
worker, and the UI renders only the returned scan summary.

No demonstration provider, fabricated source, generated candidate, simulated
progress, simulated restore, synthetic session, or runtime fallback may be
compiled into the production application. A browser-only launch fails closed
with an unavailable message and no data. Features without a real safe backend
are absent from navigation and cannot be triggered through hidden controls.

This increment does not weaken the master specification. It provides a real
subset while physical devices, an elevated read-only broker, restore, preview,
sessions, carving, and exFAT remain excluded.

## 2. Context and assumptions

The current Rust workspace already exposes a read-only regular-image scan
through `um-cli`. The desktop work under evaluation was designed around a
synthetic provider. The product owner has explicitly rejected any mockup or
fabricated application behavior.

The following assumptions bound this decision and must be revisited if they
change:

- the supported host is Windows 10 or Windows 11 on a local interactive
  desktop;
- the application runs as the signed-in user and never requests elevation;
- this slice serves one local user and one in-flight image scan per window;
- image length and byte offsets are Rust `u64` values and may exceed the exact
  integer range of JavaScript;
- scans may take long enough that they must never execute on the Tauri async
  runtime thread or the webview event loop;
- only an aggregate report crosses IPC; recovered bytes and candidate details
  do not;
- all source inputs are ordinary local, non-redirected files with `.img`,
  `.dd`, `.raw`, or `.bin` extensions;
- this increment does not claim race-free source identity or forensic chain of
  custody across picker, path validation, and open.

## 3. Scope

### 3.1 Included

- a Tauri 2 shell with React and TypeScript;
- an unelevated native file-open dialog restricted to `.img`, `.dd`, `.raw`,
  and `.bin`;
- a single Tauri command backed by the real `um-cli` library;
- read-only scanning of one regular local image at a time;
- a real summary containing source label/size, MBR/GPT recognition, volume
  offset/length, NTFS/FAT/unrecognized classification, required scan status,
  candidate count, and parser warnings;
- localized, accessible pending, success, empty, and error states;
- only preferences that immediately change the application: language,
  light/dark/system theme, and reduced motion;
- bounded, privacy-preserving IPC and report rendering.

### 3.2 Explicitly excluded

The production application in this increment has no physical/raw-device or
volume access, no UAC broker, no elevation, no working-folder workflow, no
pause/resume/cancel claim, no restore, no preview, no candidate browser, no
persisted/imported/exported sessions, no carving, no exFAT, and no shell or
Explorer launch.

Unsupported capabilities are omitted. They are not represented by disabled
flows populated with sample data, hidden routes, success toasts, timers, or
client-side emulation.

An individual candidate may be exposed in a later increment only when its
identity, metadata, extents, score inputs, and warnings come from the real
scanner response under a versioned contract. Until that contract exists, this
increment exposes aggregate counts only; it never manufactures candidate rows.

## 4. Approaches considered

### A. Retain an explicitly labeled demonstration provider

This would preserve broad visual flows and make UI iteration inexpensive.
However, it conflicts with the product-owner direction, keeps non-functional
controls in the application, and creates an avoidable risk that screenshots or
state are mistaken for recovery evidence. Rejected.

### B. Keep the browser app and call a separately installed local service

This could provide real data without Tauri, but adds service discovery,
authentication, port/CORS exposure, installation, lifecycle, and update
complexity. It expands the attack surface without helping the current
image-only requirement. Rejected.

### C. Build the full raw-device, broker, session, preview, and restore product

This aligns with the long-term master specification, but cannot be delivered
safely as a small pivot. It would require audited Windows FFI, elevation
boundaries, physical-disk identity, transactional restore, sandboxing, and
substantially broader acceptance evidence. Deferred.

### D. Ship a narrow real-only Tauri image scanner

This uses the existing safe Rust path, removes fabricated behavior, keeps the
desktop unelevated, and produces immediately useful real results. The accepted
trade-off is a smaller UI and no per-candidate recovery workflow. Proposed.

## 5. Architecture

### 5.1 Runtime boundary

Production runtime selection is binary:

1. If the Tauri 2 runtime and registered command are available, the application
   may show the real image workflow.
2. Otherwise, the application shows a static “desktop runtime required”
   unavailable state. It does not create a provider, source, session, report,
   progress percentage, or synthetic result.

There is exactly one production data path:

```text
webview invokes select_and_scan_image(requestId) without a path
  -> Rust command opens the filtered native dialog
  -> cancel returns null
  -> selected path remains inside Rust
  -> spawn_blocking
  -> um_cli::scan_image_path
  -> bounded DesktopScanReport
  -> real summary UI
```

The application does not spawn a CLI process and does not parse terminal
output. The Tauri Rust crate links the `um-cli` library and calls
`um_cli::scan_image_path` directly so the same validation and scanner behavior
is shared with the command-line entry point.

### 5.2 Native selection and backend validation

The Rust command opens the native dialog with one filter containing only `img`,
`dd`, `raw`, and `bin`. Cancel returns `Ok(None)` to the webview and restores
the idle state without an error or fabricated selection. A filesystem path is
never accepted from or returned to JavaScript.

The dialog filter is convenience, not a trust boundary. Immediately before
opening the source, the blocking Rust task passes the path to
`um_cli::scan_image_path`. `crates/io-common` and its minimal
`crates/io-windows` locality boundary revalidate:

- forbidden device, UNC, extended, NT-object-manager, reserved-name, and
  alternate-stream spellings;
- ordinary drive-letter locality through read-only `GetDriveTypeW`, rejecting
  mapped remote drives, unsupported roots, and classification errors;
- supported extension, case-insensitively;
- existing non-empty regular file;
- no directory, symlink/reparse final object or ancestor, pipe, device, or other
  special file;
- read-only open, final-handle metadata validation without following a final
  reparse point, and bounded regions.

No frontend flag can bypass those checks. The full path remains within the Rust
command and is not persisted, logged, included in the report, returned in an
error, or sent across IPC.

`GetDriveTypeW` is the only unsafe call in the scan path and only observes a
fixed drive-root buffer; it does not open or mutate a source. The picker,
drive-root classification, ancestor inspection, and final open are nevertheless
separate pathname operations. They reduce common remote/reparse redirection but
are not race-free and do not bind selection to a retained file identity. The
accepted residual and revisit boundary are specified in
[ADR-0022](../adr/0022-windows-locality-boundary-and-path-identity.md).

### 5.3 Command and concurrency model

The command contract is conceptually:

```rust
#[tauri::command]
async fn select_and_scan_image(
    app: tauri::AppHandle,
    request_id: String,
) -> Result<Option<DesktopScanReport>, DesktopCommandError>
```

The async command validates the request envelope, obtains the path from the
Rust-owned native dialog, and returns `None` on cancel. For a selection, it
calls `tauri::async_runtime::spawn_blocking` for all filesystem and scanner
work. Join failure and scanner failure are mapped to stable sanitized error
codes. There is no subprocess, write handle, device command, path-bearing IPC,
timer-driven result, or client-side filesystem parser.

The UI sets an in-flight guard synchronously before invocation, disables
selection/start controls while the request is pending, and rejects a second
submit. A monotonically increasing request token is captured by every call.
After unmount, runtime loss, or a newer token, an older response is ignored and
cannot overwrite the current state.

Because the real scanner does not yet expose observable phases or cooperative
cancellation, the UI shows only an indeterminate “scan in progress” status
while the command is pending. It never shows a percentage, phase name,
pause/resume control, or cancellation success that was not emitted by the
engine.

### 5.4 Real report contract

`DesktopScanReport` is a schema-version-2 adapter over the real
`ImageScanReport`. It contains:

- schema version;
- source filename label and size;
- partition-table kind: `mbr`, `gpt`, or `none`;
- for every bounded volume: index, byte offset, byte length, filesystem kind
  (`ntfs`, `fat12`, `fat16`, `fat32`, or `unrecognized`), required
  `scanStatus` (`complete`, `partial`, or `unrecognized`), candidate count, and
  warnings;
- partition-level warnings;
- explicit warning totals and any bounded-transport omission count.

The adapter and TypeScript boundary reject invalid status combinations:
`complete` or `partial` is permitted only for a recognized NTFS/FAT volume, and
`unrecognized` is permitted only with the unrecognized filesystem kind. NTFS
is partial when an MFT work cap, shortened trusted prefix, nonblank record
without a `FILE` signature, skipped unreadable/torn/corrupt record, malformed
ordinary/extension-record attributes, an `$ATTRIBUTE_LIST` whose complete
reference and candidate-defining-attribute resolution is not proven, or
extension-record merge failure prevents exhaustive enumeration. The current
scanner therefore marks every record with an `$ATTRIBUTE_LIST`, including a
well-formed resident value, incomplete. FAT permits one through four total
declared copies and is partial when any secondary copy diverges or is
unreadable; reachable directory traversal is broken, cyclic, unreadable, or
lacks a usable start cluster; or its directory count/byte/depth/chain safety
bound stops enumeration. The first FAT copy remains authoritative.

A record-unaligned `$MFT.data_size`, or an aligned size covering fewer than all
16 reserved record slots, is a corrupt NTFS volume, not a partial report.
Malformed record-0 attributes are also fatal. The FAT directory-chain cluster
limit is derived from 8 MiB divided by cluster size and applied before reading
the directory clusters. A declared FAT table above 64 MiB is rejected before
allocation; a fatal first-FAT-copy/root read likewise remains a typed scan
error.
Partition-table kind inherits the canonical GPT contract from SDD-006: both
canonical copies are evaluated, only the final-LBA backup is eligible, and two
valid headers must be reciprocal and metadata-consistent. Primary read failure
may fall back only to an independently valid canonical backup.

The UI derives totals only by adding values from this DTO. It renders a status
for every volume and, if any volume is partial, a localized coverage caveat
that says the observed candidate count is not exhaustive. It must not invent
candidate names, scores, recoverability, timestamps, volume health, scan
phases, or restore outcomes.

Candidate-level results are prohibited in this slice. A future row/detail view
must consume individual records emitted by the real Rust scanner; a client-side
expansion of counts or a synthetic fallback is never permitted.

### 5.5 `u64`, IPC, and large-report limits

Rust remains authoritative for all sizes, offsets, lengths, and counts. Every
potentially large integer crosses IPC as a canonical unsigned decimal string.
The TypeScript boundary accepts only `/^(0|[1-9][0-9]*)$/`, converts with
`BigInt`, and never routes these values through `Number`. Schema version and
bounded volume index remain JSON numbers.

Defense-in-depth transport limits are:

- at most 1,024 volume summaries;
- at most 512 returned warnings across the report;
- at most 512 Unicode scalar values per returned warning;
- at most 1 MiB of serialized command payload.

The adapter counts real warnings before bounding them and returns
`warningCount` and `warningsOmitted` as decimal strings. If structural limits
or the final payload limit cannot be satisfied, the command fails closed with
`SCAN_REPORT_TOO_LARGE`; it never fabricates or silently drops structural
records. The UI renders report sections incrementally and does not duplicate
the report into multiple application stores.

### 5.6 Error and privacy contract

The Rust boundary returns one of these stable codes with localized UI text:

- `DESKTOP_RUNTIME_UNAVAILABLE`;
- `SOURCE_FORBIDDEN`;
- `SOURCE_UNSUPPORTED`;
- `SOURCE_NOT_REGULAR`;
- `SOURCE_EMPTY`;
- `SOURCE_IO`;
- `SCAN_CORRUPT`;
- `SCAN_REPORT_TOO_LARGE`;
- `SCAN_INTERNAL`.

Parser fallthrough occurs only when a parser returns `NotRecognized`.
Partition discovery may use a whole-image volume only for that result, and FAT
may run only after NTFS returns it. A recognized volume's typed `Read` failure
maps to `SOURCE_IO`; its typed `Corrupt` failure maps to `SCAN_CORRUPT`.
Neither may be hidden by probing a different filesystem. Invalid `$MFT`
logical size and invalid canonical/reciprocal GPT topology therefore surface as
`SCAN_CORRUPT`, without raw parser detail.

Errors never include a full path, recovered content, backtrace, raw OS error
string, partition-read offset, or source bytes. The UI may show the selected
filename label. Parser warnings and labels are treated as untrusted text:
control characters and Unicode bidi formatting controls are removed, length is
bounded, and rendering uses React text nodes. The source heading additionally
uses `<bdi dir="auto">`; other Unicode confusables remain possible, so a label
is display metadata rather than authoritative identity.

### 5.7 Tauri security profile

The Tauri process uses the Windows `asInvoker` execution level. No code path
requests administrator rights. The WebView capability grants no plugin or host
permission. The dialog plugin is invoked only by trusted Rust code, and native
file drag/drop is disabled so it cannot become a second path-ingress channel.
There is no shell, process, filesystem, HTTP, updater, clipboard, global
shortcut, or device capability.

The production content security policy is:

```text
default-src 'self'; script-src 'self'; style-src 'self';
img-src 'self' data:; font-src 'self';
connect-src ipc: http://ipc.localhost;
object-src 'none'; base-uri 'none'; frame-ancestors 'none'
```

No remote script, font, image, analytics, telemetry, or network endpoint is
required. The custom command allowlist contains only
`select_and_scan_image`. The stylesheet is a same-origin linked resource so
both development and production remain styled without adding
`style-src 'unsafe-inline'`.

Development adds exactly `ws://localhost:1420` to `connect-src` for the fixed
Vite live-reload server. A development-only HTML transform adds that same
single origin to the source document's meta policy while Vite serves it, so
both enforced development policies agree. This `devCsp` exception and
transform are development-only; production configuration and built HTML are
unchanged and contain no WebSocket origin.

### 5.8 Effective preferences

Only preferences with immediate observable behavior are shown. For this
increment, `locale`, light/dark/system `theme`, and `reducedMotion` are wired
and persisted under `undelete-master.preferences.v1`. Locale changes the
document language and catalog, theme changes the resolved document theme, and
`reducedMotion` sets `data-reduced-motion` on the document root; the stylesheet
then removes nonessential transition timing and scan animation. Invalid stored
values fall back safely. No image path, report, or warning is persisted.

Controls for scan depth, carving, validation, repair, telemetry, restore,
session retention, performance, updates, or broker behavior are absent until a
real implementation consumes and verifies them.

### 5.9 Navigation focus and forced colors

Changing the active top-level view moves programmatic focus to the new view's
level-one heading so keyboard and assistive-technology users receive a
deterministic context change. Focusable controls and headings have a visible
focus indicator. Under Windows forced-colors mode, the indicator uses the
system `Highlight` color and an outline rather than depending on box shadow.

The volume table's horizontal-scroll container is a keyboard-focusable
`region` with a localized accessible name. The table carries a visually hidden
caption with the same localized name, so both the scroll boundary and table
purpose are available to assistive technology.

Focused component/CSS tests cover this contract. Native visual, 200% zoom, and
assistive-technology acceptance remain open.

## 6. Normative requirements

### SDD-REAL-001 — No fabricated production behavior

- **Rationale:** application output must be attributable to a real scanner
  invocation.
- **Priority:** Must.
- **Source:** product-owner direction; `AGENTS.md`.
- **Preconditions:** a production desktop build or browser-only launch starts.
- **Behavior:** production code and assets contain no demonstration provider,
  fabricated source/result/session/restore data, simulated progress, timer
  completion, or fallback dataset.
- **Error behavior:** unavailable runtime produces an unavailable state with no
  data.
- **Security implications:** prevents false recovery evidence and unsupported
  trust claims.
- **Observability:** build inspection and runtime tests can identify the sole
  Tauri data path.
- **Acceptance criteria:** `REAL-AC-001` and `REAL-AC-002` pass.
- **Test IDs:** `DESKTOP-TAURI-ONLY-001`,
  `DESKTOP-UNSUPPORTED-ABSENT-001`, `DESKTOP-REAL-ONLY-001`.
- **Implementation links:** `apps/desktop/src`,
  `apps/desktop/src-tauri`, `.github/scripts/validate_real_only_desktop.py`.
- **Status:** Implemented-unverified.

### SDD-REAL-002 — Unelevated Tauri-only runtime

- **Rationale:** the image-only flow needs native integration but no privilege.
- **Priority:** Must.
- **Source:** master specification §§4, 7; `AGENTS.md`.
- **Preconditions:** the production executable starts normally or the web app
  is opened without Tauri.
- **Behavior:** Tauri 2 runs as the current user; browser runtime fails closed.
- **Error behavior:** missing/invalid IPC never selects another provider.
- **Security implications:** preserves the privilege boundary and excludes raw
  devices.
- **Observability:** runtime state and Windows manifest are inspectable.
- **Acceptance criteria:** `REAL-AC-002` and `REAL-AC-010` pass.
- **Test IDs:** `DESKTOP-TAURI-ONLY-001`,
  `DESKTOP-AS-INVOKER-001`.
- **Implementation links:** `apps/desktop/src/api/desktop.ts`,
  `apps/desktop/src-tauri/windows-app-manifest.xml`.
- **Status:** Implemented-unverified.

### SDD-REAL-003 — Native picker and authoritative path validation

- **Rationale:** a native filter improves selection but cannot be trusted as
  source validation.
- **Priority:** Must.
- **Source:** master specification FR-003, FR-033; ADR-0003.
- **Preconditions:** the user chooses or cancels an image.
- **Behavior:** the Rust command opens a dialog offering only `.img`, `.dd`,
  `.raw`, and `.bin`; the path never crosses IPC and is revalidated through
  `um_cli`, `io-common`, and the `io-windows` locality boundary before
  read-only open. Mapped-remote classification, any reparse ancestor, and final
  redirected/non-file handles fail closed.
- **Error behavior:** all excluded paths fail before scanner invocation and
  expose only a stable code.
- **Security implications:** blocks device/special-file access, common
  mapped-remote/reparse redirection, and source writes. It does not claim a
  race-free retained identity.
- **Observability:** Rust tests exercise the private post-selection boundary
  with rejected paths while IPC tests prove the command has no path argument.
- **Acceptance criteria:** `REAL-AC-003` passes.
- **Test IDs:** `DESKTOP-PICKER-FILTER-001`,
  `DESKTOP-PICKER-CANCEL-001`, `DESKTOP-COMMAND-INVENTORY-001`,
  `DESKTOP-BACKEND-PATH-001`, `DESKTOP-REPARSE-GUARD-001`,
  `WINDOWS-DRIVE-TYPE-001`, `WINDOWS-DRIVE-ROOT-001`,
  `IO-REMOTE-PATH-001`, `IO-ANCESTOR-REPARSE-001`.
- **Implementation links:** `apps/desktop/src-tauri/src/scan.rs`,
  `crates/cli/src/lib.rs`, `crates/io-common/src/lib.rs`,
  `crates/io-windows/src/lib.rs`.
- **Status:** Implemented-unverified.

### SDD-REAL-004 — Real blocking scan execution

- **Rationale:** scanner I/O and parsing must not block async runtime or webview
  event handling.
- **Priority:** Must.
- **Source:** master specification §§8, 20; ADR-0003.
- **Preconditions:** the Rust-owned dialog returned a selection and the
  webview supplied a unique request ID.
- **Behavior:** `spawn_blocking` calls `um_cli::scan_image_path` directly and
  returns its mapped report.
- **Error behavior:** task panic/join and scanner errors fail closed. Typed
  recognized-volume `Read` and `Corrupt` failures remain distinct for sanitized
  desktop mapping.
- **Security implications:** eliminates shell execution and preserves the
  audited read-only library path.
- **Observability:** fixture parity tests compare Tauri DTO values with direct
  `um_cli` output.
- **Acceptance criteria:** `REAL-AC-004` passes.
- **Test IDs:** `DESKTOP-CLI-PARITY-001`,
  `DESKTOP-CLI-PARITY-002`, `DESKTOP-CLI-PARITY-003`,
  `DESKTOP-VOLUME-SCAN-ERROR-001`.
- **Implementation links:** `apps/desktop/src-tauri/src/scan.rs`.
- **Status:** Implemented-unverified.

### SDD-REAL-005 — Truthful scan summary

- **Rationale:** useful desktop output must reflect actual partition and
  filesystem scanners.
- **Priority:** Must.
- **Source:** master specification FR-030, FR-033, FR-050, FR-052, FR-054.
- **Preconditions:** `um_cli` returns an `ImageScanReport`.
- **Behavior:** schema version 2 requires the UI to render the real
  MBR/GPT/none kind, NTFS/FAT/unrecognized volumes, per-volume scan status,
  candidate counts, warnings, and a coverage caveat for any partial recognized
  volume.
- **Error behavior:** unknown enum/schema values fail to an incompatible-report
  state rather than being relabeled.
- **Security implications:** avoids overstating filesystem or recovery support.
- **Observability:** deterministic FAT, NTFS, MBR, and GPT fixtures have exact
  expected summaries and status combinations. Direct CLI and Tauri paths are
  compared for
  unpartitioned FAT, MBR/NTFS, and GPT/NTFS images without changing fixture
  SHA-256. Focused tests also preserve incomplete FAT through the CLI, Rust
  desktop adapter, and TypeScript report boundary, and exercise canonical GPT
  backup selection/conflict handling before adaptation. Parser regressions
  distinguish fatal bootstrap/allocation bounds from recoverable NTFS
  attribute and FAT directory bounds. `DESKTOP-SCAN-STATUS-001` explicitly
  preserves both NTFS and FAT partial statuses in the Rust adapter.
- **Acceptance criteria:** `REAL-AC-005` passes.
- **Test IDs:** `DESKTOP-REAL-SUMMARY-001`,
  `DESKTOP-REPORT-SCHEMA-001`, `DESKTOP-CLI-PARITY-001`,
  `DESKTOP-CLI-PARITY-002`, `DESKTOP-CLI-PARITY-003`,
  `DESKTOP-SCAN-STATUS-001`, `DESKTOP-PARTIAL-SCAN-STATUS-001`,
  `DESKTOP-FAT-PARTIAL-001`, `CLI-IMAGE-FAT-PARTIAL-001`,
  `FAT-COMPLETENESS-001` through `FAT-COMPLETENESS-004`,
  `FAT-TABLE-BOUND-001`, `FAT-DEPTH-BOUND-001`,
  `FAT-DIRECTORY-CHAIN-BOUND-001`, `NTFS-MFT-SIZE-001`,
  `NTFS-RECORD-SIGNATURE-001`, `NTFS-ATTRIBUTE-BOUNDS-001`,
  `NTFS-ATTRIBUTE-LIST-PARTIAL-001`,
  `PART-GPT-BACKUP-001` through `PART-GPT-BACKUP-005`.
- **Implementation links:** `crates/partition/src/gpt.rs`,
  `crates/fs-ntfs/src/scan.rs`, `crates/fs-fat/src/scan.rs`,
  `crates/cli/src/lib.rs`,
  `apps/desktop/src/views/AnalysisView.tsx`,
  `apps/desktop/src/api/report.ts`, `apps/desktop/src-tauri/src/scan.rs`.
- **Status:** Implemented-unverified.

### SDD-REAL-006 — Unsupported workflows are absent

- **Rationale:** a visible but non-functional feature is a product defect and a
  trust risk.
- **Priority:** Must.
- **Source:** product-owner direction; master specification delivery rules.
- **Preconditions:** the production navigation and command registry are
  inspected.
- **Behavior:** raw devices, broker, restore, preview, sessions, carving,
  exFAT, working folders, and Explorer launch have no production route,
  command, or control.
- **Error behavior:** deep-link attempts resolve to the real image start view
  or a not-supported page with no data or side effect.
- **Security implications:** keeps unimplemented privileged/write paths
  unreachable.
- **Observability:** route, command, capability, and bundle inventories are
  finite and testable.
- **Acceptance criteria:** `REAL-AC-006` passes.
- **Test IDs:** `DESKTOP-UNSUPPORTED-ABSENT-001`,
  `DESKTOP-COMMAND-INVENTORY-001`.
- **Implementation links:** `apps/desktop/src/App.tsx`,
  `apps/desktop/src-tauri/src/lib.rs`.
- **Status:** Implemented-unverified.

### SDD-REAL-007 — Sanitized errors and Rust-confined path

- **Rationale:** local paths and recovered metadata are sensitive.
- **Priority:** Must.
- **Source:** master specification FR-101 and §22.
- **Preconditions:** selection, scan, serialization, or IPC fails.
- **Behavior:** the backend returns stable codes; typed recognized-volume read
  failures map to `SOURCE_IO`, and corrupt-structure failures map to
  `SCAN_CORRUPT`. The full selected path exists only inside Rust and only long
  enough to complete the scan. Labels and warnings have control and bidi
  formatting characters removed before IPC, and the source heading is
  directionally isolated.
- **Error behavior:** path, raw OS error, partition-read offset, content, and
  backtrace are omitted.
- **Security implications:** limits privacy leakage and unsafe warning
  rendering.
- **Observability:** error and warning tests use a unique secret path marker.
- **Acceptance criteria:** `REAL-AC-007` passes.
- **Test IDs:** `DESKTOP-ERROR-PRIVACY-001`,
  `DESKTOP-PARTITION-READ-ERROR-001`, `DESKTOP-WARNING-TEXT-001`,
  `DESKTOP-BIDI-TEXT-001`, `DESKTOP-BIDI-ISOLATE-001`,
  `CLI-PROBE-ERROR-PRIVACY-001`, `DESKTOP-VOLUME-SCAN-ERROR-001`.
- **Implementation links:** `apps/desktop/src-tauri/src/scan.rs`,
  `apps/desktop/src/api/desktop.ts`,
  `apps/desktop/src/views/AnalysisView.tsx`.
- **Status:** Implemented-unverified.

### SDD-REAL-008 — Duplicate and stale response control

- **Rationale:** repeated clicks and late async responses must not create
  ambiguous results.
- **Priority:** Must.
- **Source:** master specification §§19, 21.
- **Preconditions:** a scan is pending, the view unmounts, or a later request
  token exists.
- **Behavior:** a synchronous in-flight guard admits one invocation and a
  monotonic token gates state updates.
- **Error behavior:** stale success/error responses are ignored and do not
  mutate current UI.
- **Security implications:** prevents mismatching a report with a later source
  selection.
- **Observability:** deferred-promise tests control response ordering.
- **Acceptance criteria:** `REAL-AC-008` passes.
- **Test IDs:** `DESKTOP-DOUBLE-SUBMIT-001`,
  `DESKTOP-STALE-RESPONSE-001`.
- **Implementation links:** `apps/desktop/src/views/AnalysisView.tsx`.
- **Status:** Verified.

### SDD-REAL-009 — Preferences are effective

- **Rationale:** a saved control that changes no behavior is another form of
  fabricated functionality.
- **Priority:** High.
- **Source:** product-owner direction; master specification FR-100, FR-103.
- **Preconditions:** a preference is displayed or changed.
- **Behavior:** locale, light/dark/system theme, and reduced motion immediately
  change the corresponding document/UI behavior and persist locally.
  `reducedMotion` sets the document-root state consumed by CSS to remove
  nonessential transitions and scan animation.
- **Error behavior:** invalid persisted values reset to a documented safe
  default.
- **Security implications:** no source/report data is stored with preferences.
- **Observability:** reload tests verify locale, resolved theme,
  `data-reduced-motion`, the exact three-field storage scope, and safe fallback
  behavior.
- **Acceptance criteria:** `REAL-AC-009` passes.
- **Test IDs:** `DESKTOP-EFFECTIVE-PREFS-001`.
- **Implementation links:** `apps/desktop/src/state/preferences.ts`,
  `apps/desktop/src/views/SettingsView.tsx`.
- **Status:** Verified.

### SDD-REAL-010 — Minimal CSP, capabilities, and resilient focus

- **Rationale:** the real application does not need general host or network
  access.
- **Priority:** Must.
- **Source:** master specification §§7, 21, 22.
- **Preconditions:** Tauri configuration and generated capabilities are built.
- **Behavior:** the WebView receives no plugin permission, native file
  drag/drop is disabled, the stylesheet is loaded from the same origin without
  inline-style permission, and only the
  application-owned `select_and_scan_image` command is registered under the
  documented production CSP. Development adds only
  `ws://localhost:1420` through `devCsp` and the dev-only served-HTML
  transform; built production HTML stays WebSocket-free. The dialog plugin is
  called exclusively from Rust. Navigation focuses the new view heading,
  forced-colors focus uses a system outline, and the scrollable volume table is
  a named focusable region with an accessible caption.
- **Error behavior:** denied operations remain unavailable; no permissive
  fallback is introduced.
- **Security implications:** reduces webview-to-host and supply-chain blast
  radius.
- **Observability:** configuration tests enumerate grants, production/dev CSP
  directives, navigation focus, and forced-colors focus rules.
- **Acceptance criteria:** `REAL-AC-010` passes.
- **Test IDs:** `DESKTOP-CAPABILITY-MIN-001`, `DESKTOP-CSP-001`,
  `DESKTOP-CSP-STYLES-001`, `DESKTOP-DRAG-DROP-OFF-001`,
  `DESKTOP-DEV-CSP-001`, `DESKTOP-DEV-CSP-002`,
  `DESKTOP-NAVIGATION-FOCUS-001`,
  `DESKTOP-FORCED-COLORS-FOCUS-001`,
  `DESKTOP-VOLUME-TABLE-A11Y-001`.
- **Implementation links:** `apps/desktop/src-tauri/capabilities/main.json`,
  `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/index.html`,
  `apps/desktop/dev-csp.ts`, `apps/desktop/vite.config.ts`,
  `apps/desktop/src/App.tsx`, `apps/desktop/src/views/AnalysisView.tsx`,
  `apps/desktop/src/styles/global.css`.
- **Status:** Implemented-unverified.

### SDD-REAL-011 — Exact large-integer and bounded-report transport

- **Rationale:** JSON numbers can silently corrupt `u64` values and unbounded
  reports can exhaust the webview.
- **Priority:** Must.
- **Source:** master specification §§10, 20, 21.
- **Preconditions:** a report contains large offsets/counts or many warnings.
- **Behavior:** large values use canonical decimal strings/`BigInt`; structural
  and payload limits are enforced with explicit warning counts. Schema version
  2 and required per-volume status/filesystem combinations, including partial
  recognized FAT, are validated before presentation.
- **Error behavior:** malformed integers or oversized structural reports fail
  closed without partial structural claims.
- **Security implications:** prevents precision bugs and resource exhaustion.
- **Observability:** boundary tests cover `2^53 - 1`, `2^53`, `u64::MAX`,
  warning caps, and the payload ceiling.
- **Acceptance criteria:** `REAL-AC-011` passes.
- **Test IDs:** `DESKTOP-U64-IPC-001`, `DESKTOP-U64-IPC-002`,
  `DESKTOP-REPORT-BOUND-001`, `DESKTOP-PAYLOAD-LIMIT-001`,
  `DESKTOP-REPORT-SCHEMA-001`, `DESKTOP-SCAN-STATUS-001`,
  `DESKTOP-FAT-PARTIAL-001`.
- **Implementation links:** `apps/desktop/src-tauri/src/scan.rs`,
  `apps/desktop/src/api/report.ts`.
- **Status:** Verified.

### SDD-REAL-012 — Real-only verification gate

- **Rationale:** passing component tests is not proof that the packaged app
  contains only real behavior.
- **Priority:** Must.
- **Source:** `AGENTS.md`; master specification §§23.7, 28, 30.
- **Preconditions:** a candidate production build exists.
- **Behavior:** source, tests, compiled assets, Tauri configuration, browser
  fail-closed state, and a real packaged fixture scan are independently
  inspected.
- **Error behavior:** any synthetic production path, unsupported command,
  warning, failing test, or missing evidence blocks publication.
- **Security implications:** prevents accidental reintroduction of fabricated
  or privileged paths.
- **Observability:** command logs and evidence name the exact revision and
  artifact hash.
- **Acceptance criteria:** `REAL-AC-012` passes.
- **Test IDs:** `DESKTOP-REAL-ONLY-001`,
  `DESKTOP-UNSUPPORTED-ABSENT-001`.
- **Implementation links:** `.github/scripts/validate_real_only_desktop.py`,
  `.github/workflows/quality.yml`.
- **Status:** Implemented-unverified.

## 7. Acceptance criteria

| ID | Pass condition |
| --- | --- |
| REAL-AC-001 | Production source and built assets contain no synthetic provider, fabricated domain dataset, timer-driven scan/restore/session result, or fallback report. Test-only fixtures cannot be imported by production modules. |
| REAL-AC-002 | Browser-only runtime shows one unavailable state and invokes no data command; Tauri runtime has exactly one real provider path. |
| REAL-AC-003 | `select_and_scan_image` has no path argument, opens the Rust-owned picker filtered to four extensions, returns null on cancel, and private post-selection tests reject unsupported, device, UNC, alternate-stream, mapped-remote, directory, final/ancestor symlink/reparse, and empty inputs before scanning. This is not a claim that the pathname sequence is race-free. |
| REAL-AC-004 | A deterministic image produces the same sanitized values through the Tauri command and direct `um_cli::scan_image_path`, while the UI remains responsive; recognized-volume read and corruption failures preserve their typed `SOURCE_IO`/`SCAN_CORRUPT` mapping. |
| REAL-AC-005 | Deterministic MBR, GPT, NTFS, and FAT images display exact real table kind, volume kind, required scan status, candidate count, and warnings; partial NTFS or FAT results preserve warnings and display a coverage caveat; direct CLI/Tauri parity includes a deterministic GPT/NTFS fixture and unchanged SHA-256; no candidate detail is invented. |
| REAL-AC-006 | Production route, command, capability, and UI inventories contain none of the explicitly excluded workflows. |
| REAL-AC-007 | A unique full-path marker never crosses IPC and is absent from success JSON, UI, errors, logs, screenshots, and built assets; recognized parser errors map to stable generic codes without fallback, raw OS details or partition-read offsets; bidi formatting controls are removed and the source heading is directionally isolated. |
| REAL-AC-008 | Rapid double activation invokes once; a late result after unmount or a newer token does not change the visible report/error. |
| REAL-AC-009 | Locale, light/dark/system theme, and reduced motion have tested immediate effects and persist as exactly those three fields; reduced motion drives the document CSS state; no inert preference or source/report persistence exists. |
| REAL-AC-010 | The Windows manifest is `asInvoker`; production CSP and capabilities match §5.7; Tauri `devCsp` and served development HTML add only one `ws://localhost:1420`, while built production HTML remains WebSocket-free; no shell, process, general filesystem, production-network, updater, clipboard, shortcut, or device permission is granted; navigation focuses the new view heading, forced-colors mode retains a system outline, and the scrollable volume table is a named focusable region with an accessible caption. |
| REAL-AC-011 | `2^53 - 1`, `2^53`, and `u64::MAX` round-trip exactly; schema version 2 rejects missing or invalid per-volume scan-status combinations; bounded warnings disclose omission count; payload overflow returns `SCAN_REPORT_TOO_LARGE`. |
| REAL-AC-012 | Required Rust/frontend gates, Tauri unit/integration tests, production build inspection, browser fail-closed test, packaged real-fixture smoke, and independent review all pass on the same revision. |

## 8. Delivery phases and gates

Implementation paths for R1–R3 exist, but none of those phases is a final
same-revision acceptance claim until Task 8 command logs are retained. R4 and
R5 remain open. The sealed security review is one bounded input and does not
replace the gates below.

### R0 — Specification gate

- SDD-017 and ADR-0021 are reviewed.
- ADR-0007 is marked superseded.
- traceability, risks, limitations, and this implementation plan agree.
- all SDD-REAL requirements remain `Not started`.

No application implementation starts before R0 passes.

### R1 — Backend contract gate

- Tauri 2 shell is unelevated and least-privileged.
- real `um-cli` invocation, locality/ancestor path revalidation, sanitized
  typed volume errors/text, schema-version-2 NTFS/FAT scan status, exact
  integer mapping, and report limits have focused regression tests.

### R2 — Real UI gate

- production mock/demo modules and unsupported routes are removed;
- browser runtime fails closed;
- image selection, true pending state, report, partial-coverage caveat, errors,
  heading focus, named focusable volume-table region/caption, forced-colors
  focus, accessibility, and effective preferences are connected only to the
  Tauri command.

### R3 — Integration gate

- direct CLI and Tauri reports have parity tests for deterministic
  unpartitioned FAT, MBR/NTFS, and GPT/NTFS images;
- incomplete FAT status has focused scanner-to-CLI and Rust/TypeScript desktop
  boundary regressions;
- duplicate/stale response, privacy, typed parser errors, NTFS/FAT status
  combinations, limits, production/development served-HTML CSP, focus,
  accessible volume table, and capability tests pass;
- no real disk/device is opened by any test.

### R4 — Package gate

- the production bundle is inspected for fabricated data and excluded commands;
- a packaged, unelevated executable scans an allowlisted synthetic image;
- artifact hash, revision, exact commands, and screenshots are recorded.

### R5 — Publication gate

- all repository-required gates pass on the final tree;
- independent security and UX reviews pass;
- traceability changes to `Implemented-unverified` or `Verified` only for
  acceptance criteria backed by same-revision evidence.

## 9. Rollback

Before publication, rollback is removal of the desktop package while retaining
the image-only CLI. After publication, any runtime fallback, path leak,
privilege expansion, source-write possibility, precision loss, or report-bound
failure immediately blocks the desktop artifact. Rollback never restores the
demonstration provider; the safe degraded state is the real CLI plus a desktop
runtime-unavailable message.

## 10. Revisit triggers

Create a new SDD/ADR before changing this architecture when any of the following
becomes necessary:

- physical/raw devices or an elevated broker;
- candidate-level results, recovery, restore, preview, or session persistence;
- cooperative scanner progress/cancellation;
- segmented RAW, VHD, VHDX, exFAT, carving, or repair;
- reports that cannot fit the bounded summary contract;
- more than one concurrent scan per window;
- a browser-accessible or remote-service deployment;
- a Tauri capability, network origin, or privilege not listed in §5.7.
