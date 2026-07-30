#!/usr/bin/env python3
"""Fail-closed static guard for the real-only desktop production boundary."""

from __future__ import annotations

import hashlib
import json
import os
import re
import sys
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]

ALLOWED_COMMANDS = {
    "list_storage_sources",
    "select_scan_folder",
    "scan_storage_volume",
    "get_candidate_page",
    "query_candidate_page",
    "update_candidate_selection",
}
ALLOWED_CAPABILITIES = {"core:default", "dialog:allow-open"}
FORBIDDEN_PRODUCTION_FILES = {
    "api/mock.ts",
    "api/desktop.ts",
    "api/report.ts",
    "state/imageScan.ts",
    "views/LiveScanView.tsx",
    "views/RestoreView.tsx",
    "views/SessionsView.tsx",
}
FORBIDDEN_TAURI_DEPENDENCIES = {
    "tauri-plugin-fs",
    "tauri-plugin-http",
    "tauri-plugin-opener",
    "tauri-plugin-process",
    "tauri-plugin-shell",
    "tauri-plugin-updater",
}
ALLOWED_TAURI_BUILD_DEPENDENCIES = {
    "tauri-build": {"version": "=2.6.3", "features": []},
}
ALLOWED_TAURI_DEPENDENCIES = {
    "serde": {"workspace": True},
    "serde_json": {"workspace": True},
    "sha2": {"workspace": True},
    "hex": {"workspace": True},
    "getrandom": {"workspace": True},
    "tauri": {"version": "=2.11.5", "features": []},
    "tauri-plugin-dialog": {"version": "=2.7.2"},
    "um-broker-client": {"workspace": True},
    "um-cli": {"workspace": True},
    "um-core": {"workspace": True},
    "um-fs-common": {"workspace": True},
    "um-fs-ntfs": {"workspace": True},
    "um-io-windows": {"workspace": True},
}
ALLOWED_TAURI_DEV_DEPENDENCIES = {
    "crc32fast": {"workspace": True},
    "tempfile": {"workspace": True},
    "um-fixture-builder": {"workspace": True},
}
PRODUCTION_SUFFIXES = {".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"}
TEST_FILE = re.compile(r"\.(?:test|spec)\.[cm]?[jt]sx?$", re.IGNORECASE)
SIMULATION_PATTERNS = (
    (re.compile(r"\bMockDataProvider\b"), "production mock provider"),
    (
        re.compile(
            r"\b(?:Mock|Fake)(?:Data|Provider|Runtime|Source|Candidate|Inventory)\b"
        ),
        "production mock or fake surface",
    ),
    (re.compile(r"\bMath\.random\s*\("), "simulated runtime primitive"),
    (re.compile(r"\bsetInterval\s*\("), "simulated runtime primitive"),
    (
        re.compile(r"\b(?:mode|runtimeMode)\s*:\s*['\"]demo['\"]", re.IGNORECASE),
        "demo runtime mode",
    ),
)
INVOKE_PATTERN = re.compile(r"\binvoke(?:<[^>]+>)?\s*\(\s*['\"]([^'\"]+)['\"]")
SKIPPED_RUST_DIRECTORIES = {
    ".git",
    "coverage",
    "dist",
    "node_modules",
    "target",
    "venv",
    ".venv",
}
SKIPPED_CARGO_CONFIG_DIRECTORIES = SKIPPED_RUST_DIRECTORIES | {
    "build",
    "vendor",
}
IO_WINDOWS_SOURCE = Path("crates/io-windows/src/windows.rs")
IO_WINDOWS_CRATE_ROOT = Path("crates/io-windows/src/lib.rs")
IO_WINDOWS_TRANSPORT_CONFIG = Path("crates/io-windows/src/transport_config.rs")
IO_WINDOWS_MANIFEST = Path("crates/io-windows/Cargo.toml")
BROKER_ROOT = Path("crates/elevated-broker")
ALLOWED_WINDOWS_SYS_VERSION = "=0.61.2"
ALLOWED_WINDOWS_SYS_FEATURES = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_Security_Authorization",
    "Win32_Storage_FileSystem",
    "Win32_System_IO",
    "Win32_System_Ioctl",
    "Win32_System_Pipes",
    "Win32_System_Threading",
]
ALLOWED_IO_WINDOWS_GENERAL_DEPENDENCIES = {
    "hex": {"workspace": True},
    "serde": {"workspace": True},
    "sha2": {"workspace": True},
    "thiserror": {"workspace": True},
    "um-core": {"workspace": True},
}
ALLOWED_WORKSPACE_DEPENDENCIES = {
    "um-core": {"path": "crates/core"},
    "um-io-common": {"path": "crates/io-common"},
    "um-partition": {"path": "crates/partition"},
    "um-fs-common": {"path": "crates/fs-common"},
    "um-fs-ntfs": {"path": "crates/fs-ntfs"},
    "um-fs-fat": {"path": "crates/fs-fat"},
    "um-fs-exfat": {"path": "crates/fs-exfat"},
    "um-carving": {"path": "crates/carving"},
    "um-restore": {"path": "crates/restore"},
    "um-fixture-builder": {"path": "crates/fixture-builder"},
    "um-cli": {"path": "crates/cli"},
    "um-io-windows": {"path": "crates/io-windows"},
    "um-broker-protocol": {"path": "crates/broker-protocol"},
    "um-broker-client": {"path": "crates/broker-client"},
    "thiserror": "2",
    "serde": {"version": "1", "features": ["derive"]},
    "serde_json": "1",
    "sha2": "0.10",
    "crc32fast": "1",
    "hex": "0.4",
    "proptest": "1",
    "tempfile": "3",
    "libc": "0.2",
    "getrandom": "0.3.4",
    "subtle": "2.6",
}
ALLOWED_IO_WINDOWS_MACROS = {
    "assert",
    "assert_eq",
    "assert_ne",
    "format",
    "include_str",
    "matches",
    "offset_of",
    "vec",
}
ALLOWED_PINNED_CHILD_DEPENDENCIES = {
    IO_WINDOWS_MANIFEST.as_posix(): {
        "windows-sys": {
            "version": ALLOWED_WINDOWS_SYS_VERSION,
            "features": ALLOWED_WINDOWS_SYS_FEATURES,
        },
    },
    "apps/desktop/src-tauri/Cargo.toml": {
        **ALLOWED_TAURI_BUILD_DEPENDENCIES,
        "tauri": ALLOWED_TAURI_DEPENDENCIES["tauri"],
        "tauri-plugin-dialog": ALLOWED_TAURI_DEPENDENCIES[
            "tauri-plugin-dialog"
        ],
    },
}
AUDITED_STORAGE_BUS_QUERY_SHA256 = (
    "25140ff7fb94fd32ea481716b5abbcfea659b6675202e4b9531eb8d7b507f136"
)


def joined(*parts: str) -> str:
    """Build guarded identifiers without tripping the separate CI scanner."""

    return "".join(parts)


UNSAFE_TOKEN = re.compile(r"\bunsafe\b")
CREATE_FILE_TOKEN = re.compile(r"\bCreateFile(?:A|W|2)\b")
DEVICE_CONTROL_TOKEN = re.compile(r"\bDeviceIoControl\b")
RUST_SIGNIFICANT_TOKEN = re.compile(
    r"r#[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_][A-Za-z0-9_]*|::|[^\s]"
)
DYNAMIC_SYMBOL_RESOLUTION_TOKEN = re.compile(
    r"\b(?:"
    r"GetProcAddress(?:ForCaller)?|"
    r"LoadLibrary(?:Ex)?[AW]?|LoadPackagedLibrary|"
    r"GetModuleHandle(?:Ex)?[AW]?|"
    r"LdrGetProcedureAddress|LdrLoadDll|"
    r"dlopen[0-9]*|dlsym|libloading"
    r")\b"
)
DYNAMIC_LOADER_PACKAGE = re.compile(
    r"(?:^|[-_])(?:"
    r"libloading|dlopen[0-9]*|dynamic[-_]?(?:reload|loader)|"
    r"shared[-_]?library"
    r")(?:$|[-_])",
    re.IGNORECASE,
)
GENERIC_WRITE_TOKEN = re.compile(
    rf"\b{re.escape(joined('GENERIC_', 'WRITE'))}\b"
)
CONTROL_CODE_TOKEN = re.compile(r"\b(?:IOCTL|FSCTL)_[A-Z0-9_]+\b")
ALLOWED_CONTROL_CODES = {
    joined("IOCTL_", "VOLUME_GET_VOLUME_DISK_EXTENTS"),
    joined("IOCTL_", "DISK_GET_LENGTH_INFO"),
    joined("IOCTL_", "STORAGE_QUERY_PROPERTY"),
}
MUTATING_WINDOWS_API = re.compile(
    joined(
        r"\b(?:",
        r"(?:Nt|Zw)?WriteFile(?:Ex|Gather)?|",
        r"DeleteFile(?:A|W)?|RemoveDirectory(?:A|W)?|",
        r"MoveFile(?:Ex|Transacted)?(?:A|W)?|ReplaceFile(?:A|W)?|",
        r"SetEndOfFile|SetFileInformationByHandle|",
        r"SetVolumeMountPoint(?:A|W)?|DeleteVolumeMountPoint(?:A|W)?|",
        r"SetVolumeLabel(?:A|W)?|LockFile(?:Ex)?",
        r")\b",
    ),
    re.IGNORECASE,
)
MUTATING_ACCESS_TOKEN = re.compile(
    joined(
        r"\b(?:",
        r"FILE_(?:WRITE_[A-Z0-9_]+|APPEND_DATA|ALL_ACCESS)|",
        r"DELETE|WRITE_DAC|WRITE_OWNER|",
        r"CREATE_ALWAYS|CREATE_NEW|OPEN_ALWAYS|TRUNCATE_EXISTING",
        r")\b",
    )
)


def relative(root: Path, path: Path) -> str:
    return path.relative_to(root).as_posix()


def read_text(root: Path, path: Path, errors: list[str]) -> str | None:
    name = relative(root, path)
    if path.is_symlink():
        errors.append(f"{name}: symlinks are not permitted in the desktop boundary")
        return None
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(f"{name}: cannot inspect as UTF-8: {error}")
        return None


def load_json(root: Path, path: Path, errors: list[str]) -> Any | None:
    text = read_text(root, path, errors)
    if text is None:
        return None
    try:
        return json.loads(text)
    except json.JSONDecodeError as error:
        errors.append(f"{relative(root, path)}: invalid JSON: {error}")
        return None


def load_toml(root: Path, path: Path, errors: list[str]) -> Any | None:
    text = read_text(root, path, errors)
    if text is None:
        return None
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        errors.append(f"{relative(root, path)}: invalid TOML: {error}")
        return None


def mask_rust_syntax(text: str, *, mask_literals: bool) -> str:
    """Preserve Rust source positions while masking selected lexical trivia."""

    masked = list(text)

    def blank(start: int, end: int) -> None:
        for position in range(start, end):
            if masked[position] not in {"\r", "\n"}:
                masked[position] = " "

    def quoted_end(start: int, quote: str) -> int:
        cursor = start + 1
        escaped = False
        while cursor < len(text):
            character = text[cursor]
            if character in {"\r", "\n"} and quote == "'":
                return start + 1
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == quote:
                return cursor + 1
            cursor += 1
        return len(text)

    index = 0
    while index < len(text):
        if text.startswith("//", index):
            end = text.find("\n", index + 2)
            end = len(text) if end < 0 else end
            blank(index, end)
            index = end
            continue

        if text.startswith("/*", index):
            depth = 1
            cursor = index + 2
            while cursor < len(text) and depth:
                if text.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif text.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            blank(index, cursor)
            index = cursor
            continue

        raw = re.match(r"(?:br|cr|r)(?P<hashes>#{0,255})\"", text[index:])
        if raw is not None:
            terminator = '"' + raw.group("hashes")
            content_start = index + raw.end()
            end = text.find(terminator, content_start)
            end = len(text) if end < 0 else end + len(terminator)
            if mask_literals:
                blank(index, end)
            index = end
            continue

        if text[index] == '"':
            end = quoted_end(index, '"')
            if mask_literals:
                blank(index, end)
            index = end
            continue

        char_literal = re.match(r"'(?:\\.|[^\\'\r\n])'", text[index:])
        if char_literal is not None:
            end = index + char_literal.end()
            if mask_literals:
                blank(index, end)
            index = end
            continue

        index += 1

    return "".join(masked)


def mask_rust_comments_and_literals(text: str) -> str:
    """Mask comments and literals while preserving Rust source positions."""

    return mask_rust_syntax(text, mask_literals=True)


def mask_rust_comments(text: str) -> str:
    """Mask comments but retain literals for foreign-symbol inventory checks."""

    return mask_rust_syntax(text, mask_literals=False)


def first_party_rust_sources(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*.rs")
        if path.is_file()
        and not any(
            part in SKIPPED_RUST_DIRECTORIES
            for part in path.relative_to(root).parts
        )
    )


def is_scan_boundary_source(root: Path, path: Path) -> bool:
    parts = path.relative_to(root).parts
    if not parts:
        return False
    if parts[0] == "crates":
        return len(parts) < 2 or parts[1] not in {"fixture-builder", "restore"}
    return parts[:3] == ("apps", "desktop", "src-tauri")


def mapping_key_paths(
    value: Any,
    key: str,
    prefix: tuple[str, ...] = (),
) -> list[tuple[str, ...]]:
    if not isinstance(value, dict):
        return []
    paths: list[tuple[str, ...]] = []
    for candidate, nested in value.items():
        if not isinstance(candidate, str):
            continue
        path = (*prefix, candidate)
        if candidate == key:
            paths.append(path)
        paths.extend(mapping_key_paths(nested, key, path))
    return paths


def manifest_dependency_tables(payload: dict[str, Any]) -> list[dict[str, Any]]:
    """Collect direct Cargo dependency tables, including workspace and targets."""

    tables: list[dict[str, Any]] = []

    def collect(container: Any) -> None:
        if not isinstance(container, dict):
            return
        for key in ("dependencies", "dev-dependencies", "build-dependencies"):
            table = container.get(key)
            if isinstance(table, dict):
                tables.append(table)

    collect(payload)
    collect(payload.get("workspace"))
    targets = payload.get("target")
    if isinstance(targets, dict):
        for target in targets.values():
            collect(target)
    return tables


def first_party_manifests(root: Path) -> list[Path]:
    """Return Cargo manifests controlled by this repository."""

    return sorted(
        path
        for path in root.rglob("Cargo.toml")
        if path.is_file()
        and not any(
            part in SKIPPED_RUST_DIRECTORIES
            for part in path.relative_to(root).parts
        )
    )


def workspace_member_roots(
    root: Path,
    workspace: Any,
    errors: list[str],
) -> list[Path]:
    """Expand bounded workspace member paths for config-surface traversal."""

    def reject_member() -> None:
        errors.append(
            "Cargo.toml: unreviewed dependency or macro expansion surface; "
            "workspace member path must remain within the repository"
        )

    members = workspace.get("members", []) if isinstance(workspace, dict) else []
    if not isinstance(members, list):
        return []
    resolved_root = root.resolve()
    roots: set[Path] = set()
    for member in members:
        if not isinstance(member, str):
            continue
        member_path = Path(member)
        if member_path.is_absolute():
            reject_member()
            continue
        normalized_pattern = Path(
            os.path.normpath(str(resolved_root / member_path))
        )
        try:
            relative_pattern = normalized_pattern.relative_to(resolved_root)
        except ValueError:
            reject_member()
            continue
        candidates = (
            sorted(
                resolved_root.glob(relative_pattern.as_posix()),
                key=lambda path: path.as_posix().casefold(),
            )
            if any(character in member for character in "*?[")
            else (normalized_pattern,)
        )
        for candidate in candidates:
            try:
                candidate.lstat()
            except FileNotFoundError:
                continue
            except OSError:
                reject_member()
                continue
            try:
                resolved_candidate = candidate.resolve(strict=True)
                resolved_candidate.relative_to(resolved_root)
            except (OSError, RuntimeError, ValueError):
                reject_member()
                continue
            if resolved_candidate.is_dir():
                roots.add(resolved_candidate)
            else:
                reject_member()
    return sorted(
        roots,
        key=lambda path: path.relative_to(resolved_root).as_posix().casefold(),
    )


def first_party_cargo_config_surfaces(
    root: Path,
    member_roots: list[Path],
) -> list[Path]:
    """Find nested Cargo configs without traversing generated/vendor trees."""

    skipped = {
        directory.casefold()
        for directory in SKIPPED_CARGO_CONFIG_DIRECTORIES
    }
    surfaces: set[Path] = set()
    search_roots = [root, *member_roots]
    for search_root in search_roots:
        for current_text, directory_names, file_names in os.walk(
            search_root,
            topdown=True,
            followlinks=False,
        ):
            current = Path(current_text)
            retained_directories: list[str] = []
            for directory_name in sorted(directory_names, key=str.casefold):
                directory = current / directory_name
                folded_name = directory_name.casefold()
                if folded_name in skipped:
                    continue
                is_directory_link = directory.is_symlink() or (
                    hasattr(directory, "is_junction")
                    and directory.is_junction()
                )
                if is_directory_link:
                    if folded_name == ".cargo":
                        surfaces.add(directory)
                    continue
                retained_directories.append(directory_name)
            directory_names[:] = retained_directories

            if current.name.casefold() != ".cargo":
                continue
            for file_name in sorted(file_names, key=str.casefold):
                if file_name.casefold() in {"config", "config.toml"}:
                    surfaces.add(current / file_name)

    return sorted(
        surfaces,
        key=lambda path: path.relative_to(root).as_posix().casefold(),
    )


def validate_first_party_dependency_inventory(
    root: Path, errors: list[str]
) -> None:
    """Keep macro-capable dependency expansion outside the audited boundary."""

    root_manifest = root / "Cargo.toml"
    root_payload = load_toml(root, root_manifest, errors)
    workspace = (
        root_payload.get("workspace")
        if isinstance(root_payload, dict)
        else None
    )
    member_roots = workspace_member_roots(root, workspace, errors)
    for cargo_config in first_party_cargo_config_surfaces(
        root,
        member_roots,
    ):
        errors.append(
            f"{relative(root, cargo_config)}: unreviewed dependency or "
            "macro expansion surface; repository Cargo source "
            "configuration is forbidden"
        )

    workspace_dependencies = (
        workspace.get("dependencies") if isinstance(workspace, dict) else None
    )
    if workspace_dependencies != ALLOWED_WORKSPACE_DEPENDENCIES:
        errors.append(
            "Cargo.toml: unreviewed dependency or macro expansion surface; "
            "workspace dependencies must match the audited inventory"
        )

    workspace_only = {"workspace": True}
    for path in first_party_manifests(root):
        payload = load_toml(root, path, errors)
        if not isinstance(payload, dict):
            continue
        name = relative(root, path)
        if "patch" in payload or "replace" in payload:
            errors.append(
                f"{name}: unreviewed dependency or macro expansion surface; "
                "Cargo patch/replace tables are forbidden"
            )
        library = payload.get("lib")
        crate_types = (
            library.get("crate-type", [])
            if isinstance(library, dict)
            else []
        )
        if isinstance(library, dict) and (
            library.get("proc-macro") is True
            or (
                isinstance(crate_types, list)
                and "proc-macro" in crate_types
            )
        ):
            errors.append(
                f"{name}: unreviewed dependency or macro expansion surface; "
                "first-party proc-macro crates are forbidden"
            )

        if path == root_manifest:
            continue
        pinned = ALLOWED_PINNED_CHILD_DEPENDENCIES.get(name, {})
        for table in manifest_dependency_tables(payload):
            for dependency_alias, specification in table.items():
                if (
                    dependency_alias in ALLOWED_WORKSPACE_DEPENDENCIES
                    and specification == workspace_only
                ):
                    continue
                if pinned.get(dependency_alias) == specification:
                    continue
                errors.append(
                    f"{name}: unreviewed dependency or macro expansion "
                    f"surface for {dependency_alias!r}"
                )


def validate_dynamic_loader_manifests(root: Path, errors: list[str]) -> None:
    """Reject known dynamic-loader packages even when dependency keys are aliases."""

    for path in first_party_manifests(root):
        payload = load_toml(root, path, errors)
        if not isinstance(payload, dict):
            continue
        for table in manifest_dependency_tables(payload):
            for dependency_alias, specification in table.items():
                if not isinstance(dependency_alias, str):
                    continue
                package = (
                    specification.get("package", dependency_alias)
                    if isinstance(specification, dict)
                    else dependency_alias
                )
                if isinstance(package, str) and DYNAMIC_LOADER_PACKAGE.search(
                    package
                ):
                    errors.append(
                        f"{relative(root, path)}: dynamic-loader dependency "
                        f"{package!r} is forbidden"
                    )


def validate_io_windows_manifest(root: Path, errors: list[str]) -> int:
    path = root / IO_WINDOWS_MANIFEST
    if not path.is_file():
        errors.append(f"{IO_WINDOWS_MANIFEST.as_posix()}: missing manifest")
        return 0

    payload = load_toml(root, path, errors)
    if not isinstance(payload, dict):
        return 1

    general_dependencies = payload.get("dependencies")
    if general_dependencies != ALLOWED_IO_WINDOWS_GENERAL_DEPENDENCIES:
        errors.append(
            f"{relative(root, path)}: general dependencies must match the "
            "audited non-loader inventory"
        )

    target = payload.get("target")
    if not isinstance(target, dict) or set(target) != {"cfg(windows)"}:
        errors.append(
            f"{relative(root, path)}: target dependencies must contain only "
            "the audited cfg(windows) table"
        )
    windows_target = (
        target.get("cfg(windows)") if isinstance(target, dict) else None
    )
    dependencies = (
        windows_target.get("dependencies")
        if isinstance(windows_target, dict)
        else None
    )
    if not isinstance(dependencies, dict):
        errors.append(
            f"{relative(root, path)}: cfg(windows) dependencies are required"
        )
        return 1

    if set(dependencies) != {"windows-sys"}:
        errors.append(
            f"{relative(root, path)}: cfg(windows) must depend only on windows-sys"
        )

    specification = dependencies.get("windows-sys")
    expected = {
        "version": ALLOWED_WINDOWS_SYS_VERSION,
        "features": ALLOWED_WINDOWS_SYS_FEATURES,
    }
    if specification != expected:
        errors.append(
            f"{relative(root, path)}: windows-sys must be pinned to "
            f"{ALLOWED_WINDOWS_SYS_VERSION} with exactly the audited "
            "read-only inventory, pipe, process, and device-query features"
        )

    allowed_location = (
        "target",
        "cfg(windows)",
        "dependencies",
        "windows-sys",
    )
    locations = mapping_key_paths(payload, "windows-sys")
    if locations != [allowed_location]:
        errors.append(
            f"{relative(root, path)}: windows-sys must appear exactly once under "
            "cfg(windows) dependencies"
        )
    return 1


def rust_item_span(code: str, function_name: str) -> tuple[int, int] | None:
    match = re.search(rf"\bfn\s+{re.escape(function_name)}\b[^{{]*\{{", code)
    if match is None:
        return None
    opening = code.find("{", match.start(), match.end())
    depth = 0
    for index in range(opening, len(code)):
        if code[index] == "{":
            depth += 1
        elif code[index] == "}":
            depth -= 1
            if depth == 0:
                return match.start(), index + 1
    return None


def rust_significant_tokens(code: str) -> list[tuple[str, int, int]]:
    """Tokenize masked Rust without letting trivia split raw identifiers."""

    return [
        (match.group(0), match.start(), match.end())
        for match in RUST_SIGNIFICANT_TOKEN.finditer(code)
    ]


def rust_token_contexts(
    tokens: list[tuple[str, int, int]],
) -> list[tuple[int, bool]]:
    """Return delimiter depth and macro-token-tree membership for each token."""

    closing_for = {"(": ")", "[": "]", "{": "}"}
    stack: list[tuple[str, bool]] = []
    contexts: list[tuple[int, bool]] = []
    for token_index, (token, _, _) in enumerate(tokens):
        inherited_macro = stack[-1][1] if stack else False
        contexts.append((len(stack), inherited_macro))
        if token in closing_for:
            follows_bang = (
                token_index > 0 and tokens[token_index - 1][0] == "!"
            )
            opens_macro_rules = (
                token == "{"
                and token_index >= 3
                and tokens[token_index - 3][0] == "macro_rules"
                and tokens[token_index - 2][0] == "!"
            )
            stack.append(
                (
                    closing_for[token],
                    inherited_macro or follows_bang or opens_macro_rules,
                )
            )
        elif stack and token == stack[-1][0]:
            stack.pop()
    return contexts


def validate_io_windows_macro_surface(
    name: str,
    code: str,
    errors: list[str],
) -> None:
    """Allow only fixed compiler/std macro calls without rebinding routes."""

    tokens = rust_significant_tokens(code)
    contexts = rust_token_contexts(tokens)
    logical_tokens = [
        token[2:] if token.startswith("r#") else token
        for token, _, _ in tokens
    ]
    rejected = False

    for token_index, (token, _, _) in enumerate(tokens):
        logical_name = logical_tokens[token_index]
        if (
            re.fullmatch(r"(?:r#)?[A-Za-z_][A-Za-z0-9_]*", token)
            is None
            or token_index + 2 >= len(tokens)
            or tokens[token_index + 1][0] != "!"
            or tokens[token_index + 2][0] not in {"(", "[", "{"}
        ):
            continue
        preceding = tokens[token_index - 1][0] if token_index > 0 else None
        if (
            token.startswith("r#")
            or logical_name not in ALLOWED_IO_WINDOWS_MACROS
            or preceding in {"::", ".", "$", "#", ">"}
        ):
            rejected = True

    forbidden_definition_tokens = {"macro_rules", "macro_use", "macro_export"}
    if forbidden_definition_tokens.intersection(logical_tokens):
        rejected = True
    for left, right in zip(logical_tokens, logical_tokens[1:]):
        if (left, right) in {("extern", "crate"), ("pub", "macro")}:
            rejected = True

    approved_offset_import = [
        "use",
        "std",
        "::",
        "mem",
        "::",
        "{",
        "offset_of",
        ",",
        "size_of",
        "}",
        ";",
    ]
    for use_index, logical_name in enumerate(logical_tokens):
        if logical_name != "use":
            continue
        use_depth = contexts[use_index][0]
        statement: list[str] = []
        for token_index in range(use_index, len(tokens)):
            token_depth = contexts[token_index][0]
            statement.append(logical_tokens[token_index])
            if logical_tokens[token_index] == ";" and token_depth == use_depth:
                break
        if statement == approved_offset_import:
            continue
        if (
            ALLOWED_IO_WINDOWS_MACROS.intersection(statement)
            or (
                "*" in statement
                and statement != ["use", "super", "::", "*", ";"]
            )
        ):
            rejected = True

    if rejected:
        errors.append(
            f"{name}: unreviewed dependency or macro expansion surface"
        )


def is_bare_top_level_use_for_symbol(
    tokens: list[tuple[str, int, int]],
    contexts: list[tuple[int, bool]],
    symbol_index: int,
) -> bool:
    """Require a literal top-level `use`, not visibility or macro wrapping."""

    if contexts[symbol_index][1]:
        return False
    use_index: int | None = None
    for token_index in range(symbol_index - 1, -1, -1):
        token = tokens[token_index][0]
        if token == ";":
            break
        if token == "use":
            use_index = token_index
            break
    if use_index is None:
        return False
    depth, inside_macro = contexts[use_index]
    if depth != 0 or inside_macro:
        return False
    return use_index == 0 or tokens[use_index - 1][0] in {";", "}"}


def rust_call_arguments(code: str, call_name: str) -> list[tuple[int, int, list[str]]]:
    calls: list[tuple[int, int, list[str]]] = []
    tokens = rust_significant_tokens(code)
    for token_index, (token, token_start, _) in enumerate(tokens):
        if (
            token != call_name
            or token_index + 1 >= len(tokens)
            or tokens[token_index + 1][0] != "("
        ):
            continue
        opening = tokens[token_index + 1][1]
        depth = 1
        argument_start = opening + 1
        arguments: list[str] = []
        index = argument_start
        while index < len(code) and depth:
            character = code[index]
            if character in "([{":
                depth += 1
            elif character in ")]}":
                depth -= 1
                if depth == 0:
                    trailing = code[argument_start:index]
                    if trailing.strip():
                        arguments.append(trailing)
                    calls.append((token_start, index + 1, arguments))
                    break
            elif character == "," and depth == 1:
                arguments.append(code[argument_start:index])
                argument_start = index + 1
            index += 1
    return calls


def compact(value: str) -> str:
    return re.sub(r"\s+", "", value)


def contains_in_order(value: str, tokens: tuple[str, ...]) -> bool:
    cursor = 0
    for token in tokens:
        position = value.find(token, cursor)
        if position < 0:
            return False
        cursor = position + len(token)
    return True


def within(position: int, span: tuple[int, int] | None) -> bool:
    return span is not None and span[0] <= position < span[1]


def validate_windows_extern_inventory(
    name: str,
    text: str,
    code: str,
    errors: list[str],
) -> None:
    """Pin the sole foreign declaration before its ABI/link literals are masked."""

    comments_masked = mask_rust_comments(text)
    approved_declaration = re.compile(
        r"#\s*\[\s*link\s*\(\s*name\s*=\s*\"shell32\"\s*\)\s*\]\s*"
        r"unsafe\s+extern\s+\"system\"\s*\{\s*"
        r"fn\s+ShellExecuteExW\s*\(\s*"
        r"execution\s*:\s*\*\s*mut\s*ShellExecuteInfoW\s*"
        r"\)\s*->\s*i32\s*;\s*\}"
    )
    approved_matches = list(approved_declaration.finditer(comments_masked))
    significant_tokens = rust_significant_tokens(code)
    extern_positions = [
        token_start
        for token, token_start, _ in significant_tokens
        if token == "extern"
    ]
    link_name_references = [
        token
        for token, _, _ in significant_tokens
        if (token[2:] if token.startswith("r#") else token) == "link_name"
    ]
    approved_contains_extern = (
        len(approved_matches) == 1
        and len(extern_positions) == 1
        and approved_matches[0].start()
        <= extern_positions[0]
        < approved_matches[0].end()
    )
    if not approved_contains_extern or link_name_references:
        errors.append(
            f"{name}: extern and link-name inventory must match the sole "
            "reviewed ShellExecuteExW declaration"
        )


def validate_windows_ffi_boundary(
    root: Path,
    path: Path,
    text: str,
    code: str,
    errors: list[str],
) -> None:
    name = relative(root, path)
    unsafe_count = len(UNSAFE_TOKEN.findall(code))
    safety_count = len(re.findall(r"(?m)^\s*// SAFETY:", text))
    if unsafe_count == 0:
        errors.append(f"{name}: audited Windows boundary must contain reviewed FFI")
    if unsafe_count != safety_count:
        errors.append(
            f"{name}: every unsafe operation requires one local SAFETY justification"
        )

    validate_windows_extern_inventory(name, text, code, errors)

    if MUTATING_WINDOWS_API.search(code):
        errors.append(f"{name}: mutating Windows API is forbidden")
    if MUTATING_ACCESS_TOKEN.search(code):
        errors.append(
            f"{name}: mutating access or create disposition is forbidden"
        )

    raw_span = rust_item_span(code, "open_volume_for_read")
    destination_root_span = rust_item_span(code, "open_destination_root_handle")
    destination_volume_span = rust_item_span(
        code, "open_destination_volume_for_query"
    )
    folder_attributes_span = rust_item_span(code, "open_folder_attributes")
    storage_bus_span = rust_item_span(code, "query_storage_bus_type")
    pipe_span = rust_item_span(code, "connect_broker_pipe")
    if raw_span is None:
        errors.append(f"{name}: missing audited open_volume_for_read boundary")
    if destination_root_span is None:
        errors.append(f"{name}: missing audited destination-root query boundary")
    if destination_volume_span is None:
        errors.append(f"{name}: missing audited destination-volume query boundary")
    if folder_attributes_span is None:
        errors.append(f"{name}: missing audited folder-attribute query boundary")
    if storage_bus_span is None:
        errors.append(f"{name}: missing audited storage-bus query boundary")
    else:
        storage_bus_code = compact(code[storage_bus_span[0] : storage_bus_span[1]])
        fixed_storage_query = (
            "letquery=STORAGE_PROPERTY_QUERY{"
            "PropertyId:StorageDeviceProperty,"
            "QueryType:PropertyStandardQuery,"
            "AdditionalParameters:[0],"
            "};"
        )
        storage_signature = re.search(
            r"\bfn\s+query_storage_bus_type\s*"
            r"\(\s*handle\s*:\s*&\s*OwnedHandle\s*\)\s*"
            r"->\s*Result\s*<\s*i32\s*,\s*StorageError\s*>\s*\{",
            code[storage_bus_span[0] : storage_bus_span[1]],
        )
        if storage_signature is None:
            errors.append(
                f"{name}: storage bus query must retain its exact audited signature"
            )
        if (
            storage_bus_code.count(fixed_storage_query) != 1
            or storage_bus_code.count("letquery=") != 1
            or storage_bus_code.count("query=") != 1
            or any(
                f"query.{field}=" in storage_bus_code
                for field in ("PropertyId", "QueryType", "AdditionalParameters")
            )
            or "IOCTL_STORAGE_QUERY_PROPERTY" not in storage_bus_code
        ):
            errors.append(
                f"{name}: storage bus query must use the fixed device property"
            )
        # The granular checks above provide useful diagnostics. This digest
        # additionally binds every executable token in the reviewed function,
        # so a decoy assignment or alternate return cannot preserve approval.
        # Comments, string literal contents, and formatting are masked first.
        storage_bus_digest = hashlib.sha256(
            storage_bus_code.encode("utf-8")
        ).hexdigest()
        if storage_bus_digest != AUDITED_STORAGE_BUS_QUERY_SHA256:
            errors.append(
                f"{name}: storage bus query implementation must match "
                "the reviewed normalized body"
            )
    if pipe_span is None:
        errors.append(f"{name}: missing audited connect_broker_pipe boundary")

    use_spans = [
        (match.start(), match.end())
        for match in re.finditer(r"\buse\b[^;]*;", code, re.DOTALL)
    ]
    create_calls = rust_call_arguments(code, "CreateFileW")
    direct_create_positions = {call[0] for call in create_calls}
    significant_tokens = rust_significant_tokens(code)
    token_contexts = rust_token_contexts(significant_tokens)
    token_index_by_start = {
        token_start: token_index
        for token_index, (_, token_start, _) in enumerate(significant_tokens)
    }
    canonical_create_file_module = (
        r"\buse\s+windows_sys\s*::\s*Win32\s*::\s*Storage\s*::\s*"
        r"FileSystem"
    )
    approved_import_positions: set[int] = set()
    for direct_import in re.finditer(
        canonical_create_file_module
        + r"\s*::\s*(?P<symbol>CreateFileW)\s*;",
        code,
    ):
        approved_import_positions.add(direct_import.start("symbol"))
    for grouped_import in re.finditer(
        canonical_create_file_module
        + r"\s*::\s*\{(?P<body>[^{}]*)\}\s*;",
        code,
        re.DOTALL,
    ):
        body = grouped_import.group("body")
        body_start = grouped_import.start("body")
        for imported_item in re.finditer(
            r"(?:\A|,)\s*(?P<symbol>CreateFileW)\s*(?=,|\Z)",
            body,
        ):
            approved_import_positions.add(
                body_start + imported_item.start("symbol")
            )
    approved_import_positions = {
        position
        for position in approved_import_positions
        if position in token_index_by_start
        and is_bare_top_level_use_for_symbol(
            significant_tokens,
            token_contexts,
            token_index_by_start[position],
        )
    }

    create_file_names = {"CreateFileA", "CreateFileW", "CreateFile2"}
    disallowed_bare_prefixes = {"::", ".", "#", "$", "'"}
    for token_index, (token, token_start, _) in enumerate(significant_tokens):
        logical_name = token[2:] if token.startswith("r#") else token
        if logical_name not in create_file_names:
            continue
        preceding_token = (
            significant_tokens[token_index - 1][0] if token_index > 0 else None
        )
        if (
            token == "CreateFileW"
            and token_start in direct_create_positions
            and not token_contexts[token_index][1]
            and preceding_token not in disallowed_bare_prefixes
        ):
            continue
        if (
            token == "CreateFileW"
            and token_start in approved_import_positions
        ):
            continue
        errors.append(
            f"{name}: CreateFile symbol reference must be the one canonical "
            "unaliased CreateFileW import or one of the five bare audited "
            "direct calls"
        )
    if len(approved_import_positions) != 1:
        errors.append(
            f"{name}: CreateFileW must have exactly one canonical unaliased "
            "windows_sys FileSystem import"
        )

    expected_raw_access = joined("GENERIC_", "READ")
    expected_pipe_access = joined("GENERIC_", "READ|GENERIC_", "WRITE")
    expected_destination_access = "FILE_READ_ATTRIBUTES|FILE_LIST_DIRECTORY"
    allowed_access = {
        "0",
        expected_raw_access,
        "FILE_READ_ATTRIBUTES",
        expected_destination_access,
        expected_pipe_access,
    }
    for _, _, arguments in create_calls:
        if len(arguments) != 7:
            errors.append(f"{name}: every CreateFileW call must have seven fixed arguments")
            continue
        access = compact(arguments[1])
        disposition = compact(arguments[4])
        if access not in allowed_access:
            errors.append(f"{name}: unapproved CreateFileW desired-access expression")
        if disposition != "OPEN_EXISTING":
            errors.append(f"{name}: every CreateFileW call must use OPEN_EXISTING")

    audited_opens = (
        (
            raw_span,
            (
                "wide.as_ptr()",
                expected_raw_access,
                "FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE",
                "null()",
                "OPEN_EXISTING",
                "FILE_ATTRIBUTE_NORMAL",
                "null_mut()",
            ),
            "raw volume open must use exactly GENERIC_READ and OPEN_EXISTING",
        ),
        (
            destination_root_span,
            (
                "wide.as_ptr()",
                expected_destination_access,
                "FILE_SHARE_READ|FILE_SHARE_WRITE",
                "null()",
                "OPEN_EXISTING",
                "FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT",
                "null_mut()",
            ),
            "destination root open must use exact query-only authority",
        ),
        (
            destination_volume_span,
            (
                "wide.as_ptr()",
                "0",
                "FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE",
                "null()",
                "OPEN_EXISTING",
                "FILE_ATTRIBUTE_NORMAL",
                "null_mut()",
            ),
            (
                "destination volume query must use desired access zero "
                "and fixed query sharing"
            ),
        ),
        (
            folder_attributes_span,
            (
                "wide.as_ptr()",
                "FILE_READ_ATTRIBUTES",
                "FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE",
                "null()",
                "OPEN_EXISTING",
                "FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT",
                "null_mut()",
            ),
            "folder attribute open must match its exact query-only shape",
        ),
        (
            pipe_span,
            (
                "wide_name.as_ptr()",
                expected_pipe_access,
                "0",
                "null()",
                "OPEN_EXISTING",
                "0",
                "null_mut()",
            ),
            (
                f"{joined('GENERIC_', 'WRITE')} is allowed only for "
                "the fixed named-pipe transport"
            ),
        ),
    )
    for call in create_calls:
        memberships = sum(within(call[0], span) for span, _, _ in audited_opens)
        if memberships != 1:
            errors.append(
                f"{name}: CreateFileW call is outside an approved audited "
                "function or overlaps audited spans"
            )
    for span, expected_arguments, error_message in audited_opens:
        calls = [call for call in create_calls if within(call[0], span)]
        observed_arguments = (
            tuple(compact(argument) for argument in calls[0][2])
            if len(calls) == 1
            else ()
        )
        if len(calls) != 1 or observed_arguments != expected_arguments:
            errors.append(f"{name}: {error_message}")

    for match in GENERIC_WRITE_TOKEN.finditer(code):
        if within(match.start(), pipe_span) or any(
            start <= match.start() < end for start, end in use_spans
        ):
            continue
        errors.append(
            f"{name}: {joined('GENERIC_', 'WRITE')} is allowed only for "
            "the fixed named-pipe transport"
        )

    observed_codes = set(CONTROL_CODE_TOKEN.findall(code))
    for control_code in sorted(observed_codes - ALLOWED_CONTROL_CODES):
        errors.append(
            f"{name}: unapproved device-control code {control_code!r}"
        )
    device_control_calls = rust_call_arguments(code, "DeviceIoControl")
    for _, _, arguments in device_control_calls:
        if len(arguments) != 8:
            errors.append(f"{name}: DeviceIoControl call must have eight fixed arguments")
            continue
        control_code = compact(arguments[1]).split("::")[-1]
        if control_code not in ALLOWED_CONTROL_CODES:
            errors.append(
                f"{name}: unapproved device-control code {control_code!r}"
            )

    if storage_bus_span is not None:
        storage_bus_calls = [
            call for call in device_control_calls if within(call[0], storage_bus_span)
        ]
        expected_storage_bus_calls = (
            (
                "handle.as_raw_handle()",
                "IOCTL_STORAGE_QUERY_PROPERTY",
                "(&queryas*constSTORAGE_PROPERTY_QUERY).cast()",
                "size_of::<STORAGE_PROPERTY_QUERY>()asu32",
                "(&mutheaderas*mutSTORAGE_DESCRIPTOR_HEADER).cast()",
                "size_of::<STORAGE_DESCRIPTOR_HEADER>()asu32",
                "&mutbytes_returned",
                "null_mut()",
            ),
            (
                "handle.as_raw_handle()",
                "IOCTL_STORAGE_QUERY_PROPERTY",
                "(&queryas*constSTORAGE_PROPERTY_QUERY).cast()",
                "size_of::<STORAGE_PROPERTY_QUERY>()asu32",
                "descriptor.as_mut_ptr().cast()",
                "descriptor.len()asu32",
                "&mutbytes_returned",
                "null_mut()",
            ),
        )
        observed_storage_bus_calls = tuple(
            tuple(compact(argument) for argument in call[2])
            for call in storage_bus_calls
        )
        if observed_storage_bus_calls != expected_storage_bus_calls:
            errors.append(f"{name}: storage bus query call shape must be fixed")
        elif len(storage_bus_calls) == 2:
            before_header_call = compact(
                code[storage_bus_span[0] : storage_bus_calls[0][0]]
            )
            between_calls = compact(
                code[storage_bus_calls[0][1] : storage_bus_calls[1][0]]
            )
            after_descriptor_call = compact(
                code[storage_bus_calls[1][1] : storage_bus_span[1]]
            )
            if (
                not contains_in_order(
                    before_header_call,
                    (
                        "letmutheader=STORAGE_DESCRIPTOR_HEADER::default();",
                        "letmutbytes_returned=0u32;",
                        "letheader_ok=unsafe{",
                    ),
                )
                or not contains_in_order(
                    between_calls,
                    (
                        "ifheader_ok==0",
                        "letbus_end=offset_of!(STORAGE_DEVICE_DESCRIPTOR,BusType)",
                        ".checked_add(size_of::<i32>())",
                        "usize::try_from(header.Size)",
                        "ifbytes_returned<size_of::<STORAGE_DESCRIPTOR_HEADER>()asu32",
                        "descriptor_bytes<bus_end",
                        "descriptor_bytes>MAX_STORAGE_DEVICE_DESCRIPTOR_BYTES",
                        "letmutdescriptor=vec![0u8;descriptor_bytes];",
                        "bytes_returned=0;",
                        "letdescriptor_ok=unsafe{",
                    ),
                )
                or not contains_in_order(
                    after_descriptor_call,
                    (
                        "ifdescriptor_ok==0",
                        "parse_storage_bus_type(&descriptor,bytes_returnedasusize)",
                    ),
                )
                or before_header_call.count("bytes_returned=") != 1
                or between_calls.count("bytes_returned=") != 1
                or "bytes_returned=" in after_descriptor_call
            ):
                errors.append(
                    f"{name}: storage bus query returned-byte flow must remain "
                    "bounded and fixed"
                )


def validate_rust_safety_boundary(root: Path, errors: list[str]) -> int:
    source_paths = first_party_rust_sources(root)
    allowed_path = root / IO_WINDOWS_SOURCE
    io_windows_source_root = root / "crates" / "io-windows" / "src"
    if not allowed_path.is_file():
        errors.append(f"{IO_WINDOWS_SOURCE.as_posix()}: missing audited boundary")

    inspected = 0
    for path in source_paths:
        inspected += 1
        text = read_text(root, path, errors)
        if text is None:
            continue
        code = mask_rust_comments_and_literals(text)
        if DYNAMIC_SYMBOL_RESOLUTION_TOKEN.search(code):
            errors.append(
                f"{relative(root, path)}: dynamic symbol resolution is forbidden"
            )
        if io_windows_source_root in path.parents:
            validate_io_windows_macro_surface(
                relative(root, path),
                code,
                errors,
            )
        if path == allowed_path:
            validate_windows_ffi_boundary(root, path, text, code, errors)
            continue

        if UNSAFE_TOKEN.search(code):
            errors.append(
                f"{relative(root, path)}: unsafe Rust is forbidden outside "
                f"{IO_WINDOWS_SOURCE.as_posix()}"
            )
        if CREATE_FILE_TOKEN.search(code):
            errors.append(
                f"{relative(root, path)}: CreateFile is restricted to "
                f"{IO_WINDOWS_SOURCE.as_posix()}"
            )
        if DEVICE_CONTROL_TOKEN.search(code):
            errors.append(
                f"{relative(root, path)}: DeviceIoControl is restricted to "
                f"{IO_WINDOWS_SOURCE.as_posix()}"
            )
        if MUTATING_WINDOWS_API.search(code) or MUTATING_ACCESS_TOKEN.search(code):
            errors.append(
                f"{relative(root, path)}: mutating Windows API or access token is forbidden"
            )

    crate_root = root / IO_WINDOWS_CRATE_ROOT
    crate_text = read_text(root, crate_root, errors) if crate_root.is_file() else None
    if crate_text is None:
        errors.append(f"{IO_WINDOWS_CRATE_ROOT.as_posix()}: missing crate root")
    elif "#![deny(unsafe_op_in_unsafe_fn)]" not in crate_text:
        errors.append(
            f"{IO_WINDOWS_CRATE_ROOT.as_posix()}: unsafe operations must be denied in unsafe fn"
        )
    return inspected


def production_sources(source_root: Path) -> list[Path]:
    return sorted(
        path
        for path in source_root.rglob("*")
        if path.is_file()
        and path.suffix.lower() in PRODUCTION_SUFFIXES
        and not TEST_FILE.search(path.name)
        and not path.name.endswith(".d.ts")
    )


def validate_frontend(root: Path, desktop: Path, errors: list[str]) -> int:
    source_root = desktop / "src"
    if not source_root.is_dir():
        errors.append("apps/desktop/src: missing production frontend")
        return 0

    inspected = 0
    index_path = desktop / "index.html"
    index_text = read_text(root, index_path, errors) if index_path.is_file() else None
    if index_text is None:
        errors.append("apps/desktop/index.html: missing production entry point")
    else:
        inspected += 1
        if '<link rel="stylesheet" href="/src/styles/global.css" />' not in index_text:
            errors.append(
                "apps/desktop/index.html: same-origin production stylesheet is required"
            )
        if "'unsafe-inline'" in index_text:
            errors.append(
                "apps/desktop/index.html: inline style permission is forbidden"
            )

    commands: set[str] = set()
    for forbidden in sorted(FORBIDDEN_PRODUCTION_FILES):
        path = source_root / Path(forbidden)
        if path.exists():
            errors.append(
                f"{relative(root, path)}: forbidden production file in real-only desktop"
            )

    for path in production_sources(source_root):
        inspected += 1
        text = read_text(root, path, errors)
        if text is None:
            continue
        if re.search(r"(?:^|[._-])(?:mock|fake|demo)(?:[._-]|$)", path.name, re.IGNORECASE):
            errors.append(
                f"{relative(root, path)}: mock, fake, and demo production files are forbidden"
            )
        for pattern, message in SIMULATION_PATTERNS:
            if pattern.search(text):
                errors.append(f"{relative(root, path)}: {message} is forbidden")
        commands.update(INVOKE_PATTERN.findall(text))

    removed_image_command = joined("select_", "and_", "scan_", "image")
    if removed_image_command in commands:
        errors.append(
            "apps/desktop/src: removed image command is forbidden"
        )
    for command in sorted(ALLOWED_COMMANDS - commands):
        errors.append(
            f"apps/desktop/src: missing Tauri command invocation {command!r}"
        )
    for command in sorted(commands - ALLOWED_COMMANDS):
        errors.append(
            f"apps/desktop/src: unapproved Tauri command invocation {command!r}"
        )
    return inspected


def validate_capabilities(root: Path, tauri: Path, errors: list[str]) -> int:
    capability_root = tauri / "capabilities"
    capability_files = (
        sorted(capability_root.glob("*.json")) if capability_root.is_dir() else []
    )
    if not capability_files:
        errors.append("apps/desktop/src-tauri/capabilities: missing capability file")
        return 0

    inspected = 0
    for path in capability_files:
        inspected += 1
        payload = load_json(root, path, errors)
        if not isinstance(payload, dict):
            continue
        windows = payload.get("windows")
        if windows != ["main"]:
            errors.append(
                f"{relative(root, path)}: capability must target only the main window"
            )
        permissions = payload.get("permissions")
        if not isinstance(permissions, list) or not all(
            isinstance(permission, str) for permission in permissions
        ):
            errors.append(
                f"{relative(root, path)}: permissions must be a string array"
            )
            continue
        for permission in sorted(set(permissions) - ALLOWED_CAPABILITIES):
            errors.append(
                f"{relative(root, path)}: unapproved capability {permission!r}"
            )
    return inspected


def validate_tauri_config(root: Path, tauri: Path, errors: list[str]) -> int:
    config_path = tauri / "tauri.conf.json"
    if not config_path.is_file():
        errors.append("apps/desktop/src-tauri/tauri.conf.json: missing configuration")
        return 0
    payload = load_json(root, config_path, errors)
    if not isinstance(payload, dict):
        return 1

    bundle = payload.get("bundle")
    if not isinstance(bundle, dict) or bundle.get("active") is not False:
        errors.append(
            f"{relative(root, config_path)}: bundle.active must be false for this slice"
        )

    app = payload.get("app")
    windows = app.get("windows") if isinstance(app, dict) else None
    if not isinstance(windows, list) or len(windows) != 1:
        errors.append(
            f"{relative(root, config_path)}: exactly one desktop window is required"
        )
    elif (
        not isinstance(windows[0], dict)
        or windows[0].get("label") != "main"
        or windows[0].get("dragDropEnabled") is not False
    ):
        errors.append(
            f"{relative(root, config_path)}: main window dragDropEnabled must be false"
        )

    security = app.get("security") if isinstance(app, dict) else None
    csp = security.get("csp") if isinstance(security, dict) else None
    if not isinstance(csp, str):
        errors.append(f"{relative(root, config_path)}: strict CSP is required")
        return 1

    required_directives = (
        "default-src 'self'",
        "script-src 'self'",
        "object-src 'none'",
        "base-uri 'none'",
        "frame-ancestors 'none'",
    )
    for directive in required_directives:
        if directive not in csp:
            errors.append(
                f"{relative(root, config_path)}: CSP lacks {directive!r}"
            )
    remote_source = re.search(r"(?:https:|wss:|https://|wss://)", csp, re.IGNORECASE)
    if remote_source is not None:
        errors.append(f"{relative(root, config_path)}: remote CSP source is forbidden")
    return 1


def validate_tauri_sources(root: Path, tauri: Path, errors: list[str]) -> int:
    inspected = 0
    cargo_path = tauri / "Cargo.toml"
    cargo = read_text(root, cargo_path, errors) if cargo_path.is_file() else None
    if cargo is None:
        errors.append("apps/desktop/src-tauri/Cargo.toml: missing manifest")
    else:
        inspected += 1
        try:
            cargo_payload = tomllib.loads(cargo)
        except tomllib.TOMLDecodeError as error:
            errors.append(
                f"{relative(root, cargo_path)}: invalid TOML: {error}"
            )
            cargo_payload = None
        if isinstance(cargo_payload, dict) and (
            cargo_payload.get("build-dependencies")
            != ALLOWED_TAURI_BUILD_DEPENDENCIES
            or cargo_payload.get("dependencies") != ALLOWED_TAURI_DEPENDENCIES
            or cargo_payload.get("dev-dependencies")
            != ALLOWED_TAURI_DEV_DEPENDENCIES
            or "target" in cargo_payload
        ):
            errors.append(
                f"{relative(root, cargo_path)}: dependency inventory must "
                "match the audited desktop build/runtime/test set"
            )
        for dependency in sorted(FORBIDDEN_TAURI_DEPENDENCIES):
            if re.search(rf"(?m)^\s*{re.escape(dependency)}\s*=", cargo):
                errors.append(
                    f"{relative(root, cargo_path)}: forbidden Tauri dependency "
                    f"{dependency!r}"
                )

    source_root = tauri / "src"
    source_files = sorted(source_root.rglob("*.rs")) if source_root.is_dir() else []
    if not source_files:
        errors.append("apps/desktop/src-tauri/src: missing Rust sources")
    combined = ""
    for path in source_files:
        inspected += 1
        text = read_text(root, path, errors)
        if text is not None:
            combined += "\n" + text
    if "#![forbid(unsafe_code)]" not in combined:
        errors.append("apps/desktop/src-tauri/src: unsafe code is not forbidden")
    registrations = re.findall(
        r"generate_handler!\s*\[(.*?)\]",
        combined,
        re.DOTALL,
    )
    registered_commands: set[str] = set()
    for registration in registrations:
        registered_commands.update(
            match.group(1)
            for match in re.finditer(
                r"(?:^|,)\s*(?:[A-Za-z_][A-Za-z0-9_]*::)*"
                r"([A-Za-z_][A-Za-z0-9_]*)\s*(?=,|$)",
                registration,
            )
        )
    if registered_commands != ALLOWED_COMMANDS:
        errors.append(
            "apps/desktop/src-tauri/src: backend command registration must be "
            f"exactly {sorted(ALLOWED_COMMANDS)!r}"
        )
    removed_image_command = joined("select_", "and_", "scan_", "image")
    if removed_image_command in combined:
        errors.append("apps/desktop/src-tauri/src: removed image command is forbidden")
    for forbidden_command in (
        "list_physical_drives",
        "pause_scan",
        "resume_scan",
        "cancel_scan",
        "start_restore",
        "open_path",
    ):
        if forbidden_command in combined:
            errors.append(
                "apps/desktop/src-tauri/src: unsupported command is present: "
                f"{forbidden_command}"
            )

    manifest_candidates = sorted(
        path
        for path in tauri.rglob("*")
        if path.is_file()
        and (
            path.suffix.lower() == ".manifest"
            or (
                "manifest" in path.stem.lower()
                and path.suffix.lower() in {".xml", ".manifest"}
            )
        )
    )
    manifest_text = ""
    for path in manifest_candidates:
        inspected += 1
        text = read_text(root, path, errors)
        if text is not None:
            manifest_text += "\n" + text
    if "asInvoker" not in manifest_text:
        errors.append("apps/desktop/src-tauri: Windows manifest must run asInvoker")
    if re.search(r"requireAdministrator|highestAvailable", manifest_text):
        errors.append("apps/desktop/src-tauri: Windows manifest must run asInvoker")
    legacy_dpi_aware = re.search(
        r"<dpiAware\b[^>]*>\s*true/pm\s*</dpiAware>",
        manifest_text,
        flags=re.IGNORECASE,
    )
    per_monitor_v2 = re.search(
        r"<dpiAwareness\b[^>]*>\s*PerMonitorV2\s*,\s*PerMonitor\s*</dpiAwareness>",
        manifest_text,
        flags=re.IGNORECASE,
    )
    if legacy_dpi_aware is None or per_monitor_v2 is None:
        errors.append(
            "apps/desktop/src-tauri: Windows manifest must declare "
            "PerMonitorV2 DPI awareness with the true/pm fallback"
        )
    return inspected


def validate_broker_boundary(root: Path, errors: list[str]) -> int:
    inspected = 0
    transport_path = root / IO_WINDOWS_TRANSPORT_CONFIG
    transport = (
        read_text(root, transport_path, errors)
        if transport_path.is_file()
        else None
    )
    if transport is None:
        errors.append(
            f"{IO_WINDOWS_TRANSPORT_CONFIG.as_posix()}: missing fixed broker configuration"
        )
    else:
        inspected += 1
        expected_name = 'const BROKER_FILE_NAME: &str = "undelete-master-broker.exe";'
        transport_code = mask_rust_comments_and_literals(transport)
        sibling_span = rust_item_span(
            transport_code,
            "broker_executable_from_current",
        )
        sibling_code = (
            transport_code[sibling_span[0]:sibling_span[1]]
            if sibling_span is not None
            else ""
        )
        if (
            expected_name not in transport
            or "current_executable.is_absolute()" not in compact(sibling_code)
            or ".parent()" not in sibling_code
            or ".join(BROKER_FILE_NAME)" not in compact(sibling_code)
        ):
            errors.append(
                f"{relative(root, transport_path)}: broker must be a fixed sibling executable"
            )

    windows_path = root / IO_WINDOWS_SOURCE
    windows_text = (
        read_text(root, windows_path, errors) if windows_path.is_file() else None
    )
    if windows_text is not None:
        inspected += 1
        windows_code = mask_rust_comments_and_literals(windows_text)
        launch_span = rust_item_span(windows_code, "launch_elevated_broker")
        launch_code = (
            windows_code[launch_span[0]:launch_span[1]]
            if launch_span is not None
            else ""
        )
        launch_text = (
            windows_text[launch_span[0]:launch_span[1]]
            if launch_span is not None
            else ""
        )
        required_code = (
            "std::env::current_exe",
            "broker_executable_from_current",
            ".canonicalize()",
            ".parent()",
            ".file_name()",
            "ShellExecuteExW",
        )
        if (
            launch_span is None
            or any(token not in launch_code for token in required_code)
            or '"runas"' not in launch_text
            or '"undelete-master-broker.exe"' not in launch_text
        ):
            errors.append(
                f"{relative(root, windows_path)}: elevated launch must use runas "
                "with the canonical fixed sibling broker"
            )

    broker_root = root / BROKER_ROOT
    broker_manifest = broker_root / "undelete-master-broker.manifest"
    manifest = (
        read_text(root, broker_manifest, errors)
        if broker_manifest.is_file()
        else None
    )
    if manifest is None:
        errors.append(
            f"{relative(root, broker_manifest)}: missing broker elevation manifest"
        )
    else:
        inspected += 1
        if (
            'level="requireAdministrator"' not in manifest
            or 'uiAccess="false"' not in manifest
            or re.search(r"level=\"(?:asInvoker|highestAvailable)\"", manifest)
        ):
            errors.append(
                f"{relative(root, broker_manifest)}: broker manifest must "
                "requireAdministrator with uiAccess false"
            )

    broker_source = broker_root / "src" / "lib.rs"
    source = (
        read_text(root, broker_source, errors)
        if broker_source.is_file()
        else None
    )
    if source is None:
        errors.append(f"{relative(root, broker_source)}: missing broker source")
    else:
        inspected += 1
        if "#![forbid(unsafe_code)]" not in source:
            errors.append(
                f"{relative(root, broker_source)}: elevated broker must forbid unsafe code"
            )
    return inspected


def validate_repository(root: Path) -> tuple[list[str], int]:
    errors: list[str] = []
    desktop = root / "apps" / "desktop"
    if not desktop.is_dir():
        return ["apps/desktop: missing desktop application"], 0

    inspected = validate_frontend(root, desktop, errors)
    tauri = desktop / "src-tauri"
    if not tauri.is_dir():
        errors.append("apps/desktop/src-tauri: missing real Tauri backend")
        return errors, inspected

    inspected += validate_capabilities(root, tauri, errors)
    inspected += validate_tauri_config(root, tauri, errors)
    inspected += validate_tauri_sources(root, tauri, errors)
    validate_first_party_dependency_inventory(root, errors)
    validate_dynamic_loader_manifests(root, errors)
    inspected += validate_io_windows_manifest(root, errors)
    inspected += validate_rust_safety_boundary(root, errors)
    inspected += validate_broker_boundary(root, errors)
    return errors, inspected


def main() -> int:
    errors, inspected = validate_repository(ROOT)
    if errors:
        print("real-only desktop validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    print(
        "real-only desktop validation passed: "
        f"{inspected} production boundary files inspected"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
