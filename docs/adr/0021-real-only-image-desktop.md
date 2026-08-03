# ADR-0021 — Real-only Tauri Image Desktop

Status: Accepted historical slice; desktop workflow superseded by ADR-0023
through ADR-0028

> **Evolution notice:** the real-only, fail-closed and unelevated-WebView
> decisions remain active. The image-picker desktop, absence of candidate
> detail/restore and absence of measurable scan progress describe the
> 2026-07-29 increment only. The current desktop uses connected mounted volumes,
> exposes request-bound MFT progress and provides native selection and
> transactional recovery under [ADR-0023](0023-windows-read-only-broker-and-folder-scope.md)
> through [ADR-0028](0028-restore-plan-job-and-manifest-lifecycle.md). The
> regular-image workflow remains available only in the CLI.

## Context

The Rust workspace has a real, read-only regular-image scan path in `um-cli`.
The desktop design under evaluation used a synthetic provider to exercise
sources, sessions, progress, results, preview, and restore. The product owner
requires that the application contain no mockup and expose only real,
functional behavior.

The desktop must stay unelevated. Physical devices, an elevated broker,
destination safety, restore, sandboxed preview, sessions, carving, and exFAT
do not yet have production implementations or acceptance evidence.

## Options considered

| Option | Advantages | Costs and risks | Complexity | Valid when |
| --- | --- | --- | --- | --- |
| Keep a labeled demonstration provider | Preserves broad UI iteration and screenshots | Conflicts with product direction; fabricated state can be mistaken for recovery evidence | Low | Only in a separate design artifact, never this application |
| Browser UI plus local service | Uses a browser while returning real data | Adds service lifecycle, authentication, ports, CORS, install, and update attack surface | High | A separately secured service becomes a product requirement |
| Implement the complete master-spec desktop now | Delivers the long-term workflow | Requires audited elevation, device identity, transactional restore, sandboxing, and much broader evidence | Very high | Those subsystems have accepted designs and tests |
| Narrow real-only Tauri image scanner | Reuses the read-only Rust path; no service or fabricated behavior; immediately useful | Smaller feature set; no candidate browser, progress percentage, sessions, or restore | Medium | The product accepts an honest image-only slice |

## Decision

Adopt a narrow Tauri 2 application that:

- runs unelevated with Windows `asInvoker`;
- exposes `select_and_scan_image(requestId)` with no path argument and opens a
  Rust-owned native dialog restricted to `.img`, `.dd`, `.raw`, or `.bin`;
- revalidates every path in Rust and opens it read-only through
  `um_cli::scan_image_path`;
- performs scanner work with `tauri::async_runtime::spawn_blocking`;
- returns schema-version-2 bounded summaries of real MBR/GPT, NTFS/FAT
  recognition, candidate counts, warnings, and required per-volume
  `scanStatus`;
- permits `partial` for recognized NTFS and FAT volumes, preserves each
  scanner's incompleteness warnings, and shows a localized coverage caveat
  rather than presenting the observed candidate count as exhaustive;
- persists only the effective `locale`, light/dark/system `theme`, and
  `reducedMotion` preferences. `reducedMotion` immediately sets the document
  state consumed by CSS to remove nonessential transitions and scan animation;
- exposes no individual candidate until a versioned IPC contract carries that
  candidate directly from the real Rust scanner;
- uses decimal strings across IPC for values that may exceed JavaScript's exact
  integer range;
- fails closed outside Tauri or when IPC is unavailable;
- never sends the selected filesystem path to the webview or across IPC;
- ships no demonstration provider, fabricated dataset, simulated progress,
  restore/session emulation, or runtime fallback;
- omits every feature without a real safe backend;
- maps typed volume read failures to sanitized `SOURCE_IO` and typed
  corrupt-structure failures to sanitized `SCAN_CORRUPT`, without raw parser
  detail or fallback to a different filesystem;
- moves focus to each newly selected view heading and supplies explicit
  forced-colors focus outlines;
- keeps the production CSP network-free except for Tauri IPC. Development adds
  only `ws://localhost:1420` to `connect-src` for the fixed Vite dev server,
  through `devCsp` and a dev-only served-HTML transform; that WebSocket origin
  is absent from production configuration and built HTML.

The detailed contract and acceptance gates are normative in
[SDD-017](../specs/017-real-only-image-desktop.md).

## Rationale

1. It is the smallest architecture that satisfies the request for real,
   functional application behavior.
2. It reuses the existing read-only scanner rather than creating a second
   parser or shelling out to a process.
3. It keeps privilege and write-capable functionality outside the current
   trust boundary.
4. It makes unsupported scope visible through omission, not fabricated success.
5. It gives later device, restore, and session work explicit revisit gates.

## Trade-offs accepted

- The application initially exposes a much smaller workflow than the master
  product vision.
- A scan has a real indeterminate pending state but no percentage, pause,
  resume, or cancellation until the engine exposes those capabilities.
- The web build is diagnostic only and fails closed; useful scanning requires
  the Tauri desktop runtime.
- Aggregate candidate counts are shown, but candidate detail and recovery are
  absent.
- Only effective language, light/dark/system theme, and reduced-motion
  preferences may remain. All three are persisted locally and have immediate
  observable behavior; no source or report data is stored with them.

These costs are preferable to shipping controls or evidence that do not
correspond to real backend behavior.

## Consequences

### Positive

- every production scan result is attributable to `um-cli`;
- the UI cannot silently switch to synthetic data;
- no new elevated, write, shell, network, or device capability is introduced;
- large offsets remain exact across IPC;
- bounded NTFS coverage is explicit instead of being implied by a candidate
  count;
- development live reload does not widen the production CSP;
- scope and acceptance evidence remain auditable.

### Negative

- design work for broader mock-backed screens is removed from the production
  application;
- long scans cannot be cooperatively canceled in this slice;
- a packaged Tauri smoke test is required in addition to browser component
  tests;
- later full-product work must add real subsystems rather than only restoring
  routes.

### Mitigations

- retain the real CLI as a fallback user tool, not as an application data
  fallback;
- execute scans on a blocking worker and guard duplicate/stale responses;
- bound report structure and payload, sanitize errors, and inspect production
  assets;
- use separate non-application design documents if future visual exploration
  is needed.

## Revisit triggers

Reconsider through a new SDD and ADR before adding raw devices/elevation,
candidate-level recovery, restore, preview, sessions, cooperative progress or
cancellation, segmented/VHD formats, exFAT/carving, remote/browser service
access, concurrent scans, or broader Tauri capabilities.
