//! Per-instance log buffer. Collapses a block of lines that repeats, such as a TUI redraw.
use std::collections::VecDeque;

/// Shown lines kept, repeat markers included. The oldest line drops first.
const LOG_LINES: usize = 500;
/// Longest block that collapses. A longer repeat shows in full.
// ponytail: block cap, a longer redraw block is stored in full. Raise it if redraws grow.
const MAX_BLOCK: usize = 10;

/// What the newest held line means for the repeat in progress.
enum Check {
    /// The held lines copy the last `k` stored lines. A full copy.
    Repeat(usize),
    /// The held lines may still become a copy. Hold them.
    Partial,
    /// No copy fits. Store the held lines as they are.
    Mismatch,
}

#[derive(Default)]
pub struct LogBuffer {
    /// Lines shown to the reader, repeat marker included.
    shown: VecDeque<String>,
    /// Last raw lines stored in `shown`, newest last. Never a marker.
    recent: VecDeque<String>,
    /// Lines not yet shown. Each may start a copy of a block in `recent`.
    held: Vec<String>,
    /// Length of the block being repeated, or 0 when no repeat runs.
    period: usize,
    /// Full copies of that block dropped so far.
    repeats: usize,
}

impl LogBuffer {
    pub fn push(&mut self, line: String) {
        self.held.push(line);
        loop {
            match self.check() {
                Check::Repeat(k) => return self.collapse(k),
                Check::Partial => return,
                Check::Mismatch if self.held.len() == 1 => return self.release(),
                Check::Mismatch => {
                    // Store the older held lines. Check the newest line again.
                    let newest = self.held.pop();
                    self.release();
                    self.held.extend(newest);
                }
            }
        }
    }

    /// Last `LOG_LINES` of shown lines followed by held lines. A held line is not yet known to repeat.
    pub fn lines(&self) -> Vec<String> {
        let all: Vec<String> = self.shown.iter().chain(&self.held).cloned().collect();
        let skip = all.len().saturating_sub(LOG_LINES);
        all.into_iter().skip(skip).collect()
    }

    fn check(&self) -> Check {
        let (lo, hi) = if self.period > 0 {
            (self.period, self.period)
        } else {
            (1, MAX_BLOCK.min(self.recent.len()))
        };
        let held = self.held.len();
        let mut partial = false;
        for k in lo..=hi {
            if held > k {
                continue;
            }
            let block = self.recent.iter().skip(self.recent.len() - k);
            if self.held.iter().zip(block).all(|(a, b)| a == b) {
                if held == k {
                    return Check::Repeat(k);
                }
                partial = true;
            }
        }
        if partial {
            Check::Partial
        } else {
            Check::Mismatch
        }
    }

    /// Drops the held copy and updates the marker. The first copy stays in `shown`.
    fn collapse(&mut self, k: usize) {
        self.held.clear();
        self.period = k;
        self.repeats += 1;
        let marker = format!("… previous {k} lines repeated {}x", self.repeats);
        if self.repeats == 1 {
            self.add_shown(marker);
        } else if let Some(last) = self.shown.back_mut() {
            *last = marker;
        }
    }

    /// Ends any repeat and stores every held line as a plain line.
    fn release(&mut self) {
        self.period = 0;
        self.repeats = 0;
        for line in std::mem::take(&mut self.held) {
            self.add_shown(line.clone());
            self.recent.push_back(line);
            if self.recent.len() > MAX_BLOCK {
                self.recent.pop_front();
            }
        }
    }

    fn add_shown(&mut self, line: String) {
        if self.shown.len() == LOG_LINES {
            self.shown.pop_front();
        }
        self.shown.push_back(line);
    }
}

#[cfg(test)]
mod tests {
    use super::LogBuffer;

    fn push_all(buf: &mut LogBuffer, lines: &[&str]) {
        for line in lines {
            buf.push((*line).to_owned());
        }
    }

    const BLOCK: [&str; 3] = [
        "space to show QR code",
        "Connected · ofek",
        "mac-lan-lively-shore",
    ];

    #[test]
    fn repeated_block_collapses() {
        let mut buf = LogBuffer::default();
        for _ in 0..72 {
            push_all(&mut buf, &BLOCK);
        }
        let mut want: Vec<String> = BLOCK.iter().map(|s| (*s).to_owned()).collect();
        want.push("… previous 3 lines repeated 71x".to_owned());
        assert_eq!(buf.lines(), want);
    }

    #[test]
    fn single_repeated_line_collapses() {
        let mut buf = LogBuffer::default();
        push_all(&mut buf, &["x", "x", "x", "x", "x"]);
        assert_eq!(buf.lines(), ["x", "… previous 1 lines repeated 4x"]);
    }

    #[test]
    fn differing_lines_all_show() {
        let mut buf = LogBuffer::default();
        push_all(&mut buf, &["a", "b", "c", "d"]);
        assert_eq!(buf.lines(), ["a", "b", "c", "d"]);
    }

    #[test]
    fn error_line_after_repeat_shows() {
        let mut buf = LogBuffer::default();
        for _ in 0..5 {
            push_all(&mut buf, &BLOCK);
        }
        push_all(&mut buf, &["Error: exit 1"]);
        let lines = buf.lines();
        assert_eq!(lines.last().map(String::as_str), Some("Error: exit 1"));
        assert!(lines.iter().any(|l| l == "… previous 3 lines repeated 4x"));
    }

    #[test]
    fn partial_copy_at_end_shows() {
        let mut buf = LogBuffer::default();
        push_all(&mut buf, &BLOCK);
        push_all(&mut buf, &BLOCK[..2]);
        assert_eq!(buf.lines().len(), 5);
    }

    #[test]
    fn lines_cap_holds_with_held_block() {
        let mut buf = LogBuffer::default();
        for i in 0..600 {
            buf.push(format!("line {i}"));
        }
        // Start a copy of the last stored lines, so two lines are held when the cap applies.
        push_all(&mut buf, &["line 597", "line 598"]);
        let lines = buf.lines();
        assert!(lines.len() <= 500, "got {}", lines.len());
        assert_eq!(lines.len(), 500);
        assert_eq!(lines.last().map(String::as_str), Some("line 598"));
    }
}
