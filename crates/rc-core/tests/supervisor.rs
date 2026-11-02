//! End-to-end tests against a fake `claude` script. The real binary is never run.
#![cfg(unix)]

use rc_core::{InstanceConfig, InstanceStatus, Manager, Settings, State};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const SESSION_URL: &str = "https://claude.ai/code/session_123";

fn fake_claude() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-claude.sh")
}

/// A fresh manager whose settings point at the fake script, plus a work folder.
fn setup(name: &str) -> (Manager, PathBuf) {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("work");
    std::fs::create_dir_all(&folder).unwrap();
    let manager = Manager::load(root.join("config"));
    manager
        .set_settings(Settings {
            claude_bin: Some(fake_claude()),
        })
        .unwrap();
    (manager, folder)
}

fn instance(folder: &Path, auto_trust: bool, extra_args: &[&str]) -> InstanceConfig {
    InstanceConfig {
        folder: folder.to_path_buf(),
        auto_trust,
        extra_args: extra_args.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn status(manager: &Manager, id: &str) -> InstanceStatus {
    manager
        .list()
        .into_iter()
        .find(|v| v.status.id == id)
        .expect("instance listed")
        .status
}

fn wait_for(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    done()
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn auto_trust_reaches_running_with_session_url() {
    let (manager, folder) = setup("auto_trust");
    let id = manager.add(instance(&folder, true, &[])).unwrap();
    manager.start(&id).unwrap();

    let ready = wait_for(Duration::from_secs(20), || {
        let s = status(&manager, &id);
        s.state == State::Running && s.session_urls.iter().any(|u| u == SESSION_URL)
    });
    assert!(ready, "never reached running: {:?}", status(&manager, &id));

    let pid = status(&manager, &id).pid.expect("pid while running");
    manager.stop(&id).unwrap();
    assert!(!pid_alive(pid), "child {pid} survived stop");
    manager.shutdown();
}

#[test]
fn busy_folder_reports_folder_busy_then_restarts() {
    let (manager, folder) = setup("busy");
    let id = manager
        .add(instance(&folder, false, &["--fake-busy"]))
        .unwrap();
    manager.start(&id).unwrap();

    let busy = wait_for(Duration::from_secs(10), || {
        status(&manager, &id).state == State::FolderBusy
    });
    assert!(
        busy,
        "never reported folder-busy: {:?}",
        status(&manager, &id)
    );

    let retried = wait_for(Duration::from_secs(10), || {
        status(&manager, &id).restarts >= 1
    });
    assert!(retried, "never retried: {:?}", status(&manager, &id));

    manager.stop(&id).unwrap();
    assert_eq!(status(&manager, &id).state, State::Stopped);
    manager.shutdown();
}

#[test]
fn untrusted_folder_needs_trust_and_does_not_restart() {
    let (manager, folder) = setup("needs_trust");
    let id = manager.add(instance(&folder, false, &[])).unwrap();
    manager.start(&id).unwrap();

    let needs = wait_for(Duration::from_secs(10), || {
        status(&manager, &id).state == State::NeedsTrust
    });
    assert!(needs, "never asked for trust: {:?}", status(&manager, &id));

    std::thread::sleep(Duration::from_secs(3));
    let s = status(&manager, &id);
    assert_eq!((s.state, s.pid, s.restarts), (State::NeedsTrust, None, 0));
    manager.shutdown();
}
