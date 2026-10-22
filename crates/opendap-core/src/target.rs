//! Resolving `--target` (or the single running candidate) to a device.

use opendap_model::error as codes;
use opendap_model::vm::DEFAULT_NAME;
use opendap_model::{Error, Target, Transport, VmState};

use crate::ports::{VmBackend, VmProvider};
use crate::vm::check_name;

/// What a targeted use case needs from the virtual T1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    /// SSH must answer (the guest has booted).
    Ssh,
    /// The emulator must run (QMP answers from the first instant).
    Qmp,
}

/// Resolve a target selector, or the single running T1 when there is none.
/// Only virtual T1s are reachable today.
pub fn resolve_target<P: VmProvider>(
    provider: &P,
    target: Option<&Target>,
    need: Need,
) -> Result<(P::Vm, Transport), Error> {
    let explicit = match target {
        None => None,
        Some(Target::Transport(Transport::Vm { name })) => {
            check_name(name)?;
            Some(name.clone())
        }
        Some(Target::Transport(t)) => {
            return Err(Error::new(
                &codes::TARGET_UNSUPPORTED,
                format!("`{t}`: {} targets are not supported yet", t.kind()),
            )
            .fix("opendap device list"));
        }
        Some(Target::Context(name)) => {
            return Err(Error::new(
                &codes::TARGET_UNSUPPORTED,
                format!("`{name}`: contexts are not supported yet"),
            )
            .hint("Pass a selector such as vm:default."));
        }
    };
    let vm = provider.locate()?;
    let transport = Transport::Vm {
        name: DEFAULT_NAME.into(),
    };
    let state = vm.state();
    let usable = match need {
        Need::Ssh => state == VmState::Running,
        Need::Qmp => state != VmState::Stopped,
    };
    if usable {
        return Ok((vm, transport));
    }
    if explicit.is_none() && state == VmState::Stopped {
        return Err(Error::new(&codes::DEVICE_NOT_FOUND, "no T1 is running")
            .fix("opendap vm start")
            .fix("opendap device list"));
    }
    let what = if state == VmState::Booting {
        "still booting"
    } else {
        "stopped"
    };
    Err(
        Error::new(&codes::VM_NOT_RUNNING, format!("vm:default is {what}"))
            .fix("opendap vm start")
            .details(serde_json::json!({ "target": "vm:default", "state": state })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeProvider, FakeVm};

    fn sel(s: &str) -> Target {
        s.parse().unwrap()
    }

    fn code(r: Result<(FakeVm, Transport), Error>) -> &'static str {
        r.err().expect("an error").code.code
    }

    #[test]
    fn the_single_running_t1_is_the_default_target() {
        let p = FakeProvider::running();
        let (_, t) = resolve_target(&p, None, Need::Ssh).unwrap();
        assert_eq!(t.selector(), "vm:default");
        let (_, t) = resolve_target(&p, Some(&sel("vm:default")), Need::Qmp).unwrap();
        assert_eq!(t.selector(), "vm:default");
    }

    #[test]
    fn nothing_running() {
        let p = FakeProvider::stopped();
        assert_eq!(
            code(resolve_target(&p, None, Need::Qmp)),
            "device.not_found"
        );
        let err = resolve_target(&p, Some(&sel("vm:default")), Need::Ssh)
            .err()
            .unwrap();
        assert_eq!(err.code.code, "vm.not_running");
        assert_eq!(err.message, "vm:default is stopped");
        assert_eq!(err.details.unwrap()["state"], "stopped");
    }

    #[test]
    fn a_booting_vm_answers_qmp_but_not_ssh() {
        let p = FakeProvider::booting();
        assert!(resolve_target(&p, None, Need::Qmp).is_ok());
        let err = resolve_target(&p, None, Need::Ssh).err().unwrap();
        assert_eq!(err.code.code, "vm.not_running");
        assert_eq!(err.message, "vm:default is still booting");
    }

    #[test]
    fn unsupported_and_unknown_targets_fail_before_locating() {
        let p = FakeProvider::missing();
        assert_eq!(
            code(resolve_target(&p, Some(&sel("usb:ABC")), Need::Ssh)),
            "target.unsupported"
        );
        assert_eq!(
            code(resolve_target(&p, Some(&sel("lab")), Need::Ssh)),
            "target.unsupported"
        );
        assert_eq!(
            code(resolve_target(&p, Some(&sel("vm:other")), Need::Ssh)),
            "vm.not_found"
        );
        assert_eq!(
            code(resolve_target(&p, None, Need::Ssh)),
            "workspace.not_found"
        );
    }
}
