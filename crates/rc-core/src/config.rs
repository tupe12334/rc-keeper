//! Config dir layout, git-friendly (see CONTRACT.md v3):
//! `settings.json` (shared), `settings.local.json` (machine-only), `instances/<id>.json`, `.gitignore`.
//! Every file is written with fixed bytes, and only when those bytes change.
use crate::model::{AppConfig, InstanceConfig, Settings};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE: &str = "settings.json";
pub const LOCAL_SETTINGS_FILE: &str = "settings.local.json";
pub const INSTANCES_DIR: &str = "instances";
pub const STATUS_FILE: &str = "status.json";
const LEGACY_FILE: &str = "config.json";
const MIGRATED_FILE: &str = "config.json.migrated";
/// Shared settings hold no portable values yet, so the file is an empty object.
const SHARED_SETTINGS: &[u8] = b"{}\n";
const GITIGNORE: &str =
    "settings.local.json\nstatus.json\ndaemon.log\nrc-keeper.sock\n*.tmp\nconfig.json.migrated\n";

/// On-disk shape of `instances/<id>.json`. Same keys in the same order as `InstanceConfig`,
/// with `None` fields omitted. A separate type, so IPC keeps sending `null` for unset fields.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct InstanceFile {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    folder: PathBuf,
    auto_start: bool,
    auto_trust: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    capacity: Option<u32>,
    chrome: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    create_session_in_dir: Option<bool>,
    extra_args: Vec<String>,
    /// Written only when true, so files of instances without it keep their bytes.
    #[serde(default, skip_serializing_if = "is_false")]
    force_boot: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_name_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spawn: Option<String>,
}

// Destructuring (no `..`) makes a new InstanceConfig field a compile error here until it is stored.
impl From<&InstanceConfig> for InstanceFile {
    fn from(cfg: &InstanceConfig) -> Self {
        let InstanceConfig {
            id,
            name,
            folder,
            auto_start,
            auto_trust,
            capacity,
            chrome,
            create_session_in_dir,
            extra_args,
            force_boot,
            permission_mode,
            session_name_prefix,
            spawn,
        } = cfg;
        InstanceFile {
            id: id.clone(),
            name: name.clone(),
            folder: folder.clone(),
            auto_start: *auto_start,
            auto_trust: *auto_trust,
            capacity: *capacity,
            chrome: *chrome,
            create_session_in_dir: *create_session_in_dir,
            extra_args: extra_args.clone(),
            force_boot: *force_boot,
            permission_mode: permission_mode.clone(),
            session_name_prefix: session_name_prefix.clone(),
            spawn: spawn.clone(),
        }
    }
}

impl From<InstanceFile> for InstanceConfig {
    fn from(f: InstanceFile) -> Self {
        InstanceConfig {
            id: f.id,
            name: f.name,
            folder: f.folder,
            auto_start: f.auto_start,
            auto_trust: f.auto_trust,
            capacity: f.capacity,
            chrome: f.chrome,
            create_session_in_dir: f.create_session_in_dir,
            extra_args: f.extra_args,
            force_boot: f.force_boot,
            permission_mode: f.permission_mode,
            session_name_prefix: f.session_name_prefix,
            spawn: f.spawn,
        }
    }
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// What `load` found on disk. `broken` maps an id to its parse error.
#[derive(Default)]
pub struct Loaded {
    pub config: AppConfig,
    pub broken: BTreeMap<String, String>,
}

/// `USERPROFILE` on Windows, since Git Bash and MSYS also set `HOME` to a path Windows cannot use.
pub fn home_dir() -> Option<PathBuf> {
    let vars: &[&str] = if cfg!(windows) {
        &["USERPROFILE", "HOME"]
    } else {
        &["HOME"]
    };
    vars.iter().find_map(std::env::var_os).map(PathBuf::from)
}

/// Stores a folder under `home` as `~/...` with `/` separators. Other paths stay as they are.
pub fn to_portable(folder: &Path, home: Option<&Path>) -> PathBuf {
    let Some(rest) = home.and_then(|h| folder.strip_prefix(h).ok()) else {
        return folder.to_path_buf();
    };
    let parts: Option<Vec<&str>> = rest.iter().map(|p| p.to_str()).collect();
    match parts {
        Some(parts) if parts.is_empty() => PathBuf::from("~"),
        Some(parts) => PathBuf::from(format!("~/{}", parts.join("/"))),
        None => folder.to_path_buf(),
    }
}

/// Expands a leading `~` to `home`.
pub fn from_portable(folder: &Path, home: Option<&Path>) -> PathBuf {
    let (Some(home), Some(text)) = (home, folder.to_str()) else {
        return folder.to_path_buf();
    };
    if text == "~" {
        home.to_path_buf()
    } else if let Some(rest) = text.strip_prefix("~/") {
        home.join(rest)
    } else {
        folder.to_path_buf()
    }
}

/// Slug of the folder's last segment. Empty names become `instance`, and Windows device names get a suffix.
pub fn slug_of(folder: &Path) -> String {
    let name = folder.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "instance".to_owned()
    } else if is_windows_device(&slug) {
        format!("{slug}-instance")
    } else {
        slug
    }
}

/// `con`, `nul`, `com1`..`com9` and `lpt1`..`lpt9` cannot be file names on Windows.
fn is_windows_device(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    matches!(slug, "con" | "prn" | "aux" | "nul")
        || (bytes.len() == 4
            && (slug.starts_with("com") || slug.starts_with("lpt"))
            && (b'1'..=b'9').contains(&bytes[3]))
}

/// `base`, or `base-2`, `base-3`... for the first free name.
pub fn unique_id(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_owned();
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Ids become file names, so only letters, digits, `-` and `_` pass.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

pub fn instance_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(INSTANCES_DIR).join(format!("{id}.json"))
}

pub fn instance_exists(dir: &Path, id: &str) -> bool {
    instance_path(dir, id).exists()
}

pub fn load(dir: &Path, home: Option<&Path>) -> Loaded {
    let settings = match fs::read(dir.join(LOCAL_SETTINGS_FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            eprintln!("rc-core: invalid {LOCAL_SETTINGS_FILE}: {e}");
            Settings::default()
        }),
        Err(_) => Settings::default(),
    };
    let mut loaded = Loaded {
        config: AppConfig {
            settings,
            instances: Vec::new(),
        },
        broken: BTreeMap::new(),
    };
    let Ok(entries) = fs::read_dir(dir.join(INSTANCES_DIR)) else {
        return loaded;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    for path in files {
        let Some(id) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        match read_instance(&path, &id, home) {
            Ok(cfg) => loaded.config.instances.push(cfg),
            Err(message) => {
                loaded.broken.insert(id, message);
            }
        }
    }
    loaded
}

fn read_instance(path: &Path, id: &str, home: Option<&Path>) -> Result<InstanceConfig, String> {
    let file = path.file_name().unwrap_or_default().to_string_lossy();
    let bytes = fs::read(path).map_err(|e| format!("cannot read {INSTANCES_DIR}/{file}: {e}"))?;
    let stored: InstanceFile = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid {INSTANCES_DIR}/{file}: {e}"))?;
    let mut cfg = InstanceConfig::from(stored);
    if cfg.id.is_empty() {
        cfg.id = id.to_owned();
    } else if cfg.id != id {
        return Err(format!(
            "invalid {INSTANCES_DIR}/{file}: id \"{}\" does not match the file name",
            cfg.id
        ));
    }
    cfg.folder = from_portable(&cfg.folder, home);
    Ok(cfg)
}

pub fn save_settings(dir: &Path, settings: &Settings) -> Result<(), String> {
    write_if_changed(&dir.join(SETTINGS_FILE), SHARED_SETTINGS)?;
    write_if_changed(&dir.join(LOCAL_SETTINGS_FILE), &to_json(settings)?)
}

pub fn save_instance(dir: &Path, cfg: &InstanceConfig, home: Option<&Path>) -> Result<(), String> {
    if !valid_id(&cfg.id) {
        return Err(format!("invalid instance id \"{}\"", cfg.id));
    }
    let mut stored = InstanceFile::from(cfg);
    stored.folder = to_portable(&cfg.folder, home);
    write_if_changed(&instance_path(dir, &cfg.id), &to_json(&stored)?)
}

pub fn remove_instance(dir: &Path, id: &str) -> Result<(), String> {
    match fs::remove_file(instance_path(dir, id)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot remove {id}.json: {e}")),
    }
}

/// Writes `.gitignore` when the dir has none. Never overwrites a user's file.
pub fn ensure_gitignore(dir: &Path) -> Result<(), String> {
    let path = dir.join(".gitignore");
    if path.exists() {
        return Ok(());
    }
    write_atomic(&path, GITIGNORE.as_bytes())
}

/// Splits a legacy single `config.json` into the new layout, then renames it to `config.json.migrated`.
/// Safe to re-run: a legacy folder that already has an instance file is skipped, and an existing
/// `config.json.migrated` is never overwritten.
pub fn migrate_legacy(dir: &Path, home: Option<&Path>) -> Result<(), String> {
    let legacy = dir.join(LEGACY_FILE);
    if !legacy.exists() {
        return Ok(());
    }
    let bytes = fs::read(&legacy).map_err(|e| format!("cannot read {LEGACY_FILE}: {e}"))?;
    let old: AppConfig = serde_json::from_slice(&bytes)
        .map_err(|e| format!("cannot migrate {LEGACY_FILE}, left in place: {e}"))?;
    if !dir.join(LOCAL_SETTINGS_FILE).exists() {
        save_settings(dir, &old.settings)?;
    }
    let mut present: Vec<PathBuf> = load(dir, home)
        .config
        .instances
        .iter()
        .map(|c| to_portable(&c.folder, home))
        .collect();
    for mut cfg in old.instances {
        let portable = to_portable(&cfg.folder, home);
        if present.contains(&portable) {
            continue;
        }
        // Legacy ids were random. New ids come from the folder and never collide with a file on disk.
        cfg.id = unique_id(&slug_of(&cfg.folder), |id| instance_exists(dir, id));
        save_instance(dir, &cfg, home)?;
        present.push(portable);
    }
    ensure_gitignore(dir)?;
    if dir.join(MIGRATED_FILE).exists() {
        return Ok(());
    }
    fs::rename(&legacy, dir.join(MIGRATED_FILE))
        .map_err(|e| format!("cannot rename {LEGACY_FILE}: {e}"))
}

pub fn write_status(dir: &Path, views: &[crate::model::InstanceView]) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(views).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(STATUS_FILE), &bytes)
}

/// Pretty JSON, 2-space indent, LF, trailing newline. Key order follows the struct fields.
fn to_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text.into_bytes())
}

/// Skips the write when the file already holds these bytes. No mtime churn for git.
fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    write_atomic(path, bytes)
}

/// Writes to `<name>.tmp`, fsyncs, then renames over the target. Readers never see half a file.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let result = (|| {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let tmp = path.with_file_name(format!("{name}.tmp"));
        let mut file = fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    result.map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_round_trips() {
        let home = Path::new("/home/me");
        let folder = Path::new("/home/me/dev/app");
        let stored = to_portable(folder, Some(home));
        assert_eq!(stored, PathBuf::from("~/dev/app"));
        assert_eq!(from_portable(&stored, Some(home)), folder);
        assert_eq!(to_portable(home, Some(home)), PathBuf::from("~"));
        assert_eq!(from_portable(Path::new("~"), Some(home)), home);
        let outside = Path::new("/srv/app");
        assert_eq!(to_portable(outside, Some(home)), outside);
        assert_eq!(from_portable(outside, Some(home)), outside);
    }

    #[test]
    fn slugs_and_collisions() {
        assert_eq!(slug_of(Path::new("/a/My Project_v2")), "my-project-v2");
        assert_eq!(slug_of(Path::new("/")), "instance");
        assert_eq!(slug_of(Path::new("/a/con")), "con-instance");
        assert_eq!(slug_of(Path::new("/a/COM1")), "com1-instance");
        assert_eq!(slug_of(Path::new("/a/com10")), "com10");
        let taken = ["app", "app-2"];
        assert_eq!(unique_id("app", |id| taken.contains(&id)), "app-3");
        assert_eq!(unique_id("web", |id| taken.contains(&id)), "web");
    }
}
