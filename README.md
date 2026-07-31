# Undelete Master

Undelete Master is a Windows-first, local data-recovery project focused on
source preservation and evidence-based results.

> **Development status:** the desktop can discover connected local storage,
> scan a selected mounted volume, and optionally restrict NTFS results to a
> selected folder. An explicit whole-volume NTFS mode also performs bounded
> contiguous-JPEG carving over bitmap-proven free space. The actionable results
> workspace and transactional restore workflow are implemented and locally
> fixture-verified, but packaged real-device acceptance is still unverified.
> Do not use this pre-release as the only means of recovering important data.

[Português do Brasil](README.pt-BR.md)

## Implemented now

- A Rust `SourceReader` abstraction with no write operation.
- Real, unelevated Windows discovery of supported mounted volumes. The initial
  cards are logical storage groups; physical extents are deliberately resolved
  only inside the elevated broker after the user starts a scan.
- A real, unelevated Tauri 2 desktop application with no image-file picker,
  mock provider, fabricated result, or fake progress.
- Selection of a supported mounted volume and, on NTFS, an optional folder.
  The native path remains in Rust and never crosses IPC into the WebView.
- A short-lived, elevated, read-only Windows broker. It accepts only opaque
  volume identities and bounded reads, revalidates source identity, and has no
  write, trim, format, lock, dismount, restore, or generic device-command API.
- Defensive NTFS and FAT12/16/32 metadata scanning with bounded, paginated
  candidate results. NTFS enumeration reports quantitative MFT coverage and no
  longer stops at the former 64 MiB prefix. Folder-scoped NTFS results include
  only candidates whose ancestry can be proven; unknown ancestry is counted
  separately.
- An explicit deep-JPEG mode for a whole NTFS volume. It submits only ranges
  that a validated `$Bitmap` snapshot proves free, uses bounded incremental
  structural validation, and records physical range, SHA-256, validator
  version, coverage, and work-limit evidence. It never silently expands into
  allocated or unknown space.
- A backend-owned actionable results workspace with explicit search, dynamic
  extension facets, evidence filters, stable sorting, bounded cursor pages and
  native selection that survives page, query, filter and sort changes within
  the current process.
- A native transactional restore workflow for explicitly selected candidates.
  It requires an authorized NTFS folder on one proven different physical disk,
  preserves the recovered tree, renames collisions without replacing existing
  files, streams bounded reads, and publishes a versioned JSON manifest.
  Metadata-backed restoration does not depend on the filename extension when a
  usable content plan exists; that does not make every candidate recoverable or
  expand deep carving beyond its implemented JPEG plugin.
- Explicit best-effort consent and exact zero-filled-range sidecars for partial
  output. Item/byte progress, cancellation, terminal counts and manifest
  identity come from the native restore job. The final action opens only the
  completed job's destination folder and never executes a recovered file.
- A separate headless `undelete-master scan-image` CLI for ordinary local image
  files with privacy-preserving JSON.
- Deterministic synthetic-fixture and protocol tests. No automated test scans an
  ordinary mounted volume or physical disk.
- Fail-closed CI guards for destructive storage operations and for
  reintroduction of image selection, fabricated desktop behavior, or an
  over-broad command surface.

The exact implementation and verification state is maintained in the
[traceability matrix](docs/traceability-matrix.md). A metadata candidate is not
a guarantee that its content can be recovered.

## Deliberately absent

The desktop does not scan a whole physical disk, unmounted partition,
multi-disk volume, network share, optical drive, or RAM disk. Folder scope is
currently NTFS-only. Carving is limited to contiguous JPEGs in proven-free NTFS
space; other formats, fragmented reconstruction, damaged-filesystem RAW
scanning, and folder carving are not implemented. Restore cannot target the
source disk, original path, or a non-NTFS/unproven destination and does not
preserve ACLs, EFS, alternate data streams or transparent compression. Preview,
content execution, persistent sessions, restart resume, alternate restore
layouts, scan progress/ETA/cancellation, and a signed installer are also
absent. Restore jobs do expose native progress and cooperative cancellation;
those controls do not apply to a metadata or deep scan. See
[known limitations](docs/specs/015-known-limitations.md).

## Safety

- Never scan a real disk or ordinary mounted volume through test code.
- Never add source-write, trim, format, lock, dismount, or generic device
  commands to the scan path.
- Use deterministic repository images, memory images, temporary regular files,
  or a separately governed and explicitly allowlisted disposable test VHD.
- Never execute recovered content.
- Keep the desktop unelevated. Elevation is limited to the read-only broker
  launched when the user explicitly starts a volume scan.
- Restore only to the retained destination authority on a proven different
  physical disk; destination writes remain in the unelevated restore component,
  never in the source broker.

See [SECURITY.md](SECURITY.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## Development

Rust quality gates:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Desktop gates:

```powershell
Set-Location apps/desktop
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Build and run the native application:

```powershell
Set-Location apps/desktop
pnpm desktop:dev
# or build the release desktop and its sibling broker
pnpm desktop:build
```

Opening the Vite URL alone intentionally shows a desktop-runtime-required state
and never substitutes browser data.

Locally generated executables are unsigned development artifacts. Norton
removed one build on 2026-07-29; the classification remains unresolved. Do not
redistribute that binary, disable protection permanently, or call the alert a
false positive without independent analysis. Signing and reputation
requirements are documented in
[SDD-013](docs/specs/013-build-release-and-signing.md).

CLI image example:

```powershell
cargo run -p um-cli -- scan-image C:\images\evidence.img --pretty
```

The CLI accepts ordinary local image files only. The desktop uses the connected
mounted-volume workflow described above and does not expose image selection.

## Specification-driven documentation

- [Master specification](UNDELETE_MASTER_CODEX_MASTER_SPEC.md)
- [SDD-018 Windows volume and folder scan](docs/specs/018-windows-volume-and-folder-scan.md)
- [SDD-019 NTFS coverage and bounded JPEG deep scan](docs/specs/019-ntfs-coverage-and-jpeg-deep-scan.md)
- [SDD-020 actionable results and transactional restore](docs/specs/020-actionable-results-and-transactional-restore.md)
- [ADR-0023 read-only broker and folder scope](docs/adr/0023-windows-read-only-broker-and-folder-scope.md)
- [ADR-0024 streaming MFT and bounded content carving](docs/adr/0024-streaming-mft-and-bounded-content-carving.md)
- [ADR-0025 native result query and selection authority](docs/adr/0025-native-result-query-and-selection-authority.md)
- [ADR-0026 bounded content plans and partial recovery](docs/adr/0026-bounded-content-plan-and-partial-recovery.md)
- [ADR-0027 destination capability and disk separation](docs/adr/0027-destination-capability-and-disk-separation.md)
- [ADR-0028 restore plan, job and manifest lifecycle](docs/adr/0028-restore-plan-job-and-manifest-lifecycle.md)
- [Functional requirements](docs/specs/001-functional-requirements.md)
- [Architecture](docs/specs/004-architecture.md)
- [Security and privacy](docs/specs/011-security-and-privacy.md)
- [Test plan](docs/specs/012-test-and-validation-plan.md)
- [ADRs](docs/adr/README.md)
- [Risk register](docs/risk-register.md)
- [Traceability](docs/traceability-matrix.md)

## Git integration

The remote `main` history is preserved. Development branches are integrated
without force-pushing or replacing `main`.
