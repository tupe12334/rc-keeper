//! One module per subcommand group.

pub mod config;
pub mod control;
pub mod daemon;
pub mod instances;
pub mod logs;
pub mod selector;

use anyhow::Result;
use rc_core::InstanceView;
use rc_ipc::Client;
use serde_json::Value;

use crate::autostart_daemon::connect_or_start;

pub fn fetch_views(client: &mut Client) -> Result<Vec<InstanceView>> {
    Ok(serde_json::from_value(
        client.call("list_instances", Value::Null)?,
    )?)
}

/// Resolves `<id|folder>` to one instance.
pub fn resolve(client: &mut Client, selector: &str) -> Result<InstanceView> {
    let views = fetch_views(client)?;
    let folder = selector::folder_of(selector);
    selector::select(views, selector, Some(&folder))
}

pub fn connect() -> Result<Client> {
    connect_or_start()
}
