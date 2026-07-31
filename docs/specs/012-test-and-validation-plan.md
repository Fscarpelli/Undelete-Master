# SDD-012 — Test and Validation Plan

Status: Normative; same-revision local gates, release-pair inspection and
GitHub Actions quality run #9 passed; signed-release and governed real-device
recovery evidence pending

## Mandatory same-revision gates

Run from the repository root:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run from `apps/desktop`:

```text
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Also run the documentation, CI-safety and real-only desktop validators and
build the native desktop and broker from the same frozen revision. A focused
test is useful evidence but never replaces a required full gate.

## Prohibited validation

Ordinary local and pull-request tests must not:

- open a real disk, mounted volume or `PhysicalDriveN`;
- attach or initialize a VHD;
- format, trim, repair, lock, dismount, mount or delete storage;
- launch a real scan automatically;
- depend on a sample production provider.

Tests use deterministic in-repository images, synthetic readers, in-memory
protocol transports and disposable temporary ordinary files/directories.
Read-only live inventory may be inspected manually, but inventory is not proof
that a real source scan succeeded.

## Required evidence layers

| Layer | Required checks | Current disposition |
| --- | --- | --- |
| Windows inventory | local/mapped/composite policy, opaque IDs, display grouping, decimal strings, no native paths | Same-revision synthetic/full gate passed; release desktop visibly listed the observed C: and E: mounted NTFS volumes without launching the broker |
| Folder authority | NTFS-only, remote/reparse/cross-volume refusal, volume serial and record/sequence binding | Focused tests present; native acceptance pending |
| Destination authority | retained query-only root handle, handle-derived volume, serial agreement, NTFS/single-disk/direct-bus admission, no WebView path/disk/handle | Focused tests and static regressions passed; governed picker/recovery acceptance on a different physical disk remains pending |
| Protocol v3 | exact ten-message schema, authoritative `Opened` disk field, explicit v2 rejection, one-megabyte cap, malformed frames, sequence replay/gap, nonce mismatch | Focused v3 and same-revision full Rust gates passed; a live elevated exchange and binary-import evidence remain pending |
| Broker lifecycle | fixed sibling, manifests, current-user pipe, peer PID/liveness, timeout, close/shutdown | Focused tests passed; the release pair is sibling and its embedded manifests were extracted; an elevated live broker session was intentionally not started |
| Read path | exact geometry, checked ranges, one-megabyte chunking, source change, 256-and-one-second cadence | Synthetic tests present; no real scan claim |
| Scanner | deterministic NTFS/FAT/partition candidates, partial truth and namespace ancestry | Focused tests present; external corpora pending |
| Desktop DTO/state | 12 commands, inventory/folder schema 1, summary schema 3, candidate-page schema 2, query-page schema 1, decimal strings, 100-row pages, scan-bound cursors, native query/selection/restore authority, 32 scopes/4 sessions | Focused Rust/frontend gates passed; deleted-file recovery acceptance remains pending |
| Frontend | fail closed, real inventory flow, folder cancel, metadata/deep scan, bounded query/paging/selection, restore review/progress/cancel/manifest/open, privacy, provenance, stale/duplicate control, a11y | Same-revision lint/typecheck/build and 146-test suite passed; assistive-technology acceptance remains pending |
| Static safety | no source mutation; physical-disk identity only through fixed query-only extent/property calls; one private top-level canonical import and exactly five bare non-macro audited `CreateFileW` shapes; raw/qualified/rebound/link-name/dynamic resolution denied; one exact `ShellExecuteExW` extern; exact root/io-windows/Tauri dependency inventories, workspace-only first-party child dependencies, no first-party proc-macro crates, Cargo patch rejection, deterministic nested `.cargo/config(.toml)` rejection outside generated/vendor trees with normalized in-repo member traversal and fail-closed escape/loop handling, closed io-windows macro invocation/rebinding surface, and resolved-package loader deny; exactly 12 Tauri commands; closed protocol/opcode surface; no real-device CI | Same-revision static validators and their regression suites passed; live destination/recovery acceptance remains pending |
| Native package | both binaries, hashes, fixed sibling layout, extracted `asInvoker`/`requireAdministrator` manifests | Passed for the unsigned local release pair at the Task 7 revision; hashes and exact limitations are retained in the Task 7 evidence |
| Native UX | actual Tauri window, inventory only, required sizes, focus/zoom/forced colors | Release window launch/close and mounted-volume rendering observed; scan, restore, 200% zoom, forced-colors and assistive-technology acceptance remain pending |
| Remote | pushed revision and successful GitHub Actions conclusions | Passed for commit `41b06d790d47699994a82c41eba44a0a314cc9ab`: `quality` run #9 completed successfully on draft PR #2; later revisions require their own run |
| Release | Authenticode, clean machine, SBOM/licensing and endpoint-security disposition | Pending |

## Focused regression families

- `WINDOWS-INVENTORY-*`, `WINDOWS-FOLDER-SCOPE-*`,
  `WINDOWS-RAW-READ-PLAN-*`, `WINDOWS-NAMED-PIPE-*`;
- `BROKER-PROTOCOL-ARCH-*`, `BROKER-PROTOCOL-MALFORMED-*`,
  `BROKER-PROTOCOL-SESSION-*`, `BROKER-PROTOCOL-BOUNDS-*`;
- `BROKER-CLIENT-PROTOCOL-*`, `BROKER-CLIENT-READER-*`,
  `BROKER-CLIENT-ARCH-*`;
- `ELEVATED-BROKER-ARCH-*`, `ELEVATED-BROKER-SESSION-*`,
  `ELEVATED-BROKER-ARGS-*`, `ELEVATED-BROKER-REVALIDATION-*`;
- `DESKTOP-INVENTORY-*`, `DESKTOP-FOLDER-SCOPE-*`,
  `DESKTOP-PAGINATION-*`, `DESKTOP-CANDIDATE-*`,
  `DESKTOP-STATE-*`, `DESKTOP-REQUEST-ID-*`,
  `DESKTOP-DISK-POLICY-*`;
- `WINDOWS-DESTINATION-BINDING-*`, `WINDOWS-PHYSICAL-BACKING-*`,
  `WINDOWS-RAW-IDENTITY-*`, `DESKTOP-REAL-ONLY-023` through
  `DESKTOP-REAL-ONLY-070`;
- `RESULT-QUERY-*`, `RESULT-CURSOR-*`, `RESULT-SELECTION-*`,
  `RESULT-SELECT-*`, `RESULT-PAGE-*`, `RESULT-LATE-RESPONSE-*`;
- `RESTORE-STREAM-*`, `RESTORE-PARTIAL-*`, `RESTORE-READ-*`,
  `RESTORE-CANCEL-*`, `RESTORE-MEMORY-*`, `RESTORE-HASH-*`,
  `RESTORE-PATH-*`, `RESTORE-COLLISION-*`, `RESTORE-NO-CLOBBER-*`,
  `RESTORE-TEMP-*`, `RESTORE-MANIFEST-*`, `RESTORE-JOURNAL-*`,
  `RESTORE-REPARSE-*`, `RESTORE-DIFFERENT-DISK-*`,
  `RESTORE-SAME-DISK-*`, `RESTORE-UNKNOWN-DISK-*`,
  `RESTORE-SOURCE-CHANGED-*`;
- `DESKTOP-RESULT-CONTROLS-*`, `DESKTOP-RESTORE-CONTRACT-*`,
  `DESKTOP-RESTORE-FLOW-*`, `DESKTOP-RESTORE-A11Y-*`,
  `DESKTOP-RESTORE-OPEN-DESTINATION-*`;
- `WIN-REAL-INVENTORY-*`, `WIN-FOLDER-CANCEL-*`,
  `WIN-REAL-SCAN-*`, `WIN-CANDIDATE-PAGINATION-*`,
  `WIN-DUPLICATE-SCAN-*`, `WIN-ERROR-PRIVACY-*`,
  `WIN-STALE-UNMOUNT-*`, `WIN-VOLUME-RADIO-A11Y-*`,
  `WIN-CANDIDATE-TABLE-A11Y-*`, `WIN-PARTIAL-UNKNOWN-*`.

Existing GPT, NTFS, FAT and image-CLI regressions remain required because the
desktop reuses those parsers.

## Evidence rule

`Verified` requires an exact revision, command, exit status and retained
artifact or log. Native evidence also requires executable hashes, manifest
levels and screenshot provenance. Remote evidence requires the workflow URL
and conclusions.

The [connected-volume evidence record](../evidence/windows-volume-scan-2026-07-29.md)
must remain explicit about pending gates and the absence of a real-volume scan.
The [2026-07-30 recovery-hardening evidence](../evidence/recovery-hardening-2026-07-30.md)
records the newer same-revision local command results and their exact
uncommitted-snapshot boundary. No component result may be generalized to
signed production readiness.

The [2026-07-30 actionable-restore evidence](../evidence/actionable-restore-2026-07-30.md)
records the frozen revision, required gates, release-pair hashes, extracted
manifest levels, bounded packaged launch, successful remote `quality` run #9
and explicitly unverified real-device recovery. It does not promote unsigned
local binaries to a redistributable release.
