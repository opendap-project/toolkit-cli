use std::time::{Duration, Instant};

use opendap_cli_kit::output::{note, table};
use opendap_cli_kit::{CliError, OutputMode};
use opendap_model::error as codes;
use serde::Serialize;

use super::{Ctx, Doc, emit_plan};
use crate::cli::VmCmd;
use crate::virt::{self, VirtualT1, VmInfo, VmState, require_tool, tail};

#[derive(Serialize)]
struct Items<T: Serialize> {
    items: Vec<T>,
}

/// A vm document after a mutation.
#[derive(Serialize)]
struct Changed {
    #[serde(flatten)]
    vm: VmInfo,
    changed: bool,
}

pub fn run(ctx: &Ctx, cmd: VmCmd) -> Result<u8, CliError> {
    match cmd {
        VmCmd::List => list(ctx),
        VmCmd::Get { name } => get(ctx, &name),
        VmCmd::Start {
            name,
            detach,
            wait_timeout,
        } => start(ctx, &name, detach, wait_timeout),
        VmCmd::Stop { name } => stop(ctx, &name),
    }
}

fn state_str(s: VmState) -> &'static str {
    match s {
        VmState::Stopped => "stopped",
        VmState::Booting => "booting",
        VmState::Running => "running",
    }
}

pub fn human_row(v: &VmInfo) -> Vec<String> {
    vec![
        v.id.clone(),
        state_str(v.state).into(),
        v.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
        v.endpoints.ssh.clone(),
        v.endpoints.vnc.clone(),
        if v.image.present {
            "present".into()
        } else {
            "missing".into()
        },
    ]
}

const HEADERS: &[&str] = &["ID", "STATE", "PID", "SSH", "VNC", "IMAGE"];

fn list(ctx: &Ctx) -> Result<u8, CliError> {
    let vm = VirtualT1::locate()?;
    let info = vm.info();
    let human = || {
        if ctx.out.mode == OutputMode::Plain {
            format!("{}\t{}\n", info.id, state_str(info.state))
        } else {
            table(HEADERS, &[human_row(&info)])
        }
    };
    ctx.out.emit(
        &Doc {
            schema: "opendap.vm_list/v1",
            body: Items {
                items: vec![info.clone()],
            },
        },
        human,
    );
    Ok(0)
}

fn detail(v: &VmInfo) -> String {
    format!(
        "{}\n  state:   {}\n  pid:     {}\n  ssh:     {}\n  vnc:     {}\n  qmp:     {}\n  image:   {} ({})\n  console: {}\n",
        v.id,
        state_str(v.state),
        v.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
        v.endpoints.ssh,
        v.endpoints.vnc,
        v.endpoints.qmp,
        v.image.path,
        if v.image.present {
            "present"
        } else {
            "missing"
        },
        v.console_log,
    )
}

fn get(ctx: &Ctx, name: &str) -> Result<u8, CliError> {
    virt::check_name(name)?;
    let info = VirtualT1::locate()?.info();
    ctx.out.emit(
        &Doc {
            schema: "opendap.vm/v1",
            body: &info,
        },
        || detail(&info),
    );
    Ok(0)
}

fn emit_changed(ctx: &Ctx, vm: &VirtualT1, changed: bool, verb: &str) {
    let info = vm.info();
    let human = format!("vm:default {verb} ({})\n", state_str(info.state));
    ctx.out.emit(
        &Doc {
            schema: "opendap.vm/v1",
            body: Changed { vm: info, changed },
        },
        || {
            // Mutations print nothing on stdout in human mode; status goes to stderr.
            note(human.trim_end());
            String::new()
        },
    );
}

fn start(ctx: &Ctx, name: &str, detach: bool, wait_timeout: u64) -> Result<u8, CliError> {
    virt::check_name(name)?;
    let vm = VirtualT1::locate()?;
    let state = vm.state();
    let wait_step = format!(
        "wait until SSH answers on 127.0.0.1:{} (up to {wait_timeout}s)",
        vm.ssh_port()
    );
    if ctx.dry_run {
        let mut plan = Vec::new();
        if state == VmState::Stopped {
            plan.push(format!("sh {} start", vm.qemu_script().display()));
        }
        if !detach && state != VmState::Running {
            plan.push(wait_step);
        }
        return emit_plan(ctx, "opendap.vm/v1", plan);
    }
    let changed = state == VmState::Stopped;
    if changed {
        if !vm.image().is_file() {
            let building = VirtualT1::image_build_running();
            let msg = if building {
                "the virtual T1 image is being rebuilt (dap-image.sh is running)".to_string()
            } else {
                format!("no virtual T1 image at {}", vm.image().display())
            };
            let mut err = CliError::new(&codes::VM_IMAGE_MISSING, msg);
            if building {
                err = err.hint("Wait for the build to finish, then retry.");
            } else {
                err = err.fix(format!(
                    "sh {} qemu",
                    vm.os_dir.join("build/dap-image.sh").display()
                ));
            }
            return Err(err);
        }
        require_tool("qemu-system-aarch64", "runs the virtual T1")?;
        require_tool("qemu-img", "creates the virtual T1 disk")?;
        let out = vm.run_qemu_script(&["start"])?;
        if !out.status.success() || vm.pid().is_none() {
            return Err(
                CliError::new(&codes::VM_START_FAILED, "dap-qemu.sh start failed").details(
                    serde_json::json!({
                        "stderr": String::from_utf8_lossy(&out.stderr),
                        "qemu_log": tail(&vm.qemu_log(), 20),
                    }),
                ),
            );
        }
        note(format!(
            "vm:default started (pid {})",
            vm.pid().unwrap_or_default()
        ));
    }
    if !detach && vm.state() != VmState::Running {
        note(format!(
            "waiting for vm:default to boot (up to {wait_timeout}s)…"
        ));
        let deadline = Instant::now() + Duration::from_secs(wait_timeout);
        loop {
            if vm.pid().is_none() {
                return Err(
                    CliError::new(&codes::VM_START_FAILED, "QEMU exited during boot")
                        .details(serde_json::json!({ "qemu_log": tail(&vm.qemu_log(), 20) })),
                );
            }
            if vm.ssh_ready() {
                break;
            }
            if Instant::now() >= deadline {
                return Err(CliError::new(
                    &codes::VM_BOOT_TIMEOUT,
                    format!("SSH did not answer within {wait_timeout}s"),
                )
                .details(serde_json::json!({ "console_tail": tail(&vm.console_log(), 20) })));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    emit_changed(
        ctx,
        &vm,
        changed,
        if changed {
            "started"
        } else {
            "already running"
        },
    );
    Ok(0)
}

fn stop(ctx: &Ctx, name: &str) -> Result<u8, CliError> {
    virt::check_name(name)?;
    let vm = VirtualT1::locate()?;
    let running = vm.pid().is_some();
    if ctx.dry_run {
        let plan = if running {
            vec![format!("sh {} stop", vm.qemu_script().display())]
        } else {
            vec![]
        };
        return emit_plan(ctx, "opendap.vm/v1", plan);
    }
    if running {
        vm.run_qemu_script(&["stop"])?;
        let deadline = Instant::now() + Duration::from_secs(30);
        while vm.pid().is_some() {
            if Instant::now() >= deadline {
                return Err(CliError::new(
                    &codes::VM_STOP_TIMEOUT,
                    "QEMU still runs 30s after the stop request",
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    emit_changed(
        ctx,
        &vm,
        running,
        if running {
            "stopped"
        } else {
            "already stopped"
        },
    );
    Ok(0)
}
