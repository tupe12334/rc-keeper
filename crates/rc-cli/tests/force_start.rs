//! `start --force` and the force-boot flags, through the real `rc-keeper` binary. The folder is
//! a temp dir and the claude stand-in is a copy of the fake named `claude`. Unix only.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command as Std, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::assert::Assert;
use assert_cmd::Command;
use serde_json::Value;

const SESSION_URL: &str = "https://claude.ai/code/session_takeover";

/// Stops the daemon and the foreign fake when the test ends, even on panic.
struct Guard {
    root: PathBuf,
    foreign: Option<u32>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = rk(&self.root).args(["daemon", "stop"]).output();
        if let Some(pid) = self.foreign {
            let _ = Std::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

fn rk(root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rc-keeper").unwrap();
    cmd.env("RC_KEEPER_CONFIG_DIR", root.join("config"))
        .env("RC_KEEPER_SERVICE_DIR", root.join("services"));
    cmd
}

fn fresh_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rck-force-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
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

fn pid_alive(pid: u32) -> bool {
    Std::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn command_line(pid: u32) -> String {
    let out = Std::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Starts a foreign rc from `claude` in `folder`. Its shell exits, so it is re-parented to init.
fn spawn_foreign(claude: &Path, folder: &Path) -> u32 {
    let out = Std::new("sh")
        .arg("-c")
        .arg(r#""$0" remote-control >/dev/null 2>&1 & echo $!"#)
        .arg(claude)
        .current_dir(folder)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let pid: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
    wait_until("foreign exec", Duration::from_secs(5), || {
        command_line(pid)
            .contains("claude remote-control")
            .then_some(())
    });
    pid
}

fn views(root: &Path) -> Value {
    serde_json::from_slice(&rk(root).args(["list", "--json"]).output().unwrap().stdout).unwrap()
}

#[test]
fn start_force_needs_consent_then_takes_the_folder() {
    let root = fresh_root("start");
    let folder = root.join("project");
    std::fs::create_dir_all(&folder).unwrap();
    let pidfile = root.join("foreign.pid");

    // One copy of the fake, named `claude`. The instance reads --pidfile to see if it is busy.
    let claude = root.join("bin/claude");
    std::fs::create_dir_all(claude.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_fake-claude-cli"), &claude).unwrap();

    let mut guard = Guard {
        root: root.clone(),
        foreign: None,
    };
    rk(&root)
        .args(["config", "set", "claude-bin", claude.to_str().unwrap()])
        .assert()
        .success();

    eprintln!("step: add with --no-auto-start; --force-boot and --no-force-boot round-trip");
    let pidarg = format!("--pidfile={}", pidfile.display());
    let id = stdout(
        &rk(&root)
            .args([
                "add",
                folder.to_str().unwrap(),
                "--no-auto-start",
                "--",
                &pidarg,
            ])
            .assert()
            .success(),
    )
    .trim()
    .to_string();
    rk(&root)
        .args(["edit", &id, "--force-boot"])
        .assert()
        .success();
    assert_eq!(views(&root)[0]["config"]["forceBoot"], true);
    rk(&root)
        .args(["edit", &id, "--no-force-boot"])
        .assert()
        .success();
    assert_eq!(views(&root)[0]["config"]["forceBoot"], false);
    rk(&root)
        .args(["edit", &id, "--force-boot", "--no-force-boot"])
        .assert()
        .code(1);

    eprintln!("step: plain start reports folder-busy and leaves the foreign rc alive");
    let foreign = spawn_foreign(&claude, &folder);
    std::fs::write(&pidfile, foreign.to_string()).unwrap();
    guard.foreign = Some(foreign);
    rk(&root).args(["start", &id]).assert().success();
    wait_until("folder busy", Duration::from_secs(15), || {
        (views(&root)[0]["status"]["state"] == "folder-busy").then_some(())
    });
    assert!(pid_alive(foreign), "plain start stopped the foreign rc");

    eprintln!("step: --all with --force is a usage error");
    rk(&root)
        .args(["start", "--all", "--force"])
        .assert()
        .code(1);

    eprintln!("step: --yes without --force is a usage error");
    rk(&root).args(["start", &id, "--yes"]).assert().code(1);

    eprintln!("step: --force with no terminal and no --yes refuses and stops nothing");
    let refused = rk(&root).args(["start", &id, "--force"]).assert().code(1);
    let err = String::from_utf8_lossy(&refused.get_output().stderr).into_owned();
    assert!(err.contains("--yes"), "got: {err}");
    assert!(
        pid_alive(foreign),
        "refused force still stopped the foreign rc"
    );

    eprintln!("step: --force --yes previews, stops the foreign pid, and starts the instance");
    let out = stdout(
        &rk(&root)
            .args(["start", &id, "--force", "--yes"])
            .assert()
            .success(),
    );
    assert!(
        out.contains(&format!("will stop pid {foreign}")),
        "got: {out}"
    );
    assert!(
        out.contains(&format!("stopped pid {foreign}")),
        "got: {out}"
    );
    assert!(out.contains(&format!("started {id}")), "got: {out}");
    wait_until("foreign gone", Duration::from_secs(10), || {
        (!pid_alive(foreign)).then_some(())
    });

    eprintln!("step: instance reaches running with its session URL");
    wait_until("running", Duration::from_secs(20), || {
        let v = views(&root);
        (v[0]["status"]["state"] == "running" && v[0]["status"]["sessionUrls"][0] == SESSION_URL)
            .then_some(())
    });

    // Stop the daemon while its config dir still exists: the socket name is derived from it.
    drop(guard);
    let _ = std::fs::remove_dir_all(&root);
}
