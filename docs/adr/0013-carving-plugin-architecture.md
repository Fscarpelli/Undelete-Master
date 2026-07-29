# ADR-0013 — Carving Plugin Architecture

Master specification topic: 8 — carving plugin architecture

Status: Proposed

## Context

Signature carving can create false positives and unbounded work unless formats
declare strict validators and resource limits.

## Proposed decision

Use a build-time registry of carving plugins. Each plugin declares signatures,
minimum/maximum size, alignment policy, bounded footer/structure validation,
confidence factors, and supported preview/repair handoffs. Plugins consume
read-only regions and emit evidence with physical provenance.

## Security and verification consequences

No plugin executes recovered bytes, loads external code, or performs network or
source writes. Overlap arbitration, malformed structures, decompression bombs,
timeouts, deterministic outputs, and external-corpus false-positive tests are
required. Carving is not implemented in the current increment.
