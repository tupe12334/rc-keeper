use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// IPC connection to the daemon, shared through Tauri state.
pub struct Daemon {
    conn: Mutex<Option<rc_ipc::Client>>,
    /// Set by "Stop daemon". Blocks respawns until `start`.
    stopped: AtomicBool,
}

const WAIT_FOR_DAEMON: Duration = Duration::from_secs(5);
const STOPPED: &str = "daemon is stopped; start it from the tray or Settings";

impl Daemon {
    pub fn new() -> Self {
        Daemon {
            conn: Mutex::new(None),
            stopped: AtomicBool::new(false),
        }
    }
}

/// Connects to a running daemon, or spawns the sidecar and waits up to 5 s.
fn connect_or_spawn() -> Result<rc_ipc::Client, String> {
    if let Some((client, _)) = try_ping() {
        return Ok(client);
    }
    spawn_sidecar()?;
    let deadline = Instant::now() + WAIT_FOR_DAEMON;
    loop {
        if let Some((client, _)) = try_ping() {
            return Ok(client);
        }
        if Instant::now() >= deadline {
            return Err("daemon did not answer within 5 s".into());
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// Connects and pings without spawning. Returns the client and the daemon version.
pub fn try_ping() -> Option<(rc_ipc::Client, String)> {
    let mut client = rc_ipc::Client::connect().ok()?;
    let reply = client.call("ping", json!({})).ok()?;
    let version = reply["version"].as_str().unwrap_or_default().to_string();
    Some((client, version))
}

/// Runs one IPC call. Connects lazily. Drops the connection on error; the next call reconnects.
// ponytail: no retry after an error, so a failed add is never sent twice.
pub fn call(daemon: &Daemon, method: &str, params: Value) -> Result<Value, String> {
    let mut slot = daemon
        .conn
        .lock()
        .map_err(|_| "daemon lock poisoned".to_string())?;
    if slot.is_none() {
        if daemon.stopped.load(Ordering::SeqCst) {
            return Err(STOPPED.into());
        }
        *slot = Some(connect_or_spawn()?);
    }
    let result = slot.as_mut().expect("connected above").call(method, params);
    result.map_err(|e| {
        *slot = None;
        e.to_string()
    })
}

/// Clears the stop flag and starts the daemon if no daemon answers.
pub fn start(daemon: &Daemon) -> Result<(), String> {
    let mut slot = daemon
        .conn
        .lock()
        .map_err(|_| "daemon lock poisoned".to_string())?;
    daemon.stopped.store(false, Ordering::SeqCst);
    if slot.is_none() {
        *slot = Some(connect_or_spawn()?);
    }
    Ok(())
}

/// Asks a running daemon to shut down, and keeps it stopped until `start`. Never spawns one.
pub fn stop(daemon: &Daemon) -> Result<(), String> {
    daemon.stopped.store(true, Ordering::SeqCst);
    let mut slot = daemon
        .conn
        .lock()
        .map_err(|_| "daemon lock poisoned".to_string())?;
    *slot = None;
    if let Some((mut client, _)) = try_ping() {
        client
            .call("shutdown", json!({}))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Version and reachability of the daemon. Never spawns one.
pub fn info() -> Value {
    match try_ping() {
        Some((_, version)) => json!({ "version": version, "running": true }),
        None => json!({ "version": "", "running": false }),
    }
}

/// Sidecar sits next to the GUI binary: in the bundle, and in target/<profile> after tauri-build copies it.
fn spawn_sidecar() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let sidecar = exe.with_file_name(format!("rc-keeper{}", std::env::consts::EXE_SUFFIX));
    let mut cmd = Command::new(&sidecar);
    cmd.arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Own process group: Ctrl-C in the GUI's terminal does not reach the daemon.
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to start {}: {e}", sidecar.display()))?;
    // Reap the child when it exits, so it does not stay a zombie while the GUI runs.
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
