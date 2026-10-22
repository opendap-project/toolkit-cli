//! Command implementations. Each returns the process exit code or a `CliError`.

mod device;
mod doctor;
mod exec;
mod input;
mod schema;
mod screen;
mod version;
mod vm;

use opendap_cli_kit::{CliError, Output};
use opendap_model::error as codes;
use opendap_model::{Target, Transport};
use serde::Serialize;

use crate::cli::Command;
use crate::virt::{self, VirtualT1, VmState};

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
    }
}

/// What a targeted command needs from the virtual T1.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// SSH must answer (the guest has booted).
    Ssh,
    /// QEMU must run (QMP answers from the first instant).
    Qmp,
}

/// Resolve `--target` (or the single running candidate) to the virtual T1.
pub fn resolve_target(ctx: &Ctx, need: Need) -> Result<(VirtualT1, Transport), CliError> {
    let explicit = match &ctx.target {
        None => None,
        Some(Target::Transport(Transport::Vm { name })) => {
            virt::check_name(name)?;
            Some(name.clone())
        }
        Some(Target::Transport(t)) => {
            return Err(CliError::new(
                &codes::TARGET_UNSUPPORTED,
                format!("`{t}`: {} targets are not supported yet", t.kind()),
            )
            .fix("opendap device list"));
        }
        Some(Target::Context(name)) => {
            return Err(CliError::new(
                &codes::TARGET_UNSUPPORTED,
                format!("`{name}`: contexts are not supported yet"),
            )
            .hint("Pass a selector such as vm:default."));
        }
    };
    let vm = VirtualT1::locate()?;
    let transport = Transport::Vm {
        name: virt::DEFAULT_NAME.into(),
    };
    let state = vm.state();
    let usable = match need {
        Need::Ssh => state == VmState::Running,
        Need::Qmp => state != VmState::Stopped,
    };
    if usable {
        return Ok((vm, transport));
    }
    if explicit.is_none() && state == VmState::Stopped {
        return Err(CliError::new(&codes::DEVICE_NOT_FOUND, "no T1 is running")
            .fix("opendap vm start")
            .fix("opendap device list"));
    }
    let what = if state == VmState::Booting {
        "still booting"
    } else {
        "stopped"
    };
    Err(
        CliError::new(&codes::VM_NOT_RUNNING, format!("vm:default is {what}"))
            .fix("opendap vm start")
            .details(serde_json::json!({ "target": "vm:default", "state": state })),
    )
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
