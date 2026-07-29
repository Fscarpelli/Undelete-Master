# SDD-013 — Build, Release, and Signing

Status: PR quality workflow `Implemented-unverified`; release `Not started`

## Build inputs

Rust 1.88 or the repository-pinned compatible toolchain, Cargo lockfile, Node
runtime, and `package-lock.json` as the single frontend dependency lockfile.
Frontend dependencies install with `npm ci`; the required `pnpm` commands invoke
package scripts only. Dependency updates require review; warnings and lint
failures block integration.

## Pull requests

`.github/workflows/quality.yml` runs only repository code against synthetic
fixtures and temporary regular files. It does not attach disks/VHDs, request
elevation, enable device-test features, or consume signing secrets.

## Release requirements

A production/RC pipeline must produce installer EXE, MSI, portable ZIP,
SHA-256 sums, SBOM, third-party notices, release notes, known limitations, and a
completion report. Authenticode is verified when a certificate is configured;
otherwise artifacts are explicitly unsigned development/RC builds and are not
called production-ready.

## Integration policy

The remote `main` history must be preserved. Integrate through a normal
non-force branch/PR or merge; never replace or force-push `main`. This task does
not publish, tag, sign, or move any remote reference.
