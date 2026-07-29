# ADR-0006 — Explainable Recoverability Score

## Status

Accepted

## Context

A filename or intact metadata record does not prove that content is available.
Overlapping, missing, conflicting, zeroed, inferred, or unvalidated ranges must
not produce misleading confidence.

## Options

- A single opaque percentage.
- Binary recoverable/not-recoverable labels.
- Separate explainable content, metadata, and structural assessments.

## Decision

Store content recoverability (bounded 0–99 without a trusted original hash),
metadata confidence, and structural validation separately. Content scores expose
factors and mandatory caps. Logical coverage uses normalized range unions;
names/paths do not increase content availability.

## Consequences

The UI needs “why this score” evidence and must avoid “100% recoverable.”
Scoring changes require regression/property tests and traceability updates.

