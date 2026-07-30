# Recovery hardening same-revision evidence — 2026-07-30

Status: Partial local evidence. Required Rust/frontend/static gates passed on
the frozen recovery-hardening code snapshot; external-corpus, long-running
performance, native-package, signing, endpoint-security and remote-CI evidence
remain open.

## Snapshot identity and boundary

- Repository base at evidence reconciliation:
  `aab396af66dab483bf7ddc76f1ac0d912ce11295`.
- The tested recovery increment was still an uncommitted working tree when the
  coordinator ran the final commands. The ensuing commit will assign its Git
  identity; this record therefore does not independently satisfy an
  exact-commit `Verified` gate.
- The coordinator ran the commands after the final MFT nested-evidence budget
  change and before documentation-only reconciliation.
- Tests used deterministic/in-memory sources and disposable ordinary
  files/directories. No ordinary gate opened or modified `C:`, another mounted
  volume, `PhysicalDriveN`, or a real VHD.
- No result here proves arbitrary real-volume compatibility, recoverability of
  overwritten/TRIM-discarded bytes, or production readiness.

## Final same-revision local gates

| Command | Result | Evidence boundary |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS; exit 0 | Workspace format gate. |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS; exit 0 | Workspace lint gate with warnings denied. |
| `cargo test --workspace` | PASS; exit 0 | Included 33 NTFS unit tests, 12 NTFS internal-bounds tests, 9 NTFS recovery tests, and 14 desktop Rust tests; no real-device acceptance implied. |
| `pnpm lint` in `apps/desktop` | PASS; exit 0 | Frontend lint gate. |
| `pnpm typecheck` in `apps/desktop` | PASS; exit 0 | Strict TypeScript gate. |
| `pnpm test` in `apps/desktop` | PASS; exit 0; 41 tests | Frontend unit/integration gate; not native visual acceptance. |
| `pnpm build` in `apps/desktop` | PASS; exit 0 | Production frontend build; not a signed or packaged Tauri release. |
| `python -B .github/scripts/validate_ci_safety.py --root .` | PASS; 1 workflow and 98 enumerated surfaces | Static defense in depth; not a remote GitHub Actions result. |
| `python -B .github/scripts/validate_real_only_desktop.py` | PASS; 94 files inspected | Production-boundary inventory; not installer, binary-import, or native visual evidence. |
| `.github/scripts/tests` unittest discovery | PASS; 71 tests | Validator regression suite. |

## Post-evidence documentation reconciliation

These checks ran after this evidence file and its links were added:

| Command | Result | Evidence boundary |
| --- | --- | --- |
| `python -B .github/scripts/validate_docs.py` | PASS; 58/58 exact FR catalog/matrix, 8 NFR, 11 foundation, 12 real-only and 13 connected-volume requirements, 17 ADR topics, 28 formal justifications | Documentation structure, status, test-reference and local-link validation. |
| `git diff --check -- docs` | PASS; exit 0 | No whitespace errors; only expected LF-to-CRLF working-copy warnings. |

## Recovery behavior covered by the passing gates

- MFT enumeration no longer stops at the former 64 MiB prefix and reads in
  bounded 1 MiB batches.
- Retention has four independent 100,000-item ceilings for deleted entries,
  directories, extension references, and extension streams.
- Nested retained evidence has checked aggregate ceilings of 400,000 names,
  200,000 streams, and 1,000,000 run elements. An over-budget base entry or
  extension merge is rejected atomically, marks coverage partial, and emits one
  bounded aggregate warning.
- The JPEG carver validates incrementally with fixed-size reads and explicit
  region, candidate, signature-attempt, aggregate-validation and output
  budgets.
- Product deep mode admits only whole-volume NTFS ranges proven free by the
  hardened `$Bitmap` authority. Folder, FAT, unknown and untrusted allocation
  scopes fail closed.
- Summary schema 3 and candidate-page schema 2 carry bounded coverage,
  method, exact-range SHA-256 and the `jpeg-structural-v1` validator without
  sending recovered bytes to JavaScript.
- Exact-range evidence merging preserves ambiguity instead of assigning a
  carved result to an arbitrary metadata record.

## Remaining claim boundary

This evidence does not close:

- external forensic corpora, fuzzing, hostile-volume memory behavior, or
  long-running throughput evidence;
- fragmented JPEG recovery, formats beyond JPEG, slack/RAW damaged-filesystem
  recovery, repair, extraction, restore, or preview;
- real progress, cooperative cancellation, checkpoint/resume, or a deep-mode
  `scan-image` process option;
- live mutable-system-volume consistency or a real `C:` recovery acceptance
  test;
- packaged native binaries, administrator-protected installation,
  Authenticode, clean-machine testing, SBOM/license release gates, Norton
  disposition, or remote GitHub Actions.

Accordingly, the recovery slice remains `Implemented-unverified` or `Partial`
as mapped in the traceability matrix and must not be described as
production-ready.
