//! Fake adapters for the core's unit tests. Each one also passes its port's
//! contract suite (see the tests at the end).

use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

use opendap_model::error as codes;
use opendap_model::vm::DEFAULT_NAME;
use opendap_model::{
    Attachment, Endpoints, Error, Finding, HardwareChange, HardwareState, ImageInfo, InputEvent,
    Status, Transport, VmInfo,
};

use crate::ports::{
    Captured, Clock, DeviceTransport, Exit, HardwareBench, HostProbe, InputSink, ScreenSource,
    VmBackend, VmProvider,
};

/// A clock that only moves when the code under test sleeps.
#[derive(Default)]
pub struct FakeClock(Cell<Duration>);

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        self.0.get()
    }
    fn sleep(&self, d: Duration) {
        self.0.set(self.0.get() + d);
    }
}

#[derive(Default)]
pub struct FakeVmFlags {
    pub running: Cell<bool>,
    /// `ssh_ready` polls (after launch) before SSH answers.
    pub boot_secs: Cell<u32>,
    pub polls: Cell<u32>,
    pub image: Cell<bool>,
    pub building: Cell<bool>,
    pub dies_on_boot: Cell<bool>,
    pub ignores_stop: Cell<bool>,
    pub launches: Cell<u32>,
    pub stops: Cell<u32>,
}

/// A virtual T1 driven by flags.
#[derive(Clone, Default)]
pub struct FakeVm(Rc<FakeVmFlags>);

impl Deref for FakeVm {
    type Target = FakeVmFlags;
    fn deref(&self) -> &FakeVmFlags {
        &self.0
    }
}

impl VmBackend for FakeVm {
    fn pid(&self) -> Option<u32> {
        self.running.get().then_some(4242)
    }
    fn ssh_ready(&self) -> bool {
        if !self.running.get() {
            return false;
        }
        if self.dies_on_boot.get() {
            self.running.set(false);
            return false;
        }
        let n = self.polls.get();
        self.polls.set(n + 1);
        n >= self.boot_secs.get()
    }
    fn info(&self) -> VmInfo {
        let pid = self.pid();
        VmInfo {
            id: format!("vm:{DEFAULT_NAME}"),
            name: DEFAULT_NAME.into(),
            state: self.state(),
            pid,
            endpoints: Endpoints {
                ssh: self.ssh_endpoint(),
                vnc: "127.0.0.1:5900".into(),
                qmp: "/os/build/run/qmp.sock".into(),
            },
            image: ImageInfo {
                path: self.image_path(),
                present: self.image.get(),
                bytes: self.image.get().then_some(1),
            },
            console_log: "/os/build/run/console.log".into(),
            runner_dir: "/vd".into(),
        }
    }
    fn ssh_endpoint(&self) -> String {
        "127.0.0.1:2222".into()
    }
    fn image_present(&self) -> bool {
        self.image.get()
    }
    fn image_path(&self) -> String {
        "/os/image.raw".into()
    }
    fn image_building(&self) -> bool {
        self.building.get()
    }
    fn image_hint(&self) -> String {
        "put an image in place".into()
    }
    fn start_command(&self) -> String {
        "/vd/bin/odap-vdev start".into()
    }
    fn stop_command(&self) -> String {
        "/vd/bin/odap-vdev stop".into()
    }
    fn launch(&self) -> Result<(), Error> {
        self.launches.set(self.launches.get() + 1);
        self.running.set(true);
        self.polls.set(0);
        Ok(())
    }
    fn request_stop(&self) -> Result<(), Error> {
        self.stops.set(self.stops.get() + 1);
        if !self.ignores_stop.get() {
            self.running.set(false);
        }
        Ok(())
    }
    fn emulator_log_tail(&self, _lines: usize) -> String {
        "emulator tail".into()
    }
    fn console_log_tail(&self, _lines: usize) -> String {
        "console tail".into()
    }
}

/// Hands out one [`FakeVm`], or fails like a host without blackbird-os.
pub struct FakeProvider {
    pub vm: FakeVm,
    pub missing: bool,
}

impl FakeProvider {
    fn with(running: bool, boot_secs: u32) -> Self {
        let vm = FakeVm::default();
        vm.image.set(true);
        vm.running.set(running);
        vm.boot_secs.set(boot_secs);
        FakeProvider { vm, missing: false }
    }
    pub fn stopped() -> Self {
        Self::with(false, 0)
    }
    pub fn running() -> Self {
        Self::with(true, 0)
    }
    pub fn booting() -> Self {
        Self::with(true, u32::MAX)
    }
    pub fn missing() -> Self {
        FakeProvider {
            missing: true,
            ..Self::stopped()
        }
    }
}

impl VmProvider for FakeProvider {
    type Vm = FakeVm;
    fn locate(&self) -> Result<FakeVm, Error> {
        if self.missing {
            Err(Error::new(
                &codes::WORKSPACE_NOT_FOUND,
                "no public/virtual-devices found above /",
            ))
        } else {
            Ok(self.vm.clone())
        }
    }
}

/// A device that understands `echo …`, `true`, `sh -c 'exit N'` and an
/// empty command (a shell); anything else fails. It records what it ran.
#[derive(Default)]
pub struct FakeTransport {
    pub ran: RefCell<Vec<Vec<String>>>,
    /// Make every run fail at the transport level.
    pub broken: bool,
}

impl FakeTransport {
    fn interpret(&self, argv: &[String]) -> (i32, String) {
        self.ran.borrow_mut().push(argv.to_vec());
        match argv.first().map(String::as_str) {
            Some("echo") => (0, format!("{}\n", argv[1..].join(" "))),
            // A device with the normal and debug boot modes only.
            Some("odap-reboot") => match argv.get(1).map(String::as_str) {
                Some("normal" | "debug") => (0, String::new()),
                _ => (2, String::new()),
            },
            Some("systemctl") if argv.get(1).map(String::as_str) == Some("poweroff") => {
                (0, String::new())
            }
            // `true`, or an interactive shell that the user leaves at once.
            Some("true") | None => (0, String::new()),
            Some("sh") if argv.get(1).map(String::as_str) == Some("-c") => {
                let code = argv
                    .get(2)
                    .and_then(|s| s.strip_prefix("exit "))
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(127);
                (code, String::new())
            }
            _ => (1, String::new()),
        }
    }
}

impl DeviceTransport for FakeTransport {
    fn ensure_available(&self) -> Result<(), Error> {
        Ok(())
    }
    fn describe(&self, target: &Transport, argv: &[String]) -> String {
        format!("fake exec on {target}: {}", argv.join(" "))
    }
    fn run_captured(&self, argv: &[String]) -> Result<Captured, Error> {
        if self.broken {
            return Ok(Captured {
                exit: Exit::TransportFailed,
                stdout: String::new(),
                stderr: "connection refused".into(),
            });
        }
        let (code, stdout) = self.interpret(argv);
        Ok(Captured {
            exit: Exit::Code(code),
            stdout,
            stderr: String::new(),
        })
    }
    fn run_attached(&self, argv: &[String]) -> Result<Exit, Error> {
        if self.broken {
            return Ok(Exit::TransportFailed);
        }
        Ok(Exit::Code(self.interpret(argv).0))
    }
}

/// A display that returns a fixed PNG header, or garbage.
pub struct FakeScreen {
    pub width: u32,
    pub height: u32,
    pub garbage: bool,
}

impl Default for FakeScreen {
    fn default() -> Self {
        FakeScreen {
            width: 320,
            height: 480,
            garbage: false,
        }
    }
}

impl ScreenSource for FakeScreen {
    fn describe(&self, target: &Transport, path: &str) -> String {
        format!("fake capture of {target} to {path}")
    }
    fn capture_png(&self, _path: &str) -> Result<Vec<u8>, Error> {
        if self.garbage {
            return Ok(b"definitely not a png image".to_vec());
        }
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&self.width.to_be_bytes());
        png.extend_from_slice(&self.height.to_be_bytes());
        Ok(png)
    }
}

/// Records every event it receives.
#[derive(Default)]
pub struct FakeInput {
    pub sent: RefCell<Vec<InputEvent>>,
}

impl InputSink for FakeInput {
    fn describe(&self, event: &InputEvent) -> String {
        format!("fake {event:?}")
    }
    fn send(&self, event: &InputEvent) -> Result<(), Error> {
        self.sent.borrow_mut().push(event.clone());
        Ok(())
    }
}

/// Hardware that does what it is told at once.
pub struct FakeBench {
    pub attached: RefCell<Vec<Attachment>>,
    pub battery: Cell<u8>,
    pub applied: RefCell<Vec<HardwareChange>>,
}

impl Default for FakeBench {
    fn default() -> Self {
        FakeBench {
            attached: RefCell::default(),
            battery: Cell::new(80),
            applied: RefCell::default(),
        }
    }
}

impl HardwareBench for FakeBench {
    fn describe(&self, change: &HardwareChange) -> String {
        format!("fake {change:?}")
    }
    fn hardware(&self, target: &Transport) -> Result<HardwareState, Error> {
        let attached = self.attached.borrow();
        Ok(HardwareState {
            target: target.clone(),
            attached: Attachment::ALL
                .into_iter()
                .filter(|a| attached.contains(a))
                .collect(),
            battery: Some(self.battery.get()),
        })
    }
    fn apply(&self, change: &HardwareChange) -> Result<(), Error> {
        self.applied.borrow_mut().push(*change);
        match *change {
            HardwareChange::Attach { attachment } => self.attached.borrow_mut().push(attachment),
            HardwareChange::Detach { attachment } => {
                self.attached.borrow_mut().retain(|a| *a != attachment)
            }
            HardwareChange::Battery { percent } => self.battery.set(percent),
        }
        Ok(())
    }
}

/// Reports fixed findings.
pub struct FakeProbe(pub Vec<Finding>);

impl FakeProbe {
    pub fn one(id: &str, status: Status) -> Self {
        let fix = (status == Status::Error).then(|| opendap_model::Fix::describe("fix it"));
        FakeProbe(vec![Finding {
            id: id.into(),
            status,
            title: id.into(),
            message: "message".into(),
            fix,
        }])
    }
}

impl HostProbe for FakeProbe {
    fn check(&self) -> Vec<Finding> {
        self.0.clone()
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::contract;

    #[test]
    fn fake_vm_passes_the_vm_backend_contract() {
        contract::vm_backend(&FakeProvider::stopped().vm);
        contract::vm_backend(&FakeProvider::running().vm);
    }

    #[test]
    fn fake_transport_passes_the_device_transport_contract() {
        contract::device_transport(&FakeTransport::default());
    }

    #[test]
    fn fake_screen_passes_the_screen_source_contract() {
        contract::screen_source(&FakeScreen::default(), "/tmp/contract.png");
    }

    #[test]
    fn fake_input_passes_the_input_sink_contract() {
        contract::input_sink(&FakeInput::default());
    }

    #[test]
    fn fake_probes_pass_the_host_probe_contract() {
        contract::host_probe(&FakeProbe::one("a.b", Status::Ok));
        contract::host_probe(&FakeProbe::one("a.c", Status::Error));
    }
}
