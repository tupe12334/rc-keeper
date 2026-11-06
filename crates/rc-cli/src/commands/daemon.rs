//! `daemon`, `daemon run`, `daemon stop`, `daemon status`.

use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use rc_core::update_check::update_in_progress;
use rc_core::Manager;
use rc_ipc::{config_dir, shutdown_and_exit, socket_path};
use serde_json::{json, Value};

use crate::autostart_daemon::live_client;
use crate::cli::Exit;
use crate::output::print_json;

pub fn run() -> Result<()> {
    // Bind before loading: a second daemon must fail without starting auto-start children.
    let listener = rc_ipc::bind()?;
    let manager = Manager::load(config_dir());
    let for_signal = manager.clone();
    ctrlc::set_handler(move || shutdown_and_exit(&for_signal))?;
    ignore_hangup();
    rc_ipc::serve(listener, manager)
}

/// Runs after the ctrlc handler, which also catches SIGHUP. The daemon must survive hangup.
#[cfg(unix)]
fn ignore_hangup() {
    // SAFETY: SIG_IGN is async-signal-safe and touches no memory.
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
}

#[cfg(not(unix))]
fn ignore_hangup() {}

/// Exit 0 when the daemon stopped or was not running.
pub fn stop() -> Result<()> {
    let Some(mut client) = live_client() else {
        println!("not running");
        return Ok(());
    };
    client.call("shutdown", Value::Null)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while live_client().is_some() {
        if Instant::now() > deadline {
            bail!("daemon did not exit after shutdown");
        }
        thread::sleep(Duration::from_millis(100));
    }
    println!("daemon stopped");
    Ok(())
}

/// Exit 0 when running, 2 when not.
pub fn status(json: bool) -> Result<()> {
    let socket = socket_path();
    let updating = update_in_progress(&config_dir());
    let Some(mut client) = live_client() else {
        if json {
            print_json(&json!({
                "running": false,
                "socketPath": socket,
                "updating": updating.is_some(),
                "updatingTo": updating,
            }))?;
        } else {
            println!("not running\nsocket {}", socket.display());
            if let Some(target) = updating {
                println!("updating to {target}");
            }
        }
        return Err(Exit(2).into());
    };
    let info = client.ping()?;
    if json {
        print_json(&json!({
            "running": true,
            "version": info["version"],
            "pid": info["pid"],
            "socketPath": socket,
            "updating": updating.is_some(),
            "updatingTo": updating,
        }))
    } else {
        println!(
            "running\nversion {}\npid {}\nsocket {}",
            info["version"].as_str().unwrap_or("unknown"),
            info["pid"],
            socket.display()
        );
        if let Some(target) = updating {
            println!("updating to {target}");
        }
        Ok(())
    }
}
