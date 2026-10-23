//! Ports: what the use cases need from the outside world. Adapter crates
//! implement them; [`crate::contract`] holds the suite every implementation
//! of a port passes.

use std::time::Duration;

use opendap_model::{Error, Finding, InputEvent, Transport, VmInfo, VmState};

/// Finds the virtual T1 backend on this host.
pub trait VmProvider {
    type Vm: VmBackend;

    /// Locate the backend. Fails when it is not installed (for example no
    /// blackbird-os checkout).
    fn locate(&self) -> Result<Self::Vm, Error>;
}

/// One virtual T1 instance and its emulator.
pub trait VmBackend {
    /// The emulator's process ID, when it runs.
    fn pid(&self) -> Option<u32>;

    /// Whether the guest's SSH server answers.
    fn ssh_ready(&self) -> bool;

    /// The lifecycle state, derived from [`Self::pid`] and [`Self::ssh_ready`].
    fn state(&self) -> VmState {
        crate::vm::state_of(self.pid(), || self.ssh_ready())
    }

    /// The full description of the instance, with its current state.
    fn info(&self) -> VmInfo;

    /// The SSH endpoint as `host:port`.
    fn ssh_endpoint(&self) -> String;

    /// Whether the disk image exists.
    fn image_present(&self) -> bool;

    /// The disk image path, for messages.
    fn image_path(&self) -> String;

    /// Whether an image build is running right now.
    fn image_build_running(&self) -> bool;

    /// The command that builds the image, for fixes.
    fn image_build_command(&self) -> String;

    /// The command [`Self::launch`] runs, for dry-run plans.
    fn start_command(&self) -> String;

    /// The command [`Self::request_stop`] runs, for dry-run plans.
    fn stop_command(&self) -> String;

    /// Start the emulator. Returns once it runs (the guest may still boot);
    /// fails with `vm.start_failed` when it does not come up.
    fn launch(&self) -> Result<(), Error>;

    /// Ask the emulator to stop. Returns without waiting.
    fn request_stop(&self) -> Result<(), Error>;

    /// The last lines of the emulator's log.
    fn emulator_log_tail(&self, lines: usize) -> String;

    /// The last lines of the guest's console.
    fn console_log_tail(&self, lines: usize) -> String;
}

/// How a remote command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// The command exited with this status.
    Code(i32),
    /// The command was killed by a signal.
    Signal,
    /// The transport itself failed (connection, authentication).
    TransportFailed,
}

/// A remote command run with its output captured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub exit: Exit,
    pub stdout: String,
    pub stderr: String,
}

/// Runs commands on a device. Each argument must reach the device unchanged.
pub trait DeviceTransport {
    /// Fail when the transport cannot work on this host (a missing program).
    fn ensure_available(&self) -> Result<(), Error>;

    /// What [`Self::run_attached`] would do, for dry-run plans. An empty
    /// `argv` means an interactive shell.
    fn describe(&self, target: &Transport, argv: &[String]) -> String;

    /// Run `argv` with no input and capture its output.
    fn run_captured(&self, argv: &[String]) -> Result<Captured, Error>;

    /// Run `argv` attached to the caller's terminal; an empty `argv` opens an
    /// interactive shell.
    fn run_attached(&self, argv: &[String]) -> Result<Exit, Error>;
}

/// Captures a device display.
pub trait ScreenSource {
    /// What [`Self::capture_png`] would do, for dry-run plans.
    fn describe(&self, target: &Transport, path: &str) -> String;

    /// Save the display as a PNG at `path` and return its bytes.
    fn capture_png(&self, path: &str) -> Result<Vec<u8>, Error>;
}

/// Sends input events to a device.
pub trait InputSink {
    /// What [`Self::send`] would do, for dry-run plans.
    fn describe(&self, event: &InputEvent) -> String;

    /// Deliver one event.
    fn send(&self, event: &InputEvent) -> Result<(), Error>;
}

/// One host check of `doctor`. A probe may report several findings.
pub trait HostProbe {
    fn check(&self) -> Vec<Finding>;
}

/// Monotonic time, so waits can be tested without sleeping.
pub trait Clock {
    /// Time since an arbitrary, fixed origin.
    fn now(&self) -> Duration;

    fn sleep(&self, d: Duration);
}

impl<T: DeviceTransport + ?Sized> DeviceTransport for &T {
    fn ensure_available(&self) -> Result<(), Error> {
        (**self).ensure_available()
    }
    fn describe(&self, target: &Transport, argv: &[String]) -> String {
        (**self).describe(target, argv)
    }
    fn run_captured(&self, argv: &[String]) -> Result<Captured, Error> {
        (**self).run_captured(argv)
    }
    fn run_attached(&self, argv: &[String]) -> Result<Exit, Error> {
        (**self).run_attached(argv)
    }
}
