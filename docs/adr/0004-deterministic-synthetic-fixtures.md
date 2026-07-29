# ADR-0004 — Deterministic Synthetic Fixtures

## Status

Accepted

## Context

Parser and recovery tests must be safe, reproducible, and able to prove exact
bytes without using real disks.

## Options

- Hand-maintained binary images.
- Real disposable disks/VHDs in ordinary tests.
- Programmatically built deterministic images with truth manifests.

## Decision

Use `crates/fixture-builder` to create bounded in-memory or temporary regular
images. Expected candidates and content are recorded in truth manifests and
verified by SHA-256. External corpora are a separate extended gate.

## Consequences

Pull requests remain safe and deterministic. Synthetic coverage is not a
production compatibility claim; independent corpora, fuzzing, and isolated VHD
tests remain required before release.

