//! Process-tree queries via sysinfo: session counts and killing leftover children.
use std::collections::VecDeque;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

const EXIT_POLL: Duration = Duration::from_millis(100);

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

/// Kills the whole process group led by `pid`. The PTY child calls `setsid()`, so its pid is
/// the group id. Orphans keep the group id even after their parent is gone.
#[cfg(unix)]
pub fn kill_group(pid: u32) {
    // ponytail: a recycled pid could match an unrelated group. Rare, accepted.
    // SAFETY: a negative pid targets a process group. It is harmless when the group is empty.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
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

/// Stops `pids`. Unix: SIGTERM, then SIGKILL for any still alive after `grace`.
/// Windows: terminates each process at once.
#[cfg(unix)]
pub fn terminate(pids: &[u32], grace: Duration) {
    for &pid in pids {
        signal(pid, libc::SIGTERM);
    }
    let until = Instant::now() + grace;
    while Instant::now() < until && pids.iter().any(|&pid| alive(pid)) {
        thread::sleep(EXIT_POLL);
    }
    for &pid in pids.iter().filter(|&&pid| alive(pid)) {
        signal(pid, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
pub fn terminate(pids: &[u32], _grace: Duration) {
    let Ok(sys) = System::new() else {
        return;
    };
    for &pid in pids {
        if let Some(process) = sys.process(Pid::from_u32(pid)) {
            process.kill();
        }
    }
}

#[cfg(unix)]
fn signal(pid: u32, sig: libc::c_int) {
    // SAFETY: plain kill(2) on one pid. A recycled pid is the only risk, and the caller holds the
    // pid from a fresh scan made moments earlier.
    unsafe {
        libc::kill(pid as libc::pid_t, sig);
    }
}

/// `kill(pid, 0)`: true while the process exists, including zombies not yet reaped.
#[cfg(unix)]
fn alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks that the pid exists.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}
