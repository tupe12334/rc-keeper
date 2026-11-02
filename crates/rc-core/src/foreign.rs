//! Finds the foreign `claude remote-control` processes that serve one folder (CONTRACT.md v4).
//! The matching is pure over a process table, so the rules are unit-tested with fake tables.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Argument that marks a remote-control session child.
const SESSION_FLAG: &str = "--sdk-url";
const INIT_PID: u32 = 1;
const INTERPRETERS: [&str; 6] = ["sh", "bash", "zsh", "node", "bun", "python3"];

/// One process as the matcher sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub cmd: Vec<String>,
    pub cwd: Option<PathBuf>,
}

/// What a takeover would stop, and the supervisor that may start it again.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Foreign rc pids first, then their `--sdk-url` children.
    pub victims: Vec<u32>,
    pub respawn_risk: Option<String>,
}

/// Reads the whole process table, with command lines and working dirs.
pub fn snapshot() -> Vec<ProcInfo> {
    let Ok(mut sys) = System::new() else {
        return Vec::new();
    };
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_cwd(UpdateKind::Always),
    );
    sys.processes()
        .iter()
        .map(|(pid, p)| ProcInfo {
            pid: pid.as_u32(),
            parent: p.parent().map(|parent| parent.as_u32()),
            cmd: p
                .cmd()
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
            cwd: p.cwd().map(Path::to_path_buf),
        })
        .collect()
}

/// Plans a takeover of `folder` for the daemon `daemon`. Stops nothing.
pub fn plan(table: &[ProcInfo], folder: &Path, daemon: u32) -> Plan {
    let by_pid: HashMap<u32, &ProcInfo> = table.iter().map(|p| (p.pid, p)).collect();
    let foreign = foreign_pids(table, folder, daemon);
    if foreign.is_empty() {
        return Plan::default();
    }
    let mut victims = foreign.clone();
    for &root in &foreign {
        for child in session_children(table, root) {
            if !victims.contains(&child) {
                victims.push(child);
            }
        }
    }
    // Never stop anything under the daemon, even a session child that matches.
    victims.retain(|&pid| pid != daemon && !is_under(&by_pid, pid, daemon));

    let mut risks: Vec<String> = Vec::new();
    for &pid in &foreign {
        if let Some(risk) = respawn_risk(&by_pid, pid).filter(|risk| !risks.contains(risk)) {
            risks.push(risk);
        }
    }
    Plan {
        victims,
        respawn_risk: (!risks.is_empty()).then(|| risks.join(", ")),
    }
}

/// Pids that run `claude remote-control` (or `rc`) in `folder`, outside the daemon's tree.
pub fn foreign_pids(table: &[ProcInfo], folder: &Path, daemon: u32) -> Vec<u32> {
    let folder = canonical(folder);
    let by_pid: HashMap<u32, &ProcInfo> = table.iter().map(|p| (p.pid, p)).collect();
    table
        .iter()
        .filter(|p| p.pid != daemon)
        .filter(|p| is_remote_control(&p.cmd))
        .filter(|p| p.cwd.as_deref().is_some_and(|cwd| canonical(cwd) == folder))
        .filter(|p| !is_under(&by_pid, p.pid, daemon))
        .map(|p| p.pid)
        .collect()
}

/// True when the command line runs `claude remote-control` or `claude rc`. Also matches a
/// shell script or a node launcher that runs `claude` as its first argument.
pub fn is_remote_control(cmd: &[String]) -> bool {
    let Some(at) = claude_index(cmd) else {
        return false;
    };
    cmd.get(at + 1)
        .is_some_and(|sub| sub == "remote-control" || sub == "rc")
}

fn claude_index(cmd: &[String]) -> Option<usize> {
    let first = cmd.first()?;
    if is_claude(first) {
        return Some(0);
    }
    let interpreter = INTERPRETERS.contains(&base(first));
    (interpreter && cmd.get(1).is_some_and(|arg| is_claude(arg))).then_some(1)
}

fn is_claude(arg: &str) -> bool {
    matches!(base(arg), "claude" | "claude.exe" | "claude.cmd")
}

fn base(arg: &str) -> &str {
    arg.rsplit(['/', '\\']).next().unwrap_or(arg)
}

/// True when `pid` sits below `root` in the parent chain. Guards against parent loops.
fn is_under(by_pid: &HashMap<u32, &ProcInfo>, pid: u32, root: u32) -> bool {
    let mut current = pid;
    for _ in 0..=by_pid.len() {
        let Some(process) = by_pid.get(&current) else {
            return false;
        };
        match process.parent {
            Some(parent) if parent == root => return true,
            Some(parent) if parent != current => current = parent,
            _ => return false,
        }
    }
    false
}

/// Every descendant of `root` whose command line carries the session flag.
fn session_children(table: &[ProcInfo], root: u32) -> Vec<u32> {
    let mut found = Vec::new();
    let mut queue = vec![root];
    while let Some(parent) = queue.pop() {
        for child in table.iter().filter(|p| p.parent == Some(parent)) {
            if found.contains(&child.pid) || child.pid == root {
                continue;
            }
            found.push(child.pid);
            queue.push(child.pid);
        }
    }
    found
        .into_iter()
        .filter(|pid| {
            table
                .iter()
                .find(|p| p.pid == *pid)
                .is_some_and(|p| p.cmd.iter().any(|arg| arg.contains(SESSION_FLAG)))
        })
        .collect()
}

/// Names the supervisor that would start `pid` again, or None when nothing restarts it.
fn respawn_risk(by_pid: &HashMap<u32, &ProcInfo>, pid: u32) -> Option<String> {
    let parent = by_pid.get(&pid)?.parent?;
    if parent == INIT_PID {
        let owner = if cfg!(target_os = "macos") {
            "launchd"
        } else {
            "init or systemd"
        };
        return Some(format!("{owner} (ppid 1)"));
    }
    let name = base(by_pid.get(&parent)?.cmd.first()?);
    if name.contains("rc-keeper") {
        return Some(format!("rc-keeper (pid {parent})"));
    }
    if name.contains("systemd") {
        return Some(format!("systemd (pid {parent})"));
    }
    None
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, parent: Option<u32>, cmd: &[&str], cwd: Option<&str>) -> ProcInfo {
        ProcInfo {
            pid,
            parent,
            cmd: cmd.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.map(PathBuf::from),
        }
    }

    const DAEMON: u32 = 100;

    #[test]
    fn matches_claude_and_remote_control_forms() {
        assert!(is_remote_control(&[
            "claude".into(),
            "remote-control".into()
        ]));
        assert!(is_remote_control(&["/x/bin/claude".into(), "rc".into()]));
        assert!(is_remote_control(&[
            "/bin/sh".into(),
            "/x/claude".into(),
            "remote-control".into()
        ]));
        assert!(is_remote_control(&[
            "node".into(),
            "/x/claude".into(),
            "remote-control".into()
        ]));
    }

    #[test]
    fn rejects_other_commands() {
        assert!(!is_remote_control(&["claude".into(), "--print".into()]));
        assert!(!is_remote_control(&["vim".into(), "remote-control".into()]));
        // A flag value named `rc` is not the subcommand.
        assert!(!is_remote_control(&[
            "claude".into(),
            "--name".into(),
            "rc".into()
        ]));
        assert!(!is_remote_control(&["claude".into()]));
        assert!(!is_remote_control(&[]));
    }

    #[test]
    fn finds_only_matching_cwd_outside_daemon_tree() {
        let table = vec![
            proc(DAEMON, Some(1), &["rc-keeper", "daemon"], Some("/x")),
            proc(
                200,
                Some(1),
                &["claude", "remote-control"],
                Some("/work/app"),
            ),
            proc(
                201,
                Some(1),
                &["claude", "remote-control"],
                Some("/work/other"),
            ),
            proc(202, Some(1), &["vim", "remote-control"], Some("/work/app")),
            // Own instance child: a descendant of the daemon.
            proc(
                300,
                Some(DAEMON),
                &["claude", "remote-control"],
                Some("/work/app"),
            ),
        ];
        assert_eq!(
            foreign_pids(&table, Path::new("/work/app"), DAEMON),
            vec![200]
        );
    }

    #[test]
    fn descendant_of_daemon_is_never_foreign_even_through_a_middle_process() {
        let table = vec![
            proc(DAEMON, Some(1), &["rc-keeper"], None),
            proc(400, Some(DAEMON), &["sh"], None),
            proc(
                401,
                Some(400),
                &["claude", "remote-control"],
                Some("/work/app"),
            ),
        ];
        assert!(foreign_pids(&table, Path::new("/work/app"), DAEMON).is_empty());
    }

    #[test]
    fn plan_adds_session_children_and_skips_daemon_children() {
        let table = vec![
            proc(DAEMON, Some(1), &["rc-keeper"], None),
            proc(
                200,
                Some(1),
                &["claude", "remote-control"],
                Some("/work/app"),
            ),
            proc(
                210,
                Some(200),
                &["claude", "--sdk-url", "wss://x"],
                Some("/work/app"),
            ),
            proc(211, Some(210), &["sleep", "5"], None),
            proc(220, Some(200), &["sleep", "5"], None),
        ];
        let plan = plan(&table, Path::new("/work/app"), DAEMON);
        assert_eq!(plan.victims, vec![200, 210]);
    }

    #[test]
    fn plan_reports_respawn_risk_for_launchd_and_daemon_parents() {
        let table = vec![
            proc(
                DAEMON,
                Some(1),
                &["/usr/local/bin/rc-keeper", "daemon"],
                None,
            ),
            proc(
                200,
                Some(1),
                &["claude", "remote-control"],
                Some("/work/app"),
            ),
            proc(500, Some(1), &["/usr/local/bin/rc-keeper", "daemon"], None),
            proc(
                501,
                Some(500),
                &["claude", "remote-control"],
                Some("/work/app"),
            ),
        ];
        let risk = plan(&table, Path::new("/work/app"), DAEMON)
            .respawn_risk
            .unwrap();
        assert!(risk.contains("(ppid 1)"), "{risk}");
        assert!(risk.contains("rc-keeper (pid 500)"), "{risk}");
    }

    #[test]
    fn plan_has_no_risk_under_a_shell() {
        let table = vec![
            proc(DAEMON, Some(1), &["rc-keeper"], None),
            proc(300, Some(7), &["zsh"], None),
            proc(
                200,
                Some(300),
                &["claude", "remote-control"],
                Some("/work/app"),
            ),
        ];
        let plan = plan(&table, Path::new("/work/app"), DAEMON);
        assert_eq!(plan.victims, vec![200]);
        assert_eq!(plan.respawn_risk, None);
    }

    #[test]
    fn empty_plan_when_nothing_foreign() {
        let table = vec![proc(DAEMON, Some(1), &["rc-keeper"], None)];
        assert_eq!(
            plan(&table, Path::new("/work/app"), DAEMON),
            Plan::default()
        );
    }
}
