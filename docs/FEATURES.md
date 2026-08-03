# Feature catalog

This catalog describes the behavior present in the current `main` candidate.
The exact verification status remains authoritative in the
[traceability matrix](traceability-matrix.md).

## Connected-storage workflow

1. The unelevated desktop discovers real mounted local Windows volumes.
2. The user chooses a supported volume and may optionally choose a folder on
   NTFS.
3. The desktop starts a short-lived elevated broker that accepts only an opaque
   volume identity and bounded read requests.
4. The broker re-resolves and revalidates the mounted volume, opens it with
   `GENERIC_READ`, and never exposes source-write, trim, format, lock, dismount
   or arbitrary device-control operations.
5. Results are retained by the native backend and presented to the unelevated
   UI.

The Windows reader supports modern devices and a narrowly vetted compatibility
path for legacy direct USB bridges that reject the alignment-property query.
That path uses the logical sector already validated by Windows, records the
fallback mode and requires it to remain stable during revalidation.

## Scan modes

### Metadata scan

- NTFS deleted-record discovery with bounded MFT streaming.
- FAT12/16/32 deleted-entry discovery, including long-file-name handling.
- NTFS folder scope based on native directory identity and proven ancestry.
- Quantitative NTFS MFT coverage and explicit partial/incomplete reasons.
- Files of any extension may be candidates when metadata provides a usable
  content plan.

### Deep JPEG scan

- Whole-volume NTFS only.
- Reads only ranges proven free by a validated NTFS `$Bitmap` snapshot.
- Finds bounded contiguous JPEG signatures and applies structural validation.
- Records physical range, SHA-256, validator version and coverage evidence.
- Does not claim fragmented JPEG recovery or support other carving formats.

## Native scan feedback

- Animated current-phase indication.
- Real MFT record counts and percentage when a trustworthy total is available.
- Elapsed time updated during the scan.
- ETA calculated only from observed throughput; no invented estimate.
- Truthful indeterminate state for namespace, candidate and carving phases that
  do not have a reliable total.
- Scan pause/resume and cooperative scan cancellation are not implemented.

## Actionable results

- Explicit search and dynamic extension facets.
- Filters for kind, discovery method, metadata confidence, candidate state,
  recovery eligibility and score.
- Stable column sorting and bounded cursor-based pages.
- Individual checkbox selection, row selection and select-all-matching.
- Selection is native-authoritative and survives page, filter, search and sort
  changes during the current process.
- Global selection summary includes item counts, logical bytes, partial items,
  conflicts and ineligible items.

## Transactional recovery

- Recovers only explicitly selected eligible candidates.
- Requires an authorized local NTFS folder on a known different physical disk.
- Preserves the reconstructed tree and sanitizes destination paths.
- Never overwrites an existing file; collisions receive deterministic renamed
  destinations.
- Streams bounded source reads into unique temporary files, validates length
  and hash evidence, then publishes atomically where the platform permits.
- Partial recovery requires explicit best-effort consent and records exact
  zero-filled ranges in a sidecar.
- Native item/byte progress, cooperative cancellation and terminal counts.
- Writes a versioned JSON manifest and opens only the completed job folder.
- Never previews or executes recovered content in the elevated process.

## Headless image CLI

`undelete-master scan-image` scans an ordinary local image file and can emit
privacy-preserving JSON. Device paths, directories and unsupported image
spellings are rejected. The desktop intentionally does not expose an image
picker; its workflow is connected storage.

## Security and reliability

- All scan-source access is read-only.
- The WebView remains unelevated; elevation is isolated to the fixed sibling
  broker.
- The broker authenticates the already-bound pipe peer by PID, liveness and
  fixed executable path before serving reads.
- Parser arithmetic is checked and bounded.
- Recursive namespace expansion applies budgets before descending.
- Production guards reject mock/fallback datasets and destructive storage API
  additions.
- Synthetic fixtures assert expected recovered content by SHA-256.

## Current boundaries

The desktop does not yet provide exFAT/ReFS recovery, fragmented or all-format
carving, unmounted-partition scans, whole-physical-disk scans, persistent
sessions, restart resume, file preview, scan cancellation, ACL/EFS/ADS
restoration, a signed installer or a production support guarantee. See
[known limitations](specs/015-known-limitations.md) for the complete list.
