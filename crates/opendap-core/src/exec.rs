//! Running a command on a device.

use opendap_model::error as codes;
use opendap_model::{Error, ExecResult, Target};

use crate::Outcome;
use crate::ports::{DeviceTransport, Exit, VmProvider};
use crate::target::{Need, resolve_target};

/// Options of [`exec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecOptions {
    /// Capture the output and return it (JSON surfaces) instead of attaching
    /// the command to the caller's terminal.
    pub capture: bool,
    /// An interactive shell may be opened (a TTY, no `--no-input`).
    pub interactive: bool,
    pub dry_run: bool,
}

/// How [`exec`] ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecDone {
    /// Captured mode: the result, and the process exit code (1 when the
    /// remote command failed).
    Captured { result: ExecResult, exit: u8 },
    /// Attached mode: the remote exit code, passed through.
    Attached { exit: u8 },
}

/// Run `argv` on the target, or open a shell when `argv` is empty.
/// `connect` builds the transport for the resolved virtual T1.
pub fn exec<P, T, F>(
    provider: &P,
    target: Option<&Target>,
    argv: Vec<String>,
    opts: ExecOptions,
    connect: F,
) -> Result<Outcome<ExecDone>, Error>
where
    P: VmProvider,
    T: DeviceTransport,
    F: FnOnce(P::Vm) -> T,
{
    let shell = argv.is_empty();
    if shell && (!opts.interactive || opts.capture) {
        return Err(Error::new(
            &codes::INPUT_MISSING,
            "no command given, and an interactive shell needs a TTY without --no-input or JSON output",
        )
        .hint("Pass the command after `--`, for example `opendap exec -- uname -a`."));
    }
    let (vm, target) = resolve_target(provider, target, Need::Ssh)?;
    let transport = connect(vm);
    transport.ensure_available()?;
    if opts.dry_run {
        return Ok(Outcome::Plan(vec![transport.describe(&target, &argv)]));
    }
    let failed = || {
        Error::new(
            &codes::TRANSPORT_SSH_FAILED,
            format!("ssh to {target} failed"),
        )
    };
    if opts.capture {
        let out = transport.run_captured(&argv)?;
        let code = match out.exit {
            Exit::TransportFailed => {
                return Err(failed().details(serde_json::json!({ "stderr": out.stderr })));
            }
            Exit::Code(c) => c,
            Exit::Signal => -1,
        };
        let exit = if code == 0 { 0 } else { codes::exit::FAILURE };
        return Ok(Outcome::Done(ExecDone::Captured {
            result: ExecResult {
                target,
                command: argv,
                exit_code: code,
                stdout: out.stdout,
                stderr: out.stderr,
            },
            exit,
        }));
    }
    let exit = match transport.run_attached(&argv)? {
        Exit::TransportFailed => return Err(failed()),
        Exit::Code(c) => u8::try_from(c).unwrap_or(codes::exit::FAILURE),
        Exit::Signal => codes::exit::INTERRUPTED,
    };
    Ok(Outcome::Done(ExecDone::Attached { exit }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeProvider, FakeTransport};

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    const CAPTURE: ExecOptions = ExecOptions {
        capture: true,
        interactive: false,
        dry_run: false,
    };
    const ATTACHED: ExecOptions = ExecOptions {
        capture: false,
        interactive: true,
        dry_run: false,
    };

    fn run(
        p: &FakeProvider,
        a: &[&str],
        opts: ExecOptions,
        t: FakeTransport,
    ) -> Result<Outcome<ExecDone>, Error> {
        exec(p, None, argv(a), opts, |_| t)
    }

    #[test]
    fn a_shell_needs_an_interactive_attached_session() {
        let p = FakeProvider::running();
        for opts in [
            CAPTURE,
            ExecOptions {
                interactive: false,
                ..ATTACHED
            },
        ] {
            let err = run(&p, &[], opts, FakeTransport::default()).unwrap_err();
            assert_eq!(err.code.code, "input.missing");
        }
        // Checked before the target, so it fails the same with nothing running.
        let err = run(
            &FakeProvider::missing(),
            &[],
            CAPTURE,
            FakeTransport::default(),
        )
        .unwrap_err();
        assert_eq!(err.code.code, "input.missing");
        let done = run(&p, &[], ATTACHED, FakeTransport::default()).unwrap();
        assert_eq!(done, Outcome::Done(ExecDone::Attached { exit: 0 }));
    }

    #[test]
    fn captured_results_carry_the_remote_status() {
        let p = FakeProvider::running();
        let Outcome::Done(ExecDone::Captured { result, exit }) = run(
            &p,
            &["echo", "-oProxyCommand=x"],
            CAPTURE,
            FakeTransport::default(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(exit, 0);
        assert_eq!(result.stdout, "-oProxyCommand=x\n");
        assert_eq!(result.command, ["echo", "-oProxyCommand=x"]);
        assert_eq!(result.target.selector(), "vm:default");

        let Outcome::Done(ExecDone::Captured { result, exit }) = run(
            &p,
            &["sh", "-c", "exit 3"],
            CAPTURE,
            FakeTransport::default(),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!((result.exit_code, exit), (3, 1));
    }

    #[test]
    fn attached_runs_pass_the_exit_code_through() {
        let p = FakeProvider::running();
        let done = run(
            &p,
            &["sh", "-c", "exit 3"],
            ATTACHED,
            FakeTransport::default(),
        )
        .unwrap();
        assert_eq!(done, Outcome::Done(ExecDone::Attached { exit: 3 }));
        let done = run(
            &p,
            &["sh", "-c", "exit 300"],
            ATTACHED,
            FakeTransport::default(),
        )
        .unwrap();
        assert_eq!(done, Outcome::Done(ExecDone::Attached { exit: 1 }));
    }

    #[test]
    fn transport_failures_are_errors() {
        let p = FakeProvider::running();
        let broken = || FakeTransport {
            broken: true,
            ..Default::default()
        };
        let err = run(&p, &["true"], CAPTURE, broken()).unwrap_err();
        assert_eq!(err.code.code, "transport.ssh_failed");
        assert_eq!(err.message, "ssh to vm:default failed");
        assert_eq!(err.details.unwrap()["stderr"], "connection refused");
        let err = run(&p, &["true"], ATTACHED, broken()).unwrap_err();
        assert_eq!(err.code.code, "transport.ssh_failed");
        assert!(err.details.is_none());
    }

    #[test]
    fn dry_run_describes_and_runs_nothing() {
        let p = FakeProvider::running();
        let t = FakeTransport::default();
        let out = exec(
            &p,
            None,
            argv(&["echo", "hi"]),
            ExecOptions {
                dry_run: true,
                ..CAPTURE
            },
            |_| &t,
        )
        .unwrap();
        assert_eq!(
            out,
            Outcome::Plan(vec!["fake exec on vm:default: echo hi".into()])
        );
        assert!(t.ran.borrow().is_empty());
    }

    #[test]
    fn needs_a_booted_target() {
        let err = run(
            &FakeProvider::booting(),
            &["true"],
            CAPTURE,
            FakeTransport::default(),
        )
        .unwrap_err();
        assert_eq!(err.code.code, "vm.not_running");
    }
}
