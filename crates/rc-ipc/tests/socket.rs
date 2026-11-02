//! Socket location: long config dirs, two daemons, runtime dir checks, legacy fallback.
//! Unix only. Env vars are process-wide, so each test takes `LOCK`.
#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;

use rc_core::Manager;
use rc_ipc::{bind, serve, socket_path, Client};

static LOCK: Mutex<()> = Mutex::new(());

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcipc-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Points the env at `root`: config dir `root/cfg`, runtime dir `root/run/rc-keeper`.
fn use_roots(root: &Path) {
    std::fs::create_dir_all(root.join("run")).unwrap();
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("cfg"));
    std::env::set_var("XDG_RUNTIME_DIR", root.join("run"));
}

/// Binds a daemon for the env's config dir and serves it on a thread.
fn start_daemon() {
    let listener = bind().expect("bind");
    let manager = Manager::load(std::env::var_os("RC_KEEPER_CONFIG_DIR").unwrap().into());
    thread::spawn(move || {
        serve(listener, manager);
    });
}

#[test]
fn long_config_dir_binds_and_connects() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = scratch("long");
    // Default runtime dir (/tmp/rc-keeper-<uid>): the production path.
    std::env::remove_var("XDG_RUNTIME_DIR");
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("c".repeat(160)));

    start_daemon();
    assert!(socket_path().as_os_str().len() < 104, "{:?}", socket_path());
    let info = Client::connect().unwrap().ping().unwrap();
    assert!(info["pid"].as_u64().is_some());
}

#[test]
fn two_config_dirs_run_two_daemons() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = scratch("two");
    use_roots(&root);

    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("a"));
    start_daemon();
    let sock_a = socket_path();

    // A shared socket would make this bind fail with "already running".
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("b"));
    start_daemon();
    let sock_b = socket_path();
    assert_ne!(sock_a, sock_b);

    // Same dir, same socket.
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("a"));
    assert_eq!(socket_path(), sock_a);
    Client::connect().unwrap().ping().unwrap();
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("b"));
    Client::connect().unwrap().ping().unwrap();
}

#[test]
fn runtime_dir_with_wrong_mode_is_refused() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = scratch("mode");
    use_roots(&root);
    let dir = root.join("run").join("rc-keeper");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    let Err(err) = bind() else {
        panic!("bind must fail")
    };
    let msg = format!("{err:#}");
    assert!(
        msg.contains("must be a real directory owned by uid"),
        "{msg}"
    );
    assert!(Client::connect().is_err());
}

#[test]
fn symlinked_runtime_dir_is_refused() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = scratch("symlink");
    use_roots(&root);
    let real = root.join("elsewhere");
    std::fs::create_dir(&real).unwrap();
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&real, root.join("run").join("rc-keeper")).unwrap();

    let Err(err) = bind() else {
        panic!("bind must fail")
    };
    let msg = format!("{err:#}");
    assert!(msg.contains("must be a real directory"), "{msg}");
}

#[test]
fn client_falls_back_to_legacy_socket() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = scratch("legacy");
    use_roots(&root);
    let cfg = root.join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();

    // Stands in for an old daemon: one line in, one ping reply out.
    let listener = UnixListener::bind(cfg.join("rc-keeper.sock")).unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut line = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            stream
                .write_all(b"{\"id\":1,\"result\":{\"version\":\"legacy\",\"pid\":1}}\n")
                .unwrap();
        }
    });

    let info = Client::connect().unwrap().ping().unwrap();
    assert_eq!(info["version"], "legacy");
    // The legacy daemon still counts as running, so a new daemon must not start.
    assert!(bind().is_err());
}
