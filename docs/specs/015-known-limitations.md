# SDD-015 — Known Limitations

Status: Current as of 2026-07-31

## Product status

This repository is not a production data-recovery release. The desktop now has
an implemented real mounted-volume discovery, bounded results workspace and
transactional restore workflow. Required local gates, an unsigned release
build, embedded-manifest inspection and a bounded inventory-only launch have
passed. Remote CI, signing, endpoint-security reputation, clean-machine and
governed real deleted-file recovery gates remain pending. An interactive scan
reported by the product owner is diagnostic input, not retained acceptance
evidence; no real-volume scan has been executed as a project acceptance test.

The `scan-image` CLI process command continues to analyze approved regular
image files in metadata mode and does not restore them. The desktop can restore
an explicit eligible native selection to a retained NTFS destination authority
on one proven different physical disk. Metadata-backed restoration is
extension-agnostic only when a bounded readable content plan exists. Deep
carving remains plugin-based: the current product slice is contiguous JPEG on
a whole NTFS mounted volume; metadata remains the default scan mode.

## Source limitations

- The desktop lists mounted local volumes only.
- Logical cards group mounted volumes without a physical disk number; they are
  not a physical-disk map or whole-disk scan controls.
- `PhysicalDriveN`, unmounted partitions and whole-physical-disk scans are not
  supported.
- Remote/mapped, CD-ROM, RAM-disk and unknown roots fail closed unelevated.
  Missing and multi-disk/composite mappings are detected and rejected only by
  the elevated broker after explicit scan activation.
- There is no Storage Spaces/dynamic-volume proof, locked-BitLocker key flow,
  VHD/VHDX attach flow or ReFS support.
- A mounted volume stays online and mutable during scanning. The app does not
  lock, dismount, snapshot or freeze it, so results are not a forensic snapshot
  or chain-of-custody record.
- Identity is re-enumerated after both 256 valid reads and one elapsed second,
  not before every physical read. Each accepted read still performs actual
  source I/O and fails closed on I/O failure.
- Pre-UAC volume identity is GUID plus serial. A replacement that preserves
  both values exactly is indistinguishable at first broker open. UAC and
  independent broker enumeration still occur, but extents, canonical length
  and sector geometry are derived from the source currently mounted and become
  its later revalidation baseline. This is not a snapshot guarantee.

## Folder-scope limitations

- Folder scope is NTFS-only. FAT can be scanned only as a whole mounted volume.
- The folder picker path stays native, but selected NTFS authority depends on
  Windows volume serial and file-reference semantics.
- Only candidates whose ancestry is proven `Match` are shown.
- Missing, corrupt, reused, cyclic or ambiguous ancestry is counted as
  `Unknown` and omitted from rows. It is never silently treated as `NoMatch`.
- Display paths are not containment authority and may be reconstructed,
  incomplete, orphaned or ambiguous.
- A folder-scoped scan still reads the containing volume's metadata; it is not
  an ordinary visible-file directory walk.
- Folder scope is metadata-only. Deep JPEG is disabled in the UI and rejected
  natively because a carved byte range cannot prove historical folder
  ancestry.

## Filesystem and recovery limitations

- NTFS and FAT support covers deterministic fixtures and selected hostile
  structures, not a complete compatibility matrix.
- No production exFAT support; incomplete local exFAT work is outside release
  claims.
- Product deep scan is limited to contiguous JPEG discovery on a whole mounted
  NTFS volume. It is not a general raw, damaged-filesystem, slack-space,
  fragment, or multi-format recovery engine. The `scan-image` process command
  has no deep-mode flag.
- No repair derivative, journal/recycle-bin enrichment, NTFS LZNT1, full
  attribute-list/ADS/EFS workflow or damaged-volume reconstruction.
- Recognized NTFS/FAT scans may be `partial`. Candidate counts then reflect only
  bounded observed coverage, not an exhaustive total.
- The former 64 MiB MFT enumeration prefix was a confirmed defect: with 1 KiB
  records it examined at most the first 65,536 records. The component now reads
  the trusted MFT in bounded 1 MiB batches and exposes
  declared/available/examined records and bytes, but an explicit
  defense-in-depth ceiling of 8,388,608 records remains. Reaching that ceiling
  is partial, not exhaustive.
- MFT retained evidence has four separate ceilings: 100,000 deleted entries,
  100,000 directories, 100,000 extension references, and 100,000 extension
  streams merged into base records. Reaching any cap skips later evidence,
  emits a bounded warning, and makes the metadata result partial even when
  record examination continued. Nested evidence across retained base and
  extension records is additionally capped at 400,000 names, 200,000 streams,
  and 1,000,000 run elements. An over-budget record or extension merge is
  omitted as a unit and the result becomes partial; external hostile-corpus
  and long-running memory evidence is still incomplete.
- Quantitative MFT coverage now propagates through CLI/Tauri/TypeScript, and the
  desktop shows examined versus declared records with distinct complete/partial
  zero messages. Machine-readable partial reasons and a complete byte/reason
  presentation are still absent; bounded prose warnings remain the limiting
  context. Even a complete metadata zero does not prove that content cannot be
  found by an unimplemented technique.
- NTFS MFT and allocation-bitmap work remains bounded. Missing/torn records,
  malformed ordinary attributes, unresolved `$ATTRIBUTE_LIST`, trusted
  initialized/physical prefix limits and extension merge failures can force
  partial status. Recursive namespace expansion checks the 256-path per-name
  cap before descending into another saturated sibling; saturation marks
  namespace/ancestry evidence incomplete, keeps unproven membership `Unknown`
  and may force partial status rather than overclaiming complete coverage.
- The retained NTFS `$Bitmap` is still capped at 64 MiB. It becomes allocation
  authority only for an active base non-directory record named `$Bitmap` under
  the root, with exactly one unnamed stream, no attribute list or
  compressed/encrypted/sparse flags, starting VCN zero, and coherent stream
  sizes. The scanner exposes and the deep integrator submits coalesced physical
  regions only for clusters explicitly proven free inside that snapshot.
  Unknown/capped/untrusted suffixes are not free and never trigger a RAW
  fallback.
- JPEG carving is contiguous only. It does not reconstruct generic
  fragmentation, decode pixels, prove visual integrity, infer original
  name/path/date, or support the broader baseline format list.
- The product deep profile is bounded to 1 MiB buffers, 128 MiB per contiguous
  candidate, 10,000 candidates, 65,536 coalesced regions, 16 TiB of submitted
  free space, 10,000,000 signature validations, and 8 GiB of aggregate
  validation reads. Any reached budget makes coverage partial.
- Exact-range corroboration is intentionally narrow. One unambiguous NTFS
  metadata owner may receive the JPEG hash/validator; ambiguous multiple owners
  retain a separate carving candidate. This is not general deduplication.
- FAT copy disagreement/read failure, directory-chain damage/cycles and
  traversal limits can force partial status. A declared FAT table above 64 MiB
  is rejected before allocation.
- GPT parsing deliberately rejects conflicting/nonreciprocal canonical copies
  and metadata outside reserved GPT space. It does not infer a partition map
  through carving.
- Overwritten, trimmed, securely erased, encrypted-unavailable or physically
  unreadable bytes cannot be recreated. A name, metadata record or score does
  not prove intact content. The product cannot reverse SSD TRIM/garbage
  collection, break BitLocker/EFS, or guarantee consistency while Windows is
  writing to the live system volume.
- Restore destinations are NTFS-only, must resolve to exactly one proven
  physical disk, and must be disjoint from the source disk. Same, unknown,
  virtual, composite and multi-disk destinations fail closed with no override.
- The implemented layout is preserve-tree only. Flatten and by-type layouts
  are absent. Collision handling is deterministic rename plus atomic
  no-clobber publication; existing destination entries are never replaced.
- Exact best-effort recovery currently uses explicit `zeroFillAndMap` consent.
  It preserves logical length, zero-fills unavailable ranges and emits
  sidecar/manifest evidence. Truncation, separate-segment output and
  validator-recommended policy are absent.
- Recovered destination metadata is intentionally limited: ACL, EFS, alternate
  data streams, reparse semantics and optional timestamp/attribute restoration
  are not implemented.
- Jobs remain in memory for one process lifetime. There is no restore resume
  after closing or restarting the desktop. Protected temporary links may be
  retained by the documented safe-cleanup/reconciliation policy.

## Product-flow limitations

- No persistent sessions, checkpoint resume, import or export.
- The scan itself has no pause, resume, cooperative cancellation, phase
  percentage, heartbeat or ETA.
- The desktop deep scan is exposed with an honest indeterminate state and
  explicit text that percentage, ETA and cancellation are absent. Closing or
  interrupting that workflow has no supported cooperative checkpoint contract.
- Restore has native item/byte progress and cooperative cancellation only
  during the current process. Cancellation is best-effort between bounded
  reads/writes and never represents an incomplete file as published.
- The results workspace keeps at most three 100-row pages in the frontend.
  Search, facets, sorting, selection and selected-only state are authoritative
  in bounded native memory, not durable across application restart.
- Opening the destination is available only for a terminal job with a
  revalidated retained authority. It opens the recovery job directory through
  a fixed Windows shell operation; the WebView never supplies a path or
  executable.
- There is no recovered-content preview or execution. Recovered files are
  untrusted and are never opened by the broker or another privileged process.
- No hotplug subscription. Refresh is explicit; a disappearing source fails
  when native identity or I/O detects it.
- Candidate-page schema 2 can expose the discovery method, lowercase SHA-256
  and `jpeg-structural-v1` validator after native evidence-link validation.
  It exposes no recovered bytes and is not a safe pixel decode or preview.
- Candidate pages are fixed at at most 100 rows. Native memory retains at most
  32 folder scopes and 4 scan sessions; older authorities are evicted. The
  restore coordinator additionally retains bounded destination, plan and job
  stores and rejects saturation instead of silently evicting active authority.
- Browser execution is intentionally unavailable and has no fallback data.
- Displayed labels may contain Unicode confusables even though control and bidi
  formatting characters are removed; labels are not authority.

## Validation and release limitations

- Deterministic component tests do not prove compatibility with arbitrary real
  media or hostile filesystems.
- Fuzzing, external forensic corpora, million-row performance and long-running
  mutable-volume campaigns remain incomplete.
- Required same-revision Rust/frontend/static gates and the local release build
  are recorded in the Task 7 evidence. This proves the tested revision only.
- The unsigned desktop/broker pair, hashes, extracted manifests and an actual
  inventory-only Tauri launch are recorded. A real scan, destination picker,
  deleted-file restore, open-destination action, screen reader, 200% zoom,
  forced-colors, remote CI and clean-machine exercise remain pending.
- The broker now queries the already-bound peer with
  `QueryFullProcessImageNameW` and requires the canonical fixed
  `undelete-master-desktop.exe` sibling before serving. This reduces the
  confused-deputy surface but does not authenticate publisher/package revision
  or protect a user-writable sibling directory.
- No signed installer, portable release, SBOM, upgrade or uninstall evidence
  has been accepted.
- Norton deleted one unsigned local development executable on 2026-07-29. The
  classification remains unresolved and the artifact is not approved for
  redistribution. It is neither confirmed malware nor a confirmed false
  positive.
- Temporarily disabling endpoint protection is not a release workaround.
  Protection should be restored after the local build exercise.
- Unsigned artifacts remain blocked from production redistribution until
  Authenticode, administrator-protected package placement and clean-machine
  evidence pass.

## Real-only invariant

Production contains no sample disk provider, fabricated candidate, simulated
progress, restore/session emulation or browser data fallback. If the real
native boundary is unavailable, the desktop fails closed.
