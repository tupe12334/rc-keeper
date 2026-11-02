//! Watches the config dir and its parent. Calls `reload` once config files settle.
use notify::event::{AccessKind, AccessMode, CreateKind};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

/// Quiet period after the last change before a reload.
const QUIET: Duration = Duration::from_millis(300);
/// Upper bound, so a steady stream of events still reloads.
const MAX_WAIT: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_secs(1);

/// The thread ends when `alive` returns false or the event channel closes.
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
    // If the dir is removed or replaced, its inode changes. Only the parent sees that.
    if let Some(parent) = dir.parent()
        && let Err(e) = watcher.watch(parent, RecursiveMode::NonRecursive)
    {
        eprintln!("rc-core: cannot watch {}: {e}", parent.display());
    }
    let mut armed = arm(&mut watcher, &dir);
    thread::spawn(move || {
        let mut dirty_since: Option<Instant> = None;
        loop {
            if !alive() {
                return;
            }
            let timeout = match dirty_since {
                Some(since) => QUIET.min(MAX_WAIT.saturating_sub(since.elapsed())),
                None => POLL,
            };
            let received = rx.recv_timeout(timeout);
            let timed_out = matches!(received, Err(RecvTimeoutError::Timeout));
            match received {
                Ok(Ok(event)) if !is_read_only(&event.kind) => {
                    if event.paths.iter().any(|p| p == &dir) {
                        // The dir itself changed: re-watch the new inode, then reload from disk.
                        armed = arm(&mut watcher, &dir);
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                    // A new subfolder (`instances/`) gets its watch a moment later, so files
                    // written into it meanwhile produce no events. Reload to read them.
                    let new_folder = matches!(event.kind, EventKind::Create(CreateKind::Folder));
                    if new_folder || event.paths.iter().any(|p| is_config_file(&dir, p)) {
                        dirty_since.get_or_insert_with(Instant::now);
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) => {
                    dirty_since.get_or_insert_with(Instant::now);
                }
                Err(RecvTimeoutError::Timeout) => {
                    // The dir was missing at the last attempt. Retry until it returns.
                    if !armed {
                        armed = arm(&mut watcher, &dir);
                        if armed {
                            dirty_since.get_or_insert_with(Instant::now);
                        }
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
            let due = dirty_since.is_some_and(|since| timed_out || since.elapsed() >= MAX_WAIT);
            if due {
                dirty_since = None;
                reload();
            }
        }
    });
    Ok(())
}

/// Drops any watch on a stale inode, then watches the dir as it is now.
fn arm(watcher: &mut RecommendedWatcher, dir: &Path) -> bool {
    let _ = watcher.unwatch(dir);
    watcher.watch(dir, RecursiveMode::Recursive).is_ok()
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

/// Opens and reads change nothing. Our own reload reads files, and inotify reports each read,
/// so reacting to them would reload forever. `Close(Write)` is a real write, so it stays.
fn is_read_only(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Access(a) if *a != AccessKind::Close(AccessMode::Write))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::ModifyKind;

    #[test]
    fn only_reads_are_ignored() {
        assert!(is_read_only(&EventKind::Access(AccessKind::Open(
            AccessMode::Read
        ))));
        assert!(is_read_only(&EventKind::Access(AccessKind::Read)));
        assert!(is_read_only(&EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(!is_read_only(&EventKind::Access(AccessKind::Close(
            AccessMode::Write
        ))));
        assert!(!is_read_only(&EventKind::Modify(ModifyKind::Any)));
    }
}
