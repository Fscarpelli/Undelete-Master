# ADR-0016 — Dependency and License Policy

Master specification topic: 12 — dependency and license policy

Status: Proposed

## Context

Build dependencies can introduce vulnerabilities, incompatible licenses,
network behavior, native code, or unreviewed execution in CI.

## Proposed decision

Commit one lockfile per ecosystem, pin CI actions to reviewed commit SHAs, and
review direct dependency purpose, provenance, license, maintenance, native
code, and network behavior. Release gates include vulnerability, license,
provenance, and SBOM checks with documented exceptions and expiry.

## Security and verification consequences

Pull requests receive read-only tokens and no signing secrets. Dependency
updates are isolated and reproducible. Lockfile drift, denied license,
vulnerability exception, offline build, and SBOM completeness tests are
required before this ADR can be accepted.
