use serde_json::{json, Value};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager as _};

use crate::daemon::{self, Daemon};

pub const TRAY_ID: &str = "main";
pub const OFFLINE_TEXT: &str = "rc-keeper: daemon not running";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", "Open", true, None::<&str>)?,
            &MenuItem::with_id(app, "start_all", "Start all", true, None::<&str>)?,
            &MenuItem::with_id(app, "stop_all", "Stop all", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "stop_daemon", "Stop daemon", true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("rc-keeper")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| on_menu(app, event.id.0.as_str()))
        .build(app)?;
    Ok(())
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    let daemon = app.state::<Daemon>();
    // ponytail: menu actions ignore IPC errors; the tooltip shows the daemon state on the next tick.
    match id {
        "open" => show_main(app),
        "start_all" => {
            let _ = daemon::call(&daemon, "start_all", json!({}));
        }
        "stop_all" => {
            let _ = daemon::call(&daemon, "stop_all", json!({}));
        }
        "stop_daemon" => {
            let _ = daemon::stop(&daemon);
        }
        // Quitting the GUI leaves the daemon running.
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Tooltip text from an `list_instances` result, e.g. "rc-keeper: 2 running, 1 restarting".
pub fn summary(views: &Value) -> String {
    let (mut running, mut restarting, mut attention) = (0, 0, 0);
    for view in views.as_array().into_iter().flatten() {
        match view["status"]["state"].as_str() {
            Some("running") => running += 1,
            Some("restarting") => restarting += 1,
            Some("needs-trust" | "folder-busy" | "error") => attention += 1,
            _ => {}
        }
    }
    format_summary(running, restarting, attention)
}

fn format_summary(running: usize, restarting: usize, attention: usize) -> String {
    let mut text = format!("rc-keeper: {running} running, {restarting} restarting");
    if attention > 0 {
        text.push_str(&format!(", {attention} need attention"));
    }
    text
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{format_summary, summary};

    #[test]
    fn summary_text_lists_running_and_restarting() {
        assert_eq!(
            format_summary(2, 1, 0),
            "rc-keeper: 2 running, 1 restarting"
        );
    }

    #[test]
    fn summary_text_adds_attention_count_when_needed() {
        assert_eq!(
            format_summary(0, 0, 2),
            "rc-keeper: 0 running, 0 restarting, 2 need attention"
        );
    }

    #[test]
    fn summary_counts_states_from_ipc_json() {
        let views = json!([
            { "status": { "state": "running" } },
            { "status": { "state": "running" } },
            { "status": { "state": "restarting" } },
            { "status": { "state": "stopped" } },
            { "status": { "state": "needs-trust" } }
        ]);
        assert_eq!(
            summary(&views),
            "rc-keeper: 2 running, 1 restarting, 1 need attention"
        );
    }
}
