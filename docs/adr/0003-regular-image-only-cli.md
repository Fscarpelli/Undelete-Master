# ADR-0003 — Regular-image-only CLI

## Status

Accepted for the foundation-hardening increment

## Context

Existing read-only image, partition, NTFS, and FAT components need a real
end-to-end entry point. The privileged Windows broker and its threat model are
not implemented.

## Options

- Add physical-device CLI access now.
- Add a regular-image-only CLI.
- Keep libraries without an entry point.

## Decision

The current CLI accepts existing regular image files only and composes
`FileImageReader`, partition discovery, bounded regions, and supported metadata
scanners. It rejects device selectors and performs no extraction or restore.
Default JSON redacts full local paths.

The stable report is schema version 2. Every `VolumeReport` carries a required
`scanStatus`:

- `complete` for recognized NTFS/FAT enumeration that did not hit a known
  completeness boundary;
- `partial` for recognized NTFS or FAT when that engine structurally reports
  incomplete enumeration. NTFS uses it for bounded/truncated MFT work or
  skipped nonblank/unreadable/corrupt records, including malformed candidate
  attributes and records that depend on an `$ATTRIBUTE_LIST` whose complete
  reference resolution is not proven; FAT uses it when any declared secondary
  copy diverges or is unreadable, or for broken/cyclic/unreadable directory
  traversal, unusable directory start clusters, or directory
  count/byte/depth/chain safety limits;
- `unrecognized` only when neither supported filesystem recognizes the region.

Parser fallback is classification, not error recovery. Partition discovery
falls back to a whole-image region only on `ScanError::NotRecognized`. Within a
region, FAT is tried only when NTFS returns `NotRecognized`. A recognized
parser's `Read` or `Corrupt` result becomes a typed `CliError::VolumeScan`;
it is never masked by another parser or converted into an unrecognized/complete
report. Structural failures needed to bootstrap a recognized scanner remain
fatal: examples include malformed record-0 NTFS attributes, an invalid logical
MFT size, and a declared FAT table larger than the 64 MiB read/allocation
bound.

GPT fallback is likewise bounded and canonical. Both canonical copies are
evaluated even when the primary is usable. The alternate header is read only
from the final logical block, must point back to LBA 1, and must keep its entry
array in the reserved metadata area. When both headers are valid, they must
agree on reciprocal locations, usable range, disk GUID, entry geometry, and
entry-array CRC; conflict fails closed. A primary-header read failure still
permits evaluation of the independently canonical backup copy.

## Consequences

The slice can prove useful image scanning without privilege expansion. It is
only partial evidence for FR-033/AC-027 and no evidence for physical disks,
broker security, restore, carving, or production exFAT.

Candidate counts on any `partial` recognized volume are bounded observations,
not totals for the entire metadata space. Consumers must preserve
`scanStatus`, warnings, and schema version together.
