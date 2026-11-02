//! systemd user unit (Linux).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::{exe, print_daemon_state, run_cmd, service_dir, write_file};

pub const UNIT_FILE: &str = "rc-keeper.service";

/// `ExecStart` value: quoted, with systemd specifiers (`%`) and variables (`$`) escaped.
pub fn exec_start(exe: &Path) -> String {
    let mut out = String::from("\"");
    for c in exe.display().to_string().chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '%' => out.push_str("%%"),
            '$' => out.push_str("$$"),
            c => out.push(c),
        }
    }
    out.push_str("\" daemon");
    out
}

pub fn unit_text(exe: &Path) -> String {
    let path = std::iter::once("%h/.local/bin")
        .chain(super::PATH_TAIL)
        .collect::<Vec<_>>()
        .join(":");
    format!(
        "[Unit]\nDescription=rc-keeper daemon (claude remote-control)\n\n\
         [Service]\nExecStart={}\nEnvironment=PATH={path}\nRestart=on-failure\nRestartSec=5\n\n\
         [Install]\nWantedBy=default.target\n",
        exec_start(exe)
    )
}

fn unit_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("no home directory")?;
    Ok(service_dir(home.join(".config/systemd/user")).join(UNIT_FILE))
}

pub fn install(dry_run: bool) -> Result<()> {
    let path = unit_path()?;
    write_file(&path, &unit_text(&exe()?))?;
    println!("wrote {}", path.display());
    if dry_run {
        println!("dry run: skipped systemctl");
    } else {
        run_cmd("systemctl", &["--user", "daemon-reload"])?;
        run_cmd("systemctl", &["--user", "enable", "--now", UNIT_FILE])?;
    }
    let user = std::env::var("USER").unwrap_or_else(|_| "$USER".into());
    println!("To run without a login session, run:\n  loginctl enable-linger {user}");
    Ok(())
}

pub fn uninstall(dry_run: bool) -> Result<()> {
    let path = unit_path()?;
    if !dry_run {
        // ponytail: disable fails when the unit is absent. Ignore that.
        let _ = run_cmd("systemctl", &["--user", "disable", "--now", UNIT_FILE]);
    }
    if path.exists() {
        std::fs::remove_file(&path)?;
        println!("removed {}", path.display());
    } else {
        println!("not installed");
    }
    if !dry_run {
        run_cmd("systemctl", &["--user", "daemon-reload"])?;
    }
    Ok(())
}

pub fn status() -> Result<()> {
    let path = unit_path()?;
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
    use super::{exec_start, unit_text};
    use std::path::Path;

    #[test]
    fn unit_restarts_only_on_failure_so_stop_sticks() {
        let text = unit_text(Path::new("/usr/local/bin/rc-keeper"));
        assert!(text.contains("ExecStart=\"/usr/local/bin/rc-keeper\" daemon\n"));
        assert!(text.contains("Restart=on-failure\n"));
        assert!(!text.contains("Restart=always"));
        assert!(text.contains("RestartSec=5\n"));
        assert!(text.contains("WantedBy=default.target\n"));
    }

    #[test]
    fn unit_sets_path_for_claude() {
        let text = unit_text(Path::new("/usr/local/bin/rc-keeper"));
        assert!(text.contains(
            "Environment=PATH=%h/.local/bin:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin\n"
        ));
    }

    #[test]
    fn exec_start_quotes_and_escapes_specials() {
        assert_eq!(
            exec_start(Path::new("/opt/rc keeper")),
            "\"/opt/rc keeper\" daemon"
        );
        assert_eq!(exec_start(Path::new("/a%b")), "\"/a%%b\" daemon");
        assert_eq!(exec_start(Path::new("/a$b")), "\"/a$$b\" daemon");
        assert_eq!(exec_start(Path::new("/a\"b\\c")), "\"/a\\\"b\\\\c\" daemon");
    }
}
