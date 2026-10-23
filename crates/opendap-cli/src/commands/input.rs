use opendap_cli_kit::CliError;
use opendap_cli_kit::output::note;
use opendap_core::{Need, Outcome, resolve_target};
use opendap_transport_vm::LocalVms;

use super::{Ctx, Doc, emit_plan};
use crate::cli::{InputCmd, InputEvent};

pub fn run(ctx: &Ctx, cmd: InputCmd) -> Result<u8, CliError> {
    let InputCmd::Send { event } = cmd;
    let (vm, target) = resolve_target(&LocalVms, ctx.target.as_ref(), Need::Qmp)?;
    let event = match event {
        InputEvent::Key { role, hold_ms } => opendap_model::InputEvent::Key {
            role: opendap_model::Role(role),
            hold_ms,
        },
        InputEvent::Tap { x, y } => opendap_model::InputEvent::Tap { x, y },
    };
    let text = match &event {
        opendap_model::InputEvent::Key { role, hold_ms } => {
            format!("pressed {} for {hold_ms} ms on {target}", role.as_str())
        }
        opendap_model::InputEvent::Tap { x, y } => format!("tapped {x},{y} on {target}"),
    };
    let sent = match opendap_core::input::send(&vm, target, event, ctx.dry_run)? {
        Outcome::Plan(plan) => return emit_plan(ctx, "opendap.input/v1", plan),
        Outcome::Done(sent) => sent,
    };
    ctx.out.emit(
        &Doc {
            schema: "opendap.input/v1",
            body: sent,
        },
        || {
            note(&text);
            String::new()
        },
    );
    Ok(0)
}
