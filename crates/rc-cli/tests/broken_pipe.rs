//! A client command whose reader closes early exits quietly instead of panicking.
#![cfg(unix)]

use std::process::{Command, Stdio};

#[test]
fn version_into_closed_pipe_does_not_panic() {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin("rc-keeper"))
        .arg("version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Close the read end before the CLI writes, so its write hits EPIPE.
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("Broken pipe"), "panicked: {err}");
    assert_ne!(out.status.code(), Some(101), "panic exit code");
}
