# Threat Model

Status: Current for SDD-018; final native/package/security evidence pending

## Scope

This model covers the unelevated Tauri desktop, mounted-volume inventory,
native NTFS folder authority, protocol-v2 broker client, short-lived elevated
broker, read-only mounted-volume source, unelevated partition/filesystem
parsers, candidate adaptation and React rendering.

The image CLI remains a separate read-only path. Restore, preview, carving,
exFAT, persistent sessions, whole-physical-disk scans and unmounted sources are
absent.

## Data flow and trust boundaries

```mermaid
flowchart LR
    UI["React WebView"] -->|"requestId + opaque IDs"| Facade["Four Tauri commands (asInvoker)"]
    Facade -->|"query only"| Inventory["Mounted local volume inventory"]
    Facade -->|"native picker; path retained in Rust"| Folder["NTFS folder authority"]
    Facade -->|"fixed sibling + CSPRNG values"| Pipe["Current-user local pipe"]
    Pipe -->|"peer PID/liveness + fixed sibling image + protocol v2"| Broker["Elevated read-only broker"]
    Broker -->|"GENERIC_READ / OPEN_EXISTING"| Volume["Selected mounted volume"]
    Broker -->|"bounded bytes"| Scanner["Unelevated partition + NTFS/FAT scanner"]
    Scanner -->|"namespace evidence"| Filter["Match / NoMatch / Unknown"]
    Filter -->|"sanitized summary + 100-row pages"| UI
```

The logical group is outside source authority and has no physical disk number.
Unelevated inventory uses no DASD/IOCTL mapping. Production never opens
`PhysicalDriveN`.

Primary trust boundaries are WebView/native IPC, unelevated/elevated pipe,
display inventory/authoritative open, mutable volume/read session, native
folder/NTFS namespace, untrusted metadata/DTO and DTO/React rendering.

## STRIDE register

| Threat | Boundary | Impact | Current mitigation | Residual / required evidence | Status |
| --- | --- | --- | --- | --- | --- |
| Spoofed broker peer | Native client to pipe | Privileged read oracle or redirected session | Fixed sibling, restrictive one-instance pipe, bidirectional PID/liveness, broker-side `QueryFullProcessImageNameW` equality with the canonical fixed desktop sibling, v2 sequences and exact nonce echo | Image-path equality reduces confused-deputy exposure but is not publisher/package authentication; Authenticode, protected-directory and same-revision native evidence remain pending | Open |
| Broker binary replacement | Package to process launch | Attacker receives elevated execution | The launcher derives the fixed broker sibling internally; the genuine broker checks the desktop sibling image before service; no UI executable path | A replaced broker can ignore its own peer check, and a user-replaceable sibling directory defeats path-only identity; Authenticode, hash binding, protected installation and clean-machine evidence pending | Open |
| Protocol replay/confusion | Pipe framing | Wrong command/result applied | Fixed 20-byte header, exact v2, independent contiguous sequences, closed ten-message schema | Final full suite and fuzz campaign pending | Mitigating |
| Privilege expansion | Protocol/native boundary | Write, arbitrary path or generic control under elevation | No mutation/path/access-mask/control field; three query-only IOCTLs; scan source uses `GENERIC_READ` | Static import/access-mask review and built-binary evidence pending | Open |
| Stale volume substitution | Inventory to open/read | Wrong source scanned | Stable GUID+serial ID; UAC; independent broker canonical length/extents/mapping checks on the currently mounted source | An exact GUID+serial clone is indistinguishable at first open; post-open extents/length/geometry become that source's baseline; live volume remains mutable and no snapshot is claimed | Open |
| Identity change between reads | Mutable volume | Mixed or misleading result | Re-enumerate after both 256 valid reads and one second; every accepted read still reaches `ReadFile` | Not a snapshot; change between cadence points remains possible | Open |
| Out-of-range/oversized read | Protocol to source | Escape, allocation or denial of service | Checked `offset + length`, source-length bound, 1 MiB cap, client chunking, one source | Hostile live-device campaign pending | Mitigating |
| Source mutation by application | Broker to volume | Destroyed recoverable evidence | Read-only trait, `GENERIC_READ`, no mutation opcode or storage-changing IOCTL | Final static/native binary evidence pending | Open |
| False folder containment | Folder picker to namespace | Wrong candidates attributed to folder | Native NTFS identity: volume serial + record/sequence; exact active-directory resolution; trivalent ancestry | External NTFS corpus and native selection evidence pending | Mitigating |
| Path disclosure | Native to WebView/log | Private path/device information exposed | Commands use opaque IDs; absolute folder/device/pipe paths and raw OS errors remain native | Final artifact/log marker scan pending | Mitigating |
| Recovered-text spoofing | Parser to UI | Visually misleading path/warning | Bounded text, control/bidi formatting removal, React text nodes and directional isolation | Unicode confusables and native visual review remain | Mitigating |
| Parser memory/CPU denial | Volume bytes to parser | Hang, panic or excessive allocation | Unelevated `spawn_blocking`, checked arithmetic, region/work/allocation limits, bounded DTO; NTFS per-name path saturation is checked before another recursive sibling descent | `NTFS-NAMESPACE-WORK-BOUND-001` reduces the synthetic case from 8,191 calls to at most 600, but no cooperative cancellation and fuzz/performance corpora remain pending | Partial |
| False completeness | Parser/filter to UI | User treats partial count as exhaustive | NTFS/FAT status/warnings retained; unknown ancestry separate; score not a guarantee | Native presentation and external corpus pending | Mitigating |
| Fabricated production state | Browser/UI data path | User trusts nonexistent scan | Browser fails closed; exact four commands; no production sample/fallback/timer result | Final bundle/static inspection pending | Mitigating |
| Recovered-content execution | Candidate UI | Malware executes | No preview, open, execute, restore or Explorer action | Future feature requires a new threat model | Not started |
| Destructive test | CI/local tests | Real media damaged | Synthetic images/readers only; static CI/device guard | Static analysis is defense in depth; final validator run pending | Mitigating |
| Unsigned artifact quarantine | Release | User disables protection or trusts altered binary | Event remains unresolved; no false-positive claim; release requires signing and exact hashes | Norton disposition, Authenticode and clean-machine evidence pending | Open |

## Protocol security facts

- `HelloAck` echoes the exact 32-byte CSPRNG client nonce.
- The client comparison is constant-time.
- The echo binds the response to this connection but provides no shared-secret
  authentication.
- Native peer identity combines PID/liveness with
  `QueryFullProcessImageNameW` on the already-bound handle and exact canonical
  equality to the fixed desktop sibling.
- Peer image-path equality is not Authenticode, publisher, package-revision or
  protected-directory authentication.
- Protocol v2 permits exactly `Hello`, `HelloAck`, `OpenSource`, `Opened`,
  `ReadAt`, `ReadData`, `CloseSource`, `Closed`, `Shutdown` and `Error`.
- Maximum payload/read is 1 MiB.
- One session opens at most one source.

## Privacy facts

No account, cloud, analytics or telemetry is required. Only locale, theme and
reduced-motion preferences persist. Native folder/source authorities and scan
results live in bounded memory. Candidate bytes never enter JavaScript.

## Residual risk and acceptance boundary

The current implementation is not a forensic snapshot, signed release or proof
of broad real-media compatibility. An exact volume clone that preserves GUID
plus serial cannot be distinguished at first broker open, and namespace-budget
saturation remains explicitly partial/unknown rather than complete evidence.
No real volume was scanned for this evidence. Final local gates, static guards,
native binaries/manifests/hashes, actual Tauri accessibility, remote CI,
external corpora, Authenticode, administrator-protected installation,
clean-machine and endpoint-security disposition remain pending.

Norton’s deletion of an unsigned development build is inconclusive. Temporary
local protection state is not a mitigation and must not be generalized into a
release instruction.
