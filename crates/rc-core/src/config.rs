//! On-disk files: `config.json` (instances and settings) and `status.json` (read by the CLI).
use crate::model::{AppConfig, InstanceView};
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

pub const CONFIG_FILE: &str = "config.json";
pub const STATUS_FILE: &str = "status.json";

pub fn read_config(dir: &Path) -> Result<AppConfig, String> {
    match fs::read(dir.join(CONFIG_FILE)) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| format!("invalid {CONFIG_FILE}: {e}"))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(AppConfig::default()),
        Err(e) => Err(format!("cannot read {CONFIG_FILE}: {e}")),
    }
}

pub fn write_config(dir: &Path, cfg: &AppConfig) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(cfg).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(CONFIG_FILE), &bytes)
}

pub fn write_status(dir: &Path, views: &[InstanceView]) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(views).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(STATUS_FILE), &bytes)
}

/// Writes to a temp file, then renames, so readers never see a half-written file.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let result = fs::File::create(&tmp)
        .and_then(|mut f| {
            f.write_all(bytes)?;
            f.sync_all()
        })
        .and_then(|_| fs::rename(&tmp, path));
    result.map_err(|e| format!("cannot write {}: {e}", path.display()))
}
