//! Keeps `claude remote-control` running for several folders. See `CONTRACT.md`.
mod args;
mod backoff;
mod config;
mod locate;
mod manager;
mod model;
mod parser;
mod procs;
mod supervisor;

pub use args::build_args;
pub use locate::find_claude_bin;
pub use manager::Manager;
pub use model::{AppConfig, InstanceConfig, InstanceStatus, InstanceView, Settings, State};
