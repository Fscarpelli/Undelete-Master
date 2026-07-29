# ADR-0017 — Installer and Update Strategy

Master specification topic: 13 — installer and update strategy

Status: Proposed

## Context

Installation, elevation, signing, and updating can weaken the desktop/broker
boundary or misrepresent unsigned artifacts as production.

## Proposed decision

Produce separately identified EXE, MSI, and portable artifacts from reproducible
release inputs. Install the unelevated desktop and minimal broker components
with least privilege. Updates are disabled until signed metadata, rollback,
channel separation, and offline behavior are designed and tested.

## Security and verification consequences

Authenticode and update signatures are verified when configured; unsigned
artifacts are labeled development/RC only. Install/uninstall, upgrade/downgrade,
rollback, tampered package, offline, privilege, and clean-machine tests are
required. No installer or updater is currently delivered.
