# ADR-0019 — Windows 10/11 Support Matrix

Master specification topic: 15 — Windows 10 and 11 support

Status: Proposed

## Context

Device APIs, UAC, filesystem behavior, packaging, DPI, and accessibility differ
across supported Windows releases.

## Proposed decision

Target Windows 10 22H2 and supported Windows 11 x64 releases, with Windows 11 as
the primary development baseline. Publish an evidence matrix for each supported
OS/build, architecture, filesystem, privilege path, display scale, installer,
and signing state. Unverified combinations are labeled experimental or
unsupported.

## Security and verification consequences

Support claims require clean-machine and upgrade testing on real Windows
environments outside pull-request device CI. The current image-only libraries
do not constitute complete Windows product support.
