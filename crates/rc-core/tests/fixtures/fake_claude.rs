//! Test stand-in for `claude remote-control`. Tests copy it to a folder as `claude`, so the
//! matcher sees a program named `claude`. Never run against a real folder.
//!
//! `--pidfile=PATH`: when the pid in PATH is alive, print the folder-busy notice and exit 1.
//! Otherwise print a session URL and sleep until killed.
use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(path) = args.iter().find_map(|a| a.strip_prefix("--pidfile=")) {
        let pid = std::fs::read_to_string(path).unwrap_or_default();
        let pid = pid.trim();
        let alive = !pid.is_empty()
            && Command::new("kill")
                .args(["-0", pid])
                .status()
                .is_ok_and(|s| s.success());
        if alive {
            eprintln!(
                "Error: This folder is already served by a terminal `claude remote-control`."
            );
            std::process::exit(1);
        }
    }
    println!("Remote Control ready https://claude.ai/code/session_takeover");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
