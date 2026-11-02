//! The config dir works as a git repo: only instance and shared files get tracked.
#![cfg(unix)]

use rc_core::{InstanceConfig, Manager, Settings};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn git(dir: &PathBuf, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=rc-test",
            "-c",
            "user.email=rc-test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn edit_after_commit_shows_one_modified_file() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("git-flow");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);

    let m = Manager::load(dir.clone());
    m.set_settings(Settings {
        claude_bin: Some("/opt/claude".into()),
    })
    .unwrap();
    for folder in ["/srv/projects/one", "/srv/projects/two"] {
        m.add(InstanceConfig {
            folder: folder.into(),
            ..Default::default()
        })
        .unwrap();
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "init"]);
    assert_eq!(git(&dir, &["status", "--porcelain"]), "");

    let mut cfg = m
        .list()
        .into_iter()
        .find(|v| v.config.id == "one")
        .unwrap()
        .config;
    cfg.chrome = true;
    m.update(cfg).unwrap();

    // The daemon rewrites status.json every 2 s. It must stay ignored.
    std::thread::sleep(std::time::Duration::from_millis(2300));
    let status = git(&dir, &["status", "--porcelain"]);
    assert_eq!(status, " M instances/one.json\n");
}
