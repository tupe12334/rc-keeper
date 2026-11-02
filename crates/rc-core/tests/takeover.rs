//! Force boot against a fake `claude` binary. A foreign fake plays the other `claude remote-control`
//! that holds a folder. Only processes in temp folders are touched.
#![cfg(unix)]

use rc_core::{InstanceConfig, InstanceStatus, Manager, Settings, State};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SESSION_URL: &str = "https://claude.ai/code/session_takeover";

/// Kills the given pids when dropped, so a failed test leaves no fake behind.
struct Reap(Vec<u32>);

impl Drop for Reap {
    fn drop(&mut self) {
        for pid in &self.0 {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

struct Env {
    manager: Manager,
    folder: PathBuf,
    other: PathBuf,
    foreign_bin: PathBuf,
    pidfile: PathBuf,
}

/// Fresh temp tree: two work folders, a foreign fake, an instance fake, and a manager.
fn setup(name: &str) -> Env {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("work");
    let other = root.join("other");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    // Both roles run one copy of the fake, named `claude`, so the matcher sees that argv[0].
    let claude = root.join("bin/claude");
    std::fs::create_dir_all(claude.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_fake-claude"), &claude).unwrap();
    let pidfile = root.join("foreign.pid");

    let manager = Manager::load(root.join("config"));
    manager
        .set_settings(Settings {
            claude_bin: Some(claude.clone()),
        })
        .unwrap();
    Env {
        manager,
        folder,
        other,
        foreign_bin: claude,
        pidfile,
    }
}

/// Starts a foreign rc in `cwd` the way a terminal would: its shell exits, so it is re-parented
/// to init and is not a child of this test process. Records the pid for the instance fake.
fn spawn_foreign(bin: &Path, cwd: &Path, pidfile: Option<&Path>) -> u32 {
    let out = Command::new("sh")
        .arg("-c")
        .arg(r#""$0" remote-control >/dev/null 2>&1 & echo $!"#)
        .arg(bin)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let pid: u32 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
    // The forked child may still be pre-exec, so its command line lacks `claude`. Wait for exec.
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end && !command_line(pid).contains("claude remote-control") {
        std::thread::sleep(Duration::from_millis(20));
    }
    if let Some(path) = pidfile {
        std::fs::write(path, pid.to_string()).unwrap();
    }
    pid
}

fn command_line(pid: u32) -> String {
    let out = Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Instance config. The fake reads `--pidfile` to decide whether the folder is busy.
fn instance(env: &Env, force_boot: bool) -> InstanceConfig {
    InstanceConfig {
        folder: env.folder.clone(),
        auto_trust: true,
        force_boot,
        extra_args: vec![format!("--pidfile={}", env.pidfile.display())],
        ..Default::default()
    }
}

fn status(manager: &Manager, id: &str) -> InstanceStatus {
    manager
        .list()
        .into_iter()
        .find(|v| v.status.id == id)
        .expect("instance listed")
        .status
}

fn wait_for(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    done()
}

fn running(manager: &Manager, id: &str) -> bool {
    let s = status(manager, id);
    s.state == State::Running && s.session_urls.iter().any(|u| u == SESSION_URL)
}

#[test]
fn force_start_stops_the_foreign_rc_and_reaches_running() {
    let env = setup("force_start");
    let foreign = spawn_foreign(&env.foreign_bin, &env.folder, Some(&env.pidfile));
    let _reap = Reap(vec![foreign]);
    let id = env.manager.add(instance(&env, false)).unwrap();

    env.manager.start(&id).unwrap();
    let busy = wait_for(Duration::from_secs(15), || {
        status(&env.manager, &id).state == State::FolderBusy
    });
    assert!(
        busy,
        "never reported folder-busy: {:?}",
        status(&env.manager, &id)
    );
    assert!(pid_alive(foreign), "foreign rc must survive a plain start");

    let taken = env.manager.force_start(&id).unwrap();
    assert!(taken.stopped.contains(&foreign), "{taken:?}");
    assert!(
        taken
            .respawn_risk
            .as_deref()
            .is_some_and(|r| r.contains("(ppid 1)")),
        "orphaned foreign rc should report its ppid 1 risk: {taken:?}"
    );
    assert!(
        !pid_alive(foreign),
        "foreign rc {foreign} survived force start"
    );

    assert!(
        wait_for(Duration::from_secs(20), || running(&env.manager, &id)),
        "never reached running: {:?}",
        status(&env.manager, &id)
    );
    env.manager.shutdown();
}

#[test]
fn force_boot_takes_over_automatically() {
    let env = setup("force_boot_auto");
    let foreign = spawn_foreign(&env.foreign_bin, &env.folder, Some(&env.pidfile));
    let _reap = Reap(vec![foreign]);
    let id = env.manager.add(instance(&env, true)).unwrap();

    env.manager.start(&id).unwrap();
    assert!(
        wait_for(Duration::from_secs(25), || running(&env.manager, &id)),
        "never reached running: {:?}",
        status(&env.manager, &id)
    );
    assert!(
        !pid_alive(foreign),
        "forceBoot left foreign rc {foreign} alive"
    );
    env.manager.shutdown();
}

#[test]
fn foreign_rc_in_another_folder_is_never_killed() {
    let env = setup("other_folder");
    let foreign_here = spawn_foreign(&env.foreign_bin, &env.folder, Some(&env.pidfile));
    let bystander = spawn_foreign(&env.foreign_bin, &env.other, None);
    let _reap = Reap(vec![foreign_here, bystander]);
    let id = env.manager.add(instance(&env, true)).unwrap();

    env.manager.start(&id).unwrap();
    assert!(
        wait_for(Duration::from_secs(25), || running(&env.manager, &id)),
        "never reached running: {:?}",
        status(&env.manager, &id)
    );
    assert!(!pid_alive(foreign_here));
    assert!(pid_alive(bystander), "rc in another folder was killed");

    let taken = env.manager.force_start(&id).unwrap();
    assert!(!taken.stopped.contains(&bystander), "{taken:?}");
    assert!(
        pid_alive(bystander),
        "rc in another folder was killed by force start"
    );
    env.manager.shutdown();
}

#[test]
fn preview_lists_the_foreign_pid_and_never_stops_our_own_child() {
    let env = setup("preview");
    let id = env.manager.add(instance(&env, false)).unwrap();
    env.manager.start(&id).unwrap();
    assert!(
        wait_for(Duration::from_secs(20), || running(&env.manager, &id)),
        "never reached running: {:?}",
        status(&env.manager, &id)
    );
    // Our own claude runs in this folder with `remote-control`, but it is our child.
    let none = env.manager.preview_force_start(&id).unwrap();
    assert!(none.stopped.is_empty(), "own child listed: {none:?}");

    let foreign = spawn_foreign(&env.foreign_bin, &env.folder, None);
    let _reap = Reap(vec![foreign]);
    let preview = env.manager.preview_force_start(&id).unwrap();
    assert_eq!(preview.stopped, vec![foreign]);
    assert!(pid_alive(foreign), "preview stopped a process");
    env.manager.shutdown();
}
