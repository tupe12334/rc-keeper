use std::thread;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter as _};

use crate::{daemon, tray};

/// Polls the daemon once per second: emits `instances` and refreshes the tray tooltip.
/// Never spawns the daemon, so "Stop daemon" stays stopped.
pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let mut client = None;
        loop {
            if client.is_none() {
                client = daemon::try_ping().map(|(c, _)| c);
            }
            let views = client
                .as_mut()
                .and_then(|c| c.call("list_instances", json!({})).ok());
            match views {
                Some(views) => {
                    // ponytail: emit errors (no window yet, window gone) are ignored; next tick retries.
                    let _ = app.emit("instances", &views);
                    set_tooltip(&app, &tray::summary(&views));
                }
                None => {
                    client = None;
                    set_tooltip(&app, tray::OFFLINE_TEXT);
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

fn set_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(tray::TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}
