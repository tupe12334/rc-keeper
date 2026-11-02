//! Protocol, client and server for the rc-keeper daemon.
//!
//! Wire format: one JSON object per line.

mod client;
pub mod method;
mod paths;
mod protocol;
pub mod schema;
mod server;

pub use client::{Client, IO_TIMEOUT};
pub use method::{Method, MethodSpec};
pub use paths::{config_dir, socket_path};
pub use protocol::{Request, Response};
pub use server::{bind, serve, serve_until, shutdown_and_exit};
