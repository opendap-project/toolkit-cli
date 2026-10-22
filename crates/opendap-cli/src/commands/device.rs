use opendap_cli_kit::output::{note, table};
use opendap_cli_kit::{CliError, OutputMode};
use opendap_model::{Device, DeviceState, Mode, Transport};
use serde::Serialize;

use super::{Ctx, Doc};
use crate::cli::DeviceCmd;
use crate::virt::{self, VirtualT1, VmState};

#[derive(Serialize)]
struct Warning {
    code: &'static str,
    message: String,
}

#[derive(Serialize)]
struct DeviceList {
    items: Vec<Device>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<Warning>,
}

pub fn run(ctx: &Ctx, cmd: DeviceCmd) -> Result<u8, CliError> {
    match cmd {
        DeviceCmd::List => list(ctx),
    }
}

/// Discover devices on every transport this build supports (today: the virtual T1).
fn discover() -> DeviceList {
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    match VirtualT1::locate() {
        Ok(vm) => {
            let state = match vm.state() {
                VmState::Running => DeviceState::Online,
                VmState::Booting => DeviceState::Booting,
                VmState::Stopped => DeviceState::Offline,
            };
            let transport = Transport::Vm {
                name: virt::DEFAULT_NAME.into(),
            };
            items.push(Device {
                id: transport.selector(),
                model: "T1".into(),
                is_virtual: true,
                transport,
                state,
                // The virtual T1 only boots into normal mode.
                mode: (state == DeviceState::Online).then_some(Mode::Normal),
                serial: None,
            });
        }
        Err(e) => warnings.push(Warning {
            code: e.code.code,
            message: e.message,
        }),
    }
    DeviceList { items, warnings }
}

fn state_str(s: DeviceState) -> &'static str {
    match s {
        DeviceState::Online => "online",
        DeviceState::Booting => "booting",
        DeviceState::Offline => "offline",
    }
}

fn list(ctx: &Ctx) -> Result<u8, CliError> {
    let list = discover();
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
