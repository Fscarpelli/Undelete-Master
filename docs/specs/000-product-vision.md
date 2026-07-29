# SDD-000 — Product Vision

Status: Normative  
Source of truth: `UNDELETE_MASTER_CODEX_MASTER_SPEC.md`

## Purpose

Undelete Master is a local-first Windows application for finding deleted-file
evidence, explaining recoverability, and restoring only explicitly selected
items. Preservation of the source, truthful results, security, correctness, and
testability take precedence over feature breadth.

## Product promises

The product may report metadata evidence, readable bytes, allocation conflicts,
structural validation, and bounded recovery estimates. It must never claim to
recreate overwritten, trimmed, securely erased, or unavailable encrypted data.

## Version 1 target

- Windows 10 22H2 and Windows 11 x64, with Windows 11 as the primary baseline.
- Tauri 2, React, TypeScript strict, and Rust stable.
- NTFS, FAT12/16/32, and exFAT only after independent fixtures and acceptance
  evidence exist.
- Local operation without account, cloud dependency, or telemetry.
- An unelevated desktop process and a future minimal read-only elevated broker.

## Current increment

The `codex/foundation-hardening` increment is not a production release. It is
limited to bounded parser hardening, regular image-file scanning, visual audit
assets for a synthetic desktop concept, documentation, and deterministic CI.
The frontend source is not present in the versioned revision. Physical disks,
Tauri integration, restore, carving, production exFAT, installers, and signing
remain excluded.

## Status vocabulary

| Status | Meaning |
| --- | --- |
| `Not started` | No implementation evidence exists. |
| `Partial` | Some design or behavior exists, but the requirement is incomplete. |
| `Implemented-unverified` | The intended implementation exists, but final required gates have not passed. |
| `Verified` | Implementation and linked acceptance evidence passed on the referenced revision. |

Status is requirement-specific. A passing component test never implies that the
whole product or a broader requirement is verified.
