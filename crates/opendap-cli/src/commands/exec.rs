use std::process::Stdio;

use opendap_cli_kit::CliError;
use opendap_model::Transport;
use opendap_model::error as codes;
use serde::Serialize;

use super::{Ctx, Doc, Need, emit_plan, resolve_target};
use crate::virt::{require_tool, shell_quote};

#[derive(Serialize)]
struct ExecResult {
    target: Transport,
    command: Vec<String>,
    exit_code: i32,
    stdout: String,
    stderr: String,
}

/// ssh's own exit status for connection and authentication failures.
const SSH_FAILURE: i32 = 255;

pub fn run(ctx: &Ctx, argv: Vec<String>) -> Result<u8, CliError> {
    let interactive = argv.is_empty();
    if interactive && (!ctx.interactive || ctx.out.mode.is_json()) {
        return Err(CliError::new(
            &codes::INPUT_MISSING,
            "no command given, and an interactive shell needs a TTY without --no-input or JSON output",
        )
        .hint("Pass the command after `--`, for example `opendap exec -- uname -a`."));
    }
    let (vm, target) = resolve_target(ctx, Need::Ssh)?;
    require_tool("ssh", "reaches the device")?;
    let remote = (!interactive).then(|| {
        argv.iter()
            .map(|a| shell_quote(a))
            .collect::<Vec<_>>()
            .join(" ")
    });

    if ctx.dry_run {
        let what = remote
            .clone()
            .unwrap_or_else(|| "an interactive shell".into());
        return emit_plan(
            ctx,
            "opendap.exec/v1",
            vec![format!("ssh root@{target} via dap-qemu.sh: {what}")],
        );
    }

    let mut cmd = vm.ssh_command(remote);
    if ctx.out.mode.is_json() {
        let out = cmd
            .stdin(Stdio::null())
            .output()
            .map_err(|e| CliError::new(&codes::HOST_IO, format!("cannot run ssh: {e}")))?;
        let code = out.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if code == SSH_FAILURE {
            return Err(CliError::new(
                &codes::TRANSPORT_SSH_FAILED,
                format!("ssh to {target} failed"),
            )
            .details(serde_json::json!({ "stderr": stderr })));
        }
        let result = ExecResult {
            target,
            command: argv,
            exit_code: code,
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr,
        };
        ctx.out.emit(
            &Doc {
                schema: "opendap.exec/v1",
                body: &result,
            },
            String::new,
        );
        // The remote status is in `exit_code`; the process fails when it is non-zero.
        return Ok(if code == 0 { 0 } else { codes::exit::FAILURE });
    }

    // Human and plain modes: stream through and return the remote exit code, like ssh.
    let status = cmd
        .status()
        .map_err(|e| CliError::new(&codes::HOST_IO, format!("cannot run ssh: {e}")))?;
    match status.code() {
        Some(SSH_FAILURE) => Err(CliError::new(
            &codes::TRANSPORT_SSH_FAILED,
            format!("ssh to {target} failed"),
        )),
        Some(c) => Ok(u8::try_from(c).unwrap_or(codes::exit::FAILURE)),
        None => Ok(codes::exit::INTERRUPTED),
    }
}
