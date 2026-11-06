//! Update check and update for the GUI. Both run the bundled `rc-keeper` CLI, which holds the logic.

use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use std::thread;

use serde_json::Value;

use crate::daemon::sidecar_path;

/// `rc-keeper update --check --json`: current and latest version, and whether an update runs.
pub async fn info() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let out = Command::new(sidecar_path()?)
            .args(["update", "--check", "--json"])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Starts `rc-keeper update --gui` detached. Its output goes to `update.log` in the config dir.
pub fn start() -> Result<(), String> {
    let dir = rc_ipc::config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let log = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(dir.join("update.log"))
        .map_err(|e| e.to_string())?;
    let mut cmd = Command::new(sidecar_path()?);
    cmd.args(["update", "--gui"])
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // Own process group: quitting the GUI does not stop the install.
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    // Reap the child when it exits, so it does not stay a zombie while the GUI runs.
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
