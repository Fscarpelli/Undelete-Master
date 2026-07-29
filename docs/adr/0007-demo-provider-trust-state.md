# ADR-0007 — Demonstration Provider Trust State

## Status

Superseded by [ADR-0021](0021-real-only-image-desktop.md) on 2026-07-29

## Context

A synthetic React demonstration was observed during design work, but its source
is not present in the versioned revision. Without an explicit runtime state,
future users and screenshots could mistake a demonstration for verified engine
behavior.

## Options

- Remove the demonstration until Tauri exists.
- Keep the mock and rely on README disclosure.
- Expose provider runtime metadata and a persistent in-app disclosure.

## Decision

Every future provider exposes a mode and whether read-only source state was
verified. The mock must report `demo` and `false`, display persistent localized
synthetic-data disclosure, and never cause the verified read-only seal to
appear. Missing sessions produce empty states rather than synthetic fallback
IDs.

## Consequences

UI development remains possible without a false production claim after the
frontend is versioned. Future Tauri providers must fail unavailable when runtime
verification is absent; they cannot silently fall back to demo data.

## Supersession

This proposal was never accepted for product delivery. The product owner now
requires a real-only application. ADR-0021 therefore prohibits a demonstration
provider, synthetic runtime data, simulated progress, restore/session
emulation, and every fallback dataset in production. Browser-only execution
fails closed, and unsupported workflows remain absent until a real backend
exists.
