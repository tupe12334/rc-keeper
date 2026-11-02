//! Wire types for the docs and for dispatch.
//!
//! The `rc-core` data types have no `JsonSchema` derive, and `rc-core` is owned elsewhere.
//! Each shadow below mirrors one real type. `check_*` fns destructure the real type
//! with no `..`, so a new or renamed field breaks the build here.
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One folder that runs `claude remote-control`.
#[derive(Default, JsonSchema)]
#[schemars(remote = "rc_core::InstanceConfig")]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceConfig {
    /// Stable id. Empty on `add_instance`: the daemon generates one.
    pub id: String,
    /// Folder that `claude remote-control` runs in.
    pub folder: PathBuf,
    pub name: Option<String>,
    pub chrome: bool,
    /// One of `acceptEdits`, `auto`, `bypassPermissions`, `default`, `dontAsk`, `plan`.
    pub permission_mode: Option<String>,
    /// One of `same-dir`, `worktree`, `session`.
    pub spawn: Option<String>,
    pub capacity: Option<u32>,
    pub create_session_in_dir: Option<bool>,
    pub session_name_prefix: Option<String>,
    /// Extra arguments for `claude remote-control`.
    pub extra_args: Vec<String>,
    pub auto_trust: bool,
    pub auto_start: bool,
}

/// Daemon settings. Shared by all instances.
#[derive(Default, JsonSchema)]
#[schemars(remote = "rc_core::Settings")]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Path to the `claude` binary. `null` means detect it.
    pub claude_bin: Option<PathBuf>,
}

#[derive(Default, JsonSchema)]
#[schemars(remote = "rc_core::State")]
#[serde(rename_all = "kebab-case")]
pub enum State {
    #[default]
    Stopped,
    Starting,
    Running,
    Restarting,
    FolderBusy,
    NeedsTrust,
    Error,
}

#[derive(JsonSchema)]
#[schemars(remote = "rc_core::InstanceStatus")]
#[serde(rename_all = "camelCase")]
pub struct InstanceStatus {
    pub id: String,
    pub state: State,
    pub pid: Option<u32>,
    pub sessions: u32,
    pub uptime_secs: Option<u64>,
    pub restarts: u32,
    pub next_retry_secs: Option<u64>,
    pub last_error: Option<String>,
    pub session_urls: Vec<String>,
}

#[derive(JsonSchema)]
#[schemars(remote = "rc_core::InstanceView")]
#[serde(rename_all = "camelCase")]
pub struct InstanceView {
    pub config: InstanceConfig,
    pub status: InstanceStatus,
}

/// Compile-time checks. Each `check_*` destructures the real `rc_core` type with no `..`,
/// then builds the shadow from those fields. A missing, renamed or retyped field fails the build.
#[allow(dead_code)]
fn check_instance_config(v: rc_core::InstanceConfig) -> InstanceConfig {
    let rc_core::InstanceConfig {
        id,
        folder,
        name,
        chrome,
        permission_mode,
        spawn,
        capacity,
        create_session_in_dir,
        session_name_prefix,
        extra_args,
        auto_trust,
        auto_start,
    } = v;
    InstanceConfig {
        id,
        folder,
        name,
        chrome,
        permission_mode,
        spawn,
        capacity,
        create_session_in_dir,
        session_name_prefix,
        extra_args,
        auto_trust,
        auto_start,
    }
}

#[allow(dead_code)]
fn check_settings(v: rc_core::Settings) -> Settings {
    let rc_core::Settings { claude_bin } = v;
    Settings { claude_bin }
}

#[allow(dead_code)]
fn check_state(v: rc_core::State) -> State {
    match v {
        rc_core::State::Stopped => State::Stopped,
        rc_core::State::Starting => State::Starting,
        rc_core::State::Running => State::Running,
        rc_core::State::Restarting => State::Restarting,
        rc_core::State::FolderBusy => State::FolderBusy,
        rc_core::State::NeedsTrust => State::NeedsTrust,
        rc_core::State::Error => State::Error,
    }
}

#[allow(dead_code)]
fn check_instance_status(v: rc_core::InstanceStatus) -> InstanceStatus {
    let rc_core::InstanceStatus {
        id,
        state,
        pid,
        sessions,
        uptime_secs,
        restarts,
        next_retry_secs,
        last_error,
        session_urls,
    } = v;
    InstanceStatus {
        id,
        state: check_state(state),
        pid,
        sessions,
        uptime_secs,
        restarts,
        next_retry_secs,
        last_error,
        session_urls,
    }
}

#[allow(dead_code)]
fn check_instance_view(v: rc_core::InstanceView) -> InstanceView {
    let rc_core::InstanceView { config, status } = v;
    InstanceView {
        config: check_instance_config(config),
        status: check_instance_status(status),
    }
}

/// Params of `get_logs`, `remove_instance`, `start_instance`, `stop_instance`, `restart_instance`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdParams {
    pub id: String,
}

/// Params of `add_instance` and `update_instance`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CfgParams {
    #[schemars(with = "InstanceConfig")]
    pub cfg: rc_core::InstanceConfig,
}

/// Params of `set_settings`. Absent or `null` clears the claude binary path.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SetSettingsParams {
    pub claude_bin: Option<PathBuf>,
}

/// Result of `ping`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PingResult {
    pub version: String,
    pub pid: u32,
}

/// Result of `get_settings`. `autostart` is always `null`: the GUI owns login autostart.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetSettingsResult {
    pub claude_bin: Option<PathBuf>,
    pub autostart: (),
}
