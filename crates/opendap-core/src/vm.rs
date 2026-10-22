//! Virtual T1 lifecycle: list, get, start, stop.

use std::time::Duration;

use opendap_model::error as codes;
use opendap_model::vm::DEFAULT_NAME;
use opendap_model::{Error, VmInfo, VmState};

use crate::Outcome;
use crate::ports::{Clock, VmBackend, VmProvider};

/// How long `stop` waits for the emulator to exit.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(30);

/// The state of an instance from its emulator PID and whether SSH answers.
/// `ssh_ready` is asked only when the emulator runs.
pub fn state_of(pid: Option<u32>, ssh_ready: impl FnOnce() -> bool) -> VmState {
    match pid {
        None => VmState::Stopped,
        Some(_) if ssh_ready() => VmState::Running,
        Some(_) => VmState::Booting,
    }
}

/// Check a VM name; only `default` exists.
pub fn check_name(name: &str) -> Result<(), Error> {
    if name == DEFAULT_NAME {
        Ok(())
    } else {
        Err(Error::new(
            &codes::VM_NOT_FOUND,
            format!("no virtual T1 named `{name}`"),
        )
        .fix("opendap vm list"))
    }
}

/// Every virtual T1 instance.
pub fn list<P: VmProvider>(provider: &P) -> Result<Vec<VmInfo>, Error> {
    Ok(vec![provider.locate()?.info()])
}

/// One virtual T1 instance by name.
pub fn get<P: VmProvider>(provider: &P, name: &str) -> Result<VmInfo, Error> {
    check_name(name)?;
    Ok(provider.locate()?.info())
}

/// An instance after a lifecycle change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub info: VmInfo,
    /// False when it already was in the requested state.
    pub changed: bool,
}

/// Options of [`start`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartOptions {
    /// Return once the emulator runs, without waiting for SSH.
    pub detach: bool,
    /// How long to wait for SSH.
    pub wait_timeout: Duration,
    pub dry_run: bool,
}

/// Boot the virtual T1 and, unless detached, wait until SSH answers.
/// Idempotent. `progress` receives status lines for people.
pub fn start<P: VmProvider>(
    provider: &P,
    name: &str,
    opts: StartOptions,
    clock: &dyn Clock,
    progress: &dyn Fn(&str),
) -> Result<Outcome<Changed>, Error> {
    check_name(name)?;
    let vm = provider.locate()?;
    let state = vm.state();
    let secs = opts.wait_timeout.as_secs();
    if opts.dry_run {
        let mut plan = Vec::new();
        if state == VmState::Stopped {
            plan.push(vm.start_command());
        }
        if !opts.detach && state != VmState::Running {
            plan.push(format!(
                "wait until SSH answers on {} (up to {secs}s)",
                vm.ssh_endpoint()
            ));
        }
        return Ok(Outcome::Plan(plan));
    }
    let changed = state == VmState::Stopped;
    if changed {
        if !vm.image_present() {
            let building = vm.image_build_running();
            let err = if building {
                Error::new(
                    &codes::VM_IMAGE_MISSING,
                    "the virtual T1 image is being rebuilt (dap-image.sh is running)",
                )
                .hint("Wait for the build to finish, then retry.")
            } else {
                Error::new(
                    &codes::VM_IMAGE_MISSING,
                    format!("no virtual T1 image at {}", vm.image_path()),
                )
                .fix(vm.image_build_command())
            };
            return Err(err);
        }
        vm.launch()?;
        progress(&format!(
            "vm:default started (pid {})",
            vm.pid().unwrap_or_default()
        ));
    }
    if !opts.detach && vm.state() != VmState::Running {
        progress(&format!("waiting for vm:default to boot (up to {secs}s)…"));
        let deadline = clock.now() + opts.wait_timeout;
        loop {
            if vm.pid().is_none() {
                return Err(
                    Error::new(&codes::VM_START_FAILED, "QEMU exited during boot")
                        .details(serde_json::json!({ "qemu_log": vm.emulator_log_tail(20) })),
                );
            }
            if vm.ssh_ready() {
                break;
            }
            if clock.now() >= deadline {
                return Err(Error::new(
                    &codes::VM_BOOT_TIMEOUT,
                    format!("SSH did not answer within {secs}s"),
                )
                .details(serde_json::json!({ "console_tail": vm.console_log_tail(20) })));
            }
            clock.sleep(Duration::from_secs(1));
        }
    }
    Ok(Outcome::Done(Changed {
        info: vm.info(),
        changed,
    }))
}

/// Stop the virtual T1 and wait until the emulator exits. Idempotent.
pub fn stop<P: VmProvider>(
    provider: &P,
    name: &str,
    dry_run: bool,
    clock: &dyn Clock,
) -> Result<Outcome<Changed>, Error> {
    check_name(name)?;
    let vm = provider.locate()?;
    let running = vm.pid().is_some();
    if dry_run {
        let plan = if running {
            vec![vm.stop_command()]
        } else {
            vec![]
        };
        return Ok(Outcome::Plan(plan));
    }
    if running {
        vm.request_stop()?;
        let deadline = clock.now() + STOP_TIMEOUT;
        while vm.pid().is_some() {
            if clock.now() >= deadline {
                return Err(Error::new(
                    &codes::VM_STOP_TIMEOUT,
                    "QEMU still runs 30s after the stop request",
                ));
            }
            clock.sleep(Duration::from_millis(250));
        }
    }
    Ok(Outcome::Done(Changed {
        info: vm.info(),
        changed: running,
    }))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::fake::{FakeClock, FakeProvider};

    fn opts(detach: bool, dry_run: bool) -> StartOptions {
        StartOptions {
            detach,
            wait_timeout: Duration::from_secs(180),
            dry_run,
        }
    }

    fn done<T>(o: Outcome<T>) -> T {
        match o {
            Outcome::Done(t) => t,
            Outcome::Plan(p) => panic!("unexpected plan {p:?}"),
        }
    }

    fn plan<T: std::fmt::Debug>(o: Outcome<T>) -> Vec<String> {
        match o {
            Outcome::Plan(p) => p,
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn names_other_than_default_do_not_exist() {
        assert!(check_name("default").is_ok());
        assert_eq!(check_name("other").unwrap_err().code.code, "vm.not_found");
        let p = FakeProvider::running();
        assert_eq!(get(&p, "x").unwrap_err().code.code, "vm.not_found");
        assert_eq!(get(&p, "default").unwrap().state, VmState::Running);
        assert_eq!(list(&p).unwrap().len(), 1);
    }

    #[test]
    fn state_needs_ssh_only_when_the_emulator_runs() {
        assert_eq!(state_of(None, || panic!("not asked")), VmState::Stopped);
        assert_eq!(state_of(Some(1), || false), VmState::Booting);
        assert_eq!(state_of(Some(1), || true), VmState::Running);
    }

    #[test]
    fn dry_run_plans_follow_the_state() {
        let clock = FakeClock::default();
        let p = FakeProvider::stopped();
        let steps = plan(start(&p, "default", opts(false, true), &clock, &|_| {}).unwrap());
        assert_eq!(
            steps,
            [
                "sh /os/build/dap-qemu.sh start",
                "wait until SSH answers on 127.0.0.1:2222 (up to 180s)"
            ]
        );
        let steps = plan(start(&p, "default", opts(true, true), &clock, &|_| {}).unwrap());
        assert_eq!(steps, ["sh /os/build/dap-qemu.sh start"]);
        assert!(plan(stop(&p, "default", true, &clock).unwrap()).is_empty());

        let p = FakeProvider::running();
        assert!(plan(start(&p, "default", opts(false, true), &clock, &|_| {}).unwrap()).is_empty());
        assert_eq!(
            plan(stop(&p, "default", true, &clock).unwrap()),
            ["sh /os/build/dap-qemu.sh stop"]
        );
        assert_eq!(
            p.vm.launches.get() + p.vm.stops.get(),
            0,
            "dry-run changes nothing"
        );
    }

    #[test]
    fn start_boots_and_waits_for_ssh() {
        let clock = FakeClock::default();
        let p = FakeProvider::stopped();
        p.vm.boot_secs.set(5);
        let notes = RefCell::new(Vec::new());
        let c = done(
            start(&p, "default", opts(false, false), &clock, &|s| {
                notes.borrow_mut().push(s.to_string())
            })
            .unwrap(),
        );
        assert!(c.changed);
        assert_eq!(c.info.state, VmState::Running);
        assert_eq!(
            notes.into_inner(),
            [
                "vm:default started (pid 4242)",
                "waiting for vm:default to boot (up to 180s)…"
            ]
        );
        assert_eq!(p.vm.launches.get(), 1);
    }

    #[test]
    fn start_is_idempotent_and_detach_does_not_wait() {
        let clock = FakeClock::default();
        let p = FakeProvider::running();
        let c = done(start(&p, "default", opts(false, false), &clock, &|_| {}).unwrap());
        assert!(!c.changed);
        assert_eq!(p.vm.launches.get(), 0);

        let p = FakeProvider::stopped();
        p.vm.boot_secs.set(1000);
        let c = done(start(&p, "default", opts(true, false), &clock, &|_| {}).unwrap());
        assert!(c.changed);
        assert_eq!(c.info.state, VmState::Booting);
    }

    #[test]
    fn start_reports_a_missing_image() {
        let clock = FakeClock::default();
        let p = FakeProvider::stopped();
        p.vm.image.set(false);
        let err = start(&p, "default", opts(false, false), &clock, &|_| {}).unwrap_err();
        assert_eq!(err.code.code, "vm.image_missing");
        assert_eq!(err.message, "no virtual T1 image at /os/image.raw");
        assert_eq!(
            err.fix[0].command.as_deref(),
            Some("sh /os/build/dap-image.sh qemu")
        );
        p.vm.building.set(true);
        let err = start(&p, "default", opts(false, false), &clock, &|_| {}).unwrap_err();
        assert_eq!(err.code.code, "vm.image_missing");
        assert!(err.fix.is_empty() && err.hint.is_some());
        assert_eq!(p.vm.launches.get(), 0);
    }

    #[test]
    fn start_times_out_or_sees_the_emulator_die() {
        let clock = FakeClock::default();
        let p = FakeProvider::stopped();
        p.vm.boot_secs.set(1000);
        let o = StartOptions {
            wait_timeout: Duration::from_secs(10),
            ..opts(false, false)
        };
        let err = start(&p, "default", o, &clock, &|_| {}).unwrap_err();
        assert_eq!(err.code.code, "vm.boot_timeout");
        assert_eq!(err.message, "SSH did not answer within 10s");
        assert_eq!(err.details.unwrap()["console_tail"], "console tail");
        assert_eq!(clock.now(), Duration::from_secs(10));

        let p = FakeProvider::stopped();
        p.vm.dies_on_boot.set(true);
        let err = start(&p, "default", opts(false, false), &clock, &|_| {}).unwrap_err();
        assert_eq!(err.code.code, "vm.start_failed");
        assert_eq!(err.details.unwrap()["qemu_log"], "emulator tail");
    }

    #[test]
    fn stop_waits_for_the_emulator() {
        let clock = FakeClock::default();
        let p = FakeProvider::running();
        let c = done(stop(&p, "default", false, &clock).unwrap());
        assert!(c.changed);
        assert_eq!(c.info.state, VmState::Stopped);
        let c = done(stop(&p, "default", false, &clock).unwrap());
        assert!(!c.changed);
        assert_eq!(p.vm.stops.get(), 1);

        let p = FakeProvider::running();
        p.vm.ignores_stop.set(true);
        let err = stop(&p, "default", false, &clock).unwrap_err();
        assert_eq!(err.code.code, "vm.stop_timeout");
    }
}
