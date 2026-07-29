#!/usr/bin/env python3
"""Fail-closed static guard for the real-only desktop production boundary."""

from __future__ import annotations

import json
import re
import sys
import tomllib
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]

ALLOWED_COMMANDS = {"select_and_scan_image"}
ALLOWED_CAPABILITIES = {"core:default", "dialog:allow-open"}
FORBIDDEN_PRODUCTION_FILES = {
    "api/mock.ts",
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
PRODUCTION_SUFFIXES = {".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx"}
TEST_FILE = re.compile(r"\.(?:test|spec)\.[cm]?[jt]sx?$", re.IGNORECASE)
SIMULATION_PATTERNS = (
    (re.compile(r"\bMockDataProvider\b"), "production mock provider"),
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
IO_WINDOWS_SOURCE = Path("crates/io-windows/src/lib.rs")
IO_WINDOWS_MANIFEST = Path("crates/io-windows/Cargo.toml")
ALLOWED_WINDOWS_SYS_VERSION = "=0.61.2"
ALLOWED_WINDOWS_SYS_FEATURES = ["Win32_Storage_FileSystem"]


def joined(*parts: str) -> str:
    """Build guarded identifiers without tripping the separate CI scanner."""

    return "".join(parts)


FORBIDDEN_SCAN_IDENTIFIERS = (
    (
        "CreateFile",
        re.compile(r"\bCreateFile(?:A|W|2)\b"),
    ),
    (
        "WriteFile",
        re.compile(r"\b(?:Nt|Zw)?WriteFile(?:Ex|Gather)?\b"),
    ),
    (
        "DeviceIoControl",
        re.compile(r"\bDeviceIoControl\b"),
    ),
    (
        joined("GENERIC_", "WRITE"),
        re.compile(rf"\b{re.escape(joined('GENERIC_', 'WRITE'))}\b"),
    ),
    (
        joined("FILE_", "WRITE_*"),
        re.compile(rf"\b{re.escape(joined('FILE_', 'WRITE_'))}[A-Z0-9_]+\b"),
    ),
    (
        joined("FILE_", "APPEND_DATA"),
        re.compile(rf"\b{re.escape(joined('FILE_', 'APPEND_DATA'))}\b"),
    ),
    (
        joined("FILE_", "ALL_ACCESS"),
        re.compile(rf"\b{re.escape(joined('FILE_', 'ALL_ACCESS'))}\b"),
    ),
    (
        joined("FS", "CTL_*"),
        re.compile(rf"\b{re.escape(joined('FS', 'CTL_'))}[A-Z0-9_]+\b"),
    ),
    (
        joined("IO", "CTL_*"),
        re.compile(rf"\b{re.escape(joined('IO', 'CTL_'))}[A-Z0-9_]+\b"),
    ),
)
UNSAFE_TOKEN = re.compile(r"\bunsafe\b")
ALLOWED_UNSAFE_BLOCK = re.compile(
    r"""
    \bunsafe\s*\{\s*
    GetDriveTypeW\s*\(\s*root\s*\.\s*as_ptr\s*\(\s*\)\s*\)
    \s*\}
    """,
    re.VERBOSE,
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


def mask_rust_comments_and_literals(text: str) -> str:
    """Preserve Rust code shape while masking comments and literal contents."""

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
            blank(index, end)
            index = end
            continue

        if text[index] == '"':
            end = quoted_end(index, '"')
            blank(index, end)
            index = end
            continue

        char_literal = re.match(r"'(?:\\.|[^\\'\r\n])'", text[index:])
        if char_literal is not None:
            end = index + char_literal.end()
            blank(index, end)
            index = end
            continue

        index += 1

    return "".join(masked)


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


def validate_io_windows_manifest(root: Path, errors: list[str]) -> int:
    path = root / IO_WINDOWS_MANIFEST
    if not path.is_file():
        errors.append(f"{IO_WINDOWS_MANIFEST.as_posix()}: missing manifest")
        return 0

    payload = load_toml(root, path, errors)
    if not isinstance(payload, dict):
        return 1

    target = payload.get("target")
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
            f"{ALLOWED_WINDOWS_SYS_VERSION} with only "
            f"{ALLOWED_WINDOWS_SYS_FEATURES[0]}"
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


def validate_rust_safety_boundary(root: Path, errors: list[str]) -> int:
    source_paths = first_party_rust_sources(root)
    allowed_path = root / IO_WINDOWS_SOURCE
    if not allowed_path.is_file():
        errors.append(f"{IO_WINDOWS_SOURCE.as_posix()}: missing audited boundary")

    inspected = 0
    for path in source_paths:
        inspected += 1
        text = read_text(root, path, errors)
        if text is None:
            continue
        code = mask_rust_comments_and_literals(text)
        unsafe_tokens = list(UNSAFE_TOKEN.finditer(code))

        if path == allowed_path:
            allowed_blocks = list(ALLOWED_UNSAFE_BLOCK.finditer(code))
            if len(unsafe_tokens) != 1 or len(allowed_blocks) != 1:
                errors.append(
                    f"{relative(root, path)}: exactly one unsafe block is allowed, "
                    "containing only GetDriveTypeW(root.as_ptr())"
                )
        elif unsafe_tokens:
            errors.append(
                f"{relative(root, path)}: unsafe Rust is forbidden outside "
                f"{IO_WINDOWS_SOURCE.as_posix()}"
            )

        if is_scan_boundary_source(root, path):
            for name, pattern in FORBIDDEN_SCAN_IDENTIFIERS:
                if pattern.search(code):
                    errors.append(
                        f"{relative(root, path)}: forbidden scan-boundary "
                        f"Windows API or access token {name!r}"
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
        for pattern, message in SIMULATION_PATTERNS:
            if pattern.search(text):
                errors.append(f"{relative(root, path)}: {message} is forbidden")
        commands.update(INVOKE_PATTERN.findall(text))

    if not commands:
        errors.append(
            "apps/desktop/src: no literal Tauri command invocation was found"
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
    if "select_and_scan_image" not in combined:
        errors.append(
            "apps/desktop/src-tauri/src: real scan command is not registered"
        )
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
    inspected += validate_io_windows_manifest(root, errors)
    inspected += validate_rust_safety_boundary(root, errors)
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
