//! `opendap`: the command-line tool for the T1 and OpenDAP OS.
//!
//! Interface layer: parses arguments, wires the adapters into `opendap-core`
//! and renders the results through `opendap-cli-kit`.

mod cli;
mod commands;
mod meta;

use std::io::IsTerminal;
use std::process::ExitCode;

use clap::Parser;
use clap::error::ErrorKind;
use opendap_cli_kit::env::{Tty, agent_detected, interactive_allowed};
use opendap_cli_kit::{CliError, Output, OutputMode};
use opendap_model::error as codes;

use crate::cli::Cli;
use crate::commands::Ctx;

fn main() -> ExitCode {
    let agent = agent_detected();
    let tty = Tty::detect();
    let env_output = std::env::var("OPENDAP_OUTPUT").ok();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => return usage_error(e, agent, tty, env_output.as_deref()),
    };

    let flag = cli.global.output.as_deref().and_then(OutputMode::parse);
    let mode = OutputMode::resolve(
        flag,
        cli.global.json,
        env_output.as_deref(),
        agent,
        tty.stdout,
    );
    let out = Output::new(mode, agent, tty.stdout);

    if cli.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let Some(command) = cli.command else {
        let err = CliError::new(&codes::INPUT_MISSING, "no command given")
            .fix("opendap --help")
            .fix("opendap schema");
        out.error(&err);
        return ExitCode::from(err.exit_code());
    };

    let target = match cli.global.target.as_deref().map(str::parse) {
        None => None,
        Some(Ok(t)) => Some(t),
        Some(Err(e)) => {
            let err = CliError::new(&codes::TARGET_INVALID, format!("--target: {e}"));
            out.error(&err);
            return ExitCode::from(err.exit_code());
        }
    };

    let ci = std::env::var_os("CI").is_some_and(|v| !v.is_empty());
    let ctx = Ctx {
        out,
        target,
        dry_run: cli.global.dry_run,
        interactive: interactive_allowed(cli.global.no_input, agent, ci, tty),
    };

    match commands::run(&ctx, command) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            ctx.out.error(&err);
            ExitCode::from(err.exit_code())
        }
    }
}

/// clap errors: help and version go out as clap renders them; usage errors
/// also produce the `input.invalid` envelope in JSON modes. Exit code 2.
fn usage_error(e: clap::Error, agent: bool, tty: Tty, env_output: Option<&str>) -> ExitCode {
    if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        let _ = e.print();
        return ExitCode::SUCCESS;
    }
    if e.kind() == ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand {
        let _ = e.print();
        return ExitCode::from(codes::exit::USAGE);
    }
    // Best effort: the arguments did not parse, so look for the output flags by hand.
    let args: Vec<String> = std::env::args().collect();
    let json_flag = args.iter().any(|a| a == "--json");
    let flag = args
        .windows(2)
        .find(|w| w[0] == "-o" || w[0] == "--output")
        .and_then(|w| OutputMode::parse(&w[1]))
        .or_else(|| {
            args.iter()
                .find_map(|a| a.strip_prefix("--output=").and_then(OutputMode::parse))
        });
    let mode = OutputMode::resolve(
        flag,
        json_flag,
        env_output,
        agent,
        std::io::stdout().is_terminal(),
    );
    let out = Output::new(mode, agent, tty.stdout);
    if mode.is_json() {
        let rendered = e.to_string();
        let msg = rendered
            .lines()
            .next()
            .unwrap_or("invalid usage")
            .trim_start_matches("error: ")
            .to_string();
        let err = CliError::new(&codes::INPUT_INVALID, msg)
            .details(serde_json::json!({ "usage": rendered.trim_end() }));
        println!("{}", out.json_string(&err.document()));
    }
    let _ = e.print();
    ExitCode::from(codes::exit::USAGE)
}
