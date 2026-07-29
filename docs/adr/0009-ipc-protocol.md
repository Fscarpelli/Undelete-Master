# ADR-0009 — Broker IPC Protocol

Master specification topic: 4 — IPC protocol

Status: Proposed

## Context

A future read-only broker needs a narrow protocol that cannot become a generic
privileged file or device service.

## Proposed decision

Use a versioned, length-prefixed local protocol with an explicit message
allowlist for source inventory, open-read-only, bounded read, close, and health
operations. Bind each session to the authenticated desktop process and a
broker-issued nonce. Reject unknown versions, message kinds, duplicate/replayed
requests, oversized frames, invalid UTF-8, and out-of-range reads.

## Security and verification consequences

The protocol carries stable source identifiers and offsets, never destination
paths or recovered content. Fuzzing, replay/spoofing, frame-limit, disconnect,
and compatibility tests are required. IPC and the broker remain unimplemented.
