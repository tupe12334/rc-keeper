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
        rest.into_iter()
            .filter(|v| folder == Some(v.config.folder.as_path()))
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
        let views = vec![view("a", "/x"), view("b", "/y")];
        assert_eq!(
            select(views, "/y", Some(Path::new("/y")))
                .unwrap()
                .config
                .id,
            "b"
        );
    }

    #[test]
    fn unknown_selector_errors() {
        let views = vec![view("a", "/x")];
        assert!(select(views, "zzz", None).is_err());
    }

    #[test]
    fn two_folder_matches_are_ambiguous() {
        let views = vec![view("a", "/x"), view("b", "/x")];
        let err = select(views, "/x", Some(Path::new("/x"))).unwrap_err();
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
