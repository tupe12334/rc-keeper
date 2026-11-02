//! Protocol, client and server for the rc-keeper daemon.
//!
//! Wire format: one JSON object per line.

mod client;
mod paths;
mod protocol;
mod server;

pub use client::Client;
pub use paths::{config_dir, socket_path};
pub use protocol::{Request, Response};
pub use server::{serve, shutdown_and_exit};
