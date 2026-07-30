# Connected-storage Security Review — 2026-07-30

Status: complete static review; production release remains blocked

Reviewed snapshot:
`codex-security-snapshot/v1:sha256:cab2988d9b74e58b709880d7d5b7859700c68e6538712e8f079305d4abd2ab1c`

Formal scan ID: `aafbf084-264a-4849-af3b-973580d0275e`

## Scope and method

The review covered the connected mounted-volume inventory, optional NTFS
folder scope, unelevated Tauri desktop, elevated read-only broker, protocol v2,
broker-backed `SourceReader`, NTFS/FAT scanners and real-only results UI in the
working-tree increment.

The scan completed 44 of 44 deterministic discovery work items. Every candidate
received a focused validation receipt and an independent source-to-sink attack
path review. Four candidates entered validation; three survived as reportable
findings. The prepared exact-volume-clone candidate remained visible in
coverage but was policy-rejected because its demonstrated path required
physical media control or operator cooperation outside the normal attacker
model.

No real volume was opened or scanned. No UAC prompt, malicious executable
replacement, reparse-point race or source mutation was executed. Synthetic
fixtures and static source evidence were used. Untracked user-owned
`crates/fs-exfat` work was excluded.

## Findings

| ID | Finding | Severity | Confidence | Release disposition |
| --- | --- | --- | --- | --- |
| UM-BROKER-SIBLING-SUBSTITUTION-001 | An unsigned fixed-sibling broker in a user-writable package can be replaced before the desktop invokes `runas`. | Medium / P2 | High | Block public release until package identity, signing and protected placement are enforced. |
| UM-BROKER-RELOCATED-PEER-001 | A relocated genuine broker derives trust from its current directory and can accept an attacker-chosen desktop sibling. | Medium / P2 | High | Use one independent same-revision package-authentication contract in both processes. |
| UM-FOLDER-ANCESTRY-TOCTOU-001 | Folder ancestry is checked by pathname before a later independent open, leaving a same-volume redirection race. | Low / P3 | High | Replace check-then-reopen validation with a retained handle-relative no-reparse walk, or move folder selection to the reconstructed NTFS namespace. |

The first two findings are two sides of one missing package-identity root. The
current PID, liveness, canonical sibling-path, nonce and bounded-protocol
controls remain useful defense in depth, but they do not authenticate publisher
or package revision and do not make a writable directory trustworthy.

The folder finding does not authorize a source write and does not escape the
selected mounted volume. It can, however, cause a different ordinary directory
to become the accepted identity during a carefully timed race, so folder scope
must not be described as race-free.

## Reviewed surfaces without an additional finding

- The broker protocol exposes exactly ten bounded messages, one opaque source,
  contiguous sequence numbers and no write, destination or generic device
  control.
- The source boundary opens mounted volumes with `GENERIC_READ` only and
  revalidates identity, mapping and geometry from the live handle.
- NTFS namespace work is bounded before saturated sibling recursion; omitted
  ancestry remains partial and `Unknown`.
- The React/Tauri production surface uses four real commands, opaque IDs,
  bounded text and no image picker, sample storage, fabricated progress or
  browser fallback.
- FAT, CLI and supporting diff surfaces introduced no additional credible
  source-write or parser-boundary issue in this review.

## Hardening decision record

Two independent opportunities were derived from the accepted finding set:

1. **Authenticate the desktop/broker package boundary.** Prefer an on-demand
   signed package capability while explicit UAC and a small privileged
   footprint remain requirements. The desktop should elevate only a held,
   signed, same-revision broker from an administrator-protected location; the
   broker should independently prove that its live desktop peer belongs to that
   package. An installer-owned broker service is the alternative if enterprise
   deployment or repeated scans justifies a persistent privileged lifecycle.
2. **Bind folder selection to stable object identity.** Prefer a retained,
   handle-relative, no-reparse component walk whose final handle is the only
   source of `FolderScope`. Selecting a directory from the reconstructed NTFS
   namespace after scanning is the alternative if the product can change when
   folder filtering occurs.

These are proposals, not completed remediation. They do not weaken the current
release blocks.

## Required release gates

Public distribution remains blocked until the same revision has:

- independent Authenticode and publisher verification for both executables;
- same-revision package binding and administrator-protected placement;
- a clean-machine result for the exact hashes;
- vendor disposition for the endpoint-security event;
- successful local and GitHub Actions quality gates;
- retained native manifest, accessibility and artifact evidence.

Norton’s deletion of an unsigned development executable and the later
`Access is denied (os error 5)` release-build failure remain inconclusive. The
security review proves neither malware nor a false positive.

## Traceability

- Architecture and accepted boundary:
  [ADR-0023](../adr/0023-windows-read-only-broker-and-folder-scope.md)
- Normative requirements and release gates:
  [SDD-018](../specs/018-windows-volume-and-folder-scan.md)
- Threat analysis:
  [Threat model](../threat-model/THREAT_MODEL.md)
- Risk ownership and residual risk:
  [Risk register](../risk-register.md)
- Build and native acceptance evidence:
  [Connected-volume evidence](windows-volume-scan-2026-07-29.md)

The sealed security snapshot preceded the narrow per-monitor-DPI manifest and
SDD-018 validator-governance follow-up. Those later changes alter presentation
metadata and documentation enforcement, not the broker/source dataflow, and
were separately covered by focused regressions and the final full gate run.
