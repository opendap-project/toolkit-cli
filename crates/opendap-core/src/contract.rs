//! Contract suites: one per port. Every implementation of a port runs its
//! suite in its own tests (the fakes in this crate, the adapters in theirs).
//! Enable the `contract` feature to use them from another crate.
//!
//! The suites panic on the first broken rule, like `assert!`.

use opendap_model::vm::DEFAULT_NAME;
use opendap_model::{Attachment, HardwareChange, InputEvent, Role, Status, Transport, VmState};

use crate::ports::{
    DeviceTransport, Exit, HardwareBench, HostProbe, InputSink, ScreenSource, VmBackend,
};
use crate::screen::png_size;

fn vm_target() -> Transport {
    Transport::Vm {
        name: DEFAULT_NAME.into(),
    }
}

/// [`VmBackend`]: the description agrees with the probes, and every command
/// shown to people is non-empty. Read-only: it neither starts nor stops.
pub fn vm_backend<B: VmBackend>(vm: &B) {
    let info = vm.info();
    let state = vm.state();
    assert_eq!(info.id, format!("vm:{DEFAULT_NAME}"));
    assert_eq!(info.name, DEFAULT_NAME);
    assert_eq!(info.state, state, "info() and state() agree");
    assert_eq!(info.pid.is_none(), state == VmState::Stopped);
    assert_eq!(info.pid, vm.pid());
    assert_eq!(info.endpoints.ssh, vm.ssh_endpoint());
    assert!(!info.endpoints.vnc.is_empty() && !info.endpoints.qmp.is_empty());
    assert_eq!(info.image.present, vm.image_present());
    assert_eq!(info.image.path, vm.image_path());
    assert_eq!(info.image.bytes.is_some(), info.image.present);
    for cmd in [vm.start_command(), vm.stop_command(), vm.image_hint()] {
        assert!(!cmd.trim().is_empty());
    }
}

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// [`DeviceTransport`] on a device with a POSIX shell: each argument reaches
/// the device unchanged (in particular one that starts with `-`), and the
/// remote exit status comes back.
pub fn device_transport<T: DeviceTransport>(t: &T) {
    let argv = args(&[
        "echo",
        "-oProxyCommand=x",
        "it's a b",
        "$HOME",
        "*",
        ";",
        "|",
    ]);
    let out = t.run_captured(&argv).expect("runs");
    assert_eq!(out.exit, Exit::Code(0));
    assert_eq!(out.stdout, "-oProxyCommand=x it's a b $HOME * ; |\n");

    let out = t.run_captured(&args(&["-oProxyCommand=x"])).expect("runs");
    assert!(
        matches!(out.exit, Exit::Code(c) if c != 0),
        "a first argument that looks like an option is a command, never a transport option"
    );

    let out = t
        .run_captured(&args(&["sh", "-c", "exit 3"]))
        .expect("runs");
    assert_eq!(out.exit, Exit::Code(3));
    assert_eq!(
        t.run_attached(&args(&["sh", "-c", "exit 4"]))
            .expect("runs"),
        Exit::Code(4)
    );

    let target = vm_target();
    assert!(!t.describe(&target, &args(&["true"])).is_empty());
    assert!(!t.describe(&target, &[]).is_empty());
}

/// [`ScreenSource`]: a capture is a PNG.
pub fn screen_source<S: ScreenSource>(s: &S, path: &str) {
    assert!(!s.describe(&vm_target(), path).is_empty());
    let bytes = s.capture_png(path).expect("captures");
    let (w, h) = png_size(&bytes).expect("a PNG");
    assert!(w > 0 && h > 0);
}

/// [`InputSink`]: every event can be described. Sends nothing.
pub fn input_sink<S: InputSink>(s: &S) {
    for role in Role::COMMON {
        let d = s.describe(&InputEvent::Key {
            role: Role::from(role),
            hold_ms: 100,
        });
        assert!(!d.is_empty(), "{role}");
    }
    for role in Role::COMMON {
        let role = Role::from(role);
        assert!(
            !s.describe(&InputEvent::KeyDown { role: role.clone() })
                .is_empty()
        );
        assert!(!s.describe(&InputEvent::KeyUp { role }).is_empty());
    }
    assert!(!s.describe(&InputEvent::Tap { x: 0, y: 0 }).is_empty());
}

/// [`HardwareBench`]: every change can be described, and the state lists each attachment once,
/// in [`Attachment::ALL`] order, with a charge of at most 100 %. Changes nothing.
pub fn hardware_bench<B: HardwareBench>(b: &B) {
    for attachment in Attachment::ALL {
        assert!(
            !b.describe(&HardwareChange::Attach { attachment })
                .is_empty()
        );
        assert!(
            !b.describe(&HardwareChange::Detach { attachment })
                .is_empty()
        );
    }
    assert!(
        !b.describe(&HardwareChange::Battery { percent: 50 })
            .is_empty()
    );
    let st = b.hardware(&vm_target()).expect("reads the hardware");
    let order: Vec<Attachment> = Attachment::ALL
        .into_iter()
        .filter(|a| st.attached.contains(a))
        .collect();
    assert_eq!(st.attached, order);
    assert!(st.battery.is_none_or(|p| p <= 100));
}

/// [`HostProbe`]: findings have a dotted ID, a title and a message, and every
/// error comes with a fix.
pub fn host_probe<P: HostProbe + ?Sized>(p: &P) {
    let findings = p.check();
    assert!(!findings.is_empty(), "a probe reports at least one finding");
    for f in findings {
        assert!(f.id.contains('.'), "dotted id: {}", f.id);
        assert!(!f.title.is_empty() && !f.message.is_empty(), "{}", f.id);
        if f.status == Status::Error {
            assert!(f.fix.is_some(), "{} is an error without a fix", f.id);
        }
    }
}
