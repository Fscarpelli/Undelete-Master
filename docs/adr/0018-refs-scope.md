# ADR-0018 — ReFS Scope

Master specification topic: 14 — ReFS scope

Status: Proposed

## Context

ReFS versions and metadata layouts vary, and unsupported recovery claims could
mislead users.

## Proposed decision

ReFS is outside version 1 recovery support unless a separately reviewed engine,
version matrix, deterministic fixtures, external corpora, and acceptance
evidence are delivered. Source detection may identify ReFS only to present an
explicit unsupported state; it must not fall through to a misleading scanner.

## Security and verification consequences

No heuristic metadata write or fabricated candidate is permitted. Detection,
unsupported-state, version, hostile-input, and corpus tests are prerequisites
for any future scope change.
