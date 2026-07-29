//! Unelevated Tauri shell for the real, read-only image scanner.

#![forbid(unsafe_code)]

mod scan;

/// Starts the single-window desktop application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan::select_and_scan_image])
        .run(tauri::generate_context!())
        .expect("failed to run Undelete Master desktop");
}

#[cfg(test)]
mod tests {
    #[test]
    fn desktop_command_inventory_001_registers_only_the_real_image_command() {
        let source = include_str!("lib.rs");
        let registration = source
            .lines()
            .find(|line| line.contains(".invoke_handler("))
            .expect("invoke handler registration");
        assert!(registration.contains("scan::select_and_scan_image"));
        assert!(!registration.contains(','));
    }
}
