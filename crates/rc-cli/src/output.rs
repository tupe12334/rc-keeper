//! Human tables and JSON output.

use anyhow::Result;
use comfy_table::Table;
use rc_core::{InstanceView, State};
use serde::Serialize;

pub fn print_json<T: Serialize + ?Sized>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn state_label(state: &State) -> &'static str {
    match state {
        State::Stopped => "stopped",
        State::Starting => "starting",
        State::Running => "running",
        State::Restarting => "restarting",
        State::FolderBusy => "folder-busy",
        State::NeedsTrust => "needs-trust",
        State::Error => "error",
    }
}

pub fn fmt_uptime(secs: Option<u64>) -> String {
    match secs {
        None => "-".into(),
        Some(s) if s < 60 => format!("{s}s"),
        Some(s) if s < 3600 => format!("{}m{:02}s", s / 60, s % 60),
        Some(s) => format!("{}h{:02}m", s / 3600, (s % 3600) / 60),
    }
}

fn yes_no(v: bool) -> &'static str {
    if v {
        "yes"
    } else {
        "no"
    }
}

pub fn list_table(views: &[InstanceView]) -> Table {
    let mut t = Table::new();
    t.set_header(["ID", "FOLDER", "NAME", "CHROME", "AUTO-START"]);
    for v in views {
        t.add_row([
            v.config.id.clone(),
            v.config.folder.display().to_string(),
            v.config.name.clone().unwrap_or_else(|| "-".into()),
            yes_no(v.config.chrome).into(),
            yes_no(v.config.auto_start).into(),
        ]);
    }
    t
}

pub fn status_table(views: &[InstanceView]) -> Table {
    let mut t = Table::new();
    t.set_header([
        "FOLDER",
        "NAME",
        "STATE",
        "SESSIONS",
        "UPTIME",
        "RESTARTS",
        "LAST ERROR",
    ]);
    for v in views {
        let s = &v.status;
        t.add_row([
            v.config.folder.display().to_string(),
            v.config.name.clone().unwrap_or_else(|| "-".into()),
            state_label(&s.state).into(),
            s.sessions.to_string(),
            fmt_uptime(s.uptime_secs),
            s.restarts.to_string(),
            s.last_error.clone().unwrap_or_else(|| "-".into()),
        ]);
    }
    t
}
