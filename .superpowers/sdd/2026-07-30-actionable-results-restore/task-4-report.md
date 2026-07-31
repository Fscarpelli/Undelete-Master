# Task 4 implementer report

## Resume handoff — 2026-07-30

Status at pause/resume: implementation work in progress, not committed, not reviewed.
Task base commit: `3432959b0dd26f70d5eeee82fab5934fcdb51b01`.

### Preserved TDD evidence from the interrupted implementer

- Baseline: `cargo test -p um-restore` passed the pre-Task-4 24 tests.
- Path RED: `cargo test -p um-restore path_` failed because the Task-4 safe-path API did not exist.
- Path GREEN: the focused path suite later passed 7/7.
- Dependency validator RED: exact cap dependency acceptance initially failed on the four missing inventory entries; version-drift rejection was also added.
- Transaction RED: after correcting test-harness-only compile issues, the focused transaction suite failed only because the Task-4 transaction/journal API did not exist.
- Journal fault tests were added for write, flush, and sync failures before implementation.

The resumed controller freshly observed:

- `cargo fmt --all -- --check` — FAIL, formatting-only diff in the new Task-4 files.
- `cargo test -p um-restore --test path_safety path_` — PASS, 7/7.
- `cargo test -p um-restore --test transactional_restore transaction_` — PASS, 4/4.
- `cargo test -p um-restore` — PASS: 2 journal + 7 path + 24 planner/streamer + 4 transaction tests; 0 failures.

### Load-bearing audit decisions

- Use safe `cap_fs_ext::MetadataExt::{dev, ino}` on capability metadata for root/temp identity. Do not use unstable `std::os::windows::fs::MetadataExt` fields and do not add `unsafe` to `um-restore`.
- `cap_std::fs::Dir::from_std_file` consumes the exact Task-3 handle. The retained root was opened without `FILE_SHARE_DELETE`.
- Ordinary capability file opens include delete sharing. On Windows, temporary creation must use `maybe_dir(true)` together with `create_new`, no-follow, and an immediate regular-file check.
- Pre-link verification must compare `(dev, ino)` from the still-open temporary handle with a capability-relative no-follow reopen.
- Parent directory sync is conditional. Expose `Synced`, `Unsupported`, or `Failed`; never claim unconditional Windows namespace durability.
- The journal chain advances only after write, flush, and `sync_all`; any durability failure poisons the journal and forbids later publication/appending behind a possibly torn tail.
- The hard-link through durable `ItemPublished` section is non-cancellable.
- A namespace-sync or published-record failure after linking preserves final and temporary entries and returns explicit reconciliation-needed evidence.
- Partial sidecar publication precedes data publication and is no-clobber.
- Manifest preparation/publication uses a frozen outcomes-prefix hash to avoid circular dependence on the final journal hash.
- Task 3 admission-to-handoff continuity relies on the exact retained root handle opened without delete sharing; Task 4 captures a local baseline from that same handle and revalidates it before work. Document this precisely without exposing identity.

### Required remaining work

- Reformat and then inspect/finish the preserved implementation; do not assume the passing focused tests prove Task 4 complete.
- Add the planned deterministic transition-race coverage and real disposable-NTFS Windows junction race test in `crates/restore/tests/reparse_race_windows.rs`.
- Add fault-injection/cancellation/reconciliation tests required by the brief and the audit decisions, including namespace-sync and `ItemPublished` durability failures.
- Verify journal canonicalization, bounds, torn-tail audit, sidecar ordering, manifest no-clobber, 10,000 collision bound, open-temp rename/delete resistance, and no allocation proportional to candidate size.
- Update ADR-0027 and traceability only for behavior proven by passing tests.
- Run all focused, validator, workspace, frontend, and build gates from the final snapshot before commit.

---

## Final implementation report — 2026-07-30

Status: `DONE`.

Task 4 now provides typed Windows-portable recovery paths, capability-relative
descendant traversal, protected temporary files, atomic hard-link publication,
deterministic bounded collision names, versioned partial sidecars, a durable
hash-chained JSONL journal, and a versioned final manifest with literal SHA-256
identity. Production restore code has no ambient-path constructor, rename,
copy-to-final, overwrite, or unsupported publication fallback.

### Scope delivered

- Added the pinned `cap-std = 4.0.2` and `cap-fs-ext = 4.0.2` workspace
  dependencies and Windows-only `junction = 2.0.0` test dependency, plus exact
  dependency-validator inventory and version-drift tests.
- Added one-time typed path validation for empty/traversal/prefix/separator,
  control/forbidden/ADS, trailing dot/space, Windows device names (including
  superscript COM/LPT digits), component/depth/total UTF-16 limits, and
  deterministic SHA-256 substitutions whose untrusted spelling is retained
  only as JSON evidence.
- Consumed the exact retained root file into `cap_std::fs::Dir`, captured and
  revalidated root identity, and traversed each parent with no-follow
  capability opens and immediate create/rebind.
- Streamed with the Task-2 engine and one fixed 1 MiB buffer into
  `create_new`/no-follow `.umrecovering` files; flushed, synced, length/hash
  checked, and rebound file identity before publication.
- Published partial sidecar before data and manifest last, using only
  capability-relative hard links. Final-name races are atomic and bounded to
  10,000 deterministic extension-preserving candidates.
- Recorded namespace durability as `Synced`, `Unsupported`, or `Failed`;
  preserved linked evidence and returned `NeedsReconciliation` after
  post-publication durability failures; recorded cleanup warnings without
  removing or replacing the final link.
- Added bounded journal payload/record/count enforcement, monotonic sequence,
  fixed hashed job identity, previous/record SHA-256 chaining, poison-on-write,
  flush, or sync failure, and audit rejection of torn, tampered, oversized, or
  cross-job records.
- Added manifest/sidecar evidence for requested/safe paths, expected and actual
  hashes, readable/zero/conflict/read-error ranges, warnings, sidecar identity,
  temporary disposition, namespace durability, and completion state.
- Added deterministic scripted create/rebind replacement coverage plus real
  disposable-NTFS Windows junction and open-handle rename/delete races. Tests
  use only `TempDir` destinations and synthetic memory sources.

### RED/GREEN evidence

- Preserved RED: safe-path APIs absent; GREEN: 7 path-security tests.
- Preserved RED: transaction/journal APIs absent; GREEN: transactional
  publication, collision, cancellation, sidecar, manifest, and journal suites.
- Fresh RED: journal audit accepted a payload slightly over the append payload
  bound while the complete record still fit its envelope; GREEN:
  `journal_audit_rejects_a_payload_over_the_append_bound`.
- Fresh compile RED/GREEN seams covered `ItemPublished` sync failure, manifest
  collision, namespace temporary disposition, cleanup failure, and manifest
  cleanup warning ordering.
- Mutation guard: removing superscript reserved-device recognition failed the
  exact path test; restoring it passed.
- Mutation guard: removing record-hash comparison failed the exact tamper test;
  restoring it passed.
- Mutation guard: removing `maybe_dir(true)` let the competing Windows rename
  win and failed pre-link rebinding; restoring it passed.
- Mutation guard: an ambient descendant reopen at the junction transition
  created outside-root evidence and failed the real Windows race assertion;
  restoring capability-relative no-follow traversal passed. A naive
  no-follow-to-ordinary-capability-open mutation remained contained by
  `cap-std` and was recorded as non-evidence rather than claimed as a kill.

### Final verification snapshot

- `cargo fmt --all -- --check` — PASS.
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS.
- `cargo test --workspace` — PASS; Task-4 restore slice includes 24 unit,
  7 path, 2 Windows race, 24 planner/stream, and 4 external transaction tests,
  all with zero failures or ignored tests.
- `pnpm lint` — PASS.
- `pnpm typecheck` — PASS.
- `pnpm test` — PASS, 6 files / 48 tests.
- `python -B .github/scripts/tests/test_validate_real_only_desktop.py` — PASS,
  72 tests.
- `python -B .github/scripts/validate_real_only_desktop.py --root .` — PASS,
  106 production boundary files.
- `python -B .github/scripts/validate_ci_safety.py --root .` — PASS, 1 workflow
  and 109 first-party surfaces.
- `git diff --check` — PASS.

### Known non-Task-4 diagnostic and remaining boundaries

`python -B .github/scripts/validate_docs.py` still reports the unchanged
Task-1 plan line
`docs/superpowers/plans/2026-07-30-actionable-results-restore.md:31` for the
literal placeholder token `stubs`. The parent controller classified this as a
pre-existing, non-required Task-4 diagnostic scheduled for Task 7; Task 4 did
not alter the plan.

Persistent resume is intentionally out of scope. Tasks 5–7 still own the
native coordinator/Tauri API, end-user workflow, vertical fixture, packaging,
and release evidence. Directory sync can legitimately be reported as
unsupported; in that state temporary links remain for reconciliation rather
than being misreported as cleanup failures.

Scoped commit: `b43ab9015bffb1219b43888d6040492fe3fbb56d`
(`feat: publish recovered files transactionally`).

---

## Fix Round 1 — 2026-07-30

Status while recording this section: implementation and focused tests are
green; final same-snapshot gates and the dedicated fix commit are pending.
Round base commit: `b43ab9015bffb1219b43888d6040492fe3fbb56d`.

### Review findings and exact RED/GREEN evidence

1. **CRITICAL — protected Windows temporary handles made path cleanup both
   ineffective and unsafe.**
   - RED: the first Windows regression,
     `transaction_windows_closes_protected_handles_before_durable_temp_cleanup`,
     failed because durable publication retained protected `.umrecovering`
     links while the no-delete-share handles were still live.
   - An intermediate close-then-`remove_file(name)` repair made that test pass,
     but the follow-up security review correctly identified a substitution
     window between handle release and path deletion. That intermediate design
     was discarded and is not present in the final diff.
   - GREEN: production code performs no path-based temporary deletion.
     `transaction_protected_temps_are_retained_without_path_based_cleanup` and
     `transaction_windows_data_collision_keeps_item_sidecar_and_safe_temp_evidence`
     prove protected temporary evidence remains, with
     `retainedBySafeCleanupPolicy` after synchronized publication and
     `retainedForReconciliation` otherwise. The existing Windows
     `transaction_windows_open_temporary_handle_denies_competing_rename_and_delete`
     regression continues to prove the pre-link protection itself.

2. **IMPORTANT — failed and cancelled jobs did not publish a truthful manifest
   with exactly the immutable plan item count.**
   - RED:
     `transaction_failure_manifest_preserves_the_immutable_plan_item_count`
     and
     `transaction_cancellation_manifest_marks_current_and_unattempted_items`
     both failed with `NotFound` for the final manifest.
   - GREEN: ordinary item failure/cancellation with a healthy journal proceeds
     to final-manifest publication; every successfully published final manifest
     has one entry per planned item, using explicit `published`, `failed`,
     `cancelled`, `notAttempted`, or `directoryCreated` dispositions. The two
     named regressions pass with `[published, failed, notAttempted]` and
     `[cancelled, notAttempted]` respectively. Manifest preparation or
     no-clobber publication can still fail explicitly without replacing an
     existing manifest. Existing durability-injection tests prove that a
     poisoned journal publishes no manifest and makes no unsupported claim past
     its durable prefix.

3. **IMPORTANT — partial sidecars were incomplete and coupled to data-name
   retries.**
   - RED: `transaction_partial_file_emits_exact_versioned_range_sidecar`
     observed the former minimal JSON instead of the complete versioned
     evidence object.
     `transaction_independent_sidecar_and_data_collisions_do_not_retract_the_sidecar`
     observed a data output ending in `(recovered 2)` when independent
     sidecar/data collision state required `(recovered 1)`.
     `transaction_failed_data_publication_manifest_binds_the_published_sidecar`
     observed a null item key in the failure manifest.
   - GREEN: a sidecar is keyed by immutable ordered plan index plus candidate
     ID, published exactly once and durably journaled before data-name retries.
     It carries logical size, readable/zero/missing/conflict/read-error ranges,
     output and expected hashes, validation, and warnings. The manifest binds
     item key, sidecar path, and sidecar SHA-256 even when later data
     publication fails. The three named regressions plus
     `transaction_sidecar_journal_fault_stops_before_data_and_keeps_reconciliation_evidence`
     and the real Windows collision test pass. The pre-existing colliding
     sidecar sentinel is never removed.

4. **IMPORTANT — journal audit accepted alternate encodings and applied no
   pre-retention record-count bound.**
   - RED:
     `journal_audit_rejects_noncanonical_and_unknown_envelope_bytes` showed
     that leading whitespace was accepted; the initial count-bound regression
     also failed to compile because no bounded audit seam existed.
     A closure audit then exposed a second byte-canonicalization RED: Rust
     `str::lines()` normalized CRLF, so the same regression failed when it
     required a CRLF record to be rejected.
   - GREEN: audit canonicalizes each parsed record and requires byte-for-byte
     equality with the raw JSONL bytes, including the line terminator boundary,
     rejects CRLF/alternate whitespace and unknown envelope fields, and checks
     the record limit before retaining the record.
     `journal_audit_rejects_noncanonical_and_unknown_envelope_bytes` and
     `journal_audit_enforces_the_record_count_bound_before_retention` pass,
     together with the existing tamper, torn-tail, payload, complete-record,
     and cross-job audit tests.

5. **IMPORTANT — restore planning lacked explicit aggregate path and original
   evidence budgets.**
   - RED: the new per-item evidence regression initially failed to compile
     because `MAX_PATH_EVIDENCE_BYTES_PER_ITEM` and
     `PathSafetyError::EvidenceBudgetExceeded` did not exist. The aggregate
     constructor regressions likewise failed to compile without job-level
     constants and structured errors.
   - GREEN: untrusted original spelling is capped before cloning and again
     after JSON serialization at 256 KiB per item. `RestoreJobPlan::new`
     performs checked accumulation with an 8 MiB aggregate evidence cap and
     one-million sanitized-component cap.
     `path_rejects_untrusted_original_evidence_over_the_explicit_byte_budget`,
     `restore_job_plan_rejects_aggregate_untrusted_path_evidence_over_budget`,
     `restore_job_plan_rejects_more_than_one_million_sanitized_components`,
     and
     `restore_job_path_budget_checked_arithmetic_accepts_boundary_and_rejects_overflow`
     pass.

6. **IMPORTANT — Windows directory-sync classification treated unrelated OS
   failures as “unsupported.”**
   - RED:
     `windows_namespace_sync_classifier_distinguishes_unsupported_from_media_failures`
     showed raw code `1` was incorrectly classified as `Unsupported`.
   - GREEN: Windows now treats only raw codes `5`, `50`, and `120` as the
     reviewed unsupported/read-query boundary; codes `1`, `6`, `21`, `23`,
     `29`, `31`, `87`, `112`, `1117`, and `1167` are `Failed`. The table
     regression passes.

7. **IMPORTANT — selected directories had no Task-4 constructor or manifest
   path independent of file content.**
   - RED:
     `transaction_directory_only_creates_exactly_the_selected_sanitized_directory`
     initially failed to compile because a directory-only restore item
     constructor did not exist.
   - GREEN: `FileRestorePlan::directory` carries no `ContentPlan`, walks only
     sanitized ancestors, creates and immediately no-follow rebinds the
     selected directory, performs no source read or data/sidecar temporary
     publication, and records one `directoryCreated` manifest entry. The named
     regression passes and proves no historical child or sibling is created.

### Review ledger

- The review's minor maintainability concern remains valid:
  `crates/restore/src/transaction.rs` is large. Round 1 did not perform a
  security-sensitive module split while closing the bounded correctness gaps;
  that refactor remains separate work.
- SDD-020, ADR-0027, and the traceability matrix now state the item-keyed
  sidecar, safe retained-temporary policy, exact manifest cardinality,
  poisoned-journal boundary, canonical/count-bounded audit, path-evidence
  budgets, Windows sync classifier, and directory-only behavior.
- The tracked implementation plan and internal Task 4 brief record that the
  reviewed safe-retention policy supersedes their original path-cleanup step;
  no close-then-delete fallback remains specified.
- No test writes to a scan source. Windows namespace tests use only `TempDir`
  destinations and synthetic in-memory sources.

### Fix Round 1 same-snapshot verification

- `cargo fmt --all` — PASS.
- `cargo fmt --all -- --check` — PASS.
- `cargo clippy -p um-restore --all-targets -- -D warnings` — PASS.
- `cargo test -p um-restore` — PASS: 32 unit, 8 path, 3 Windows
  race/protection, 24 planner/stream, and 7 external transaction tests; 74
  passed in total, with 0 failed and 0 ignored.
- `python -B .github/scripts/tests/test_validate_real_only_desktop.py` —
  PASS, 72 tests.
- `python -B .github/scripts/validate_real_only_desktop.py --root .` — PASS,
  106 production boundary files.
- `python -B .github/scripts/validate_ci_safety.py --root .` — PASS, 1
  workflow and 109 enumerated first-party code/command surfaces.
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS.
- `cargo test --workspace` — PASS.
- `pnpm lint` in `apps/desktop` — PASS.
- `pnpm typecheck` in `apps/desktop` — PASS.
- `pnpm test` in `apps/desktop` — PASS, 6 files / 48 tests.
- `git diff --check` — PASS; Git emitted only line-ending conversion warnings,
  with no whitespace error.
- Optional `python -B .github/scripts/validate_docs.py` — the only diagnostic
  remains the previously ledgered Task 1 literal `stubs` token at plan line
  31; it reported no Task 4 plan/spec/ADR/traceability inconsistency.

Status after this snapshot: implementation and all required gates are green;
the dedicated fix commit and scoped independent re-review are the remaining
Task 4 closure steps.

---

## Fix Round 2 — 2026-07-30

Round base: `963d2b811fd399b24c7dcd1f060809ec532e70d3`
(`fix: close transactional restore review gaps`).

### New scoped finding

The Round 1 directory-only path used direct `create_dir`, then performed a
fallible transition hook, no-follow bind, parent sync, and `ItemPublished`.
Any failure after the direct create left a visible directory, but the healthy
error path emitted a plain `failed` manifest item with no publication
evidence. A poisoned `ItemPublished` path also lacked a durable record naming
the exact directory candidate. This contradicted the publication invariant and
made a collision-resolved residual ambiguous.

### RED evidence

The four regressions were added before production changes and run with:

```text
cargo test -p um-restore transaction_directory_ -- --nocapture
```

Result: `0 passed; 4 failed`.

- `transaction_directory_created_before_bind_failure_is_manifested_for_reconciliation`
  failed because the returned error was not `RestoreError::NeedsReconciliation`.
  Its fixture first created a real base-name collision, so the visible residual
  was the literal `selected/only-directory (recovered 1)`.
- `transaction_directory_nofollow_bind_substitution_is_contained_and_manifested`
  failed because the returned error was not `NeedsReconciliation`; the test
  retained the substituted final name and original created directory while
  proving the outside sentinel/count were unchanged.
- `transaction_directory_namespace_sync_failure_is_manifested_for_reconciliation`
  failed with `left: "directoryCreated"` and
  `right: "directoryNeedsReconciliation"`.
- `transaction_directory_item_published_journal_failure_stops_with_durable_candidate_evidence`
  failed because the returned error was not `NeedsReconciliation`; the former
  journal had no durable collision-candidate record before direct create.

The successful directory-only regression was also strengthened to require
that directory results expose no file-only length, SHA-256, or temporary-file
disposition.

### GREEN state machine

- Path evidence is deserialized and validated before any final-directory
  create.
- Before every collision candidate, durable
  `directoryPublicationPlanned` evidence binds candidate ID, item key,
  directory kind, exact collision-resolved path/name, and collision index.
- Direct no-clobber `create_dir` is the directory publication point. There is
  no post-create path rollback, remove, rename, or substituted-entry cleanup.
- After create, the exact final name is opened no-follow. The bound capability
  identity is compared with a capability-relative no-follow name lookup, and
  the bound `Dir` remains alive through parent sync and durable
  `itemPublished`.
- A transition, bind, or identity failure after create appends
  `directoryReconciliationRequired` while the journal is healthy and carries
  a generalized publication result into the manifest. It cannot fall through
  the dense success prefix or become a plain unpublished failure.
- Visible unbound/unvalidated directories and bound directories with
  `Unsupported`/`Failed` namespace durability use
  `directoryNeedsReconciliation`, with actual path/name, item key/kind,
  no-follow-bind flag, validation state, namespace durability, completion
  status, and `needsReconciliation` failure kind.
- A poisoned reconciliation or `itemPublished` boundary stops immediately,
  publishes no manifest or later item, preserves the visible directory, and
  returns a structured reconciliation error containing the actual path. The
  durable pre-create prefix identifies every attempted candidate.
- Directory result fields for output length/hash and temporary-file
  disposition are nullable and remain absent/JSON `null`; no empty-file
  semantics are fabricated.

Focused GREEN:

```text
cargo test -p um-restore transaction_directory_ -- --nocapture
```

Result: the four new internal regressions and the existing external
`transaction_directory_only_creates_exactly_the_selected_sanitized_directory`
all passed.

`cargo clippy -p um-restore --all-targets -- -D warnings` passed.
`cargo test -p um-restore` passed with 36 unit, 8 path, 3 Windows,
24 planner/stream, and 7 external transaction tests: 78 passed total, 0 failed,
0 ignored.

### Documentation and residual ledger

SDD-020, ADR-0027, the traceability matrix, tracked Task 4 plan, and internal
Task 4 brief now define the direct-directory publication boundary and both
forms of `directoryNeedsReconciliation`. The progress ledger records Fix Round
2 as implemented/re-review pending.

The maintainability Minor remains open and is now measured accurately:
`crates/restore/src/transaction.rs` is 3,507 lines after formatting. This round
does not mix a security-sensitive module split into the state-machine repair.

Final same-snapshot workspace/frontend/validator gates and the dedicated
Round 2 commit remain pending at this report checkpoint.

### Fix Round 2 final same-snapshot verification

- `cargo fmt --all` — PASS.
- `cargo fmt --all -- --check` — PASS.
- `cargo clippy -p um-restore --all-targets -- -D warnings` — PASS.
- `cargo test -p um-restore` — PASS: 36 unit, 8 path, 3 Windows,
  24 planner/stream, and 7 external transaction tests; 78 passed total,
  0 failed, 0 ignored.
- `python -B .github/scripts/tests/test_validate_real_only_desktop.py` —
  PASS, 72 tests.
- `python -B .github/scripts/validate_real_only_desktop.py --root .` — PASS,
  106 production boundary files.
- `python -B .github/scripts/validate_ci_safety.py --root .` — PASS, 1 workflow
  and 109 enumerated first-party code/command surfaces.
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS.
- `cargo test --workspace` — PASS.
- `pnpm lint` in `apps/desktop` — PASS.
- `pnpm typecheck` in `apps/desktop` — PASS.
- `pnpm test` in `apps/desktop` — PASS, 6 files / 48 tests.
- `git diff --check` — PASS; only line-ending conversion notices were emitted.
- Optional `python -B .github/scripts/validate_docs.py` — the sole diagnostic
  remains the previously ledgered Task 1 literal `stubs` token in the tracked
  implementation plan; no Round 2 document produced a new diagnostic.

Final source audit found no production path-based directory rollback or
temporary cleanup, no released-handle deletion, and no unsafe code in
`crates/restore`. The only `std::fs::rename`/`remove_file` matches are the
explicit test-only substitution helpers.

Status after this snapshot: Fix Round 2 implementation and all required gates
are green; the separate commit and scoped re-review remain.
