//! Watches the config dir. Calls `reload` once config files have been quiet for 300 ms.
use notify::{RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

const QUIET: Duration = Duration::from_millis(300);
const POLL: Duration = Duration::from_secs(1);

/// The thread ends when `alive` returns false or the watcher fails.
pub fn spawn(
    dir: PathBuf,
    alive: impl Fn() -> bool + Send + 'static,
    reload: impl Fn() + Send + 'static,
) -> notify::Result<()> {
    // Event paths come back canonical (macOS /private/var), so compare canonical paths.
    let dir = dir.canonicalize().unwrap_or(dir);
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let _ = tx.send(event);
    })?;
    watcher.watch(&dir, RecursiveMode::Recursive)?;
    thread::spawn(move || {
        let _watcher = watcher; // Dropping the watcher stops the watch.
        let mut dirty = false;
        loop {
            if !alive() {
                return;
            }
            match rx.recv_timeout(if dirty { QUIET } else { POLL }) {
                Ok(Ok(event)) => dirty |= event.paths.iter().any(|p| is_config_file(&dir, p)),
                Ok(Err(_)) => dirty = true,
                Err(RecvTimeoutError::Timeout) => {
                    if std::mem::take(&mut dirty) {
                        reload();
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    });
    Ok(())
}

/// `settings*.json`, `config.json` and `instances/*.json`. Runtime files (status, log, tmp) are ignored.
fn is_config_file(dir: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(dir) else {
        return false;
    };
    if rel.extension().is_none_or(|e| e != "json") {
        return false;
    }
    match rel.parent() {
        Some(p) if p == Path::new("instances") => true,
        Some(p) if p == Path::new("") => matches!(
            rel.file_name().and_then(|n| n.to_str()),
            Some("settings.json" | "settings.local.json" | "config.json")
        ),
        _ => false,
    }
}
