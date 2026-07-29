# Undelete Master

Undelete Master is a Windows-first, local data-recovery project focused on
source preservation and evidence-based results.

> **Development status:** real image analysis is implemented; file restore and
> physical-disk access are not. Do not use this pre-release as the only means of
> recovering important data.

[Português do Brasil](README.pt-BR.md)

## Implemented now

- A Rust `SourceReader` abstraction with no write operation.
- Read-only access to ordinary `.img`, `.dd`, `.raw`, and `.bin` files.
- Defensive MBR/GPT discovery plus partial NTFS and FAT12/16/32 metadata scans.
- A real, unelevated Tauri 2 desktop application linked directly to the Rust
  scanner.
- A Rust-owned native picker: the selected filesystem path never crosses IPC
  into the WebView.
- A bounded desktop report containing the actual source label/size, partition
  kind, volumes, metadata-candidate counts, and parser warnings.
- A headless `undelete-master scan-image` CLI with privacy-preserving JSON.
- Deterministic synthetic fixture tests, including a desktop parity test that
  verifies the fixture SHA-256 is unchanged after scanning.
- Fail-closed CI guards for destructive storage operations and reintroduction
  of fabricated desktop behavior.

The exact implementation and verification state is maintained in the
[traceability matrix](docs/traceability-matrix.md). A metadata-candidate count
does not guarantee that file content can be recovered.

## Deliberately absent

The application does not expose physical disks, raw-device handles, elevation,
restore, preview, persistent sessions, carving, exFAT, fake progress
percentages, pause/resume, or cancellation claims. These capabilities remain
outside the UI until a real, tested backend exists. See
[known limitations](docs/specs/015-known-limitations.md).

## Safety

- Never scan a real disk through test code.
- Never add source-write, trim, format, lock, dismount, or generic device
  commands to the scan path.
- Use only deterministic repository images, memory images, temporary regular
  files, or a separately governed allowlisted test VHD workflow.
- Never execute recovered content.

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
pnpm tauri build --no-bundle
```

Run the real native application with `pnpm desktop:dev`. Opening the Vite URL
alone intentionally shows a desktop-runtime-required state and never substitutes
browser data.

The locally generated executable is an unsigned development artifact. Norton
removed one build on 2026-07-29; the classification remains unresolved. Do not
redistribute that binary, disable protection permanently, or treat the alert as
a confirmed false positive without independent analysis. Signing and
reputation requirements are documented in
[SDD-013](docs/specs/013-build-release-and-signing.md).

CLI example:

```powershell
cargo run -p um-cli -- scan-image C:\images\evidence.img --pretty
```

The CLI and desktop accept ordinary local image files only. Do not pass a disk,
volume, network share, pipe, alternate data stream, symlink, or reparse source.

## Specification-driven documentation

- [Master specification](UNDELETE_MASTER_CODEX_MASTER_SPEC.md)
- [SDD-017 real-only desktop](docs/specs/017-real-only-image-desktop.md)
- [Functional requirements](docs/specs/001-functional-requirements.md)
- [Architecture](docs/specs/004-architecture.md)
- [Desktop data contract](docs/ui-data-contract.md)
- [Security and privacy](docs/specs/011-security-and-privacy.md)
- [Test plan](docs/specs/012-test-and-validation-plan.md)
- [ADRs](docs/adr/README.md)
- [Risk register](docs/risk-register.md)
- [Traceability](docs/traceability-matrix.md)

## Git integration

The remote `main` history is preserved. Development branches are integrated
without force-pushing or replacing `main`.
