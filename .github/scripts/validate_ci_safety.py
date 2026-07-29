#!/usr/bin/env python3
"""Fail PR CI if it gains real-device/destructive storage behavior."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

FORBIDDEN_WORKFLOW_PATTERNS = {
    r"self-hosted": "PR quality jobs must use isolated hosted runners",
    r"PhysicalDrive": "PR quality must not address physical disks",
    r"\bdiskpart\b": "diskpart is forbidden in PR quality",
    r"(?im)^\s*(?:run:\s*)?format(?:\.com)?\s+[a-z]:": "format commands are forbidden in PR quality",
    r"\bmount-vhd\b|\bnew-vhd\b": "VHD device tests require a separate governed workflow",
    r"GENERIC_WRITE|FSCTL_(?:LOCK|DISMOUNT)_VOLUME": "write/lock/dismount access is forbidden",
}

FORBIDDEN_RUST_TEST_PATTERNS = {
    r"\\\\\.\\PhysicalDrive": "Rust tests must not open a physical disk",
    r"(?i)\bdiskpart\b": "Rust tests must not invoke diskpart",
    r"""(?i)(?:Command::new\(|cmd(?:\.exe)?\s+/c\s+)["']?format(?:\.com)?\b""": "Rust tests must not invoke format",
    r"GENERIC_WRITE": "Rust tests must not request source write access",
    r"FSCTL_(?:LOCK|DISMOUNT)_VOLUME": "Rust tests must not lock/dismount volumes",
}


def scan(path: Path, patterns: dict[str, str], errors: list[str]) -> None:
    text = path.read_text(encoding="utf-8")
    for pattern, message in patterns.items():
        if re.search(pattern, text):
            errors.append(f"{path.relative_to(ROOT)}: {message}")


def main() -> int:
    errors: list[str] = []
    workflow = ROOT / ".github/workflows/quality.yml"
    scan(workflow, FORBIDDEN_WORKFLOW_PATTERNS, errors)

    for path in (ROOT / "crates").rglob("*.rs"):
        normalized = path.as_posix()
        if "/tests/" in normalized or path.name.endswith("_test.rs"):
            scan(path, FORBIDDEN_RUST_TEST_PATTERNS, errors)

    if errors:
        print("CI safety validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print("CI safety validation passed: no real-device or destructive PR path")
    return 0


if __name__ == "__main__":
    sys.exit(main())
