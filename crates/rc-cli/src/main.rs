mod autostart_daemon;
mod cli;
mod commands;
mod output;
mod service;

fn main() {
    std::process::exit(cli::run());
}
