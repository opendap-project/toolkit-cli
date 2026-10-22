//! `opendap doctor`: host checks, each with a status and a fix.

use opendap_cli_kit::CliError;
use opendap_core::HostProbe;
use opendap_host::{HostPlatform, HostTools, Lima, PreCommit, Qemu};
use opendap_model::{DoctorReport, Status};
use opendap_transport_vm::CheckoutProbe;

use super::Ctx;

fn mark(s: Status) -> &'static str {
    match s {
        Status::Ok => "[✓]",
        Status::Warn => "[!]",
        Status::Error => "[✗]",
        Status::Skip => "[-]",
    }
}

fn human(r: &DoctorReport) -> String {
    let mut s = String::new();
    for c in &r.checks {
        s.push_str(&format!("{} {}: {}\n", mark(c.status), c.title, c.message));
        if let Some(f) = c.fix.as_ref().filter(|_| c.status != Status::Ok) {
            if let Some(d) = &f.description {
                s.push_str(&format!("    fix: {d}\n"));
            }
            if let Some(cmd) = &f.command {
                s.push_str(&format!("    run: {cmd}\n"));
            }
        }
    }
    let m = &r.summary;
    s.push_str(&format!(
        "\n{} ok, {} warning(s), {} error(s), {} skipped\n",
        m.ok, m.warn, m.error, m.skip
    ));
    s
}

pub fn run(ctx: &Ctx) -> Result<u8, CliError> {
    let probes: [&dyn HostProbe; 6] = [
        &HostPlatform,
        &HostTools,
        &Qemu,
        &Lima,
        &CheckoutProbe,
        &PreCommit,
    ];
    let report = opendap_core::doctor::run(&probes);
    ctx.out.emit(&report, || human(&report));
    Ok(if report.ok { 0 } else { 1 })
}
