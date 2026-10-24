use opendap_cli_kit::CliError;
use opendap_cli_kit::output::note;
use opendap_core::{Need, Outcome, resolve_target};
use opendap_model::{HardwareChange, HardwareState};
use opendap_transport_vm::LocalVms;

use super::{Ctx, Doc, emit_plan};
use crate::cli::HardwareCmd;

pub fn run(ctx: &Ctx, cmd: HardwareCmd) -> Result<u8, CliError> {
    match cmd {
        HardwareCmd::Get => get(ctx),
        HardwareCmd::Update {
            attach,
            detach,
            battery,
        } => {
            // clap requires exactly one of the three.
            let change = match (attach, detach, battery) {
                (Some(a), _, _) => HardwareChange::Attach {
                    attachment: a.into(),
                },
                (_, Some(a), _) => HardwareChange::Detach {
                    attachment: a.into(),
                },
                (_, _, Some(percent)) => HardwareChange::Battery { percent },
                _ => unreachable!("clap requires one change"),
            };
            update(ctx, change)
        }
    }
}

fn line(st: &HardwareState) -> String {
    let attached: Vec<&str> = st.attached.iter().map(|a| a.as_str()).collect();
    let battery = st
        .battery
        .map(|p| format!("{p} %"))
        .unwrap_or_else(|| "unknown".into());
    format!(
        "{}: attached: {}; battery: {battery}",
        st.target,
        if attached.is_empty() {
            "nothing".to_string()
        } else {
            attached.join(", ")
        }
    )
}

fn get(ctx: &Ctx) -> Result<u8, CliError> {
    let (vm, target) = resolve_target(&LocalVms, ctx.target.as_ref(), Need::Qmp)?;
    let st = opendap_core::hardware::state(&vm, &target)?;
    let text = line(&st);
    ctx.out.emit(
        &Doc {
            schema: "opendap.hardware/v1",
            body: st,
        },
        || format!("{text}\n"),
    );
    Ok(0)
}

fn update(ctx: &Ctx, change: HardwareChange) -> Result<u8, CliError> {
    let (vm, target) = resolve_target(&LocalVms, ctx.target.as_ref(), Need::Qmp)?;
    let done = match opendap_core::hardware::change(&vm, target, change, ctx.dry_run)? {
        Outcome::Plan(plan) => return emit_plan(ctx, "opendap.hardware/v1", plan),
        Outcome::Done(done) => done,
    };
    let what = match change {
        HardwareChange::Attach { attachment } if done.changed => format!("attached {attachment}"),
        HardwareChange::Attach { attachment } => format!("{attachment} was already attached"),
        HardwareChange::Detach { attachment } if done.changed => format!("detached {attachment}"),
        HardwareChange::Detach { attachment } => format!("{attachment} was not attached"),
        HardwareChange::Battery { percent } => format!("battery at {percent} %"),
    };
    let text = line(&done.state);
    ctx.out.emit(
        &Doc {
            schema: "opendap.hardware/v1",
            body: done,
        },
        || {
            note(&what);
            note(&text);
            String::new()
        },
    );
    Ok(0)
}
