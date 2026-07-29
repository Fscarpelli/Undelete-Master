# ADR-0008 — Process Separation and UAC

Master specification topic: 2 — process separation and UAC

Status: Proposed

## Context

Physical-source reads may require privileges that the desktop UI, parsers, and
recovered-content handlers must never inherit.

## Proposed decision

Keep the Tauri desktop process unelevated. A future minimal Windows broker may
elevate only for read-only source enumeration and bounded reads. Parsing,
preview, validation, restore, and UI rendering remain outside that privileged
process. The broker exposes no write, trim, format, lock, dismount, or arbitrary
device-control operation.

## Security and verification consequences

The broker must enforce caller identity, source allowlists, range bounds,
read-only handle flags, request limits, and audit-safe error codes. UAC,
least-privilege, spoofing, and source-write runtime tests are required before
acceptance. No broker is delivered in the current increment.
