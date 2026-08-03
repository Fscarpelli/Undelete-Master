# Undelete Master documentation

This directory is the documentation portal for the executable behavior,
architecture, safety model and verification evidence of Undelete Master.

## Start here

- [Feature catalog](FEATURES.md) - implemented behavior, user workflow and
  current product boundaries.
- [Screenshot gallery](screenshots/README.md) - visual guide and acceptance
  captures.
- [Traceability matrix](traceability-matrix.md) - requirement-to-code and
  requirement-to-test mapping.
- [Known limitations](specs/015-known-limitations.md) - unsupported or
  unverified behavior stated without marketing claims.
- [Risk register](risk-register.md) - active technical and operational risks.
- [Security policy](../SECURITY.md) and [threat model](threat-model/THREAT_MODEL.md).

## Specification-driven development

The canonical product baseline is
[UNDELETE_MASTER_CODEX_MASTER_SPEC.md](../UNDELETE_MASTER_CODEX_MASTER_SPEC.md).
Increment specifications record the implemented slices:

- [SDD-016 - foundation and safe image CLI](specs/016-foundation-hardening-and-safe-image-cli.md)
- [SDD-017 - real-only desktop](specs/017-real-only-image-desktop.md)
- [SDD-018 - connected Windows volumes and folder scope](specs/018-windows-volume-and-folder-scan.md)
- [SDD-019 - NTFS coverage and bounded JPEG carving](specs/019-ntfs-coverage-and-jpeg-deep-scan.md)
- [SDD-020 - actionable results and transactional restore](specs/020-actionable-results-and-transactional-restore.md)

Architecture decisions are indexed in [docs/adr/README.md](adr/README.md).
Evidence produced by governed validation runs is retained in
[docs/evidence](evidence/).

## Documentation truth rules

- Implemented, locally verified and packaged-real-device verified are distinct
  states.
- A metadata candidate is not proof that its content remains readable.
- Deep carving currently means contiguous JPEG carving in NTFS regions proven
  free by a validated `$Bitmap`; it is not all-format forensic carving.
- The scan source is always read-only. Recovery writes are permitted only to a
  separately authorized NTFS folder on a proven different physical disk.
- Screenshots are presentation or acceptance evidence, not proof of recovery
  correctness.
