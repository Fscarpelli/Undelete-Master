# SDD-015 — Known Limitations

Status: Current as of 2026-07-29

## Product status

This repository is not a production data-recovery release. The real desktop
analyzes ordinary image files but does not restore data. Do not use it as the
only means of recovering important information.

## Current limitations

- No physical-disk/volume enumeration, UAC broker, hot-plug identity, BitLocker
  workflow or raw-device access.
- The desktop returns only a real aggregate image report. Individual candidate
  rows remain absent until the Rust scanner exposes a bounded versioned record
  contract.
- No restore engine, destination containment, transactional write, manifest or
  Explorer launch.
- No persisted SQLite sessions, checkpoint resume, import or export.
- No carving, validators, sandboxed preview worker or repair derivatives.
- No production exFAT support; the incomplete local exFAT work is excluded from
  the workspace and publication claims.
- NTFS and FAT support covers deterministic fixtures and selected hostile
  structures, not a complete compatibility matrix.
- Schema-version-2 reports expose `complete`, `partial`, or `unrecognized` for
  every volume. Recognized NTFS and FAT scans may be `partial`. Any partial
  candidate count is the number observed within bounded coverage, not an
  exhaustive total; the desktop shows a coverage caveat.
- NTFS MFT enumeration is intentionally incomplete after its configured
  byte/record cap, a shortened trusted physical prefix, skipped
  unreadable/torn/corrupt records, a nonblank record without a `FILE`
  signature, malformed attributes in an ordinary/extension record, or
  extension-record merge failures. Any `$ATTRIBUTE_LIST`, even a well-formed
  resident value, remains partial because complete resolution of every
  reference and candidate-defining attribute is not yet proven. A
  record-unaligned `$MFT` logical size, one spanning fewer than the 16 reserved
  record slots, or malformed record-0 attributes rejects the NTFS scan as
  corrupt instead of returning a partial result.
- The NTFS allocation `$Bitmap` read and backing allocation stop at 64 MiB.
  Allocation bits beyond that retained prefix are unknown, which can reduce
  allocation certainty for later clusters.
- FAT permits up to four total declared copies. Enumeration is partial when any
  declared secondary copy disagrees with the authoritative first copy or is
  unreadable; a reachable directory has a broken/cyclic/unusable chain or no
  usable start cluster; a descended directory read fails; or traversal reaches
  10,000 directories, 256 levels, or 8 MiB for one directory stream. The
  8 MiB-derived cluster-count limit bounds the chain vector before directory
  cluster reads. A declared FAT table above 64 MiB is rejected as corrupt
  before allocation rather than returned as partial.
- GPT recovery evaluates both canonical copies and intentionally rejects
  relocated backup headers, conflicting valid headers, nonreciprocal or
  inconsistent primary/backup metadata, and entry arrays outside reserved GPT
  metadata space. This strictness can reject damaged/noncanonical media rather
  than infer a partition map; RAW/carving fallback remains absent.
- No NTFS LZNT1 decompression, full attribute-list/ADS/EFS workflow or broad
  journal/recycle-bin enrichment.
- Scans have a truthful indeterminate pending state, but no phase percentage,
  ETA, pause, resume or cooperative cancellation.
- Mapped remote drives and symlink/reparse ancestors are rejected, and the
  final Windows open validates the opened handle without following a final
  reparse point. However, picker selection, `GetDriveTypeW` classification,
  ancestor checks, and open are separate pathname operations. The current
  single command minimizes the interval but is not race-free and does not
  provide forensic chain-of-custody identity.
- Unicode bidi formatting controls are removed before IPC and the source label
  is directionally isolated in the UI. Other visually confusable Unicode
  characters remain possible; displayed labels are not authoritative identity.
- No fuzz campaign, external forensic corpus, million-row, performance or
  native assistive-technology completion evidence.
- Focused GPT, NTFS, FAT, CLI, desktop status/error, focus and CSP regressions
  do not replace the required full Rust/frontend gates or native acceptance.
- Development permits only `ws://localhost:1420` for Vite live reload. It is
  injected into served development HTML by a dev-only transform so it agrees
  with Tauri `devCsp`, and is absent from production configuration and built
  HTML. This is a development exception, not a production network capability.
- No installer, portable package, SBOM, signing, clean-machine, upgrade or
  uninstall evidence.
- Frozen-code Rust/frontend gates pass. Packaged native scan, production
  artifact inspection, native visual acceptance, external corpora/real-media
  compatibility, and remote CI are still pending.
- The current executable is an unsigned development artifact. Norton deleted
  one local build on 2026-07-29; that classification is unresolved and the
  artifact is not approved for redistribution. Do not disable security
  software permanently or treat the alert as a confirmed false positive.

## Security-review limitation

The [sealed 2026-07-29 Codex Security snapshot](../evidence/real-only-desktop-2026-07-29.md)
contains `34/34` unique review receipts, three technically valid candidates
whose final policy decisions were `ignore`, and zero reportable findings. That
is a bounded reportability outcome, not proof that vulnerabilities are absent.
It does not attest later changes, reproduce the concurrent TOCTOU candidates,
replace final CI, or resolve Norton's classification of the unsigned build.

The production application contains no synthetic provider, fabricated result,
simulated progress, restore/session emulation or browser data fallback.

## Recovery truth

Overwritten, trimmed, securely erased, unavailable encrypted or physically
unreadable bytes cannot be recreated by software. A filename, metadata record
or candidate count does not prove intact recoverable content.
