use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt as _;

use crate::daemon::{self, Daemon};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub claude_bin: Option<PathBuf>,
    pub autostart: bool,
}

/// Claude binary path comes from the daemon; autostart is an OS setting owned by the GUI.
pub fn get(app: &AppHandle, daemon: &Daemon) -> Result<SettingsDto, String> {
    let settings = daemon::call(daemon, "get_settings", json!({}))?;
    Ok(SettingsDto {
        claude_bin: serde_json::from_value(settings["claudeBin"].clone())
            .map_err(|e| e.to_string())?,
        autostart: app.autolaunch().is_enabled().map_err(|e| e.to_string())?,
    })
}

pub fn set(app: &AppHandle, daemon: &Daemon, dto: SettingsDto) -> Result<(), String> {
    // ponytail: autostart goes first. If the daemon write then fails, autostart stays changed and the user retries.
    let launcher = app.autolaunch();
    let result = if dto.autostart {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|e| e.to_string())?;
    daemon::call(
        daemon,
        "set_settings",
        json!({ "claudeBin": dto.claude_bin }),
    )?;
    Ok(())
}
