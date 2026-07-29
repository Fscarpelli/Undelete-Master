# SDD-013 — Build, Release, and Signing

Status: PR quality workflow `Implemented-unverified`; release `Not started`

## Build inputs

Rust 1.88 or the repository-pinned compatible toolchain, Cargo lockfile, Node
runtime, pnpm, and `apps/desktop/pnpm-lock.yaml` as the single frontend
dependency lockfile. Frontend dependencies install with
`pnpm install --frozen-lockfile --ignore-scripts`; dependency updates require
review, and warnings or lint failures block integration.

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

An unsigned local development executable was deleted by Norton on the product
owner's Windows host on 2026-07-29. That observation is neither proof of
malware nor proof of a false positive. Before public distribution, the exact
release artifact must receive independent malware scanning, dependency/SBOM
review, Authenticode signing, signature verification, and vendor reputation or
false-positive submission when applicable. Documentation must not instruct
users to disable protection or bypass a warning.

If the detection remains after current definitions and independent review, use
the official
[Norton file/URL review process](https://support.norton.com/sp/pt/br/norton-antivirus/19.0/solutions/kb20090410134005EN)
for the exact hashed artifact. Uploading a binary is an external disclosure and
requires the owner's explicit approval; this repository workflow does not
submit it automatically.

## Integration policy

The remote `main` history must be preserved. Integrate through a normal
non-force branch/PR or merge; never replace or force-push `main`. Signing,
tagging, and release publication remain separate from the development-branch
push authorized for this increment.
