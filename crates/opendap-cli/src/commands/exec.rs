use opendap_cli_kit::CliError;
use opendap_core::Outcome;
use opendap_core::exec::{ExecDone, ExecOptions, exec};
use opendap_transport_ssh::SshExec;
use opendap_transport_vm::{LocalVms, SSH_LAUNCHER};

use super::{Ctx, Doc, emit_plan};

pub fn run(ctx: &Ctx, argv: Vec<String>) -> Result<u8, CliError> {
    let json = ctx.out.mode.is_json();
    let opts = ExecOptions {
        capture: json,
        interactive: ctx.interactive,
        dry_run: ctx.dry_run,
    };
    let outcome = exec(&LocalVms, ctx.target.as_ref(), argv, opts, |vm| {
        SshExec::new(move || vm.ssh_command(), SSH_LAUNCHER)
    })?;
    match outcome {
        Outcome::Plan(plan) => emit_plan(ctx, "opendap.exec/v1", plan),
        Outcome::Done(ExecDone::Captured { result, exit }) => {
            ctx.out.emit(
                &Doc {
                    schema: "opendap.exec/v1",
                    body: &result,
                },
                String::new,
            );
            // The remote status is in `exit_code`; the process fails when it is non-zero.
            Ok(exit)
        }
        // Human and plain modes stream through and return the remote exit code, like ssh.
        Outcome::Done(ExecDone::Attached { exit }) => Ok(exit),
    }
}
