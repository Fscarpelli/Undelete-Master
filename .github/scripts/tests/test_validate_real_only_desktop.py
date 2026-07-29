from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).resolve().parents[1] / "validate_real_only_desktop.py"
SPEC = importlib.util.spec_from_file_location("validate_real_only_desktop", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
validator = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = validator
SPEC.loader.exec_module(validator)


class RealOnlyDesktopValidatorTests(unittest.TestCase):
    def make_repo(self) -> tuple[tempfile.TemporaryDirectory[str], Path]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        desktop = root / "apps" / "desktop"
        source = desktop / "src"
        tauri = desktop / "src-tauri"
        io_windows = root / "crates" / "io-windows"
        (source / "api").mkdir(parents=True)
        (tauri / "src").mkdir(parents=True)
        (tauri / "capabilities").mkdir()
        (io_windows / "src").mkdir(parents=True)

        (source / "api" / "desktop.ts").write_text(
            'import { invoke } from "@tauri-apps/api/core";\n'
            'export const scan = (requestId: string) => '
            'invoke("select_and_scan_image", { requestId });\n',
            encoding="utf-8",
        )
        (source / "App.tsx").write_text(
            "export function App() { return <main>Real image scan</main>; }\n",
            encoding="utf-8",
        )
        (desktop / "index.html").write_text(
            "<!doctype html><html><head>"
            "<meta http-equiv=\"Content-Security-Policy\" "
            "content=\"default-src 'self'; style-src 'self'\">"
            '<link rel="stylesheet" href="/src/styles/global.css" />'
            "</head><body><div id=\"root\"></div></body></html>\n",
            encoding="utf-8",
        )
        (tauri / "src" / "lib.rs").write_text(
            "#![forbid(unsafe_code)]\n"
            "pub fn run() {\n"
            "  tauri::Builder::default()\n"
            "    .invoke_handler(tauri::generate_handler![select_and_scan_image]);\n"
            "}\n",
            encoding="utf-8",
        )
        (tauri / "Cargo.toml").write_text(
            "[dependencies]\n"
            'tauri = "2.11.5"\n'
            'tauri-plugin-dialog = "2.7.2"\n',
            encoding="utf-8",
        )
        (tauri / "capabilities" / "main.json").write_text(
            json.dumps(
                {
                    "identifier": "main",
                    "windows": ["main"],
                    "permissions": ["core:default", "dialog:allow-open"],
                }
            ),
            encoding="utf-8",
        )
        (tauri / "tauri.conf.json").write_text(
            json.dumps(
                {
                    "app": {
                        "windows": [
                            {
                                "label": "main",
                                "dragDropEnabled": False,
                            }
                        ],
                        "security": {
                            "csp": "default-src 'self'; script-src 'self'; "
                            "style-src 'self'; img-src 'self' data:; "
                            "connect-src ipc: http://ipc.localhost; "
                            "object-src 'none'; base-uri 'none'; "
                            "frame-ancestors 'none'"
                        }
                    },
                    "bundle": {"active": False},
                }
            ),
            encoding="utf-8",
        )
        (tauri / "windows-app-manifest.xml").write_text(
            '<requestedExecutionLevel level="asInvoker" uiAccess="false" />\n',
            encoding="utf-8",
        )
        (io_windows / "src" / "lib.rs").write_text(
            "#![deny(unsafe_op_in_unsafe_fn)]\n"
            "use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;\n"
            "fn classify(root: &[u16; 4]) -> u32 {\n"
            "  // The root is a live NUL-terminated buffer.\n"
            "  unsafe { GetDriveTypeW(root.as_ptr()) }\n"
            "}\n",
            encoding="utf-8",
        )
        (io_windows / "Cargo.toml").write_text(
            "[package]\n"
            'name = "um-io-windows"\n'
            'version = "0.1.0"\n'
            'edition = "2021"\n'
            "\n"
            "[dependencies]\n"
            'thiserror = "2"\n'
            "\n"
            "[target.'cfg(windows)'.dependencies]\n"
            'windows-sys = { version = "=0.61.2", '
            'features = ["Win32_Storage_FileSystem"] }\n',
            encoding="utf-8",
        )
        return temporary, root

    def test_desktop_real_only_001_accepts_minimal_real_runtime(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)

        errors, inspected = validator.validate_repository(root)

        self.assertEqual(errors, [])
        self.assertGreaterEqual(inspected, 6)

    def test_desktop_real_only_002_rejects_production_mock_and_simulation(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        source = root / "apps" / "desktop" / "src"
        (source / "api" / "mock.ts").write_text(
            "export class MockDataProvider { start() { "
            "return setInterval(() => Math.random(), 1000); } }\n",
            encoding="utf-8",
        )

        errors, _ = validator.validate_repository(root)

        self.assertTrue(any("forbidden production file" in error for error in errors))
        self.assertTrue(any("simulated runtime primitive" in error for error in errors))

    def test_desktop_real_only_003_rejects_extra_frontend_commands(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        api = root / "apps" / "desktop" / "src" / "api" / "desktop.ts"
        api.write_text(
            'import { invoke } from "@tauri-apps/api/core";\n'
            'export const restore = () => invoke("start_restore");\n',
            encoding="utf-8",
        )

        errors, _ = validator.validate_repository(root)

        self.assertTrue(any("unapproved Tauri command" in error for error in errors))

    def test_desktop_real_only_004_rejects_remote_csp_and_elevation(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        tauri = root / "apps" / "desktop" / "src-tauri"
        config = json.loads((tauri / "tauri.conf.json").read_text(encoding="utf-8"))
        config["app"]["security"]["csp"] += "; connect-src https://example.invalid"
        (tauri / "tauri.conf.json").write_text(
            json.dumps(config),
            encoding="utf-8",
        )
        (tauri / "windows-app-manifest.xml").write_text(
            '<requestedExecutionLevel level="requireAdministrator" />\n',
            encoding="utf-8",
        )

        errors, _ = validator.validate_repository(root)

        self.assertTrue(any("remote CSP source" in error for error in errors))
        self.assertTrue(any("must run asInvoker" in error for error in errors))

    def test_desktop_real_only_005_rejects_privileged_plugins(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        tauri = root / "apps" / "desktop" / "src-tauri"
        cargo = tauri / "Cargo.toml"
        cargo.write_text(
            cargo.read_text(encoding="utf-8") + 'tauri-plugin-shell = "2"\n',
            encoding="utf-8",
        )
        capabilities = tauri / "capabilities" / "main.json"
        payload = json.loads(capabilities.read_text(encoding="utf-8"))
        payload["permissions"].append("fs:allow-write-file")
        capabilities.write_text(json.dumps(payload), encoding="utf-8")

        errors, _ = validator.validate_repository(root)

        self.assertTrue(any("forbidden Tauri dependency" in error for error in errors))
        self.assertTrue(any("unapproved capability" in error for error in errors))

    def test_desktop_real_only_006_rejects_drag_drop_path_ingress(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        config_path = (
            root / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
        )
        config = json.loads(config_path.read_text(encoding="utf-8"))
        config["app"]["windows"][0]["dragDropEnabled"] = True
        config_path.write_text(json.dumps(config), encoding="utf-8")

        errors, _ = validator.validate_repository(root)

        self.assertTrue(any("dragDropEnabled" in error for error in errors))

    def test_desktop_real_only_007_rejects_csp_incompatible_styles(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        index = root / "apps" / "desktop" / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                '<link rel="stylesheet" href="/src/styles/global.css" />',
                "",
            ),
            encoding="utf-8",
        )

        errors, _ = validator.validate_repository(root)

        self.assertTrue(any("same-origin production stylesheet" in error for error in errors))

    def test_desktop_real_only_008_rejects_second_unsafe_block(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "lib.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn duplicate(root: &[u16; 4]) -> u32 {\n"
            + "  unsafe { GetDriveTypeW(root.as_ptr()) }\n"
            + "}\n",
            encoding="utf-8",
        )

        errors, _ = validator.validate_repository(root)

        self.assertTrue(any("exactly one unsafe block" in error for error in errors))

    def test_desktop_real_only_009_rejects_write_api_in_scan_boundary(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "lib.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn mutate_source() {\n"
            + "  WriteFile();\n"
            + "}\n",
            encoding="utf-8",
        )

        errors, _ = validator.validate_repository(root)

        self.assertTrue(
            any("forbidden scan-boundary" in error for error in errors)
        )

    def test_desktop_real_only_010_rejects_unsafe_outside_boundary(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        tauri_source = root / "apps" / "desktop" / "src-tauri" / "src" / "lib.rs"
        tauri_source.write_text(
            tauri_source.read_text(encoding="utf-8")
            + "fn bypass() {\n"
            + "  unsafe { core::ptr::read(core::ptr::null()) };\n"
            + "}\n",
            encoding="utf-8",
        )

        errors, _ = validator.validate_repository(root)

        self.assertTrue(
            any("unsafe Rust is forbidden outside" in error for error in errors)
        )

    def test_desktop_real_only_011_rejects_extra_windows_sys_feature(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "crates" / "io-windows" / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                'features = ["Win32_Storage_FileSystem"]',
                'features = ["Win32_Storage_FileSystem", "Win32_System_IO"]',
            ),
            encoding="utf-8",
        )

        errors, _ = validator.validate_repository(root)

        self.assertTrue(any("windows-sys must be pinned" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
