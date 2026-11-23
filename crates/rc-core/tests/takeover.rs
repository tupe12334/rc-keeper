//! Force boot against the fake `claude` binary. Portable: the same tests run on unix and
//! Windows. A foreign fake plays another `claude remote-control` that holds a folder. Only
//! temp folders are touched.
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
            kill_hard(*pid);
        }
    }
}

fn kill_hard(pid: u32) {
    #[cfg(unix)]
    let _ = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .stderr(Stdio::null())
        .status();
    #[cfg(windows)]
    let _ = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// True while the pid names a live process. Unix: `kill -0`. Windows: `tasklist` lists it.
fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
    }
}

struct Env {
    manager: Manager,
    folder: PathBuf,
    other: PathBuf,
    claude: PathBuf,
    /// Written by the foreign fake, read by the instance fake (`--pidfile`).
    pidfile: PathBuf,
}

/// Fresh temp tree: two work folders, one copy of the fake named `claude`, and a manager.
fn setup(name: &str) -> Env {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("work");
    let other = root.join("other");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(&other).unwrap();

    let file = if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    };
    let claude = root.join("bin").join(file);
    std::fs::create_dir_all(claude.parent().unwrap()).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_fake-claude"), &claude).unwrap();

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
        claude,
        pidfile: root.join("foreign.pid"),
    }
}

/// Starts a foreign rc in `cwd` and returns its pid, read from `pidout`. It runs detached,
/// with no parent in this test process: its launcher exits first, so it is re-parented (unix)
/// or left with a dead parent (Windows). Either way it is not a descendant of the test.
fn spawn_foreign(claude: &Path, cwd: &Path, pidout: &Path, extra: &[&str]) -> u32 {
    let out = format!("--pidout={}", pidout.display());
    let mut args: Vec<&str> = vec![&out];
    args.extend_from_slice(extra);
    let _ = std::fs::remove_file(pidout);
    #[cfg(unix)]
    let status = Command::new("sh")
        .arg("-c")
        .arg(r#""$0" "$@" >/dev/null 2>&1 &"#)
        .arg(claude)
        .arg("remote-control")
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    #[cfg(windows)]
    let status = Command::new("cmd")
        .args(["/C", "start", "", "/B"])
        .arg(claude)
        .arg("remote-control")
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    status.unwrap();
    wait_for(Duration::from_secs(10), || {
        std::fs::read_to_string(pidout).is_ok_and(|s| !s.trim().is_empty())
    });
    std::fs::read_to_string(pidout)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

/// Reads the session child pid that the foreign fake wrote next to its own pid.
fn sdk_child_of(pidout: &Path) -> u32 {
    let child = PathBuf::from(format!("{}.child", pidout.display()));
    wait_for(Duration::from_secs(10), || child.exists());
    std::fs::read_to_string(child)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

/// Instance config. The fake reads `--pidfile` to decide whether the folder is busy.
fn instance(env: &Env, folder: &Path, force_boot: bool, extra: &[&str]) -> InstanceConfig {
    let mut extra_args = vec![format!("--pidfile={}", env.pidfile.display())];
    extra_args.extend(extra.iter().map(ToString::to_string));
    InstanceConfig {
        folder: folder.to_path_buf(),
        auto_trust: true,
        force_boot,
        extra_args,
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

/// Waits until a pid is gone. Returns false when it is still alive at the deadline.
fn gone(pid: u32) -> bool {
    wait_for(Duration::from_secs(10), || !alive(pid))
}

#[test]
fn force_start_stops_the_foreign_rc_and_its_sdk_child() {
    let env = setup("force_start");
    let foreign = spawn_foreign(&env.claude, &env.folder, &env.pidfile, &["--spawn-sdk"]);
    let child = sdk_child_of(&env.pidfile);
    let _reap = Reap(vec![foreign, child]);
    let id = env
        .manager
        .add(instance(&env, &env.folder, false, &[]))
        .unwrap();

    env.manager.start(&id).unwrap();
    let busy = wait_for(Duration::from_secs(20), || {
        status(&env.manager, &id).state == State::FolderBusy
    });
    assert!(
        busy,
        "never reported folder-busy: {:?}",
        status(&env.manager, &id)
    );
    assert!(alive(foreign), "foreign rc must survive a plain start");

    let taken = env.manager.force_start(&id).unwrap();
    assert!(taken.stopped.contains(&foreign), "{taken:?}");
    assert!(
        taken.stopped.contains(&child),
        "sdk child not listed: {taken:?}"
    );
    if cfg!(unix) {
        // Re-parented to init, so the risk names it.
        assert!(
            taken
                .respawn_risk
                .as_deref()
                .is_some_and(|r| r.contains("(ppid 1)")),
            "{taken:?}"
        );
    }
    assert!(gone(foreign), "foreign rc {foreign} survived force start");
    assert!(gone(child), "sdk child {child} survived force start");

    assert!(
        wait_for(Duration::from_secs(25), || running(&env.manager, &id)),
        "never reached running: {:?}",
        status(&env.manager, &id)
    );
    env.manager.shutdown();
}

#[test]
fn force_boot_takes_over_automatically() {
    let env = setup("force_boot_auto");
    let foreign = spawn_foreign(&env.claude, &env.folder, &env.pidfile, &[]);
    let _reap = Reap(vec![foreign]);
    let id = env
        .manager
        .add(instance(&env, &env.folder, true, &[]))
        .unwrap();

    env.manager.start(&id).unwrap();
    assert!(
        wait_for(Duration::from_secs(30), || running(&env.manager, &id)),
        "never reached running: {:?}",
        status(&env.manager, &id)
    );
    assert!(!alive(foreign), "forceBoot left foreign rc {foreign} alive");
    env.manager.shutdown();
}

#[test]
fn foreign_rc_in_another_folder_is_never_killed() {
    let env = setup("other_folder");
    let foreign_here = spawn_foreign(&env.claude, &env.folder, &env.pidfile, &[]);
    let bystander = spawn_foreign(
        &env.claude,
        &env.other,
        &env.other.join("bystander.pid"),
        &[],
    );
    let _reap = Reap(vec![foreign_here, bystander]);
    let id = env
        .manager
        .add(instance(&env, &env.folder, true, &[]))
        .unwrap();

    env.manager.start(&id).unwrap();
    assert!(
        wait_for(Duration::from_secs(30), || running(&env.manager, &id)),
        "never reached running: {:?}",
        status(&env.manager, &id)
    );
    assert!(!alive(foreign_here));
    assert!(alive(bystander), "rc in another folder was killed");

    let taken = env.manager.force_start(&id).unwrap();
    assert!(!taken.stopped.contains(&bystander), "{taken:?}");
    assert!(
        alive(bystander),
        "rc in another folder was killed by force start"
    );
    env.manager.shutdown();
}

#[test]
fn takeover_leaves_other_claude_processes_in_the_folder_alone() {
    let env = setup("other_process");
    let foreign = spawn_foreign(&env.claude, &env.folder, &env.pidfile, &[]);
    // Same program and folder, but not a remote-control: must survive.
    let mut bystander = Command::new(&env.claude)
        .arg("--print")
        .current_dir(&env.folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let bystander_pid = bystander.id();
    let _reap = Reap(vec![foreign, bystander_pid]);
    let id = env
        .manager
        .add(instance(&env, &env.folder, false, &[]))
        .unwrap();

    let taken = env.manager.force_start(&id).unwrap();
    assert!(taken.stopped.contains(&foreign), "{taken:?}");
    assert!(!taken.stopped.contains(&bystander_pid), "{taken:?}");
    assert!(gone(foreign));
    assert!(
        alive(bystander_pid),
        "non-rc process in the folder was killed"
    );
    let _ = bystander.kill();
    let _ = bystander.wait();
    env.manager.shutdown();
}

#[test]
fn preview_lists_the_foreign_pid_and_never_stops_our_own_child() {
    let env = setup("preview");
    let id = env
        .manager
        .add(instance(&env, &env.folder, false, &[]))
        .unwrap();
    env.manager.start(&id).unwrap();
    assert!(
        wait_for(Duration::from_secs(25), || running(&env.manager, &id)),
        "never reached running: {:?}",
        status(&env.manager, &id)
    );
    // Our own claude runs in this folder with `remote-control`, but it is our child.
    let none = env.manager.preview_force_start(&id).unwrap();
    assert!(none.stopped.is_empty(), "own child listed: {none:?}");

    let foreign = spawn_foreign(
        &env.claude,
        &env.folder,
        &env.other.join("preview.pid"),
        &[],
    );
    let _reap = Reap(vec![foreign]);
    let preview = env.manager.preview_force_start(&id).unwrap();
    assert_eq!(preview.stopped, vec![foreign]);
    assert!(alive(foreign), "preview stopped a process");
    env.manager.shutdown();
}

#[test]
fn plain_start_is_not_running_before_the_busy_notice() {
    let env = setup("late_busy");
    // Prints a banner, then the busy notice two seconds later. Banner alone must not be Running.
    let id = env
        .manager
        .add(instance(&env, &env.folder, false, &["--late-busy"]))
        .unwrap();
    env.manager.start(&id).unwrap();

    let mut seen_running = false;
    let busy = wait_for(Duration::from_secs(15), || {
        let state = status(&env.manager, &id).state;
        seen_running |= state == State::Running;
        state == State::FolderBusy
    });
    assert!(
        busy,
        "never reported folder-busy: {:?}",
        status(&env.manager, &id)
    );
    assert!(!seen_running, "showed running before the busy notice");
    env.manager.shutdown();
}

#[cfg(windows)]
#[test]
fn windows_tasklist_liveness_sees_live_and_dead_pids() {
    let mut done = Command::new("cmd")
        .args(["/C", "exit", "0"])
        .spawn()
        .unwrap();
    let dead = done.id();
    done.wait().unwrap();
    assert!(
        alive(std::process::id()),
        "this test process must read as alive"
    );
    assert!(!alive(dead), "an exited process must read as dead");
}
