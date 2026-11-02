use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::daemon::{self, Daemon};
use crate::settings::{self, SettingsDto};

type CmdResult = Result<Value, String>;

#[tauri::command]
pub fn list_instances(daemon: State<'_, Daemon>) -> CmdResult {
    daemon::call(&daemon, "list_instances", json!({}))
}

#[tauri::command]
pub fn add_instance(daemon: State<'_, Daemon>, cfg: Value) -> CmdResult {
    daemon::call(&daemon, "add_instance", json!({ "cfg": cfg }))
}

#[tauri::command]
pub fn update_instance(daemon: State<'_, Daemon>, cfg: Value) -> CmdResult {
    daemon::call(&daemon, "update_instance", json!({ "cfg": cfg }))
}

#[tauri::command]
pub fn remove_instance(daemon: State<'_, Daemon>, id: String) -> CmdResult {
    daemon::call(&daemon, "remove_instance", json!({ "id": id }))
}

#[tauri::command]
pub fn start_instance(daemon: State<'_, Daemon>, id: String) -> CmdResult {
    daemon::call(&daemon, "start_instance", json!({ "id": id }))
}

#[tauri::command]
pub fn stop_instance(daemon: State<'_, Daemon>, id: String) -> CmdResult {
    daemon::call(&daemon, "stop_instance", json!({ "id": id }))
}

#[tauri::command]
pub fn restart_instance(daemon: State<'_, Daemon>, id: String) -> CmdResult {
    daemon::call(&daemon, "restart_instance", json!({ "id": id }))
}

#[tauri::command]
pub fn get_logs(daemon: State<'_, Daemon>, id: String) -> CmdResult {
    daemon::call(&daemon, "get_logs", json!({ "id": id }))
}

#[tauri::command]
pub fn start_all(daemon: State<'_, Daemon>) -> CmdResult {
    daemon::call(&daemon, "start_all", json!({}))
}

#[tauri::command]
pub fn stop_all(daemon: State<'_, Daemon>) -> CmdResult {
    daemon::call(&daemon, "stop_all", json!({}))
}

#[tauri::command]
pub fn get_settings(app: AppHandle, daemon: State<'_, Daemon>) -> Result<SettingsDto, String> {
    settings::get(&app, &daemon)
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    daemon: State<'_, Daemon>,
    claude_bin: Option<PathBuf>,
    autostart: bool,
) -> Result<(), String> {
    settings::set(
        &app,
        &daemon,
        SettingsDto {
            claude_bin,
            autostart,
        },
    )
}

#[tauri::command]
pub fn detect_claude_bin(daemon: State<'_, Daemon>) -> CmdResult {
    daemon::call(&daemon, "detect_claude_bin", json!({}))
}

#[tauri::command]
pub fn daemon_info() -> Value {
    daemon::info()
}

#[tauri::command]
pub fn stop_daemon(daemon: State<'_, Daemon>) -> Result<(), String> {
    daemon::stop(&daemon)
}
