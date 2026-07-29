# ADR-0003 — Regular-image-only CLI

## Status

Accepted for the foundation-hardening increment

## Context

Existing read-only image, partition, NTFS, and FAT components need a real
end-to-end entry point. The privileged Windows broker and its threat model are
not implemented.

## Options

- Add physical-device CLI access now.
- Add a regular-image-only CLI.
- Keep libraries without an entry point.

## Decision

The current CLI accepts existing regular image files only and composes
`FileImageReader`, partition discovery, bounded regions, and supported metadata
scanners. It rejects device selectors and performs no extraction or restore.
Default JSON redacts full local paths.

## Consequences

The slice can prove useful image scanning without privilege expansion. It is
only partial evidence for FR-033/AC-027 and no evidence for physical disks,
broker security, restore, carving, or production exFAT.

