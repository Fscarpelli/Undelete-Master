#!/usr/bin/env python3
"""Validate the versioned SDD skeleton without third-party dependencies."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

REQUIRED = [
    *(f"docs/specs/{index:03d}-{name}.md" for index, name in enumerate([
        "product-vision",
        "functional-requirements",
        "non-functional-requirements",
        "domain-model",
        "architecture",
        "windows-io-and-privilege-model",
        "partition-and-filesystem-engines",
        "carving-validation-repair",
        "session-data-model",
        "restore-semantics",
        "ux-ui-and-accessibility",
        "security-and-privacy",
        "test-and-validation-plan",
        "build-release-and-signing",
        "localization",
        "known-limitations",
    ])),
    "docs/traceability-matrix.md",
    "docs/risk-register.md",
    "docs/threat-model/THREAT_MODEL.md",
    "docs/evidence/environment-inventory.md",
    "README.md",
    "README.pt-BR.md",
    "SECURITY.md",
    "CONTRIBUTING.md",
]

ALLOWED_REQUIREMENT_STATUSES = {
    "Not started",
    "Partial",
    "Implemented-unverified",
    "Verified",
}


def fail(message: str, errors: list[str]) -> None:
    errors.append(message)


def markdown_files() -> list[Path]:
    files = [
        ROOT / "README.md",
        ROOT / "README.pt-BR.md",
        ROOT / "SECURITY.md",
        ROOT / "CONTRIBUTING.md",
    ]
    files.extend((ROOT / "docs").rglob("*.md"))
    return [path for path in files if path.exists()]


def main() -> int:
    errors: list[str] = []

    for relative in REQUIRED:
        if not (ROOT / relative).is_file():
            fail(f"missing required document: {relative}", errors)

    master = ROOT / "UNDELETE_MASTER_CODEX_MASTER_SPEC.md"
    master_text = master.read_text(encoding="utf-8")
    defined_fr = set(re.findall(r"^### (FR-\d{3})\b", master_text, re.MULTILINE))

    for path in markdown_files():
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT)

        for requirement in sorted(set(re.findall(r"\bFR-\d{3}\b", text))):
            if requirement not in defined_fr:
                fail(f"{relative}: undefined functional requirement {requirement}", errors)

        for match in re.finditer(r"\[[^\]]+\]\(([^)]+)\)", text):
            raw_target = match.group(1).strip()
            target = raw_target.split("#", 1)[0]
            if (
                not target
                or target.startswith(("http://", "https://", "mailto:"))
                or target.startswith("<")
            ):
                continue
            candidate = (path.parent / target).resolve()
            try:
                candidate.relative_to(ROOT.resolve())
            except ValueError:
                fail(f"{relative}: local link escapes repository: {raw_target}", errors)
                continue
            if not candidate.exists():
                fail(f"{relative}: broken local link: {raw_target}", errors)

    matrix = ROOT / "docs/traceability-matrix.md"
    if matrix.exists():
        matrix_text = matrix.read_text(encoding="utf-8")
        seen_statuses = set(
            re.findall(
                r"\|\s*(Not started|Partial|Implemented-unverified|Verified)\s*\|",
                matrix_text,
            )
        )
        invalid_statuses = seen_statuses - ALLOWED_REQUIREMENT_STATUSES
        if invalid_statuses:
            fail(f"traceability matrix has invalid statuses: {invalid_statuses}", errors)
        for status in ("Not started", "Partial", "Implemented-unverified"):
            if status not in seen_statuses:
                fail(f"traceability matrix does not exercise status: {status}", errors)

    if errors:
        print("documentation validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print(
        f"documentation validation passed: {len(REQUIRED)} required files, "
        f"{len(defined_fr)} master FR IDs"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
