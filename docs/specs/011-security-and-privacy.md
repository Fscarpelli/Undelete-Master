# SDD-011 — Security and Privacy

Status: Normative; connected-volume controls `Implemented-unverified`

## Protected assets

- integrity and recoverability of every scan source;
- source identity during selection, elevation and reads;
- local paths, device names, pipe names and raw source bytes;
- administrative privilege and broker process identity;
- credibility of candidate and folder-membership claims;
- availability under malformed frames and hostile filesystem metadata;
- the unelevated desktop and user environment.

## Implemented control design

### Never-write source boundary

`SourceReader` exposes no mutation method. The broker protocol contains no
write, destination, path, access-mask or generic control field. The selected
mounted volume is opened internally with exactly `GENERIC_READ` and
`OPEN_EXISTING`; all offsets and lengths are checked before I/O.

The only allowed storage IOCTLs query volume extents, disk length and storage
alignment. No trim, format, delete, repair, lock, dismount or mount operation is
present. Duplex `GENERIC_WRITE` belongs only to named-pipe transport, never a
scan-source handle.

### Privilege separation

The desktop manifest is `asInvoker`; the fixed sibling broker manifest is
`requireAdministrator`. Inventory and folder selection stay unelevated. The
broker has no WebView and imports no partition or filesystem parser. Source
bytes return to the unelevated scanner and never to JavaScript.

The broker exists only for a scan session. Fixed sibling resolution rejects an
arbitrary executable path. A restrictive one-instance local pipe admits the
launched broker PID; peer PID and liveness are the primary identity checks.

### Protocol boundary

Protocol v2 has exactly ten messages, a fixed 20-byte header, monotonic
independent sequences, bounded opaque identifiers and at most a 1 MiB payload.
One session opens at most one source.

The client generates a 32-byte nonce and checks the exact `HelloAck` echo in
constant time. The echo is not a MAC, shared secret or proof of publisher
identity. Package location, native peer PID and future signature evidence carry
those responsibilities.

Unknown versions/opcodes, sequence gaps/replays, malformed fields, invalid
sector geometry, zero/stale handles, overflows, out-of-range reads and
oversized payloads fail closed. Broker errors are stable codes without
free-form native diagnostics.

### Identity and mutable-source boundary

Unelevated inventory is display metadata. Its stable volume ID uses GUID plus
serial and excludes mount, quota-visible size/free data, filesystem, extents
and disk number. The broker independently derives disk/volume mapping,
canonical length and sector geometry, rejects composite mappings and verifies
the opened source.

Every valid `ReadAt` invokes a revalidation hook. Expensive Windows
re-enumeration occurs only when both 256 valid requests and one second have
elapsed; actual `ReadFile` still occurs for every accepted request unless
identity validation first fails. This limits enumeration overhead but does not
create a snapshot. A live volume may change during a scan.

### Folder identity

The absolute selected folder path remains native. Folder scope is NTFS-only and
requires the selected volume serial plus exact MFT record and reuse sequence.
Remote, reparse and cross-volume selection fails closed.

The scanner resolves the same active directory identity before filtering.
Candidate ancestry is trivalent: `Match`, `NoMatch` or `Unknown`. Only
`Match` crosses the result boundary. Unknown ancestry is counted separately;
path-prefix text is never containment authority.

### WebView and data minimization

The command registry contains exactly inventory, native-folder selection,
volume scan and candidate-page operations. Commands accept bounded request IDs
and opaque IDs only. Tauri capabilities grant no plugin permission; the browser
runtime fails closed.

All exposed `u64` values are canonical decimal strings. Native inventory,
folder authorities, scan sessions, warnings, text and pages are bounded.
Candidate pages contain at most 100 rows and use scan-bound opaque cursors.

Recovered paths, labels and warnings are untrusted. Native adaptation removes
control and Unicode bidi-formatting characters; React renders text and
directionally isolates recovered paths. Directory rows have no fabricated
recoverability score.

No account, cloud, analytics or telemetry is required. Preferences persist only
language, theme and reduced-motion choices. Scan authorities and results remain
in bounded memory and are not persisted.

## Parser controls

Parsers remain unelevated and preserve the existing checked-arithmetic,
region-bounds and work-limit rules. A recognized filesystem `Read` or `Corrupt`
failure is not hidden by another parser. NTFS/FAT partial coverage and warnings
survive the desktop boundary; candidate counts are not described as
exhaustive when coverage is partial.

## Explicitly absent security claims

- no snapshot or chain-of-custody guarantee;
- no cancellation or hotplug-subscription guarantee;
- no restore destination containment or preview sandbox;
- no whole-physical-disk, unmounted, composite or remote scan authority;
- no exFAT, carving or recovered-content execution;
- no proof from unit tests that a hostile real disk is safe;
- no signed installer, clean-machine or endpoint-vendor approval.

## Verification boundary

Focused deterministic tests exist for protocol schema/framing, nonce mismatch,
sequence replay, broker lifecycle, read bounds/chunking, source change, mounted
volume policy, folder identity, namespace ancestry, DTO bounds, pagination,
privacy and browser fail-closed behavior.

Final same-revision Rust/frontend/static gates, native build and manifest
inspection, actual Tauri UX/accessibility, remote CI, signing, clean-machine and
endpoint-security evidence remain pending. No real source scan has been
executed as validation.

Norton deleted one unsigned development executable on 2026-07-29. Its
classification remains inconclusive and must not be dismissed as a confirmed
false positive. Disabling endpoint protection is not a release control.
