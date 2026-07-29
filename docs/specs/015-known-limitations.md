# SDD-015 — Known Limitations

Status: Current as of 2026-07-29

## Product status

This repository is not a production data-recovery release. Do not use current
builds as the only means of recovering important data.

## Current implementation limitations

- No physical-disk/volume enumeration, UAC broker, hot-plug identity, BitLocker
  workflow, or raw-device access.
- No Tauri shell or real desktop provider; the React app is a labeled
  deterministic demonstration.
- No persisted SQLite sessions, checkpoint resume, import/export, or reports.
- No restore engine, destination containment, transactional writes, or manifests.
- No carving, validators, sandboxed preview worker, or repair derivatives.
- No production exFAT support; incomplete exFAT files are excluded.
- NTFS and FAT cover limited synthetic cases and are not a complete production
  compatibility matrix.
- No NTFS LZNT1 decompression, complete attribute-list handling, full ADS/EFS
  workflow, or broad journal/recycle-bin enrichment.
- No fuzz, external forensic corpus, million-row, performance, or real Tauri E2E
  completion evidence.
- No installer, portable package, SBOM, signing, clean-machine, upgrade, or
  uninstall evidence.

## Recovery truth

Overwritten, trimmed, securely erased, unavailable encrypted, or physically
unreadable bytes cannot be recreated by software. A filename or metadata record
does not prove intact content.

