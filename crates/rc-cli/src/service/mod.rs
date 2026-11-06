//! Login-service install/uninstall/status. Platform files live in the sibling modules.

pub mod launchd;
pub mod systemd;
pub mod windows;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use rc_core::update_check::update_in_progress;
use rc_ipc::config_dir;

use crate::autostart_daemon::live_client;

/// `RC_KEEPER_SERVICE_DIR` overrides the default service file directory (tests).
pub fn service_dir(default: PathBuf) -> PathBuf {
    std::env::var_os("RC_KEEPER_SERVICE_DIR")
        .map(PathBuf::from)
        .unwrap_or(default)
}

pub fn exe() -> Result<PathBuf> {
    std::env::current_exe().context("cannot find rc-keeper binary path")
}

/// Runs a command with inherited stdio. Errors on non-zero exit.
pub fn run_cmd(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("cannot run {program}"))?;
    if !status.success() {
        bail!("`{program} {}` failed", args.join(" "));
    }
    Ok(())
}

/// PATH for the service. `claude` lives in one of these, or in `~/.local/bin`.
pub const PATH_TAIL: [&str; 4] = ["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin", "/bin"];

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn write_file(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
}

pub fn print_daemon_state() {
    match live_client().map(|mut c| c.ping()) {
        Some(Ok(info)) => println!(
            "daemon: running (version {})",
            info["version"].as_str().unwrap_or("unknown")
        ),
        _ => println!("daemon: stopped"),
    }
    if let Some(target) = update_in_progress(&config_dir()) {
        println!("daemon: updating to {target}");
    }
}

/// Restarts the login-service daemon so it runs the new binary. False when no service is installed.
pub fn restart() -> Result<bool> {
    if cfg!(target_os = "macos") {
        launchd::restart()
    } else if cfg!(target_os = "linux") {
        systemd::restart()
    } else {
        Ok(false)
    }
}

type Op = fn(bool) -> Result<()>;

struct Ops {
    install: Op,
    uninstall: Op,
    status: fn() -> Result<()>,
}

fn ops() -> Result<Ops> {
    if cfg!(target_os = "linux") {
        Ok(Ops {
            install: systemd::install,
            uninstall: systemd::uninstall,
            status: systemd::status,
        })
    } else if cfg!(target_os = "macos") {
        Ok(Ops {
            install: launchd::install,
            uninstall: launchd::uninstall,
            status: launchd::status,
        })
    } else if cfg!(windows) {
        Ok(Ops {
            install: windows::install,
            uninstall: windows::uninstall,
            status: windows::status,
        })
    } else {
        bail!("no login service support on this OS")
    }
}

pub fn install(dry_run: bool) -> Result<()> {
    (ops()?.install)(dry_run)
}

pub fn uninstall(dry_run: bool) -> Result<()> {
    (ops()?.uninstall)(dry_run)
}

pub fn status() -> Result<()> {
    (ops()?.status)()
}
