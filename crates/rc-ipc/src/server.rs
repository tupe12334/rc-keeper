//! Daemon side: accept connections, one thread each, dispatch methods onto the Manager.

use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::OnceLock;
use std::thread;

use anyhow::{bail, Context, Result};
use interprocess::local_socket::{prelude::*, Listener, ListenerOptions, Stream};
use rc_core::{find_claude_bin, InstanceConfig, Manager, Settings};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::client::Client;
use crate::paths::{config_dir, socket_name, socket_path};
use crate::protocol::{Request, Response};

/// Binds the daemon socket. Fails when a live daemon answers ping.
///
/// Never unlinks a socket that answers ping: a daemon that lost the race exits here
/// and leaves the winner's socket in place.
pub fn bind() -> Result<Listener> {
    prepare_config_dir().context("cannot create config dir")?;
    match try_bind() {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            if Client::connect().and_then(|mut c| c.ping()).is_ok() {
                bail!("rc-keeper daemon is already running");
            }
            // Nothing answers, so the file belongs to a dead daemon.
            let _ = std::fs::remove_file(socket_path());
            try_bind().context("cannot bind rc-keeper socket")
        }
        Err(e) => Err(e).context("cannot bind rc-keeper socket"),
    }
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
    match method {
        "ping" => Ok(json!({ "version": env!("CARGO_PKG_VERSION"), "pid": std::process::id() })),
        "shutdown" => Ok(Value::Null),
        "list_instances" => to_value(manager.list()),
        "add_instance" => manager.add(cfg_param(params)?).map(Value::from),
        "update_instance" => unit(manager.update(cfg_param(params)?)),
        "remove_instance" => unit(manager.remove(&str_param(params, "id")?)),
        "start_instance" => unit(manager.start(&str_param(params, "id")?)),
        "stop_instance" => unit(manager.stop(&str_param(params, "id")?)),
        "restart_instance" => unit(manager.restart(&str_param(params, "id")?)),
        "get_logs" => to_value(manager.logs(&str_param(params, "id")?)),
        "start_all" => {
            manager.start_all();
            Ok(Value::Null)
        }
        "stop_all" => {
            manager.stop_all();
            Ok(Value::Null)
        }
        // ponytail: GUI owns login autostart, so the daemon reports null for it.
        "get_settings" => {
            Ok(json!({ "claudeBin": manager.settings().claude_bin, "autostart": null }))
        }
        "set_settings" => {
            let claude_bin = params
                .get("claudeBin")
                .and_then(Value::as_str)
                .map(PathBuf::from);
            unit(manager.set_settings(Settings { claude_bin }))
        }
        "detect_claude_bin" => to_value(find_claude_bin(&manager.settings())),
        other => Err(format!("unknown method: {other}")),
    }
}

fn unit(result: Result<(), String>) -> Result<Value, String> {
    result.map(|()| Value::Null)
}

fn to_value<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn param<T: DeserializeOwned>(params: &Value, key: &str) -> Result<T, String> {
    let v = params
        .get(key)
        .ok_or_else(|| format!("missing param: {key}"))?;
    serde_json::from_value(v.clone()).map_err(|e| format!("bad param {key}: {e}"))
}

fn str_param(params: &Value, key: &str) -> Result<String, String> {
    param(params, key)
}

fn cfg_param(params: &Value) -> Result<InstanceConfig, String> {
    param(params, "cfg")
}
