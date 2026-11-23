//! Task Scheduler task that starts the daemon at logon, restarts it on failure and has no time limit.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use super::{exe, print_daemon_state, run_cmd, service_dir, xml_escape};
use rc_ipc::config_dir;

pub const TASK: &str = "rc-keeper";

/// Task definition. `ExecutionTimeLimit` PT0S means no limit. The default is 72 h.
pub fn task_xml(exe: &Path) -> String {
    let command = xml_escape(&exe.display().to_string());
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>999</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <Arguments>daemon</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

fn xml_path() -> PathBuf {
    service_dir(config_dir()).join("rc-keeper-task.xml")
}

/// Writes the task XML as UTF-16 LE with a BOM, which schtasks reads.
fn write_task_file(path: &Path, text: &str) -> Result<()> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))
}

pub fn create_args(xml: &Path) -> Vec<String> {
    [
        "/Create",
        "/TN",
        TASK,
        "/XML",
        xml.to_str().unwrap_or_default(),
        "/F",
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}

pub fn install(dry_run: bool) -> Result<()> {
    let path = xml_path();
    write_task_file(&path, &task_xml(&exe()?))?;
    println!("wrote {}", path.display());
    let args = create_args(&path);
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
    use super::{create_args, task_xml};
    use std::path::Path;

    #[test]
    fn task_has_no_time_limit_and_restarts_on_failure() {
        let xml = task_xml(Path::new(r"C:\Tools\rc-keeper.exe"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains(
            "<RestartOnFailure>\n      <Interval>PT1M</Interval>\n      <Count>999</Count>"
        ));
        assert!(xml.contains("<LogonTrigger>"));
    }

    #[test]
    fn task_runs_daemon_argument_and_escapes_path() {
        let xml = task_xml(Path::new(r"C:\R&D\rc-keeper.exe"));
        assert!(xml.contains(r"<Command>C:\R&amp;D\rc-keeper.exe</Command>"));
        assert!(xml.contains("<Arguments>daemon</Arguments>"));
    }

    #[test]
    fn create_imports_the_xml_file() {
        let args = create_args(Path::new(r"C:\x\rc-keeper-task.xml"));
        assert_eq!(&args[..4], ["/Create", "/TN", "rc-keeper", "/XML"]);
        assert_eq!(args[4], r"C:\x\rc-keeper-task.xml");
    }
}
