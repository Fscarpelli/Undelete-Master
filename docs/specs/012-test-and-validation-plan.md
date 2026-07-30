# SDD-012 — Test and Validation Plan

Status: Normative; connected-volume final evidence pending

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
| Windows inventory | local/mapped/composite policy, opaque IDs, display grouping, decimal strings, no native paths | Focused tests present; final full gate pending |
| Folder authority | NTFS-only, remote/reparse/cross-volume refusal, volume serial and record/sequence binding | Focused tests present; native acceptance pending |
| Protocol v2 | exact ten-message schema, one-megabyte cap, malformed frames, sequence replay/gap, nonce mismatch | Focused tests present; final full gate pending |
| Broker lifecycle | fixed sibling, manifests, current-user pipe, peer PID/liveness, timeout, close/shutdown | Focused tests present; packaged evidence pending |
| Read path | exact geometry, checked ranges, one-megabyte chunking, source change, 256-and-one-second cadence | Synthetic tests present; no real scan claim |
| Scanner | deterministic NTFS/FAT/partition candidates, partial truth and namespace ancestry | Focused tests present; external corpora pending |
| Desktop DTO/state | four commands, schema 1, decimal strings, 100-row pages, scan-bound cursors, 32 scopes/4 sessions | Focused tests present; final full gate pending |
| Frontend | fail closed, real inventory flow, folder cancel, scan/page flow, privacy, stale/duplicate control, a11y | 28-test suite reported during implementation; final frozen revision pending |
| Static safety | no source mutation, no physical-disk production authority, exact API/opcode/command allowlists, no real-device CI | Validator update and final run pending |
| Native package | both binaries, hashes, fixed sibling layout, extracted `asInvoker`/`requireAdministrator` manifests | Pending |
| Native UX | actual Tauri window, inventory only, required sizes, focus/zoom/forced colors | Pending |
| Remote | pushed revision and successful GitHub Actions conclusions | Pending |
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
  `DESKTOP-STATE-*`, `DESKTOP-REQUEST-ID-*`;
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
No component result may be generalized to signed production readiness.
