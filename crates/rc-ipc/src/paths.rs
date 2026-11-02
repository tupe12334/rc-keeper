//! Config dir, runtime dir and socket location.

use std::io;
use std::path::{Path, PathBuf};

use interprocess::local_socket::Name;

pub const APP_ID: &str = "io.github.tupe12334.rckeeper";
/// Socket name before the runtime dir existed. Old daemons listen here, so clients still try it.
#[cfg(unix)]
const LEGACY_SOCKET_FILE: &str = "rc-keeper.sock";

/// `RC_KEEPER_CONFIG_DIR` wins; else the platform config dir plus the app id.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("RC_KEEPER_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_ID)
}

/// Unix: `<runtime dir>/<hash>.sock`. The hash names the config dir, so two config dirs get two daemons.
/// Windows: display only, the pipe name is `rc-keeper-<user>-<hash>`.
pub fn socket_path() -> PathBuf {
    runtime_dir().join(format!("{}.sock", config_hash()))
}

/// Unix: `$XDG_RUNTIME_DIR/rc-keeper`, else `$TMPDIR/rc-keeper-<uid>` on macOS and
/// `/tmp/rc-keeper-<uid>` elsewhere. Short, so the socket path fits `sun_path`.
#[cfg(unix)]
pub fn runtime_dir() -> PathBuf {
    match xdg_runtime_dir() {
        Some(xdg) => xdg.join("rc-keeper"),
        None => default_base().join(format!("rc-keeper-{}", current_uid())),
    }
}

#[cfg(not(unix))]
pub fn runtime_dir() -> PathBuf {
    config_dir()
}

#[cfg(unix)]
fn xdg_runtime_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn default_base() -> PathBuf {
    std::env::temp_dir()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_base() -> PathBuf {
    PathBuf::from("/tmp")
}

/// Lock file that allows one daemon per config dir. Unix only.
#[cfg(unix)]
pub(crate) fn lock_path() -> PathBuf {
    runtime_dir().join(format!("{}.lock", config_hash()))
}

#[cfg(unix)]
pub(crate) fn current_uid() -> u32 {
    // SAFETY: getuid cannot fail.
    unsafe { libc::getuid() }
}

/// Makes the runtime dir and checks it is ours. Refuses anything else, so another user cannot plant a socket.
/// With `XDG_RUNTIME_DIR` the parent is checked too: it must not be writable by others.
#[cfg(unix)]
pub(crate) fn ensure_runtime_dir() -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    if let Some(xdg) = xdg_runtime_dir() {
        check_dir(
            &xdg,
            |mode| mode & 0o022 == 0,
            "not writable by group or others",
        )?;
    }
    let dir = runtime_dir();
    match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    check_dir(&dir, |mode| mode == 0o700, "mode 0700")
}

/// `symlink_metadata`, so a symlink fails the is-dir check instead of being followed.
#[cfg(unix)]
fn check_dir(dir: &Path, mode_ok: impl Fn(u32) -> bool, need: &str) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let meta = std::fs::symlink_metadata(dir)?;
    let uid = current_uid();
    if !meta.is_dir() || meta.uid() != uid || !mode_ok(meta.permissions().mode() & 0o777) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{} must be a real directory owned by uid {uid} with {need}",
                dir.display()
            ),
        ));
    }
    Ok(())
}

/// Full 64-bit FNV-1a over the canonical config dir, as 16 hex digits.
// ponytail: FNV, not sha256: no new dep, and the hash only names a socket. Identity is checked by ping.
fn config_hash() -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in canonical_config_dir().bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// What `ping` reports as `configDir`. Clients compare it with their own.
pub(crate) fn canonical_config_dir() -> String {
    canonical(&config_dir()).to_string_lossy().into_owned()
}

/// Canonical form of the nearest existing ancestor, so the hash is the same before the dir exists.
fn canonical(dir: &Path) -> PathBuf {
    if let Ok(p) = dir.canonicalize() {
        return p;
    }
    match (dir.parent(), dir.file_name()) {
        (Some(parent), Some(name)) => canonical(parent).join(name),
        _ => dir.to_path_buf(),
    }
}

/// Names to try, newest first. Old Unix daemons listen only on the legacy file.
pub(crate) fn socket_names() -> io::Result<Vec<Name<'static>>> {
    #[cfg(unix)]
    {
        Ok(vec![socket_name()?, legacy_socket_name()?])
    }
    #[cfg(not(unix))]
    {
        Ok(vec![socket_name()?])
    }
}

/// Unix: the socket file. Windows: named pipe `rc-keeper-<user>-<hash>`.
pub(crate) fn socket_name() -> io::Result<Name<'static>> {
    #[cfg(unix)]
    {
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        ensure_runtime_dir()?;
        socket_path().to_fs_name::<GenericFilePath>()
    }
    #[cfg(windows)]
    {
        use interprocess::local_socket::{GenericNamespaced, ToNsName};
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
        format!("rc-keeper-{user}-{}", config_hash()).to_ns_name::<GenericNamespaced>()
    }
}

#[cfg(unix)]
fn legacy_socket_name() -> io::Result<Name<'static>> {
    use interprocess::local_socket::{GenericFilePath, ToFsName};
    config_dir()
        .join(LEGACY_SOCKET_FILE)
        .to_fs_name::<GenericFilePath>()
}
