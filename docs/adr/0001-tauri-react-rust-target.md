# ADR-0001 — Tauri, React, TypeScript, and Rust Target

Master specification topic: 1 — Tauri, React, TypeScript, and Rust target

## Status

Accepted

## Context

The product needs native Windows access, an unelevated modern desktop UI, safe
low-level parsers, and reusable OS-independent engines. The image-only slice now
has a real Tauri shell; privileged device, restore and preview boundaries remain
future work.

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

Tauri commands/capabilities, CSP and IPC are implemented for one path-free
image-scan command. Browser-only execution fails closed. Installer, signing,
device access, restore and full native E2E remain required before a production
recovery release.
