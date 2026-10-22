//! SSH adapter of the OpenDAP toolkit: implements
//! [`opendap_core::DeviceTransport`] over an `ssh` command line.
//!
//! The adapter does not choose the key, port or user: it is given a factory
//! for the base command (for the virtual T1, `dap-qemu.sh ssh`, which ends
//! ssh's own options with `--`) and appends the remote command as one
//! argument, quoted for the remote POSIX shell.

use std::process::{Command, Stdio};

use opendap_core::{Captured, DeviceTransport, Exit};
use opendap_model::error as codes;
use opendap_model::{Error, Transport};

/// ssh's own exit status for connection and authentication failures.
pub const SSH_FAILURE: i32 = 255;

/// Quote one argument for the remote POSIX shell, so argv reaches the device unchanged.
pub fn shell_quote(arg: &str) -> String {
    // An argument starting with '-' is always quoted, so the remote command can never start with
    // something ssh could read as an option (dap-qemu.sh also ends ssh options with `--`).
    let safe = !arg.is_empty()
        && !arg.starts_with('-')
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c));
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// The remote command line for `argv`: each argument quoted, joined by spaces.
pub fn remote_command(argv: &[String]) -> String {
    argv.iter()
        .map(|a| shell_quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Runs commands through an ssh command line built by `base`.
pub struct SshExec<F: Fn() -> Command> {
    base: F,
    via: String,
}

impl<F: Fn() -> Command> SshExec<F> {
    /// `base` builds the ssh command line without the remote command; `via`
    /// names it in dry-run plans (for example `dap-qemu.sh`).
    pub fn new(base: F, via: impl Into<String>) -> Self {
        SshExec {
            base,
            via: via.into(),
        }
    }

    fn command(&self, argv: &[String]) -> Command {
        let mut c = (self.base)();
        if !argv.is_empty() {
            c.arg(remote_command(argv));
        }
        c
    }
}

fn exit_of(code: Option<i32>) -> Exit {
    match code {
        Some(SSH_FAILURE) => Exit::TransportFailed,
        Some(c) => Exit::Code(c),
        None => Exit::Signal,
    }
}

fn spawn_error(e: std::io::Error) -> Error {
    Error::new(&codes::HOST_IO, format!("cannot run ssh: {e}"))
}

impl<F: Fn() -> Command> DeviceTransport for SshExec<F> {
    fn ensure_available(&self) -> Result<(), Error> {
        opendap_host::require_tool("ssh", "reaches the device").map(|_| ())
    }

    fn describe(&self, target: &Transport, argv: &[String]) -> String {
        let what = if argv.is_empty() {
            "an interactive shell".to_string()
        } else {
            remote_command(argv)
        };
        format!("ssh root@{target} via {}: {what}", self.via)
    }

    fn run_captured(&self, argv: &[String]) -> Result<Captured, Error> {
        let out = self
            .command(argv)
            .stdin(Stdio::null())
            .output()
            .map_err(spawn_error)?;
        Ok(Captured {
            exit: exit_of(out.status.code()),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }

    fn run_attached(&self, argv: &[String]) -> Result<Exit, Error> {
        let status = self.command(argv).status().map_err(spawn_error)?;
        Ok(exit_of(status.code()))
    }
}

#[cfg(test)]
mod tests {
    use opendap_core::contract;

    use super::*;

    #[test]
    fn quoting_preserves_argv() {
        assert_eq!(shell_quote("uname"), "uname");
        assert_eq!(shell_quote("-a"), "'-a'");
        assert_eq!(shell_quote("ls | wc"), "'ls | wc'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("-oProxyCommand=x"), "'-oProxyCommand=x'");
    }

    /// A local POSIX shell stands in for ssh: it receives the remote command
    /// line exactly as ssh would hand it to the device's shell.
    fn local() -> SshExec<impl Fn() -> Command> {
        SshExec::new(
            || {
                let mut c = Command::new("sh");
                c.arg("-c");
                c
            },
            "a local shell",
        )
    }

    #[test]
    fn passes_the_device_transport_contract() {
        contract::device_transport(&local());
    }

    #[test]
    fn plans_name_the_launcher_and_the_quoted_command() {
        let t = Transport::Vm {
            name: "default".into(),
        };
        let argv = vec!["echo".to_string(), "-oProxyCommand=x".to_string()];
        assert_eq!(
            local().describe(&t, &argv),
            "ssh root@vm:default via a local shell: echo '-oProxyCommand=x'"
        );
        assert_eq!(
            local().describe(&t, &[]),
            "ssh root@vm:default via a local shell: an interactive shell"
        );
    }

    #[test]
    fn status_255_is_a_transport_failure() {
        assert_eq!(exit_of(Some(255)), Exit::TransportFailed);
        assert_eq!(exit_of(Some(0)), Exit::Code(0));
        assert_eq!(exit_of(None), Exit::Signal);
        let out = local()
            .run_captured(&["sh".into(), "-c".into(), "exit 255".into()])
            .unwrap();
        assert_eq!(out.exit, Exit::TransportFailed);
    }
}
