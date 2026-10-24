//! Layer 2 — nexus-shell. A deliberately thin Tauri v2 host:
//! window + plugins + the IPC commands defined in `commands.rs`.
//! Everything that touches the disk, the network or a JVM lives in `nexus-core`.

pub mod commands;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin: a second launch just focuses the running window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::save_config,
            commands::detect_java,
            commands::list_java,
            commands::install_java,
            commands::open_data_folder,
            commands::open_instance_folder,
            commands::open_url,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::ms_device_start,
            commands::ms_check_client,
            commands::ms_device_poll,
            commands::ms_logout,
            commands::set_active_account,
            commands::launch_instance,
            commands::search_modrinth,
            commands::search_curseforge,
            commands::install_project,
            commands::install_custom_pack,
            commands::suggested_export_path,
            commands::export_instance,
            commands::import_instance,
            commands::list_content,
            commands::mod_content_meta,
            commands::toggle_content,
            commands::delete_content,
            commands::read_log,
            commands::list_crashes,
            commands::read_crash,
            commands::check_updates,
            commands::update_pack,
            commands::list_loader_versions
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nexus Launcher");
}
