//! Finds the `claude` executable: settings path, then PATH, then the default install dir.
use crate::model::Settings;
use std::path::PathBuf;

pub fn find_claude_bin(settings: &Settings) -> Option<PathBuf> {
    if let Some(path) = settings.claude_bin.as_ref().filter(|p| p.exists()) {
        return Some(path.clone());
    }
    if let Ok(path) = which::which("claude") {
        return Some(path);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    let dir = home.join(".local").join("bin");
    let names: &[&str] = if cfg!(windows) {
        &["claude.exe", "claude.cmd"]
    } else {
        &["claude"]
    };
    names.iter().map(|name| dir.join(name)).find(|p| p.exists())
}
