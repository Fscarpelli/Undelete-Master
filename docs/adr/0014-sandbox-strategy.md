# ADR-0014 — Validation and Preview Sandbox

Master specification topic: 9 — sandbox strategy

Status: Proposed

## Context

Recovered files are attacker-controlled and may exploit parsers, codecs, or
document handlers.

## Proposed decision

Run validation and preview in separate unelevated, network-disabled workers
with restricted tokens, explicit format allowlists, bounded input copies,
memory/CPU/time/output quotas, and disposable working directories. Never shell
open, macro-enable, import, or execute recovered content.

## Security and verification consequences

The UI receives only bounded sanitized render results and structured errors.
Worker crash, timeout, escape, malformed IPC, bomb, cleanup, and unsupported
format tests are required. Until this boundary exists, production preview and
validation remain disabled.
