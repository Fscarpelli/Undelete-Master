# ADR-0007 — Demonstration Provider Trust State

## Status

Accepted for the foundation-hardening increment

## Context

The React frontend currently uses deterministic synthetic sources, candidates,
progress, and restore results. Without an explicit runtime state, users and
screenshots can mistake a demonstration for verified engine behavior.

## Options

- Remove the demonstration until Tauri exists.
- Keep the mock and rely on README disclosure.
- Expose provider runtime metadata and a persistent in-app disclosure.

## Decision

Every provider exposes a mode and whether read-only source state was verified.
The mock reports `demo` and `false`, displays persistent localized synthetic-data
disclosure, and never causes the verified read-only seal to appear. Missing
sessions produce empty states rather than synthetic fallback IDs.

## Consequences

UI development remains possible without a false production claim. Future Tauri
providers must fail unavailable when runtime verification is absent; they cannot
silently fall back to demo data.

