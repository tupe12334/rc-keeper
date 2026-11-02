//! Data types shared by the manager, the UI and the CLI. Serialized as camelCase JSON.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Field order is the order of the keys in `instances/<id>.json`. Keep it stable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceConfig {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub folder: PathBuf,
    pub auto_start: bool,
    pub auto_trust: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity: Option<u32>,
    pub chrome: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_session_in_dir: Option<bool>,
    pub extra_args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_name_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spawn: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_bin: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub settings: Settings,
    pub instances: Vec<InstanceConfig>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceView {
    pub config: InstanceConfig,
    pub status: InstanceStatus,
}
