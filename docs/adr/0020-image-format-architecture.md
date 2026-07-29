# ADR-0020 — Image Format Architecture

Master specification topic: 16 — RAW, segmented RAW, VHD, and VHDX image formats

Status: Proposed

## Context

The master specification requires an explicit decision for RAW, segmented RAW,
VHD, and VHDX. ADR-0003 deliberately narrows the current CLI to one ordinary
regular file; it does not define the product-wide image architecture and must
not be cited as support for containers or segment sets.

An image adapter is part of the scan-source trust boundary. It must expose only
bounded reads and a stable logical length, must never attach or mount a
container, and must reject ambiguous or incomplete input rather than silently
falling back to a different interpretation.

## Options considered

1. Treat every extension as a contiguous RAW file.
2. Ask the operating system to mount VHD/VHDX and scan the resulting device.
3. Use explicit read-only adapters with capability detection and format-specific
   validation.

Option 1 can misinterpret container metadata and incomplete segment sets.
Option 2 expands the privileged/device boundary and can create host-side state.
Option 3 is selected.

## Proposed decision

The source layer will use an explicit, capability-reported adapter selected by
validated content and user intent:

- **Contiguous RAW:** the current foundation supports one regular `.img`,
  `.dd`, or `.raw` file through the bounded read-only file reader. The CLI's
  accepted `.bin` suffix is only a generic contiguous-RAW alias; it is not a
  separate format claim.
- **Segmented RAW:** future support requires an ordered-segment adapter. It
  validates naming/order, rejects gaps and duplicate positions, computes the
  aggregate logical length with checked arithmetic, bounds every cross-segment
  read, and records immutable identity evidence for every segment.
- **VHD and VHDX:** future support requires a parser-backed read-only adapter
  that validates headers, region tables, virtual capacity, allocation metadata,
  parent relationships, and all offset arithmetic. It must not use automount,
  attach, format, trim, lock, dismount, or writable APIs. Differencing images
  remain unsupported until parent-chain identity and containment are specified
  and tested.
- **Other containers:** E01, AFF4, compressed archives, sparse bundles, and
  unknown formats fail with an explicit unsupported-format result until their
  own governed adapter, threat analysis, deterministic fixtures, and external
  corpus evidence exist.

Extension alone never overrides a conflicting signature. There is no silent
container-to-RAW fallback. A recognized but unsupported format produces a
specific diagnostic without reading beyond the bytes already available to the
detector.

## Security and correctness constraints

- Every adapter implements only the read-only `SourceReader` contract.
- Logical offsets, segment totals, block maps, and parent references use
  checked and region-bounded arithmetic.
- Adapters do not execute, preview, mount, or trust recovered content.
- PR tests use deterministic ordinary files only. VHD/VHDX attachment and
  physical-device tests are excluded by ADR-0005.
- Format and capability names in the CLI/UI are derived from implemented,
  tested adapters; a filename suffix is not support evidence.

## Verification required before acceptance

- Deterministic positive, truncated, malformed, overlap, overflow, sparse, and
  cross-boundary fixtures with expected SHA-256 recovery truth.
- Negative tests for missing/reordered RAW segments and invalid VHD/VHDX tables.
- External, legally redistributable corpus evidence with provenance.
- Proof that no adapter opens a source writable or invokes mount/device-control
  operations.
- Traceability and compatibility-matrix updates that keep unimplemented
  adapters labeled unsupported.

## Consequences

The current contiguous-RAW boundary stays small and honest. Segmented RAW,
VHD, and VHDX remain not implemented until separate adapter work satisfies this
record and is independently reviewed. The explicit adapters add code and test
cost, but prevent extension-based misclassification and avoid expanding the
privileged device boundary.
