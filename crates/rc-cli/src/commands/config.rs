//! config get | set claude-bin `<path>` | set keep-awake on|off

use anyhow::{bail, Result};
use rc_ipc::config_dir;
use serde_json::{json, Value};

use super::connect;

pub fn get() -> Result<()> {
    let mut client = connect()?;
    let settings = client.call("get_settings", Value::Null)?;
    println!(
        "claude-bin = {}",
        settings["claudeBin"].as_str().unwrap_or("(not set)")
    );
    println!("keep-awake = {}", keep_awake_text(&settings));
    Ok(())
}

/// `on`, `off`, or `on (not active: ...)` when the OS refused the sleep block.
fn keep_awake_text(settings: &Value) -> &'static str {
    match (
        settings["keepAwake"].as_bool().unwrap_or(false),
        settings["keepingAwake"].as_bool().unwrap_or(false),
    ) {
        (false, _) => "off",
        (true, true) => "on",
        (true, false) => "on (not active: the OS refused the sleep block, see daemon.log)",
    }
}

fn parse_switch(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => Ok(true),
        "off" | "false" | "no" | "0" => Ok(false),
        _ => bail!("`keep-awake` takes `on` or `off`, not `{value}`"),
    }
}

pub fn set(key: &str, value: &str) -> Result<()> {
    match key {
        "claude-bin" => {
            let mut client = connect()?;
            client.call("set_settings", json!({ "claudeBin": value }))?;
            println!("claude-bin = {value}");
        }
        "keep-awake" => {
            let on = parse_switch(value)?;
            let mut client = connect()?;
            // `claudeBin` absent would clear it, so send the current value back.
            let current = client.call("get_settings", Value::Null)?;
            client.call(
                "set_settings",
                json!({ "claudeBin": current["claudeBin"], "keepAwake": on }),
            )?;
            let settings = client.call("get_settings", Value::Null)?;
            println!("keep-awake = {}", keep_awake_text(&settings));
        }
        _ => bail!("unknown key `{key}` (supported: claude-bin, keep-awake)"),
    }
    Ok(())
}

/// Prints the config dir. Does not contact the daemon.
pub fn path() -> Result<()> {
    println!("{}", config_dir().display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_words() {
        assert!(parse_switch("on").unwrap());
        assert!(parse_switch("TRUE").unwrap());
        assert!(!parse_switch("off").unwrap());
        assert!(parse_switch("maybe").is_err());
    }

    #[test]
    fn keep_awake_states() {
        assert_eq!(keep_awake_text(&json!({})), "off");
        assert_eq!(
            keep_awake_text(&json!({"keepAwake": true, "keepingAwake": true})),
            "on"
        );
        assert!(keep_awake_text(&json!({"keepAwake": true})).starts_with("on (not active"));
    }
}
