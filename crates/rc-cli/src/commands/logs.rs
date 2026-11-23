//! logs <id|folder> [-f]

use std::thread;
use std::time::Duration;

use anyhow::Result;
use rc_ipc::Client;
use serde_json::json;

use super::{connect, resolve};

pub fn run(selector: &str, follow: bool) -> Result<()> {
    let mut client = connect()?;
    let id = resolve(&mut client, selector)?.config.id;
    let mut seen = get_logs(&mut client, &id)?;
    for line in &seen {
        println!("{line}");
    }
    if !follow {
        return Ok(());
    }
    loop {
        thread::sleep(Duration::from_secs(1));
        // A fresh connection per poll, so one timed-out call does not end the stream.
        let now = get_logs(&mut connect()?, &id)?;
        for line in new_lines(&seen, &now) {
            println!("{line}");
        }
        seen = now;
    }
}

fn get_logs(client: &mut Client, id: &str) -> Result<Vec<String>> {
    Ok(serde_json::from_value(
        client.call("get_logs", json!({ "id": id }))?,
    )?)
}

/// Lines in `now` after the last line of `prev`. The log is a rolling window, so
/// when that line is gone every line of `now` counts as new.
pub fn new_lines<'a>(prev: &[String], now: &'a [String]) -> &'a [String] {
    match prev.last() {
        None => now,
        Some(last) => match now.iter().rposition(|l| l == last) {
            Some(i) => &now[i + 1..],
            None => now,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::new_lines;

    fn v(xs: &[&str]) -> Vec<String> {
        xs.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn empty_previous_prints_everything() {
        assert_eq!(new_lines(&[], &v(&["a", "b"])), &v(&["a", "b"])[..]);
    }

    #[test]
    fn prints_only_lines_after_last_seen() {
        assert_eq!(
            new_lines(&v(&["a", "b"]), &v(&["a", "b", "c"])),
            &v(&["c"])[..]
        );
    }

    #[test]
    fn unchanged_log_prints_nothing() {
        assert!(new_lines(&v(&["a", "b"]), &v(&["a", "b"])).is_empty());
    }

    #[test]
    fn rolled_window_prints_after_surviving_line() {
        assert_eq!(
            new_lines(&v(&["a", "b", "c"]), &v(&["b", "c", "d"])),
            &v(&["d"])[..]
        );
    }

    #[test]
    fn vanished_last_line_prints_whole_window() {
        assert_eq!(new_lines(&v(&["a"]), &v(&["x", "y"])), &v(&["x", "y"])[..]);
    }
}
