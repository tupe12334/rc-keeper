//! Finds the `claude` executable: settings path, then the newest `claude` on PATH or in the default install dir.
use crate::model::Settings;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn find_claude_bin(settings: &Settings) -> Option<PathBuf> {
    if let Some(path) = settings.claude_bin.as_ref().filter(|p| p.exists()) {
        return Some(path.clone());
    }
    let mut candidates: Vec<PathBuf> = which::which_all("claude").into_iter().flatten().collect();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let dir = PathBuf::from(home).join(".local").join("bin");
        let names: &[&str] = if cfg!(windows) {
            &["claude.exe", "claude.cmd"]
        } else {
            &["claude"]
        };
        candidates.extend(
            names
                .iter()
                .map(|name| dir.join(name))
                .filter(|p| p.exists()),
        );
    }
    newest(candidates)
}

/// A stale install earlier on PATH (say an old npm global in /usr/local/bin) lacks newer
/// `remote-control` flags, so the first match is not good enough. Ties keep PATH order.
fn newest(mut candidates: Vec<PathBuf>) -> Option<PathBuf> {
    let mut seen = Vec::new();
    candidates.retain(|p| {
        let key = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
        let fresh = !seen.contains(&key);
        seen.push(key);
        fresh
    });
    if candidates.len() < 2 {
        return candidates.pop();
    }
    candidates.into_iter().rev().max_by_key(|p| version(p))
}

/// A binary that hangs on `--version` ranks lowest after 5 s instead of blocking the start.
fn version(bin: &Path) -> Option<Vec<u64>> {
    let mut child = Command::new(bin)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().ok()?.is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    parse_version(&out)
}

/// `2.1.306 (Claude Code)` gives `[2, 1, 306]`.
fn parse_version(text: &str) -> Option<Vec<u64>> {
    let word = text.split_whitespace().next()?;
    word.split('.').map(|n| n.parse().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_claude_version() {
        assert_eq!(
            parse_version("2.1.306 (Claude Code)\n"),
            Some(vec![2, 1, 306])
        );
        assert_eq!(parse_version("garbage"), None);
        assert_eq!(parse_version(""), None);
    }

    #[cfg(unix)]
    #[test]
    fn picks_newest_over_path_order() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("rc-locate-{}", std::process::id()));
        let fake = |name: &str, out: &str| {
            let sub = dir.join(name);
            std::fs::create_dir_all(&sub).unwrap();
            let bin = sub.join("claude");
            std::fs::write(&bin, format!("#!/bin/sh\necho '{out}'\n")).unwrap();
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
            bin
        };
        let old = fake("old", "2.1.220 (Claude Code)");
        let new = fake("new", "2.1.306 (Claude Code)");
        let broken = fake("broken", "nope");
        assert_eq!(newest(vec![old.clone(), new.clone(), broken]), Some(new));
        let same = fake("same", "2.1.220 (Claude Code)");
        assert_eq!(newest(vec![old.clone(), same]), Some(old.clone()));
        // Would claim 9.9.9, but only after the 5 s deadline.
        let hung = fake("hung", "x'; sleep 30; echo '9.9.9");
        assert_eq!(newest(vec![hung, old.clone()]), Some(old.clone()));
        assert_eq!(newest(vec![old.clone(), old.clone()]), Some(old));
        assert_eq!(newest(vec![]), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
