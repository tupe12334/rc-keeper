//! Turns raw PTY bytes into events. No I/O and no threads, so it is easy to test.
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;
use strip_ansi_escapes::Writer as AnsiStripper;

const BUSY: &str = "already served by a terminal";
const TRUST: &str = "[y/N]";
const SESSION_URL: &str = "https://claude.ai/code";
/// Unfinished-line cap. Keeps memory flat when a program prints without newlines.
const MAX_PARTIAL: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Some output arrived.
    Output,
    /// One complete line, ANSI stripped, without its line break.
    Line(String),
    FolderBusy,
    TrustPrompt,
    Error(String),
    SessionUrl(String),
}

/// Collects what the ANSI stripper writes out. `Rc` because the stripper owns its writer.
struct Sink(Rc<RefCell<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Parser {
    ansi: AnsiStripper<Sink>,
    clean: Rc<RefCell<Vec<u8>>>,
    /// Incomplete UTF-8 sequence left at the end of the last chunk.
    utf8_rest: Vec<u8>,
    /// Text after the last line break. Prompts arrive here because they have no newline.
    partial: String,
}

impl Parser {
    pub fn new() -> Self {
        let clean = Rc::new(RefCell::new(Vec::new()));
        Self {
            ansi: AnsiStripper::new(Sink(Rc::clone(&clean))),
            clean,
            utf8_rest: Vec::new(),
            partial: String::new(),
        }
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Vec<Event> {
        if chunk.is_empty() {
            return Vec::new();
        }
        // Flush so the stripper never holds back a newline-less prompt.
        let _ = self.ansi.write_all(chunk);
        let _ = self.ansi.flush();
        let bytes = std::mem::take(&mut *self.clean.borrow_mut());
        let text = self.decode(bytes);

        let mut events = vec![Event::Output];
        self.partial.push_str(&text);
        while let Some(pos) = self.partial.find('\n') {
            let raw: String = self.partial.drain(..=pos).collect();
            classify_line(raw.trim_end_matches(['\r', '\n']), &mut events);
        }
        // Prompts and busy notices may sit on an unfinished line.
        if self.partial.contains(BUSY) {
            events.push(Event::FolderBusy);
            self.partial.clear();
        }
        if self.partial.contains(TRUST) {
            events.push(Event::TrustPrompt);
            self.partial.clear();
        }
        self.trim_partial();
        events
    }

    /// Decodes UTF-8, holding back a sequence that a later chunk completes.
    fn decode(&mut self, bytes: Vec<u8>) -> String {
        let mut buf = std::mem::take(&mut self.utf8_rest);
        buf.extend(bytes);
        let mut out = String::new();
        let mut rest = buf.as_slice();
        loop {
            match std::str::from_utf8(rest) {
                Ok(text) => {
                    out.push_str(text);
                    break;
                }
                Err(e) => {
                    let (valid, tail) = rest.split_at(e.valid_up_to());
                    out.push_str(&String::from_utf8_lossy(valid));
                    match e.error_len() {
                        Some(n) => {
                            out.push('\u{FFFD}');
                            rest = &tail[n..];
                        }
                        None => {
                            self.utf8_rest = tail.to_vec();
                            break;
                        }
                    }
                }
            }
        }
        out
    }

    fn trim_partial(&mut self) {
        if self.partial.len() <= MAX_PARTIAL {
            return;
        }
        let mut cut = self.partial.len() - MAX_PARTIAL / 2;
        while !self.partial.is_char_boundary(cut) {
            cut += 1;
        }
        self.partial.drain(..cut);
    }
}

fn classify_line(line: &str, events: &mut Vec<Event>) {
    if line.contains(BUSY) {
        events.push(Event::FolderBusy);
    }
    if line.contains(TRUST) {
        events.push(Event::TrustPrompt);
    }
    let trimmed = line.trim();
    if trimmed.starts_with("Error:") || line.contains("ECONNRESET") || line.contains("ENOTFOUND") {
        events.push(Event::Error(trimmed.to_owned()));
    }
    if let Some(start) = line.find(SESSION_URL) {
        let url = line[start..].split_whitespace().next().unwrap_or_default();
        events.push(Event::SessionUrl(url.to_owned()));
    }
    events.push(Event::Line(line.to_owned()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_line_is_folder_busy() {
        let events = Parser::new().feed(b"this folder is already served by a terminal\r\n");
        assert!(events.contains(&Event::FolderBusy));
    }

    #[test]
    fn trust_prompt_split_across_chunks_fires_once() {
        let mut parser = Parser::new();
        let mut events = parser.feed(b"Trust this folder? [");
        events.extend(parser.feed(b"y/N] "));
        let prompts = events.iter().filter(|e| **e == Event::TrustPrompt).count();
        assert_eq!(prompts, 1);
    }

    #[test]
    fn ansi_codes_are_stripped() {
        let events = Parser::new().feed(b"\x1b[31mRemote Control\x1b[0m ready\r\n");
        assert!(events.contains(&Event::Line("Remote Control ready".into())));
    }

    #[test]
    fn session_url_is_extracted() {
        let events = Parser::new().feed(b"Open https://claude.ai/code/session_1 now\r\n");
        assert!(events.contains(&Event::SessionUrl(
            "https://claude.ai/code/session_1".into()
        )));
    }

    #[test]
    fn error_line_is_reported() {
        let events = Parser::new().feed(b"Error: getaddrinfo ENOTFOUND api.example\n");
        assert!(events.contains(&Event::Error(
            "Error: getaddrinfo ENOTFOUND api.example".into()
        )));
    }

    #[test]
    fn utf8_split_across_chunks_is_joined() {
        let mut parser = Parser::new();
        let mut events = parser.feed(&[0xE2]);
        events.extend(parser.feed(&[0x94, 0x80, b'\n']));
        assert!(events.contains(&Event::Line("─".into())));
    }
}
