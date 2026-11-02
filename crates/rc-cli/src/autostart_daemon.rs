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
    if let Err(e) = spawn_daemon() {
        eprintln!("rc-keeper: cannot start daemon: {e:#}");
        return Err(DaemonUnreachable.into());
    }
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
        // SAFETY: runs between fork and exec and only calls setsid, which is async-signal-safe.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
        stop_std_handle_inheritance();
    }
    cmd.spawn()?;
    Ok(())
}

/// `CreateProcess` runs with `bInheritHandles = TRUE`, so the daemon would also get
/// every inheritable handle of this CLI. Our own stdio handles are inheritable when a
/// parent passed them in, for example a pwsh pipe in `(rc-keeper status) | Out-String`.
/// The daemon would then hold the pipe open and the caller would wait for EOF forever.
/// The daemon gets its own stdio from `Command`, so clear the flag on ours first.
#[cfg(windows)]
fn stop_std_handle_inheritance() {
    use windows_sys::Win32::Foundation::{
        SetHandleInformation, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: GetStdHandle and SetHandleInformation take no pointers we own; a null or
        // invalid handle is skipped, and a failure only leaves the old flag in place.
        unsafe {
            let h = GetStdHandle(id);
            if !h.is_null() && h != INVALID_HANDLE_VALUE {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}
