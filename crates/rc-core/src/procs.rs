//! Process-tree queries via sysinfo: session counts and killing leftover children.
use std::collections::VecDeque;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Argument that marks a remote-control session child.
const SESSION_FLAG: &str = "--sdk-url";

pub fn refresh(sys: &mut System) {
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet),
    );
}

/// Counts the direct children of `root` whose command line marks a session.
pub fn sdk_sessions(sys: &System, root: u32) -> u32 {
    let root = Pid::from_u32(root);
    let count = sys
        .processes()
        .values()
        .filter(|p| p.parent() == Some(root))
        .filter(|p| {
            p.cmd()
                .iter()
                .any(|arg| arg.to_string_lossy().contains(SESSION_FLAG))
        })
        .count();
    count as u32
}

/// Kills every descendant of `root`, then runs `kill_root`. Descendants are listed first,
/// because orphans lose their parent link once `root` dies.
pub fn kill_tree(root: u32, kill_root: impl FnOnce()) {
    let Ok(mut sys) = System::new() else {
        kill_root();
        return;
    };
    refresh(&mut sys);
    let victims = descendants(&sys, root);
    kill_root();
    for pid in victims {
        if let Some(process) = sys.process(pid) {
            process.kill();
        }
    }
}

fn descendants(sys: &System, root: u32) -> Vec<Pid> {
    let mut found: Vec<Pid> = Vec::new();
    let mut queue = VecDeque::from([Pid::from_u32(root)]);
    while let Some(parent) = queue.pop_front() {
        for (pid, process) in sys.processes() {
            if process.parent() == Some(parent) && !found.contains(pid) {
                found.push(*pid);
                queue.push_back(*pid);
            }
        }
    }
    found
}
