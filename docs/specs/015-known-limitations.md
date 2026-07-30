# SDD-015 — Known Limitations

Status: Current as of 2026-07-29

## Product status

This repository is not a production data-recovery release. The desktop now has
an implemented real mounted-volume workflow, but its final native, package,
remote, signing and endpoint-security gates remain pending. No real-volume scan
has been executed as acceptance evidence.

The CLI continues to analyze approved regular image files. Neither desktop nor
CLI restores data.

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

## Filesystem and recovery limitations

- NTFS and FAT support covers deterministic fixtures and selected hostile
  structures, not a complete compatibility matrix.
- No production exFAT support; incomplete local exFAT work is outside release
  claims.
- No carving, repair derivative, journal/recycle-bin enrichment, NTFS LZNT1,
  full attribute-list/ADS/EFS workflow or damaged-volume reconstruction.
- Recognized NTFS/FAT scans may be `partial`. Candidate counts then reflect only
  bounded observed coverage, not an exhaustive total.
- NTFS MFT and allocation-bitmap work is bounded. Missing/torn records,
  malformed ordinary attributes, unresolved `$ATTRIBUTE_LIST`, prefix limits
  and extension merge failures can force partial status. Recursive namespace
  expansion checks the 256-path per-name cap before descending into another
  saturated sibling; saturation marks namespace/ancestry evidence incomplete,
  keeps unproven membership `Unknown` and may force partial status rather than
  overclaiming complete coverage.
- FAT copy disagreement/read failure, directory-chain damage/cycles and
  traversal limits can force partial status. A declared FAT table above 64 MiB
  is rejected before allocation.
- GPT parsing deliberately rejects conflicting/nonreciprocal canonical copies
  and metadata outside reserved GPT space. It does not infer a partition map
  through carving.
- Overwritten, trimmed, securely erased, encrypted-unavailable or physically
  unreadable bytes cannot be recreated. A name, metadata record or score does
  not prove intact content.

## Product-flow limitations

- No restore, preview, content execution, Explorer launch or destination
  containment.
- No persistent sessions, checkpoint resume, import or export.
- No pause, resume, cooperative cancellation, phase percentage or ETA.
- No hotplug subscription. Refresh is explicit; a disappearing source fails
  when native identity or I/O detects it.
- No candidate content validation. Candidate rows contain metadata only.
- Candidate pages are fixed at at most 100 rows. Native memory retains at most
  32 folder scopes and 4 scan sessions; older authorities are evicted.
- Browser execution is intentionally unavailable and has no fallback data.
- Displayed labels may contain Unicode confusables even though control and bidi
  formatting characters are removed; labels are not authority.

## Validation and release limitations

- Deterministic component tests do not prove compatibility with arbitrary real
  media or hostile filesystems.
- Fuzzing, external forensic corpora, million-row performance and long-running
  mutable-volume campaigns remain incomplete.
- Final same-revision Rust/frontend/static gates remain to be recorded.
- Native main/broker builds, hashes, extracted manifests, actual Tauri visual
  and assistive-technology review, remote CI and clean-machine evidence remain
  pending.
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
