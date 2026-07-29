#!/usr/bin/env python3
"""Static, fail-closed guard for destructive storage behavior in PR CI.

This guard is defense in depth. It reviews every workflow plus enumerated
first-party code, command-manifest, shebang-script, and explicitly invoked local
helper surfaces, including Rust files that contain inline ``#[cfg(test)]``
modules. It does not claim to prove runtime safety or detect deliberately
obfuscated commands.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

SKIPPED_DIRECTORIES = {
    ".git",
    "coverage",
    "dist",
    "node_modules",
    "target",
    "venv",
    ".venv",
}

WEB_SUFFIXES = {".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"}
SCRIPT_SUFFIXES = {".bat", ".cmd", ".ps1", ".psm1", ".py", ".sh"}


def joined(*parts: str) -> str:
    """Keep prohibited command literals out of this validator's own source."""

    return "".join(parts)


@dataclass(frozen=True)
class Rule:
    id: str
    pattern: str
    message: str


DESTRUCTIVE_COMMAND_RULES = (
    Rule(
        "storage-shell",
        joined(
            r"\b(?:disk",
            r"part|Clear",
            r"-Disk|Initialize",
            r"-Disk|Remove",
            r"-Partition|Format",
            r"-Volume|format",
            r"\.com|mk",
            r"fs(?:\.[a-z0-9]+)?|wipe",
            r"fs|blk",
            r"discard)\b",
        ),
        "destructive storage tooling is forbidden in PR execution paths",
    ),
    Rule(
        "raw-copy",
        joined(r"\bdd\b[^\r\n]{0,240}\bof\s*=\s*(?:/dev/|\\\\[.?]\\)"),
        "raw-device copy output is forbidden in PR execution paths",
    ),
    Rule(
        "storage-erase",
        joined(
            r"\b(?:shr",
            r"ed|hd",
            r"parm)\b|\b(?:fs",
            r"trim|trim)\s+(?:/dev/|\\\\[.?]\\)",
        ),
        "erase or trim tooling is forbidden in PR execution paths",
    ),
    Rule(
        "vhd-control",
        joined(r"\b(?:New", r"-VHD|Mount", r"-VHD|Dismount", r"-VHD)\b"),
        "VHD device attachment is forbidden in pull-request CI",
    ),
)

RAW_DEVICE_RULES = (
    Rule(
        "windows-raw-device",
        joined(
            r"(?:\\\\[.?]\\(?:Physical",
            r"Drive\d+|GLOBALROOT\\Device\\Harddisk[^\\\s]*|[A-Za-z]:))",
        ),
        "a Windows raw disk or volume path is forbidden in PR execution paths",
    ),
    Rule(
        "unix-raw-device",
        r"/dev/(?:sd[a-z]|hd[a-z]|vd[a-z]|xvd[a-z]|nvme\d+n\d+|mmcblk\d+)(?:p\d+)?\b",
        "a Unix raw block-device path is forbidden in PR execution paths",
    ),
)

WINDOWS_WRITE_RULES = (
    Rule(
        "windows-write-access",
        joined(
            r"\b(?:GENERIC_",
            r"WRITE|FSCTL_(?:LOCK|DISMOUNT)_VOLUME|FSCTL_SET_",
            r"ZERO_DATA|IOCTL_STORAGE_MANAGE_DATA_SET_",
            r"ATTRIBUTES)\b",
        ),
        "write, lock, dismount, zero, or trim device control is forbidden",
    ),
)

ELEVATION_RULES = (
    Rule(
        "elevation",
        joined(r"\bStart-Process\b[^\r\n]{0,240}\b-Verb\s+Run", r"As\b|\bsudo\b"),
        "elevation is forbidden in pull-request quality jobs",
    ),
    Rule(
        joined("self", "-hosted"),
        joined(r"\bself", r"-hosted\b"),
        "pull-request quality jobs must use isolated managed runners",
    ),
)

WORKFLOW_RULES = (
    *DESTRUCTIVE_COMMAND_RULES,
    *RAW_DEVICE_RULES,
    *WINDOWS_WRITE_RULES,
    *ELEVATION_RULES,
)
RUST_RULES = (*DESTRUCTIVE_COMMAND_RULES, *RAW_DEVICE_RULES, *WINDOWS_WRITE_RULES)
SCRIPT_RULES = (
    *DESTRUCTIVE_COMMAND_RULES,
    *RAW_DEVICE_RULES,
    *WINDOWS_WRITE_RULES,
    *ELEVATION_RULES,
)
# The production desktop is real-image-only. Raw-device selectors have no
# legitimate frontend use; even inert examples belong in reviewed documentation
# or tests outside the production source boundary.
WEB_RULES = (
    *DESTRUCTIVE_COMMAND_RULES,
    *RAW_DEVICE_RULES,
    *WINDOWS_WRITE_RULES,
    *ELEVATION_RULES,
)

# This file contains negative tests proving that raw device spellings are
# rejected before any open. The reviewed hash makes the exception fail closed:
# any source change requires explicit re-review of this single rule exception.
RULE_HASH_ALLOWLIST = {
    (
        "crates/cli/src/lib.rs",
        "windows-raw-device",
    ): "44d7ed6c177e9aec18e4d0da1f1a7aaa18b51831c717f77b4f82b361016c1e0a",
}


def relative_name(root: Path, path: Path) -> str:
    return path.relative_to(root).as_posix()


def read_text_fail_closed(path: Path, root: Path, errors: list[str]) -> str | None:
    relative = relative_name(root, path)
    if path.is_symlink():
        errors.append(f"{relative}: executable/workflow symlinks are not permitted")
        return None
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(f"{relative}: cannot inspect file as UTF-8: {error}")
        return None


def hash_is_allowlisted(root: Path, path: Path, rule: Rule) -> bool:
    relative = relative_name(root, path)
    expected = RULE_HASH_ALLOWLIST.get((relative, rule.id))
    if expected is None:
        return False
    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    return actual == expected


def scan(
    root: Path,
    path: Path,
    rules: tuple[Rule, ...],
    errors: list[str],
    *,
    strip_shell_comments: bool = False,
) -> None:
    text = read_text_fail_closed(path, root, errors)
    if text is None:
        return
    if strip_shell_comments:
        text = without_unquoted_shell_comments(
            without_inert_multiline_shell_text(text)
        )
    for rule in rules:
        if re.search(rule.pattern, text, re.IGNORECASE | re.MULTILINE):
            if hash_is_allowlisted(root, path, rule):
                continue
            errors.append(f"{relative_name(root, path)} [{rule.id}]: {rule.message}")


def beneath_skipped_directory(path: Path, root: Path) -> bool:
    return any(part in SKIPPED_DIRECTORIES for part in path.relative_to(root).parts)


def is_extensionless_shebang_script(path: Path) -> bool:
    if path.suffix:
        return False
    if path.is_symlink():
        # Selection is intentionally fail closed; read_text_fail_closed reports
        # the symlink rather than following it.
        return True
    try:
        with path.open("rb") as handle:
            return handle.read(2) == b"#!"
    except OSError:
        # Let the normal scanner produce the actionable inspection error.
        return True


def workflow_files(root: Path) -> list[Path]:
    workflow_root = root / ".github" / "workflows"
    if not workflow_root.is_dir():
        return []
    return sorted(
        path
        for path in workflow_root.rglob("*")
        if path.is_file() and path.suffix.lower() in {".yaml", ".yml"}
    )


LOCAL_INVOCATION_PATTERNS = (
    re.compile(
        r"""(?ix)
        (?:^|[\r\n]|&&|\|\||[;&|])\s*
        (?:pwsh|powershell(?:\.exe)?)\b
        [^\r\n;&|]{0,240}?
        (?:-File|-f)\s+
        (?P<path>"[^"\r\n]+"|'[^'\r\n]+'|[^\s;&|]+)
        """
    ),
    re.compile(
        r"""(?ix)
        (?:^|[\r\n]|&&|\|\||[;&|])\s*
        (?:python(?:3(?:\.\d+)?)?|node|bash|sh)\b
        (?:\s+-[^\s;&|]+){0,6}\s+
        (?P<path>
            "[^"\r\n]+"
            |'[^'\r\n]+'
            |(?:\.{1,2}[\\/]|[A-Za-z0-9_.-]+[\\/])[^\s;&|]+
        )
        """
    ),
    re.compile(
        r"""(?ix)
        (?:^|[\r\n]|&&|\|\||[;&|])\s*
        (?P<path>\.{1,2}[\\/][^\s;&|]+)
        """
    ),
)


def command_texts_from_manifest(
    root: Path,
    manifest: Path,
    errors: list[str],
) -> list[str]:
    try:
        payload = json.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        errors.append(
            f"{relative_name(root, manifest)}: cannot inspect package scripts: {error}"
        )
        return []
    scripts = payload.get("scripts", {}) if isinstance(payload, dict) else {}
    if not isinstance(scripts, dict):
        errors.append(
            f"{relative_name(root, manifest)}: package scripts must be an object"
        )
        return []
    return [command for command in scripts.values() if isinstance(command, str)]


def without_unquoted_shell_comments(command: str) -> str:
    cleaned: list[str] = []
    for line in command.splitlines():
        quote: str | None = None
        escaped = False
        kept: list[str] = []
        for character in line:
            if escaped:
                kept.append(character)
                escaped = False
                continue
            if character == "\\" and quote == '"':
                kept.append(character)
                escaped = True
                continue
            if quote is not None:
                kept.append(character)
                if character == quote:
                    quote = None
                continue
            if character in {"'", '"'}:
                quote = character
                kept.append(character)
                continue
            if character == "#" and (not kept or kept[-1].isspace()):
                break
            kept.append(character)
        cleaned.append("".join(kept))
    return "\n".join(cleaned)


def unwrap_quoted_yaml_scalar(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
        return value[1:-1]
    return value


def indentation_width(line: str) -> int:
    prefix = line[: len(line) - len(line.lstrip(" \t"))]
    return len(prefix.expandtabs(8))


def workflow_step_working_directory(
    lines: list[str],
    run_index: int,
    run_indent: int,
) -> str | None:
    step_start: int | None = None
    step_indent: int | None = None
    for index in range(run_index, -1, -1):
        match = re.match(r"^(?P<indent>[ \t]*)-\s+", lines[index])
        if match is None:
            continue
        indentation = len(match.group("indent").expandtabs(8))
        if indentation <= run_indent:
            step_start = index
            step_indent = indentation
            break
    if step_start is None or step_indent is None:
        return None

    step_end = len(lines)
    for index in range(step_start + 1, len(lines)):
        if not lines[index].strip():
            continue
        indentation = indentation_width(lines[index])
        if indentation < step_indent:
            step_end = index
            break
        if (
            indentation == step_indent
            and re.match(r"^[ \t]*-\s+", lines[index])
        ):
            step_end = index
            break

    working_directory = re.compile(
        r"^[ \t]*(?:-\s+)?working-directory:\s*(?P<value>.+?)\s*$",
        re.IGNORECASE,
    )
    for line in lines[step_start:step_end]:
        match = working_directory.match(line)
        if match is not None:
            return unwrap_quoted_yaml_scalar(match.group("value").strip())
    return None


def yaml_block_end(lines: list[str], start: int, indentation: int) -> int:
    for index in range(start + 1, len(lines)):
        stripped = lines[index].strip()
        if not stripped or stripped.startswith("#"):
            continue
        if indentation_width(lines[index]) <= indentation:
            return index
    return len(lines)


def workflow_default_working_directory(
    lines: list[str],
    run_index: int,
) -> str | None:
    candidates: list[tuple[int, str]] = []
    defaults_pattern = re.compile(
        r"^(?P<indent>[ \t]*)defaults:\s*(?:#.*)?$",
        re.IGNORECASE,
    )
    run_pattern = re.compile(
        r"^(?P<indent>[ \t]*)run:\s*(?:#.*)?$",
        re.IGNORECASE,
    )
    working_pattern = re.compile(
        r"^[ \t]*working-directory:\s*(?P<value>.+?)\s*$",
        re.IGNORECASE,
    )
    parent_pattern = re.compile(r"^[ \t]*[A-Za-z0-9_.-]+:\s*(?:#.*)?$")

    for defaults_index, line in enumerate(lines):
        defaults_match = defaults_pattern.match(line)
        if defaults_match is None:
            continue
        defaults_indent = len(defaults_match.group("indent").expandtabs(8))
        defaults_end = yaml_block_end(lines, defaults_index, defaults_indent)

        value: str | None = None
        for index in range(defaults_index + 1, defaults_end):
            run_match = run_pattern.match(lines[index])
            if run_match is None:
                continue
            run_indent = len(run_match.group("indent").expandtabs(8))
            if run_indent <= defaults_indent:
                continue
            run_end = yaml_block_end(lines, index, run_indent)
            for candidate in lines[index + 1:run_end]:
                working_match = working_pattern.match(candidate)
                if working_match is not None:
                    cleaned = without_unquoted_shell_comments(
                        working_match.group("value")
                    ).strip()
                    value = unwrap_quoted_yaml_scalar(cleaned)
                    break
            if value is not None:
                break
        if value is None:
            continue

        if defaults_indent == 0:
            scope_start, scope_end = 0, len(lines)
        else:
            scope_start = 0
            scope_indent = -1
            for index in range(defaults_index - 1, -1, -1):
                if not parent_pattern.match(lines[index]):
                    continue
                indentation = indentation_width(lines[index])
                if indentation < defaults_indent:
                    scope_start = index
                    scope_indent = indentation
                    break
            scope_end = (
                yaml_block_end(lines, scope_start, scope_indent)
                if scope_indent >= 0
                else len(lines)
            )

        if scope_start <= run_index < scope_end:
            candidates.append((defaults_indent, value))

    if not candidates:
        return None
    return max(candidates, key=lambda candidate: candidate[0])[1]


def workflow_run_commands(text: str) -> list[tuple[str, str | None]]:
    lines = text.splitlines()
    commands: list[tuple[str, str | None]] = []
    index = 0
    run_line = re.compile(
        r"^(?P<indent>[ \t]*)(?:-\s+)?run:\s*(?P<value>.*)$",
        re.IGNORECASE,
    )
    while index < len(lines):
        match = run_line.match(lines[index])
        if match is None:
            index += 1
            continue
        run_index = index
        value = without_unquoted_shell_comments(match.group("value")).strip()
        base_indent = len(match.group("indent").expandtabs(8))
        working_directory = workflow_step_working_directory(
            lines,
            run_index,
            base_indent,
        )
        if working_directory is None:
            working_directory = workflow_default_working_directory(
                lines,
                run_index,
            )
        indicator = value.split(maxsplit=1)[0] if value else ""
        if indicator in {"|", ">", "|-", ">-", "|+", ">+"}:
            block: list[str] = []
            index += 1
            while index < len(lines):
                candidate = lines[index]
                if candidate.strip():
                    indentation = len(candidate) - len(candidate.lstrip(" \t"))
                    if indentation <= base_indent:
                        break
                block.append(candidate)
                index += 1
            block_text = (
                " ".join(line.strip() for line in block)
                if indicator.startswith(">")
                else "\n".join(block)
            )
            commands.append(
                (without_unquoted_shell_comments(block_text), working_directory)
            )
            continue
        commands.append(
            (
                unwrap_quoted_yaml_scalar(value),
                working_directory,
            )
        )
        index += 1
    return commands


def without_inert_multiline_shell_text(command: str) -> str:
    output: list[str] = []
    persistent_quote: str | None = None
    here_terminator: str | None = None
    block_depth = 0

    for line in command.splitlines():
        masked = list(line)
        index = 0
        if here_terminator is not None:
            if re.match(
                rf"^[ \t]*{re.escape(here_terminator)}(?:\s*;)?\s*$",
                line,
            ):
                here_terminator = None
            output.append("".join(" " for _ in line))
            continue

        while index < len(line):
            following = line[index + 1] if index + 1 < len(line) else ""

            if block_depth:
                if line[index] == "<" and following == "#":
                    masked[index:index + 2] = [" ", " "]
                    block_depth += 1
                    index += 2
                    continue
                if line[index] == "#" and following == ">":
                    masked[index:index + 2] = [" ", " "]
                    block_depth -= 1
                    index += 2
                    continue
                masked[index] = " "
                index += 1
                continue

            if persistent_quote is not None:
                masked[index] = " "
                if line[index] == "`":
                    if index + 1 < len(line):
                        masked[index + 1] = " "
                        index += 2
                        continue
                if line[index] == persistent_quote:
                    if (
                        persistent_quote == "'"
                        and index + 1 < len(line)
                        and line[index + 1] == "'"
                    ):
                        masked[index + 1] = " "
                        index += 2
                        continue
                    persistent_quote = None
                index += 1
                continue

            if line[index] == "<" and following == "#":
                masked[index:index + 2] = [" ", " "]
                block_depth = 1
                index += 2
                continue

            opener = re.match(r"@(?P<quote>['\"])\s*$", line[index:])
            if opener is not None:
                for position in range(index, len(line)):
                    masked[position] = " "
                here_terminator = opener.group("quote") + "@"
                break

            if line[index] in {"'", '"'}:
                quote = line[index]
                cursor = index + 1
                escaped = False
                close: int | None = None
                while cursor < len(line):
                    if escaped:
                        escaped = False
                    elif line[cursor] == "`":
                        escaped = True
                    elif line[cursor] == quote:
                        if (
                            quote == "'"
                            and cursor + 1 < len(line)
                            and line[cursor + 1] == "'"
                        ):
                            cursor += 1
                        else:
                            close = cursor
                            break
                    cursor += 1
                if close is None:
                    for position in range(index, len(line)):
                        masked[position] = " "
                    persistent_quote = quote
                    break
                index = close + 1
                continue

            index += 1

        output.append("".join(masked))
    return "\n".join(output)


def invoked_path_tokens(command: str) -> set[str]:
    executable_text = without_unquoted_shell_comments(
        without_inert_multiline_shell_text(command)
    )
    return {
        match.group("path")
        for pattern in LOCAL_INVOCATION_PATTERNS
        for match in pattern.finditer(executable_text)
    }


def resolve_invoked_path(
    root: Path,
    base: Path,
    source: Path,
    token: str,
    errors: list[str],
) -> Path | None:
    value = token.strip().strip("\"'")
    source_name = relative_name(root, source)
    if not value:
        return None
    if any(marker in value for marker in ("$", "%", "{", "}", "`")):
        errors.append(
            f"{source_name}: dynamic invoked helper path cannot be inspected: {value}"
        )
        return None
    normalized = value.replace("\\", "/")
    if normalized.startswith("/") or re.match(r"^[A-Za-z]:/", normalized):
        errors.append(
            f"{source_name}: invoked helper escapes repository inspection boundary: "
            f"{value}"
        )
        return None

    root_resolved = root.resolve()
    candidate = (base / normalized).resolve()
    try:
        candidate.relative_to(root_resolved)
    except ValueError:
        errors.append(
            f"{source_name}: invoked helper escapes repository inspection boundary: "
            f"{value}"
        )
        return None

    if not candidate.is_file() and not candidate.is_symlink():
        errors.append(
            f"{source_name}: invoked local helper is not a repository file: {value}"
        )
        return None
    return candidate


def resolve_working_directory(
    root: Path,
    source: Path,
    value: str | None,
    errors: list[str],
) -> Path | None:
    if value is None:
        return root
    source_name = relative_name(root, source)
    if any(marker in value for marker in ("$", "%", "{", "}", "`")):
        errors.append(
            f"{source_name}: dynamic workflow working-directory cannot be inspected: "
            f"{value}"
        )
        return None
    normalized = value.replace("\\", "/")
    if normalized.startswith("/") or re.match(r"^[A-Za-z]:/", normalized):
        errors.append(
            f"{source_name}: workflow working-directory escapes repository "
            f"inspection boundary: {value}"
        )
        return None
    candidate = (root / normalized).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError:
        errors.append(
            f"{source_name}: workflow working-directory escapes repository "
            f"inspection boundary: {value}"
        )
        return None
    if not candidate.is_dir():
        errors.append(
            f"{source_name}: workflow working-directory is not a repository "
            f"directory: {value}"
        )
        return None
    return candidate


def invoked_local_surfaces(
    root: Path,
    workflows: list[Path],
    actions: list[Path],
    manifests: list[Path],
    errors: list[str],
) -> set[Path]:
    invoked: set[Path] = set()
    for workflow in (*workflows, *actions):
        try:
            commands = workflow_run_commands(workflow.read_text(encoding="utf-8"))
        except (OSError, UnicodeError):
            # The normal workflow scan already reports the inspection failure.
            continue
        for command, working_directory in commands:
            base = resolve_working_directory(
                root,
                workflow,
                working_directory,
                errors,
            )
            if base is None:
                continue
            for token in invoked_path_tokens(command):
                path = resolve_invoked_path(root, base, workflow, token, errors)
                if path is not None:
                    invoked.add(path)

    for manifest in manifests:
        for command in command_texts_from_manifest(root, manifest, errors):
            for token in invoked_path_tokens(command):
                path = resolve_invoked_path(
                    root,
                    manifest.parent,
                    manifest,
                    token,
                    errors,
                )
                if path is not None:
                    invoked.add(path)
    return invoked


def executable_surfaces(root: Path) -> list[tuple[Path, tuple[Rule, ...]]]:
    surfaces: dict[Path, tuple[Rule, ...]] = {}
    for path in root.rglob("*"):
        if not path.is_file() or beneath_skipped_directory(path, root):
            continue
        suffix = path.suffix.lower()
        relative_parts = path.relative_to(root).parts
        if suffix == ".rs":
            surfaces[path] = RUST_RULES
        elif path.name == "package.json":
            # npm/pnpm execute scripts and lifecycle hooks from this manifest.
            # Scanning the whole first-party file is deliberately conservative.
            surfaces[path] = SCRIPT_RULES
        elif suffix in SCRIPT_SUFFIXES:
            surfaces[path] = SCRIPT_RULES
        elif is_extensionless_shebang_script(path):
            surfaces[path] = SCRIPT_RULES
        elif suffix in WEB_SUFFIXES:
            surfaces[path] = WEB_RULES
        elif (
            suffix in {".yaml", ".yml"}
            and len(relative_parts) >= 3
            and relative_parts[:2] == (".github", "actions")
        ):
            surfaces[path] = WORKFLOW_RULES

    return sorted(surfaces.items(), key=lambda item: item[0].as_posix())


def validate_repository(root: Path) -> tuple[list[str], int, int]:
    errors: list[str] = []
    workflows = workflow_files(root)
    if not workflows:
        errors.append(".github/workflows: no YAML workflow found; safety review fails closed")
    for path in workflows:
        scan(
            root,
            path,
            WORKFLOW_RULES,
            errors,
            strip_shell_comments=True,
        )

    surfaces_by_path = dict(executable_surfaces(root))
    manifests = [path for path in surfaces_by_path if path.name == "package.json"]
    actions = [
        path
        for path in surfaces_by_path
        if path.suffix.lower() in {".yaml", ".yml"}
        and path.relative_to(root).parts[:2] == (".github", "actions")
    ]
    for path in invoked_local_surfaces(
        root,
        workflows,
        actions,
        manifests,
        errors,
    ):
        surfaces_by_path[path] = SCRIPT_RULES
    surfaces = sorted(surfaces_by_path.items(), key=lambda item: item[0].as_posix())
    if not surfaces:
        errors.append("repository: no first-party executable surface found")
    for path, rules in surfaces:
        scan(root, path, rules, errors)

    return errors, len(workflows), len(surfaces)


def main() -> int:
    errors, workflow_count, surface_count = validate_repository(ROOT)
    if errors:
        print("CI safety static guard failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print(
        "CI safety static guard passed: "
        f"reviewed {workflow_count} workflow(s) and "
        f"{surface_count} enumerated first-party code/command surface(s). "
        "This is defense in depth, not runtime proof or an obfuscation detector."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
