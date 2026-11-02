//! Data types shared by the manager, the UI and the CLI. Serialized as camelCase JSON.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InstanceConfig {
    pub id: String,
    pub folder: PathBuf,
    pub name: Option<String>,
    pub chrome: bool,
    pub permission_mode: Option<String>,
    pub spawn: Option<String>,
    pub capacity: Option<u32>,
    pub create_session_in_dir: Option<bool>,
    pub session_name_prefix: Option<String>,
    pub extra_args: Vec<String>,
    pub auto_trust: bool,
    pub auto_start: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
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
