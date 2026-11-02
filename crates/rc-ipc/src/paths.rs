//! Config dir and socket location.

use std::path::PathBuf;

use interprocess::local_socket::Name;

pub const APP_ID: &str = "io.github.tupe12334.rckeeper";
const SOCKET_FILE: &str = "rc-keeper.sock";

/// `RC_KEEPER_CONFIG_DIR` wins; else the platform config dir plus the app id.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("RC_KEEPER_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_ID)
}

pub fn socket_path() -> PathBuf {
    config_dir().join(SOCKET_FILE)
}

/// Unix: the socket file. Windows: named pipe `rc-keeper-<user>`.
pub(crate) fn socket_name() -> std::io::Result<Name<'static>> {
    #[cfg(unix)]
    {
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        socket_path().to_fs_name::<GenericFilePath>()
    }
    #[cfg(windows)]
    {
        use interprocess::local_socket::{GenericNamespaced, ToNsName};
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
        format!("rc-keeper-{user}").to_ns_name::<GenericNamespaced>()
    }
}
