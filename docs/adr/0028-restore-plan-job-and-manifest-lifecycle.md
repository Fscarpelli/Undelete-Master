# ADR-0028 — Restore Plan, Job, and Manifest Lifecycle

Status: Accepted
Decision date: 2026-07-30

Implementation status: `Implemented-unverified`. The native coordinator,
transaction observer, immutable plan binding, background job lifecycle, and
deterministic vertical fixture are implemented. Same-revision local evidence
is recorded in
[the Task 5 evidence report](../evidence/actionable-restore-coordinator-2026-07-30.md).
The actionable results workspace and real native-snapshot UI were subsequently
implemented under [SDD-020](../specs/020-actionable-results-and-transactional-restore.md)
and [the traceability matrix](../traceability-matrix.md). Packaged-host,
physical-device, accessibility, external-corpus, performance, signing, and
release evidence remain outside this decision and must not be inferred from
the deterministic fixture or local component tests.

## Context

SDD-020 requires a selected candidate to become a real recovery operation
without allowing JavaScript to acquire a source path, destination path,
physical-disk number, extent, handle, access mask, control code, executable,
or recovered byte. Tasks 1–4 established native query/selection authority,
bounded content plans, retained destination handles, transactional
publication, journals, partial sidecars, and manifests. A native coordinator
is still required to bind these capabilities into an immutable plan and an
observable, cancellable job.

The coordinator must remain responsive while destination revalidation and
content planning perform I/O. It must also fail closed when the scan,
selection, source, or destination changes. A native command failure must not
leave an unreachable background job, and bounded retention must never evict
active evidence silently.

## Decision

### Native-only authority and bounded retention

`RestoreCoordinator` is Tauri-managed native state. It retains at most:

- 32 destination authorities;
- eight immutable restore plans; and
- eight restore-job lifecycles.

Every identifier returned to the WebView is a random opaque identifier with a
fixed kind prefix and canonical length. The destination map retains the
native `DestinationRootBinding`; the plan map retains the engine plan and
source/selection binding; and the job map retains cancellation, progress,
terminal status, and manifest summary. Each immutable plan payload is held by
`Arc`, so start preflight shares the exact engine plan and ordered-ID authority
instead of duplicating up to 110,000 paths/ranges/warnings under the
coordinator mutex. None is serialized by value.

Admission rejects an overflow instead of evicting an existing authority,
plan, or job. Plans and jobs remain retained for the desktop process lifetime;
closing or restarting the desktop intentionally clears them. Persistent
resume and automatic cleanup are not part of this increment.

Native picker cancellation returns `null` and creates no authority. A
destination ID is valid only for the exact scan/source binding used at
admission. Revalidation must preserve NTFS, reparse safety, root identity,
physical-disk identity, and enough free space. The destination disk must
remain different from the scan-bound source disk. The modal picker reads only
a lightweight scan/source binding before and after selection; it never clones
the selected candidate payload.

### Immutable restore plan

`create_restore_plan` consumes a native storage snapshot for one exact
selection revision. The snapshot preserves the scan's candidate order and
retains the complete native candidates, optional expected content hashes, and
bounded warnings. Hash-set iteration is not plan authority. The complete
snapshot and all per-candidate planning run inside `spawn_blocking`; the
desktop async runtime never clones or plans up to the 110,000-candidate
native bound.

The immutable plan binds:

- scan ID and source identity/length;
- selection revision and ordered candidate IDs;
- destination ID and its retained capability;
- `rename` collision policy;
- `completeOnly` or explicit `zeroFillAndMap` partial policy;
- exact engine items and sanitized relative paths;
- file logical bytes, excluding directory metadata sizes; and
- a SHA-256 plan digest over the canonical binding.

Metadata-only or otherwise unbounded candidates cannot enter a plan.
Best-effort candidates require explicit `zeroFillAndMap` consent. A selected
directory creates only that directory; it contributes no logical content
bytes and implies no historical descendants.

The coordinator mutex is held only while reading or committing retained
authority state. Destination I/O, revalidation, path derivation, complete
candidate snapshotting, and per-candidate planning execute outside it. Before
insertion, the coordinator atomically rechecks the retained capability, scan
binding, and retention limit.

### Start, source reopen, and background work

`start_restore` accepts only a plan ID and uses a two-phase start inside
`spawn_blocking`. Phase one clones only the immutable plan `Arc` and
capability under the coordinator mutex, releases it, and performs destination baseline, disk,
free-space, retained-root, and `DestinationRoot` I/O. Phase two locks the
storage authority only long enough to compare the current revision and
ordered selected candidate IDs by borrow, without cloning candidate
paths/extents/warnings. While that storage lock is held, the coordinator
atomically rechecks and consumes the exact plan, one-shot marker, destination
authority, and job bound. Neither lock is held across I/O, and the worker is
not launched until both locks have been released.

After the native job entry is committed, a named worker thread is scheduled
and the command returns an initial `queued` snapshot immediately. A worker
spawn error means no worker was scheduled; the coordinator removes the
unreachable job and resets the plan's one-shot marker before returning the
error. The same plan may then be retried. This avoids an invisible retained
job for which the caller never received a job ID.

The worker reopens the source through the read-only broker and compares both
the authoritative physical-disk number and canonical length with the
plan-bound source before creating any destination entry. A mismatch produces
terminal `RESTORE_SOURCE_CHANGED` evidence and no manifest. All source reads
continue through `SourceReader`; the restore coordinator has no source-write
operation.

The worker passes the immutable engine plan, destination capability, progress
sink, cooperative cancellation probe, and job observer to
`DestinationRoot::restore_job_observed`. The observer reports item start and
terminal item outcomes. Byte counters come from accepted streaming progress,
not timers or simulations.

### Terminal state and manifest summary

`get_restore_job` returns native item and byte counters, current item,
sanitized warnings, status, and an optional manifest summary.
`cancel_restore` is idempotent and requests cooperative cancellation; it does
not interrupt a non-cancellable publication boundary.

A job is `completed` only when:

- observer item completion equals the immutable item count;
- no item failed or was cancelled;
- the engine summary contains exactly the immutable item count; and
- the final manifest was actually published.

Any count disagreement fails with
`RESTORE_TERMINAL_COUNT_MISMATCH`. `publishedItems` is derived from the
published manifest summary. When an engine-success observer count disagrees,
the published manifest is the durable terminal authority: public disposition
and byte counters are normalized to that summary, the manifest remains
visible, and status remains `failed` with the mismatch warning. This preserves
truthful, parseable evidence without claiming a clean completion.
`partialItems` is not the number of
best-effort-planned candidates: it counts actual published file results with a
published partial sidecar and at least one non-sparse zero-filled range.

Failed and cancelled transactions may also have a successfully published
final manifest. The engine notifies the coordinator only after manifest
publication; the coordinator commits that summary and the terminal state
under one job-state lock. Nonterminal snapshots never expose a pending
manifest.

`open_restore_destination` accepts only a completed job with a manifest. It
revalidates the job-bound destination, opens only the exact opaque engine job
directory below the retained capability with no-follow semantics, and invokes
the fixed Windows Explorer/COM shell operation from the unelevated desktop.
No caller-selected path or executable is accepted.

### Native command surface

The existing six result/selection commands and these six restore commands are
the complete desktop command inventory:

- `select_restore_destination`;
- `create_restore_plan`;
- `start_restore`;
- `get_restore_job`;
- `cancel_restore`; and
- `open_restore_destination`.

The production-surface validator checks the exact registrations,
dependencies, fixed shell helper, blocking command boundaries, and absence of
forbidden caller-controlled path/device/write surfaces. Planning and the
complete two-phase start use `spawn_blocking` at the Tauri command boundary so
synchronous native work does not block the async runtime.

## Failure model

The following fail closed:

- stale or missing scan, selection, destination, plan, or job;
- changed ordered selection or source identity/length;
- destination identity, filesystem, reparse, disk, or free-space drift;
- same or unknown physical-disk relationship;
- unsafe derived path or an ineligible content plan;
- missing explicit best-effort consent;
- retention saturation;
- worker scheduling failure;
- engine, journal, sidecar, or manifest failure; and
- terminal observer/manifest cardinality disagreement.

Errors contain stable codes and generic messages. They do not disclose native
paths, volume IDs, disk numbers, extents, handles, or recovered content.

## Verification boundary

The vertical acceptance test builds a deterministic NTFS image with
`fixture-builder`, invokes the real CLI scan, native storage query/selection,
restore snapshot, coordinator, and transactional engine, and writes only to a
temporary destination. It proves:

- a pre-existing collision sentinel is unchanged;
- same-name files use deterministic no-clobber recovery names;
- intact and fragmented outputs match expected SHA-256 values;
- a conflicted range is zero-filled exactly;
- the partial sidecar and manifest contain exact ranges and hashes;
- `partialItems` equals the observed partial publication count; and
- the serialized public contract contains no source or destination path or
  physical-disk field.

This fixture is deterministic component acceptance. It is not proof for a
real deleted file, live mounted source stability, TRIM reversal, overwritten
bytes, damaged hardware, every file format, packaged Windows shell behavior,
or long-running performance. Destructive tests against real disks remain
forbidden.

## Consequences

### Positive

- restore authority remains native and opaque;
- no source-write capability is introduced;
- immutable selection order and plan digest make the requested work
  auditable;
- progress, cancellation, output hashes, partial evidence, and completion are
  produced by the real engine;
- slow plan work does not monopolize the coordinator mutex;
- retention is bounded without silent eviction; and
- a worker scheduling failure cannot strand an invisible job.

### Negative and residual risks

- job state is process-local and cannot resume after restart;
- completed jobs consume one of eight lifecycle slots until restart;
- destination support remains NTFS on a known, separate, direct physical
  disk;
- synchronized publication intentionally retains protected
  `.umrecovering` links until an identity-atomic cleanup primitive is
  implemented;
- a live mounted source is not a snapshot and may change during work;
- recovery cannot reconstruct bytes already overwritten, trimmed, encrypted
  without keys, or physically unreadable; and
- the local user workflow and unsigned release pair are recorded in the
  [Task 7 evidence](../evidence/actionable-restore-2026-07-30.md), while
  governed physical real-device recovery remains unverified.

## Rejected alternatives

- **Expose a picker path to JavaScript:** turns untrusted presentation data
  into authority and reintroduces path-substitution risk.
- **Reopen destination paths for planning or shell launch:** separates use
  from the retained handle that was validated.
- **Evict old plans or jobs automatically:** can discard active or terminal
  evidence without user acknowledgement.
- **Count planned best-effort items as partial outputs:** overstates what the
  manifest actually published.
- **Return an error while retaining a failed-to-spawn job:** leaves an
  unreachable lifecycle because no job ID reached the caller.
- **Simulate progress or completion:** violates SDD-020's real-only
  requirement.

## Related decisions

- [ADR-0002](0002-read-only-source-invariant.md): the scan source is never
  written.
- [ADR-0023](0023-windows-read-only-broker-and-folder-scope.md): broker and
  fixed Windows API boundary.
- [ADR-0025](0025-native-result-query-and-selection-authority.md): native
  query and selection authority.
- [ADR-0026](0026-bounded-content-plan-and-partial-recovery.md): content plan
  and streaming extraction.
- [ADR-0027](0027-destination-capability-and-disk-separation.md): retained
  destination authority and transactional publication.
- [SDD-020](../specs/020-actionable-results-and-transactional-restore.md):
  approved end-to-end behavior.

## Revisit triggers

Revisit before adding persistent job resume, automatic retention eviction,
another destination filesystem, same-disk override, destination cleanup,
parallel item publication, caller-selected paths/executables, broker-side
destination writes, or any new Windows API.
