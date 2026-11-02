//! Library target of the `rc-keeper` package. The binary and `xtask` both use it.
pub mod cli;

mod autostart_daemon;
mod commands;
mod output;
mod service;
