//! End-to-end runs through the real `rc-keeper` binary. The daemon starts by auto-start.
//! Unix only: uses a `#!/bin/sh` fake claude, `kill`, and `ps`.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::assert::Assert;
use assert_cmd::Command;
use serde_json::{json, Value};

const SESSION_URL: &str = "https://claude.ai/code/session_abc";

/// Stops the daemon if the test panics before it does.
struct DaemonGuard(PathBuf);

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = rk(&self.0).args(["daemon", "stop"]).output();
    }
}

fn rk(root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rc-keeper").unwrap();
    cmd.env("RC_KEEPER_CONFIG_DIR", root.join("config"))
        .env("RC_KEEPER_SERVICE_DIR", root.join("services"));
    cmd
}

fn fresh_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rck-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn json(assert: &Assert) -> Value {
    serde_json::from_slice(&assert.get_output().stdout).expect("stdout is JSON")
}

fn stdout(assert: &Assert) -> String {
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

fn wait_until<T>(what: &str, timeout: Duration, mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(100));
    }
}

fn pid_alive(pid: u64) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn pgid(pid: u64) -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "pgid=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

fn send_signal(sig: &str, target: &str) {
    let status = std::process::Command::new("kill")
        .args(["-s", sig, "--", target])
        .status()
        .unwrap();
    assert!(status.success(), "kill -s {sig} {target} failed");
}

fn status_views(root: &Path) -> Value {
    json(&rk(root).args(["status", "--json"]).assert().success())
}

fn child_pid(root: &Path) -> u64 {
    wait_until("instance running", Duration::from_secs(15), || {
        let views = status_views(root);
        (views[0]["status"]["state"] == "running")
            .then(|| views[0]["status"]["pid"].as_u64().unwrap())
    })
}

#[test]
fn daemon_lifecycle_through_cli() {
    let root = fresh_root("life");
    let folder = root.join("project");
    std::fs::create_dir_all(&folder).unwrap();
    let canonical = std::fs::canonicalize(&folder).unwrap();
    let fake = root.join("fake-claude");
    std::fs::write(
        &fake,
        format!("#!/bin/sh\necho \"Remote Control ready {SESSION_URL}\"\nexec sleep 600\n"),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _guard = DaemonGuard(root.clone());

    eprintln!("step: daemon status with no daemon -> exit 2");
    rk(&root).args(["daemon", "status"]).assert().code(2);
    rk(&root)
        .args(["daemon", "status", "--json"])
        .assert()
        .code(2);

    eprintln!("step: daemon stop with no daemon -> exit 0");
    rk(&root).args(["daemon", "stop"]).assert().success();

    eprintln!("step: config set claude-bin (auto-starts the daemon)");
    rk(&root)
        .args(["config", "set", "claude-bin", fake.to_str().unwrap()])
        .assert()
        .success();

    eprintln!("step: daemon status --json -> running");
    let st = json(
        &rk(&root)
            .args(["daemon", "status", "--json"])
            .assert()
            .success(),
    );
    assert_eq!(st["running"], true);
    assert!(st["pid"].as_u64().is_some());

    eprintln!("step: config dir 0700, socket 0600");
    assert_eq!(mode(&root.join("config")), 0o700);
    assert_eq!(mode(&root.join("config").join("rc-keeper.sock")), 0o600);

    eprintln!("step: second foreground daemon fails, first stays reachable");
    rk(&root)
        .arg("daemon")
        .timeout(Duration::from_secs(10))
        .assert()
        .failure();
    rk(&root).args(["daemon", "status"]).assert().success();

    eprintln!("step: add folder");
    let id = stdout(
        &rk(&root)
            .args(["add", folder.to_str().unwrap(), "--name", "demo"])
            .assert()
            .success(),
    )
    .trim()
    .to_string();
    assert!(!id.is_empty());

    eprintln!("step: list --json has 1 instance");
    let list = json(&rk(&root).args(["list", "--json"]).assert().success());
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["config"]["id"], id.as_str());
    assert_eq!(list[0]["config"]["folder"], canonical.to_str().unwrap());

    eprintln!("step: start by folder, wait for running");
    rk(&root)
        .args(["start", folder.to_str().unwrap()])
        .assert()
        .success();
    let pid = child_pid(&root);
    let views = status_views(&root);
    assert_eq!(views[0]["status"]["sessionUrls"][0], SESSION_URL);
    assert!(stdout(&rk(&root).args(["status"]).assert().success()).contains(SESSION_URL));

    eprintln!("step: start with a selector and --all together is a usage error");
    rk(&root).args(["start", &id, "--all"]).assert().code(1);

    eprintln!("step: stop by id, child gone");
    rk(&root).args(["stop", &id]).assert().success();
    wait_until("child gone after stop", Duration::from_secs(5), || {
        (!pid_alive(pid)).then_some(())
    });

    eprintln!("step: remove, list is empty");
    rk(&root).args(["remove", &id]).assert().success();
    assert_eq!(
        json(&rk(&root).args(["list", "--json"]).assert().success()),
        json!([])
    );

    eprintln!("step: add again, start, daemon stop kills child");
    let id2 = stdout(
        &rk(&root)
            .args(["add", folder.to_str().unwrap()])
            .assert()
            .success(),
    )
    .trim()
    .to_string();
    rk(&root).args(["start", &id2]).assert().success();
    let pid2 = child_pid(&root);

    eprintln!("step: daemon stop -> exit 0, daemon and child gone");
    let out = stdout(&rk(&root).args(["daemon", "stop"]).assert().success());
    assert!(out.contains("daemon stopped"), "got: {out}");
    wait_until(
        "child gone after daemon stop",
        Duration::from_secs(5),
        || (!pid_alive(pid2)).then_some(()),
    );

    eprintln!("step: daemon status after stop -> exit 2");
    rk(&root).args(["daemon", "status"]).assert().code(2);

    eprintln!("step: service install/uninstall --dry-run in temp dir");
    let services = root.join("services");
    rk(&root)
        .args(["service", "install", "--dry-run"])
        .assert()
        .success();
    assert_eq!(std::fs::read_dir(&services).unwrap().count(), 1);
    rk(&root)
        .args(["service", "uninstall", "--dry-run"])
        .assert()
        .success();
    assert_eq!(std::fs::read_dir(&services).unwrap().count(), 0);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn daemon_started_from_shell_survives_hangup() {
    let root = fresh_root("hup");
    let _guard = DaemonGuard(root.clone());
    let bin = assert_cmd::cargo::cargo_bin("rc-keeper");

    // The shell leads its own process group. `list` auto-starts the daemon from it.
    let mut shell = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "'{}' list --json >/dev/null; exec sleep 60",
            bin.display()
        ))
        .env("RC_KEEPER_CONFIG_DIR", root.join("config"))
        .env("RC_KEEPER_SERVICE_DIR", root.join("services"))
        .process_group(0)
        .spawn()
        .unwrap();
    let shell_pid = u64::from(shell.id());

    let pid = wait_until("daemon from shell", Duration::from_secs(15), || {
        let out = rk(&root)
            .args(["daemon", "status", "--json"])
            .output()
            .ok()?;
        let v: Value = serde_json::from_slice(&out.stdout).ok()?;
        v["pid"].as_u64()
    });
    assert_ne!(
        pgid(pid),
        shell_pid,
        "daemon must not share the shell's process group"
    );
    assert_eq!(pgid(pid), pid, "daemon must lead its own session (setsid)");

    eprintln!("step: hang up the shell group, then the daemon directly");
    send_signal("HUP", &format!("-{shell_pid}"));
    send_signal("HUP", &pid.to_string());
    thread::sleep(Duration::from_millis(500));
    rk(&root).args(["daemon", "status"]).assert().success();

    rk(&root).args(["daemon", "stop"]).assert().success();
    let _ = shell.kill();
    let _ = shell.wait();
    let _ = std::fs::remove_dir_all(&root);
}
