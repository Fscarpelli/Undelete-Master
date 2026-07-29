# SDD-007 — Carving, Validation, and Repair

Status: `Not started`

## Carving

Carving will scan bounded unallocated regions when allocation evidence is
reliable, or explicitly selected RAW regions otherwise. Built-in carvers are
compiled with the product. User extensions are declarative, schema-limited
signatures; arbitrary DLLs and scripts are forbidden.

## Validation

Recovered bytes are hostile input. Validators run in a future restricted,
networkless worker with strict file-size, CPU, memory, recursion, expansion, and
time limits. A validator crash must not terminate the scan.

## Preview

Preview must escape text/HTML, forbid external navigation, limit decoded
dimensions and byte windows, and never execute macros, scripts, DLL entry points,
or recovered applications.

## Repair

Repairs are deterministic derivatives. The original extraction remains
unchanged; both files receive hashes and a provenance report listing every
transformation. Repair must never fabricate original bytes.

No current screen, mock result, signature, or placeholder is evidence that these
requirements are implemented.

