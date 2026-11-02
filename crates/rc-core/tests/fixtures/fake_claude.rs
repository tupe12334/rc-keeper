//! Test stand-in for `claude remote-control`. Tests copy it to a temp folder as `claude` (or
//! `claude.exe` on Windows), so the matcher sees a program with that name. Never run in a real
//! folder.
//!
//! Flags:
//! - `--pidfile=PATH`: when the pid in PATH is alive, print the folder-busy notice and exit 1.
//! - `--pidout=PATH`: write this process's pid to PATH. With `--spawn-sdk`, also write the
//!   session child's pid to `PATH.child`.
//! - `--spawn-sdk`: start one session child (`--sdk-url`) and leave it running.
//! - `--late-busy`: print a banner, wait 2 s, print the folder-busy notice, and exit 1.
//! - `--sdk-url ...`: a session child. It only idles.
//!
//! Otherwise it prints a session URL and idles until killed.
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const BUSY: &str = "Error: This folder is already served by a terminal `claude remote-control`.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(name))
            .map(str::to_owned)
    };

    if args.iter().any(|a| a == "--sdk-url") {
        idle();
    }
    if let Some(path) = flag("--pidfile=") {
        let pid = std::fs::read_to_string(path).unwrap_or_default();
        if is_alive(pid.trim()) {
            eprintln!("{BUSY}");
            std::process::exit(1);
        }
    }
    if args.iter().any(|a| a == "--late-busy") {
        println!("Remote Control starting");
        thread::sleep(Duration::from_secs(2));
        eprintln!("{BUSY}");
        std::process::exit(1);
    }
    if let Some(path) = flag("--pidout=") {
        std::fs::write(&path, std::process::id().to_string()).unwrap();
        if args.iter().any(|a| a == "--spawn-sdk") {
            // The child runs until a test kills it. Nothing waits on it on purpose.
            #[allow(clippy::zombie_processes)]
            let child = Command::new(std::env::current_exe().unwrap())
                .args(["--sdk-url", "https://sdk.example"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            std::fs::write(format!("{path}.child"), child.id().to_string()).unwrap();
        }
    }
    println!("Remote Control ready https://claude.ai/code/session_takeover");
    std::io::stdout().flush().unwrap();
    idle();
}

fn idle() -> ! {
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

/// True while the pid names a live process. Unix: `kill -0`. Windows: `tasklist` lists it.
fn is_alive(pid: &str) -> bool {
    if pid.is_empty() {
        return false;
    }
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", pid])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
    }
}
