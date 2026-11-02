# Module contract (rc-keeper)

Layout (each folder has one owner):
- `crates/rc-core/` — Rust library: config, args, output parser, PTY supervisor, manager. No Tauri.
- `src-tauri/` — Tauri 2 shell: commands, tray, autostart, single-instance, `--status` CLI. Thin calls into rc-core.
- `ui/` — React + TS + Vite + Tailwind + shadcn. Builds to `ui/dist`.
- `.github/`, `install.sh`, `install.ps1`, `LICENSE`, README body — CI, release, install.

## rc-core public API (serde camelCase)
```rust
pub struct InstanceConfig { pub id: String, pub folder: PathBuf, pub name: Option<String>, pub chrome: bool,
  pub permission_mode: Option<String>, pub spawn: Option<String>, pub capacity: Option<u32>,
  pub create_session_in_dir: Option<bool>, pub session_name_prefix: Option<String>,
  pub extra_args: Vec<String>, pub auto_trust: bool, pub auto_start: bool }
pub struct Settings { pub claude_bin: Option<PathBuf> }
pub struct AppConfig { pub settings: Settings, pub instances: Vec<InstanceConfig> }
#[serde(rename_all = "kebab-case")]
pub enum State { Stopped, Starting, Running, Restarting, FolderBusy, NeedsTrust, Error }
pub struct InstanceStatus { pub id: String, pub state: State, pub pid: Option<u32>, pub sessions: u32,
  pub uptime_secs: Option<u64>, pub restarts: u32, pub next_retry_secs: Option<u64>,
  pub last_error: Option<String>, pub session_urls: Vec<String> }
pub struct InstanceView { pub config: InstanceConfig, pub status: InstanceStatus }

pub fn build_args(cfg: &InstanceConfig) -> Vec<String>;      // ["remote-control", ...flags]
pub fn find_claude_bin(settings: &Settings) -> Option<PathBuf>;

impl Manager {               // Send + Sync, cheap to clone (Arc inside)
  pub fn load(config_dir: PathBuf) -> Manager;        // reads config.json, starts auto_start instances
  pub fn list(&self) -> Vec<InstanceView>;
  pub fn add(&self, cfg: InstanceConfig) -> Result<String, String>;   // empty id → generated; returns id
  pub fn update(&self, cfg: InstanceConfig) -> Result<(), String>;    // restarts if running
  pub fn remove(&self, id: &str) -> Result<(), String>;               // stops first
  pub fn start(&self, id: &str) -> Result<(), String>;
  pub fn stop(&self, id: &str) -> Result<(), String>;
  pub fn restart(&self, id: &str) -> Result<(), String>;
  pub fn logs(&self, id: &str) -> Vec<String>;                        // last 500 lines, ANSI stripped
  pub fn settings(&self) -> Settings;  pub fn set_settings(&self, s: Settings) -> Result<(), String>;
  pub fn start_all(&self); pub fn stop_all(&self); pub fn shutdown(&self);  // shutdown: kill children, no orphans
}
```
The Manager writes `<config_dir>/status.json` (Vec<InstanceView>) every 2 s.

## Tauri commands (names the UI invokes; args camelCase)
`list_instances() -> InstanceView[]`, `add_instance({cfg}) -> string`, `update_instance({cfg})`,
`remove_instance({id})`, `start_instance({id})`, `stop_instance({id})`, `restart_instance({id})`,
`get_logs({id}) -> string[]`, `start_all()`, `stop_all()`, `get_settings() -> {claudeBin, autostart}`,
`set_settings({claudeBin, autostart})`, `detect_claude_bin() -> string | null`.
Event `instances` emitted every 1 s with `InstanceView[]`. Folder picker: `@tauri-apps/plugin-dialog` in the UI.
CLI: `rc-keeper --status` prints `status.json` from the config dir and exits 0 (1 if absent).
Config dir: Tauri `app_config_dir()` (identifier `io.github.tupe12334.rckeeper`).
