# ADR-0002 — Read-only Source Invariant

## Status

Accepted

## Context

Any write, trim, format, lock/dismount, or metadata change can destroy evidence
the product is trying to recover.

## Options

- One read/write device abstraction with runtime flags.
- Separate read-only source and destination-write abstractions.
- Snapshot/copy all sources before analysis.

## Decision

`SourceReader` exposes only identity, length, sector layout, and bounded reads.
Source mutation is absent by construction. Future Windows raw access belongs in
a minimal audited broker with allowlisted read/query commands. Restore receives
logical candidate streams and a destination, never a mutable source handle.

## Consequences

Some workflows require a separate working/destination disk and explicit image
creation. Architecture and runtime tests must prove absence of source writes;
UI seals require verified runtime state, not configuration or mock data.

