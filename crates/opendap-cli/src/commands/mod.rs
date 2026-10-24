//! Command implementations. Each returns the process exit code or a `CliError`.

mod device;
mod doctor;
mod exec;
mod hardware;
mod input;
mod schema;
mod screen;
mod version;
mod vm;

use opendap_cli_kit::{CliError, Output};
use opendap_model::Target;
use serde::Serialize;

use crate::cli::Command;

pub struct Ctx {
    pub out: Output,
    pub target: Option<Target>,
    pub dry_run: bool,
    /// Prompts and interactive sessions are allowed.
    pub interactive: bool,
}

/// A top-level document: `{"schema": …, …body}`.
#[derive(Serialize)]
pub struct Doc<T: Serialize> {
    pub schema: &'static str,
    #[serde(flatten)]
    pub body: T,
}

pub fn run(ctx: &Ctx, command: Command) -> Result<u8, CliError> {
    match command {
        Command::Version => version::run(ctx),
        Command::Doctor => doctor::run(ctx),
        Command::Schema { command } => schema::run(ctx, &command),
        Command::Vm { cmd } => vm::run(ctx, cmd),
        Command::Device { cmd } => device::run(ctx, cmd),
        Command::Exec { argv } => exec::run(ctx, argv),
        Command::Screen { cmd } => screen::run(ctx, cmd),
        Command::Input { cmd } => input::run(ctx, cmd),
        Command::Hardware { cmd } => hardware::run(ctx, cmd),
    }
}

/// The plan document printed under --dry-run.
#[derive(Serialize)]
pub struct Plan {
    pub dry_run: bool,
    pub changed: bool,
    pub plan: Vec<String>,
}

pub fn emit_plan(ctx: &Ctx, schema: &'static str, plan: Vec<String>) -> Result<u8, CliError> {
    let human = plan
        .iter()
        .map(|s| format!("would run: {s}\n"))
        .collect::<String>();
    ctx.out.emit(
        &Doc {
            schema,
            body: Plan {
                dry_run: true,
                changed: false,
                plan,
            },
        },
        || human,
    );
    Ok(0)
}
