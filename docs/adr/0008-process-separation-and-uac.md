# ADR-0008 — Process Separation and UAC

Master specification topic: 2 — process separation and UAC

Status: Accepted through [ADR-0023](0023-windows-read-only-broker-and-folder-scope.md)

## Context

Physical-source reads may require privileges that the desktop UI, parsers, and
recovered-content handlers must never inherit.

## Decision

Keep the Tauri desktop process unelevated. A minimal Windows broker may elevate
only for identity revalidation, read-only source opening, and bounded reads.
Display-only inventory may run query-only without elevation. Parsing,
preview, validation, restore, and UI rendering remain outside that privileged
process. The broker exposes no write, trim, format, lock, dismount, or arbitrary
device-control operation.

## Security and verification consequences

The broker must enforce caller identity, source allowlists, range bounds,
read-only handle flags, request limits, and audit-safe error codes. UAC,
least-privilege, spoofing, and source-write runtime tests are required before
acceptance. Delivery status and exact gates are tracked by SDD-018.
