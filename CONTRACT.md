# Module contract (rc-keeper)

Layout (each folder has one owner):
- `crates/rc-core/` — Rust library: config, args, output parser, PTY supervisor, manager. No Tauri.
- `src-tauri/` — Tauri 2 shell: commands, tray, autostart, single-instance, `--status` CLI. Thin calls into rc-core.
- `ui/` — React + TS + Vite + Tailwind + shadcn. Builds to `ui/dist`.
- `.github/`, `install.sh`, `install.ps1`, `LICENSE`, README body — CI, release, install.

## rc-core public API
Signatures and doc comments are generated. Run `cargo doc -p rc-core --no-deps` and open `target/doc/rc_core/index.html`.
- Serde shapes use camelCase. The exception is `State`, which uses kebab-case.
- The Manager writes `<config_dir>/status.json` (a `Vec<InstanceView>`) every 2 s.
- The IPC wire types are in the [IPC reference](docs/reference/ipc.md).

## Tauri commands (names the UI invokes; args camelCase)
Tauri commands mirror the IPC method names. Signatures: see the [IPC reference](docs/reference/ipc.md).
Event `instances` emitted every 1 s with `InstanceView[]`. Folder picker: `@tauri-apps/plugin-dialog` in the UI.
CLI: `rc-keeper --status` prints `status.json` from the config dir and exits 0 (1 if absent).
Config dir: Tauri `app_config_dir()` (identifier `io.github.tupe12334.rckeeper`).

## v2 addendum: headless daemon + CLI (servers over SSH)
The GUI no longer owns the Manager. One **daemon** per user owns it; the **CLI** and the **GUI** are clients.

New crates (one owner each):
- `crates/rc-ipc/` — protocol + client + server over a local socket (`interprocess` crate: Unix socket on macOS/Linux at `<config_dir>/rc-keeper.sock`, named pipe `rc-keeper-<user>` on Windows). JSON lines: request `{"id":n,"method":"<name>","params":{...}}` → response `{"id":n,"result":...}` or `{"id":n,"error":"..."}`. Methods: see the [IPC reference](docs/reference/ipc.md). `pub fn config_dir() -> PathBuf` lives here (dirs crate + identifier `io.github.tupe12334.rckeeper`; macOS `~/Library/Application Support/<id>`, Linux `$XDG_CONFIG_HOME|~/.config/<id>`, Windows `%APPDATA%\<id>`). `pub struct Client { pub fn connect() -> Result<Client>; pub fn call(&mut self, method, params: Value) -> Result<Value> }`, `pub fn serve(manager: rc_core::Manager) -> !`.
- `crates/rc-cli/` — binary **`rc-keeper`** (no GUI deps, builds on headless Linux). Subcommands and flags: see [CLI reference](docs/reference/cli.md).
  Any client command auto-starts the daemon (detached) if ping fails. Human output = aligned table; `--json` = machine output. Exit codes: 0 ok, 1 error, 2 daemon unreachable.
- GUI (`src-tauri`) binary name **`rc-keeper-gui`**; bundles `rc-keeper` as a Tauri sidecar (`externalBin`), on launch connects via `rc_ipc::Client`, spawning the sidecar `rc-keeper daemon` if ping fails. Tauri commands proxy to the IPC client; the `instances` event polls `list_instances` every 1 s. Quitting the GUI does NOT stop the daemon (tray menu offers "Stop daemon"). `--status` moves to the CLI (`rc-keeper status --json`).
- Releases ship: GUI bundles per OS **and** standalone CLI archives `rc-keeper-<target>.tar.gz` / `.zip` (x86_64 + aarch64 Linux, universal macOS, x86_64 Windows). `install.sh --cli` (auto when no `$DISPLAY`/`$WAYLAND_DISPLAY` on Linux) installs only the CLI to `~/.local/bin`.
- GUI-only Tauri commands map onto IPC `ping` and `shutdown`. See the [IPC reference](docs/reference/ipc.md).
- Allowed values: see the field descriptions in the [IPC reference](docs/reference/ipc.md) (from `claude remote-control --help`, v2.1.316).
- Daemon subcommands and exit codes: see the [CLI reference](docs/reference/cli.md).
- Every crate `Cargo.toml` and `src-tauri/Cargo.toml` uses a literal `version = "x.y.z"` (no `version.workspace`), so release-please can bump it.
- Service restart semantics: the OS service restarts the daemon only on failure (systemd `Restart=on-failure`, launchd `KeepAlive.SuccessfulExit=false`), so `rc-keeper daemon stop` stays stopped. Socket: config dir 0700, socket 0600.

## v3 addendum: git-compatible config
The config dir can be a git repo (dotfiles), shared across machines. Rules:
- **Only diffable text is tracked.** Layout in `<config_dir>`:
  - `settings.json` — shared settings (portable values only).
  - `instances/<id>.json` — one file per instance, so adding or editing one instance never touches another's file.
  - `settings.local.json` — machine-specific values (`claudeBin`); never shared.
  - runtime (never tracked): `status.json`, `daemon.log`, `rc-keeper.sock`, `*.tmp`.
  - The daemon writes `<config_dir>/.gitignore` (if missing) listing the local + runtime files.
- **Deterministic bytes:** pretty JSON, 2-space indent, `\n` line endings, trailing newline, fixed key order = struct declaration order (`id`, `name`, `folder`, then the rest A–Z); `null`/unset optional fields omitted; `extraArgs` kept in user order. Re-saving unchanged config produces identical bytes (no timestamps, no runtime state in config).
- **Portable paths:** `folder` under the home dir is stored as `~/...`, expanded at load; other paths stay absolute.
- **Stable ids:** slug of the folder's last path segment (`rc-keeper`, collisions → `-2`), not random UUIDs; the file name equals the id and is never renamed on edit.
- **Live reload:** the daemon watches the dir (`notify` crate) and applies external edits (e.g. after `git pull`): added → added, removed → stopped+removed, changed → restarted if running. Invalid file → logged + shown as `error` state, others unaffected.
- **Migration:** a legacy single `config.json` is split into the new layout once, then renamed `config.json.migrated`.
- `rc-keeper config path` prints the dir; env `RC_KEEPER_CONFIG_DIR` overrides it (point it at a git repo).

## v4 addendum: force boot (take over a folder)
Only one `claude remote-control` may serve a folder. "Force boot" stops the other one and starts the folder under rc-keeper.
- Detect: a process is a *foreign rc* for folder F when its command line runs `claude` with `remote-control` or `rc`, its cwd equals F (canonical), and it is not a child of this daemon. Use `sysinfo` (cmd + cwd) — works on macOS, Linux, Windows.
- Stop: SIGTERM (Windows: terminate), wait up to 5 s, then SIGKILL; also kill its `--sdk-url` session children. Never kill by name pattern alone; never touch processes outside folder F.
- Supervisor: if the foreign rc's parent is a supervisor that will respawn it (launchd, systemd, another rc-keeper), report that in the result (`respawnRisk: "<parent command>"`) so the user can disable it.
- Per-instance option `forceBoot: bool` (default false): on folder-busy, take over automatically instead of backing off.
- One-shot: IPC method `force_start({id}) -> {stopped: [pid], respawnRisk: string|null}`; Tauri command of the same name; CLI `rc-keeper start <sel> --force` and `rc-keeper add <folder> --force-boot`.
- UI: a folder-busy card shows a "Take over" button (confirm dialog naming the PIDs it will stop); the form gets a "Force boot" switch with an info popover.
