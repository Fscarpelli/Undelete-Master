# ADR-0005 — Pull-request CI Excludes Devices

## Status

Accepted

## Context

Hosted pull-request runners are unsuitable for destructive or privileged storage
tests, and untrusted changes must not receive signing or device privileges.

## Options

- Run all tests, including device/VHD tests, on every PR.
- Skip automated quality checks.
- Split deterministic PR gates from future isolated extended device gates.

## Decision

PR CI runs formatting, lint, type, unit/property/integration tests, frontend
build, documentation validation, and fixture-only scans. It never attaches VHDs,
addresses physical devices, requests elevation, or enables a real-device test
feature. Frontend dependencies install with lifecycle scripts disabled. A
static guard reviews every workflow and enumerated first-party executable
surface, including package manifests and extensionless shebang helpers. A
future isolated workflow requires explicit allowlists and guards.

## Consequences

PR feedback is safe and repeatable, but does not verify Windows broker/device
behavior. Release readiness remains blocked until the separate controlled
evidence exists. Static inspection is defense in depth: it does not prove
runtime behavior or detect intentionally obfuscated commands.
