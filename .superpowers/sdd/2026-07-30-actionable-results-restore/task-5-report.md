# Task 5 implementer report

Date: 2026-07-30
Status: implementation complete; scoped verification green
Native consolidation base: `bb412a3`

## Requirements delivered

This task implements `DESKTOP-RESTORE-CONTRACT-023`, the native half of
`DESKTOP-RESTORE-FLOW-024`, and `DESKTOP-RESTORE-OPEN-DESTINATION-026`.

The desktop now has a real native recovery path:

- `select_restore_destination` retains a native directory capability and
  returns only an opaque destination summary;
- `create_restore_plan` freezes the exact scan/source, selection revision,
  ordered candidate IDs, safe paths, policies, destination, engine work, byte
  counts, and SHA-256 plan digest;
- `start_restore` revalidates storage and destination authority, consumes a
  one-shot plan, and launches a real background restore worker;
- `get_restore_job` returns engine-derived progress and terminal evidence;
- `cancel_restore` requests idempotent cooperative cancellation; and
- `open_restore_destination` opens only the completed opaque job directory
  through the retained capability and fixed unelevated shell helper.

The native coordinator bounds retention to 32 destinations, eight plans, and
eight job lifecycles. It rejects saturation without evicting active or
terminal evidence.

## Authority and concurrency design

No command accepts or returns a source/destination path, disk number, extent,
offset, native handle, access mask, control code, executable, or recovered
bytes.

Destination selection clones only a lightweight scan/source binding around
the modal picker. Full candidate snapshotting and planning execute in
`spawn_blocking`, outside the desktop async runtime.

Start is two-phase:

1. snapshot the immutable plan/capability under the coordinator mutex, release
   it, then perform destination revalidation, free-space checks, disk
   separation, retained-root cloning, and `DestinationRoot` construction;
2. borrow the current storage selection under its short lock, compare revision
   and ordered IDs without cloning candidate payloads, then atomically
   recheck/consume the plan, destination authority, one-shot marker, and job
   limit under the coordinator mutex.

No storage or coordinator lock is held across I/O. The worker is launched only
after both locks have been released. A scheduling failure removes the
unobservable job and restores plan eligibility.

The retained plan stores its complete immutable payload behind one `Arc`.
Preflight therefore shares the exact ordered IDs and engine plan by identity
instead of cloning up to 110,000 paths/ranges/warnings while polling and
cancellation wait on the coordinator mutex.

## Engine and terminal integration

The worker reopens the exact scan source through the read-only broker and
verifies physical-disk number and source length before creating any
destination entry. It then invokes
`DestinationRoot::restore_job_observed` with:

- the immutable engine plan;
- the retained destination capability;
- a real streaming progress sink;
- one atomic cancellation probe; and
- an item/manifest observer.

The restore engine now reports final-manifest publication only after it
succeeds. The coordinator keeps that summary pending until it atomically
commits terminal state, so a nonterminal snapshot cannot expose a final
manifest. Failed and cancelled jobs retain published manifest evidence.

`partialItems` counts actual published partial outputs, not planned
best-effort candidates. A late cancel request cannot rewrite a real failed
engine terminal. If engine-success and observer counts disagree, the
published manifest remains authoritative for terminal public counters and
bytes; the job remains `failed` with
`RESTORE_TERMINAL_COUNT_MISMATCH`.

## Deterministic vertical fixture

`apps/desktop/src-tauri/tests/restore_fixture.rs` uses
`um-fixture-builder`, a memory source, and a temporary destination. It drives
the real scan, native query/selection, immutable plan, worker, transaction,
sidecar, and manifest path for:

- one intact deleted file;
- one fragmented deleted file;
- one conflicting/incomplete file with an exact 4 KiB zero-fill range; and
- a same-name collision while preserving an existing sentinel.

Outputs, sidecar bytes, and final manifest bytes are verified against
deterministic expected SHA-256 values. No test opens or modifies a real disk.

## TDD and review closure

Preserved RED evidence from the task brief covered absent native coordinator
commands and the absent vertical path. Subsequent review-driven RED/GREEN
regressions closed:

- destination drift before source open;
- reopened source identity drift before any destination entry;
- worker spawn failure leaving an invisible job;
- terminal manifest appearing before terminal status;
- failed/cancelled manifest publication being discarded;
- directory publication needing reconciliation but missing from final
  manifest counts;
- actual partial outputs being confused with planned partial candidates;
- late cancellation rewriting a failed engine result;
- native selection bound exceeding engine plan capacity;
- terminal count mismatch losing already-published evidence;
- a missed observer event producing a native response rejected by the
  TypeScript contract;
- slow destination I/O holding the coordinator mutex;
- selection changing between slow preflight and plan consumption;
- full candidate cloning during start/picker authorization; and
- retained-job saturation changing during blocking preflight.

## Verification snapshot

Same-snapshot focused verification:

- `cargo fmt --all -- --check` — PASS;
- `cargo clippy -p um-desktop --all-targets -- -D warnings` — PASS;
- `cargo test -p um-desktop restore_ --no-fail-fast` — PASS, 20 native unit
  tests plus 21 white-box/vertical filtered tests;
- `cargo test -p um-restore` — PASS: 39 library, 8 path, 3 Windows race,
  24 planner/stream, and 7 external transaction tests.

Required same-snapshot branch gates:

- `cargo clippy --workspace --all-targets -- -D warnings` — PASS;
- `cargo test --workspace` — PASS;
- `pnpm lint` — PASS;
- `pnpm typecheck` — PASS;
- `pnpm test` — PASS, 6 files / 69 tests.

`git diff --check` also passed. The optional documentation validator continues
to report only the previously ledgered Task 1 diagnostic at
`docs/superpowers/plans/2026-07-30-actionable-results-restore.md:31`; Task 5
did not change the approved plan line.

Public evidence:
[actionable-restore-coordinator-2026-07-30.md](../../../docs/evidence/actionable-restore-coordinator-2026-07-30.md).

## Honest completion boundary

This task proves deterministic local component behavior. It does not claim a
real deleted-file recovery from a user's disk, recovery of overwritten or
TRIM-discarded bytes, every format/filesystem/device, external-corpus
coverage, multi-terabyte performance, packaged Windows picker/Explorer
behavior, accessibility, signing, installer reputation, antivirus trust, or
remote CI. Those remain Task 6/7 acceptance and release evidence.
