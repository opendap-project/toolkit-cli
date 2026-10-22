use opendap_cli_kit::output::{note, table};
use opendap_cli_kit::{CliError, OutputMode};
use opendap_model::DeviceState;
use opendap_transport_vm::LocalVms;

use super::{Ctx, Doc};
use crate::cli::DeviceCmd;

pub fn run(ctx: &Ctx, cmd: DeviceCmd) -> Result<u8, CliError> {
    match cmd {
        DeviceCmd::List => list(ctx),
    }
}

fn state_str(s: DeviceState) -> &'static str {
    match s {
        DeviceState::Online => "online",
        DeviceState::Booting => "booting",
        DeviceState::Offline => "offline",
    }
}

fn list(ctx: &Ctx) -> Result<u8, CliError> {
    let list = opendap_core::device::list(&LocalVms);
    if !ctx.out.mode.is_json() {
        for w in &list.warnings {
            note(format!("warning[{}]: {}", w.code, w.message));
        }
    }
    let rows: Vec<Vec<String>> = list
        .items
        .iter()
        .map(|d| {
            vec![
                d.id.clone(),
                d.model.clone() + if d.is_virtual { " (virtual)" } else { "" },
                d.transport.kind().into(),
                state_str(d.state).into(),
                d.mode
                    .map(|m| {
                        serde_json::to_value(m)
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .to_string()
                    })
                    .unwrap_or_else(|| "-".into()),
            ]
        })
        .collect();
    let plain = ctx.out.mode == OutputMode::Plain;
    ctx.out.emit(
        &Doc {
            schema: "opendap.device_list/v1",
            body: &list,
        },
        || {
            if plain {
                rows.iter()
                    .map(|r| format!("{}\t{}\n", r[0], r[3]))
                    .collect()
            } else {
                table(&["ID", "MODEL", "TRANSPORT", "STATE", "MODE"], &rows)
            }
        },
    );
    Ok(0)
}
