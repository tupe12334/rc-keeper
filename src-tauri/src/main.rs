#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod daemon;
mod events;
mod settings;
mod tray;
mod update;

use tauri::{Manager as _, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

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
            // Connects lazily on the first command, so startup never waits on the daemon.
            app.manage(daemon::Daemon::new());
            tray::build(app.handle())?;
            // ponytail: a failed Dock change only leaves the icon visible; startup goes on.
            let _ = settings::apply_saved(app.handle());
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
            commands::force_start,
            commands::preview_force_start,
            commands::get_logs,
            commands::start_all,
            commands::stop_all,
            commands::get_settings,
            commands::set_settings,
            commands::detect_claude_bin,
            commands::daemon_info,
            commands::start_daemon,
            commands::stop_daemon,
            commands::update_info,
            commands::start_update,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run rc-keeper");
}
