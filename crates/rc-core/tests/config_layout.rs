//! Config dir layout: exact bytes, determinism, migration, live reload. No `claude` process runs.
use rc_core::{InstanceConfig, Manager, Settings, State};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn fresh_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("config-layout")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn inst(folder: &str) -> InstanceConfig {
    InstanceConfig {
        folder: folder.into(),
        ..Default::default()
    }
}

fn read(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Polls `cond` for up to two seconds, the reload budget in the contract.
fn within_two_seconds(mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    cond()
}

#[test]
fn golden_instance_and_settings_bytes() {
    let dir = fresh_dir("golden");
    let m = Manager::load(dir.clone());
    m.set_settings(Settings {
        claude_bin: Some("/opt/claude/bin/claude".into()),
    })
    .unwrap();
    let id = m
        .add(InstanceConfig {
            name: Some("Work".into()),
            auto_start: true,
            capacity: Some(3),
            chrome: true,
            create_session_in_dir: Some(true),
            extra_args: vec!["--verbose".into()],
            permission_mode: Some("acceptEdits".into()),
            session_name_prefix: Some("rc".into()),
            spawn: Some("worktree".into()),
            ..inst("/srv/projects/rc-keeper")
        })
        .unwrap();
    assert_eq!(id, "rc-keeper");
    assert_eq!(
        read(&dir, "instances/rc-keeper.json"),
        "{\n  \"id\": \"rc-keeper\",\n  \"name\": \"Work\",\n  \"folder\": \"/srv/projects/rc-keeper\",\n  \"autoStart\": true,\n  \"autoTrust\": false,\n  \"capacity\": 3,\n  \"chrome\": true,\n  \"createSessionInDir\": true,\n  \"extraArgs\": [\n    \"--verbose\"\n  ],\n  \"permissionMode\": \"acceptEdits\",\n  \"sessionNamePrefix\": \"rc\",\n  \"spawn\": \"worktree\"\n}\n"
    );
    assert_eq!(read(&dir, "settings.json"), "{}\n");
    assert_eq!(
        read(&dir, "settings.local.json"),
        "{\n  \"claudeBin\": \"/opt/claude/bin/claude\"\n}\n"
    );
}

#[test]
fn save_load_save_is_byte_identical() {
    let dir = fresh_dir("roundtrip");
    let m = Manager::load(dir.clone());
    m.add(inst("/srv/roundtrip/app")).unwrap();
    let first = read(&dir, "instances/app.json");
    let reopened = Manager::load(dir.clone());
    let cfg = reopened.list()[0].config.clone();
    reopened.update(cfg).unwrap();
    assert_eq!(read(&dir, "instances/app.json"), first);
}

#[test]
fn reversed_insertion_order_gives_identical_files() {
    let a = fresh_dir("order-ab");
    let b = fresh_dir("order-ba");
    let ma = Manager::load(a.clone());
    ma.add(inst("/srv/alpha")).unwrap();
    ma.add(inst("/srv/beta")).unwrap();
    let mb = Manager::load(b.clone());
    mb.add(inst("/srv/beta")).unwrap();
    mb.add(inst("/srv/alpha")).unwrap();
    for file in ["instances/alpha.json", "instances/beta.json"] {
        assert_eq!(read(&a, file), read(&b, file), "{file}");
    }
}

#[test]
fn two_edits_change_only_two_files() {
    let dir = fresh_dir("two-edits");
    let m = Manager::load(dir.clone());
    m.set_settings(Settings::default()).unwrap();
    for folder in ["/srv/one", "/srv/two", "/srv/three"] {
        m.add(inst(folder)).unwrap();
    }
    let mtime = |rel: &str| fs::metadata(dir.join(rel)).unwrap().modified().unwrap();
    let untouched = mtime("instances/three.json");
    let settings_before = read(&dir, "settings.json");
    std::thread::sleep(Duration::from_millis(20));

    for id in ["one", "two"] {
        let mut cfg = m
            .list()
            .into_iter()
            .find(|v| v.config.id == id)
            .unwrap()
            .config;
        cfg.chrome = true;
        m.update(cfg).unwrap();
    }

    assert_eq!(mtime("instances/three.json"), untouched);
    assert!(read(&dir, "instances/one.json").contains("\"chrome\": true"));
    assert!(read(&dir, "instances/two.json").contains("\"chrome\": true"));
    assert!(!read(&dir, "instances/three.json").contains("\"chrome\": true"));
    assert_eq!(read(&dir, "settings.json"), settings_before);
}

#[cfg(unix)]
#[test]
fn tilde_folder_round_trips_on_disk() {
    let home = PathBuf::from(std::env::var_os("HOME").expect("HOME is set"));
    let dir = fresh_dir("tilde");
    fs::create_dir_all(dir.join("instances")).unwrap();
    // Canonical bytes as the daemon writes them. Nothing is spawned, so the folder need not exist.
    let canonical = "{\n  \"id\": \"home-app\",\n  \"folder\": \"~/rc-test-app\",\n  \"autoStart\": false,\n  \"autoTrust\": false,\n  \"chrome\": false,\n  \"extraArgs\": []\n}\n";
    fs::write(dir.join("instances/home-app.json"), canonical).unwrap();

    let m = Manager::load(dir.clone());
    let view = m
        .list()
        .into_iter()
        .find(|v| v.config.id == "home-app")
        .unwrap();
    assert_eq!(view.config.folder, home.join("rc-test-app"));
    m.update(view.config).unwrap();
    assert_eq!(read(&dir, "instances/home-app.json"), canonical);
}

#[test]
fn slug_collisions_get_suffixes() {
    let dir = fresh_dir("slugs");
    let m = Manager::load(dir.clone());
    let ids: Vec<String> = ["/srv/a/work", "/srv/b/work", "/srv/c/work"]
        .into_iter()
        .map(|f| m.add(inst(f)).unwrap())
        .collect();
    assert_eq!(ids, ["work", "work-2", "work-3"]);
    for id in &ids {
        assert!(dir.join(format!("instances/{id}.json")).exists());
    }
    // Editing never renames the file or the id. Look up by id: the watcher reloads in file-name
    // order, which puts "work-2" before "work", so list position is not stable.
    let by_id = |m: &Manager, id: &str| {
        m.list()
            .into_iter()
            .find(|v| v.config.id == id)
            .unwrap()
            .config
    };
    let mut cfg = by_id(&m, "work-2");
    cfg.folder = "/srv/z/renamed".into();
    m.update(cfg).unwrap();
    assert_eq!(by_id(&m, "work-2").folder, Path::new("/srv/z/renamed"));
    assert!(dir.join("instances/work-2.json").exists());
}

#[test]
fn legacy_config_json_is_migrated_once() {
    let dir = fresh_dir("migrate");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("config.json"),
        r#"{
  "settings": { "claudeBin": "/opt/claude" },
  "instances": [
    { "id": "3f2b-random", "folder": "/srv/projects/alpha", "chrome": true, "extraArgs": ["--verbose"] },
    { "id": "9c1d-random", "folder": "/srv/projects/beta", "name": "Beta" }
  ]
}"#,
    )
    .unwrap();

    let m = Manager::load(dir.clone());
    let ids: Vec<String> = m.list().into_iter().map(|v| v.config.id).collect();
    assert_eq!(ids, ["alpha", "beta"]);
    assert!(!dir.join("config.json").exists());
    assert!(dir.join("config.json.migrated").exists());
    assert_eq!(
        read(&dir, "settings.local.json"),
        "{\n  \"claudeBin\": \"/opt/claude\"\n}\n"
    );
    assert!(read(&dir, "instances/alpha.json").contains("\"chrome\": true"));
    assert!(read(&dir, ".gitignore").contains("settings.local.json"));

    // A second load finds no legacy file and changes nothing.
    let before = read(&dir, "instances/alpha.json");
    drop(m);
    let again = Manager::load(dir.clone());
    assert_eq!(again.list().len(), 2);
    assert_eq!(read(&dir, "instances/alpha.json"), before);
}

#[test]
fn added_file_appears_and_removed_file_disappears() {
    let dir = fresh_dir("reload-add");
    let m = Manager::load(dir.clone());
    fs::create_dir_all(dir.join("instances")).unwrap();
    // Write to a temp name and rename, as git pull or an editor would.
    let tmp = dir.join("instances/extra.json.tmp");
    fs::write(
        &tmp,
        "{\n  \"id\": \"extra\",\n  \"folder\": \"/srv/extra\"\n}\n",
    )
    .unwrap();
    fs::rename(&tmp, dir.join("instances/extra.json")).unwrap();

    assert!(within_two_seconds(|| m.list().iter().any(|v| v.config.id
        == "extra"
        && v.status.state != State::Error)));

    fs::remove_file(dir.join("instances/extra.json")).unwrap();
    assert!(within_two_seconds(|| !m
        .list()
        .iter()
        .any(|v| v.config.id == "extra")));
}

#[test]
fn corrupt_file_shows_error_and_others_stay() {
    let dir = fresh_dir("corrupt");
    let m = Manager::load(dir.clone());
    m.add(inst("/srv/good")).unwrap();
    m.add(inst("/srv/bad")).unwrap();

    fs::write(dir.join("instances/bad.json"), "{ not json").unwrap();

    assert!(within_two_seconds(|| {
        m.list()
            .iter()
            .any(|v| v.config.id == "bad" && v.status.state == State::Error)
    }));
    let views = m.list();
    let bad = views.iter().find(|v| v.config.id == "bad").unwrap();
    let message = bad.status.last_error.as_deref().unwrap();
    assert!(
        message.starts_with("invalid instances/bad.json:"),
        "{message}"
    );
    let good = views.iter().find(|v| v.config.id == "good").unwrap();
    assert_eq!(good.status.state, State::Stopped);
    assert_eq!(good.config.folder, PathBuf::from("/srv/good"));
    assert!(m.start("bad").is_err());
}

#[test]
fn gitignore_is_written_once_and_never_overwritten() {
    let dir = fresh_dir("gitignore");
    let _m = Manager::load(dir.clone());
    let text = read(&dir, ".gitignore");
    for line in [
        "settings.local.json",
        "status.json",
        "daemon.log",
        "rc-keeper.sock",
        "*.tmp",
    ] {
        assert!(text.lines().any(|l| l == line), "missing {line}");
    }
    drop(_m);

    fs::write(dir.join(".gitignore"), "custom\n").unwrap();
    let _again = Manager::load(dir.clone());
    assert_eq!(read(&dir, ".gitignore"), "custom\n");
}

fn ids(m: &Manager) -> Vec<String> {
    m.list().into_iter().map(|v| v.config.id).collect()
}

#[test]
fn list_json_keeps_null_for_unset_fields() {
    let dir = fresh_dir("ipc-null");
    let m = Manager::load(dir);
    m.add(inst("/srv/nulls")).unwrap();
    let json = serde_json::to_string(&m.list()).unwrap();
    assert!(json.contains("\"name\":null"), "{json}");
    assert!(json.contains("\"permissionMode\":null"), "{json}");
}

#[test]
fn legacy_shared_folder_name_gets_suffix() {
    let dir = fresh_dir("migrate-same-name");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("config.json"),
        r#"{"instances":[{"id":"x1","folder":"/srv/a/app"},{"id":"x2","folder":"/srv/b/app"}]}"#,
    )
    .unwrap();
    let m = Manager::load(dir);
    let folder_of = |id: &str| {
        m.list()
            .into_iter()
            .find(|v| v.config.id == id)
            .unwrap()
            .config
            .folder
    };
    assert_eq!(folder_of("app"), PathBuf::from("/srv/a/app"));
    assert_eq!(folder_of("app-2"), PathBuf::from("/srv/b/app"));
}

#[test]
fn migration_rerun_after_partial_failure_does_not_duplicate() {
    let dir = fresh_dir("migrate-rerun");
    fs::create_dir_all(dir.join("instances")).unwrap();
    // The first write of an interrupted run landed. config.json is still in place.
    let alpha = "{\n  \"id\": \"alpha\",\n  \"folder\": \"/srv/projects/alpha\",\n  \"autoStart\": false,\n  \"autoTrust\": false,\n  \"chrome\": false,\n  \"extraArgs\": []\n}\n";
    fs::write(dir.join("instances/alpha.json"), alpha).unwrap();
    fs::write(
        dir.join("config.json"),
        r#"{"instances":[{"id":"a","folder":"/srv/projects/alpha"},{"id":"b","folder":"/srv/projects/beta"}]}"#,
    )
    .unwrap();

    let m = Manager::load(dir.clone());
    assert_eq!(ids(&m), ["alpha", "beta"]);
    assert_eq!(read(&dir, "instances/alpha.json"), alpha);
    assert!(dir.join("config.json.migrated").exists());
}

#[test]
fn restored_legacy_file_is_absorbed_without_overwriting_migrated() {
    let dir = fresh_dir("migrate-restored");
    fs::create_dir_all(&dir).unwrap();
    let legacy = r#"{"instances":[{"id":"a","folder":"/srv/projects/alpha"},{"id":"b","folder":"/srv/projects/beta"}]}"#;
    fs::write(dir.join("config.json"), legacy).unwrap();
    let first = Manager::load(dir.clone());
    assert_eq!(ids(&first), ["alpha", "beta"]);
    let migrated = read(&dir, "config.json.migrated");
    drop(first);

    // An old-version machine pushes config.json back through git.
    fs::write(dir.join("config.json"), legacy).unwrap();
    let again = Manager::load(dir.clone());
    assert_eq!(ids(&again), ["alpha", "beta"]);
    assert_eq!(read(&dir, "config.json.migrated"), migrated);
}

#[test]
fn replaced_config_dir_is_reloaded_and_watched_again() {
    let root = fresh_dir("dir-replace");
    let dir = root.join("cfg");
    let m = Manager::load(dir.clone());
    m.add(inst("/srv/old")).unwrap();

    // Move the live dir aside, then rename a fully written sibling dir into its place.
    let staged = root.join("cfg.staged");
    fs::create_dir_all(staged.join("instances")).unwrap();
    fs::write(
        staged.join("instances/new.json"),
        "{\n  \"id\": \"new\",\n  \"folder\": \"/srv/new\"\n}\n",
    )
    .unwrap();
    fs::rename(&dir, root.join("cfg.old")).unwrap();
    fs::rename(&staged, &dir).unwrap();

    assert!(within_two_seconds(|| ids(&m) == ["new"]), "{:?}", ids(&m));

    // The watch now follows the new inode, so later edits arrive too.
    fs::write(
        dir.join("instances/later.json"),
        "{\n  \"id\": \"later\",\n  \"folder\": \"/srv/later\"\n}\n",
    )
    .unwrap();
    assert!(within_two_seconds(|| ids(&m).contains(&"later".to_owned())));
}
