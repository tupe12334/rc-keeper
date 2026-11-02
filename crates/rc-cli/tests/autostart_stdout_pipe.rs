//! A command that auto-starts the daemon must not keep the caller's stdout pipe open.
//! Regression: on Windows the daemon inherited the CLI's stdout pipe, so
//! `(rc-keeper status --json) | Out-String` in pwsh waited for EOF forever.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[test]
fn autostart_releases_caller_stdout_pipe() {
    let root = std::env::temp_dir().join(format!("rck-pipe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let bin = assert_cmd::cargo::cargo_bin("rc-keeper");
    let run = |args: &[&str]| {
        let mut cmd = Command::new(&bin);
        cmd.args(args)
            .env("RC_KEEPER_CONFIG_DIR", root.join("config"))
            .env("RC_KEEPER_SERVICE_DIR", root.join("services"));
        cmd
    };

    let mut child = run(&["status", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut pipe = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut out = Vec::new();
        let _ = pipe.read_to_end(&mut out);
        let _ = tx.send(out);
    });
    // EOF arrives only when every holder of the pipe's write end has closed it.
    let got = rx.recv_timeout(Duration::from_secs(30));
    let _ = child.wait();
    let _ = run(&["daemon", "stop"]).stdout(Stdio::null()).status();
    let _ = std::fs::remove_dir_all(&root);

    let out = got.expect("stdout pipe stayed open: the auto-started daemon holds it");
    let v: serde_json::Value = serde_json::from_slice(&out).expect("stdout is JSON");
    assert_eq!(v, serde_json::json!([]));
}
