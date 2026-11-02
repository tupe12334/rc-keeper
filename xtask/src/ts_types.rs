//! TypeScript types for the UI, generated from the IPC JSON Schema by `json-schema-to-typescript`.
//! That tool is a devDependency of `ui`, so this step runs it through pnpm.

use std::path::Path;
use std::process::Command;

use anyhow::{ensure, Context, Result};

use crate::docs::HEADER;

/// Output file, relative to the repo root.
pub const OUT: &str = "ui/src/api/generated.ts";

pub fn write(root: &Path, schema: &Path) -> Result<()> {
    let status = Command::new("pnpm")
        .arg("-C")
        .arg(root.join("ui"))
        .args(["exec", "json2ts"])
        .arg("--input")
        .arg(schema)
        .arg("--output")
        .arg(root.join(OUT))
        .args(["--additionalProperties", "false"])
        .arg("--bannerComment")
        .arg(format!("/* {HEADER} */"))
        .status()
        .context("cannot run pnpm. Install pnpm, then run `pnpm -C ui install`")?;
    ensure!(
        status.success(),
        "json2ts failed. Run `pnpm -C ui install` first, then retry."
    );
    Ok(())
}
