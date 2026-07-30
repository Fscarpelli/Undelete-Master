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


def token(*parts: str) -> str:
    return "".join(parts)


class RealOnlyDesktopValidatorTests(unittest.TestCase):
    def make_repo(self) -> tuple[tempfile.TemporaryDirectory[str], Path]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        desktop = root / "apps" / "desktop"
        source = desktop / "src"
        tauri = desktop / "src-tauri"
        io_windows = root / "crates" / "io-windows"
        broker = root / "crates" / "elevated-broker"
        (source / "api").mkdir(parents=True)
        (tauri / "src").mkdir(parents=True)
        (tauri / "capabilities").mkdir()
        (io_windows / "src").mkdir(parents=True)
        (broker / "src").mkdir(parents=True)

        (source / "api" / "storageDesktop.ts").write_text(
            'import { invoke } from "@tauri-apps/api/core";\n'
            'export const list = (requestId: string) => '
            'invoke("list_storage_sources", { requestId });\n'
            'export const folder = (requestId: string, volumeId: string) => '
            'invoke("select_scan_folder", { requestId, volumeId });\n'
            'export const scan = (requestId: string, volumeId: string) => '
            'invoke("scan_storage_volume", { requestId, volumeId });\n'
            'export const page = (requestId: string, sessionId: string) => '
            'invoke("get_candidate_page", { requestId, sessionId });\n'
            'export const query = (requestId: string, sessionId: string) => '
            'invoke("query_candidate_page", { requestId, sessionId });\n'
            'export const select = (requestId: string, sessionId: string) => '
            'invoke("update_candidate_selection", { requestId, sessionId });\n',
            encoding="utf-8",
        )
        (source / "App.tsx").write_text(
            "export function App() { return <main>Real mounted-volume scan</main>; }\n",
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
            "    .invoke_handler(tauri::generate_handler![\n"
            "      storage::list_storage_sources,\n"
            "      storage::select_scan_folder,\n"
            "      storage::scan_storage_volume,\n"
            "      storage::get_candidate_page,\n"
            "      storage::query_candidate_page,\n"
            "      storage::update_candidate_selection,\n"
            "    ]);\n"
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
                        },
                    },
                    "bundle": {"active": False},
                }
            ),
            encoding="utf-8",
        )
        (tauri / "windows-app-manifest.xml").write_text(
            '<requestedExecutionLevel level="asInvoker" uiAccess="false" />\n'
            '<dpiAware>true/pm</dpiAware>\n'
            '<dpiAwareness>PerMonitorV2, PerMonitor</dpiAwareness>\n',
            encoding="utf-8",
        )

        (io_windows / "src" / "lib.rs").write_text(
            "#![deny(unsafe_op_in_unsafe_fn)]\n"
            "#[cfg(windows)] mod windows;\n"
            "mod transport_config;\n",
            encoding="utf-8",
        )
        read_access = token("GENERIC_", "READ")
        pipe_write_access = token("GENERIC_", "WRITE")
        windows_source = (
            "use windows_sys::Win32::Foundation::{"
            f"{read_access}, {pipe_write_access}"
            "};\n"
            "use windows_sys::Win32::Storage::FileSystem::CreateFileW;\n"
            "fn open_volume_for_read(wide: &[u16]) {\n"
            "  // SAFETY: fixed read-only selector and arguments.\n"
            "  unsafe { CreateFileW(\n"
            f"    wide.as_ptr(), {read_access}, "
            "FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,\n"
            "    null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, null_mut(),\n"
            "  ); }\n"
            "}\n"
            "fn open_destination_root_handle(wide: &[u16]) {\n"
            "  // SAFETY: fixed query-only destination root arguments.\n"
            "  unsafe { CreateFileW(\n"
            "    wide.as_ptr(), FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY,\n"
            "    FILE_SHARE_READ | FILE_SHARE_WRITE,\n"
            "    null(), OPEN_EXISTING,\n"
            "    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,\n"
            "    null_mut(),\n"
            "  ); }\n"
            "}\n"
            "fn open_destination_volume_for_query(wide: &[u16]) {\n"
            "  // SAFETY: fixed derived-volume query-only arguments.\n"
            "  unsafe { CreateFileW(\n"
            "    wide.as_ptr(), 0,\n"
            "    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,\n"
            "    null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, null_mut(),\n"
            "  ); }\n"
            "}\n"
            "fn open_folder_attributes(wide: &[u16]) {\n"
            "  // SAFETY: fixed query-only folder attribute arguments.\n"
            "  unsafe { CreateFileW(\n"
            "    wide.as_ptr(), FILE_READ_ATTRIBUTES,\n"
            "    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,\n"
            "    null(), OPEN_EXISTING,\n"
            "    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,\n"
            "    null_mut(),\n"
            "  ); }\n"
            "}\n"
            "fn connect_broker_pipe(pipe_suffix: &str, wide_name: &[u16]) {\n"
            "  let _pipe_name = build_pipe_name(pipe_suffix);\n"
            "  // SAFETY: fixed local named-pipe selector and duplex transport.\n"
            "  unsafe { CreateFileW(\n"
            f"    wide_name.as_ptr(), {read_access} | {pipe_write_access}, 0,\n"
            "    null(), OPEN_EXISTING, 0, null_mut(),\n"
            "  ); }\n"
            "}\n"
            "fn query_volume(handle: HANDLE) {\n"
            "  // SAFETY: fixed read-only volume extents query.\n"
            "  unsafe { DeviceIoControl(handle, "
            "IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, null(), 0, null_mut(), 0, "
            "null_mut(), null_mut()); }\n"
            "}\n"
            "fn query_length(handle: HANDLE) {\n"
            "  // SAFETY: fixed read-only length query.\n"
            "  unsafe { DeviceIoControl(handle, IOCTL_DISK_GET_LENGTH_INFO, "
            "null(), 0, null_mut(), 0, null_mut(), null_mut()); }\n"
            "}\n"
            "fn query_alignment(handle: HANDLE) {\n"
            "  // SAFETY: fixed read-only storage-property query.\n"
            "  unsafe { DeviceIoControl(handle, IOCTL_STORAGE_QUERY_PROPERTY, "
            "null(), 0, null_mut(), 0, null_mut(), null_mut()); }\n"
            "}\n"
            "fn query_storage_bus_type(handle: &OwnedHandle) -> Result<i32, StorageError> {\n"
            "  let query = STORAGE_PROPERTY_QUERY {\n"
            "    PropertyId: StorageDeviceProperty,\n"
            "    QueryType: PropertyStandardQuery,\n"
            "    AdditionalParameters: [0],\n"
            "  };\n"
            "  let mut header = STORAGE_DESCRIPTOR_HEADER::default();\n"
            "  let mut bytes_returned = 0u32;\n"
            "  // SAFETY: fixed read-only device-property header query.\n"
            "  let header_ok = unsafe {\n"
            "    DeviceIoControl(\n"
            "      handle.as_raw_handle(),\n"
            "      IOCTL_STORAGE_QUERY_PROPERTY,\n"
            "      (&query as *const STORAGE_PROPERTY_QUERY).cast(),\n"
            "      size_of::<STORAGE_PROPERTY_QUERY>() as u32,\n"
            "      (&mut header as *mut STORAGE_DESCRIPTOR_HEADER).cast(),\n"
            "      size_of::<STORAGE_DESCRIPTOR_HEADER>() as u32,\n"
            "      &mut bytes_returned,\n"
            "      null_mut(),\n"
            "    )\n"
            "  };\n"
            "  if header_ok == 0 {\n"
            "    return Err(last_windows_error(\n"
            '      "device-header",\n'
            "    ));\n"
            "  }\n"
            "  let bus_end = offset_of!(STORAGE_DEVICE_DESCRIPTOR, BusType)\n"
            "    .checked_add(size_of::<i32>())\n"
            "    .ok_or(StorageError::InvalidGeometry)?;\n"
            "  let descriptor_bytes =\n"
            "    usize::try_from(header.Size).map_err(|_| StorageError::InvalidGeometry)?;\n"
            "  if bytes_returned < size_of::<STORAGE_DESCRIPTOR_HEADER>() as u32\n"
            "    || descriptor_bytes < bus_end\n"
            "    || descriptor_bytes > MAX_STORAGE_DEVICE_DESCRIPTOR_BYTES\n"
            "  {\n"
            "    return Err(StorageError::InvalidGeometry);\n"
            "  }\n"
            "\n"
            "  let mut descriptor = vec![0u8; descriptor_bytes];\n"
            "  bytes_returned = 0;\n"
            "  // SAFETY: fixed bounded device-property descriptor query.\n"
            "  let descriptor_ok = unsafe {\n"
            "    DeviceIoControl(\n"
            "      handle.as_raw_handle(),\n"
            "      IOCTL_STORAGE_QUERY_PROPERTY,\n"
            "      (&query as *const STORAGE_PROPERTY_QUERY).cast(),\n"
            "      size_of::<STORAGE_PROPERTY_QUERY>() as u32,\n"
            "      descriptor.as_mut_ptr().cast(),\n"
            "      descriptor.len() as u32,\n"
            "      &mut bytes_returned,\n"
            "      null_mut(),\n"
            "    )\n"
            "  };\n"
            "  if descriptor_ok == 0 {\n"
            '    return Err(last_windows_error("device"));\n'
            "  }\n"
            "  parse_storage_bus_type(&descriptor, bytes_returned as usize)\n"
            "}\n"
            "fn launch_elevated_broker() {\n"
            "  let current = std::env::current_exe().unwrap();\n"
            "  let candidate = broker_executable_from_current(&current).unwrap();\n"
            "  let canonical = candidate.canonicalize().unwrap();\n"
            "  let package = current.parent().unwrap().canonicalize().unwrap();\n"
            '  assert_eq!(canonical.file_name().unwrap(), "undelete-master-broker.exe");\n'
            "  assert_eq!(canonical.parent().unwrap(), package);\n"
            '  let verb = "runas";\n'
            "  // SAFETY: fixed sibling broker and runas verb.\n"
            "  unsafe { ShellExecuteExW(null_mut()); }\n"
            "}\n"
        )
        (io_windows / "src" / "windows.rs").write_text(
            windows_source,
            encoding="utf-8",
        )
        (io_windows / "src" / "transport_config.rs").write_text(
            'const BROKER_FILE_NAME: &str = "undelete-master-broker.exe";\n'
            "fn build_pipe_name(pipe_suffix: &str) -> String {\n"
            '  format!(r"\\\\.\\pipe\\UndeleteMaster-v1-{pipe_suffix}")\n'
            "}\n"
            "fn broker_executable_from_current(current_executable: &Path) -> PathBuf {\n"
            "  assert!(current_executable.is_absolute());\n"
            "  current_executable.parent().unwrap().join(BROKER_FILE_NAME)\n"
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
            'windows-sys = { version = "=0.61.2", features = [\n'
            '  "Win32_Foundation",\n'
            '  "Win32_Security",\n'
            '  "Win32_Security_Authorization",\n'
            '  "Win32_Storage_FileSystem",\n'
            '  "Win32_System_IO",\n'
            '  "Win32_System_Ioctl",\n'
            '  "Win32_System_Pipes",\n'
            '  "Win32_System_Threading",\n'
            "] }\n",
            encoding="utf-8",
        )

        (broker / "src" / "lib.rs").write_text(
            "#![forbid(unsafe_code)]\n",
            encoding="utf-8",
        )
        (broker / "Cargo.toml").write_text(
            "[dependencies]\num-broker-protocol = { workspace = true }\n"
            "[target.'cfg(windows)'.dependencies]\n"
            "um-io-windows = { workspace = true }\n",
            encoding="utf-8",
        )
        (broker / "undelete-master-broker.manifest").write_text(
            '<requestedExecutionLevel level="requireAdministrator" '
            'uiAccess="false" />\n',
            encoding="utf-8",
        )
        return temporary, root

    def validate(self, root: Path) -> list[str]:
        errors, _ = validator.validate_repository(root)
        return errors

    def test_desktop_real_only_001_accepts_minimal_real_runtime(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)

        errors, inspected = validator.validate_repository(root)

        self.assertEqual(errors, [])
        self.assertGreaterEqual(inspected, 10)

    def test_desktop_real_only_002_rejects_production_mock_and_simulation(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        source = root / "apps" / "desktop" / "src"
        (source / "api" / "mock.ts").write_text(
            "export class MockDataProvider { start() { "
            "return setInterval(() => Math.random(), 1000); } }\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("forbidden production file" in error for error in errors))
        self.assertTrue(any("simulated runtime primitive" in error for error in errors))

    def test_desktop_real_only_003_rejects_extra_frontend_commands(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        api = root / "apps" / "desktop" / "src" / "api" / "storageDesktop.ts"
        api.write_text(
            api.read_text(encoding="utf-8")
            + 'export const restore = () => invoke("start_restore");\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("unapproved Tauri command" in error for error in errors))

    def test_desktop_real_only_004_rejects_remote_csp_and_desktop_elevation(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        tauri = root / "apps" / "desktop" / "src-tauri"
        config = json.loads((tauri / "tauri.conf.json").read_text(encoding="utf-8"))
        config["app"]["security"]["csp"] += "; connect-src https://example.invalid"
        (tauri / "tauri.conf.json").write_text(json.dumps(config), encoding="utf-8")
        (tauri / "windows-app-manifest.xml").write_text(
            '<requestedExecutionLevel level="requireAdministrator" />\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

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

        errors = self.validate(root)

        self.assertTrue(any("forbidden Tauri dependency" in error for error in errors))
        self.assertTrue(any("unapproved capability" in error for error in errors))

    def test_desktop_real_only_006_rejects_drag_drop_path_ingress(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        config_path = root / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
        config = json.loads(config_path.read_text(encoding="utf-8"))
        config["app"]["windows"][0]["dragDropEnabled"] = True
        config_path.write_text(json.dumps(config), encoding="utf-8")

        errors = self.validate(root)

        self.assertTrue(any("dragDropEnabled" in error for error in errors))

    def test_desktop_real_only_007_rejects_csp_incompatible_styles(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        index = root / "apps" / "desktop" / "index.html"
        index.write_text(
            index.read_text(encoding="utf-8").replace(
                '<link rel="stylesheet" href="/src/styles/global.css" />', ""
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("same-origin production stylesheet" in error for error in errors))

    def test_desktop_real_only_008_rejects_undocumented_unsafe_block(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8") + "fn extra() { unsafe { probe(); } }\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("SAFETY justification" in error for error in errors))

    def test_desktop_real_only_009_rejects_mutating_windows_api(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn mutate_source() { WriteFile(handle, ptr, 1, out, null_mut()); }\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("mutating Windows API" in error for error in errors))

    def test_desktop_real_only_010_rejects_unsafe_outside_windows_boundary(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        tauri_source = root / "apps" / "desktop" / "src-tauri" / "src" / "lib.rs"
        tauri_source.write_text(
            tauri_source.read_text(encoding="utf-8")
            + "fn bypass() { unsafe { core::ptr::read(core::ptr::null()) }; }\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("unsafe Rust is forbidden outside" in error for error in errors))

    def test_desktop_real_only_011_rejects_extra_windows_sys_feature(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "crates" / "io-windows" / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                '"Win32_System_Threading",',
                '"Win32_System_Threading",\n  "Win32_System_Registry",',
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("windows-sys must be pinned" in error for error in errors))

    def test_desktop_real_only_012_requires_all_six_frontend_commands(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        api = root / "apps" / "desktop" / "src" / "api" / "storageDesktop.ts"
        api.write_text(
            api.read_text(encoding="utf-8").replace(
                'invoke("get_candidate_page", { requestId, sessionId });',
                "Promise.resolve();",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("missing Tauri command invocation" in error for error in errors))

    def test_desktop_real_only_013_rejects_removed_image_command(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        api = root / "apps" / "desktop" / "src" / "api" / "storageDesktop.ts"
        old_command = token("select_", "and_", "scan_", "image")
        api.write_text(
            api.read_text(encoding="utf-8")
            + f'export const old = () => invoke("{old_command}");\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("removed image command" in error for error in errors))

    def test_desktop_real_only_014_rejects_create_file_outside_windows_rs(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        tauri_source = root / "apps" / "desktop" / "src-tauri" / "src" / "lib.rs"
        tauri_source.write_text(
            tauri_source.read_text(encoding="utf-8")
            + "fn open() { CreateFileW(path, 0, 0, null(), OPEN_EXISTING, 0, null()); }\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("CreateFile is restricted" in error for error in errors))

    def test_desktop_real_only_015_rejects_device_control_outside_windows_rs(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        tauri_source = root / "apps" / "desktop" / "src-tauri" / "src" / "lib.rs"
        tauri_source.write_text(
            tauri_source.read_text(encoding="utf-8")
            + "fn query() { DeviceIoControl(handle, code, null(), 0, null(), 0, null(), null()); }\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("DeviceIoControl is restricted" in error for error in errors))

    def test_desktop_real_only_016_raw_open_is_exactly_read_open_existing(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        read_access = token("GENERIC_", "READ")
        pipe_write_access = token("GENERIC_", "WRITE")
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                f"wide.as_ptr(), {read_access},",
                f"wide.as_ptr(), {read_access} | {pipe_write_access},",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("raw volume open must use exactly" in error for error in errors))

    def test_desktop_real_only_017_rejects_unapproved_ioctl(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        forbidden_ioctl = token("IOCTL_", "STORAGE_", "EJECT_", "MEDIA")
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "IOCTL_STORAGE_QUERY_PROPERTY", forbidden_ioctl
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("unapproved device-control code" in error for error in errors))

    def test_desktop_real_only_018_broker_must_be_fixed_sibling(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        config = root / "crates" / "io-windows" / "src" / "transport_config.rs"
        config.write_text(
            config.read_text(encoding="utf-8").replace(
                'const BROKER_FILE_NAME: &str = "undelete-master-broker.exe";',
                'const BROKER_FILE_NAME: &str = "arbitrary.exe";',
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("fixed sibling executable" in error for error in errors))

    def test_desktop_real_only_019_broker_requires_administrator_only(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = (
            root
            / "crates"
            / "elevated-broker"
            / "undelete-master-broker.manifest"
        )
        manifest.write_text(
            '<requestedExecutionLevel level="asInvoker" uiAccess="false" />\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("broker manifest must requireAdministrator" in error for error in errors))

    def test_desktop_real_only_020_rejects_non_existing_create_disposition(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL",
                "null(), OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("OPEN_EXISTING" in error for error in errors))

    def test_desktop_real_only_021_rejects_extra_backend_command(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        backend = root / "apps" / "desktop" / "src-tauri" / "src" / "lib.rs"
        backend.write_text(
            backend.read_text(encoding="utf-8").replace(
                "storage::get_candidate_page,",
                "storage::get_candidate_page,\n      storage::start_restore,",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("backend command registration" in error for error in errors))

    def test_desktop_real_only_022_requires_per_monitor_v2_dpi_awareness(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = (
            root
            / "apps"
            / "desktop"
            / "src-tauri"
            / "windows-app-manifest.xml"
        )
        manifest.write_text(
            '<requestedExecutionLevel level="asInvoker" uiAccess="false" />\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("PerMonitorV2 DPI awareness" in error for error in errors))

    def test_desktop_real_only_023_destination_root_open_has_exact_query_authority(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "FILE_SHARE_READ | FILE_SHARE_WRITE,\n"
                "    null(), OPEN_EXISTING,\n"
                "    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,",
                "FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,\n"
                "    null(), OPEN_EXISTING,\n"
                "    FILE_FLAG_BACKUP_SEMANTICS,",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("destination root open must use exact query-only authority" in error for error in errors)
        )

    def test_desktop_real_only_024_destination_volume_open_is_access_zero(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        read_access = token("GENERIC_", "READ")
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "wide.as_ptr(), 0,\n"
                "    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,",
                f"wide.as_ptr(), {read_access},\n"
                "    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("destination volume query must use desired access zero" in error for error in errors)
        )

    def test_desktop_real_only_025_storage_bus_query_is_fixed_and_not_caller_chosen(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "PropertyId: StorageDeviceProperty,",
                "PropertyId: caller_chosen_property,",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("storage bus query must use the fixed device property" in error for error in errors)
        )

    def test_desktop_real_only_026_allows_exactly_six_production_commands(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)

        errors = self.validate(root)

        expected = {
            "list_storage_sources",
            "select_scan_folder",
            "scan_storage_volume",
            "get_candidate_page",
            "query_candidate_page",
            "update_candidate_selection",
        }
        self.assertEqual(validator.ALLOWED_COMMANDS, expected)
        self.assertFalse(any("Tauri command" in error for error in errors))
        self.assertFalse(any("backend command registration" in error for error in errors))

    def test_desktop_real_only_027_rejects_create_file_outside_an_audited_span(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn extra_open(wide: &[u16]) {\n"
            + "  // SAFETY: regression-only extra open.\n"
            + "  unsafe { CreateFileW(wide.as_ptr(), FILE_READ_ATTRIBUTES, "
            + "FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, null(), "
            + "OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | "
            + "FILE_FLAG_OPEN_REPARSE_POINT, null_mut()); }\n"
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("outside an approved audited function" in error for error in errors)
        )

    def test_desktop_real_only_028_rejects_caller_controlled_create_file_path(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "wide.as_ptr(), FILE_READ_ATTRIBUTES,\n"
                "    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,",
                "caller_path.as_ptr(), FILE_READ_ATTRIBUTES,\n"
                "    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("folder attribute open must match" in error for error in errors))

    def test_desktop_real_only_029_rejects_create_file_share_injection(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "wide.as_ptr(), FILE_READ_ATTRIBUTES,\n"
                "    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,",
                "wide.as_ptr(), FILE_READ_ATTRIBUTES,\n"
                "    FILE_SHARE_READ | FILE_SHARE_WRITE,",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("folder attribute open must match" in error for error in errors))

    def test_desktop_real_only_030_rejects_create_file_flag_injection(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "wide.as_ptr(), FILE_READ_ATTRIBUTES,\n"
                "    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,\n"
                "    null(), OPEN_EXISTING,\n"
                "    FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,",
                "wide.as_ptr(), FILE_READ_ATTRIBUTES,\n"
                "    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,\n"
                "    null(), OPEN_EXISTING,\n"
                "    FILE_FLAG_BACKUP_SEMANTICS,",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("folder attribute open must match" in error for error in errors))

    def test_desktop_real_only_031_rejects_decoy_fixed_storage_query_input(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "(&query as *const STORAGE_PROPERTY_QUERY).cast(),",
                "caller_query.cast(),",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("storage bus query call shape must be fixed" in error for error in errors)
        )

    def test_desktop_real_only_032_rejects_mutation_after_a_decoy_fixed_query(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "  };\n"
                "  let mut header = STORAGE_DESCRIPTOR_HEADER::default();",
                "  };\n"
                "  query.PropertyId = caller_chosen_property;\n"
                "  let mut header = STORAGE_DESCRIPTOR_HEADER::default();",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("storage bus query must use the fixed device property" in error for error in errors)
        )

    def test_desktop_real_only_033_rejects_a_second_call_in_an_audited_span(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "  ); }\n"
                "}\n"
                "fn open_destination_root_handle",
                "  ); }\n"
                "  // SAFETY: regression-only duplicate raw open.\n"
                "  unsafe { CreateFileW(wide.as_ptr(), GENERIC_READ, "
                "FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, null(), "
                "OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, null_mut()); }\n"
                "}\n"
                "fn open_destination_root_handle",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(any("raw volume open must use exactly" in error for error in errors))

    def test_desktop_real_only_034_rejects_returned_byte_reassignment(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "  };\n"
                "  if descriptor_ok == 0 {",
                "  };\n"
                "  bytes_returned = descriptor.len() as u32;\n"
                "  if descriptor_ok == 0 {",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("returned-byte flow must remain bounded and fixed" in error for error in errors)
        )

    def test_desktop_real_only_035_rejects_decoy_header_size_dataflow(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "  let descriptor_bytes =\n"
                "    usize::try_from(header.Size).map_err(|_| StorageError::InvalidGeometry)?;",
                "  let _checked_descriptor_bytes = usize::try_from(header.Size)\n"
                "    .map_err(|_| StorageError::InvalidGeometry)?;\n"
                "  let descriptor_bytes = caller_descriptor_bytes;",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("storage bus query implementation must match" in error for error in errors)
        )

    def test_desktop_real_only_036_rejects_decoy_terminal_bus_result(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "  parse_storage_bus_type(&descriptor, bytes_returned as usize)\n",
                "  let _ = parse_storage_bus_type(&descriptor, bytes_returned as usize);\n"
                "  Ok(caller_bus_type)\n",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("storage bus query implementation must match" in error for error in errors)
        )

    def test_desktop_real_only_037_rejects_an_indirect_create_file_binding(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn indirect_open(caller_path: &[u16]) {\n"
            + "  let indirect = CreateFileW;\n"
            + "  // SAFETY: regression-only indirect open.\n"
            + "  unsafe { indirect(caller_path.as_ptr(), FILE_READ_ATTRIBUTES, "
            + "FILE_SHARE_READ, null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, "
            + "null_mut()); }\n"
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_038_rejects_a_typed_create_file_pointer_cast(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn retain_create_file_address() {\n"
            + "  let _indirect: *const () = CreateFileW as *const ();\n"
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_039_rejects_an_imported_create_file_alias(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "use windows_sys::Win32::Storage::FileSystem::"
            + "CreateFileW as ImportedCreateFile;\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_040_rejects_a_macro_create_file_reference(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "macro_rules! retain_symbol { ($symbol:path) => { const _: () = (); }; }\n"
            + "fn macro_reference() { retain_symbol!(CreateFileW); }\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_041_rejects_qualified_create_file_calls(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary_text = boundary.read_text(encoding="utf-8")
        boundary.write_text(
            "use windows_sys::Win32::Storage::FileSystem as file_system;\n"
            + boundary_text.replace("CreateFileW(", "file_system::CreateFileW("),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_042_rejects_create_file_as_an_alias_target(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "use windows_sys::Win32::Storage::FileSystem::CreateFileW;",
                "use crate::alternate::Open as CreateFileW;",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )


if __name__ == "__main__":
    unittest.main()
