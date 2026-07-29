#!/usr/bin/env python3
"""Validate the normative SDD, traceability, statuses, IDs, and local links."""

from __future__ import annotations

import ast
import re
import sys
from collections import Counter
from dataclasses import dataclass
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
    "docs/specs/017-real-only-image-desktop.md",
    "docs/risk-register.md",
    "docs/test-justifications.md",
    "docs/threat-model/THREAT_MODEL.md",
    "docs/evidence/environment-inventory.md",
    "docs/adr/README.md",
    "PLANS.md",
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

EXPECTED_NFR_IDS = {f"NFR-{index:03d}" for index in range(1, 9)}
EXPECTED_SDD016_IDS = {
    "SDD-HARD-001",
    "SDD-HARD-002",
    "SDD-HARD-003",
    "SDD-HARD-004",
    "SDD-HARD-005",
    "SDD-CLI-001",
    "SDD-CLI-002",
    "SDD-UI-001",
    "SDD-UI-002",
    "SDD-UI-003",
    "SDD-QA-001",
}
EXPECTED_SDD017_IDS = {f"SDD-REAL-{index:03d}" for index in range(1, 13)}
EXPECTED_ADR_TARGETS = {
    1: "0001-tauri-react-rust-target.md",
    2: "0008-process-separation-and-uac.md",
    3: "0002-read-only-source-invariant.md",
    4: "0009-ipc-protocol.md",
    5: "0010-sqlite-session-schema.md",
    6: "0011-ntfs-raw-parsing-and-active-record-apis.md",
    7: "0012-filesystem-plugin-architecture.md",
    8: "0013-carving-plugin-architecture.md",
    9: "0014-sandbox-strategy.md",
    10: "0006-explainable-recoverability-score.md",
    11: "0015-same-physical-disk-policy.md",
    12: "0016-dependency-and-license-policy.md",
    13: "0017-installer-and-update-strategy.md",
    14: "0018-refs-scope.md",
    15: "0019-windows-support-matrix.md",
    16: "0020-image-format-architecture.md",
    17: "0004-deterministic-synthetic-fixtures.md",
}

MATRIX_HEADER = [
    "Requirement",
    "Design section",
    "Code module",
    "Unit tests",
    "Integration tests",
    "E2E scenario",
    "Evidence artifact",
    "Status",
]

REQUIREMENT_FIELDS = {
    "Rationale",
    "Priority",
    "Source",
    "Preconditions",
    "Behavior",
    "Error behavior",
    "Security implications",
    "Observability",
    "Acceptance criteria",
    "Implementation links",
    "Status",
}

TEST_ID_PATTERN = re.compile(r"\b[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+-\d{3}\b")
JUST_ID_PATTERN = re.compile(r"\bJUST-[A-Z0-9-]+\b")
PLACEHOLDER_PATTERN = re.compile(r"\b(?:TBD|TODO|STUBS?)\b", re.IGNORECASE)


@dataclass(frozen=True)
class MatrixRow:
    requirements: tuple[str, ...]
    cells: tuple[str, ...]

    @property
    def status(self) -> str:
        return strip_code(self.cells[-1])


def strip_code(value: str) -> str:
    return value.strip().strip("`").strip()


def split_markdown_row(line: str) -> list[str]:
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def is_separator_row(cells: list[str]) -> bool:
    return bool(cells) and all(re.fullmatch(r":?-{3,}:?", cell) for cell in cells)


def is_allowed_status(status: str) -> bool:
    return status in ALLOWED_REQUIREMENT_STATUSES


def extract_sections(text: str, heading_prefix: str, id_pattern: str) -> dict[str, str]:
    heading = re.compile(
        rf"^{re.escape(heading_prefix)}\s+({id_pattern})(?:\s+[—-].*)?$",
        re.MULTILINE,
    )
    matches = list(heading.finditer(text))
    sections: dict[str, str] = {}
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(text)
        sections[match.group(1)] = text[match.start():end]
    return sections


def duplicate_section_ids(text: str, heading_prefix: str, id_pattern: str) -> list[str]:
    heading = re.compile(
        rf"^{re.escape(heading_prefix)}\s+({id_pattern})(?:\s+[—-].*)?$",
        re.MULTILINE,
    )
    counts = Counter(match.group(1) for match in heading.finditer(text))
    return sorted(identifier for identifier, count in counts.items() if count != 1)


def validate_exact_requirement_set(
    actual: set[str],
    expected: set[str],
    errors: list[str],
    context: str,
) -> None:
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    if missing:
        errors.append(f"{context}: missing requirements {missing}")
    if extra:
        errors.append(f"{context}: unknown requirements {extra}")


def validate_status_alignment(
    matrix_statuses: dict[str, str],
    normative_statuses: dict[str, str],
    errors: list[str],
    context: str,
) -> None:
    for requirement in sorted(set(matrix_statuses) & set(normative_statuses)):
        matrix_status = matrix_statuses[requirement]
        normative_status = normative_statuses[requirement]
        if matrix_status != normative_status:
            errors.append(
                f"{context}:{requirement}: status {matrix_status!r} "
                f"disagrees with normative status {normative_status!r}"
            )


def expand_fr_cell(cell: str) -> tuple[str, ...]:
    requirements: list[str] = []
    pattern = re.compile(r"FR-(\d{3})(?:\s*[–-]\s*(?:FR-)?(\d{3}))?")
    for match in pattern.finditer(cell):
        start = int(match.group(1))
        end = int(match.group(2) or match.group(1))
        if end < start:
            raise ValueError(f"descending requirement range: {match.group(0)}")
        requirements.extend(f"FR-{value:03d}" for value in range(start, end + 1))
    return tuple(requirements)


def expand_requirement_cell(cell: str) -> tuple[str, ...]:
    if re.search(r"\bSDD-[A-Z]+-\d{3}\b", cell):
        return tuple(re.findall(r"\bSDD-[A-Z]+-\d{3}\b", cell))
    if re.search(r"\bNFR-\d{3}\b", cell):
        return tuple(re.findall(r"\bNFR-\d{3}\b", cell))
    return expand_fr_cell(cell)


def governed_markdown_files(root: Path) -> list[Path]:
    files = [
        root / "README.md",
        root / "README.pt-BR.md",
        root / "SECURITY.md",
        root / "CONTRIBUTING.md",
    ]
    files.extend((root / "docs").rglob("*.md"))
    return sorted(path for path in files if path.is_file())


def validate_required_files(root: Path, errors: list[str]) -> None:
    for relative in REQUIRED:
        if not (root / relative).is_file():
            errors.append(f"missing required document: {relative}")


def validate_links_and_placeholders(root: Path, errors: list[str]) -> None:
    for path in governed_markdown_files(root):
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(root)

        placeholder = PLACEHOLDER_PATTERN.search(text)
        if placeholder:
            errors.append(
                f"{relative}: prohibited placeholder token {placeholder.group(0)!r}"
            )

        for match in re.finditer(r"\[[^\]]+\]\(([^)]+)\)", text):
            raw_target = match.group(1).strip()
            target, _, anchor = raw_target.partition("#")
            if (
                not target
                or target.startswith(("http://", "https://", "mailto:"))
                or target.startswith("<")
            ):
                continue
            candidate = (path.parent / target).resolve()
            try:
                candidate.relative_to(root.resolve())
            except ValueError:
                errors.append(f"{relative}: local link escapes repository: {raw_target}")
                continue
            if not candidate.exists():
                errors.append(f"{relative}: broken local link: {raw_target}")
                continue
            if anchor and candidate.is_dir():
                errors.append(f"{relative}: directory link cannot have anchor: {raw_target}")


def parse_functional_catalog(text: str) -> tuple[dict[str, str], list[str]]:
    statuses: dict[str, str] = {}
    duplicates: list[str] = []
    for line in text.splitlines():
        if not re.match(r"^\|\s*FR-\d{3}\s*\|", line):
            continue
        cells = split_markdown_row(line)
        requirement = cells[0]
        status = strip_code(cells[-1])
        if requirement in statuses:
            duplicates.append(requirement)
        statuses[requirement] = status
    return statuses, duplicates


def validate_functional_catalog(
    root: Path,
    master_ids: set[str],
    errors: list[str],
) -> dict[str, str]:
    path = root / "docs/specs/001-functional-requirements.md"
    if not path.is_file():
        return {}
    text = path.read_text(encoding="utf-8")
    catalog, duplicates = parse_functional_catalog(text)

    expected_header = "| ID | Title | Behavior and acceptance criteria | Status |"
    if expected_header not in text:
        errors.append(f"{path.relative_to(root)}: invalid functional table header")
    for line in text.splitlines():
        if not re.match(r"^\|\s*FR-\d{3}\s*\|", line):
            continue
        cells = split_markdown_row(line)
        if len(cells) != 4 or any(not cell for cell in cells):
            errors.append(f"{path.relative_to(root)}: malformed functional row {line}")

    if duplicates:
        errors.append(f"{path.relative_to(root)}: duplicate FR rows: {sorted(duplicates)}")
    missing = sorted(master_ids - set(catalog))
    extra = sorted(set(catalog) - master_ids)
    if missing:
        errors.append(f"{path.relative_to(root)}: missing master FRs: {missing}")
    if extra:
        errors.append(f"{path.relative_to(root)}: unknown FRs: {extra}")
    for requirement, status in catalog.items():
        if not is_allowed_status(status):
            errors.append(f"{path.relative_to(root)}: {requirement} invalid status {status!r}")

    inherited_fields = {
        "Rationale",
        "Priority",
        "Source",
        "Preconditions",
        "Error behavior",
        "Security implications",
        "Observability",
        "Test IDs or formal justification",
        "Implementation links",
    }
    for field in inherited_fields:
        if f"**{field}:" not in text:
            errors.append(f"{path.relative_to(root)}: missing inherited field {field}")
    return catalog


def parse_justifications(text: str) -> dict[str, set[str]]:
    sections = extract_sections(text, "##", r"JUST-[A-Z0-9-]+")
    result: dict[str, set[str]] = {}
    for just_id, section in sections.items():
        match = re.search(r"\*\*Requirements:\*\*(.*?)(?:\n- \*\*|\Z)", section, re.DOTALL)
        if match:
            result[just_id] = set(
                re.findall(r"\b(?:FR|NFR|SDD-[A-Z]+)-\d{3}\b", match.group(1))
            )
        else:
            result[just_id] = set()
    return result


def validate_justifications(
    root: Path,
    valid_requirements: set[str],
    errors: list[str],
) -> dict[str, set[str]]:
    path = root / "docs/test-justifications.md"
    if not path.is_file():
        return {}
    text = path.read_text(encoding="utf-8")
    duplicates = duplicate_section_ids(text, "##", r"JUST-[A-Z0-9-]+")
    if duplicates:
        errors.append(f"{path.relative_to(root)}: duplicate justification headings {duplicates}")
    justifications = parse_justifications(text)
    for just_id, requirements in justifications.items():
        section = extract_sections(text, "##", re.escape(just_id))[just_id]
        if not requirements:
            errors.append(f"{path.relative_to(root)}: {just_id} has no requirements")
        unknown = sorted(requirements - valid_requirements)
        if unknown:
            errors.append(
                f"{path.relative_to(root)}: {just_id} references unknown requirements {unknown}"
            )
        if "**Formal rationale:**" not in section:
            errors.append(f"{path.relative_to(root)}: {just_id} lacks formal rationale")
        if "**Exit criterion:**" not in section:
            errors.append(f"{path.relative_to(root)}: {just_id} lacks exit criterion")
    return justifications


def parse_matrix_section(text: str, heading: str) -> tuple[list[str], list[MatrixRow]]:
    marker = f"## {heading}"
    start = text.find(marker)
    if start < 0:
        return [], []
    following = text.find("\n## ", start + len(marker))
    section = text[start: following if following >= 0 else len(text)]
    table_lines = [line for line in section.splitlines() if line.startswith("|")]
    if len(table_lines) < 2:
        return [], []
    header = split_markdown_row(table_lines[0])
    rows: list[MatrixRow] = []
    for line in table_lines[1:]:
        cells = split_markdown_row(line)
        if is_separator_row(cells):
            continue
        try:
            requirements = expand_requirement_cell(cells[0])
        except ValueError:
            requirements = ()
        rows.append(MatrixRow(requirements=requirements, cells=tuple(cells)))
    return header, rows


def normalize_test_name(value: str) -> str:
    return re.sub(r"[^A-Z0-9]+", "-", value.upper()).strip("-")


def without_c_style_comments(
    text: str,
    *,
    nested_block_comments: bool = False,
    mask_strings: bool = False,
    single_quoted_strings: bool = True,
    backtick_strings: bool = True,
) -> str:
    """Mask comments and, optionally, strings while preserving source layout."""

    output: list[str] = []
    index = 0
    quote: str | None = None
    raw_string_end: str | None = None
    line_comment = False
    block_depth = 0
    escaped = False

    def masked(character: str) -> str:
        return character if character in "\r\n" else " "

    while index < len(text):
        character = text[index]
        following = text[index + 1] if index + 1 < len(text) else ""

        if line_comment:
            if character in "\r\n":
                line_comment = False
                output.append(character)
            else:
                output.append(" ")
            index += 1
            continue

        if block_depth:
            if nested_block_comments and character == "/" and following == "*":
                output.extend((" ", " "))
                block_depth += 1
                index += 2
                continue
            if character == "*" and following == "/":
                output.extend((" ", " "))
                block_depth -= 1
                index += 2
                continue
            output.append(masked(character))
            index += 1
            continue

        if raw_string_end is not None:
            if text.startswith(raw_string_end, index):
                output.extend(" " for _ in raw_string_end)
                index += len(raw_string_end)
                raw_string_end = None
            else:
                output.append(masked(character))
                index += 1
            continue

        if quote is not None:
            output.append(masked(character) if mask_strings else character)
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == quote:
                quote = None
            index += 1
            continue

        if character == "/" and following == "/":
            output.extend((" ", " "))
            index += 2
            line_comment = True
            continue
        if character == "/" and following == "*":
            output.extend((" ", " "))
            index += 2
            block_depth = 1
            continue

        if nested_block_comments and mask_strings:
            raw_match = re.match(r"(?:br|r)(?P<hashes>#{0,255})\"", text[index:])
            if raw_match is not None:
                prefix = raw_match.group(0)
                output.extend(" " for _ in prefix)
                index += len(prefix)
                raw_string_end = '"' + raw_match.group("hashes")
                continue

        quotes = {'"'}
        if single_quoted_strings:
            quotes.add("'")
        if backtick_strings:
            quotes.add("`")
        if character in quotes:
            quote = character
            output.append(" " if mask_strings else character)
            index += 1
            continue

        output.append(character)
        index += 1

    return "".join(output)


def web_test_names(text: str) -> list[str]:
    source = without_c_style_comments(
        text,
        single_quoted_strings=True,
        backtick_strings=True,
    )
    names: list[str] = []
    index = 0

    def regex_literal_can_start(position: int) -> bool:
        cursor = position - 1
        while cursor >= 0 and source[cursor].isspace():
            cursor -= 1
        if cursor < 0 or source[cursor] in "([{:;,=!?&|+-*%^~<>":
            return True
        end = cursor + 1
        while cursor >= 0 and (
            source[cursor].isalnum() or source[cursor] in {"_", "$"}
        ):
            cursor -= 1
        return source[cursor + 1:end] in {
            "await",
            "case",
            "delete",
            "do",
            "else",
            "in",
            "instanceof",
            "new",
            "return",
            "throw",
            "typeof",
            "void",
            "yield",
        }

    while index < len(source):
        character = source[index]
        if character == "/" and regex_literal_can_start(index):
            index += 1
            escaped = False
            in_character_class = False
            while index < len(source):
                current = source[index]
                if current in "\r\n":
                    break
                if escaped:
                    escaped = False
                elif current == "\\":
                    escaped = True
                elif current == "[":
                    in_character_class = True
                elif current == "]":
                    in_character_class = False
                elif current == "/" and not in_character_class:
                    index += 1
                    while index < len(source) and source[index].isalpha():
                        index += 1
                    break
                index += 1
            continue
        if character in {'"', "'", "`"}:
            quote = character
            index += 1
            escaped = False
            while index < len(source):
                current = source[index]
                if escaped:
                    escaped = False
                elif current == "\\":
                    escaped = True
                elif current == quote:
                    index += 1
                    break
                index += 1
            continue
        if not (character.isalpha() or character in {"_", "$"}):
            index += 1
            continue

        start = index
        index += 1
        while index < len(source) and (
            source[index].isalnum() or source[index] in {"_", "$"}
        ):
            index += 1
        identifier = source[start:index]
        if identifier not in {"it", "test"}:
            continue
        if start > 0 and source[start - 1] == ".":
            continue
        cursor = index
        while cursor < len(source) and source[cursor].isspace():
            cursor += 1
        if cursor >= len(source) or source[cursor] != "(":
            continue
        cursor += 1
        while cursor < len(source) and source[cursor].isspace():
            cursor += 1
        if cursor >= len(source) or source[cursor] not in {'"', "'", "`"}:
            continue

        quote = source[cursor]
        cursor += 1
        title: list[str] = []
        escaped = False
        while cursor < len(source):
            current = source[cursor]
            if escaped:
                title.append(current)
                escaped = False
            elif current == "\\":
                escaped = True
            elif current == quote:
                names.append("".join(title))
                cursor += 1
                break
            else:
                title.append(current)
            cursor += 1
        index = cursor
    return names


def executable_test_declarations(root: Path) -> set[str]:
    declarations: set[str] = set()

    rust_pattern = re.compile(
        r"#\s*\[\s*(?:[A-Za-z0-9_:]+::)?test(?:\s*\([^\]]*\))?\s*\]"
        r"(?:\s*#\s*\[[^\]]+\])*\s*"
        r"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)",
        re.MULTILINE,
    )
    rust_roots = [root / "crates", *root.glob("apps/*/src-tauri")]
    for rust_root in rust_roots:
        if not rust_root.is_dir():
            continue
        for path in rust_root.rglob("*.rs"):
            if not path.is_file():
                continue
            text = without_c_style_comments(
                path.read_text(encoding="utf-8"),
                nested_block_comments=True,
                mask_strings=True,
                single_quoted_strings=False,
                backtick_strings=False,
            )
            declarations.update(
                normalize_test_name(name) for name in rust_pattern.findall(text)
            )

    for path in (root / ".github/scripts/tests").rglob("*.py"):
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        try:
            tree = ast.parse(text, filename=str(path))
        except SyntaxError:
            continue
        declarations.update(
            normalize_test_name(node.name)
            for node in ast.walk(tree)
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and node.name.startswith("test_")
        )

    apps_root = root / "apps"
    if apps_root.is_dir():
        for source_root in apps_root.glob("*/src"):
            if not source_root.is_dir():
                continue
            for path in source_root.rglob("*"):
                if (
                    not path.is_file()
                    or not re.search(r"\.(?:test|spec)\.[cm]?[jt]sx?$", path.name)
                ):
                    continue
                declarations.update(
                    normalize_test_name(name)
                    for name in web_test_names(path.read_text(encoding="utf-8"))
                )

    # Python unittest convention prefixes method names with "test_"; both the
    # full declaration and the canonical suffix are resolvable.
    declarations.update(
        declaration.removeprefix("TEST-")
        for declaration in tuple(declarations)
        if declaration.startswith("TEST-")
    )
    return declarations


def test_id_is_resolvable(test_id: str, declarations: set[str]) -> bool:
    normalized = normalize_test_name(test_id)
    return any(
        declaration == normalized or declaration.startswith(f"{normalized}-")
        for declaration in declarations
    )


def validate_test_references(
    row: MatrixRow,
    justifications: dict[str, set[str]],
    test_declarations: set[str],
    errors: list[str],
    context: str,
) -> None:
    test_area = " ".join(row.cells[3:7])
    test_ids = {
        test_id
        for test_id in TEST_ID_PATTERN.findall(test_area)
        if not test_id.startswith(("JUST-", "REAL-AC-"))
    }
    just_ids = set(JUST_ID_PATTERN.findall(test_area))
    if not test_ids and not just_ids:
        errors.append(f"{context}: no executable test ID or formal justification")
    for test_id in sorted(test_ids):
        if not test_id_is_resolvable(test_id, test_declarations):
            errors.append(f"{context}: test ID is not resolvable in test source: {test_id}")
    for just_id in sorted(just_ids):
        if just_id not in justifications:
            errors.append(f"{context}: undefined formal justification {just_id}")
            continue
        if row.requirements and not set(row.requirements) <= justifications[just_id]:
            errors.append(
                f"{context}: {just_id} does not cover {sorted(row.requirements)}"
            )


def validate_code_paths(root: Path, row: MatrixRow, errors: list[str], context: str) -> None:
    code_cell = row.cells[2]
    paths = re.findall(r"`([^`]+)`", code_cell)
    if row.status == "Not started":
        if paths:
            errors.append(f"{context}: Not started row must not claim implementation paths")
        return
    if not paths:
        errors.append(f"{context}: implemented/partial row lacks versioned code path")
        return
    for relative in paths:
        if not (root / relative).exists():
            errors.append(f"{context}: implementation path does not exist: {relative}")


def validate_matrix(
    root: Path,
    master_ids: set[str],
    catalog: dict[str, str],
    nfr_statuses: dict[str, str],
    sdd016_statuses: dict[str, str],
    sdd017_statuses: dict[str, str],
    justifications: dict[str, set[str]],
    errors: list[str],
) -> None:
    path = root / "docs/traceability-matrix.md"
    if not path.is_file():
        return
    text = path.read_text(encoding="utf-8")
    test_declarations = executable_test_declarations(root)

    current_header, current_rows = parse_matrix_section(
        text, "Current increment requirements"
    )
    real_header, real_rows = parse_matrix_section(
        text, "Current real-only image desktop increment"
    )
    fr_header, fr_rows = parse_matrix_section(
        text, "Master functional requirements with current evidence"
    )
    nfr_header, nfr_rows = parse_matrix_section(
        text, "Non-functional requirements"
    )
    for header, name in (
        (current_header, "current increment"),
        (real_header, "real-only"),
        (fr_header, "master FR"),
        (nfr_header, "NFR"),
    ):
        if header != MATRIX_HEADER:
            errors.append(f"{path.relative_to(root)}: invalid {name} matrix header {header}")

    def validate_rows(
        rows: list[MatrixRow],
        expected_ids: set[str],
        normative_statuses: dict[str, str],
        label: str,
    ) -> None:
        counts: Counter[str] = Counter()
        row_statuses: dict[str, str] = {}
        for row in rows:
            if len(row.cells) != len(MATRIX_HEADER):
                errors.append(f"{path.relative_to(root)}: malformed row {row.cells}")
                continue
            if not row.requirements:
                errors.append(
                    f"{path.relative_to(root)}: invalid {label} expression {row.cells[0]}"
                )
                continue
            context = f"{path.relative_to(root)}:{row.cells[0]}"
            for requirement in row.requirements:
                counts[requirement] += 1
                row_statuses[requirement] = row.status
            if not is_allowed_status(row.status):
                errors.append(f"{context}: invalid status {row.status!r}")
            validate_code_paths(root, row, errors, context)
            validate_test_references(
                row,
                justifications,
                test_declarations,
                errors,
                context,
            )

        validate_exact_requirement_set(
            set(counts),
            expected_ids,
            errors,
            f"{path.relative_to(root)}:{label} matrix",
        )
        duplicates = sorted(
            requirement for requirement, count in counts.items() if count != 1
        )
        if duplicates:
            errors.append(
                f"{path.relative_to(root)}: {label} requirements mapped more than once: "
                f"{duplicates}"
            )
        validate_status_alignment(
            row_statuses,
            normative_statuses,
            errors,
            f"{path.relative_to(root)}:{label} matrix",
        )

    validate_rows(fr_rows, master_ids, catalog, "FR")
    validate_rows(nfr_rows, EXPECTED_NFR_IDS, nfr_statuses, "NFR")
    validate_rows(
        current_rows,
        EXPECTED_SDD016_IDS,
        sdd016_statuses,
        "current increment",
    )
    validate_rows(
        real_rows,
        EXPECTED_SDD017_IDS,
        sdd017_statuses,
        "real-only",
    )


def validate_requirement_sections(
    root: Path,
    relative: str,
    heading_prefix: str,
    id_pattern: str,
    expected_ids: set[str],
    justifications: dict[str, set[str]],
    errors: list[str],
    required_labels: tuple[str, ...] = (),
) -> dict[str, str]:
    path = root / relative
    text = path.read_text(encoding="utf-8")
    test_declarations = executable_test_declarations(root)
    duplicates = duplicate_section_ids(text, heading_prefix, id_pattern)
    if duplicates:
        errors.append(f"{relative}: duplicate requirement headings {duplicates}")
    sections = extract_sections(text, heading_prefix, id_pattern)
    validate_exact_requirement_set(
        set(sections),
        expected_ids,
        errors,
        f"{relative}:normative catalog",
    )
    statuses: dict[str, str] = {}
    for requirement, section in sections.items():
        for label in required_labels:
            if f"**{label}:" not in section:
                errors.append(f"{relative}:{requirement}: missing field group {label}")
        status_match = re.search(
            r"- \*\*Status:\*\*\s*`?([^`\n.]+)`?\.", section
        )
        if not status_match:
            errors.append(f"{relative}:{requirement}: missing explicit status")
            continue
        status = status_match.group(1).strip()
        statuses[requirement] = status
        if not is_allowed_status(status):
            errors.append(f"{relative}:{requirement}: invalid status {status!r}")

        priority_match = re.search(r"\b(Must|High|Should|Could)\b", section)
        if not priority_match or priority_match.group(1) not in {"Must", "High"}:
            continue
        test_ids = {
            test_id
            for test_id in TEST_ID_PATTERN.findall(section)
            if test_id != requirement
            and not test_id.startswith(("JUST-", "REAL-AC-"))
        }
        just_ids = set(JUST_ID_PATTERN.findall(section))
        if not test_ids and not just_ids:
            errors.append(
                f"{relative}:{requirement}: Must/High lacks test ID or justification"
            )
        for test_id in sorted(test_ids):
            if not test_id_is_resolvable(test_id, test_declarations):
                errors.append(
                    f"{relative}:{requirement}: unresolved test ID {test_id}"
                )
        for just_id in sorted(just_ids):
            if just_id not in justifications:
                errors.append(
                    f"{relative}:{requirement}: undefined justification {just_id}"
                )
            elif requirement not in justifications[just_id]:
                errors.append(
                    f"{relative}:{requirement}: {just_id} does not cover requirement"
                )
    return statuses


def validate_sdd_fields_and_paths(
    root: Path,
    relative_path: str,
    id_pattern: str,
    errors: list[str],
) -> None:
    path = root / relative_path
    text = path.read_text(encoding="utf-8")
    sections = extract_sections(text, "###", id_pattern)
    for requirement, section in sections.items():
        for field in REQUIREMENT_FIELDS:
            if f"**{field}:" not in section:
                errors.append(f"{path.relative_to(root)}:{requirement}: missing {field}")
        if "**Test IDs:" not in section and "**Test IDs or formal justification:" not in section:
            errors.append(
                f"{path.relative_to(root)}:{requirement}: missing test/justification field"
            )
        status_match = re.search(r"- \*\*Status:\*\*\s*([^.\n]+)\.", section)
        if not status_match:
            continue
        status = strip_code(status_match.group(1))
        implementation = re.search(
            r"- \*\*Implementation links:\*\*(.*?)(?=\n- \*\*Status:)",
            section,
            re.DOTALL,
        )
        paths = re.findall(r"`([^`]+)`", implementation.group(1) if implementation else "")
        if status == "Not started" and paths:
            errors.append(
                f"{path.relative_to(root)}:{requirement}: Not started claims paths {paths}"
            )
        if status != "Not started":
            if not paths:
                errors.append(
                    f"{path.relative_to(root)}:{requirement}: no implementation path"
                )
            for relative in paths:
                if not (root / relative).exists():
                    errors.append(
                        f"{path.relative_to(root)}:{requirement}: missing path {relative}"
                    )


def validate_justification_usage(
    root: Path,
    justifications: dict[str, set[str]],
    errors: list[str],
) -> None:
    references: set[str] = set()
    for relative in (
        "docs/traceability-matrix.md",
        "docs/specs/002-non-functional-requirements.md",
        "docs/specs/016-foundation-hardening-and-safe-image-cli.md",
        "docs/specs/017-real-only-image-desktop.md",
    ):
        path = root / relative
        if path.is_file():
            references.update(JUST_ID_PATTERN.findall(path.read_text(encoding="utf-8")))
    undefined = sorted(references - set(justifications))
    unused = sorted(set(justifications) - references)
    if undefined:
        errors.append(f"formal justification references are undefined: {undefined}")
    if unused:
        errors.append(f"formal justifications are not referenced: {unused}")


def validate_s0_inventory(root: Path, errors: list[str]) -> None:
    path = root / "docs/evidence/environment-inventory.md"
    if not path.is_file():
        return
    text = path.read_text(encoding="utf-8")
    required_sections = (
        "## Assumptions",
        "## Agents, skills, plugins, and MCP inventory",
        "## Preliminary risks",
    )
    for heading in required_sections:
        if heading not in text:
            errors.append(f"{path.relative_to(root)}: missing S0 section {heading[3:]}")

    def section(heading: str) -> str:
        start = text.find(heading)
        if start < 0:
            return ""
        end = text.find("\n## ", start + len(heading))
        return text[start: end if end >= 0 else len(text)]

    assumptions = re.findall(r"\bA-S0-\d{3}\b", section("## Assumptions"))
    if len(set(assumptions)) < 4 or len(assumptions) != len(set(assumptions)):
        errors.append(
            f"{path.relative_to(root)}: assumptions require at least four unique A-S0 IDs"
        )

    inventory = section("## Agents, skills, plugins, and MCP inventory")
    if "| Category | Used capability | Purpose / boundary |" not in inventory:
        errors.append(f"{path.relative_to(root)}: invalid S0 capability inventory table")
    inventory_categories = set(
        re.findall(r"^\|\s*(Agent|Skill/plugin|MCP/tool|Plugin)\s*\|", inventory, re.MULTILINE)
    )
    if not {"Agent", "Skill/plugin", "MCP/tool"} <= inventory_categories:
        errors.append(
            f"{path.relative_to(root)}: S0 inventory must include agents, "
            "skills/plugins, and MCP/tools"
        )

    risks = re.findall(
        r"\[(R-\d{3})\]\(\.\./risk-register\.md(?:#[^)]+)?\)",
        section("## Preliminary risks"),
    )
    if len(set(risks)) < 4 or len(risks) != len(set(risks)):
        errors.append(
            f"{path.relative_to(root)}: preliminary risks require at least four "
            "unique risk-register links"
        )


def adr_master_topic(text: str) -> int | None:
    match = re.search(
        r"^Master specification topic:\s*(\d+)\s+[—-]\s+.+$",
        text,
        re.MULTILINE,
    )
    return int(match.group(1)) if match else None


def adr_status(text: str) -> str | None:
    direct = re.search(r"^Status:\s*(Accepted|Proposed)\b", text, re.MULTILINE)
    if direct:
        return direct.group(1)
    section = re.search(
        r"^## Status\s*$\s*(Accepted|Proposed)\b", text, re.MULTILINE
    )
    return section.group(1) if section else None


def validate_adrs(root: Path, errors: list[str]) -> None:
    index = root / "docs/adr/README.md"
    if not index.is_file():
        return
    text = index.read_text(encoding="utf-8")
    topic_rows: dict[int, tuple[str, str]] = {}
    for line in text.splitlines():
        match = re.match(
            r"^\|\s*(\d+)\s*\|\s*\[[^\]]+\]\(([^)]+)\)\s*\|.*\|\s*(Accepted|Proposed)\s*\|$",
            line,
        )
        if not match:
            continue
        topic = int(match.group(1))
        if topic in topic_rows:
            errors.append(f"{index.relative_to(root)}: duplicate ADR topic {topic}")
        topic_rows[topic] = (match.group(2), match.group(3))
    expected = set(EXPECTED_ADR_TARGETS)
    if set(topic_rows) != expected:
        errors.append(
            f"{index.relative_to(root)}: ADR topics must be exactly 1..17; "
            f"found {sorted(topic_rows)}"
        )
    for topic, (target, status) in topic_rows.items():
        expected_target = EXPECTED_ADR_TARGETS.get(topic)
        if expected_target is not None and target != expected_target:
            errors.append(
                f"{index.relative_to(root)}: topic {topic} must reference "
                f"{expected_target}, found {target}"
            )
        path = (index.parent / target).resolve()
        if not path.is_file():
            errors.append(f"{index.relative_to(root)}: topic {topic} missing ADR {target}")
            continue
        adr_text = path.read_text(encoding="utf-8")
        actual_topic = adr_master_topic(adr_text)
        if actual_topic != topic:
            errors.append(
                f"{path.relative_to(root)}: declared master topic "
                f"{actual_topic!r} does not match index topic {topic}"
            )
        actual_status = adr_status(adr_text)
        if actual_status != status:
            errors.append(
                f"{path.relative_to(root)}: index status {status} "
                f"does not match ADR status {actual_status}"
            )


def validate_repository(root: Path) -> tuple[list[str], dict[str, int]]:
    errors: list[str] = []
    validate_required_files(root, errors)

    master = root / "UNDELETE_MASTER_CODEX_MASTER_SPEC.md"
    if not master.is_file():
        errors.append("missing master specification")
        return errors, {}
    master_text = master.read_text(encoding="utf-8")
    master_ids = set(re.findall(r"^### (FR-\d{3})\b", master_text, re.MULTILINE))

    validate_links_and_placeholders(root, errors)
    catalog = validate_functional_catalog(root, master_ids, errors)
    valid_requirements = (
        master_ids | EXPECTED_NFR_IDS | EXPECTED_SDD016_IDS | EXPECTED_SDD017_IDS
    )
    justifications = validate_justifications(root, valid_requirements, errors)
    nfr_statuses = validate_requirement_sections(
        root,
        "docs/specs/002-non-functional-requirements.md",
        "##",
        r"NFR-\d{3}",
        EXPECTED_NFR_IDS,
        justifications,
        errors,
        (
            "Rationale / priority / source",
            "Preconditions / behavior / errors",
            "Security / observability",
            "Acceptance / tests / implementation",
            "Status",
        ),
    )
    sdd016_statuses = validate_requirement_sections(
        root,
        "docs/specs/016-foundation-hardening-and-safe-image-cli.md",
        "###",
        r"SDD-[A-Z]+-\d{3}",
        EXPECTED_SDD016_IDS,
        justifications,
        errors,
    )
    sdd017_statuses = validate_requirement_sections(
        root,
        "docs/specs/017-real-only-image-desktop.md",
        "###",
        r"SDD-REAL-\d{3}",
        EXPECTED_SDD017_IDS,
        justifications,
        errors,
    )
    validate_sdd_fields_and_paths(
        root,
        "docs/specs/016-foundation-hardening-and-safe-image-cli.md",
        r"SDD-[A-Z]+-\d{3}",
        errors,
    )
    validate_sdd_fields_and_paths(
        root,
        "docs/specs/017-real-only-image-desktop.md",
        r"SDD-REAL-\d{3}",
        errors,
    )
    validate_matrix(
        root,
        master_ids,
        catalog,
        nfr_statuses,
        sdd016_statuses,
        sdd017_statuses,
        justifications,
        errors,
    )
    validate_justification_usage(root, justifications, errors)
    validate_s0_inventory(root, errors)
    validate_adrs(root, errors)

    stats = {
        "required_files": len(REQUIRED),
        "master_frs": len(master_ids),
        "catalog_frs": len(catalog),
        "nfrs": len(nfr_statuses),
        "sdd016_requirements": len(sdd016_statuses),
        "sdd017_requirements": len(sdd017_statuses),
        "justifications": len(justifications),
        "adr_topics": 17,
    }
    return errors, stats


def main() -> int:
    errors, stats = validate_repository(ROOT)
    if errors:
        print("documentation validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print(
        "documentation validation passed: "
        f"{stats['catalog_frs']}/{stats['master_frs']} exact FR catalog and matrix, "
        f"{stats['nfrs']} NFR, {stats['sdd016_requirements']} foundation and "
        f"{stats['sdd017_requirements']} real-only requirements, "
        f"{stats['adr_topics']} required ADR topics, "
        f"{stats['justifications']} formal justifications, "
        "statuses/test references/local paths checked"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
