//! Finds the foreign `claude remote-control` processes that serve one folder (CONTRACT.md v4).
//! Pure matching over a process table, so every rule is unit-tested with fake tables.
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Argument that marks a remote-control session child.
const SESSION_FLAG: &str = "--sdk-url";
const INIT_PID: u32 = 1;
/// JavaScript runtimes. They count only when they run the npm package's entry script.
const JS_RUNTIMES: [&str; 4] = ["node", "bun", "node.exe", "bun.exe"];
const NPM_ENTRY: &str = "@anthropic-ai/claude-code/cli.js";

/// One process as the matcher sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    /// Start time from the OS. Tells a reused pid from the process that was matched.
    pub start: u64,
    pub cmd: Vec<String>,
    pub cwd: Option<PathBuf>,
}

/// A process to stop. It carries the identity seen at scan time. A kill is refused when the pid
/// now names a process with another start time or another command line or cwd.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Victim {
    pub pid: u32,
    start: u64,
    fingerprint: u64,
}

impl Victim {
    fn of(p: &ProcInfo) -> Victim {
        Victim {
            pid: p.pid,
            start: p.start,
            fingerprint: fingerprint(p),
        }
    }
}

fn fingerprint(p: &ProcInfo) -> u64 {
    let mut hasher = DefaultHasher::new();
    p.cmd.hash(&mut hasher);
    p.cwd.hash(&mut hasher);
    hasher.finish()
}

/// True when `table` still holds the process that `victim` was taken from.
pub fn same_process(table: &[ProcInfo], victim: &Victim) -> bool {
    table.iter().any(|p| {
        p.pid == victim.pid && p.start == victim.start && fingerprint(p) == victim.fingerprint
    })
}

/// What a takeover would stop, and the supervisor that may start it again.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Foreign rc processes, each followed by its direct `--sdk-url` children.
    pub victims: Vec<Victim>,
    pub respawn_risk: Option<String>,
}

/// Reads the whole process table, with command lines, working dirs and start times.
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
            start: p.start_time(),
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
    let by_pid = index(table);
    let roots = foreign_pids(table, folder, daemon);
    if roots.is_empty() {
        return Plan::default();
    }
    let mut victims: Vec<Victim> = Vec::new();
    for root in &roots {
        let session = session_children(table, *root);
        for pid in std::iter::once(*root).chain(session) {
            // Never stop anything in the daemon's tree, or an ancestor of the daemon.
            let Some(process) = by_pid.get(&pid) else {
                continue;
            };
            if outside_daemon(&by_pid, pid, daemon) && !victims.iter().any(|v| v.pid == pid) {
                victims.push(Victim::of(process));
            }
        }
    }
    let mut risks: Vec<String> = Vec::new();
    for &pid in &roots {
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
/// A process whose cwd cannot be read is never matched.
pub fn foreign_pids(table: &[ProcInfo], folder: &Path, daemon: u32) -> Vec<u32> {
    let by_pid = index(table);
    table
        .iter()
        .filter(|p| p.cwd.as_deref().is_some_and(|cwd| same_dir(cwd, folder)))
        .filter(|p| is_remote_control(&p.cmd))
        .filter(|p| outside_daemon(&by_pid, p.pid, daemon))
        .map(|p| p.pid)
        .collect()
}

/// True when both paths name one directory. Case-folded on macOS and Windows, where folder
/// names ignore case. Symlinks resolve through `canonicalize` on both sides.
pub fn same_dir(a: &Path, b: &Path) -> bool {
    fold(&canonical(a)) == fold(&canonical(b))
}

/// True when the command line runs `claude remote-control` or `claude rc`.
pub fn is_remote_control(cmd: &[String]) -> bool {
    let Some(at) = claude_index(cmd) else {
        return false;
    };
    cmd.get(at + 1)
        .is_some_and(|sub| sub == "remote-control" || sub == "rc")
}

/// Index of the argument that is the claude program, or None. The program is a binary named
/// `claude` (or `claude.exe`), a native build under a `claude/versions/` dir, or a JavaScript
/// runtime that runs the npm package's `cli.js`.
fn claude_index(cmd: &[String]) -> Option<usize> {
    let first = cmd.first()?;
    if is_claude_program(first) {
        return Some(0);
    }
    let js = JS_RUNTIMES.contains(&base(first));
    (js && cmd
        .get(1)
        .is_some_and(|script| norm(script).ends_with(NPM_ENTRY)))
    .then_some(1)
}

fn is_claude_program(arg: &str) -> bool {
    matches!(base(arg), "claude" | "claude.exe" | "claude.cmd")
        || norm(arg).contains("/claude/versions/")
}

fn base(arg: &str) -> &str {
    arg.rsplit(['/', '\\']).next().unwrap_or(arg)
}

fn norm(arg: &str) -> String {
    arg.replace('\\', "/")
}

/// The direct children of `root` that are claude session processes (`--sdk-url`).
fn session_children(table: &[ProcInfo], root: u32) -> Vec<u32> {
    table
        .iter()
        .filter(|p| p.parent == Some(root))
        .filter(|p| claude_index(&p.cmd).is_some())
        .filter(|p| {
            p.cmd
                .iter()
                .any(|arg| arg == SESSION_FLAG || arg.starts_with("--sdk-url="))
        })
        .map(|p| p.pid)
        .collect()
}

/// True when `pid` is outside the daemon's tree: not the daemon, not below it, not above it.
fn outside_daemon(by_pid: &HashMap<u32, &ProcInfo>, pid: u32, daemon: u32) -> bool {
    pid != daemon && !is_under(by_pid, pid, daemon) && !is_under(by_pid, daemon, pid)
}

fn index(table: &[ProcInfo]) -> HashMap<u32, &ProcInfo> {
    table.iter().map(|p| (p.pid, p)).collect()
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

fn fold(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(any(target_os = "macos", windows)) {
        text.to_lowercase()
    } else {
        text.into_owned()
    }
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
            start: 1,
            cmd: cmd.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.map(PathBuf::from),
        }
    }

    fn cmd(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    const DAEMON: u32 = 100;

    #[test]
    fn matches_claude_binary_native_build_and_npm_entry() {
        assert!(is_remote_control(&cmd(&["claude", "remote-control"])));
        assert!(is_remote_control(&cmd(&["/x/bin/claude", "rc"])));
        assert!(is_remote_control(&cmd(&[
            "C:\\bin\\claude.exe",
            "remote-control"
        ])));
        // Native install: argv[0] is the version binary under claude/versions.
        assert!(is_remote_control(&cmd(&[
            "/Users/u/.local/share/claude/versions/2.1.316",
            "remote-control"
        ])));
        // npm install: node runs the package entry script.
        assert!(is_remote_control(&cmd(&[
            "node",
            "/usr/lib/node_modules/@anthropic-ai/claude-code/cli.js",
            "remote-control"
        ])));
        assert!(is_remote_control(&cmd(&[
            "bun",
            "/x/node_modules/@anthropic-ai/claude-code/cli.js",
            "remote-control"
        ])));
    }

    #[test]
    fn rejects_other_commands() {
        assert!(!is_remote_control(&cmd(&["claude", "--print"])));
        assert!(!is_remote_control(&cmd(&["vim", "remote-control"])));
        assert!(!is_remote_control(&cmd(&["vim", "/x/rc"])));
        // A flag value named `rc` is not the subcommand.
        assert!(!is_remote_control(&cmd(&["claude", "--name", "rc"])));
        assert!(!is_remote_control(&cmd(&["claude"])));
        assert!(!is_remote_control(&[]));
        // Shells and other interpreters are not claude launchers.
        assert!(!is_remote_control(&cmd(&[
            "/bin/sh",
            "/x/claude",
            "remote-control"
        ])));
        assert!(!is_remote_control(&cmd(&[
            "python3",
            "/x/claude",
            "remote-control"
        ])));
        // A node script that is not the npm package.
        assert!(!is_remote_control(&cmd(&[
            "node",
            "/x/app.js",
            "remote-control"
        ])));
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
    fn process_with_unreadable_cwd_is_never_matched() {
        let table = vec![proc(200, Some(1), &["claude", "remote-control"], None)];
        assert!(foreign_pids(&table, Path::new("/work/app"), DAEMON).is_empty());
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
    fn daemon_ancestor_is_never_matched() {
        // Pid 50 runs the folder's rc and is the daemon's parent.
        let table = vec![
            proc(
                50,
                Some(1),
                &["claude", "remote-control"],
                Some("/work/app"),
            ),
            proc(DAEMON, Some(50), &["rc-keeper", "daemon"], None),
        ];
        assert!(foreign_pids(&table, Path::new("/work/app"), DAEMON).is_empty());
    }

    #[test]
    fn plan_adds_direct_session_children_only() {
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
            // A deeper session child is not a direct child, so it is left alone.
            proc(211, Some(210), &["claude", "--sdk-url=wss://y"], None),
            proc(220, Some(200), &["sleep", "5"], None),
            // grep is not a claude program, even with the flag in its arguments.
            proc(230, Some(200), &["grep", "--sdk-url", "log"], None),
        ];
        let pids: Vec<u32> = plan(&table, Path::new("/work/app"), DAEMON)
            .victims
            .iter()
            .map(|v| v.pid)
            .collect();
        assert_eq!(pids, vec![200, 210]);
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
        assert_eq!(plan.victims.len(), 1);
        assert_eq!(plan.respawn_risk, None);
    }

    #[test]
    fn reused_pid_with_new_start_time_is_not_the_victim() {
        let before = vec![proc(
            200,
            Some(1),
            &["claude", "remote-control"],
            Some("/work/app"),
        )];
        let victim = plan(&before, Path::new("/work/app"), DAEMON)
            .victims
            .remove(0);
        assert!(same_process(&before, &victim));

        let mut reused = before.clone();
        reused[0].start = 2;
        assert!(!same_process(&reused, &victim));
    }

    #[test]
    fn same_pid_with_new_command_or_cwd_is_not_the_victim() {
        let before = vec![proc(
            200,
            Some(1),
            &["claude", "remote-control"],
            Some("/work/app"),
        )];
        let victim = plan(&before, Path::new("/work/app"), DAEMON)
            .victims
            .remove(0);

        let mut other_cmd = before.clone();
        other_cmd[0].cmd = cmd(&["claude", "--print"]);
        assert!(!same_process(&other_cmd, &victim));

        let mut other_cwd = before.clone();
        other_cwd[0].cwd = Some(PathBuf::from("/work/other"));
        assert!(!same_process(&other_cwd, &victim));
        assert!(!same_process(&[], &victim));
    }

    #[test]
    fn folder_compare_folds_case_only_where_names_ignore_case() {
        assert_eq!(
            same_dir(
                Path::new("/nonexistent/Work/App"),
                Path::new("/nonexistent/work/app")
            ),
            cfg!(any(target_os = "macos", windows))
        );
        assert!(same_dir(
            Path::new("/nonexistent/a"),
            Path::new("/nonexistent/a")
        ));
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
