//! Clap definitions and top-level dispatch.

use std::path::PathBuf;

use anyhow::Result;
use clap::builder::PossibleValuesParser;
use clap::{ArgGroup, Args, Parser, Subcommand};
use rc_core::InstanceConfig;

use crate::autostart_daemon::DaemonUnreachable;
use crate::commands::{self, control::Action};

/// Early exit with a code. The command already printed its output.
#[derive(Debug)]
pub struct Exit(pub i32);

impl std::fmt::Display for Exit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exit {}", self.0)
    }
}

impl std::error::Error for Exit {}

#[derive(Parser)]
#[command(
    name = "rc-keeper",
    version,
    about = "Keep `claude remote-control` running 24/7"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Daemon control. Bare `daemon` runs it in the foreground.
    Daemon {
        #[command(subcommand)]
        action: Option<DaemonCmd>,
    },
    /// Show each instance with its runtime state
    Status {
        /// Print a JSON array of instances instead of a table
        #[arg(long)]
        json: bool,
    },
    /// List configured instances
    List {
        /// Print a JSON array of instances instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Add a folder as a new instance
    Add {
        /// Folder that `claude remote-control` runs in. Must exist. Stored as an absolute path.
        folder: PathBuf,
        #[command(flatten)]
        flags: InstanceFlags,
    },
    /// Change an instance. Only given flags change.
    Edit {
        /// Instance id or folder path
        selector: String,
        #[command(flatten)]
        flags: InstanceFlags,
    },
    /// Remove an instance (stops it first)
    Remove {
        /// Instance id or folder path
        selector: String,
    },
    /// Start an instance, or all with --all
    Start {
        #[command(flatten)]
        target: Target,
    },
    /// Stop an instance, or all with --all
    Stop {
        #[command(flatten)]
        target: Target,
    },
    /// Restart an instance, or all with --all
    Restart {
        #[command(flatten)]
        target: Target,
    },
    /// Print the last log lines of an instance
    Logs {
        /// Instance id or folder path
        selector: String,
        /// Keep running and print new lines as they arrive. Stop with Ctrl+C.
        #[arg(short, long)]
        follow: bool,
    },
    /// Get or set settings
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Install, remove or inspect the login service
    Service {
        #[command(subcommand)]
        action: ServiceCmd,
    },
    /// Print the CLI version
    Version,
}

#[derive(Subcommand)]
pub enum DaemonCmd {
    /// Run the daemon in the foreground
    Run,
    /// Stop the running daemon (exit 0 also when none runs)
    Stop,
    /// Show whether the daemon runs (exit 0 running, 2 not running)
    Status {
        /// Print the daemon state as JSON instead of text
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Print all settings
    Get,
    /// Set a setting. The only key is `claude-bin`.
    Set {
        /// Setting name. Must be `claude-bin`.
        key: String,
        /// Path to the `claude` executable, e.g. `/usr/local/bin/claude`. Stored only on this machine in `settings.local.json`.
        value: String,
    },
    /// Print the config dir (RC_KEEPER_CONFIG_DIR overrides it)
    Path,
}

#[derive(Subcommand)]
pub enum ServiceCmd {
    /// Write the service file and enable it
    Install {
        /// Write the service file only. Skip systemctl, launchctl and schtasks.
        #[arg(long)]
        dry_run: bool,
    },
    /// Disable the service and remove its file
    Uninstall {
        /// Remove the service file only. Skip systemctl, launchctl and schtasks.
        #[arg(long)]
        dry_run: bool,
    },
    /// Show whether the service is installed and the daemon runs
    Status,
}

#[derive(Args)]
#[command(group(ArgGroup::new("target").required(true).multiple(false).args(["selector", "all"])))]
pub struct Target {
    /// Instance id or folder path. Give either this or `--all`, not both.
    /// An id wins when a folder path and an id match.
    pub selector: Option<String>,
    /// Apply to every instance. Give either this or a selector, not both.
    #[arg(long)]
    pub all: bool,
}

const PERMISSION_MODES: [&str; 6] = [
    "acceptEdits",
    "auto",
    "bypassPermissions",
    "default",
    "dontAsk",
    "plan",
];
const SPAWN_MODES: [&str; 3] = ["same-dir", "worktree", "session"];

/// Instance options. Unset options keep their value on `edit`.
#[derive(Args, Default)]
pub struct InstanceFlags {
    /// Session name shown in Claude. Passed as `claude remote-control --name`. Not set by default.
    #[arg(long)]
    pub name: Option<String>,
    /// Start with Chrome integration. Passed as `--chrome`. On by default for `add`.
    #[arg(long, conflicts_with = "no_chrome")]
    pub chrome: bool,
    /// Start without Chrome integration. Passed as `--no-chrome`. Turns off the `add` default.
    #[arg(long)]
    pub no_chrome: bool,
    /// Permission mode. Passed as `--permission-mode`. One of: acceptEdits, auto, bypassPermissions, default, dontAsk, plan. Not set by default, so `claude` decides.
    #[arg(long, value_parser = PossibleValuesParser::new(PERMISSION_MODES))]
    pub permission_mode: Option<String>,
    /// Session spawn mode. Passed as `--spawn`. One of: same-dir, worktree, session. Not set by default.
    #[arg(long, value_parser = PossibleValuesParser::new(SPAWN_MODES))]
    pub spawn: Option<String>,
    /// Maximum number of sessions. Passed as `--capacity`. Whole number. Not set by default.
    #[arg(long)]
    pub capacity: Option<u32>,
    /// Create sessions in the instance folder. Passed as `--create-session-in-dir`. Not set by default.
    #[arg(long, conflicts_with = "no_create_session_in_dir")]
    pub create_session_in_dir: bool,
    /// Do not create sessions in the instance folder. Passed as `--no-create-session-in-dir`.
    #[arg(long)]
    pub no_create_session_in_dir: bool,
    /// Prefix for remote session names. Passed as `--remote-control-session-name-prefix`. Not set by default.
    #[arg(long)]
    pub session_name_prefix: Option<String>,
    /// Answer the folder trust prompt for this instance. On by default for `add`. Not passed to `claude`.
    #[arg(long, conflicts_with = "no_auto_trust")]
    pub auto_trust: bool,
    /// Do not answer the folder trust prompt. You must trust the folder yourself. Not passed to `claude`.
    #[arg(long)]
    pub no_auto_trust: bool,
    /// Start this instance when the daemon starts. On by default for `add`. Not passed to `claude`.
    #[arg(long, conflicts_with = "no_auto_start")]
    pub auto_start: bool,
    /// Do not start this instance with the daemon. Start it with `rc-keeper start`. Not passed to `claude`.
    #[arg(long)]
    pub no_auto_start: bool,
    /// Extra arguments for `claude remote-control`. Add them after `--`, e.g. `add DIR -- --verbose`.
    #[arg(last = true)]
    pub extra_args: Vec<String>,
}

fn tri(on: bool, off: bool) -> Option<bool> {
    if on {
        Some(true)
    } else if off {
        Some(false)
    } else {
        None
    }
}

impl InstanceFlags {
    pub fn apply(&self, cfg: &mut InstanceConfig) {
        if let Some(v) = &self.name {
            cfg.name = Some(v.clone());
        }
        if let Some(v) = tri(self.chrome, self.no_chrome) {
            cfg.chrome = v;
        }
        if let Some(v) = &self.permission_mode {
            cfg.permission_mode = Some(v.clone());
        }
        if let Some(v) = &self.spawn {
            cfg.spawn = Some(v.clone());
        }
        if let Some(v) = self.capacity {
            cfg.capacity = Some(v);
        }
        if let Some(v) = tri(self.create_session_in_dir, self.no_create_session_in_dir) {
            cfg.create_session_in_dir = Some(v);
        }
        if let Some(v) = &self.session_name_prefix {
            cfg.session_name_prefix = Some(v.clone());
        }
        if let Some(v) = tri(self.auto_trust, self.no_auto_trust) {
            cfg.auto_trust = v;
        }
        if let Some(v) = tri(self.auto_start, self.no_auto_start) {
            cfg.auto_start = v;
        }
        if !self.extra_args.is_empty() {
            cfg.extra_args = self.extra_args.clone();
        }
    }
}

/// Runs the CLI. Returns the process exit code.
pub fn run() -> i32 {
    // Usage errors exit 1. Exit 2 means "daemon unreachable", so clap's default 2 is not used.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let code = if e.use_stderr() { 1 } else { 0 };
            let _ = e.print();
            return code;
        }
    };
    match dispatch(cli.command) {
        Ok(()) => 0,
        Err(e) => match e.downcast_ref::<Exit>() {
            Some(Exit(code)) => *code,
            None => {
                eprintln!("error: {e:#}");
                if e.is::<DaemonUnreachable>() {
                    2
                } else {
                    1
                }
            }
        },
    }
}

fn dispatch(command: Command) -> Result<()> {
    match command {
        Command::Daemon { action } => match action {
            None | Some(DaemonCmd::Run) => commands::daemon::run(),
            Some(DaemonCmd::Stop) => commands::daemon::stop(),
            Some(DaemonCmd::Status { json }) => commands::daemon::status(json),
        },
        Command::Status { json } => commands::instances::status(json),
        Command::List { json } => commands::instances::list(json),
        Command::Add { folder, flags } => commands::instances::add(&folder, &flags),
        Command::Edit { selector, flags } => commands::instances::edit(&selector, &flags),
        Command::Remove { selector } => commands::instances::remove(&selector),
        Command::Start { target } => commands::control::run(Action::Start, &target),
        Command::Stop { target } => commands::control::run(Action::Stop, &target),
        Command::Restart { target } => commands::control::run(Action::Restart, &target),
        Command::Logs { selector, follow } => commands::logs::run(&selector, follow),
        Command::Config { action } => match action {
            ConfigCmd::Get => commands::config::get(),
            ConfigCmd::Set { key, value } => commands::config::set(&key, &value),
            ConfigCmd::Path => commands::config::path(),
        },
        Command::Service { action } => match action {
            ServiceCmd::Install { dry_run } => crate::service::install(dry_run),
            ServiceCmd::Uninstall { dry_run } => crate::service::uninstall(dry_run),
            ServiceCmd::Status => crate::service::status(),
        },
        Command::Version => {
            println!("rc-keeper {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn target_selector_and_all_are_exclusive() {
        assert!(Cli::try_parse_from(["rc-keeper", "start", "foo", "--all"]).is_err());
        assert!(Cli::try_parse_from(["rc-keeper", "start", "--all"]).is_ok());
        assert!(Cli::try_parse_from(["rc-keeper", "start"]).is_err());
    }

    #[test]
    fn help_is_not_a_usage_error() {
        let err = Cli::try_parse_from(["rc-keeper", "--help"]).err().unwrap();
        assert!(!err.use_stderr());
    }
}
