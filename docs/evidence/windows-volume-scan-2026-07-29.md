# Connected-volume Desktop Evidence — 2026-07-29

Status: Open evidence record; no release conclusion

Revision under evaluation: `codex/foundation-hardening`

## Scope

This record covers SDD-018 and ADR-0023:

- unelevated mounted-volume inventory in logical groups with no physical disk
  number or DASD/IOCTL claim;
- optional identity-bound NTFS folder scope;
- short-lived elevated read-only broker;
- protocol v2 and broker-backed `SourceReader`;
- real candidate summary and bounded pages;
- replacement of the desktop image picker.

The regular-image CLI is outside this supersession and remains supported.

The completed connected-storage security review, findings and hardening
decisions are retained in
[the 2026-07-30 security evidence](security-review-2026-07-30.md).

## Claims intentionally not made

- No whole-physical-disk or unmounted-partition scan is implemented.
- No real mounted volume was scanned for this evidence.
- No cancellation, hotplug subscription or snapshot is implemented.
- No restore, preview, carving, exFAT or persistent session is implemented.
- No signed, clean-machine or production-ready artifact exists yet.
- Norton’s deletion of an unsigned development build remains inconclusive.

## Implemented source evidence

| Area | Source evidence | Disposition |
| --- | --- | --- |
| Inventory | mounted roots, stable GUID+serial ID, logical groups with null disk number; size/free are quota-visible display only | Implemented; final gates pending |
| Folder authority | volume serial plus NTFS record/sequence; path confined to native Rust | Implemented; native acceptance pending |
| Process split | main `asInvoker`; fixed sibling broker `requireAdministrator` | Implemented in source; extracted manifests pending |
| Protocol | version 2, ten messages, 20-byte header, 1 MiB payload | Implemented; final full gate pending |
| Peer binding | current-user pipe, launched peer PID/liveness, exact nonce echo | Implemented; package/signature binding pending |
| Source access | broker-only canonical length/extents/multidisk check, then `GENERIC_READ`, `OPEN_EXISTING` | Implemented; no live scan performed |
| Revalidation | both 256 valid reads and one elapsed second; each accepted read still uses `ReadFile` | Implemented; synthetic evidence only |
| Folder filtering | `Match`/`NoMatch`/`Unknown`; only `Match` rows; unknown separate | Implemented; external corpus pending |
| WebView | four commands, opaque IDs, decimal-string `u64`, 100-row pages | Implemented; production-bundle inspection pending |
| Real-only UI | no production image picker, fabricated inventory/result/progress or browser fallback | Implemented; final static/native inspection pending |
| Native DPI | manifest `true/pm` plus `PerMonitorV2, PerMonitor`; 144-DPI native WebView/UIA both 1770×1170 | Implemented; 150% acceptance passed |

## Gate ledger

| Gate | Result | Required retained evidence |
| --- | --- | --- |
| `cargo fmt --all -- --check` | Passed | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed | exit 0 |
| `cargo test --workspace` | Passed | 223 passed, 0 failed, 0 ignored; doc-tests passed |
| frontend lint/typecheck/test/build | Passed | lint/typecheck exit 0; Vitest 6 files and 29/29 tests |
| documentation validator | Passed | exact catalog/status/path checks passed |
| CI-safety validator | Passed | one workflow and 92 storage-safety surfaces passed |
| real-only desktop validator | Passed | 88 production files passed |
| validator regression suite | Passed | 71 tests passed; 0 failures, errors or skips |
| release main/broker build | Partial | release broker built; release desktop blocked by `schemars` link/copy access denied |
| manifest extraction | Partial | debug desktop contains `asInvoker`, `true/pm` and `PerMonitorV2`; broker source/build tests require `requireAdministrator` |
| actual Tauri visual/accessibility review | Passed at 150% | 144 DPI; native WebView and UIA root both 1770×1170; ratio 1.0000×1.0000 |
| read-only live inventory | Passed without scan | two supported local NTFS volumes observed; labels omitted |
| real-volume scan | Not executed and not required | no claim |
| remote GitHub Actions | Pending | commit and workflow URL |
| Authenticode/clean machine/vendor disposition | Pending | exact artifact hash and results |

## Local artifact ledger

These are unsigned development artifacts, not release candidates:

| Artifact | Size | SHA-256 | Manifest evidence | Disposition |
| --- | ---: | --- | --- | --- |
| `target/debug/undelete-master-desktop.exe` | 14,752,768 bytes | `8A2076FE6A3EA15A15C874D2380F5D4D6336BBDB3960C1DA888AB7E8278BD52D` | embedded `asInvoker`, `true/pm`, `PerMonitorV2, PerMonitor` | Debug build passed; unsigned |
| `target/debug/undelete-master-broker.exe` | 530,432 bytes | `E94B7770891FFA784D46D2A7910740337703C51346F7192C5C6BA1F2E181B2DD` | embedded `requireAdministrator` | Debug build passed; unsigned |
| `target/release/undelete-master-broker.exe` | 234,496 bytes | `3CEAE9AEA4F0DE1E88FAF5215FACE059D0EBE668E88FC3868874D6269FDA10E9` | embedded `requireAdministrator` | Release broker build passed; unsigned |
| `target/release/undelete-master-desktop.exe` | — | — | — | Not produced; `schemars` link/copy failed with `Access is denied (os error 5)` |

The exact release failure repeated after the user temporarily disabled Norton.
The source `schemars` build-script executable is readable, the requested
destination alias is absent, inherited ACLs allow modification and no relevant
Defender, Code Integrity, Application or System event names the target. This
does not rule a security filter in or out and does not justify bypassing one.

## Safety boundary

All deterministic tests must use synthetic in-repository images/readers or
disposable ordinary temporary files/directories. They must not open a real disk
or volume, attach a VHD or invoke a storage mutation command.

If the native app is launched for UI verification, inventory may load
read-only. The reviewer must not activate “scan” against an observed real
volume.

## Norton event

Norton deleted an unsigned local executable on 2026-07-29. The user temporarily
disabled Norton to permit a rebuild. Debug desktop and debug/release broker
builds succeeded, but the release desktop still failed while linking or copying
the `schemars` build script with Windows `Access is denied (os error 5)`. This
record does not attribute that denial or classify the deleted artifact.
Endpoint protection should be restored after the local exercise; public
distribution remains gated on investigation, signing and exact-artifact
evidence.
