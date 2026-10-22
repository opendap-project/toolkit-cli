use std::time::Duration;

use opendap_cli_kit::output::{note, table};
use opendap_cli_kit::{CliError, OutputMode};
use opendap_core::Outcome;
use opendap_core::vm::{self, Changed as VmChanged, StartOptions};
use opendap_host::SystemClock;
use opendap_model::VmInfo;
use opendap_transport_vm::LocalVms;
use serde::Serialize;

use super::{Ctx, Doc, emit_plan};
use crate::cli::VmCmd;

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

pub fn human_row(v: &VmInfo) -> Vec<String> {
    vec![
        v.id.clone(),
        v.state.as_str().into(),
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
    let items = vm::list(&LocalVms)?;
    let info = &items[0];
    let human = || {
        if ctx.out.mode == OutputMode::Plain {
            format!("{}\t{}\n", info.id, info.state.as_str())
        } else {
            table(HEADERS, &[human_row(info)])
        }
    };
    ctx.out.emit(
        &Doc {
            schema: "opendap.vm_list/v1",
            body: Items {
                items: items.clone(),
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
        v.state.as_str(),
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
    let info = vm::get(&LocalVms, name)?;
    ctx.out.emit(
        &Doc {
            schema: "opendap.vm/v1",
            body: &info,
        },
        || detail(&info),
    );
    Ok(0)
}

fn emit_changed(ctx: &Ctx, done: VmChanged, verb: &str) {
    let VmChanged { info, changed } = done;
    let human = format!("vm:default {verb} ({})\n", info.state.as_str());
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
    let opts = StartOptions {
        detach,
        wait_timeout: Duration::from_secs(wait_timeout),
        dry_run: ctx.dry_run,
    };
    let progress = |s: &str| note(s);
    let done = match vm::start(&LocalVms, name, opts, &SystemClock::default(), &progress)? {
        Outcome::Plan(plan) => return emit_plan(ctx, "opendap.vm/v1", plan),
        Outcome::Done(done) => done,
    };
    let verb = if done.changed {
        "started"
    } else {
        "already running"
    };
    emit_changed(ctx, done, verb);
    Ok(0)
}

fn stop(ctx: &Ctx, name: &str) -> Result<u8, CliError> {
    let done = match vm::stop(&LocalVms, name, ctx.dry_run, &SystemClock::default())? {
        Outcome::Plan(plan) => return emit_plan(ctx, "opendap.vm/v1", plan),
        Outcome::Done(done) => done,
    };
    let verb = if done.changed {
        "stopped"
    } else {
        "already stopped"
    };
    emit_changed(ctx, done, verb);
    Ok(0)
}
