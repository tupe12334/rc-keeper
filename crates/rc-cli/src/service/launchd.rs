//! launchd agent (macOS).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::{exe, print_daemon_state, run_cmd, service_dir, write_file, xml_escape};
use rc_ipc::config_dir;

pub const LABEL: &str = "io.github.tupe12334.rckeeper.daemon";

/// Restarts only after a failed exit. A clean exit from `daemon stop` stays stopped.
pub fn plist_text(exe: &Path, log: &Path, home: &Path) -> String {
    let exe = xml_escape(&exe.display().to_string());
    let log = xml_escape(&log.display().to_string());
    let local_bin = format!("{}/.local/bin", home.display());
    let path = std::iter::once(local_bin.as_str())
        .chain(super::PATH_TAIL)
        .collect::<Vec<_>>()
        .join(":");
    let path = xml_escape(&path);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>daemon</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>{path}</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#
    )
}

fn plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("no home directory")?;
    Ok(service_dir(home.join("Library/LaunchAgents")).join(format!("{LABEL}.plist")))
}

fn uid() -> Result<String> {
    let out = Command::new("id")
        .arg("-u")
        .output()
        .context("cannot run id -u")?;
    if !out.status.success() {
        bail!("id -u failed");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn install(dry_run: bool) -> Result<()> {
    let path = plist_path()?;
    let home = dirs::home_dir().context("no home directory")?;
    write_file(
        &path,
        &plist_text(&exe()?, &config_dir().join("daemon.log"), &home),
    )?;
    println!("wrote {}", path.display());
    if dry_run {
        println!("dry run: skipped launchctl");
        return Ok(());
    }
    let uid = uid()?;
    // ponytail: bootout fails when not loaded. Ignore, so reinstall works.
    let _ = run_cmd("launchctl", &["bootout", &format!("gui/{uid}/{LABEL}")]);
    run_cmd(
        "launchctl",
        &["bootstrap", &format!("gui/{uid}"), &path.to_string_lossy()],
    )
}

pub fn uninstall(dry_run: bool) -> Result<()> {
    let path = plist_path()?;
    if !dry_run {
        let uid = uid()?;
        // ponytail: bootout fails when not loaded. Ignore.
        let _ = run_cmd("launchctl", &["bootout", &format!("gui/{uid}/{LABEL}")]);
    }
    if path.exists() {
        std::fs::remove_file(&path)?;
        println!("removed {}", path.display());
    } else {
        println!("not installed");
    }
    Ok(())
}

pub fn status() -> Result<()> {
    let path = plist_path()?;
    if path.exists() {
        println!("service: installed ({})", path.display());
    } else {
        println!("service: not installed");
    }
    print_daemon_state();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{plist_text, LABEL};
    use std::path::Path;

    fn text() -> String {
        plist_text(
            Path::new("/Applications/rc & keeper"),
            Path::new("/tmp/daemon.log"),
            Path::new("/Users/me"),
        )
    }

    #[test]
    fn plist_runs_daemon_at_load() {
        let t = text();
        assert!(t.contains(&format!("<string>{LABEL}</string>")));
        assert!(t.contains(
            "<string>/Applications/rc &amp; keeper</string>\n    <string>daemon</string>"
        ));
        assert!(t.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(t.contains("<string>/tmp/daemon.log</string>"));
    }

    #[test]
    fn keepalive_restarts_only_on_failure_so_stop_sticks() {
        let t = text();
        assert!(t.contains("<key>KeepAlive</key>\n  <dict>\n    <key>SuccessfulExit</key>\n    <false/>\n  </dict>"));
        assert!(!t.contains("<key>KeepAlive</key>\n  <true/>"));
    }

    #[test]
    fn plist_sets_path_with_absolute_home_bin() {
        let t = text();
        assert!(t.contains(
            "<key>PATH</key>\n    <string>/Users/me/.local/bin:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string>"
        ));
    }
}
