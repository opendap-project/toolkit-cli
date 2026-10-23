//! Restarting a device into a boot mode, and powering it off: direct requests to the device, the
//! way `adb reboot recovery` is, never an imitation of the button combinations (those stay a way
//! to test the buttons: `opendap input`).

use opendap_model::error as codes;
use opendap_model::{Error, PowerAction, PowerDone, Target};

use crate::Outcome;
use crate::ports::{DeviceTransport, Exit, VmProvider};
use crate::target::{Need, resolve_target};

/// What runs on the device: its own `odap-reboot` (blackbird-os), which knows the modes it has.
pub fn device_command(action: PowerAction) -> Vec<String> {
    match action {
        PowerAction::Reboot { mode } => vec!["odap-reboot".into(), mode.as_str().into()],
        PowerAction::Poweroff => vec!["systemctl".into(), "poweroff".into()],
    }
}

pub fn power<P, T, F>(
    provider: &P,
    target: Option<&Target>,
    action: PowerAction,
    dry_run: bool,
    connect: F,
) -> Result<Outcome<PowerDone>, Error>
where
    P: VmProvider,
    T: DeviceTransport,
    F: FnOnce(P::Vm) -> T,
{
    let (vm, target) = resolve_target(provider, target, Need::Ssh)?;
    let transport = connect(vm);
    transport.ensure_available()?;
    let argv = device_command(action);
    if dry_run {
        return Ok(Outcome::Plan(vec![transport.describe(&target, &argv)]));
    }
    let out = transport.run_captured(&argv)?;
    let message = |fallback: &str| {
        let m = out.stderr.trim().trim_start_matches("odap-reboot: ");
        if m.is_empty() {
            fallback.to_string()
        } else {
            m.to_string()
        }
    };
    match out.exit {
        // A device going down may close the connection before the command returns.
        Exit::Code(0) | Exit::TransportFailed | Exit::Signal => {
            Ok(Outcome::Done(PowerDone { target, action }))
        }
        Exit::Code(2) => Err(Error::new(
            &codes::DEVICE_MODE_UNSUPPORTED,
            message("no such boot mode on the device"),
        )),
        Exit::Code(c) => Err(Error::new(
            &codes::DEVICE_POWER_FAILED,
            message(&format!("the device answered {c}")),
        )),
    }
}

#[cfg(test)]
mod tests {
    use opendap_model::BootMode;

    use super::*;
    use crate::fake::{FakeProvider, FakeTransport};

    fn run(
        action: PowerAction,
        dry_run: bool,
        t: &FakeTransport,
    ) -> Result<Outcome<PowerDone>, Error> {
        power(&FakeProvider::running(), None, action, dry_run, |_| t)
    }

    #[test]
    fn reboot_asks_the_device_for_the_mode() {
        let t = FakeTransport::default();
        let done = run(
            PowerAction::Reboot {
                mode: BootMode::Debug,
            },
            false,
            &t,
        )
        .unwrap();
        assert!(matches!(
            done,
            Outcome::Done(PowerDone {
                action: PowerAction::Reboot {
                    mode: BootMode::Debug
                },
                ..
            })
        ));
        assert_eq!(t.ran.borrow()[0], vec!["odap-reboot", "debug"]);
    }

    #[test]
    fn a_mode_the_device_lacks_is_unsupported() {
        let t = FakeTransport::default();
        let err = run(
            PowerAction::Reboot {
                mode: BootMode::Maskrom,
            },
            false,
            &t,
        )
        .unwrap_err();
        assert_eq!(err.code.code, "device.mode_unsupported");
    }

    #[test]
    fn poweroff_and_dry_run() {
        let t = FakeTransport::default();
        assert!(matches!(
            run(PowerAction::Poweroff, false, &t).unwrap(),
            Outcome::Done(_)
        ));
        assert_eq!(t.ran.borrow()[0], vec!["systemctl", "poweroff"]);
        let plan = run(PowerAction::Poweroff, true, &t).unwrap();
        assert!(matches!(plan, Outcome::Plan(p) if p[0].contains("systemctl poweroff")));
    }

    #[test]
    fn a_closed_connection_while_going_down_is_success() {
        let t = FakeTransport {
            broken: true,
            ..Default::default()
        };
        assert!(matches!(
            run(
                PowerAction::Reboot {
                    mode: BootMode::Normal
                },
                false,
                &t
            )
            .unwrap(),
            Outcome::Done(_)
        ));
    }
}
