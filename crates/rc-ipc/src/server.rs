//! Daemon side: accept connections, one thread each, dispatch methods onto the Manager.

use std::io::{self, BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::sync::{Mutex, OnceLock};
use std::thread;

use anyhow::{bail, Context, Result};
use interprocess::local_socket::{prelude::*, Listener, ListenerOptions, Stream};
use rc_core::{find_claude_bin, Manager, Settings};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::client::{Client, ForeignDaemon};
use crate::method::Method;
#[cfg(unix)]
use crate::paths::socket_path;
use crate::paths::{canonical_config_dir, config_dir, socket_name};
use crate::protocol::{Request, Response};
use crate::schema::{
    CfgParams, GetSettingsResult, IdParams, PingResult, SetSettingsParams, TakeoverResult,
};

/// Binds the daemon socket. Fails when a live daemon answers ping, on the new or the legacy socket.
///
/// Holds an exclusive lock on the config dir's lock file before it probes or unlinks anything.
/// A daemon that loses the race exits here and leaves the winner's socket in place.
pub fn bind() -> Result<Listener> {
    prepare_config_dir().context("cannot create config dir")?;
    lock_runtime_dir()?;
    if daemon_answers()? {
        bail!("rc-keeper daemon is already running");
    }
    match try_bind() {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            if daemon_answers()? {
                bail!("rc-keeper daemon is already running");
            }
            remove_stale_socket().context("cannot remove stale socket")?;
            try_bind().context("cannot bind rc-keeper socket")
        }
        Err(e) => Err(e).context("cannot bind rc-keeper socket"),
    }
}

/// True when a daemon for this config dir answers. Errors when another config dir owns the socket.
fn daemon_answers() -> Result<bool> {
    match Client::connect() {
        Ok(_) => Ok(true),
        Err(e) if e.is::<ForeignDaemon>() => Err(e),
        Err(_) => Ok(false),
    }
}

/// Serves requests forever on a socket from [`bind`].
pub fn serve(listener: Listener, manager: Manager) -> ! {
    serve_until(listener, manager, &AtomicBool::new(false));
    unreachable!("the accept loop only returns when stop is set")
}

/// Serves until `stop` is set and one more connection arrives to wake the accept loop.
pub fn serve_until(listener: Listener, manager: Manager, stop: &AtomicBool) {
    loop {
        match listener.accept() {
            Ok(_) if stop.load(Ordering::SeqCst) => return,
            Ok(stream) => {
                let manager = manager.clone();
                thread::spawn(move || handle(stream, &manager));
            }
            Err(e) => eprintln!("rc-keeper: accept failed: {e}"),
        }
    }
}

/// Kills all children, removes this daemon's socket and exits the process.
pub fn shutdown_and_exit(manager: &Manager) -> ! {
    manager.shutdown();
    remove_own_socket();
    std::process::exit(0)
}

/// Inode of the socket this process bound. Only that file is removed on exit.
#[cfg(unix)]
static OWN_INODE: OnceLock<u64> = OnceLock::new();

/// Lock files held for the life of the process, one per config dir.
#[cfg(unix)]
static LOCKS: Mutex<Vec<std::fs::File>> = Mutex::new(Vec::new());

/// Exclusive `flock` on `<runtime dir>/<hash>.lock`. Released when the process exits.
#[cfg(unix)]
fn lock_runtime_dir() -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;
    crate::paths::ensure_runtime_dir().context("cannot use runtime dir")?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(crate::paths::lock_path())
        .context("cannot open daemon lock")?;
    // SAFETY: the fd stays valid while `file` lives, and we keep it in LOCKS.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        bail!("rc-keeper daemon is already running");
    }
    LOCKS.lock().unwrap_or_else(|e| e.into_inner()).push(file);
    Ok(())
}

#[cfg(not(unix))]
fn lock_runtime_dir() -> Result<()> {
    Ok(())
}

fn try_bind() -> io::Result<Listener> {
    let listener = ListenerOptions::new().name(socket_name()?).create_sync()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let path = socket_path();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let _ = OWN_INODE.set(std::fs::symlink_metadata(&path)?.ino());
    }
    Ok(listener)
}

/// Removes the socket file only when it is a socket we own. Called while holding the lock.
#[cfg(unix)]
fn remove_stale_socket() -> io::Result<()> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let path = socket_path();
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if !meta.file_type().is_socket() || meta.uid() != crate::paths::current_uid() {
        return Err(io::Error::other(format!(
            "{} is not our socket",
            path.display()
        )));
    }
    std::fs::remove_file(&path)
}

#[cfg(not(unix))]
fn remove_stale_socket() -> io::Result<()> {
    // Named pipes have no file to remove.
    Ok(())
}

#[cfg(unix)]
fn remove_own_socket() {
    use std::os::unix::fs::MetadataExt;
    let Some(ino) = OWN_INODE.get() else { return };
    let path = socket_path();
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.ino() == *ino) {
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(not(unix))]
fn remove_own_socket() {
    // Named pipes have no file to remove.
}

/// Config dir is private to the user (0700). Unix only.
fn prepare_config_dir() -> io::Result<()> {
    let dir = config_dir();
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)?;
        let meta = std::fs::symlink_metadata(&dir)?;
        if !meta.is_dir() || meta.uid() != crate::paths::current_uid() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} must be a real dir we own", dir.display()),
            ));
        }
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(&dir)?;
    Ok(())
}

fn handle(stream: Stream, manager: &Manager) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let (response, stop) = match serde_json::from_str::<Request>(line.trim_end()) {
            Ok(req) => {
                let result = dispatch(manager, &req.method, &req.params);
                let resp = match result {
                    Ok(v) => Response::Ok {
                        id: req.id,
                        result: v,
                    },
                    Err(error) => Response::Err { id: req.id, error },
                };
                (resp, req.method == "shutdown")
            }
            Err(e) => (
                Response::Err {
                    id: 0,
                    error: format!("bad request: {e}"),
                },
                false,
            ),
        };
        let Ok(mut out) = serde_json::to_string(&response) else {
            return;
        };
        out.push('\n');
        let writer = reader.get_mut();
        if writer
            .write_all(out.as_bytes())
            .and_then(|_| writer.flush())
            .is_err()
        {
            return;
        }
        if stop {
            shutdown_and_exit(manager);
        }
    }
}

fn dispatch(manager: &Manager, method: &str, params: &Value) -> Result<Value, String> {
    let Some(method) = Method::from_name(method) else {
        return Err(format!("unknown method: {method}"));
    };
    match method {
        Method::Ping => to_value(PingResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
            pid: std::process::id(),
            config_dir: canonical_config_dir(),
        }),
        Method::Shutdown => Ok(Value::Null),
        Method::ListInstances => to_value(manager.list()),
        Method::AddInstance => {
            let CfgParams { cfg } = parse(params)?;
            manager.add(cfg).map(Value::from)
        }
        Method::UpdateInstance => {
            let CfgParams { cfg } = parse(params)?;
            unit(manager.update(cfg))
        }
        Method::RemoveInstance => {
            let IdParams { id } = parse(params)?;
            unit(manager.remove(&id))
        }
        Method::StartInstance => {
            let IdParams { id } = parse(params)?;
            unit(manager.start(&id))
        }
        Method::StopInstance => {
            let IdParams { id } = parse(params)?;
            unit(manager.stop(&id))
        }
        Method::RestartInstance => {
            let IdParams { id } = parse(params)?;
            unit(manager.restart(&id))
        }
        Method::GetLogs => {
            let IdParams { id } = parse(params)?;
            to_value(manager.logs(&id))
        }
        Method::StartAll => {
            manager.start_all();
            Ok(Value::Null)
        }
        Method::StopAll => {
            manager.stop_all();
            Ok(Value::Null)
        }
        // ponytail: GUI owns login autostart, so the daemon reports null for it.
        Method::GetSettings => to_value(GetSettingsResult {
            claude_bin: manager.settings().claude_bin,
            autostart: (),
        }),
        Method::SetSettings => {
            let SetSettingsParams { claude_bin } = parse(params)?;
            unit(manager.set_settings(Settings { claude_bin }))
        }
        Method::DetectClaudeBin => to_value(find_claude_bin(&manager.settings())),
        Method::ForceStart => {
            let IdParams { id } = parse(params)?;
            manager
                .force_start(&id)
                .and_then(|taken| to_value(TakeoverResult::from(taken)))
        }
        Method::PreviewForceStart => {
            let IdParams { id } = parse(params)?;
            manager
                .preview_force_start(&id)
                .and_then(|taken| to_value(TakeoverResult::from(taken)))
        }
    }
}

fn unit(result: Result<(), String>) -> Result<Value, String> {
    result.map(|()| Value::Null)
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// Parses params into the same struct the docs describe. A missing `params` is `null`.
fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, String> {
    let v = if params.is_null() { &json!({}) } else { params };
    serde_json::from_value(v.clone()).map_err(|e| format!("bad params: {e}"))
}
