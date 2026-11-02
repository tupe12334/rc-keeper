//! Running instances keep their process through broken files and migration.
//! Uses a shell stand-in for `claude`. The real binary is never run.
#![cfg(unix)]

use rc_core::{InstanceConfig, InstanceView, Manager, Settings, State};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Prints the ready line, then sleeps, so the instance stays in `running`.
fn stand_in(root: &Path) -> PathBuf {
    let path = root.join("claude-stand-in.sh");
    fs::write(
        &path,
        "#!/bin/sh\necho \"Remote Control ready https://claude.ai/code/session_keep\"\nexec sleep 600\n",
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn fresh_root(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("running")
        .join(name);
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("work")).unwrap();
    root
}

fn wait_until(secs: u64, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    cond()
}

fn view(m: &Manager, id: &str) -> Option<InstanceView> {
    m.list().into_iter().find(|v| v.config.id == id)
}

fn pid(m: &Manager, id: &str) -> Option<u32> {
    view(m, id).and_then(|v| v.status.pid)
}

fn started(m: &Manager, id: &str) -> bool {
    view(m, id).is_some_and(|v| v.status.state == State::Running && v.status.pid.is_some())
}

fn running_manager(root: &Path) -> Manager {
    let m = Manager::load(root.join("config"));
    m.set_settings(Settings {
        claude_bin: Some(stand_in(root)),
    })
    .unwrap();
    m.add(InstanceConfig {
        folder: root.join("work"),
        ..Default::default()
    })
    .unwrap();
    m.start("work").unwrap();
    assert!(
        wait_until(10, || started(&m, "work")),
        "instance did not start"
    );
    m
}

#[test]
fn corrupt_file_keeps_the_process_and_shows_the_error() {
    let root = fresh_root("corrupt-keeps-pid");
    let m = running_manager(&root);
    let file = root.join("config/instances/work.json");
    let good = fs::read(&file).unwrap();
    let before = pid(&m, "work");

    fs::write(&file, "{ half a fi").unwrap();
    assert!(wait_until(3, || view(&m, "work")
        .and_then(|v| v.status.last_error)
        .is_some_and(|e| e.contains("invalid"))));
    std::thread::sleep(Duration::from_millis(1500));
    let v = view(&m, "work").unwrap();
    assert_eq!(v.status.pid, before, "process was restarted or stopped");
    assert_eq!(v.status.state, State::Running);

    // Fixing the file with unchanged content must not restart the process.
    fs::write(&file, good).unwrap();
    assert!(wait_until(3, || view(&m, "work")
        .unwrap()
        .status
        .last_error
        .is_none()));
    assert_eq!(pid(&m, "work"), before);
    m.shutdown();
}

#[test]
fn removed_file_stops_the_running_instance() {
    let root = fresh_root("removed-stops");
    let m = running_manager(&root);
    fs::remove_file(root.join("config/instances/work.json")).unwrap();
    assert!(wait_until(3, || view(&m, "work").is_none()));
    m.shutdown();
}

#[test]
fn running_instance_survives_legacy_migration() {
    let root = fresh_root("migrate-running");
    let dir = root.join("config");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("config.json"),
        format!(
            r#"{{"settings":{{"claudeBin":"{}"}},"instances":[{{"id":"legacy-1","folder":"{}","autoStart":true}}]}}"#,
            stand_in(&root).display(),
            root.join("work").display()
        ),
    )
    .unwrap();

    let m = Manager::load(dir.clone());
    assert!(
        wait_until(10, || started(&m, "work")),
        "not started after migration"
    );
    assert!(dir.join("config.json.migrated").exists());
    m.shutdown();
}
