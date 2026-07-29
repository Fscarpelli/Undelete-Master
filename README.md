# Undelete Master

Undelete Master is a Windows-first, local data-recovery project focused on
source preservation and honest, evidence-based recoverability.

> **Development status:** foundation hardening. This repository is not a
> production recovery release. Do not use it as the only means of recovering
> important data.

[Português do Brasil](README.pt-BR.md)

## What exists

- A Rust `SourceReader` abstraction with no write operation.
- Read-only regular image-file access and bounded source regions.
- MBR/GPT, partial NTFS, and partial FAT12/16/32 parsing.
- Deterministic synthetic fixtures with SHA-256 recovery assertions.
- A React/Vite desktop demonstration using explicitly synthetic data.
- A foundation increment for a regular-image-only CLI, parser hardening,
  truthful demo state, SDD, and deterministic CI.

The exact implementation and verification state is maintained in the
[traceability matrix](docs/traceability-matrix.md). Work in progress is never
equivalent to `Verified`.

## Not delivered

Physical-disk access, an elevated broker, a Tauri production shell, real restore,
carving, sandboxed preview/validation, persistent sessions, production exFAT,
installers, signing, and a production release are not delivered. See
[known limitations](docs/specs/015-known-limitations.md).

## Safety

- Never scan a real disk through test code.
- Never add a source-write, trim, format, lock, dismount, or generic device
  command.
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

Frontend gates from `apps/desktop`:

```powershell
npm ci
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

`package-lock.json` is the canonical frontend lockfile; pnpm is used here only
to invoke the required package scripts.

The image-only CLI command is part of the current increment and must not be
treated as available or verified until its matrix row links passing Task 8
evidence.

## Documentation

- [Product vision](docs/specs/000-product-vision.md)
- [Functional requirements](docs/specs/001-functional-requirements.md)
- [Architecture](docs/specs/004-architecture.md)
- [Security and privacy](docs/specs/011-security-and-privacy.md)
- [Test plan](docs/specs/012-test-and-validation-plan.md)
- [ADRs](docs/adr/README.md)
- [Risk register](docs/risk-register.md)
- [Traceability](docs/traceability-matrix.md)

## Git integration

The remote `main` history is authoritative and must be preserved. Development
branches are integrated without force-pushing or replacing `main`. This
documentation task does not publish, tag, sign, or move remote references.
