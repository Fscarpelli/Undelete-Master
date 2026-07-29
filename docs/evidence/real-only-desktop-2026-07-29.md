# Real-only Desktop Evidence — 2026-07-29

Status: Partial. A sealed security-review snapshot, the final frozen-code local
command results, and an unsigned local native-launch smoke are retained below.
Packaged/installer acceptance, native visual and assistive-technology review,
external corpora and real-media compatibility, remote CI, signing, and vendor
disposition remain pending.

## Scope and evidence boundary

This record covers the narrow image-only desktop increment. It separates:

1. evidence completed for a specific sealed security-review snapshot;
2. implementation and regression surfaces visible after that snapshot;
3. local gates completed on the later frozen code tree;
4. the local unsigned native-launch smoke completed after the Norton deletion;
5. package, native-visual, external-corpus, remote-CI, and release checks that
   remain open.

It is not a production-release approval and does not authorize use against
physical disks.

## Completed sealed security review

| Field | Recorded value |
| --- | --- |
| Scanner | Codex Security |
| Scan ID | `ebdb9b62-f405-4e5b-837a-f58937dec74b` |
| Sealed at | `2026-07-29T21:26:36.951973Z` |
| Mode | Working-tree, diff-focused review |
| Base/head | `3b08141ae521fd0992bac06ef2f79eb0c576cff6` / `3b08141ae521fd0992bac06ef2f79eb0c576cff6` |
| Snapshot digest | `codex-security-snapshot/v1:sha256:b48534c24ce25c4d2799081366d5d970402bbcc226fd6f460be806c89f34a0b3` |
| Review receipts | `34/34` unique receipts |
| Technically valid candidates | 3 |
| Final candidate decisions | 3 `ignore` |
| Reportable findings | 0 |
| Local report SHA-256 | `F4DA89BCED64BDF8CB1295182351DE77CF4E3D76B56191782138D7BCA50381FC` |

The report is stored at this local, non-portable temporary path:

```text
C:\Users\fscar\AppData\Local\Temp\codex-security-scans-nnlDPh\Undelete-Master\3b08141ae521fd0992bac06ef2f79eb0c576cff6_20260729T205644Z_4tn30ko4\report.md
```

Temporary-file cleanup may remove that path. The measured report hash above
allows a later copy to be compared, but no durable repository copy is claimed.

## Hashes and artifact identity

- Sealed snapshot:
  `codex-security-snapshot/v1:sha256:b48534c24ce25c4d2799081366d5d970402bbcc226fd6f460be806c89f34a0b3`.
- Sealed review base/head:
  `3b08141ae521fd0992bac06ef2f79eb0c576cff6`.
- Local `report.md` SHA-256:
  `F4DA89BCED64BDF8CB1295182351DE77CF4E3D76B56191782138D7BCA50381FC`.
- Locally executed unsigned Tauri development artifact, not a release approval:
  `target/norton-retry-20260729/release/undelete-master-desktop.exe`, 9,438,720
  bytes, last written `2026-07-29T22:53:40.8186288Z`, SHA-256
  `9B08FBEAAE974FE09D2DBF77483E438041B55609E632525A96CE6FBBB86BA722`,
  Authenticode `NotSigned`.
- No signed, packaged, distributable product artifact is accepted, so no
  release-artifact hash is claimed.

### Technically valid candidates

| Candidate | Technical result | Final scan decision | Repository treatment |
| --- | --- | --- | --- |
| `UM-IO-TOCTOU-ANCESTOR-SWAP-001` | Pathname ancestor replacement is structurally possible between checks and open. | `ignore` under the scan's local single-operator reportability policy. | Retained as residual risk in ADR-0022, threat model, SDD-011/017, and R-013. |
| `UM-TAURI-TOCTOU-001` | Picker selection is not bound to the later open by a retained file identity. | `ignore` under the same reportability policy. | Retained as an open path-identity limitation and future retained-handle trigger. |
| `UM-TAURI-BIDI-001` | The sealed snapshot allowed Unicode bidi formatting to survive label sanitization. | `ignore` under the scan's one-operator display-impact policy. | Later code removes bidi formatting controls and directionally isolates the source heading; frozen-tree regressions pass, while native visual acceptance remains pending. |

Zero reportable findings means that no candidate survived the scan's final
reportability gates. It does not mean that vulnerabilities are absent. The
three technically valid candidates remain useful security evidence even though
the scanner's final policy decision was `ignore`.

## Post-snapshot implementation surfaces

These source/test surfaces were observed after the sealed snapshot. They are
not attested by that security scan. The later frozen-tree command suite verifies
their compilation, lint, and regression paths, but does not retroactively
extend the sealed scan's attestation:

- `crates/io-windows` contains the minimal `GetDriveTypeW` locality boundary;
- `crates/io-common` rejects mapped-remote classification and reparse
  ancestors, then performs a read-only final open and handle metadata check;
- `IO-ANCESTOR-REPARSE-001` and `DESKTOP-REPARSE-GUARD-001` use disposable
  temporary junctions, never real disks;
- `DESKTOP-CLI-PARITY-003` compares direct CLI and Tauri GPT/NTFS reports and
  asserts unchanged fixture SHA-256;
- `DESKTOP-BIDI-TEXT-001` removes bidi formatting controls and
  `DESKTOP-BIDI-ISOLATE-001` verifies `<bdi dir="auto">`; both are included in
  the passing frozen-tree workspace/frontend suites;
- `DESKTOP-PARTITION-READ-ERROR-001` verifies that partition-read failures omit
  raw details and offsets.

No literal fixture hash is recorded here because the tests assert before/after
equality rather than a separately retained release-evidence value.

## Test evidence

### Final frozen-code local gates

The coordinator froze the code tree before these commands. Subsequent changes
in this documentation reconciliation do not change the tested Rust or frontend
sources.

| Command | Result | Boundary |
| --- | --- | --- |
| `cargo fmt --all -- --check` | Exit 0. | Final frozen-code workspace format gate passed. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0. | Final frozen-code workspace lint gate passed with warnings denied. |
| `cargo test --workspace` | Exit 0; 147 non-doc tests passed. | Final frozen-code deterministic workspace gate; synthetic images and disposable temporary paths only. |
| `pnpm lint` in `apps/desktop` | Exit 0. | Final frozen-code frontend lint gate passed. |
| `pnpm typecheck` in `apps/desktop` | Exit 0. | Final frozen-code strict TypeScript gate passed. |
| `pnpm test` in `apps/desktop` | Exit 0; 28 tests passed. | Final frozen-code frontend regression gate passed. |
| `pnpm build` in `apps/desktop` | Exit 0. | Frontend production build passed; this is not a packaged Tauri executable. |
| `pnpm tauri build --no-bundle` with `CARGO_TARGET_DIR=target/norton-retry-20260729` | Exit 0; production `custom-protocol` executable rebuilt after the Norton deletion. | Local unsigned development artifact only; no installer, signing, clean-machine, or vendor acceptance is implied. |
| Local native-launch smoke | PID `47940`, `Responding=True`, window title `Undelete Master`, exact executable path matched the hash above. The launching identity was at Windows medium mandatory integrity (`S-1-16-8192`); the source manifest is `asInvoker`. | Confirms local process liveness and an unelevated launch context, not a fixture scan, extracted-manifest attestation, visual review, or release approval. |
| `python -B -m unittest discover -s .github/scripts/tests -p "test_*.py" -v` | Exit 0; 52 tests passed. | Validator regression suite passed on the reconciled tree. |
| `python .github/scripts/validate_docs.py` | Exit 0: 58/58 exact FR catalog/matrix, 8 NFR, 11 foundation requirements, 12 real-only requirements, 17 required ADR topics, 27 formal justifications, statuses/test references/local paths checked. | Passed after the final documentation reconciliation. |
| `python -B .github/scripts/validate_real_only_desktop.py` | Exit 0; 65 production-boundary files inspected. | Source/configuration production-boundary inventory passed; this is not native visual or installer inspection. |
| `python -B .github/scripts/validate_ci_safety.py --root .` | Exit 0; 1 workflow and 72 enumerated first-party code/command surfaces inspected. | Static defense-in-depth check passed; no remote Actions result is implied. |
| `cargo test -p um-cli --test scan_image cli_process_json_001_emits_machine_readable_json -- --exact` | Exit 0; 1 process-level deterministic image test passed. | The test asserts machine-readable output and unchanged synthetic fixture SHA-256; no real disk was opened. |
| `git diff --check` | Exit 0. | No whitespace errors; Git emitted only expected LF-to-CRLF working-copy warnings. |

### Earlier focused evidence

| Command | Result | Boundary |
| --- | --- | --- |
| `cargo test --locked -p um-desktop` | Exit 0; 22 tests passed. | Focused local desktop result reported by the desktop hardening worker; not the workspace gate. |
| `cargo check --locked -p um-desktop --tests` | Exit 0. | Focused local desktop result; not the workspace gate. |
| `cargo clippy --locked -p um-desktop --all-targets -- -D warnings` | Exit 0. | Focused local desktop result; not the workspace gate. |
| `cargo fmt --package um-desktop -- --check` | Exit 0. | Focused local desktop result; not the all-workspace format gate. |
| `pnpm test -- AnalysisView.test.tsx App.test.tsx` | Exit 0; 2 files and 10 tests passed. | Focused frontend result in `apps/desktop`; not the complete frontend test gate. |
| `pnpm typecheck` | Exit 0. | Earlier focused frontend result; superseded by the passing frozen-code frontend gate above. |
| Focused ESLint invocation | Exit 0. | Exact focused argument vector was not retained; superseded by the passing full `pnpm lint` gate above. |
| Python policy-guard tests | Exit 0; 11 of 11 tests passed. | Focused local validator-unit result. |
| Real-only source validator | Exit 0; 65 files inspected. | Earlier focused result; superseded by the final reconciled-tree run above. |
| CI-safety validator | Exit 0; 1 workflow and 71 surfaces inspected. | Earlier focused result; superseded by the final 72-surface run above. |
| `um-cli` focused tests | Exit 0; 12 tests passed. | Focused local result; the full workspace suite now also passes, while a retained standalone CLI smoke artifact remains pending. |

### Remaining acceptance and release gates

| Gate | State | Required retained evidence |
| --- | --- | --- |
| Packaged unelevated Tauri fixture scan | Pending | Executable identity/hash, fixture identity/hash, output |
| Production bundle/command/capability inspection | Source/configuration and built frontend asset checks passed; packaged installer inventory pending | Same-revision package inventory and command log |
| Native visual and accessibility review at 1440 × 900 and 1100 × 700 | Pending; native Computer/Browser capture was not callable in the current environment | Screenshots, console state, reviewer result |
| External forensic corpora and explicitly allowlisted disposable-media acceptance | Pending | Corpus/media provenance, hashes, commands, outcomes; never a real scan source outside the allowlist |
| GitHub Actions pull-request run | Pending | Workflow URL, commit, job conclusions |
| Final diff, secret/generated-artifact review | Passed for the staged delivery tree | `git diff --cached --check` exited 0; no generated build tree, environment file, key/certificate path, or high-confidence secret signature was staged. The user-owned untracked `crates/fs-exfat/` directory was explicitly excluded. |
| Signed clean-machine release and Norton classification | Pending | Authenticode result, artifact hash, vendor disposition |

The unsigned local development executable hash is recorded for reproducibility,
but no release-artifact hash is claimed because no signed package has passed
the remaining gates.

## Known evidence limitations

- The sealed scan is bound to its recorded snapshot and does not attest later
  code or documentation edits.
- A security review with zero reportable findings does not prove absence of
  vulnerabilities.
- The scan did not complete deterministic concurrent ancestor, drive-mapping,
  or picker-to-open race reproduction; its confined attempt encountered an
  MSVC `LNK1104` before product code ran.
- Synthetic fixtures and disposable junctions do not establish compatibility
  with every Windows namespace or hostile real-world image.
- The current development executable is unsigned. Norton deleted one local
  build on 2026-07-29; the classification remains unresolved. The Codex
  Security result neither proves a false positive nor establishes that the
  artifact was malicious.
- Device access, restore, preview, signing, SBOM, clean-machine validation, and
  external forensic corpora remain outside completed acceptance.
