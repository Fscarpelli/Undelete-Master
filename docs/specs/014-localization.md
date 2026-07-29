# SDD-014 — Localization

Status: `Partial`

## Required locales

- `pt-BR`: default when the Windows locale is Portuguese.
- `en-US`: complete supported fallback.

Production UI strings must use typed catalog keys; no principal user-facing
string is hard-coded. Both catalogs use identical placeholders and non-empty
values. Technical logs and code remain English; user documentation is maintained
in English and Brazilian Portuguese.

## Validation

- Catalog key and placeholder parity tests.
- Locale-sensitive byte/date/number formatting tests.
- Layout review for expansion, 200% zoom, and 1100×700.
- Keyboard/screen-reader labels in both languages.

The current catalogs and parity tests are useful component evidence, but full
localization remains unverified until every screen, error path, Tauri dialog,
installer, and accessibility label is covered.

