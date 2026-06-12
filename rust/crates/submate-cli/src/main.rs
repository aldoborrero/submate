//! `submate` binary — clap CLI (ports submate/cli/).
//!
//! Stub — implemented by the grind backlog item `port-cli-commands`. Pure
//! sub-helpers land ahead of the clap wiring in their own modules.

// Wired into the `translate` subcommand under `port-cli-commands`; until that
// lands the helpers are exercised only by the module's own tests.
#[allow(dead_code)]
mod translate_paths;

// Wired into the `config show` subcommand under `port-cli-commands`; until that
// lands `config_show_rows` is exercised only by the module's own parity test.
#[allow(dead_code)]
mod config_show;

fn main() {
    eprintln!("submate: not yet implemented (Rust port in progress)");
    std::process::exit(1);
}
