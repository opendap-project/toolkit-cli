//! Virtual T1 adapter of the OpenDAP toolkit: wraps `build/dap-qemu.sh` and
//! `build/dap-qmp.py` from an blackbird-os checkout. It manages a single
//! instance, `vm:default`, and implements the core's [`VmProvider`],
//! [`VmBackend`], [`ScreenSource`] and [`InputSink`] ports, and the
//! [`CheckoutProbe`] for `doctor`. POSIX hosts only.

pub mod workspace;

use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use opendap_core::{HostProbe, InputSink, ScreenSource, VmBackend, VmProvider};
use opendap_host::probes::finding;
use opendap_host::require_tool;
use opendap_model::error as codes;
use opendap_model::vm::DEFAULT_NAME;
use opendap_model::{
    Endpoints, Error, Finding, Fix, ImageInfo, InputEvent, Status, Transport, VmInfo,
};

/// The program that opens SSH sessions to the virtual T1, for plans.
pub const SSH_LAUNCHER: &str = "dap-qemu.sh";

fn env_num<T: std::str::FromStr>(var: &str, default: T) -> T {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Last lines of a log file, for error details.
pub fn tail(path: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

/// Whether an image build is running (`dap-image.sh`).
pub fn image_build_running() -> bool {
    Command::new("pgrep")
        .args(["-f", "dap-image.sh"])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Finds the blackbird-os checkout from `$BLACKBIRD_OS_DIR` or the current
/// directory (see [`workspace`]).
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalVms;

impl VmProvider for LocalVms {
    type Vm = VirtualT1;
    fn locate(&self) -> Result<VirtualT1, Error> {
        VirtualT1::locate()
    }
}

/// The virtual T1 of one blackbird-os checkout.
#[derive(Debug, Clone)]
pub struct VirtualT1 {
    pub os_dir: PathBuf,
}

impl VirtualT1 {
    pub fn locate() -> Result<Self, Error> {
        Ok(VirtualT1 {
            os_dir: workspace::os_checkout()?,
        })
    }

    pub fn qemu_script(&self) -> PathBuf {
        self.os_dir.join("build/dap-qemu.sh")
    }
    pub fn qmp_script(&self) -> PathBuf {
        self.os_dir.join("build/dap-qmp.py")
    }
    pub fn image_script(&self) -> PathBuf {
        self.os_dir.join("build/dap-image.sh")
    }
    pub fn run_dir(&self) -> PathBuf {
        self.os_dir.join("build/run")
    }
    pub fn pid_file(&self) -> PathBuf {
        self.run_dir().join("qemu.pid")
    }
    pub fn qmp_socket(&self) -> PathBuf {
        self.run_dir().join("qmp.sock")
    }
    pub fn console_log(&self) -> PathBuf {
        self.run_dir().join("console.log")
    }
    pub fn qemu_log(&self) -> PathBuf {
        self.run_dir().join("qemu.log")
    }
    pub fn image(&self) -> PathBuf {
        self.os_dir.join("image/mkosi.output/blackbird-os-qemu.raw")
    }
    pub fn dev_key(&self) -> PathBuf {
        self.os_dir.join("build/dev-ssh/id_ed25519")
    }
    pub fn ssh_port(&self) -> u16 {
        env_num("DAP_SSH_PORT", 2222)
    }
    pub fn vnc_port(&self) -> u16 {
        5900 + env_num::<u16>("DAP_VNC_DISPLAY", 0)
    }

    /// `sh build/dap-qemu.sh <args…>` with stdin closed and output captured.
    pub fn run_qemu_script(&self, args: &[&str]) -> Result<Output, Error> {
        Command::new("sh")
            .arg(self.qemu_script())
            .args(args)
            .current_dir(&self.os_dir)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| Error::new(&codes::HOST_IO, format!("cannot run dap-qemu.sh: {e}")))
    }

    /// The `dap-qemu.sh ssh` command line, without the remote command. Feed it
    /// to `opendap_transport_ssh::SshExec`.
    pub fn ssh_command(&self) -> Command {
        let mut c = Command::new("sh");
        c.arg(self.qemu_script())
            .arg("ssh")
            .current_dir(&self.os_dir);
        c
    }

    /// `python3 build/dap-qmp.py <args…>`; its stdout on success.
    pub fn qmp(&self, args: &[String]) -> Result<String, Error> {
        let python = require_tool("python3", "drives QMP through dap-qmp.py")?;
        let out = Command::new(python)
            .arg(self.qmp_script())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| Error::new(&codes::HOST_IO, format!("cannot run dap-qmp.py: {e}")))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            let last = err.lines().last().unwrap_or("no output").trim().to_string();
            Err(Error::new(
                &codes::TRANSPORT_QMP_FAILED,
                format!("dap-qmp.py {}: {last}", args[0]),
            )
            .details(serde_json::json!({ "stderr": err })))
        }
    }

    fn qmp_args(event: &InputEvent) -> Vec<String> {
        match *event {
            InputEvent::Key { button, hold_ms } => {
                vec!["press".into(), button.as_str().into(), hold_ms.to_string()]
            }
            InputEvent::Tap { x, y } => vec!["tap".into(), x.to_string(), y.to_string()],
        }
    }
}

impl VmBackend for VirtualT1 {
    /// The QEMU PID, when the PID file names a live QEMU process.
    fn pid(&self) -> Option<u32> {
        let pid: u32 = std::fs::read_to_string(self.pid_file())
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let out = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output()
            .ok()?;
        let comm = String::from_utf8_lossy(&out.stdout);
        (out.status.success() && comm.contains("qemu")).then_some(pid)
    }

    /// Whether sshd in the guest answers with its banner. QEMU's user-mode
    /// network accepts the TCP connection before the guest is up, so a
    /// successful connect alone proves nothing.
    fn ssh_ready(&self) -> bool {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.ssh_port()));
        let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_secs(1)) else {
            return false;
        };
        let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).is_ok() && &buf == b"SSH-"
    }

    fn info(&self) -> VmInfo {
        let pid = self.pid();
        let state = opendap_core::vm::state_of(pid, || self.ssh_ready());
        let image = self.image();
        let meta = std::fs::metadata(&image).ok();
        VmInfo {
            id: format!("vm:{DEFAULT_NAME}"),
            name: DEFAULT_NAME.into(),
            state,
            pid,
            endpoints: Endpoints {
                ssh: self.ssh_endpoint(),
                vnc: format!("127.0.0.1:{}", self.vnc_port()),
                qmp: self.qmp_socket().display().to_string(),
            },
            image: ImageInfo {
                path: image.display().to_string(),
                present: meta.is_some(),
                bytes: meta.map(|m| m.len()),
            },
            console_log: self.console_log().display().to_string(),
            os_dir: self.os_dir.display().to_string(),
        }
    }

    fn ssh_endpoint(&self) -> String {
        format!("127.0.0.1:{}", self.ssh_port())
    }

    fn image_present(&self) -> bool {
        self.image().is_file()
    }

    fn image_path(&self) -> String {
        self.image().display().to_string()
    }

    fn image_build_running(&self) -> bool {
        image_build_running()
    }

    fn image_build_command(&self) -> String {
        format!("sh {} qemu", self.image_script().display())
    }

    fn start_command(&self) -> String {
        format!("sh {} start", self.qemu_script().display())
    }

    fn stop_command(&self) -> String {
        format!("sh {} stop", self.qemu_script().display())
    }

    fn launch(&self) -> Result<(), Error> {
        require_tool("qemu-system-aarch64", "runs the virtual T1")?;
        require_tool("qemu-img", "creates the virtual T1 disk")?;
        let out = self.run_qemu_script(&["start"])?;
        if !out.status.success() || self.pid().is_none() {
            return Err(
                Error::new(&codes::VM_START_FAILED, "dap-qemu.sh start failed").details(
                    serde_json::json!({
                        "stderr": String::from_utf8_lossy(&out.stderr),
                        "qemu_log": tail(&self.qemu_log(), 20),
                    }),
                ),
            );
        }
        Ok(())
    }

    fn request_stop(&self) -> Result<(), Error> {
        self.run_qemu_script(&["stop"]).map(|_| ())
    }

    fn emulator_log_tail(&self, lines: usize) -> String {
        tail(&self.qemu_log(), lines)
    }

    fn console_log_tail(&self, lines: usize) -> String {
        tail(&self.console_log(), lines)
    }
}

impl ScreenSource for VirtualT1 {
    fn describe(&self, target: &Transport, path: &str) -> String {
        format!("QMP screendump of {target} to {path}")
    }

    fn capture_png(&self, path: &str) -> Result<Vec<u8>, Error> {
        self.qmp(&["screenshot".into(), path.to_string()])?;
        std::fs::read(path)
            .map_err(|e| Error::new(&codes::HOST_IO, format!("cannot read {path}: {e}")))
    }
}

impl InputSink for VirtualT1 {
    fn describe(&self, event: &InputEvent) -> String {
        format!("dap-qmp.py {}", Self::qmp_args(event).join(" "))
    }

    fn send(&self, event: &InputEvent) -> Result<(), Error> {
        self.qmp(&Self::qmp_args(event)).map(|_| ())
    }
}

/// `doctor` checks of the blackbird-os checkout: `workspace.blackbird_os`,
/// `auth.dev_key` and `vm.image`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CheckoutProbe;

impl HostProbe for CheckoutProbe {
    fn check(&self) -> Vec<Finding> {
        match workspace::os_checkout() {
            Ok(dir) => checkout_findings(&VirtualT1 { os_dir: dir }),
            Err(e) => {
                let skip = |id: &str, title: &str| {
                    finding(id, title, Status::Skip, "no blackbird-os checkout", None)
                };
                vec![
                    finding(
                        "workspace.blackbird_os",
                        "blackbird-os checkout",
                        Status::Error,
                        e.message,
                        Some(Fix::describe(format!(
                            "Run from inside the OpenDAP workspace (scripts/bootstrap.sh), or set {}.",
                            workspace::OS_DIR_VAR
                        ))),
                    ),
                    skip("auth.dev_key", "Development SSH key"),
                    skip("vm.image", "Virtual T1 image"),
                ]
            }
        }
    }
}

fn checkout_findings(vm: &VirtualT1) -> Vec<Finding> {
    let os_dir = vm.os_dir.display().to_string();
    let build_cmd = vm.image_build_command();
    let mut out = vec![finding(
        "workspace.blackbird_os",
        "blackbird-os checkout",
        Status::Ok,
        os_dir,
        None,
    )];

    let key = vm.dev_key();
    out.push(match std::fs::metadata(&key) {
        Ok(m) if m.permissions().mode() & 0o077 == 0 => finding(
            "auth.dev_key",
            "Development SSH key",
            Status::Ok,
            key.display().to_string(),
            None,
        ),
        Ok(m) => finding(
            "auth.dev_key",
            "Development SSH key",
            Status::Warn,
            format!(
                "{} has mode {:o}; ssh refuses keys readable by others",
                key.display(),
                m.permissions().mode() & 0o777
            ),
            Some(Fix::command(format!("chmod 600 {}", key.display()))),
        ),
        Err(_) => finding(
            "auth.dev_key",
            "Development SSH key",
            Status::Error,
            format!(
                "{} is missing; dap-image.sh creates it and installs it in the image",
                key.display()
            ),
            Some(Fix::command(build_cmd.clone())),
        ),
    });

    let image = vm.image();
    out.push(match std::fs::metadata(&image) {
        Ok(m) => finding(
            "vm.image",
            "Virtual T1 image",
            Status::Ok,
            format!("{} ({:.1} GB)", image.display(), m.len() as f64 / 1e9),
            None,
        ),
        Err(_) if image_build_running() => finding(
            "vm.image",
            "Virtual T1 image",
            Status::Warn,
            format!(
                "{} is missing: an image build is running (dap-image.sh)",
                image.display()
            ),
            Some(Fix::describe(
                "Wait for the build to finish, then run `opendap doctor` again.",
            )),
        ),
        Err(_) => finding(
            "vm.image",
            "Virtual T1 image",
            Status::Error,
            format!("{} is missing", image.display()),
            Some(Fix::command(build_cmd)),
        ),
    });
    out
}

#[cfg(test)]
mod tests {
    use opendap_core::contract;
    use opendap_model::{Button, VmState};

    use super::*;

    struct Tmp(PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// An blackbird-os checkout with no image and nothing running.
    fn checkout(name: &str) -> (Tmp, VirtualT1) {
        let p = std::env::temp_dir().join(format!("opendap-vm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("build")).unwrap();
        std::fs::write(p.join("build/dap-qemu.sh"), "#!/bin/sh\n").unwrap();
        let vm = VirtualT1 {
            os_dir: p.canonicalize().unwrap(),
        };
        (Tmp(p), vm)
    }

    #[test]
    fn passes_the_vm_backend_contract() {
        let (_t, vm) = checkout("contract");
        contract::vm_backend(&vm);
        assert_eq!(vm.state(), VmState::Stopped);
        assert!(!vm.image_present());
    }

    #[test]
    fn passes_the_input_sink_contract() {
        let (_t, vm) = checkout("input");
        contract::input_sink(&vm);
        let key = InputEvent::Key {
            button: Button::VolUp,
            hold_ms: 100,
        };
        assert_eq!(
            InputSink::describe(&vm, &key),
            "dap-qmp.py press vol-up 100"
        );
        assert_eq!(
            InputSink::describe(&vm, &InputEvent::Tap { x: 1, y: 2 }),
            "dap-qmp.py tap 1 2"
        );
    }

    #[test]
    fn plans_name_the_scripts_of_the_checkout() {
        let (_t, vm) = checkout("plans");
        let os = vm.os_dir.display().to_string();
        assert_eq!(
            vm.start_command(),
            format!("sh {os}/build/dap-qemu.sh start")
        );
        assert_eq!(vm.stop_command(), format!("sh {os}/build/dap-qemu.sh stop"));
        assert_eq!(
            vm.image_build_command(),
            format!("sh {os}/build/dap-image.sh qemu")
        );
    }

    #[test]
    fn checkout_findings_pass_the_host_probe_contract() {
        let (_t, vm) = checkout("probe");
        struct Fixed(Vec<Finding>);
        impl HostProbe for Fixed {
            fn check(&self) -> Vec<Finding> {
                self.0.clone()
            }
        }
        let findings = checkout_findings(&vm);
        let ids: Vec<_> = findings.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["workspace.blackbird_os", "auth.dev_key", "vm.image"]);
        assert_eq!(findings[1].status, Status::Error);
        contract::host_probe(&Fixed(findings));
        contract::host_probe(&CheckoutProbe);
    }

    #[test]
    fn tails_the_last_lines() {
        let (t, _vm) = checkout("tail");
        let f = t.0.join("log");
        std::fs::write(&f, "a\nb\nc\n").unwrap();
        assert_eq!(tail(&f, 2), "b\nc");
        assert_eq!(tail(&t.0.join("missing"), 2), "");
    }
}
