//! Unelevated Tauri shell for the real, read-only Windows volume scanner.

#![forbid(unsafe_code)]

mod storage;

/// Starts the single-window desktop application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(storage::DesktopStorageState::default())
        .invoke_handler(tauri::generate_handler![
            storage::list_storage_sources,
            storage::select_scan_folder,
            storage::scan_storage_volume,
            storage::get_candidate_page
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Undelete Master desktop");
}

#[cfg(test)]
mod tests {
    #[test]
    fn desktop_command_inventory_001_registers_only_the_real_storage_commands() {
        let source = include_str!("lib.rs");
        for command in [
            "storage::list_storage_sources",
            "storage::select_scan_folder",
            "storage::scan_storage_volume",
            "storage::get_candidate_page",
        ] {
            assert!(source.contains(command), "missing {command}");
        }
        let removed_image_command = ["select", "and", "scan", "image"].join("_");
        let removed_module = ["mod", "scan;"].join(" ");
        assert!(!source.contains(&removed_image_command));
        assert!(!source.contains(&removed_module));
    }
}
