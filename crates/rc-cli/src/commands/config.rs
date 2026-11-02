//! config get | set claude-bin <path>

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::connect;

pub fn get() -> Result<()> {
    let mut client = connect()?;
    let settings = client.call("get_settings", Value::Null)?;
    println!(
        "claude-bin = {}",
        settings["claudeBin"].as_str().unwrap_or("(not set)")
    );
    Ok(())
}

pub fn set(key: &str, value: &str) -> Result<()> {
    if key != "claude-bin" {
        bail!("unknown key `{key}` (supported: claude-bin)");
    }
    let mut client = connect()?;
    client.call("set_settings", json!({ "claudeBin": value }))?;
    println!("claude-bin = {value}");
    Ok(())
}
