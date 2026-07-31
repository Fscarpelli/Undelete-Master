# Task 5 evidence — native actionable restore coordinator

Date: 2026-07-30
Status: Implemented-unverified
Scope: SDD-020 Task 5 native coordinator, Tauri command surface, restore-engine
observation, and deterministic desktop vertical fixture.

## Delivered behavior

- The desktop exposes exactly six native restore commands in addition to the
  six result/query commands: destination selection, immutable plan creation,
  job start, polling, cancellation, and completed-job destination opening.
- JavaScript receives opaque destination, plan, and job identifiers. It never
  receives a source path, destination path, physical-disk number, extent,
  native handle, access mask, control code, executable, or recovered bytes.
- The destination picker retains an authorized native directory capability
  and compares only lightweight scan/source bindings around the modal picker.
- Plan creation snapshots the exact selected candidate revision in immutable
  scan order, derives safe relative paths, enforces explicit partial recovery
  consent, binds the retained destination, and creates a canonical SHA-256
  plan digest.
- Start preflight clones one immutable plan `Arc`; it never duplicates the
  bounded ordered IDs or engine paths/ranges/warnings while the coordinator
  mutex is held.
- Planning and the complete two-phase start execute in `spawn_blocking`.
  Destination I/O occurs outside locks. Start then compares revision and
  ordered selected IDs by borrow, without cloning the candidate payload, and
  atomically consumes the plan only after fresh storage authorization.
- The worker reopens only the scan-bound source through the read-only broker,
  verifies physical-disk number and length before the first destination
  entry, streams through `SourceReader`, and invokes the real transactional
  restore engine.
- Job progress, cancellation, counters, terminal status, actual partial count,
  and manifest SHA-256 come from the engine and observer. No timer, fake
  progress, fixture shortcut, or generated recovery row exists in production.
- A worker scheduling failure removes the job that was not returned and
  restores the one-shot plan marker. Failed or cancelled work retains a final
  manifest summary when the engine actually published one.
- A completed-only destination-open command uses the retained destination
  capability and fixed unelevated Windows shell helper for the exact opaque
  job directory.

## Source safety

The change introduces no source-write operation. Production restore source
access remains a `SourceReader` obtained through the existing read-only
broker. Tests use deterministic in-repository images, memory readers, and
temporary destination directories. No test writes to, trims, formats,
dismounts, or destructively exercises a real disk.

## Deterministic vertical proof

`apps/desktop/src-tauri/tests/restore_fixture.rs` builds an NTFS image with
four deleted candidates and drives the real component chain:

1. CLI filesystem scan;
2. native storage query and selection;
3. immutable native restore plan;
4. background coordinator worker;
5. transactional publication; and
6. final sidecar and manifest verification.

The test proves:

- an existing collision sentinel remains byte-identical;
- same-name candidates receive deterministic no-clobber names;
- intact and fragmented files match deterministic expected SHA-256 values;
- the conflicted 4 KiB range is zero-filled exactly;
- the partial sidecar records the exact range, reason, and SHA-256;
- the final manifest hash matches its bytes and reports one actual partial
  publication; and
- serialized public responses contain no native path or physical-disk field.

## Regression evidence

Focused same-snapshot commands passed:

- `cargo fmt --all -- --check`;
- `cargo clippy -p um-desktop --all-targets -- -D warnings`;
- `cargo test -p um-desktop restore_ --no-fail-fast`: 20 native unit tests and
  21 white-box/vertical filtered tests, zero failures;
- `cargo test -p um-restore`: 39 library tests, 8 path tests, 3 Windows
  capability-race tests, 24 stream/planner tests, and 7 external transaction
  tests, zero failures.

The required branch-wide local gates also passed from the same source state:

- `cargo clippy --workspace --all-targets -- -D warnings`;
- `cargo test --workspace`;
- `pnpm lint`;
- `pnpm typecheck`; and
- `pnpm test`: 6 files and 69 tests, zero failures.

The focused suite includes selection/order staleness, destination drift,
same/unknown disk rejection, source reopen identity drift before writes,
blocking-preflight job-limit recheck, spawn rollback, idempotent cancellation,
late-cancel terminal truth, nonterminal manifest suppression, failed and
cancelled manifest retention, actual partial classification, directory
reconciliation, and terminal observer/manifest count normalization.

## Evidence boundary

This is deterministic component and local build evidence. It does not prove:

- recovery from a user's real physical disk or a live deleted-file case;
- recovery of overwritten, TRIM-discarded, encrypted-without-key, or
  physically unreadable bytes;
- correctness over every filesystem, file type, corruption pattern, device,
  controller, or external corpus;
- long-running performance on a multi-terabyte source;
- packaged Tauri picker/Explorer behavior, accessibility, code signing,
  installer reputation, or antivirus classification; or
- a remote GitHub Actions result.

Those remain explicit Task 6/7 acceptance and release evidence. The
implementation status therefore remains `Implemented-unverified`, not
production-certified.
