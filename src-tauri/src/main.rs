#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod daemon;
mod events;
mod settings;
mod tray;

use std::sync::Mutex;

use tauri::{Manager as _, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use daemon::Daemon;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // A failed connect is not fatal: commands retry and report the error to the UI.
            let client = daemon::connect_or_spawn().ok();
            app.manage(Daemon(Mutex::new(client)));
            tray::build(app.handle())?;
            events::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Close hides the window; the tray keeps the app alive.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_instances,
            commands::add_instance,
            commands::update_instance,
            commands::remove_instance,
            commands::start_instance,
            commands::stop_instance,
            commands::restart_instance,
            commands::get_logs,
            commands::start_all,
            commands::stop_all,
            commands::get_settings,
            commands::set_settings,
            commands::detect_claude_bin,
            commands::daemon_info,
            commands::stop_daemon,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run rc-keeper");
}
