//! Task Scheduler task that starts the daemon at logon (Windows).

use std::path::Path;
use std::process::Command;

use anyhow::Result;

use super::{exe, print_daemon_state, run_cmd};

pub const TASK: &str = "rc-keeper";

pub fn create_args(exe: &Path) -> Vec<String> {
    let tr = format!("\"{}\" daemon", exe.display());
    [
        "/Create",
        "/SC",
        "ONLOGON",
        "/TN",
        TASK,
        "/TR",
        tr.as_str(),
        "/F",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn install(dry_run: bool) -> Result<()> {
    let args = create_args(&exe()?);
    if dry_run {
        println!("dry run: schtasks {}", args.join(" "));
        return Ok(());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_cmd("schtasks", &refs)?;
    run_cmd("schtasks", &["/Run", "/TN", TASK])
}

pub fn uninstall(dry_run: bool) -> Result<()> {
    if dry_run {
        println!("dry run: schtasks /Delete /TN {TASK} /F");
        return Ok(());
    }
    run_cmd("schtasks", &["/Delete", "/TN", TASK, "/F"])
}

pub fn status() -> Result<()> {
    let installed = Command::new("schtasks")
        .args(["/Query", "/TN", TASK])
        .output()
        .is_ok_and(|o| o.status.success());
    println!(
        "service: {}",
        if installed {
            "installed (Task Scheduler)"
        } else {
            "not installed"
        }
    );
    print_daemon_state();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::create_args;
    use std::path::Path;

    #[test]
    fn create_task_at_logon_with_quoted_exe() {
        let args = create_args(Path::new(r"C:\Program Files\rc-keeper.exe"));
        assert_eq!(
            &args[..6],
            ["/Create", "/SC", "ONLOGON", "/TN", "rc-keeper", "/TR"]
        );
        assert_eq!(args[6], r#""C:\Program Files\rc-keeper.exe" daemon"#);
    }
}
