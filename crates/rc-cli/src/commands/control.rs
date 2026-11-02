//! start, stop, restart for one instance or --all.

use std::io::{IsTerminal, Write};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::{connect, fetch_views, resolve};
use crate::cli::Target;

#[derive(Clone, Copy)]
pub enum Action {
    Start,
    Stop,
    Restart,
}

impl Action {
    fn method(self) -> &'static str {
        match self {
            Action::Start => "start_instance",
            Action::Stop => "stop_instance",
            Action::Restart => "restart_instance",
        }
    }

    fn past(self) -> &'static str {
        match self {
            Action::Start => "started",
            Action::Stop => "stopped",
            Action::Restart => "restarted",
        }
    }
}

pub fn run(action: Action, target: &Target) -> Result<()> {
    let mut client = connect()?;
    let ids: Vec<String> = if target.all {
        fetch_views(&mut client)?
            .into_iter()
            .map(|v| v.config.id)
            .collect()
    } else {
        let Some(selector) = &target.selector else {
            bail!("give an instance or --all")
        };
        vec![resolve(&mut client, selector)?.config.id]
    };
    for id in ids {
        client.call(action.method(), json!({ "id": id }))?;
        println!("{} {id}", action.past());
    }
    Ok(())
}

/// `start --force`: previews what it stops, asks first unless `yes`, then stops the other claude
/// rc in the folder and starts the instance.
pub fn force_start(target: &Target, yes: bool) -> Result<()> {
    if target.all {
        bail!("--force takes one instance, not --all")
    }
    let Some(selector) = &target.selector else {
        bail!("give an instance or --all")
    };
    let mut client = connect()?;
    let id = resolve(&mut client, selector)?.config.id;
    let preview = client.call("preview_force_start", json!({ "id": id }))?;
    let pids: Vec<u64> = preview["stopped"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .collect();
    let risk = preview["respawnRisk"].as_str().map(str::to_owned);
    if !pids.is_empty() {
        for pid in &pids {
            println!("will stop pid {pid}");
        }
        if let Some(risk) = &risk {
            eprintln!("warning: a supervisor may start the stopped rc again: {risk}. Disable it, or the folder is taken again.");
        }
        if !yes && !confirm(&format!("Stop these processes and start {id}?"))? {
            bail!("cancelled; nothing stopped")
        }
    }
    let taken = client.call("force_start", json!({ "id": id }))?;
    for pid in taken["stopped"].as_array().into_iter().flatten() {
        println!("stopped pid {pid}");
    }
    println!("started {id}");
    Ok(())
}

/// Asks on stderr and reads the answer from stdin. Errors when no terminal is attached.
fn confirm(question: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        bail!("no terminal to confirm on; pass --yes to stop without asking")
    }
    eprint!("{question} [y/N] ");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}
