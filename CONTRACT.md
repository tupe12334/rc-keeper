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

## v2 addendum: headless daemon + CLI (servers over SSH)
The GUI no longer owns the Manager. One **daemon** per user owns it; the **CLI** and the **GUI** are clients.

New crates (one owner each):
- `crates/rc-ipc/` — protocol + client + server over a local socket (`interprocess` crate: Unix socket on macOS/Linux at `<config_dir>/rc-keeper.sock`, named pipe `rc-keeper-<user>` on Windows). JSON lines: request `{"id":n,"method":"<name>","params":{...}}` → response `{"id":n,"result":...}` or `{"id":n,"error":"..."}`. Methods = the Tauri command names above (`list_instances`, `add_instance`, …, `get_logs`, `start_all`, `stop_all`, `get_settings`, `set_settings` (claudeBin only), `detect_claude_bin`) plus `ping` → `{"version":"x.y.z"}` and `shutdown`. `pub fn config_dir() -> PathBuf` lives here (dirs crate + identifier `io.github.tupe12334.rckeeper`; macOS `~/Library/Application Support/<id>`, Linux `$XDG_CONFIG_HOME|~/.config/<id>`, Windows `%APPDATA%\<id>`). `pub struct Client { pub fn connect() -> Result<Client>; pub fn call(&mut self, method, params: Value) -> Result<Value> }`, `pub fn serve(manager: rc_core::Manager) -> !`.
- `crates/rc-cli/` — binary **`rc-keeper`** (no GUI deps, builds on headless Linux). Subcommands (clap):
  `daemon` (foreground; refuses if another daemon answers ping) · `status [--json]` · `list [--json]` · `add <folder> [--name] [--chrome|--no-chrome] [--permission-mode] [--spawn] [--capacity] [--create-session-in-dir|--no-create-session-in-dir] [--session-name-prefix] [--auto-trust/--no-auto-trust] [--no-auto-start] [-- extra args]` · `edit <id|folder> …same flags` · `remove <id|folder>` · `start|stop|restart <id|folder|--all>` · `logs <id|folder> [-f]` · `config get|set claude-bin <path>` · `service install|uninstall|status` (systemd user unit + `loginctl enable-linger` hint on Linux; launchd agent `io.github.tupe12334.rckeeper.daemon` with KeepAlive on macOS; Task Scheduler at logon on Windows) · `version`.
  Any client command auto-starts the daemon (detached) if ping fails. Human output = aligned table; `--json` = machine output. Exit codes: 0 ok, 1 error, 2 daemon unreachable.
- GUI (`src-tauri`) binary name **`rc-keeper-gui`**; bundles `rc-keeper` as a Tauri sidecar (`externalBin`), on launch connects via `rc_ipc::Client`, spawning the sidecar `rc-keeper daemon` if ping fails. Tauri commands proxy to the IPC client; the `instances` event polls `list_instances` every 1 s. Quitting the GUI does NOT stop the daemon (tray menu offers "Stop daemon"). `--status` moves to the CLI (`rc-keeper status --json`).
- Releases ship: GUI bundles per OS **and** standalone CLI archives `rc-keeper-<target>.tar.gz` / `.zip` (x86_64 + aarch64 Linux, universal macOS, x86_64 Windows). `install.sh --cli` (auto when no `$DISPLAY`/`$WAYLAND_DISPLAY` on Linux) installs only the CLI to `~/.local/bin`.
- Extra GUI-only Tauri commands: `daemon_info() -> {version: string, running: boolean}` (IPC `ping`) and `stop_daemon()` (IPC `shutdown`).
- Allowed values: permissionMode ∈ `acceptEdits|auto|bypassPermissions|default|dontAsk|plan`; spawn ∈ `same-dir|worktree|session` (from `claude remote-control --help`, v2.1.316).
- CLI daemon group: `rc-keeper daemon` / `daemon run` (foreground), `daemon stop` (IPC `shutdown`; exit 0 also when not running), `daemon status [--json]` (exit 0 running, 2 not running).
- Every crate `Cargo.toml` and `src-tauri/Cargo.toml` uses a literal `version = "x.y.z"` (no `version.workspace`), so release-please can bump it.
- Service restart semantics: the OS service restarts the daemon only on failure (systemd `Restart=on-failure`, launchd `KeepAlive.SuccessfulExit=false`), so `rc-keeper daemon stop` stays stopped. Socket: config dir 0700, socket 0600.
