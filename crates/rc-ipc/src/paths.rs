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

/// `$XDG_RUNTIME_DIR/rc-keeper`, else `/tmp/rc-keeper-<uid>`. Short, so the path fits `sun_path`.
#[cfg(unix)]
pub fn runtime_dir() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        Some(dir) => PathBuf::from(dir).join("rc-keeper"),
        None => PathBuf::from(format!("/tmp/rc-keeper-{}", current_uid())),
    }
}

#[cfg(not(unix))]
pub fn runtime_dir() -> PathBuf {
    config_dir()
}

#[cfg(unix)]
fn current_uid() -> u32 {
    // SAFETY: getuid cannot fail.
    unsafe { libc::getuid() }
}

/// Makes the runtime dir and checks it is ours: a real dir, owned by this uid, mode 0700.
/// Refuses anything else, so another user cannot plant a socket there.
#[cfg(unix)]
fn ensure_runtime_dir() -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let dir = runtime_dir();
    match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    // symlink_metadata: a symlink fails the is_dir check instead of being followed.
    let meta = std::fs::symlink_metadata(&dir)?;
    let uid = current_uid();
    if !meta.is_dir() || meta.uid() != uid || meta.permissions().mode() & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "runtime dir {} must be a real directory owned by uid {uid} with mode 0700",
                dir.display()
            ),
        ));
    }
    Ok(())
}

/// First 12 hex digits of FNV-1a over the canonical config dir. Stable across builds.
// ponytail: FNV, not sha256: no new dep, and the hash only names a socket.
fn config_hash() -> String {
    let dir = canonical(&config_dir());
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in dir.to_string_lossy().bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")[..12].to_string()
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
