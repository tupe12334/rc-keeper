use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// IPC connection to the daemon, shared through Tauri state. `None` means not connected.
pub struct Daemon(pub Mutex<Option<rc_ipc::Client>>);

const WAIT_FOR_DAEMON: Duration = Duration::from_secs(5);

/// Connects to a running daemon, or spawns the bundled `rc-keeper daemon` sidecar and waits up to 5 s.
pub fn connect_or_spawn() -> Result<rc_ipc::Client, String> {
    if let Some(client) = try_ping() {
        return Ok(client);
    }
    spawn_sidecar()?;
    let deadline = Instant::now() + WAIT_FOR_DAEMON;
    loop {
        if let Some(client) = try_ping() {
            return Ok(client);
        }
        if Instant::now() >= deadline {
            return Err("daemon did not answer within 5 s".into());
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// Connects and pings without spawning anything.
pub fn try_ping() -> Option<rc_ipc::Client> {
    let mut client = rc_ipc::Client::connect().ok()?;
    client.call("ping", json!({})).ok()?;
    Some(client)
}

/// Runs one IPC call. Drops the connection on error; the next call reconnects.
// ponytail: no retry after an error, so a failed add is never sent twice.
pub fn call(daemon: &Daemon, method: &str, params: Value) -> Result<Value, String> {
    let mut slot = daemon
        .0
        .lock()
        .map_err(|_| "daemon lock poisoned".to_string())?;
    if slot.is_none() {
        *slot = Some(connect_or_spawn()?);
    }
    let result = slot.as_mut().expect("connected above").call(method, params);
    result.map_err(|e| {
        *slot = None;
        e.to_string()
    })
}

/// Sidecar sits next to the GUI binary: in the bundle, and in target/<profile> after tauri-build copies it.
fn spawn_sidecar() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let sidecar = exe.with_file_name(format!("rc-keeper{}", std::env::consts::EXE_SUFFIX));
    // Child is dropped without waiting: the daemon outlives the GUI.
    Command::new(&sidecar)
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to start {}: {e}", sidecar.display()))
}
