# SDD-008 — Session Data Model

Status: `Not started`

The production session store will be SQLite on a working disk different from
the source. It will use versioned migrations, short transactions, bounded batch
writes, indexed cursor pagination, crash recovery, and a safe journal mode.

Minimum logical areas are sessions, sources, regions, partitions, filesystems,
candidates, names, paths, extents, conflicts, validations, selections, restore
jobs/items, events, checkpoints, and errors.

## Required invariants

- No session database or thumbnail is written to the scan source.
- Checkpoints bind source identity, configuration, parser position, and schema
  version before resume.
- Import rejects malformed, oversized, traversal-capable, or unsupported
  `.umscan` packages.
- Deleting a session is explicit and does not claim secure erase on SSD.
- Normal logs and exports support path redaction.

The former in-memory desktop demonstration was removed. Current browser
`localStorage` contains only language, theme, and reduced-motion preferences;
it is not a session store and contains no source or report data.
