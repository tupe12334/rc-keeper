//! One supervisor thread per instance. It runs the claude child in a PTY, feeds its output to
//! the parser, and restarts it with backoff until stop is requested.
use crate::args::build_args;
use crate::backoff::Backoff;
use crate::model::{InstanceConfig, InstanceStatus, State};
use crate::parser::{Event, Parser};
use crate::procs;
use crate::takeover;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const LOG_LINES: usize = 500;
/// Answers typed before the prompt is ready are ignored, so wait before answering.
const TRUST_DELAY: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(100);
/// Pause after a takeover, so the stopped process can exit before the retry.
const TAKEOVER_SETTLE: Duration = Duration::from_secs(1);

/// Per-instance state shared by the manager and the supervisor thread.
#[derive(Default)]
pub struct Shared {
    pub stop: AtomicBool,
    /// Serializes start and stop for this instance. Taken before `live`.
    lifecycle: Mutex<()>,
    live: Mutex<Live>,
}

#[derive(Default)]
pub struct Live {
    pub status: InstanceStatus,
    pub started_at: Option<Instant>,
    pub log: VecDeque<String>,
    pub thread: Option<JoinHandle<()>>,
}

impl Shared {
    pub fn new(id: &str) -> Self {
        let shared = Self::default();
        shared.live().status.id = id.to_owned();
        shared
    }

    pub fn lifecycle(&self) -> MutexGuard<'_, ()> {
        self.lifecycle.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn live(&self) -> MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct Job {
    pub cfg: InstanceConfig,
    pub bin: PathBuf,
}

enum Outcome {
    /// Another session holds the folder. Kill and retry later.
    Busy,
    /// Folder is not trusted and auto-trust is off. Do not retry.
    NeedsTrust,
    /// The child exited on its own.
    Exited,
    /// Stop was requested.
    Stopped,
}

/// Thread body. Returns when stop is requested or when the folder needs trust.
pub fn run(shared: Arc<Shared>, job: Job) {
    let mut backoff = Backoff::default();
    let mut budget = takeover::Budget::default();
    let mut last_risk: Option<String> = None;
    loop {
        let started = Instant::now();
        let outcome = run_once(&shared, &job);
        let ran_for = started.elapsed();
        let (state, delay) = match outcome {
            Outcome::Stopped => return,
            Outcome::NeedsTrust => {
                shared.live().status.state = State::NeedsTrust;
                return;
            }
            Outcome::Busy if job.cfg.force_boot => {
                if !budget.allow(Instant::now()) {
                    let by = last_risk.as_deref().unwrap_or("an unknown parent");
                    let mut live = shared.live();
                    live.status.state = State::Error;
                    live.status.last_error = Some(format!(
                        "foreign rc keeps coming back (respawned by {by}) — disable it"
                    ));
                    return;
                }
                let taken = takeover::take_over(&job.cfg.folder);
                if let Some(risk) = taken.respawn_risk {
                    eprintln!(
                        "rc-core: {} may respawn a stopped rc: {risk}",
                        job.cfg.folder.display()
                    );
                    shared.live().status.last_error =
                        Some(format!("took over folder; respawn risk: {risk}"));
                    last_risk = Some(risk);
                }
                if taken.stopped.is_empty() {
                    (State::FolderBusy, backoff.next_delay(ran_for))
                } else {
                    (State::FolderBusy, TAKEOVER_SETTLE)
                }
            }
            Outcome::Busy => (State::FolderBusy, backoff.next_delay(ran_for)),
            Outcome::Exited => (State::Restarting, backoff.next_delay(ran_for)),
        };
        if !wait_before_restart(&shared, state, delay) {
            return;
        }
    }
}

/// Sleeps out the backoff. Returns false when stop arrives during the wait.
fn wait_before_restart(shared: &Shared, state: State, delay: Duration) -> bool {
    {
        let mut live = shared.live();
        live.status.state = state;
        live.status.next_retry_secs = Some(delay.as_secs());
        live.status.restarts += 1;
    }
    let until = Instant::now() + delay;
    while Instant::now() < until {
        if shared.stop.load(Ordering::SeqCst) {
            return false;
        }
        thread::sleep(POLL);
    }
    shared.live().status.next_retry_secs = None;
    !shared.stop.load(Ordering::SeqCst)
}

fn run_once(shared: &Shared, job: &Job) -> Outcome {
    let pty = match native_pty_system().openpty(PtySize {
        rows: 40,
        cols: 120,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(pty) => pty,
        Err(e) => return fail(shared, format!("openpty: {e}")),
    };
    let mut cmd = CommandBuilder::new(&job.bin);
    cmd.args(build_args(&job.cfg));
    cmd.cwd(&job.cfg.folder);
    let mut child = match pty.slave.spawn_command(cmd) {
        Ok(child) => child,
        Err(e) => return fail(shared, format!("spawn: {e}")),
    };
    drop(pty.slave);
    let pid = child.process_id();
    let mut reader = match pty.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(e) => {
            let _ = child.kill();
            return fail(shared, format!("pty reader: {e}"));
        }
    };
    let mut writer = match pty.master.take_writer() {
        Ok(writer) => writer,
        Err(e) => {
            let _ = child.kill();
            return fail(shared, format!("pty writer: {e}"));
        }
    };

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    {
        let mut live = shared.live();
        live.status.state = State::Starting;
        live.status.pid = pid;
        live.status.next_retry_secs = None;
        // URLs of the last child are dead now.
        live.status.session_urls.clear();
        live.started_at = Some(Instant::now());
    }

    let mut parser = Parser::new();
    let mut session = Session {
        shared,
        auto_trust: job.cfg.auto_trust,
        answer_at: None,
        answered: false,
        question_seen: false,
        ready: false,
        errored: false,
    };
    let outcome = loop {
        if shared.stop.load(Ordering::SeqCst) {
            break Outcome::Stopped;
        }
        if session.answer_at.is_some_and(|at| Instant::now() >= at) {
            session.answer_at = None;
            session.answered = true;
            session.question_seen = false;
            let _ = writer.write_all(b"y\r");
            let _ = writer.flush();
        }
        match rx.recv_timeout(POLL) {
            Ok(chunk) => {
                if let Some(outcome) = session.feed(&mut parser, &chunk) {
                    break outcome;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => thread::sleep(POLL),
        }
        if let Ok(Some(status)) = child.try_wait() {
            // Keep the output printed just before the exit, e.g. a busy notice.
            let mut late = None;
            while let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) {
                if let Some(outcome) = session.feed(&mut parser, &chunk) {
                    late = Some(outcome);
                    break;
                }
            }
            if late.is_none() && !session.errored {
                // No error line, so the exit code is the only reason the user gets.
                shared.live().status.last_error = Some(format!(
                    "claude exited with code {} and printed no error; see the logs",
                    status.exit_code()
                ));
            }
            break late.unwrap_or(Outcome::Exited);
        }
    };

    // Kill leftovers on every exit path, so the PTY slave closes and the reader thread ends.
    let exited = matches!(outcome, Outcome::Exited);
    match pid {
        Some(pid) => {
            if !exited || cfg!(not(unix)) {
                procs::kill_tree(pid, || {
                    let _ = child.kill();
                });
            }
            #[cfg(unix)]
            procs::kill_group(pid);
        }
        None if !exited => {
            let _ = child.kill();
        }
        None => {}
    }
    let _ = child.wait();
    {
        let mut live = shared.live();
        live.status.pid = None;
        live.status.sessions = 0;
        live.started_at = None;
    }
    outcome
}

fn fail(shared: &Shared, message: String) -> Outcome {
    shared.live().status.last_error = Some(message);
    Outcome::Exited
}

/// Reacts to parser events for one child process.
struct Session<'a> {
    shared: &'a Shared,
    auto_trust: bool,
    /// When the trust answer is due. Set by the first trust prompt of the run.
    answer_at: Option<Instant>,
    /// True once the trust answer was sent. At most one answer per run.
    answered: bool,
    /// True once the trust question text appeared. Blocks the Running state until answered.
    question_seen: bool,
    /// True once a session URL appeared. Later trust prompts are ignored.
    ready: bool,
    /// True while `last_error` holds an error from this child.
    errored: bool,
}

impl Session<'_> {
    /// Moves Starting to Running, unless a trust answer is still pending.
    /// The server is connected again, so an older error no longer applies.
    fn mark_running(&mut self) {
        let mut live = self.shared.live();
        live.status.last_error = None;
        self.errored = false;
        let trust_pending = self.question_seen && !self.answered;
        if live.status.state == State::Starting && !trust_pending {
            live.status.state = State::Running;
        }
    }

    fn feed(&mut self, parser: &mut Parser, chunk: &[u8]) -> Option<Outcome> {
        parser
            .feed(chunk)
            .into_iter()
            .find_map(|event| self.on_event(event))
    }

    fn on_event(&mut self, event: Event) -> Option<Outcome> {
        match event {
            Event::FolderBusy => return Some(Outcome::Busy),
            Event::TrustQuestion => self.question_seen = true,
            Event::TrustPrompt => {
                self.question_seen = true;
                let answerable = !self.ready
                    && !self.answered
                    && self.answer_at.is_none()
                    && self.shared.live().status.state != State::Running;
                if !answerable {
                    // Logged as a plain line by the parser. Never answered.
                } else if self.auto_trust {
                    self.answer_at = Some(Instant::now() + TRUST_DELAY);
                } else {
                    return Some(Outcome::NeedsTrust);
                }
            }
            // Output alone does not mean ready. A banner can come before a busy notice.
            Event::Output => {}
            Event::Ready => self.mark_running(),
            Event::Line(line) => {
                let mut live = self.shared.live();
                if live.log.len() == LOG_LINES {
                    live.log.pop_front();
                }
                live.log.push_back(line);
            }
            Event::Error(message) => {
                self.errored = true;
                self.shared.live().status.last_error = Some(message);
            }
            Event::SessionUrl(url) => {
                self.ready = true;
                self.mark_running();
                let mut live = self.shared.live();
                if !live.status.session_urls.contains(&url) {
                    live.status.session_urls.push(url);
                }
            }
        }
        None
    }
}
