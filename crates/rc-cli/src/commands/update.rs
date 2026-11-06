//! `update`: reports the latest release, or installs it with install.sh and restarts the daemon.

use std::fs;
use std::process::Command;

use anyhow::{bail, Context, Result};
use rc_core::update_check::{begin_update, is_newer, latest_tag, update_in_progress, REPO};
use rc_ipc::config_dir;
use serde_json::json;

use crate::autostart_daemon::{connect_or_start, live_client};
use crate::commands::daemon;
use crate::output::print_json;
use crate::service;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run(check: bool, json: bool, gui: bool) -> Result<()> {
    let dir = config_dir();
    let latest = latest_tag().map_err(anyhow::Error::msg)?;
    let available = is_newer(&latest, VERSION);
    if check {
        let updating = update_in_progress(&dir);
        if json {
            return print_json(&json!({
                "current": VERSION,
                "latest": latest,
                "updateAvailable": available,
                "updating": updating.is_some(),
                "updatingTo": updating,
            }));
        }
        println!("current {VERSION}\nlatest {latest}");
        match (updating, available) {
            (Some(target), _) => println!("updating to {target}"),
            (None, true) => println!("update available. Run `rc-keeper update`."),
            (None, false) => println!("up to date"),
        }
        return Ok(());
    }
    if !available {
        println!("rc-keeper {VERSION} is up to date");
        return Ok(());
    }
    // The marker shows "updating" in `daemon status` until this function returns.
    let _marker = begin_update(&dir, &latest).map_err(anyhow::Error::msg)?;
    println!("updating rc-keeper {VERSION} to {latest}");
    run_install(&latest, gui)?;
    restart_daemon()?;
    println!("updated to {latest}");
    Ok(())
}

/// Runs the release's install.sh with its tag pinned, as a manual install does.
fn run_install(tag: &str, gui: bool) -> Result<()> {
    if cfg!(windows) {
        bail!("rc-keeper update uses install.sh. On Windows, run install.ps1 instead.");
    }
    let url = format!("https://raw.githubusercontent.com/{REPO}/{tag}/install.sh");
    let script = std::env::temp_dir().join(format!("rc-keeper-install-{}.sh", std::process::id()));
    let download = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&script)
        .arg(&url)
        .status()
        .context("cannot run curl")?;
    if !download.success() {
        bail!("could not download {url}");
    }
    let mode = if gui { "--gui" } else { "--cli" };
    let status = Command::new("sh")
        .arg(&script)
        .arg(mode)
        .env("RC_KEEPER_VERSION", tag)
        .status();
    let _ = fs::remove_file(&script);
    let status = status.context("cannot run sh")?;
    if !status.success() {
        bail!("install.sh failed for {tag}");
    }
    Ok(())
}

/// Restarts a running daemon so it runs the new binary. Uses the login service when one is installed.
fn restart_daemon() -> Result<()> {
    if service::restart()? {
        return Ok(());
    }
    if live_client().is_none() {
        return Ok(());
    }
    daemon::stop()?;
    connect_or_start()?;
    Ok(())
}
