//! Keeps the computer from sleeping while the daemon runs, so rc stays reachable.
//! macOS: `caffeinate -ims -w <daemon pid>`. Linux: `systemd-inhibit` around `sleep infinity`.
//! Windows: a thread holds `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)`.
//! The display may still turn off. A closed laptop lid can still sleep the machine (OS policy).

/// Holds the OS sleep block. Dropping it releases the block.
#[derive(Default)]
pub struct KeepAwake {
    hold: Option<Hold>,
}

impl KeepAwake {
    pub fn is_active(&self) -> bool {
        self.hold.is_some()
    }

    /// Takes or releases the sleep block. Does nothing when it is already in the wanted state.
    pub fn set(&mut self, on: bool) -> Result<(), String> {
        if on == self.is_active() {
            return Ok(());
        }
        self.hold = if on { Some(Hold::take()?) } else { None };
        Ok(())
    }
}

#[cfg(unix)]
struct Hold(std::process::Child);

#[cfg(unix)]
impl Hold {
    fn take() -> Result<Hold, String> {
        use std::process::{Command, Stdio};
        let mut cmd = if cfg!(target_os = "macos") {
            // -i idle sleep, -m disk sleep, -s system sleep on AC power. -w exits with the daemon,
            // so a killed daemon never leaves the block behind.
            let mut c = Command::new("/usr/bin/caffeinate");
            c.args(["-ims", "-w", &std::process::id().to_string()]);
            c
        } else {
            let mut c = Command::new("systemd-inhibit");
            c.args([
                "--what=sleep:idle",
                "--who=rc-keeper",
                "--why=Keep claude remote-control reachable",
                "--mode=block",
                "sleep",
                "infinity",
            ]);
            c
        };
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("cannot keep the computer awake: {e}"))?;
        // A missing logind (containers, no systemd) makes systemd-inhibit exit at once.
        std::thread::sleep(std::time::Duration::from_millis(200));
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "cannot keep the computer awake: sleep blocker exited ({status})"
            ));
        }
        Ok(Hold(child))
    }
}

#[cfg(unix)]
impl Drop for Hold {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(windows)]
struct Hold {
    stop: std::sync::mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(windows)]
impl Hold {
    fn take() -> Result<Hold, String> {
        use windows_sys::Win32::System::Power::{
            ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
        };
        let (stop, wait) = std::sync::mpsc::channel::<()>();
        let (ready, started) = std::sync::mpsc::channel::<bool>();
        // The state belongs to the calling thread, so one thread sets it and waits for the release.
        let thread = std::thread::spawn(move || {
            // SAFETY: plain Win32 call with flag constants.
            let ok = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) } != 0;
            let _ = ready.send(ok);
            if ok {
                let _ = wait.recv();
                // SAFETY: as above; clears the requirement.
                unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
            }
        });
        if started.recv().unwrap_or(false) {
            Ok(Hold {
                stop,
                thread: Some(thread),
            })
        } else {
            Err("cannot keep the computer awake: SetThreadExecutionState failed".to_owned())
        }
    }
}

#[cfg(windows)]
impl Drop for Hold {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    fn caffeinate_for_me() -> bool {
        let me = std::process::id().to_string();
        let out = std::process::Command::new("pgrep")
            .args(["-f", &format!("caffeinate -ims -w {me}$")])
            .output()
            .expect("pgrep");
        !out.stdout.is_empty()
    }

    #[test]
    fn takes_and_releases_the_block() {
        let mut awake = KeepAwake::default();
        awake.set(true).unwrap();
        assert!(awake.is_active());
        assert!(caffeinate_for_me());
        awake.set(true).unwrap();
        awake.set(false).unwrap();
        assert!(!awake.is_active());
        assert!(!caffeinate_for_me());
    }
}
