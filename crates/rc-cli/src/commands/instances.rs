//! list, status, add, edit, remove.

use std::path::Path;

use anyhow::{bail, Result};
use rc_core::InstanceConfig;
use serde_json::json;

use super::selector::canonical;
use super::{connect, fetch_views, resolve};
use crate::cli::InstanceFlags;
use crate::output::{list_table, print_json, status_table};

pub fn list(json: bool) -> Result<()> {
    let mut client = connect()?;
    let views = fetch_views(&mut client)?;
    if json {
        print_json(&views)
    } else {
        println!("{}", list_table(&views));
        Ok(())
    }
}

pub fn status(json: bool) -> Result<()> {
    let mut client = connect()?;
    let views = fetch_views(&mut client)?;
    if json {
        return print_json(&views);
    }
    println!("{}", status_table(&views));
    for v in views.iter().filter(|v| !v.status.session_urls.is_empty()) {
        println!(
            "{}: {}",
            v.config.folder.display(),
            v.status.session_urls.join(" ")
        );
    }
    let version = client.ping()?["version"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    println!("daemon {version}");
    Ok(())
}

pub fn add(folder: &Path, flags: &InstanceFlags) -> Result<()> {
    let folder = canonical(folder)?;
    if !folder.is_dir() {
        bail!("not a folder: {}", folder.display());
    }
    let mut cfg = InstanceConfig {
        id: String::new(),
        folder,
        name: None,
        chrome: true,
        permission_mode: None,
        spawn: None,
        capacity: None,
        create_session_in_dir: None,
        session_name_prefix: None,
        extra_args: vec![],
        auto_trust: true,
        auto_start: true,
        force_boot: false,
    };
    flags.apply(&mut cfg);
    let mut client = connect()?;
    let id = client.call("add_instance", json!({ "cfg": cfg }))?;
    println!("{}", id.as_str().unwrap_or_default());
    Ok(())
}

pub fn edit(selector: &str, flags: &InstanceFlags) -> Result<()> {
    let mut client = connect()?;
    let mut cfg = resolve(&mut client, selector)?.config;
    flags.apply(&mut cfg);
    let id = cfg.id.clone();
    client.call("update_instance", json!({ "cfg": cfg }))?;
    println!("updated {id}");
    Ok(())
}

pub fn remove(selector: &str) -> Result<()> {
    let mut client = connect()?;
    let id = resolve(&mut client, selector)?.config.id;
    client.call("remove_instance", json!({ "id": id }))?;
    println!("removed {id}");
    Ok(())
}
