//! Developer tasks. Run `cargo run -p xtask -- docs` to regenerate `docs/reference`.

mod cli_docs;
mod docs;
mod ipc_docs;
mod ts_types;

use std::path::PathBuf;

use anyhow::{bail, Result};

fn main() -> Result<()> {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("docs") => {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
            docs::generate(&root.canonicalize()?)
        }
        _ => bail!("usage: cargo run -p xtask -- docs"),
    }
}
