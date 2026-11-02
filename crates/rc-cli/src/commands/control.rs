//! start, stop, restart for one instance or --all.

use anyhow::{bail, Result};
use serde_json::json;

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
