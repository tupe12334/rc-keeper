//! Daemon side: accept connections, one thread each, dispatch methods onto the Manager.

use std::io::{self, BufRead, BufReader, Write};
#[cfg(unix)]
use std::sync::OnceLock;
use std::thread;

use anyhow::{bail, Context, Result};
use interprocess::local_socket::{prelude::*, Listener, ListenerOptions, Stream};
use rc_core::{find_claude_bin, Manager, Settings};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::client::Client;
use crate::method::Method;
use crate::paths::{config_dir, socket_name, socket_path};
use crate::protocol::{Request, Response};
use crate::schema::{CfgParams, GetSettingsResult, IdParams, PingResult, SetSettingsParams};

/// Binds the daemon socket. Fails when a live daemon answers ping, on the new or the legacy socket.
///
/// Never unlinks a socket that answers ping: a daemon that lost the race exits here
/// and leaves the winner's socket in place.
pub fn bind() -> Result<Listener> {
    prepare_config_dir().context("cannot create config dir")?;
    if daemon_answers() {
        bail!("rc-keeper daemon is already running");
    }
    match try_bind() {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            if daemon_answers() {
                bail!("rc-keeper daemon is already running");
            }
            // Nothing answers, so the file belongs to a dead daemon.
            let _ = std::fs::remove_file(socket_path());
            try_bind().context("cannot bind rc-keeper socket")
        }
        Err(e) => Err(e).context("cannot bind rc-keeper socket"),
    }
}

fn daemon_answers() -> bool {
    Client::connect().and_then(|mut c| c.ping()).is_ok()
}

/// Serves requests forever on a socket from [`bind`].
pub fn serve(listener: Listener, manager: Manager) -> ! {
    loop {
        match listener.accept() {
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

fn try_bind() -> io::Result<Listener> {
    let listener = ListenerOptions::new().name(socket_name()?).create_sync()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let path = socket_path();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let _ = OWN_INODE.set(std::fs::metadata(&path)?.ino());
    }
    Ok(listener)
}

#[cfg(unix)]
fn remove_own_socket() {
    use std::os::unix::fs::MetadataExt;
    let Some(ino) = OWN_INODE.get() else { return };
    let path = socket_path();
    if std::fs::metadata(&path).is_ok_and(|m| m.ino() == *ino) {
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
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)?;
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
