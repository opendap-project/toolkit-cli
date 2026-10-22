use opendap_cli_kit::CliError;
use opendap_cli_kit::output::note;
use opendap_model::Transport;
use serde::Serialize;

use super::{Ctx, Doc, Need, emit_plan, resolve_target};
use crate::cli::{Button, InputCmd, InputEvent};

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Event {
    Key { button: Button, hold_ms: u64 },
    Tap { x: u32, y: u32 },
}

#[derive(Serialize)]
struct Sent {
    target: Transport,
    event: Event,
    changed: bool,
}

pub fn run(ctx: &Ctx, cmd: InputCmd) -> Result<u8, CliError> {
    let InputCmd::Send { event } = cmd;
    let (vm, target) = resolve_target(ctx, Need::Qmp)?;
    let (args, event, text): (Vec<String>, Event, String) = match event {
        InputEvent::Key { button, hold_ms } => (
            vec!["press".into(), button.as_str().into(), hold_ms.to_string()],
            Event::Key { button, hold_ms },
            format!("pressed {} for {hold_ms} ms on {target}", button.as_str()),
        ),
        InputEvent::Tap { x, y } => (
            vec!["tap".into(), x.to_string(), y.to_string()],
            Event::Tap { x, y },
            format!("tapped {x},{y} on {target}"),
        ),
    };
    if ctx.dry_run {
        return emit_plan(
            ctx,
            "opendap.input/v1",
            vec![format!("dap-qmp.py {}", args.join(" "))],
        );
    }
    vm.qmp(&args)?;
    ctx.out.emit(
        &Doc {
            schema: "opendap.input/v1",
            body: Sent {
                target,
                event,
                changed: true,
            },
        },
        || {
            note(&text);
            String::new()
        },
    );
    Ok(0)
}
