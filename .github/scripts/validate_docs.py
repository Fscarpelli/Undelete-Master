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
    "docs/specs/018-windows-volume-and-folder-scan.md",
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
EXPECTED_SDD018_IDS = {f"SDD-WIN-{index:03d}" for index in range(1, 14)}
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
CURRENT_PRODUCT_DOCUMENTS = (
    "README.md",
    "README.pt-BR.md",
    "apps/desktop/README.md",
    "docs/specs/001-functional-requirements.md",
    "docs/specs/003-domain-model.md",
    "docs/specs/004-architecture.md",
    "docs/specs/009-restore-semantics.md",
    "docs/specs/010-ux-ui-and-accessibility.md",
    "docs/specs/011-security-and-privacy.md",
    "docs/specs/012-test-and-validation-plan.md",
    "docs/specs/015-known-limitations.md",
    "docs/specs/018-windows-volume-and-folder-scan.md",
    "docs/specs/019-ntfs-coverage-and-jpeg-deep-scan.md",
    "docs/specs/020-actionable-results-and-transactional-restore.md",
    "docs/adr/0028-restore-plan-job-and-manifest-lifecycle.md",
    "docs/risk-register.md",
    "docs/traceability-matrix.md",
    "docs/threat-model/THREAT_MODEL.md",
)
NUMBER_WORDS = {
    "zero": 0,
    "one": 1,
    "um": 1,
    "uma": 1,
    "two": 2,
    "dois": 2,
    "duas": 2,
    "three": 3,
    "três": 3,
    "tres": 3,
    "four": 4,
    "quatro": 4,
    "five": 5,
    "cinco": 5,
    "six": 6,
    "seis": 6,
    "seven": 7,
    "sete": 7,
    "eight": 8,
    "oito": 8,
    "nine": 9,
    "nove": 9,
    "ten": 10,
    "dez": 10,
    "eleven": 11,
    "onze": 11,
    "twelve": 12,
    "doze": 12,
    "thirteen": 13,
    "treze": 13,
    "fourteen": 14,
    "catorze": 14,
    "quatorze": 14,
    "fifteen": 15,
    "quinze": 15,
    "sixteen": 16,
    "dezesseis": 16,
    "seventeen": 17,
    "dezessete": 17,
    "eighteen": 18,
    "dezoito": 18,
    "nineteen": 19,
    "dezenove": 19,
    "twenty": 20,
    "vinte": 20,
}
NUMBER_TOKEN = (
    r"(?:\d+|"
    + "|".join(
        sorted((re.escape(word) for word in NUMBER_WORDS), key=len, reverse=True)
    )
    + r")"
)
DOCUMENTED_COMMAND_COUNT_PATTERNS = (
    re.compile(
        rf"\b(?:exact(?:ly)?|exatamente)?\s*(?P<count>{NUMBER_TOKEN})\s+"
        rf"(?:(?:[\w/-]+\s+){{0,3}}Tauri|"
        rf"(?:desktop|native)(?:\s+(?:desktop|native))?)\s+commands?\b",
        re.IGNORECASE,
    ),
    re.compile(
        rf"\b(?:exatamente\s+)?(?P<count>{NUMBER_TOKEN})\s+comandos?\s+"
        rf"(?:Tauri|nativ[oa]s?|do\s+desktop)\b",
        re.IGNORECASE,
    ),
    re.compile(
        rf"\b(?:exact(?:ly)?\s+)?(?P<count>{NUMBER_TOKEN})\s+commands?\s+"
        rf"(?:Tauri|native|desktop)\b",
        re.IGNORECASE,
    ),
    re.compile(
        rf"\b(?:Tauri|desktop(?:\s+native)?|native(?:\s+desktop)?)\s+"
        rf"commands?\s*(?:[:=]|(?:number|count|total)\s+(?:is|of)?\s*)"
        rf"\s*(?P<count>{NUMBER_TOKEN})\b",
        re.IGNORECASE,
    ),
    re.compile(
        rf"\b(?:comandos?\s+(?:Tauri|nativ[oa]s?|do\s+desktop)|"
        rf"(?:Tauri|nativ[oa]s?|desktop)\s+comandos?)\s*"
        rf"(?:[:=]|(?:somam|total(?:izam)?|total\s+(?:de|é))\s*)"
        rf"\s*(?P<count>{NUMBER_TOKEN})\b",
        re.IGNORECASE,
    ),
    re.compile(
        rf"\b(?:inventory|registry|surface|contract|invent[aá]rio|registro|"
        rf"superf[ií]cie|contrato)\b[^.\n]{{0,60}}?\b"
        rf"(?:exact(?:ly)?|exatamente)?\s*(?P<count>{NUMBER_TOKEN})\s+"
        rf"(?:Tauri\s+)?(?:commands?|comandos?)(?:\s+Tauri)?\b",
        re.IGNORECASE,
    ),
    re.compile(
        rf"\b(?P<count>{NUMBER_TOKEN})-(?:commands?|comandos?)\b"
        rf"(?=[^.\n]{{0,80}}\b(?:Tauri|desktop|native|nativ[oa]s?|scan|"
        rf"inventory|registry|surface|contract|invent[aá]rio|registro|"
        rf"superf[ií]cie|contrato)\b)",
        re.IGNORECASE,
    ),
)
AGGREGATED_COMMAND_COUNT_PATTERNS = (
    re.compile(
        rf"\b(?:the\s+)?(?:existing\s+)?(?P<first>{NUMBER_TOKEN})\s+"
        rf"(?:[\w/-]+\s+){{0,3}}commands?\s+(?:and|plus)\s+"
        rf"(?:these\s+|existing\s+)?(?P<second>{NUMBER_TOKEN})\s+"
        rf"(?:[\w/-]+\s+){{0,3}}commands?\s+"
        rf"(?:are|form|make|constitute|comprise)\s+(?:the\s+)?"
        rf"(?:complete|full|total)\s+(?:desktop\s+)?(?:command\s+)?inventory\b",
        re.IGNORECASE,
    ),
)
HISTORICAL_CONTEXT_PATTERN = re.compile(
    r"\b(?:historical|historic|hist[oó]ric[oa]s?|baseline|linha\s+de\s+base|"
    r"original(?:ly|mente)?\s+(?:contract|inventory|surface|state|status|"
    r"implementation|behavior|behaviour|design|architecture|problem|scope|"
    r"increment|release|contrato|invent[aá]rio|superf[ií]cie|estado|"
    r"implementa[cç][aã]o|comportamento|desenho|arquitetura|problema|escopo|"
    r"incremento|vers[aã]o)|(?:contrato|invent[aá]rio|superf[ií]cie|estado|"
    r"implementa[cç][aã]o|comportamento|desenho|arquitetura|problema|escopo|"
    r"incremento|vers[aã]o)\s+original)\b",
    re.IGNORECASE,
)
HISTORICAL_QUALIFIED_CONTEXT_PATTERN = re.compile(
    r"\b(?:(?:previous|prior|former|earlier)\s+"
    r"(?:[\w/-]+\s+){0,3}(?:contract|inventory|surface|state|status|"
    r"implementation|behavior|behaviour|design|architecture|problem|scope|"
    r"increment|release|version|baseline)|"
    r"(?:contract|inventory|surface|state|status|implementation|behavior|"
    r"behaviour|design|architecture|problem|scope|increment|release|version|"
    r"baseline)\s+(?:previous|prior|former|earlier)|"
    r"(?:anterior(?:es)?|pr[eé]vi[oa]s?|antig[oa]s?)\s+(?:contrato|"
    r"invent[aá]rio|superf[ií]cie|estado|implementa[cç][aã]o|comportamento|"
    r"desenho|arquitetura|problema|escopo|incremento|vers[aã]o|linha\s+de\s+"
    r"base)|(?:contrato|invent[aá]rio|superf[ií]cie|estado|"
    r"implementa[cç][aã]o|comportamento|desenho|arquitetura|problema|escopo|"
    r"incremento|vers[aã]o|linha\s+de\s+base)\s+(?:anterior(?:es)?|"
    r"pr[eé]vi[oa]s?|antig[oa]s?)|previously|formerly|anteriormente|"
    r"previamente)\b",
    re.IGNORECASE,
)
HISTORICAL_COMPARISON_PATTERN = re.compile(
    r"\b(?:compared\s+(?:with|to)|in\s+comparison\s+(?:with|to)|unlike|"
    r"versus|vs\.?|comparad[oa]\s+com|em\s+compara[cç][aã]o\s+com|"
    r"ao\s+contr[aá]rio\s+de)\b",
    re.IGNORECASE,
)
INLINE_HISTORICAL_TRANSITION_PATTERN = re.compile(
    r";|,\s*(?:but|however|while|whereas|yet|mas|por[eé]m|enquanto)\b|"
    r"\b(?:became|becomes|has\s+become|now|current(?:ly)?|today|agora|"
    r"atualmente|passou\s+a\s+ser|tornou-se)\b",
    re.IGNORECASE,
)
CURRENT_CONTEXT_PATTERN = re.compile(
    r"\b(?:current|currently|present|now|atual|atualmente|agora)\b",
    re.IGNORECASE,
)
MARKDOWN_HEADING_PATTERN = re.compile(r"^(?P<marks>#{1,6})\s+(?P<title>.+?)\s*$")
MARKDOWN_FENCE_PATTERN = re.compile(r"^\s*(?P<marker>`{3,}|~{3,})")
EXPECTED_TAURI_REGISTRATIONS = {
    "storage::list_storage_sources",
    "storage::select_scan_folder",
    "storage::scan_storage_volume",
    "storage::get_candidate_page",
    "storage::query_candidate_page",
    "storage::update_candidate_selection",
    "restore::select_restore_destination",
    "restore::create_restore_plan",
    "restore::start_restore",
    "restore::get_restore_job",
    "restore::cancel_restore",
    "restore::open_restore_destination",
}
OBSOLETE_CAPABILITY_CELL_PATTERN = re.compile(
    r"(?:file\s+)?(?:restore|restoration|recovery)|"
    r"restaura[cç][aã]o|recupera[cç][aã]o",
    re.IGNORECASE,
)
OBSOLETE_CAPABILITY_STATUS_CELL_PATTERN = re.compile(
    r"(?:absent|unavailable|unsupported|not\s+(?:available|implemented|"
    r"supported|started)|ausente|indispon[ií]vel|n[aã]o\s+(?:implementad[oa]|"
    r"suportad[oa]|iniciad[oa]))",
    re.IGNORECASE,
)
OBSOLETE_CURRENT_STATUS_PATTERNS = (
    (
        "restore execution",
        re.compile(
            r"\bNeither\s+(?:the\s+)?desktop\s+nor\s+(?:the\s+)?CLI\s+"
            r"(?:can\s+)?restor(?:e|es)\s+(?:any\s+)?data\b"
            r"|\b(?:the\s+)?desktop\s+(?:cannot|can['’]t|does\s+not|"
            r"doesn['’]t)\s+restore\s+(?:any\s+)?data\b"
            r"|\brestore(?:\s+(?:execution|engine|boundary|boundaries))?\s+"
            r"(?:is|remains?|stays?)\s+(?:absent|unavailable|unsupported|"
            r"not\s+(?:available|implemented|supported))\b"
            r"|\bNem\s+(?:o\s+)?desktop\s+nem\s+(?:a\s+)?CLI\s+"
            r"(?:pode\s+)?restaur(?:a|am|ar)\s+(?:quaisquer\s+)?dados\b"
            r"|\b(?:o\s+)?desktop\s+n[aã]o\s+(?:pode|consegue)\s+"
            r"restaurar\s+(?:quaisquer\s+)?dados\b"
            r"|\bn[aã]o\s+(?:é|e)\s+poss[ií]vel\s+restaurar\s+"
            r"(?:quaisquer\s+)?dados\b"
            r"|\b(?:a\s+)?restaura[cç][aã]o\s+(?:ainda\s+)?n[aã]o\s+"
            r"(?:est[aá]\s+)?(?:implementada|dispon[ií]vel|suportada)\b",
            re.IGNORECASE,
        ),
    ),
    (
        "restore progress",
        re.compile(
            r"\b(?:native\s+)?restore\s+progress\s+"
            r"(?:is|remains?|stays?)\s+(?:absent|unavailable|unsupported|"
            r"not\s+(?:available|implemented|supported))\b"
            r"|\bno\s+(?:native\s+)?restore\s+progress\s+"
            r"(?:is\s+)?(?:available|implemented|supported)\b"
            r"|\b(?:o\s+)?progresso\s+da\s+restaura[cç][aã]o\s+"
            r"(?:est[aá]\s+)?(?:ausente|indispon[ií]vel|n[aã]o\s+"
            r"(?:est[aá]\s+)?(?:dispon[ií]vel|implementado|suportado))\b"
            r"|\bn[aã]o\s+existe\s+progresso\s+da\s+restaura[cç][aã]o\b",
            re.IGNORECASE,
        ),
    ),
    (
        "restore cancellation",
        re.compile(
            r"\brestore\s+cancellation\s+"
            r"(?:is|remains?|stays?)\s+(?:absent|unavailable|unsupported|"
            r"not\s+(?:available|implemented|supported))\b"
            r"|\b(?:cannot|can['’]t)\s+cancel\s+(?:a\s+)?restore\b"
            r"|\b(?:o\s+)?cancelamento\s+da\s+restaura[cç][aã]o\s+"
            r"(?:est[aá]\s+)?(?:ausente|indispon[ií]vel|n[aã]o\s+"
            r"(?:est[aá]\s+)?(?:dispon[ií]vel|implementado|suportado))\b"
            r"|\bn[aã]o\s+(?:é|e)\s+poss[ií]vel\s+cancelar\s+"
            r"(?:a\s+)?restaura[cç][aã]o\b",
            re.IGNORECASE,
        ),
    ),
    (
        "opening the restore destination",
        re.compile(
            r"\bopening\s+the\s+(?:recovery|restore)\s+destination\s+"
            r"(?:is|remains?|stays?)\s+(?:absent|unavailable|unsupported|"
            r"not\s+(?:available|implemented|supported))\b"
            r"|\b(?:cannot|can['’]t)\s+open\s+the\s+(?:recovery|restore)\s+"
            r"destination\b"
            r"|\babrir\s+o\s+destino\s+da\s+"
            r"(?:recupera[cç][aã]o|restaura[cç][aã]o)\s+"
            r"(?:est[aá]\s+)?(?:ausente|indispon[ií]vel|n[aã]o\s+"
            r"(?:est[aá]\s+)?(?:dispon[ií]vel|implementado|suportado))\b"
            r"|\bn[aã]o\s+(?:é|e)\s+poss[ií]vel\s+abrir\s+o\s+destino\s+da\s+"
            r"(?:recupera[cç][aã]o|restaura[cç][aã]o)\b",
            re.IGNORECASE,
        ),
    ),
)


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


def command_count_value(raw_count: str) -> int:
    normalized = raw_count.casefold()
    return int(normalized) if normalized.isdigit() else NUMBER_WORDS[normalized]


def aggregated_command_count_matches(
    text: str,
) -> tuple[tuple[tuple[int, int], int], ...]:
    matches: list[tuple[tuple[int, int], int]] = []
    for pattern in AGGREGATED_COMMAND_COUNT_PATTERNS:
        for match in pattern.finditer(text):
            matches.append(
                (
                    match.span(),
                    command_count_value(match.group("first"))
                    + command_count_value(match.group("second")),
                )
            )
    return tuple(matches)


def documented_command_count_matches(
    line: str,
) -> tuple[tuple[tuple[int, int], int], ...]:
    matches: dict[tuple[int, int], int] = {}
    aggregate_matches = aggregated_command_count_matches(line)
    aggregate_spans = [span for span, _count in aggregate_matches]
    matches.update(aggregate_matches)
    for pattern in DOCUMENTED_COMMAND_COUNT_PATTERNS:
        for match in pattern.finditer(line):
            if any(
                aggregate_start <= match.start()
                and match.end() <= aggregate_end
                for aggregate_start, aggregate_end in aggregate_spans
            ):
                continue
            matches[(match.start("count"), match.end("count"))] = command_count_value(
                match.group("count")
            )
    return tuple(sorted(matches.items()))


def documented_command_counts(line: str) -> tuple[int, ...]:
    return tuple(count for _span, count in documented_command_count_matches(line))


def cross_line_aggregated_command_counts(
    line: str,
    next_line: str,
) -> tuple[int, ...]:
    if (
        not line.strip()
        or not next_line.strip()
        or line.rstrip().endswith((".", "!", "?", ":", ";"))
        or MARKDOWN_HEADING_PATTERN.match(next_line)
        or MARKDOWN_FENCE_PATTERN.match(next_line)
    ):
        return ()
    combined = f"{line} {next_line.lstrip()}"
    boundary = len(line)
    return tuple(
        count
        for (start, end), count in aggregated_command_count_matches(combined)
        if start < boundary < end
    )


def obsolete_restore_capability_row(line: str) -> bool:
    if not line.lstrip().startswith("|"):
        return False
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    return any(OBSOLETE_CAPABILITY_CELL_PATTERN.fullmatch(cell) for cell in cells) and any(
        OBSOLETE_CAPABILITY_STATUS_CELL_PATTERN.fullmatch(cell) for cell in cells
    )


def line_has_explicit_historical_context(line: str) -> bool:
    has_historical_marker = bool(
        HISTORICAL_CONTEXT_PATTERN.search(line)
        or HISTORICAL_QUALIFIED_CONTEXT_PATTERN.search(line)
    )
    return (
        has_historical_marker
        and not CURRENT_CONTEXT_PATTERN.search(line)
        and not HISTORICAL_COMPARISON_PATTERN.search(line)
    )


def explicit_historical_marker_spans(line: str) -> tuple[tuple[int, int], ...]:
    spans = {
        match.span()
        for pattern in (
            HISTORICAL_CONTEXT_PATTERN,
            HISTORICAL_QUALIFIED_CONTEXT_PATTERN,
        )
        for match in pattern.finditer(line)
    }
    return tuple(sorted(spans))


def inline_historical_count_ranges(line: str) -> tuple[tuple[int, int], ...]:
    ranges: list[tuple[int, int]] = []
    comparison = HISTORICAL_COMPARISON_PATTERN.search(line)
    for marker_start, marker_end in explicit_historical_marker_spans(line):
        clause_start = max(
            line.rfind(";", 0, marker_start),
            line.rfind(".", 0, marker_start),
            line.rfind("!", 0, marker_start),
            line.rfind("?", 0, marker_start),
        ) + 1
        if CURRENT_CONTEXT_PATTERN.search(line[clause_start:marker_start]):
            continue

        range_end = len(line)
        transition = INLINE_HISTORICAL_TRANSITION_PATTERN.search(line, marker_end)
        if transition is not None:
            range_end = transition.start()
        if comparison is not None and comparison.start() <= marker_start:
            comma = line.find(",", marker_end)
            if comma >= 0:
                range_end = min(range_end, comma)
        ranges.append((clause_start, range_end))
    return tuple(ranges)


def blank_rust_region(characters: list[str], start: int, end: int) -> None:
    for index in range(start, end):
        if characters[index] not in "\r\n":
            characters[index] = " "


def rust_char_literal_end(text: str, start: int) -> int | None:
    index = start + 1
    if index >= len(text) or text[index] in "\r\n":
        return None
    if text[index] == "\\":
        index += 1
        if index >= len(text):
            return None
        if text[index] == "u" and index + 1 < len(text) and text[index + 1] == "{":
            closing_brace = text.find("}", index + 2)
            if closing_brace < 0:
                return None
            index = closing_brace + 1
        elif text[index] == "x":
            index += 3
        else:
            index += 1
    else:
        index += 1
    if index < len(text) and text[index] == "'":
        return index + 1
    return None


def rust_code_without_comments_and_literals(text: str) -> str:
    characters = list(text)
    index = 0
    while index < len(text):
        if text.startswith("//", index):
            end = text.find("\n", index + 2)
            if end < 0:
                end = len(text)
            blank_rust_region(characters, index, end)
            index = end
            continue
        if text.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < len(text) and depth:
                if text.startswith("/*", end):
                    depth += 1
                    end += 2
                elif text.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            if depth:
                raise ValueError("unclosed Rust block comment")
            blank_rust_region(characters, index, end)
            index = end
            continue

        raw_end: int | None = None
        if index == 0 or not (text[index - 1].isalnum() or text[index - 1] == "_"):
            for prefix in ("br", "cr", "r"):
                if not text.startswith(prefix, index):
                    continue
                opening = index + len(prefix)
                hash_end = opening
                while hash_end < len(text) and text[hash_end] == "#":
                    hash_end += 1
                if hash_end >= len(text) or text[hash_end] != '"':
                    continue
                terminator = '"' + text[opening:hash_end]
                closing = text.find(terminator, hash_end + 1)
                if closing < 0:
                    raise ValueError("unclosed Rust raw string literal")
                raw_end = closing + len(terminator)
                break
        if raw_end is not None:
            blank_rust_region(characters, index, raw_end)
            index = raw_end
            continue

        if text[index] == '"':
            end = index + 1
            escaped = False
            while end < len(text):
                character = text[end]
                if character == '"' and not escaped:
                    end += 1
                    break
                if character == "\\" and not escaped:
                    escaped = True
                else:
                    escaped = False
                end += 1
            else:
                raise ValueError("unclosed Rust string literal")
            blank_rust_region(characters, index, end)
            index = end
            continue

        if text[index] == "'":
            end = rust_char_literal_end(text, index)
            if end is not None:
                blank_rust_region(characters, index, end)
                index = end
                continue
        index += 1
    return "".join(characters)


def authoritative_tauri_commands(
    root: Path,
    errors: list[str],
) -> set[str] | None:
    relative = Path(".github/scripts/validate_real_only_desktop.py")
    path = root / relative
    if not path.is_file():
        errors.append(f"{relative.as_posix()}: missing desktop command authority")
        return None

    try:
        module = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    except (OSError, SyntaxError, UnicodeError) as error:
        errors.append(
            f"{relative.as_posix()}: cannot parse desktop command authority: {error}"
        )
        return None

    assignments: list[ast.expr] = []
    for statement in module.body:
        if isinstance(statement, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == "ALLOWED_COMMANDS"
            for target in statement.targets
        ):
            assignments.append(statement.value)
        elif (
            isinstance(statement, ast.AnnAssign)
            and isinstance(statement.target, ast.Name)
            and statement.target.id == "ALLOWED_COMMANDS"
            and statement.value is not None
        ):
            assignments.append(statement.value)

    if len(assignments) != 1:
        errors.append(
            f"{relative.as_posix()}: expected exactly one literal ALLOWED_COMMANDS "
            f"assignment, found {len(assignments)}"
        )
        return None

    try:
        value = ast.literal_eval(assignments[0])
    except (ValueError, TypeError, SyntaxError) as error:
        errors.append(
            f"{relative.as_posix()}: ALLOWED_COMMANDS must be a literal set: {error}"
        )
        return None
    if (
        not isinstance(value, set)
        or not value
        or any(not isinstance(command, str) or not command for command in value)
    ):
        errors.append(
            f"{relative.as_posix()}: ALLOWED_COMMANDS must be a non-empty set "
            "of non-empty strings"
        )
        return None
    return value


def registered_tauri_commands(
    root: Path,
    errors: list[str],
) -> set[str] | None:
    relative = Path("apps/desktop/src-tauri/src/lib.rs")
    path = root / relative
    if not path.is_file():
        errors.append(f"{relative.as_posix()}: missing Tauri command registration")
        return None
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(
            f"{relative.as_posix()}: cannot read Tauri command registration: {error}"
        )
        return None

    try:
        code = rust_code_without_comments_and_literals(text)
    except ValueError as error:
        errors.append(
            f"{relative.as_posix()}: cannot lex Tauri command registration: {error}"
        )
        return None

    starts = list(re.finditer(r"\bgenerate_handler\s*!\s*\[", code))
    if len(starts) != 1:
        errors.append(
            f"{relative.as_posix()}: expected exactly one generate_handler! "
            f"registration, found {len(starts)}"
        )
        return None

    opening = code.find("[", starts[0].start(), starts[0].end())
    depth = 0
    closing: int | None = None
    for index in range(opening, len(code)):
        character = code[index]
        if character == "[":
            depth += 1
        elif character == "]":
            depth -= 1
            if depth == 0:
                closing = index
                break
    if closing is None:
        errors.append(
            f"{relative.as_posix()}: unclosed generate_handler! registration"
        )
        return None

    raw_entries = code[opening + 1 : closing].split(",")
    if raw_entries and not raw_entries[-1].strip():
        raw_entries.pop()
    if not raw_entries or any(not entry.strip() for entry in raw_entries):
        errors.append(
            f"{relative.as_posix()}: malformed generate_handler! command list"
        )
        return None

    commands: list[str] = []
    command_path = re.compile(
        r"[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+"
    )
    for entry in raw_entries:
        match = command_path.fullmatch(entry.strip())
        if match is None:
            errors.append(
                f"{relative.as_posix()}: invalid generate_handler! entry "
                f"{entry.strip()!r}"
            )
            return None
        commands.append(match.group(0))
    if len(commands) != len(set(commands)):
        errors.append(
            f"{relative.as_posix()}: duplicate command in generate_handler! registration"
        )
        return None
    return set(commands)


def validate_current_product_contract(root: Path, errors: list[str]) -> None:
    authoritative = authoritative_tauri_commands(root, errors)
    registered = registered_tauri_commands(root, errors)
    expected_names = {
        registration.rsplit("::", 1)[1]
        for registration in EXPECTED_TAURI_REGISTRATIONS
    }
    if authoritative is not None and authoritative != expected_names:
        errors.append(
            ".github/scripts/validate_real_only_desktop.py: authoritative desktop "
            "command names differ from the expected module-bound registration "
            f"inventory; missing={sorted(expected_names - authoritative)!r}; "
            f"unexpected={sorted(authoritative - expected_names)!r}"
        )
    if registered is not None:
        if registered != EXPECTED_TAURI_REGISTRATIONS:
            errors.append(
                "apps/desktop/src-tauri/src/lib.rs: registered desktop command "
                "inventory differs from authoritative inventory; "
                f"missing={sorted(EXPECTED_TAURI_REGISTRATIONS - registered)!r}; "
                f"unexpected={sorted(registered - EXPECTED_TAURI_REGISTRATIONS)!r}"
            )

    if authoritative is None:
        return
    expected_count = len(authoritative)
    for relative_text in CURRENT_PRODUCT_DOCUMENTS:
        relative = Path(relative_text)
        path = root / relative
        if not path.is_file():
            continue
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (OSError, UnicodeError) as error:
            errors.append(
                f"{relative.as_posix()}: cannot inspect current product claims: {error}"
            )
            continue
        historical_heading_level: int | None = None
        fence_marker: str | None = None
        historical_fence = False
        for line_number, line in enumerate(lines, start=1):
            fence = MARKDOWN_FENCE_PATTERN.match(line)
            if fence is not None:
                marker = fence.group("marker")
                if fence_marker is None:
                    fence_marker = marker
                    historical_fence = line_has_explicit_historical_context(line)
                elif marker[0] == fence_marker[0] and len(marker) >= len(fence_marker):
                    fence_marker = None
                    historical_fence = False
                continue

            heading = MARKDOWN_HEADING_PATTERN.match(line)
            if heading is not None:
                heading_level = len(heading.group("marks"))
                if (
                    historical_heading_level is not None
                    and heading_level <= historical_heading_level
                ):
                    historical_heading_level = None
                if line_has_explicit_historical_context(heading.group("title")):
                    historical_heading_level = heading_level

            if fence_marker is not None and line_has_explicit_historical_context(line):
                historical_fence = True
            if historical_heading_level is not None or historical_fence:
                continue

            historical_count_ranges = inline_historical_count_ranges(line)
            documented_counts = [
                count
                for (count_start, count_end), count in documented_command_count_matches(
                    line
                )
                if not any(
                    range_start <= count_start and count_end <= range_end
                    for range_start, range_end in historical_count_ranges
                )
            ]
            if line_number < len(lines):
                documented_counts.extend(
                    cross_line_aggregated_command_counts(
                        line,
                        lines[line_number],
                    )
                )
            for documented_count in documented_counts:
                if documented_count != expected_count:
                    errors.append(
                        f"{relative.as_posix()}:{line_number}: documents "
                        f"{documented_count} desktop/Tauri commands, but authoritative "
                        f"inventory has {expected_count}"
                    )
            obsolete_capabilities = {
                capability
                for capability, pattern in OBSOLETE_CURRENT_STATUS_PATTERNS
                if pattern.search(line)
            }
            if obsolete_restore_capability_row(line):
                obsolete_capabilities.add("restore execution")
            for capability in sorted(obsolete_capabilities):
                errors.append(
                    f"{relative.as_posix()}:{line_number}: obsolete current-status "
                    f"claim about {capability}"
                )


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
        if not test_id.startswith(
            ("JUST-", "REAL-AC-", "SDD-", "NFR-", "FR-")
        )
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
    sdd018_statuses: dict[str, str],
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
    windows_header, windows_rows = parse_matrix_section(
        text, "Connected mounted-volume desktop increment"
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
        (windows_header, "connected-volume"),
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
    validate_rows(
        windows_rows,
        EXPECTED_SDD018_IDS,
        sdd018_statuses,
        "connected-volume",
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
            and not test_id.startswith(
                ("JUST-", "REAL-AC-", "SDD-", "NFR-", "FR-")
            )
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
        "docs/specs/018-windows-volume-and-folder-scan.md",
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
    validate_current_product_contract(root, errors)

    master = root / "UNDELETE_MASTER_CODEX_MASTER_SPEC.md"
    if not master.is_file():
        errors.append("missing master specification")
        return errors, {}
    master_text = master.read_text(encoding="utf-8")
    master_ids = set(re.findall(r"^### (FR-\d{3})\b", master_text, re.MULTILINE))

    validate_links_and_placeholders(root, errors)
    catalog = validate_functional_catalog(root, master_ids, errors)
    valid_requirements = (
        master_ids
        | EXPECTED_NFR_IDS
        | EXPECTED_SDD016_IDS
        | EXPECTED_SDD017_IDS
        | EXPECTED_SDD018_IDS
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
    sdd018_statuses = validate_requirement_sections(
        root,
        "docs/specs/018-windows-volume-and-folder-scan.md",
        "###",
        r"SDD-WIN-\d{3}",
        EXPECTED_SDD018_IDS,
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
    validate_sdd_fields_and_paths(
        root,
        "docs/specs/018-windows-volume-and-folder-scan.md",
        r"SDD-WIN-\d{3}",
        errors,
    )
    validate_matrix(
        root,
        master_ids,
        catalog,
        nfr_statuses,
        sdd016_statuses,
        sdd017_statuses,
        sdd018_statuses,
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
        "sdd018_requirements": len(sdd018_statuses),
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
        f"{stats['sdd017_requirements']} real-only and "
        f"{stats['sdd018_requirements']} connected-volume requirements, "
        f"{stats['adr_topics']} required ADR topics, "
        f"{stats['justifications']} formal justifications, "
        "statuses/test references/local paths checked"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
