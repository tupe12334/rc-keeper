//! Connects to the daemon, starting it detached when no daemon answers ping.

use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use rc_ipc::{config_dir, Client};

#[derive(Debug)]
pub struct DaemonUnreachable;

impl std::fmt::Display for DaemonUnreachable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "rc-keeper daemon is not reachable (log: {})",
            config_dir().join("daemon.log").display()
        )
    }
}

impl std::error::Error for DaemonUnreachable {}

/// A connected client, if a daemon answers ping.
pub fn live_client() -> Option<Client> {
    let mut client = Client::connect().ok()?;
    client.ping().ok()?;
    Some(client)
}

/// Connects, or starts the daemon and waits up to 5 s for it.
pub fn connect_or_start() -> Result<Client> {
    if let Some(client) = live_client() {
        return Ok(client);
    }
    spawn_daemon()?;
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if let Some(client) = live_client() {
            return Ok(client);
        }
    }
    Err(DaemonUnreachable.into())
}

fn spawn_daemon() -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("daemon.log"))?;
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.arg("daemon")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    }
    cmd.spawn()?;
    Ok(())
}
