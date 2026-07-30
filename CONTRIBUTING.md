# Contributing

Read `AGENTS.md`, the master specification, the relevant SDD document, and the
traceability matrix before changing behavior.

## Safety rules

- Never open a scan source with write access.
- Never run a test against a real physical disk or ordinary mounted volume.
- Never invoke format, trim, delete, lock, dismount, raw write, or generic
  device-control commands.
- Use deterministic fixtures, in-memory images, and temporary regular files.
- Do not execute recovered files or parse them in a privileged process.
- Keep all parser arithmetic checked and all reads within the supplied buffer
  and region.

If a proposed change needs physical-device/VHD testing, stop and design the
isolated allowlisted harness and ADR first. Ordinary PR CI is not that harness.

## Change process

1. Create a development branch; do not rewrite remote `main`.
2. Add a regression test before a bug fix.
3. Update the relevant spec and `docs/traceability-matrix.md`.
4. Add an ADR for an architecture/security deviation instead of silently
   weakening the master spec.
5. Use only the four status values defined by SDD-000.
6. Do not claim unsupported formats, whole-physical-disk scanning, restore,
   carving, installer, or release behavior.
7. Treat broker protocol or allowlist changes as security-sensitive behavior:
   add protocol regression tests, update SDD-018/ADR-0023, and keep the desktop
   manifest unelevated.

## Required checks

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

From `apps/desktop`:

```powershell
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Use the committed `pnpm-lock.yaml` as the frontend dependency lockfile; do not
generate or commit a second lockfile.

PRs must also pass documentation validation in
`.github/workflows/quality.yml`. Do not disable, skip, or convert a critical test
to a mock merely to make a gate green.

## Pull request evidence

Describe changed requirements, commands and results, deterministic fixtures,
security impact, limitations, and any unverified gate. A screenshot is visual
evidence only; it cannot prove engine wiring or secure runtime behavior.
