//! Process-tree queries via sysinfo: session counts and killing leftover children.
use crate::foreign::Victim;
use std::collections::VecDeque;
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[cfg(unix)]
const EXIT_POLL: Duration = Duration::from_millis(100);
#[cfg(unix)]
use std::{thread, time::Instant};

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

/// Stops the victims. Each one is checked with `still_same` right before its signal, so a pid
/// reused by another process is never signalled. Unix: SIGTERM, wait up to `grace`, then SIGKILL
/// for any still alive. Windows: terminates each matching process at once.
#[cfg(unix)]
pub fn terminate(victims: &[Victim], grace: Duration, still_same: impl Fn(&Victim) -> bool) {
    for victim in victims {
        if still_same(victim) {
            signal(victim.pid, libc::SIGTERM);
        }
    }
    let until = Instant::now() + grace;
    while Instant::now() < until && victims.iter().any(|v| alive(v.pid)) {
        thread::sleep(EXIT_POLL);
    }
    for victim in victims.iter().filter(|v| alive(v.pid)) {
        if still_same(victim) {
            signal(victim.pid, libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
pub fn terminate(victims: &[Victim], _grace: Duration, still_same: impl Fn(&Victim) -> bool) {
    let Ok(mut sys) = System::new() else {
        return;
    };
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    for victim in victims {
        if !still_same(victim) {
            continue;
        }
        if let Some(process) = sys.process(Pid::from_u32(victim.pid)) {
            process.kill();
        }
    }
}

/// The pid as a positive `pid_t`. Zero and values above `i32::MAX` never reach kill(2): zero
/// means "our own group" and negative values mean a process group.
#[cfg(unix)]
fn pid_of(pid: u32) -> Option<libc::pid_t> {
    libc::pid_t::try_from(pid).ok().filter(|&p| p > 0)
}

#[cfg(unix)]
fn signal(pid: u32, sig: libc::c_int) {
    let Some(pid) = pid_of(pid) else {
        return;
    };
    // SAFETY: kill(2) on one positive pid. The caller re-verified the process identity just before.
    unsafe {
        libc::kill(pid, sig);
    }
}

/// `kill(pid, 0)`: true while the process exists, including zombies not yet reaped.
#[cfg(unix)]
fn alive(pid: u32) -> bool {
    let Some(pid) = pid_of(pid) else {
        return false;
    };
    // SAFETY: signal 0 only checks that the pid exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(all(test, unix))]
mod tests {
    use super::pid_of;

    #[test]
    fn zero_and_out_of_range_pids_never_reach_kill() {
        assert_eq!(pid_of(0), None);
        assert_eq!(pid_of(u32::MAX), None);
        assert_eq!(pid_of(i32::MAX as u32 + 1), None);
        assert_eq!(pid_of(42), Some(42));
    }
}
