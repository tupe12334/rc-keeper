//! Latest-release lookup on GitHub, and the marker that says an update runs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

pub const REPO: &str = "tupe12334/rc-keeper";
const MARKER: &str = "update.json";
/// An older marker is stale. A killed update never removes its marker.
const MARKER_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

/// Tag of the latest GitHub release. Uses curl, which install.sh needs too.
pub fn latest_tag() -> Result<String, String> {
    // Same override as install.sh, so tests can point at a fake server.
    let base =
        std::env::var("RC_KEEPER_API_BASE").unwrap_or_else(|_| "https://api.github.com".into());
    let url = format!("{base}/repos/{REPO}/releases/latest");
    let out = Command::new("curl")
        .args(["-fsSL", "-H", "Accept: application/vnd.github+json", &url])
        .output()
        .map_err(|e| format!("cannot run curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("could not read the latest release from {url}"));
    }
    parse_latest_tag(&String::from_utf8_lossy(&out.stdout))
}

/// Reads `tag_name` from the JSON of GitHub's "latest release" endpoint.
pub fn parse_latest_tag(body: &str) -> Result<String, String> {
    let release: Release =
        serde_json::from_str(body).map_err(|e| format!("cannot read release JSON: {e}"))?;
    if release.tag_name.is_empty() {
        return Err("release has no tag name".into());
    }
    Ok(release.tag_name)
}

/// `(major, minor, patch)` of `1.2.3` or `v1.2.3`. Pre-release and build parts are dropped.
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// True when `latest` is a higher `major.minor.patch` than `current`.
// shortcut: pre-releases compare as their core version, so 1.2.3-rc.1 is not newer than 1.2.3.
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(l), Some(c)) if l > c)
}

/// Removes the update marker when dropped.
pub struct UpdateGuard {
    path: PathBuf,
}

impl Drop for UpdateGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Writes the marker that says an update to `target` runs. It goes away when the guard drops.
pub fn begin_update(dir: &Path, target: &str) -> Result<UpdateGuard, String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let path = dir.join(MARKER);
    fs::write(&path, serde_json::json!({ "target": target }).to_string())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(UpdateGuard { path })
}

/// Target version of an update that runs in `dir`. None when no fresh marker exists.
pub fn update_in_progress(dir: &Path) -> Option<String> {
    let path = dir.join(MARKER);
    let age = fs::metadata(&path).ok()?.modified().ok()?.elapsed().ok()?;
    if age > MARKER_TTL {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).ok()?).ok()?;
    value["target"].as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{begin_update, is_newer, parse_latest_tag, parse_version, update_in_progress};

    #[test]
    fn parses_plain_and_v_prefixed_versions() {
        assert_eq!(parse_version("0.3.1"), Some((0, 3, 1)));
        assert_eq!(parse_version("v1.10.0"), Some((1, 10, 0)));
        assert_eq!(parse_version(" v2.0.0 "), Some((2, 0, 0)));
    }

    #[test]
    fn drops_pre_release_and_build_parts() {
        assert_eq!(parse_version("v1.2.3-rc.1"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2.3+build.7"), Some((1, 2, 3)));
    }

    #[test]
    fn rejects_malformed_versions() {
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("latest"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn compares_numerically_not_as_text() {
        assert!(is_newer("v0.4.0", "0.3.1"));
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(!is_newer("v0.3.1", "0.3.1"));
        assert!(!is_newer("v0.3.0", "0.3.1"));
        assert!(!is_newer("not-a-version", "0.3.1"));
    }

    #[test]
    fn reads_tag_from_release_json() {
        let body =
            r#"{"tag_name":"v0.4.0","name":"0.4.0","assets":[{"browser_download_url":"x"}]}"#;
        assert_eq!(parse_latest_tag(body).unwrap(), "v0.4.0");
    }

    #[test]
    fn release_json_without_tag_is_an_error() {
        assert!(parse_latest_tag(r#"{"message":"Not Found"}"#).is_err());
        assert!(parse_latest_tag(r#"{"tag_name":""}"#).is_err());
        assert!(parse_latest_tag("<html>").is_err());
    }

    #[test]
    fn update_marker_lives_while_guard_lives() {
        let dir = std::env::temp_dir().join(format!("rc-core-update-test-{}", std::process::id()));
        assert_eq!(update_in_progress(&dir), None);
        let guard = begin_update(&dir, "v0.4.0").unwrap();
        assert_eq!(update_in_progress(&dir).as_deref(), Some("v0.4.0"));
        drop(guard);
        assert_eq!(update_in_progress(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
