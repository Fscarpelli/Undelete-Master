# ADR-0001 — Tauri, React, TypeScript, and Rust Target

## Status

Accepted

## Context

The product needs native Windows access, an unelevated modern desktop UI, safe
low-level parsers, and reusable OS-independent engines. The current UI is a Vite
demonstration; no production Tauri shell exists.

## Options

- Tauri + React/TypeScript + Rust: small web UI boundary and Rust engine.
- Electron + Node native addons: mature UI ecosystem but larger privileged and
  native-addon surface.
- Native Windows UI only: strong platform integration but less reuse and slower
  delivery for the existing React work.

## Decision

Use Tauri 2, React with TypeScript strict, and Rust stable as the production
target. Filesystem engines remain independent of Tauri and the DOM.

## Consequences

Tauri commands/capabilities, CSP, IPC, installer, and real-shell E2E become
required before production. The existing browser demonstration is explicitly
non-production and cannot satisfy desktop acceptance criteria.

