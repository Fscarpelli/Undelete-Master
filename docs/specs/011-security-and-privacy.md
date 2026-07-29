# SDD-011 — Security and Privacy

Status: Normative; image-only desktop controls `Implemented-unverified`

## Protected assets

Source integrity, recoverable bytes, local paths, stable source identity,
result credibility, administrative privilege and untrusted parser/display
output.

## Implemented controls

- `SourceReader` has no write operation; `FileImageReader` opens regular images
  read-only and rejects non-files, final symlink/reparse objects, and every
  symlink/reparse ancestor.
- `crates/io-windows` is the only audited unsafe boundary. It calls only
  read-only `GetDriveTypeW` to classify an ordinary drive-letter root; mapped
  remote drives, unsupported roots, and classification failures fail closed.
- `crates/io-common` owns safe path traversal and read-only open. The Windows
  open does not follow a final reparse point and validates metadata from the
  opened handle.
- The desktop stays unelevated with an embedded Windows `asInvoker` manifest.
- The Rust-owned picker keeps the path outside WebView IPC; the command accepts
  only a bounded opaque request ID.
- Device/UNC/extended/NT namespace, alternate-stream, reserved-name, unsupported,
  empty and non-regular sources fail before scanning.
- The Tauri capability has `permissions: []`; no shell, filesystem, opener,
  HTTP, updater or process plugin is exposed to the WebView.
- CSP allows local assets and Tauri IPC only; no remote content is required.
- Production CSP remains restricted to local assets and Tauri IPC. The
  development-only CSP adds exactly `ws://localhost:1420` to `connect-src` for
  the fixed Vite live-reload endpoint. The dev-only Vite transform applies the
  same single exception to served HTML so the two enforced development
  policies agree; built production HTML remains WebSocket-free.
- Scanner work runs on `spawn_blocking`; report structure, warnings and payload
  are bounded.
- GPT validates a header and its entry table atomically. The backup is read only
  at the final logical block, must point reciprocally to LBA 1, must keep its
  entry array in reserved metadata space, and both canonical copies are always
  evaluated. Two valid headers must agree on reciprocal geometry/identity or
  discovery fails closed. Primary read, header, or table failure permits an
  independently valid canonical backup. Protective `0xEE` MBR entries are
  never exposed when neither GPT copy is usable.
- NTFS caps the allocation `$Bitmap` read and backing allocation at 64 MiB.
  Unretained suffix bits remain unknown. An unaligned `$MFT` logical size or one
  covering fewer than 16 reserved records is corrupt; a nonblank record without
  a `FILE` signature, malformed ordinary/extension-record attributes, and other
  known MFT coverage boundaries are structurally reported as `partial`.
  Any `$ATTRIBUTE_LIST`, including a well-formed resident value, remains
  `partial` until all references and candidate-defining attributes are proven
  resolved. Malformed record-0 attributes remain fatal because they invalidate
  the MFT bootstrap.
- FAT accepts one through four total declared copies and keeps the first
  authoritative. It reports `partial` when any declared secondary copy
  disagrees or is unreadable, or directory-chain/cycle/start-cluster/read
  problems or count/byte/depth/chain limits prevent exhaustive traversal. The
  chain limit is applied before directory cluster reads. A declared FAT table
  above 64 MiB is rejected before allocation. Its observed partial candidate
  count is not relabeled as complete.
- Parser fallthrough occurs only on `NotRecognized`. A recognized volume's
  typed `Read` or `Corrupt` failure is preserved; the Tauri boundary maps those
  conditions to sanitized `SOURCE_IO` or `SCAN_CORRUPT`.
- Errors use stable codes and generic messages. Full paths, raw OS errors,
  source bytes, partition-read offsets and backtraces do not cross IPC.
- Labels and warnings are rendered as text. The Rust adapter removes control
  characters and Unicode bidi formatting controls; the source heading also
  uses `<bdi dir="auto">` to isolate display direction.
- The browser build fails closed and has no synthetic provider.

These controls have focused coverage in `WINDOWS-DRIVE-TYPE-001`,
`WINDOWS-DRIVE-ROOT-001`, `IO-REMOTE-PATH-001`,
`IO-ANCESTOR-REPARSE-001`, `DESKTOP-REPARSE-GUARD-001`,
`PART-GPT-BACKUP-001` through `PART-GPT-BACKUP-005`,
`PART-GPT-PROTECTIVE-001`, `NTFS-BITMAP-BOUND-004`,
`NTFS-MFT-SIZE-001`, `NTFS-RECORD-SIGNATURE-001`,
`NTFS-ATTRIBUTE-BOUNDS-001`, `NTFS-ATTRIBUTE-LIST-PARTIAL-001`,
`FAT-COMPLETENESS-001` through `FAT-COMPLETENESS-004`,
`FAT-TABLE-BOUND-001`, `FAT-DEPTH-BOUND-001`,
`FAT-DIRECTORY-CHAIN-BOUND-001`,
`CLI-IMAGE-FAT-PARTIAL-001`, `CLI-PROBE-ERROR-PRIVACY-001`,
`DESKTOP-VOLUME-SCAN-ERROR-001`, `DESKTOP-DEV-CSP-001`,
`DESKTOP-DEV-CSP-002`,
`DESKTOP-PARTITION-READ-ERROR-001`, `DESKTOP-BIDI-TEXT-001`, and
`DESKTOP-BIDI-ISOLATE-001`. Frozen-code Rust/frontend gates pass; native,
package, external-corpus, and remote results remain pending.

## Privacy

No account, cloud, analytics or telemetry is required. Preferences persist only
language, theme and reduced motion. Source paths and reports are ephemeral and
are not stored by the current desktop.

## Residual path-identity boundary

The picker, `GetDriveTypeW` classification, ancestor metadata checks, and final
open are separate pathname operations. The final handle check closes a final
symlink/reparse substitution, but the current design does not retain handles
for every ancestor or bind picker selection to a stable file/volume identity.
It therefore reduces common redirection risk but is not race-free and is not a
forensic chain-of-custody guarantee. See
[ADR-0022](../adr/0022-windows-locality-boundary-and-path-identity.md).

## Independent review boundary

The sealed Codex Security snapshot documented in the
[2026-07-29 evidence record](../evidence/real-only-desktop-2026-07-29.md)
has `34/34` unique review receipts, three technically valid candidates with
final policy decisions of `ignore`, and zero reportable findings. The result is
bounded to that snapshot and reportability policy. It does not prove absence of
vulnerabilities, attest later edits, eliminate the retained TOCTOU risks, or
resolve endpoint-security reputation.

## Future controls

Physical devices require a separate audited read broker. Restore requires
destination containment and a write process separate from source reading.
Preview requires a restricted networkless worker. Strong source identity
requires retained ancestor/source handles and deterministic race tests.
Signing, SBOM, clean-machine evidence, and resolution of the Norton
classification for the unsigned development build remain release gates.
