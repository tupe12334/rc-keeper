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

/// Reads an epoch-seconds file written by the fake script.
fn read_time(folder: &Path, name: &str) -> f64 {
    std::fs::read_to_string(folder.join(name))
        .unwrap_or_default()
        .trim()
        .parse()
        .unwrap_or(f64::NAN)
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
fn add_with_auto_start_runs_without_start_call() {
    let (manager, folder) = setup("add_auto_start");
    let cfg = InstanceConfig {
        auto_start: true,
        ..instance(&folder, true, &[])
    };
    let id = manager.add(cfg).unwrap();

    let ready = wait_for(Duration::from_secs(20), || {
        status(&manager, &id).state == State::Running
    });
    assert!(ready, "add did not start it: {:?}", status(&manager, &id));
    manager.shutdown();
}

#[test]
fn answer_is_sent_at_least_two_seconds_after_prompt() {
    let (manager, folder) = setup("answer_delay");
    let id = manager.add(instance(&folder, true, &[])).unwrap();
    manager.start(&id).unwrap();

    // The file exists before its content is written, so wait for a parsed timestamp.
    let answered = wait_for(Duration::from_secs(20), || {
        read_time(&folder, "answer.time").is_finite()
    });
    assert!(answered, "fake never received an answer");

    let gap = read_time(&folder, "answer.time") - read_time(&folder, "prompt.time");
    assert!(gap >= 2.0, "answer came {gap:.3} s after the prompt");
    manager.shutdown();
}

#[test]
fn late_trust_like_prompt_after_session_url_is_ignored() {
    let (manager, folder) = setup("late_prompt");
    let id = manager.add(instance(&folder, true, &[])).unwrap();
    manager.start(&id).unwrap();

    let ready = wait_for(Duration::from_secs(20), || {
        status(&manager, &id)
            .session_urls
            .iter()
            .any(|u| u == SESSION_URL)
    });
    assert!(ready, "never saw the session URL");
    // Give the fake time to print its late [y/N] prompt.
    std::thread::sleep(Duration::from_secs(3));

    let answers = std::fs::read_to_string(folder.join("answers.log")).unwrap_or_default();
    assert_eq!(answers.lines().count(), 1, "answers: {answers:?}");
    assert_eq!(status(&manager, &id).state, State::Running);
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
fn stop_during_backoff_returns_quickly() {
    let (manager, folder) = setup("stop_backoff");
    let id = manager
        .add(instance(&folder, false, &["--fake-busy"]))
        .unwrap();
    manager.start(&id).unwrap();

    // The busy fake exits at once, then the supervisor sleeps 2 s before retrying.
    let waiting = wait_for(Duration::from_secs(10), || {
        let s = status(&manager, &id);
        s.state == State::FolderBusy && s.next_retry_secs.is_some()
    });
    assert!(waiting, "never entered backoff");

    let began = Instant::now();
    manager.stop(&id).unwrap();
    let took = began.elapsed();
    assert!(took < Duration::from_secs(1), "stop took {took:?}");
    manager.shutdown();
}

#[test]
fn orphaned_background_child_is_killed_after_natural_exit() {
    let (manager, folder) = setup("orphan");
    let id = manager
        .add(instance(&folder, true, &["--fake-orphan"]))
        .unwrap();
    manager.start(&id).unwrap();

    let exited = wait_for(Duration::from_secs(20), || {
        status(&manager, &id).state == State::Restarting
    });
    assert!(exited, "child never exited: {:?}", status(&manager, &id));

    let pid: u32 = std::fs::read_to_string(folder.join("orphan.pid"))
        .expect("orphan.pid written")
        .trim()
        .parse()
        .expect("pid");
    let gone = wait_for(Duration::from_secs(3), || !pid_alive(pid));
    assert!(gone, "background child {pid} survived the natural exit");
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

fn last_error(manager: &Manager, id: &str) -> Option<String> {
    status(manager, id).last_error
}

#[test]
fn offline_error_is_explained_while_retrying() {
    let (manager, folder) = setup("offline");
    let id = manager
        .add(instance(&folder, true, &["--fake-offline"]))
        .unwrap();
    manager.start(&id).unwrap();

    let explained = wait_for(Duration::from_secs(10), || {
        last_error(&manager, &id).is_some_and(|e| e.starts_with("No network: DNS lookup failed"))
    });
    assert!(explained, "error: {:?}", status(&manager, &id));
    manager.shutdown();
}

#[test]
fn error_clears_once_connected_again() {
    let (manager, folder) = setup("reconnect");
    let id = manager
        .add(instance(&folder, true, &["--fake-reconnect"]))
        .unwrap();
    manager.start(&id).unwrap();

    let saw_error = wait_for(Duration::from_secs(10), || {
        last_error(&manager, &id).is_some()
    });
    assert!(saw_error, "never saw the DNS error");
    let cleared = wait_for(Duration::from_secs(10), || {
        let s = status(&manager, &id);
        s.state == State::Running && s.last_error.is_none()
    });
    assert!(cleared, "stale error stayed: {:?}", status(&manager, &id));
    manager.shutdown();
}

#[test]
fn silent_exit_reports_the_exit_code() {
    let (manager, folder) = setup("silent_exit");
    let id = manager
        .add(instance(&folder, true, &["--fake-silent-exit"]))
        .unwrap();
    manager.start(&id).unwrap();

    let reported = wait_for(Duration::from_secs(10), || {
        last_error(&manager, &id).is_some_and(|e| e.contains("exited with code 3"))
    });
    assert!(reported, "error: {:?}", status(&manager, &id));
    manager.shutdown();
}
