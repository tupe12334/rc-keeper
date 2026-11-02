//! `<id|folder>` instance selector.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use rc_core::InstanceView;

/// Canonical absolute path. Errors when the folder does not exist.
pub fn canonical(path: &Path) -> Result<PathBuf> {
    let p = std::fs::canonicalize(path)
        .with_context(|| format!("no such folder: {}", path.display()))?;
    Ok(strip_verbatim(p))
}

/// The canonical folder for a selector, if the selector names an existing folder.
pub fn folder_of(selector: &str) -> Option<PathBuf> {
    canonical(Path::new(selector)).ok()
}

/// Exact id wins. Otherwise a folder match. Errors on none or several.
pub fn select(
    views: Vec<InstanceView>,
    selector: &str,
    folder: Option<&Path>,
) -> Result<InstanceView> {
    let (by_id, rest): (Vec<_>, Vec<_>) = views.into_iter().partition(|v| v.config.id == selector);
    let mut hits = if by_id.is_empty() {
        // Compare canonical paths: a stored folder may hold a symlink.
        rest.into_iter()
            .filter(|v| {
                folder.is_some_and(|f| canonical(&v.config.folder).ok().as_deref() == Some(f))
            })
            .collect()
    } else {
        by_id
    };
    match hits.len() {
        0 => bail!("no instance matches `{selector}`"),
        1 => Ok(hits.remove(0)),
        n => bail!("`{selector}` matches {n} instances; use the id"),
    }
}

#[cfg(windows)]
fn strip_verbatim(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(s) => PathBuf::from(s),
        None => p,
    }
}

#[cfg(not(windows))]
fn strip_verbatim(p: PathBuf) -> PathBuf {
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_core::{InstanceConfig, InstanceStatus, State};

    /// A real canonical folder, so folder matching has something to compare.
    fn existing_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rck-sel-dir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::canonicalize(dir).unwrap()
    }

    fn view(id: &str, folder: &str) -> InstanceView {
        InstanceView {
            config: InstanceConfig {
                id: id.into(),
                folder: PathBuf::from(folder),
                name: None,
                chrome: true,
                permission_mode: None,
                spawn: None,
                capacity: None,
                create_session_in_dir: None,
                session_name_prefix: None,
                extra_args: vec![],
                auto_trust: true,
                auto_start: true,
            },
            status: InstanceStatus {
                id: id.into(),
                state: State::Stopped,
                pid: None,
                sessions: 0,
                uptime_secs: None,
                restarts: 0,
                next_retry_secs: None,
                last_error: None,
                session_urls: vec![],
            },
        }
    }

    #[test]
    fn exact_id_selects() {
        let views = vec![view("a", "/x"), view("b", "/y")];
        assert_eq!(select(views, "b", None).unwrap().config.id, "b");
    }

    #[test]
    fn folder_selects_when_no_id_matches() {
        let dir = existing_dir();
        let d = dir.to_str().unwrap();
        let views = vec![view("a", "/nonexistent-x"), view("b", d)];
        assert_eq!(select(views, d, Some(&dir)).unwrap().config.id, "b");
    }

    #[cfg(unix)]
    #[test]
    fn folder_matches_when_stored_path_is_a_symlink() {
        let base = std::env::temp_dir().join(format!("rck-sel-{}", std::process::id()));
        let real = base.join("real");
        let link = base.join("link");
        std::fs::create_dir_all(&real).unwrap();
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let canon = std::fs::canonicalize(&real).unwrap();
        let views = vec![view("a", link.to_str().unwrap())];
        let found = select(views, canon.to_str().unwrap(), Some(&canon));
        let _ = std::fs::remove_dir_all(&base);
        assert_eq!(found.unwrap().config.id, "a");
    }

    #[test]
    fn unknown_selector_errors() {
        let views = vec![view("a", "/x")];
        assert!(select(views, "zzz", None).is_err());
    }

    #[test]
    fn two_folder_matches_are_ambiguous() {
        let dir = existing_dir();
        let d = dir.to_str().unwrap();
        let views = vec![view("a", d), view("b", d)];
        let err = select(views, d, Some(&dir)).unwrap_err();
        assert!(err.to_string().contains("matches 2"));
    }

    #[test]
    fn id_beats_folder_on_collision() {
        let views = vec![view("/x", "/other"), view("a", "/x"), view("b", "/x")];
        assert_eq!(
            select(views, "/x", Some(Path::new("/x")))
                .unwrap()
                .config
                .id,
            "/x"
        );
    }
}
