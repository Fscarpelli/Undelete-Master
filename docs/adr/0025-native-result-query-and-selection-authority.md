# ADR-0025 — Native Result Query and Selection Authority

Status: Accepted
Decision date: 2026-07-30

Implementation status: `Implemented-unverified` for the SDD-020 Task 1 slice.
Native query, sort, pagination, facets, and selection are implemented and pass
their focused deterministic tests. The bounded-page React consumer,
late-response handling, restore planning, source revalidation, destination
authority, and packaged acceptance remain later SDD-020 tasks.

## Context

The connected-volume desktop retained only presentation rows and exposed a
legacy append-only page command. That representation could display scan
results, but it could not filter or sort the complete retained scan, keep
selection across view changes, select a filtered set without sending all IDs
through the WebView, bind cursors to their query, or retain native candidate
content evidence for later restore planning.

Making the WebView authoritative would require it to receive the full result
set or native candidate descriptors. Either choice would weaken the bounded
UI contract and expose recovery authority outside Rust.

## Decision

### Native retained state

Each bounded `ScanSession` now owns:

- a native-only `ScanSourceBinding` containing inventory generation, opaque
  volume ID, observed source length, and filesystem;
- the complete retained `Candidate` values plus sanitized presentation rows
  and optional expected SHA-256 values;
- a candidate-ID index;
- the canonical selected-ID set and monotonically increasing selection
  revision; and
- the current canonical query/cursor binding.

The source binding deliberately has no physical-disk identity. The current
inventory has display evidence only. SDD-020 Task 3 may extend the binding only
after the authoritative broker-open response carries a real physical identity;
there is no sentinel, guessed disk number, or placeholder.

### Query, facets, and sort

`query_candidate_page` evaluates search, extension, kind, metadata confidence,
discovery method, candidate state, score range, recovery eligibility, and
selected-only predicates against the complete retained session. It then
applies a stable requested sort and returns at most 100 rows.

Extensions are normalized case-insensitively. The empty extension represents
no declared extension. Extension facets count the complete active scan and are
not derived from the visible page. The facet wire bound equals the retained
scan bound of 100,000 candidates, so every distinct extension in a valid
retained scan can be represented without truncation.

Every supported sort uses the native `CandidateId` as its final ascending
tie-breaker. Null scores sort after non-null scores in both directions.

### Query and cursor binding

A query ID is a deterministic SHA-256-derived opaque token over the source
binding, scan ID, and canonical query, including its caller-owned query
revision. A cursor is separately bound to that query ID, source fingerprint,
sort, and page offset.

Only cursors issued by the active native binding are accepted. A query, query
revision, sort, scan, source binding, malformed token, or foreign-session
change rejects the cursor without returning rows. A selected-only cursor also
binds the selection revision because selection changes alter its filtered
dataset.

### Selection

`update_candidate_selection` accepts the scan ID, query ID, tagged operation,
and expected selection revision. The native selected-ID set is the authority.

Direct ID mutations accept 1 through 100 unique decimal candidate IDs.
`selectAllMatching` and `clearMatching` evaluate the active canonical query in
Rust and accept no ID list. `clearAll` clears the complete selection. Every
successful mutation increments the revision; a stale revision or invalid
candidate rejects before mutation. Each operation is applied to a proposed set;
its next revision and complete summary must succeed before the authoritative
set or revision is committed.

Selection summaries report global selected files, directories, logical bytes,
best-effort, conflicted, and ineligible counts. They also report the selected
count within the active filtered set. That matching count can never exceed the
filtered total.

Recovery eligibility in this slice is conservative:

- directories, zero-length files, and fully covered readable/sparse content
  are `complete`;
- candidates retaining some bounded but conflicted or degraded content are
  `bestEffort`; and
- overwritten, metadata-only, unknown, or wholly unreadable candidates are
  `ineligible`.

Later restore planning must revalidate content descriptors and may reject a
candidate more strictly. Query eligibility is not permission to write.

### WebView boundary

Schema-v1 query pages expose only opaque scan/query/cursor tokens, decimal
candidate IDs, sanitized presentation fields, bounded counts/facets, and
selected/eligibility flags. The exact TypeScript parser rejects unknown keys,
duplicate IDs, out-of-range decimal values, inconsistent counts, and a 101st
row.

Candidate descriptors, extents, offsets, source bindings, expected hashes,
handles, recovered bytes, and source/destination authority paths are never
serialized.

The legacy schema-v2 page remains registered for the existing UI. SDD-020 Task
6 replaces its append-only consumer after the results workspace exists.

## Security consequences

- Scan-source access remains read-only and unchanged.
- Querying and selecting perform no source reads or writes.
- The WebView cannot manufacture a select-all ID set or replay a stale
  selection revision.
- A cursor cannot cross a query, sort, source, or scan authority boundary.
- Native candidate extents and hashes remain available for later restore
  planning without crossing the Tauri serialization boundary.
- This decision adds no Windows API, elevation, physical-disk claim,
  destination, restore write, preview, or execution surface.

## Tradeoffs

- Only the current native query binding is retained. A mutation from an older
  view fails closed and the frontend must issue the current query again.
- Facets are recomputed from the bounded retained scan. This is linear in the
  candidate count but remains within the existing 100,000-candidate bound.
- Selection is durable only for the in-memory scan-session lifetime;
  persistent resume remains outside this increment.
- Decimal candidate IDs are opaque contract values even though their wire
  encoding is numeric text.

## Verification

Focused Rust tests construct real `Candidate` values and cover every filter,
every sort, extension facets, deterministic null placement and ID
tie-breaking, the exact 100-row bound, cursor binding, select-all, direct-ID
bounds, persistence, stale revisions, and summary consistency.

Focused TypeScript tests cover the exact schema-v1 page, closed keys, decimal
limits, duplicate IDs, authority-field rejection, and exact Tauri wrapper
arguments. No test opens or writes a real disk.

## Relationship to earlier decisions

- [ADR-0002](0002-read-only-source-invariant.md) remains authoritative.
- [ADR-0023](0023-windows-read-only-broker-and-folder-scope.md) remains
  authoritative for mounted-volume and broker authority.
- [ADR-0024](0024-streaming-mft-and-bounded-content-carving.md) remains
  authoritative for candidate and carving evidence.

## Revisit triggers

Revisit before persisting selections across application restarts, retaining
multiple simultaneous query bindings, changing result/page/ID limits,
serializing additional native candidate evidence, or weakening cursor or
selection revision validation.
