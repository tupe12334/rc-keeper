use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager as _};
use tauri_plugin_autostart::ManagerExt as _;

use crate::daemon::{self, Daemon};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub claude_bin: Option<PathBuf>,
    pub autostart: bool,
    /// macOS only: hide the Dock icon and keep only the menu bar icon.
    pub hide_dock: bool,
}

/// GUI-only preferences. The daemon and the CLI never read them.
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct GuiPrefs {
    hide_dock: bool,
}

fn prefs_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("gui.json"))
}

// ponytail: a missing or broken file means defaults; the next save rewrites it.
fn load_prefs(app: &AppHandle) -> GuiPrefs {
    prefs_path(app)
        .ok()
        .and_then(|p| fs::read(p).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_prefs(app: &AppHandle, prefs: &GuiPrefs) -> Result<(), String> {
    let path = prefs_path(app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(prefs).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn apply_dock(app: &AppHandle, hide: bool) -> Result<(), String> {
    app.set_dock_visibility(!hide).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn apply_dock(_app: &AppHandle, _hide: bool) -> Result<(), String> {
    Ok(())
}

/// Applies saved GUI preferences at startup.
pub fn apply_saved(app: &AppHandle) -> Result<(), String> {
    apply_dock(app, load_prefs(app).hide_dock)
}

/// Claude binary path comes from the daemon; autostart and the Dock icon are OS settings owned by the GUI.
pub fn get(app: &AppHandle, daemon: &Daemon) -> Result<SettingsDto, String> {
    let settings = daemon::call(daemon, "get_settings", json!({}))?;
    Ok(SettingsDto {
        claude_bin: serde_json::from_value(settings["claudeBin"].clone())
            .map_err(|e| e.to_string())?,
        autostart: app.autolaunch().is_enabled().map_err(|e| e.to_string())?,
        hide_dock: load_prefs(app).hide_dock,
    })
}

pub fn set(app: &AppHandle, daemon: &Daemon, dto: SettingsDto) -> Result<(), String> {
    // ponytail: GUI settings go first. If the daemon write then fails, they stay changed and the user retries.
    let launcher = app.autolaunch();
    let result = if dto.autostart {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|e| e.to_string())?;
    save_prefs(
        app,
        &GuiPrefs {
            hide_dock: dto.hide_dock,
        },
    )?;
    apply_dock(app, dto.hide_dock)?;
    daemon::call(
        daemon,
        "set_settings",
        json!({ "claudeBin": dto.claude_bin }),
    )?;
    Ok(())
}
