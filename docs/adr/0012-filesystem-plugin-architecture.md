# ADR-0012 — Filesystem Plugin Architecture

Master specification topic: 7 — filesystem plugin architecture

Status: Proposed

## Context

NTFS, FAT, exFAT, and future filesystem engines need consistent isolation,
capability reporting, and bounded access.

## Proposed decision

Define static Rust filesystem-engine interfaces over `SourceReader` regions,
typed scan options, candidates, warnings, and declared capabilities. Engines
remain OS-independent and receive no path, network, process, or write
capability. Detection and dispatch are explicit; unsupported formats fail
honestly.

## Security and verification consequences

Each engine declares limits and fixture coverage. Registration is build-time in
version 1; arbitrary third-party dynamic loading is excluded. Contract,
capability, hostile-input, deterministic fixture, and dispatch-conflict tests
are required. Existing scanners are not yet a complete plugin framework.
