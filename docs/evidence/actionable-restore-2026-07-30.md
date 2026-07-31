# Actionable Results and Transactional Restore Evidence — 2026-07-30

## Evidence boundary

This record covers the Task 7 local verification of
[SDD-020](../specs/020-actionable-results-and-transactional-restore.md).
Product code and both release executables were built from:

- branch: `codex/actionable-restore`;
- product-code commit:
  `40402fa1ad38529ad9347f89ad839b0f292a72b1`;
- commit subject: `feat: add actionable recovery workspace`;
- platform: Windows x64;
- source worktree: clean at the start of the gate/build run.

The later documentation/validator reconciliation does not alter the product
binary inputs recorded here. This is local pre-release evidence. It is not a
signed release, clean-machine certification, endpoint-security approval or
proof of recovery from arbitrary real media.

## Required local gates

The following commands completed with exit code zero:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed in 1.8 seconds |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed in 2.1 seconds |
| `cargo test --workspace` | 51 result groups; 447 passed; 0 failed; 0 ignored; 0 measured; 0 filtered |
| `cargo test -p um-restore` | 81 passed; 0 failed; 0 ignored |
| `cargo test -p um-desktop --test restore_fixture` | 51 passed; 0 failed; 0 ignored |
| `pnpm --dir apps/desktop lint` | Passed with zero ESLint warnings |
| `pnpm --dir apps/desktop typecheck` | Passed |
| `pnpm --dir apps/desktop test` | 10 files; 146 passed; 0 failed |
| `pnpm --dir apps/desktop desktop:build` | Passed in 299.8 seconds, including the Vite production build and built-asset validator |
| `python -B .github/scripts/tests/test_validate_docs.py` | 33 passed in 184.249 seconds after the final current-state validator hardening |
| `python -B .github/scripts/validate_docs.py` | Passed; 58/58 FR entries, 8 NFRs, 11 foundation, 12 real-only and 13 connected-volume requirements |
| `python -B .github/scripts/tests/test_validate_ci_safety.py` | 27 passed |
| `python -B .github/scripts/validate_ci_safety.py` | Passed; 1 workflow and 122 first-party code/command surfaces reviewed |
| `python -B .github/scripts/tests/test_validate_real_only_desktop.py` | 98 passed |
| `python -B .github/scripts/validate_real_only_desktop.py` | Passed; 117 production-boundary files inspected |

The restore tests use deterministic repository fixtures, synthetic readers and
ordinary temporary destination directories. Windows reparse/junction
regressions use disposable temporary NTFS directories. No test writes to a
scan source, opens a real disk as test input, attaches/formats a VHD or performs
a destructive storage command.

Independent whole-branch review found no Critical or Important code/security
issue. It reconfirmed:

- all scan-source opens are read-only;
- `unsafe` remains confined to the audited `crates/io-windows` boundary;
- the broker binds peer PID/liveness and queries the already-bound process
  image before serving;
- destination operations remain capability-relative, no-clobber, journaled
  and bounded;
- the WebView receives opaque IDs and sanitized display data, not source or
  destination authority;
- cancellation and lost-job tracking fail closed;
- no production sample provider, fabricated candidate or simulated restore
  progress is present.

## Unsigned release pair

The release command produced exactly these two sibling product executables in
`target/release`:

| Artifact | Size | SHA-256 | Embedded execution level | Authenticode |
| --- | ---: | --- | --- | --- |
| `undelete-master-desktop.exe` | 10,738,688 bytes | `518cf0e1e5ba30622e88226ec1c6cbcc50a38cb140ca24a4b4abd3530e94c797` | `asInvoker`, `uiAccess=false` | `NotSigned` |
| `undelete-master-broker.exe` | 235,520 bytes | `51c788b78dcc0ea662325a1b6098655883794273d56fff755a48fab507401a9b` | `requireAdministrator`, `uiAccess=false` | `NotSigned` |

The execution levels above were extracted from PE resources with the Windows
SDK x64 Manifest Tool, not inferred only from source XML. Both extraction
commands exited zero. The two regular files shared the same parent directory,
and no other `undelete-master-*.exe` sibling was present.

The unsigned artifacts are suitable only for this local validation. The prior
Norton classification remains unresolved. Disabling endpoint protection is not
an accepted distribution or release procedure; protection should be restored,
and signing/reputation/clean-machine gates remain open.

## Bounded packaged observation

The real release desktop was launched unelevated four times for bounded
startup/visual inspection only. Each run:

- created a responsive `Undelete Master` top-level window;
- did not start `undelete-master-broker.exe`;
- rendered real mounted-volume inventory;
- closed normally through its main window.

The visual inspection showed the C: and E: mounted NTFS volumes. A
`PrintWindow` capture was inspected at 1,195 by 817 pixels with SHA-256
`468f00ac18e76c8cc5846812d8ef81bab14437182c6ea6ac0ec241847a4f852d`.
One keyboard interaction used `Tab`, `Tab`, `Enter` from application startup
to activate **Configurações**. The rendered Settings heading received visible
focus and the real language/theme/reduced-motion sections appeared. Its
1,195-by-817 `PrintWindow` capture had SHA-256
`dce6a799b97c691c4c6f1961ece1049e278107f40abc11dbdf0848185d949c79`.
Neither temporary capture is a retained repository artifact, so these
observations do not close packaged visual or accessibility acceptance.

Read-only Windows inventory at the same checkpoint reported:

| Disk | Bus | Partition style | Size | Mounted NTFS volume |
| ---: | --- | --- | ---: | --- |
| 0 | NVMe | GPT | 2,048,408,248,320 bytes | C: (`OS`) |
| 1 | USB | MBR | 1,000,204,886,016 bytes | E: (`SAMSUNG`) |

This proves only that two distinct physical disks and their mounted volumes
were observable. It does not prove that a destination picker binding or a
restore job used them.

## Deliberately unperformed real-media actions

No real C: or E: scan was started during Task 7. No file was created, deleted
or modified to manufacture recoverable evidence. No destination was selected,
no UAC broker session was started, no deleted-file restore ran and no Explorer
destination-open action was exercised.

Those omissions are intentional:

- the product owner had already observed a long-running scan with no scan
  progress/cancellation contract;
- ordinary tests are forbidden from using real scan sources;
- neither physical disk was an explicitly governed disposable deleted-data
  source;
- creating/deleting a test file on either source would change the source and
  could overwrite recoverable data;
- deterministic fixtures cannot be substituted for real-device acceptance.

The remaining acceptance scenario requires controlled known deleted data on a
separately governed source and a proven different physical NTFS destination.
It must verify the published file, sidecar when applicable and terminal
manifest hashes without writing to the source.

## Current disposition

The actionable native query/filter/sort/cursor/selection workspace, opaque
destination authority, immutable restore plan, bounded restore job,
transactional no-clobber publication, partial-file consent/evidence,
item/byte progress, cooperative restore cancellation, terminal manifest and
opaque open-destination action are implemented and locally exercised through
deterministic tests.

Status remains `Implemented-unverified`. The following do not yet pass as
product/release acceptance:

- governed real deleted-file recovery to a different physical NTFS disk;
- scan progress, heartbeat, ETA and cooperative scan cancellation;
- screen-reader, 200% zoom and Windows high-contrast acceptance;
- installer/protected placement, Authenticode, SBOM/licensing release review
  and endpoint-security disposition;
- clean-machine and remote GitHub Actions evidence;
- external forensic corpus and long-running performance evidence.
