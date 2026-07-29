# ADR-0010 — SQLite Session Schema

Master specification topic: 5 — SQLite and session schema

Status: Proposed

## Context

Large scans require durable, queryable checkpoints without loading millions of
candidates into the UI.

## Proposed decision

Store versioned sessions in SQLite on the approved working disk. Normalize
source identity, scan configuration, checkpoints, candidates, extents,
evidence, validation, selection, restore journal, and schema migrations.
Transactions make checkpoint and resume boundaries atomic. Imported sessions
are untrusted and validated before opening.

## Security and verification consequences

The database must never be created on the scan source. Queries are parameterized
and bounded; migrations are forward tested and rollback-safe. Corruption,
schema migration, substituted-source, traversal, scale, and interrupted-resume
tests are required. No session database exists in the current increment.
