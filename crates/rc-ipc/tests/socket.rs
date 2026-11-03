//! Socket location: long config dirs, two daemons, runtime dir checks, lock, legacy fallback, identity.
//! Unix only. Env vars are process-wide, so each test takes `LOCK`. Every test uses its own scratch
//! runtime dir through `XDG_RUNTIME_DIR`, never the real `/tmp`.
#![cfg(unix)]

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use rc_core::Manager;
use rc_ipc::{bind, config_dir, serve_until, socket_path, Client};

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

fn scratch(name: &str) -> PathBuf {
    // Short on purpose: socket paths must stay under SUN_LEN (104 bytes on macOS).
    let dir = std::env::temp_dir().join(format!("rc{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Config dir `root/cfg`. Runtime dir `root/run/rc-keeper`, private, so the parent check passes.
fn use_roots(root: &Path) {
    let run = root.join("run");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::env::set_var("XDG_RUNTIME_DIR", &run);
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("cfg"));
}

/// Creates the runtime dir the way the daemon does, for tests that touch files in it first.
fn make_runtime_dir(root: &Path) {
    let dir = root.join("run").join("rc-keeper");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
}

/// A daemon for the env's config dir. Drop stops its accept loop and removes its socket.
struct Daemon {
    sock: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn start_daemon() -> Daemon {
    let listener = bind().expect("bind");
    let sock = socket_path();
    let manager = Manager::load(config_dir());
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = thread::spawn(move || serve_until(listener, manager, &flag));
    Daemon {
        sock,
        stop,
        thread: Some(thread),
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // The accept loop checks `stop` after each accept, so one connection wakes it.
        if UnixStream::connect(&self.sock).is_ok() {
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
        let _ = std::fs::remove_file(&self.sock);
    }
}

/// Reads one request line and answers it with `{"id":1,"result":<result>}`.
fn answer(mut stream: UnixStream, result: &str) {
    // The bind probe connects without sending a ping, so an empty read is fine.
    let mut line = String::new();
    let _ = BufReader::new(stream.try_clone().unwrap()).read_line(&mut line);
    let _ = stream.write_all(format!("{{\"id\":1,\"result\":{result}}}\n").as_bytes());
}

#[test]
fn long_config_dir_binds_and_connects() {
    let _g = lock();
    let root = scratch("lng");
    let long_cfg = root.join("c".repeat(160));

    // Production runtime dir (no XDG): only the path is computed here, nothing is created.
    std::env::remove_var("XDG_RUNTIME_DIR");
    std::env::set_var("RC_KEEPER_CONFIG_DIR", &long_cfg);
    let path = socket_path();
    eprintln!(
        "socket path: {} ({} bytes)",
        path.display(),
        path.as_os_str().len()
    );
    assert!(path.as_os_str().len() < 104, "{}", path.display());

    use_roots(&root);
    std::env::set_var("RC_KEEPER_CONFIG_DIR", &long_cfg);
    let _d = start_daemon();
    let info = Client::connect().unwrap().ping().unwrap();
    assert!(info["pid"].as_u64().is_some());
}

#[test]
fn two_config_dirs_run_two_daemons() {
    let _g = lock();
    let root = scratch("two");
    use_roots(&root);

    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("a"));
    let a = start_daemon();
    let sock_a = socket_path();

    // A shared socket would make this bind fail with "already running".
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("b"));
    let b = start_daemon();
    let sock_b = socket_path();
    assert_ne!(sock_a, sock_b);

    // Same dir, same socket.
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("a"));
    assert_eq!(socket_path(), sock_a);
    Client::connect().unwrap().ping().unwrap();
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("b"));
    Client::connect().unwrap().ping().unwrap();
    drop((a, b));
}

#[test]
fn runtime_dir_with_wrong_mode_is_refused() {
    let _g = lock();
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
    let _g = lock();
    let root = scratch("sym");
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
fn regular_file_at_socket_path_survives_bind() {
    let _g = lock();
    let root = scratch("fil");
    use_roots(&root);
    make_runtime_dir(&root);
    std::fs::write(socket_path(), b"not a socket").unwrap();

    let Err(err) = bind() else {
        panic!("bind must fail")
    };
    assert!(format!("{err:#}").contains("is not our socket"));
    assert_eq!(std::fs::read(socket_path()).unwrap(), b"not a socket");
}

#[test]
fn socket_mode_is_0600() {
    let _g = lock();
    let root = scratch("m600");
    use_roots(&root);
    let _d = start_daemon();
    let mode = std::fs::symlink_metadata(socket_path())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn second_daemon_is_blocked_by_the_lock() {
    let _g = lock();
    let root = scratch("lk");
    use_roots(&root);

    // A live daemon: a second bind fails, and the first socket still answers.
    let d = start_daemon();
    let Err(err) = bind() else {
        panic!("second bind must fail")
    };
    assert!(format!("{err:#}").contains("already running"));
    Client::connect().unwrap().ping().unwrap();
    drop(d);

    // A lock held with no daemon behind it: bind fails before it touches the socket.
    std::env::set_var("RC_KEEPER_CONFIG_DIR", root.join("other"));
    make_runtime_dir(&root);
    let lock_file = File::create(socket_path().with_extension("lock")).unwrap();
    // SAFETY: the fd stays valid while `lock_file` lives.
    assert_eq!(
        unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let Err(err) = bind() else {
        panic!("bind must fail under the lock")
    };
    assert!(format!("{err:#}").contains("already running"));
    assert!(std::fs::symlink_metadata(socket_path()).is_err());
}

#[test]
fn client_falls_back_to_legacy_socket() {
    let _g = lock();
    let root = scratch("leg");
    use_roots(&root);
    let cfg = root.join("cfg");
    std::fs::create_dir_all(&cfg).unwrap();

    // Stands in for an old daemon: answers the client's ping, then the bind probe's ping.
    let listener = UnixListener::bind(cfg.join("rc-keeper.sock")).unwrap();
    let fake = thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            answer(stream.unwrap(), "{\"version\":\"legacy\",\"pid\":1}");
        }
    });

    let info = Client::connect().unwrap().ping().unwrap();
    assert_eq!(info["version"], "legacy");
    // The legacy daemon still counts as running, so a new daemon must not start.
    assert!(bind().is_err());
    fake.join().unwrap();
}

#[test]
fn socket_answering_for_another_config_dir_is_refused() {
    let _g = lock();
    let root = scratch("frgn");
    use_roots(&root);
    make_runtime_dir(&root);

    // Hash collision stand-in: our socket path, but the daemon serves another config dir.
    let sock = socket_path();
    let listener = UnixListener::bind(&sock).unwrap();
    let fake = thread::spawn(move || {
        for stream in listener.incoming().take(2) {
            answer(
                stream.unwrap(),
                "{\"version\":\"x\",\"pid\":1,\"configDir\":\"/elsewhere\"}",
            );
        }
    });

    let Err(err) = bind() else {
        panic!("bind must fail")
    };
    assert!(format!("{err:#}").contains("belongs to config dir /elsewhere"));
    // The foreign socket is left in place.
    assert!(std::fs::symlink_metadata(&sock).is_ok());
    let Err(err) = Client::connect() else {
        panic!("connect must fail")
    };
    assert!(format!("{err:#}").contains("belongs to config dir /elsewhere"));
    fake.join().unwrap();
}
