from __future__ import annotations

import importlib.util
import json
import os
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

        (root / "Cargo.toml").write_text(
            "[workspace]\n"
            'resolver = "2"\n'
            "\n"
            "[workspace.dependencies]\n"
            'um-core = { path = "crates/core" }\n'
            'um-io-common = { path = "crates/io-common" }\n'
            'um-partition = { path = "crates/partition" }\n'
            'um-fs-common = { path = "crates/fs-common" }\n'
            'um-fs-ntfs = { path = "crates/fs-ntfs" }\n'
            'um-fs-fat = { path = "crates/fs-fat" }\n'
            'um-fs-exfat = { path = "crates/fs-exfat" }\n'
            'um-carving = { path = "crates/carving" }\n'
            'um-restore = { path = "crates/restore" }\n'
            'um-fixture-builder = { path = "crates/fixture-builder" }\n'
            'um-cli = { path = "crates/cli" }\n'
            'um-io-windows = { path = "crates/io-windows" }\n'
            'um-broker-protocol = { path = "crates/broker-protocol" }\n'
            'um-broker-client = { path = "crates/broker-client" }\n'
            'thiserror = "2"\n'
            'serde = { version = "1", features = ["derive"] }\n'
            'serde_json = "1"\n'
            'sha2 = "0.10"\n'
            'crc32fast = "1"\n'
            'hex = "0.4"\n'
            'proptest = "1"\n'
            'tempfile = "3"\n'
            'libc = "0.2"\n'
            'getrandom = "0.3.4"\n'
            'subtle = "2.6"\n'
            'cap-std = "=4.0.2"\n'
            'cap-fs-ext = "=4.0.2"\n',
            encoding="utf-8",
        )
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
            'invoke("update_candidate_selection", { requestId, sessionId });\n'
            'export const destination = (requestId: string, scanId: string) => '
            'invoke("select_restore_destination", { requestId, scanId });\n'
            'export const plan = (requestId: string, scanId: string) => '
            'invoke("create_restore_plan", { requestId, scanId });\n'
            'export const restore = (requestId: string, planId: string) => '
            'invoke("start_restore", { requestId, planId });\n'
            'export const job = (requestId: string, jobId: string) => '
            'invoke("get_restore_job", { requestId, jobId });\n'
            'export const cancel = (requestId: string, jobId: string) => '
            'invoke("cancel_restore", { requestId, jobId });\n'
            'export const open = (requestId: string, jobId: string) => '
            'invoke("open_restore_destination", { requestId, jobId });\n',
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
            "      restore::select_restore_destination,\n"
            "      restore::create_restore_plan,\n"
            "      restore::start_restore,\n"
            "      restore::get_restore_job,\n"
            "      restore::cancel_restore,\n"
            "      restore::open_restore_destination,\n"
            "    ]);\n"
            "}\n",
            encoding="utf-8",
        )
        (tauri / "src" / "restore.rs").write_text(
            "use serde::{Deserialize, Serialize};\n"
            "use crate::storage::DesktopStorageState;\n"
            "struct RestoreCoordinator {}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct DesktopRestoreError { code: &'static str, message: &'static str }\n"
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "enum CollisionPolicyDto {\n"
            "  Rename,\n"
            "}\n"
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "enum PartialFilePolicyDto {\n"
            "  CompleteOnly,\n"
            "  ZeroFillAndMap,\n"
            "}\n"
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "enum RestoreJobStatusDto {\n"
            "  Queued,\n"
            "  Running,\n"
            "  Cancelling,\n"
            "  Completed,\n"
            "  Failed,\n"
            "  Cancelled,\n"
            "}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct DestinationSummaryDto {\n"
            "  schema_version: u32,\n"
            "  destination_id: String,\n"
            "  label: String,\n"
            "  volume_label: String,\n"
            "  file_system: String,\n"
            "  free_bytes: String,\n"
            "  relation: &'static str,\n"
            "}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct RestorePlanSummaryDto {\n"
            "  schema_version: u32,\n"
            "  plan_id: String,\n"
            "  plan_digest: String,\n"
            "  scan_id: String,\n"
            "  destination_id: String,\n"
            "  selection_revision: String,\n"
            "  collision_policy: CollisionPolicyDto,\n"
            "  partial_file_policy: PartialFilePolicyDto,\n"
            "  items_total: String,\n"
            "  files_total: String,\n"
            "  directories_total: String,\n"
            "  logical_bytes: String,\n"
            "  best_effort_items: String,\n"
            "}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct RestoreCurrentItemDto {\n"
            "  ordinal: String,\n"
            "  candidate_id: String,\n"
            "  kind: &'static str,\n"
            "}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct RestoreManifestSummaryDto {\n"
            "  manifest_sha256: String,\n"
            "  completion_status: &'static str,\n"
            "  published_items: String,\n"
            "  partial_items: String,\n"
            "}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct RestoreJobSnapshotDto {\n"
            "  schema_version: u32,\n"
            "  job_id: String,\n"
            "  plan_id: String,\n"
            "  status: RestoreJobStatusDto,\n"
            "  items_total: String,\n"
            "  items_completed: String,\n"
            "  items_failed: String,\n"
            "  items_cancelled: String,\n"
            "  bytes_total: String,\n"
            "  bytes_completed: String,\n"
            "  current_item: Option<RestoreCurrentItemDto>,\n"
            "  warnings: Vec<String>,\n"
            "  manifest: Option<RestoreManifestSummaryDto>,\n"
            "}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct OpenRestoreDestinationDto { schema_version: u32, opened: bool }\n"
            "#[tauri::command]\n"
            "pub(crate) fn select_restore_destination(\n"
            "  app: tauri::AppHandle,\n"
            "  storage: tauri::State<'_, DesktopStorageState>,\n"
            "  restore: tauri::State<'_, RestoreCoordinator>,\n"
            "  request_id: String,\n"
            "  scan_id: String,\n"
            ") -> Result<Option<DestinationSummaryDto>, DesktopRestoreError> {\n"
            "  storage.restore_scan_binding(&scan_id);\n"
            "  app.dialog().blocking_pick_folder();\n"
            "  storage.restore_scan_binding(&scan_id);\n"
            "  restore.admit_destination_binding();\n"
            "  loop {}\n"
            "}\n"
            "#[tauri::command]\n"
            "pub(crate) async fn create_restore_plan(\n"
            "  storage: tauri::State<'_, DesktopStorageState>,\n"
            "  restore: tauri::State<'_, RestoreCoordinator>,\n"
            "  request_id: String,\n"
            "  scan_id: String,\n"
            "  selection_revision: String,\n"
            "  destination_id: String,\n"
            "  collision_policy: CollisionPolicyDto,\n"
            "  partial_file_policy: PartialFilePolicyDto,\n"
            ") -> Result<RestorePlanSummaryDto, DesktopRestoreError> {\n"
            "  tauri::async_runtime::spawn_blocking(move || {\n"
            "    let scan = storage.restore_snapshot(&scan_id, None);\n"
            "    restore.create_restore_plan(&scan);\n"
            "  }).await;\n"
            "  loop {}\n"
            "}\n"
            "#[tauri::command]\n"
            "pub(crate) async fn start_restore(\n"
            "  storage: tauri::State<'_, DesktopStorageState>,\n"
            "  restore: tauri::State<'_, RestoreCoordinator>,\n"
            "  request_id: String,\n"
            "  plan_id: String,\n"
            ") -> Result<RestoreJobSnapshotDto, DesktopRestoreError> {\n"
            "  tauri::async_runtime::spawn_blocking(move || {\n"
            "    restore.plan_binding(&plan_id);\n"
            "    restore.preflight_restore_start(&plan_id);\n"
            "    storage.with_restore_start_selection(&scan_id, revision, |selection| {\n"
            "      restore.commit_prepared_restore_start(selection, prepared);\n"
            "    });\n"
            "    restore.launch_restore(committed);\n"
            "  }).await;\n"
            "  loop {}\n"
            "}\n"
            "#[tauri::command]\n"
            "pub(crate) fn get_restore_job(\n"
            "  restore: tauri::State<'_, RestoreCoordinator>,\n"
            "  request_id: String,\n"
            "  job_id: String,\n"
            ") -> Result<RestoreJobSnapshotDto, DesktopRestoreError> { loop {} }\n"
            "#[tauri::command]\n"
            "pub(crate) fn cancel_restore(\n"
            "  restore: tauri::State<'_, RestoreCoordinator>,\n"
            "  request_id: String,\n"
            "  job_id: String,\n"
            ") -> Result<RestoreJobSnapshotDto, DesktopRestoreError> { loop {} }\n"
            "#[tauri::command]\n"
            "pub(crate) fn open_restore_destination(\n"
            "  restore: tauri::State<'_, RestoreCoordinator>,\n"
            "  request_id: String,\n"
            "  job_id: String,\n"
            ") -> Result<OpenRestoreDestinationDto, DesktopRestoreError> { loop {} }\n",
            encoding="utf-8",
        )
        (tauri / "Cargo.toml").write_text(
            "[build-dependencies]\n"
            'tauri-build = { version = "=2.6.3", features = [] }\n'
            "\n"
            "[dependencies]\n"
            "serde = { workspace = true }\n"
            "serde_json = { workspace = true }\n"
            "sha2 = { workspace = true }\n"
            "hex = { workspace = true }\n"
            "getrandom = { workspace = true }\n"
            "cap-std = { workspace = true }\n"
            "cap-fs-ext = { workspace = true }\n"
            'tauri = { version = "=2.11.5", features = [] }\n'
            'tauri-plugin-dialog = { version = "=2.7.2" }\n'
            "um-broker-client = { workspace = true }\n"
            "um-cli = { workspace = true }\n"
            "um-core = { workspace = true }\n"
            "um-fs-common = { workspace = true }\n"
            "um-fs-ntfs = { workspace = true }\n"
            "um-io-windows = { workspace = true }\n"
            "um-restore = { workspace = true }\n"
            "\n"
            "[dev-dependencies]\n"
            "crc32fast = { workspace = true }\n"
            "tempfile = { workspace = true }\n"
            "um-fixture-builder = { workspace = true }\n"
            "um-io-common = { workspace = true }\n",
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
            "mod transport_config;\n"
            "pub fn open_retained_directory_in_shell(retained_directory: std::fs::File) "
            "-> Result<(), StorageError> {\n"
            "  windows::open_retained_directory_in_shell(retained_directory)\n"
            "}\n",
            encoding="utf-8",
        )
        read_access = token("GENERIC_", "READ")
        pipe_write_access = token("GENERIC_", "WRITE")
        windows_source = (
            "use windows_sys::Win32::Foundation::{"
            f"{read_access}, {pipe_write_access}"
            "};\n"
            "use windows_sys::Win32::Storage::FileSystem::CreateFileW;\n"
            "use windows_sys::Win32::System::Com::{"
            "CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, "
            "COINIT_DISABLE_OLE1DDE};\n"
            '#[link(name = "shell32")]\n'
            "// SAFETY: declaration-only exact ShellExecuteExW ABI.\n"
            'unsafe extern "system" {\n'
            "  fn ShellExecuteExW(execution: *mut ShellExecuteInfoW) -> i32;\n"
            "}\n"
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
            "const SEE_MASK_NOASYNC: u32 = 0x0000_0100;\n"
            "const SHELL_COM_STA_FLAGS: u32 = "
            "(COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32;\n"
            "struct RetainedDirectoryShellRequest { target: OsString }\n"
            "impl RetainedDirectoryShellRequest {\n"
            "  fn new(target: OsString) -> Self { Self { target } }\n"
            "  fn target(&self) -> &OsStr { &self.target }\n"
            "  fn verb(&self) -> &'static str { \"explore\" }\n"
            "  fn mask(&self) -> u32 { SEE_MASK_NOASYNC }\n"
            "}\n"
            "fn initialize_retained_directory_shell_com(flags: u32) -> i32 {\n"
            "  // SAFETY: fixed COM initialization flags on the dedicated shell thread.\n"
            "  unsafe { CoInitializeEx(null(), flags) }\n"
            "}\n"
            "fn execute_fixed_retained_directory_explore("
            "request: &RetainedDirectoryShellRequest) {\n"
            "  let verb = wide_string(OsStr::new(request.verb()));\n"
            "  let final_path = wide_string(request.target());\n"
            "  let mut execution = ShellExecuteInfoW {\n"
            "    mask: request.mask(),\n"
            "    verb: verb.as_ptr(),\n"
            "    file: final_path.as_ptr(),\n"
            "    parameters: null(),\n"
            "    directory: null(),\n"
            "  };\n"
            "  // SAFETY: fixed explore request for a retained handle-derived target.\n"
            "  unsafe { ShellExecuteExW(&mut execution); }\n"
            "}\n"
            "fn uninitialize_retained_directory_shell_com() {\n"
            "  // SAFETY: balances a successful COM initialization on this thread.\n"
            "  unsafe { CoUninitialize(); }\n"
            "}\n"
            "trait RetainedDirectoryShellPlatform {\n"
            "  fn initialize_com(&mut self, flags: u32) -> i32;\n"
            "  fn execute(&mut self, request: &RetainedDirectoryShellRequest) "
            "-> Result<(), StorageError>;\n"
            "  fn uninitialize_com(&mut self);\n"
            "}\n"
            "struct WindowsRetainedDirectoryShellPlatform;\n"
            "impl RetainedDirectoryShellPlatform for WindowsRetainedDirectoryShellPlatform {\n"
            "  fn initialize_com(&mut self, flags: u32) -> i32 {\n"
            "    initialize_retained_directory_shell_com(flags)\n"
            "  }\n"
            "  fn execute(&mut self, request: &RetainedDirectoryShellRequest) "
            "-> Result<(), StorageError> {\n"
            "    execute_fixed_retained_directory_explore(request); Ok(())\n"
            "  }\n"
            "  fn uninitialize_com(&mut self) {\n"
            "    uninitialize_retained_directory_shell_com();\n"
            "  }\n"
            "}\n"
            "struct InitializedShellComApartment<'a, P: RetainedDirectoryShellPlatform> {\n"
            "  platform: &'a mut P,\n"
            "}\n"
            "impl<P: RetainedDirectoryShellPlatform> Drop "
            "for InitializedShellComApartment<'_, P> {\n"
            "  fn drop(&mut self) { self.platform.uninitialize_com(); }\n"
            "}\n"
            "fn execute_retained_directory_shell_request"
            "<P: RetainedDirectoryShellPlatform>(\n"
            "  request: &RetainedDirectoryShellRequest,\n"
            "  platform: &mut P,\n"
            ") -> Result<(), StorageError> {\n"
            "  let hresult = platform.initialize_com(SHELL_COM_STA_FLAGS);\n"
            "  if hresult < 0 { return Err(StorageError::ComInitializationFailed { hresult }); }\n"
            "  let apartment = InitializedShellComApartment { platform };\n"
            "  apartment.platform.execute(request)\n"
            "}\n"
            "fn run_retained_directory_shell_thread(retained_directory: File) {\n"
            "  let _root = query_destination_root_information(&retained_directory);\n"
            "  let target = query_final_guid_path(&retained_directory);\n"
            "  let request = RetainedDirectoryShellRequest::new(target.into());\n"
            "  let mut platform = WindowsRetainedDirectoryShellPlatform;\n"
            "  let result = execute_retained_directory_shell_request(&request, &mut platform);\n"
            "  drop(retained_directory);\n"
            "  let _ = result;\n"
            "}\n"
            "fn open_retained_directory_in_shell(retained_directory: File) {\n"
            "  std::thread::Builder::new()\n"
            "    .name(\"undelete-master-shell-open\".to_owned())\n"
            "    .spawn(move || run_retained_directory_shell_thread(retained_directory))\n"
            "    .unwrap()\n"
            "    .join()\n"
            "    .unwrap();\n"
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
            "hex = { workspace = true }\n"
            "serde = { workspace = true }\n"
            "sha2 = { workspace = true }\n"
            "thiserror = { workspace = true }\n"
            "um-core = { workspace = true }\n"
            "\n"
            "[target.'cfg(windows)'.dependencies]\n"
            'windows-sys = { version = "=0.61.2", features = [\n'
            '  "Win32_Foundation",\n'
            '  "Win32_Security",\n'
            '  "Win32_Security_Authorization",\n'
            '  "Win32_Storage_FileSystem",\n'
            '  "Win32_System_Com",\n'
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
            + 'export const destructive = () => invoke("erase_scan_source");\n',
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
                "storage::get_candidate_page,\n      storage::pause_scan,",
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

    def test_desktop_real_only_026_allows_exactly_twelve_production_commands(self) -> None:
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
            "select_restore_destination",
            "create_restore_plan",
            "start_restore",
            "get_restore_job",
            "cancel_restore",
            "open_restore_destination",
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

    def test_desktop_real_only_043_rejects_qualified_raw_create_file_calls(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary_text = boundary.read_text(encoding="utf-8")
        boundary.write_text(
            "use windows_sys::Win32::Storage::FileSystem as file_system;\n"
            + boundary_text.replace(
                "CreateFileW(",
                "file_system::r#CreateFileW(",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_044_rejects_bare_raw_create_file_calls(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "CreateFileW(",
                "r#CreateFileW(",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_045_rejects_a_raw_alias_target(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "use windows_sys::Win32::Storage::FileSystem::CreateFileW;",
                "use crate::alternate::Open as r#CreateFileW;",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_046_rejects_comment_spaced_raw_qualification(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary_text = boundary.read_text(encoding="utf-8")
        boundary.write_text(
            "use windows_sys::Win32::Storage::FileSystem as file_system;\n"
            + boundary_text.replace(
                "CreateFileW(",
                "file_system /* qualifier */ :: /* raw target */ "
                "r#CreateFileW(",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_047_rejects_comment_spaced_raw_field_calls(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "CreateFileW(",
                "holder /* field */ . /* raw target */ r#CreateFileW(",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_048_rejects_a_raw_canonical_import(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "use windows_sys::Win32::Storage::FileSystem::CreateFileW;",
                "use windows_sys::Win32::Storage::FileSystem::r#CreateFileW;",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_049_rejects_link_name_create_file_rebinding(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + '#[link(name = "kernel32")]\n'
            + "// SAFETY: regression-only alternate FFI declaration.\n"
            + 'unsafe extern "system" {\n'
            + '  #[link_name = "CreateFileW"]\n'
            + "  fn alternate_open(\n"
            + "    name: *const u16, access: u32, share: u32,\n"
            + "    security: *const c_void, disposition: u32, flags: u32,\n"
            + "    template: HANDLE,\n"
            + "  ) -> HANDLE;\n"
            + "}\n"
            + "fn rebound_open(caller_path: &[u16]) {\n"
            + "  // SAFETY: regression-only rebound call.\n"
            + "  unsafe { alternate_open(\n"
            + "    caller_path.as_ptr(), FILE_READ_ATTRIBUTES, 0, null(),\n"
            + "    OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, null_mut(),\n"
            + "  ); }\n"
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("extern and link-name inventory must match" in error for error in errors)
        )

    def test_desktop_real_only_050_rejects_dynamic_symbol_resolution(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn retain_dynamic_resolver() {\n"
            + "  let _resolver = GetProcAddress;\n"
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("dynamic symbol resolution is forbidden" in error for error in errors)
        )

    def test_desktop_real_only_051_rejects_a_macro_wrapped_import(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "use windows_sys::Win32::Storage::FileSystem::CreateFileW;",
                "macro_rules! import_symbol {\n"
                "  ($item:item) => { $item };\n"
                "}\n"
                "import_symbol! {\n"
                "  use windows_sys::Win32::Storage::FileSystem::CreateFileW;\n"
                "}",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("canonical unaliased" in error for error in errors)
        )

    def test_desktop_real_only_052_rejects_a_public_create_file_import(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "use windows_sys::Win32::Storage::FileSystem::CreateFileW;",
                "pub use windows_sys::Win32::Storage::FileSystem::CreateFileW;",
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("canonical unaliased" in error for error in errors)
        )

    def test_desktop_real_only_053_rejects_macro_wrapped_direct_calls(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary_text = boundary.read_text(encoding="utf-8")
        wrapped_calls = boundary_text.replace(
            "unsafe { CreateFileW(",
            "unsafe { passthrough!(CreateFileW(",
        ).replace(
            "  ); }\n",
            "  )); }\n",
            5,
        )
        boundary.write_text(
            "macro_rules! passthrough { ($value:expr) => { $value }; }\n"
            + wrapped_calls,
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("CreateFile symbol reference must be" in error for error in errors)
        )

    def test_desktop_real_only_054_rejects_an_aliased_dynamic_loader_dependency(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "crates" / "io-windows" / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                "thiserror = { workspace = true }\n",
                "thiserror = { workspace = true }\n"
                'resolver = { package = "libloading", version = "0.8" }\n',
            ),
            encoding="utf-8",
        )
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn resolve_hidden_symbol() {\n"
            + "  // SAFETY: regression-only dynamic loader alias.\n"
            + "  let _symbol = unsafe {\n"
            + '    let library = resolver::Library::new("kernel32.dll");\n'
            + '    library.get::<*const ()>(b"CreateFileW")\n'
            + "  };\n"
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("general dependencies must match" in error for error in errors)
        )

    def test_desktop_real_only_055_rejects_proc_macro_symbol_synthesis(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "crates" / "io-windows" / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                "thiserror = { workspace = true }\n",
                "thiserror = { workspace = true }\n"
                'paste = "1"\n',
            ),
            encoding="utf-8",
        )
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn synthesized_open(wide: &[u16]) {\n"
            + "  // SAFETY: regression-only synthesized API and access names.\n"
            + "  unsafe { paste::paste! {\n"
            + "    let _ = [<Create File W>](\n"
            + "      wide.as_ptr(), [<GENERIC_ WRITE>], FILE_SHARE_READ,\n"
            + "      null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, null_mut(),\n"
            + "    );\n"
            + "  } }\n"
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("general dependencies must match" in error for error in errors)
        )

    def test_desktop_real_only_056_rejects_tauri_dynamic_loader_dependency(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                'tauri-plugin-dialog = { version = "=2.7.2" }\n',
                'tauri-plugin-dialog = { version = "=2.7.2" }\n'
                'dlopen2 = "=0.8.2"\n',
            ),
            encoding="utf-8",
        )
        tauri_lib = root / "apps" / "desktop" / "src-tauri" / "src" / "lib.rs"
        tauri_lib.write_text(
            tauri_lib.read_text(encoding="utf-8")
            + "fn load_runtime() {\n"
            + '  let _ = dlopen2::raw::Library::open("kernel32.dll");\n'
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("dependency inventory must match" in error for error in errors)
        )

    def test_desktop_real_only_057_rejects_transitive_proc_macro_reexport(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        core = root / "crates" / "core"
        (core / "src").mkdir(parents=True)
        (core / "Cargo.toml").write_text(
            "[package]\n"
            'name = "um-core"\n'
            'version = "0.1.0"\n'
            'edition = "2021"\n'
            "\n"
            "[dependencies]\n"
            'paste = "1"\n',
            encoding="utf-8",
        )
        (core / "src" / "lib.rs").write_text(
            "#![forbid(unsafe_code)]\n"
            "pub use paste::paste as join_tokens;\n",
            encoding="utf-8",
        )
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn transitive_synthesized_open(wide: &[u16]) {\n"
            + "  // SAFETY: regression-only transitive synthesized open.\n"
            + "  unsafe { um_core::join_tokens! {\n"
            + "    let _ = [<Create File W>](\n"
            + "      wide.as_ptr(), [<GENERIC_ WRITE>], FILE_SHARE_READ,\n"
            + "      null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, null_mut(),\n"
            + "    );\n"
            + "  } }\n"
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "unreviewed dependency or macro expansion surface" in error
                for error in errors
            )
        )

    def test_desktop_real_only_058_rejects_unreviewed_boundary_macros(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8")
            + "fn invoke_transitive_macro() {\n"
            + "  um_core::join_tokens! { let _ = 0; }\n"
            + "}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "unreviewed dependency or macro expansion surface" in error
                for error in errors
            )
        )

    def test_desktop_real_only_059_rejects_reviewed_macro_name_rebinding(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            "use um_core::join_tokens as matches;\n"
            + boundary.read_text(encoding="utf-8")
            + "fn invoke_rebound_macro() { matches!(true); }\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "unreviewed dependency or macro expansion surface" in error
                for error in errors
            )
        )

    def test_desktop_real_only_060_rejects_workspace_dependency_expansion(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8")
            + 'paste = "1"\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "unreviewed dependency or macro expansion surface" in error
                for error in errors
            )
        )

    def test_desktop_real_only_061_rejects_cargo_patch_substitution(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8")
            + "\n"
            + "[patch.crates-io]\n"
            + 'serde = { path = "crates/core" }\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "unreviewed dependency or macro expansion surface" in error
                for error in errors
            )
        )

    def test_desktop_real_only_062_rejects_repository_cargo_source_config(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        cargo_config = root / ".cargo" / "config.toml"
        cargo_config.parent.mkdir()
        cargo_config.write_text(
            "[source.crates-io]\n"
            'replace-with = "vendored-sources"\n'
            "\n"
            "[source.vendored-sources]\n"
            'directory = "vendor"\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "unreviewed dependency or macro expansion surface" in error
                for error in errors
            )
        )

    def test_desktop_real_only_063_rejects_nested_member_cargo_source_config(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        cargo_config = (
            root
            / "apps"
            / "desktop"
            / "src-tauri"
            / ".cargo"
            / "config.toml"
        )
        cargo_config.parent.mkdir()
        cargo_config.write_text(
            "[source.crates-io]\n"
            'replace-with = "vendored"\n'
            "\n"
            "[source.vendored]\n"
            'directory = "vendor"\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "unreviewed dependency or macro expansion surface" in error
                for error in errors
            )
        )

    def test_desktop_real_only_064_rejects_nested_legacy_cargo_source_config(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        cargo_config = root / "crates" / "core" / ".cargo" / "config"
        cargo_config.parent.mkdir(parents=True)
        cargo_config.write_text(
            "[source.crates-io]\n"
            'replace-with = "vendored"\n'
            "\n"
            "[source.vendored]\n"
            'directory = "vendor"\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "unreviewed dependency or macro expansion surface" in error
                for error in errors
            )
        )

    def test_desktop_real_only_065_orders_nested_cargo_configs_deterministically(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        relative_configs = [
            Path("crates/zeta/.cargo/config.toml"),
            Path("apps/desktop/src-tauri/.cargo/config"),
        ]
        for relative_config in relative_configs:
            cargo_config = root / relative_config
            cargo_config.parent.mkdir(parents=True, exist_ok=True)
            cargo_config.write_text("[net]\noffline = true\n", encoding="utf-8")

        errors = self.validate(root)
        config_errors = [
            error
            for error in errors
            if "repository Cargo source configuration is forbidden" in error
        ]

        self.assertEqual(
            config_errors,
            [
                "apps/desktop/src-tauri/.cargo/config: unreviewed dependency "
                "or macro expansion surface; repository Cargo source "
                "configuration is forbidden",
                "crates/zeta/.cargo/config.toml: unreviewed dependency or "
                "macro expansion surface; repository Cargo source "
                "configuration is forbidden",
            ],
        )

    def test_desktop_real_only_066_prunes_generated_and_vendor_cargo_configs(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        for relative_config in (
            Path("build/generated/.cargo/config"),
            Path("target/debug/.cargo/config.toml"),
            Path("vendor/dependency/.cargo/config.toml"),
        ):
            cargo_config = root / relative_config
            cargo_config.parent.mkdir(parents=True)
            cargo_config.write_text("[net]\noffline = true\n", encoding="utf-8")

        errors = self.validate(root)

        self.assertEqual(errors, [])

    def test_desktop_real_only_067_scans_explicit_member_inside_pruned_tree(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                'resolver = "2"\n',
                'resolver = "2"\n'
                'members = ["vendor/first-party-member"]\n',
            ),
            encoding="utf-8",
        )
        cargo_config = (
            root
            / "vendor"
            / "first-party-member"
            / ".cargo"
            / "config.toml"
        )
        cargo_config.parent.mkdir(parents=True)
        cargo_config.write_text("[net]\noffline = true\n", encoding="utf-8")

        errors = self.validate(root)

        self.assertTrue(
            any(
                error.startswith(
                    "vendor/first-party-member/.cargo/config.toml:"
                )
                for error in errors
            )
        )

    def test_desktop_real_only_068_normalizes_in_repo_workspace_member(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                'resolver = "2"\n',
                'resolver = "2"\n'
                'members = ["crates/../vendor/first-party-member"]\n',
            ),
            encoding="utf-8",
        )
        member = root / "vendor" / "first-party-member"
        member.mkdir(parents=True)
        (member / "Cargo.toml").write_text(
            "[package]\n"
            'name = "first-party-member"\n'
            'version = "0.1.0"\n'
            'edition = "2021"\n',
            encoding="utf-8",
        )
        cargo_config = member / ".cargo" / "config.toml"
        cargo_config.parent.mkdir()
        cargo_config.write_text("[net]\noffline = true\n", encoding="utf-8")

        errors = self.validate(root)

        self.assertTrue(
            any(
                error.startswith(
                    "vendor/first-party-member/.cargo/config.toml:"
                )
                for error in errors
            )
        )

    def test_desktop_real_only_069_rejects_workspace_member_escape(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                'resolver = "2"\n',
                'resolver = "2"\n'
                'members = ["../outside-member"]\n',
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "workspace member path must remain within the repository"
                in error
                for error in errors
            )
        )

    def test_desktop_real_only_070_rejects_workspace_member_symlink_loop(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                'resolver = "2"\n',
                'resolver = "2"\n'
                'members = ["vendor/loop-member"]\n',
            ),
            encoding="utf-8",
        )
        loop = root / "vendor" / "loop-member"
        loop.parent.mkdir()
        os.symlink(loop, loop, target_is_directory=True)

        errors = self.validate(root)

        self.assertTrue(
            any(
                "workspace member path must remain within the repository"
                in error
                for error in errors
            )
        )

    def test_desktop_real_only_071_accepts_exact_restore_capability_dependencies(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "crates" / "restore"
        restore.mkdir(parents=True)
        (restore / "Cargo.toml").write_text(
            "[dependencies]\n"
            "cap-std = { workspace = true }\n"
            "cap-fs-ext = { workspace = true }\n"
            "serde = { workspace = true }\n"
            "serde_json = { workspace = true }\n"
            "sha2 = { workspace = true }\n"
            "thiserror = { workspace = true }\n"
            "um-core = { workspace = true }\n"
            "\n"
            "[target.'cfg(windows)'.dev-dependencies]\n"
            'junction = "=2.0.0"\n',
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertFalse(
            any("unreviewed dependency or macro expansion" in error for error in errors),
            errors,
        )

    def test_desktop_real_only_072_rejects_restore_capability_version_drift(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        manifest = root / "Cargo.toml"
        manifest.write_text(
            manifest.read_text(encoding="utf-8").replace(
                'cap-std = "=4.0.2"\n',
                'cap-std = "=4.0.3"\n',
            ),
            encoding="utf-8",
        )
        restore = root / "crates" / "restore"
        restore.mkdir(parents=True)
        (restore / "Cargo.toml").write_text(
            "[dependencies]\n"
            "cap-std = { workspace = true }\n"
            "cap-fs-ext = { workspace = true }\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("workspace dependencies must match" in error for error in errors)
        )

    def test_desktop_real_only_073_rejects_shell_verb_drift(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                'fn verb(&self) -> &\'static str { "explore" }',
                'fn verb(&self) -> &\'static str { "open" }',
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("retained-directory shell request must be fixed" in error for error in errors)
        )

    def test_desktop_real_only_074_rejects_shell_noasync_drift(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "fn mask(&self) -> u32 { SEE_MASK_NOASYNC }",
                "fn mask(&self) -> u32 { 0 }",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("retained-directory shell request must be fixed" in error for error in errors)
        )

    def test_desktop_real_only_075_rejects_shell_parameter_input(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "parameters: null(),",
                "parameters: caller_parameters,",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("retained-directory shell request must be fixed" in error for error in errors)
        )

    def test_desktop_real_only_076_rejects_extra_shell_execute_callsite(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "  unsafe { ShellExecuteExW(&mut execution); }\n"
                "}\n"
                "fn uninitialize_retained_directory_shell_com",
                "  unsafe { ShellExecuteExW(&mut execution); }\n"
                "  // SAFETY: regression-only duplicate shell dispatch.\n"
                "  unsafe { ShellExecuteExW(&mut execution); }\n"
                "}\n"
                "fn uninitialize_retained_directory_shell_com",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("ShellExecuteExW callsites must remain closed" in error for error in errors)
        )

    def test_desktop_real_only_077_rejects_shell_com_flag_drift(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "platform.initialize_com(SHELL_COM_STA_FLAGS)",
                "platform.initialize_com(0)",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("retained-directory shell COM lifecycle must be fixed" in error for error in errors)
        )

    def test_desktop_real_only_078_rejects_missing_shell_com_cleanup(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "  fn drop(&mut self) { self.platform.uninitialize_com(); }\n",
                "  fn drop(&mut self) { }\n",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("retained-directory shell COM lifecycle must be fixed" in error for error in errors)
        )

    def test_desktop_real_only_079_rejects_inline_shell_dispatch(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "  std::thread::Builder::new()\n",
                "  run_retained_directory_shell_thread(retained_directory);\n"
                "  std::thread::Builder::new()\n",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("retained-directory shell dispatch must use one dedicated thread" in error for error in errors)
        )

    def test_desktop_real_only_080_rejects_path_bearing_shell_api(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        crate_root = root / "crates" / "io-windows" / "src" / "lib.rs"
        crate_root.write_text(
            crate_root.read_text(encoding="utf-8").replace(
                "open_retained_directory_in_shell(retained_directory: std::fs::File)",
                "open_retained_directory_in_shell(path: &Path, retained_directory: std::fs::File)",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("retained-directory shell API must remain handle-only" in error for error in errors)
        )

    def test_desktop_real_only_081_rejects_duplicate_runas_literal(self) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        boundary = root / "crates" / "io-windows" / "src" / "windows.rs"
        boundary.write_text(
            boundary.read_text(encoding="utf-8").replace(
                "fn launch_elevated_broker() {\n",
                'const UNREVIEWED_SHELL_VERB: &str = "runas";\n'
                "fn launch_elevated_broker() {\n",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "elevated launch must preserve one fixed runas verb" in error
                for error in errors
            )
        )

    def test_desktop_real_only_082_rejects_path_bearing_restore_command_signature(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        restore.write_text(
            restore.read_text(encoding="utf-8").replace(
                "  scan_id: String,\n"
                ") -> Result<Option<DestinationSummaryDto>, DesktopRestoreError>",
                "  scan_id: String,\n"
                "  destination_path: String,\n"
                ") -> Result<Option<DestinationSummaryDto>, DesktopRestoreError>",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("restore command signature must remain exact" in error for error in errors)
        )

    def test_desktop_real_only_083_rejects_native_identity_in_restore_dto(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        restore.write_text(
            restore.read_text(encoding="utf-8").replace(
                "  relation: &'static str,\n",
                "  relation: &'static str,\n"
                "  physical_disk_number: u32,\n",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("restore DTO fields must remain exact" in error for error in errors)
        )

    def test_desktop_real_only_084_rejects_every_sensitive_restore_dto_field(
        self,
    ) -> None:
        mutations = (
            "  source_path: String,\n",
            "  destination_path: String,\n",
            "  destination_root: String,\n",
            "  volume_id: String,\n",
            "  physical_disk_number: u32,\n",
            "  extents: Vec<(u64, u64)>,\n",
            "  source_offset: u64,\n",
            "  destination_handle: usize,\n",
            "  desired_access: u32,\n",
            "  control_code: u32,\n",
            "  executable: String,\n",
            "  recovered_bytes: Vec<u8>,\n",
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation.strip()):
                temporary, root = self.make_repo()
                self.addCleanup(temporary.cleanup)
                restore = (
                    root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
                )
                restore.write_text(
                    restore.read_text(encoding="utf-8").replace(
                        "  relation: &'static str,\n",
                        "  relation: &'static str,\n" + mutation,
                        1,
                    ),
                    encoding="utf-8",
                )

                errors = self.validate(root)

                self.assertTrue(
                    any(
                        "restore DTO fields must remain exact" in error
                        for error in errors
                    ),
                    errors,
                )

    def test_desktop_real_only_085_rejects_drift_in_each_restore_command(
        self,
    ) -> None:
        mutations = (
            (
                "  scan_id: String,\n"
                ") -> Result<Option<DestinationSummaryDto>, DesktopRestoreError>",
                "  scan_id: String,\n"
                "  source_path: String,\n"
                ") -> Result<Option<DestinationSummaryDto>, DesktopRestoreError>",
            ),
            (
                "  partial_file_policy: PartialFilePolicyDto,\n"
                ") -> Result<RestorePlanSummaryDto, DesktopRestoreError>",
                "  partial_file_policy: PartialFilePolicyDto,\n"
                "  destination_path: String,\n"
                ") -> Result<RestorePlanSummaryDto, DesktopRestoreError>",
            ),
            (
                "  plan_id: String,\n"
                ") -> Result<RestoreJobSnapshotDto, DesktopRestoreError>",
                "  plan_id: String,\n"
                "  volume_id: String,\n"
                ") -> Result<RestoreJobSnapshotDto, DesktopRestoreError>",
            ),
            (
                "pub(crate) fn get_restore_job(\n"
                "  restore: tauri::State<'_, RestoreCoordinator>,\n"
                "  request_id: String,\n"
                "  job_id: String,\n",
                "pub(crate) fn get_restore_job(\n"
                "  restore: tauri::State<'_, RestoreCoordinator>,\n"
                "  request_id: String,\n"
                "  job_id: String,\n"
                "  physical_disk_number: u32,\n",
            ),
            (
                "pub(crate) fn cancel_restore(\n"
                "  restore: tauri::State<'_, RestoreCoordinator>,\n"
                "  request_id: String,\n"
                "  job_id: String,\n",
                "pub(crate) fn cancel_restore(\n"
                "  restore: tauri::State<'_, RestoreCoordinator>,\n"
                "  request_id: String,\n"
                "  job_id: String,\n"
                "  source_offset: u64,\n",
            ),
            (
                "pub(crate) fn open_restore_destination(\n"
                "  restore: tauri::State<'_, RestoreCoordinator>,\n"
                "  request_id: String,\n"
                "  job_id: String,\n",
                "pub(crate) fn open_restore_destination(\n"
                "  restore: tauri::State<'_, RestoreCoordinator>,\n"
                "  request_id: String,\n"
                "  job_id: String,\n"
                "  recovered_bytes: Vec<u8>,\n",
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=new.splitlines()[-1].strip()):
                temporary, root = self.make_repo()
                self.addCleanup(temporary.cleanup)
                restore = (
                    root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
                )
                original = restore.read_text(encoding="utf-8")
                mutated = original.replace(old, new, 1)
                self.assertNotEqual(mutated, original)
                restore.write_text(mutated, encoding="utf-8")

                errors = self.validate(root)

                self.assertTrue(
                    any(
                        "restore command signature must remain exact" in error
                        for error in errors
                    ),
                    errors,
                )

    def test_desktop_real_only_086_accepts_equivalent_restore_formatting(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        text = restore.read_text(encoding="utf-8")
        text = text.replace("String", "std::string::String")
        text = text.replace("Option<", "std::option::Option <")
        text = text.replace("Vec<", "std::vec::Vec <")
        text = text.replace("Result<", "std::result::Result <")
        text = text.replace("tauri::State", "::tauri :: State")
        text = text.replace("tauri::AppHandle", "::tauri :: AppHandle")
        text = text.replace(
            "struct DestinationSummaryDto {\n"
            "  schema_version: u32,\n",
            "struct\nDestinationSummaryDto\n{\n"
            "  schema_version : u32,\n",
            1,
        )
        text = text.replace(
            "pub(crate) fn get_restore_job(",
            "pub ( crate )\nfn get_restore_job (",
            1,
        )
        text = "use std::{collections::HashMap};\n" + text
        restore.write_text(text, encoding="utf-8")

        errors = self.validate(root)

        self.assertFalse(
            any(
                "restore command signature must remain exact" in error
                or "restore DTO fields must remain exact" in error
                for error in errors
            ),
            errors,
        )

    def test_desktop_real_only_087_rejects_restore_policy_variant_drift(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        restore.write_text(
            restore.read_text(encoding="utf-8").replace(
                "enum CollisionPolicyDto {\n"
                "  Rename,\n",
                "enum CollisionPolicyDto {\n"
                "  Rename,\n"
                "  Overwrite,\n",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("restore DTO variants must remain exact" in error for error in errors)
        )

    def test_desktop_real_only_088_rejects_macro_hidden_restore_dto_fixture(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        original = restore.read_text(encoding="utf-8")
        text = original
        text = text.replace(
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct DestinationSummaryDto {\n",
            "macro_rules! hidden_restore_dto {\n"
            "  () => {\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct DestinationSummaryDto {\n",
            1,
        )
        text = text.replace(
            "  relation: &'static str,\n"
            "}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct RestorePlanSummaryDto {\n",
            "  relation: &'static str,\n"
            "}\n"
            "  }\n"
            "}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct RestorePlanSummaryDto {\n",
            1,
        )
        self.assertNotEqual(
            text,
            original,
        )
        self.assertIn(
            "  }\n"
            "}\n"
            "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
            '#[serde(rename_all = "camelCase")]\n'
            "struct RestorePlanSummaryDto",
            text,
        )
        restore.write_text(text, encoding="utf-8")

        errors = self.validate(root)

        self.assertTrue(
            any("restore DTO fields must remain exact" in error for error in errors)
        )

    def test_desktop_real_only_089_rejects_restore_type_alias_rebinding(
        self,
    ) -> None:
        mutations = (
            "type String = std::path::PathBuf;\n",
            "type Option<T> = std::path::PathBuf;\n",
            "type Vec<T> = std::path::PathBuf;\n",
            "type Result<T, E> = std::path::PathBuf;\n",
            "type u32 = std::path::PathBuf;\n",
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation.strip()):
                temporary, root = self.make_repo()
                self.addCleanup(temporary.cleanup)
                restore = (
                    root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
                )
                restore.write_text(
                    mutation + restore.read_text(encoding="utf-8"),
                    encoding="utf-8",
                )

                errors = self.validate(root)

                self.assertTrue(
                    any(
                        "restore boundary types must not be rebound" in error
                        for error in errors
                    ),
                    errors,
                )

    def test_desktop_real_only_090_rejects_restore_import_alias_rebinding(
        self,
    ) -> None:
        mutations = (
            "use std::path::PathBuf as String;\n",
            "use crate::storage::OtherState as DesktopStorageState;\n",
            "use crate::evil::u32;\n",
            "use crate::evil::{Deserialize, Serialize};\n",
            "use crate::evil::*;\n",
            "mod serde {}\n",
            "extern crate evil_runtime as tauri;\n",
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation.strip()):
                temporary, root = self.make_repo()
                self.addCleanup(temporary.cleanup)
                restore = (
                    root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
                )
                restore.write_text(
                    mutation + restore.read_text(encoding="utf-8"),
                    encoding="utf-8",
                )

                errors = self.validate(root)

                self.assertTrue(
                    any(
                        "restore boundary types must not be rebound" in error
                        for error in errors
                    ),
                    errors,
                )

    def test_desktop_real_only_091_rejects_restore_serde_case_drift(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        restore.write_text(
            restore.read_text(encoding="utf-8").replace(
                '#[serde(rename_all = "camelCase")]\n'
                "struct DestinationSummaryDto",
                '#[serde(rename_all = "snake_case")]\n'
                "struct DestinationSummaryDto",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "restore serialization contract must remain exact" in error
                for error in errors
            )
        )

    def test_desktop_real_only_092_rejects_restore_derive_drift(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        restore.write_text(
            restore.read_text(encoding="utf-8").replace(
                "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
                '#[serde(rename_all = "camelCase")]\n'
                "struct DesktopRestoreError",
                "#[derive(Debug, Clone, PartialEq, Eq)]\n"
                '#[serde(rename_all = "camelCase")]\n'
                "struct DesktopRestoreError",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "restore serialization contract must remain exact" in error
                for error in errors
            )
        )

    def test_desktop_real_only_093_rejects_manual_restore_serialization(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        restore.write_text(
            restore.read_text(encoding="utf-8")
            + "\nimpl serde::Serialize for DestinationSummaryDto {}\n",
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "restore serialization contract must remain exact" in error
                for error in errors
            )
        )

    def test_desktop_real_only_094_rejects_qualified_type_root_rebinding(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        text = restore.read_text(encoding="utf-8").replace(
            "  label: String,\n",
            "  label: std::string::String,\n",
            1,
        )
        restore.write_text(
            "mod std { mod string { type String = crate::SensitivePath; } }\n"
            + text,
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "restore boundary types must not be rebound" in error
                for error in errors
            )
        )

    def test_desktop_real_only_095_rejects_restore_execution_boundary_drift(
        self,
    ) -> None:
        mutations = (
            (
                "storage.restore_scan_binding(&scan_id);",
                "storage.restore_snapshot(&scan_id, None);",
            ),
            (
                "tauri::async_runtime::spawn_blocking(move || {",
                "run_inline(move || {",
            ),
            (
                "tauri::async_runtime::spawn_blocking(move || {",
                "evil::spawn_blocking(move || {",
            ),
            (
                "  tauri::async_runtime::spawn_blocking(move || {\n"
                "    let scan = storage.restore_snapshot(&scan_id, None);\n"
                "    restore.create_restore_plan(&scan);\n"
                "  }).await;\n",
                "  let scan = storage.restore_snapshot(&scan_id, None);\n"
                "  tauri::async_runtime::spawn_blocking(move || {\n"
                "    restore.create_restore_plan(&scan);\n"
                "  }).await;\n",
            ),
            (
                "    storage.with_restore_start_selection(",
                "    storage.restore_snapshot(&scan_id, None);\n"
                "    storage.with_restore_start_selection(",
            ),
            (
                "storage.with_restore_start_selection(",
                "storage.copy_restore_start_selection(",
            ),
        )
        for old, new in mutations:
            with self.subTest(mutation=new.splitlines()[0].strip()):
                temporary, root = self.make_repo()
                self.addCleanup(temporary.cleanup)
                restore = (
                    root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
                )
                original = restore.read_text(encoding="utf-8")
                mutated = original.replace(old, new, 1)
                self.assertNotEqual(mutated, original)
                restore.write_text(mutated, encoding="utf-8")

                errors = self.validate(root)

                self.assertTrue(
                    any(
                        "restore command execution boundary must remain fixed"
                        in error
                        for error in errors
                    ),
                    errors,
                )

    def test_desktop_real_only_096_rejects_generic_restore_dto_defaults(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        restore.write_text(
            restore.read_text(encoding="utf-8").replace(
                "struct DestinationSummaryDto {",
                "struct DestinationSummaryDto<String = std::path::PathBuf> {",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any("restore DTO fields must remain exact" in error for error in errors)
        )

    def test_desktop_real_only_097_rejects_qualified_untrusted_derive(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        restore.write_text(
            restore.read_text(encoding="utf-8").replace(
                "#[derive(Debug, Clone, PartialEq, Eq, Serialize)]\n"
                '#[serde(rename_all = "camelCase")]\n'
                "struct DestinationSummaryDto",
                "#[derive(Debug, Clone, PartialEq, Eq, evil::Serialize)]\n"
                '#[serde(rename_all = "camelCase")]\n'
                "struct DestinationSummaryDto",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "restore serialization contract must remain exact" in error
                for error in errors
            )
        )

    def test_desktop_real_only_098_rejects_storage_state_import_drift(
        self,
    ) -> None:
        temporary, root = self.make_repo()
        self.addCleanup(temporary.cleanup)
        restore = root / "apps" / "desktop" / "src-tauri" / "src" / "restore.rs"
        restore.write_text(
            restore.read_text(encoding="utf-8").replace(
                "use crate::storage::DesktopStorageState;",
                "use crate::evil::storage::DesktopStorageState;",
                1,
            ),
            encoding="utf-8",
        )

        errors = self.validate(root)

        self.assertTrue(
            any(
                "restore boundary types must not be rebound" in error
                for error in errors
            )
        )


if __name__ == "__main__":
    unittest.main()
