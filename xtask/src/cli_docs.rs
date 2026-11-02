//! CLI reference (markdown) and man pages, both from the clap definition in `rc-cli`.

use anyhow::Result;
use clap::{Command, CommandFactory};

use crate::docs::HEADER;

fn command() -> Command {
    rc_cli::cli::Cli::command()
}

pub fn markdown() -> String {
    format!(
        "<!-- {HEADER} -->\n\n{}",
        clap_markdown::help_markdown_command(&command())
    )
}

/// `(file stem, roff text)` for the root command and every subcommand, nested ones included.
/// Each page is named after its command path, e.g. `rc-keeper-daemon-run`.
pub fn man_pages() -> Result<Vec<(String, String)>> {
    let mut pages = Vec::new();
    let mut root = command();
    root.build();
    collect(root, "rc-keeper".to_string(), &mut pages)?;
    Ok(pages)
}

fn collect(cmd: Command, path: String, out: &mut Vec<(String, String)>) -> Result<()> {
    // clap wants a `'static` name. The xtask process ends right after, so the leak is harmless.
    let leaked: &'static str = Box::leak(path.clone().into_boxed_str());
    let cmd = cmd.name(leaked).bin_name(leaked);
    let mut buf = Vec::new();
    buf.extend_from_slice(format!(".\\\" {HEADER}\n").as_bytes());
    clap_mangen::Man::new(cmd.clone()).render(&mut buf)?;
    out.push((path.clone(), String::from_utf8(buf)?));
    for sub in cmd.get_subcommands().filter(|s| s.get_name() != "help") {
        collect(sub.clone(), format!("{path}-{}", sub.get_name()), out)?;
    }
    Ok(())
}
