# SDD-017 — Historical Real-only Image Desktop

Status: Historical; superseded by SDD-018 for desktop behavior.
Decision date: 2026-07-29

This record preserves the requirement IDs and status snapshot of the former
image-picker desktop. Its adapter and frontend tests were removed when the
desktop moved to real mounted-volume inventory. They are not executable
evidence for SDD-018. ADR-0003 continues to govern the regular-image CLI.

## Historical requirements

### SDD-REAL-001 — No fabricated production data

- **Rationale:** the former desktop prohibited sample reports.
- **Priority:** Must.
- **Source:** product-owner direction.
- **Preconditions:** historical image desktop.
- **Behavior:** use only real scanner output.
- **Error behavior:** fail closed without the native runtime.
- **Security implications:** no fabricated trust signal.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** no sample provider in that increment.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Implemented-unverified.

### SDD-REAL-002 — Native-only provider

- **Rationale:** browser fallback could fabricate storage data.
- **Priority:** Must.
- **Source:** former desktop contract.
- **Preconditions:** historical image desktop.
- **Behavior:** invoke only the native image command.
- **Error behavior:** browser execution fails closed.
- **Security implications:** no browser provider.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** one native provider path.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Implemented-unverified.

### SDD-REAL-003 — Rust-owned image picker

- **Rationale:** the selected image path stayed outside JavaScript.
- **Priority:** Must.
- **Source:** ADR-0021 and ADR-0022.
- **Preconditions:** historical image desktop.
- **Behavior:** native Rust selected one regular image.
- **Error behavior:** unsafe/locality-invalid paths failed closed.
- **Security implications:** path-free WebView command.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** no WebView path argument.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Implemented-unverified.

### SDD-REAL-004 — Blocking image scan

- **Rationale:** parsing could not block the async runtime.
- **Priority:** Must.
- **Source:** former desktop contract.
- **Preconditions:** selected regular image.
- **Behavior:** run the image CLI library on a blocking worker.
- **Error behavior:** typed scanner errors stayed distinct.
- **Security implications:** no shell execution.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** direct scanner parity.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Implemented-unverified.

### SDD-REAL-005 — Truthful aggregate summary

- **Rationale:** aggregate values had to come from real parsers.
- **Priority:** Must.
- **Source:** former desktop contract.
- **Preconditions:** image scan completed.
- **Behavior:** render real partition/filesystem counts and warnings.
- **Error behavior:** incompatible reports failed closed.
- **Security implications:** no invented candidate detail.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** deterministic report parity.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Implemented-unverified.

### SDD-REAL-006 — Unsupported workflows absent

- **Rationale:** visible nonfunctional controls are false product claims.
- **Priority:** Must.
- **Source:** product-owner direction.
- **Preconditions:** historical production navigation.
- **Behavior:** omit raw device, restore, preview and session flows.
- **Error behavior:** unsupported routes had no side effect.
- **Security implications:** no privileged/write surface.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** finite command/route inventory.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Implemented-unverified.

### SDD-REAL-007 — Sanitized errors

- **Rationale:** image paths and parser details are sensitive.
- **Priority:** Must.
- **Source:** former privacy contract.
- **Preconditions:** historical selection or scan failure.
- **Behavior:** return stable codes and bounded text.
- **Error behavior:** omit paths, offsets and raw OS diagnostics.
- **Security implications:** reduced local information disclosure.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** no unique path marker in IPC.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Implemented-unverified.

### SDD-REAL-008 — Duplicate and stale response control

- **Rationale:** late results could be attributed to another request.
- **Priority:** High.
- **Source:** former UI concurrency contract.
- **Preconditions:** historical scan pending.
- **Behavior:** admit one request and ignore stale responses.
- **Error behavior:** late completion did not mutate a newer view.
- **Security implications:** report/source association remained stable.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** deterministic request ordering.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Verified.

### SDD-REAL-009 — Effective preferences

- **Rationale:** inert settings would be fabricated functionality.
- **Priority:** High.
- **Source:** former UI contract.
- **Preconditions:** preference changed.
- **Behavior:** locale, theme and reduced motion took effect.
- **Error behavior:** invalid persisted values reset safely.
- **Security implications:** no source/report persistence.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** three-field preference storage.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Verified.

### SDD-REAL-010 — Minimal capability and accessible focus

- **Rationale:** the image desktop needed no general host authority.
- **Priority:** Must.
- **Source:** former security/accessibility contract.
- **Preconditions:** historical Tauri configuration.
- **Behavior:** minimal capabilities, local CSP and focus semantics.
- **Error behavior:** denied operations stayed unavailable.
- **Security implications:** reduced WebView blast radius.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** source/config/native acceptance was required.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Implemented-unverified.

### SDD-REAL-011 — Exact large integers

- **Rationale:** JSON numbers cannot represent every `u64`.
- **Priority:** Must.
- **Source:** former DTO contract.
- **Preconditions:** historical report crossed IPC.
- **Behavior:** transport large values as decimal strings.
- **Error behavior:** malformed/oversized reports failed closed.
- **Security implications:** prevented precision and allocation bugs.
- **Observability:** retained historical evidence only.
- **Acceptance criteria:** boundary values round-tripped exactly.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Verified.

### SDD-REAL-012 — Historical package gate

- **Rationale:** component tests did not prove a packaged application.
- **Priority:** Must.
- **Source:** repository delivery rules.
- **Preconditions:** historical image desktop candidate build.
- **Behavior:** require source, bundle, native and remote inspection.
- **Error behavior:** missing evidence blocked publication.
- **Security implications:** no unsupported production claim.
- **Observability:** evidence record retains the open gates.
- **Acceptance criteria:** native/package/remote gates were required.
- **Test IDs or formal justification:** `JUST-SDD-REAL-HISTORICAL`.
- **Implementation links:** `docs/evidence/real-only-desktop-2026-07-29.md`.
- **Status:** Implemented-unverified.

## Supersession

SDD-018 replaces all desktop source selection, execution and candidate delivery
described here. Historical statuses do not transfer verification to SDD-018.
The image CLI remains supported under ADR-0003.
