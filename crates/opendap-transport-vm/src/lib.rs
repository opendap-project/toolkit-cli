//! Virtual device adapter of the `opendap` CLI: drives the virtual devices of a virtual-devices
//! checkout through its documented tool, `odap-vdev` (status as JSON, lifecycle, the management
//! link, screen and input), never through its files (RFC 0008). It manages one instance,
//! `vm:default`, and implements the core's [`VmProvider`], [`VmBackend`], [`ScreenSource`],
//! [`InputSink`] and [`HardwareBench`] ports, and the [`CheckoutProbe`] for `doctor`. For surfaces
//! that follow a hand in real time, [`live::LiveKeys`] sends the controls over a QMP session of
//! their own. POSIX hosts only.

pub mod live;
pub mod workspace;

use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use opendap_core::{HardwareBench, HostProbe, InputSink, ScreenSource, VmBackend, VmProvider};
use opendap_host::probes::finding;
use opendap_host::require_tool;
use opendap_model::error as codes;
use opendap_model::vm::DEFAULT_NAME;
use opendap_model::{
    Attachment, Endpoints, Error, Finding, Fix, HardwareChange, HardwareState, ImageInfo,
    InputEvent, Status, Transport, VmInfo,
};
use serde_json::Value;

/// The program that opens SSH sessions to the virtual device, for plans.
pub const SSH_LAUNCHER: &str = "odap-vdev";

/// How to get an image when there is none, without any toolkit (rfcs 0009). Released images, and
/// `opendap image pull` to fetch them, are not published yet.
pub const IMAGE_HINT: &str = "Put a Blackbird OS qemu image at ~/.local/share/opendap/images/blackbird-os-qemu.raw, or point ODAP_IMAGE at one.";

/// Last lines of a log file, for error details.
pub fn tail(path: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

/// Finds the virtual-devices checkout from `$ODAP_VIRTUAL_DEVICES_DIR` or the workspace.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalVms;

impl VmProvider for LocalVms {
    type Vm = VirtualDevice;
    fn locate(&self) -> Result<VirtualDevice, Error> {
        VirtualDevice::locate()
    }
}

/// The virtual device of one checkout, whatever the workspace says: for tools that ship inside
/// virtual-devices itself (its simulator window).
#[derive(Debug, Clone)]
pub struct Checkout(pub PathBuf);

impl VmProvider for Checkout {
    type Vm = VirtualDevice;
    fn locate(&self) -> Result<VirtualDevice, Error> {
        Ok(VirtualDevice::at(self.0.clone()))
    }
}

/// A virtual device, run by a virtual-devices checkout.
#[derive(Debug, Clone)]
pub struct VirtualDevice {
    pub vd_dir: PathBuf,
}

impl VirtualDevice {
    pub fn locate() -> Result<Self, Error> {
        Ok(VirtualDevice {
            vd_dir: workspace::vd_checkout()?,
        })
    }

    /// The device of the checkout at `vd_dir`.
    pub fn at(vd_dir: PathBuf) -> Self {
        VirtualDevice { vd_dir }
    }

    pub fn tool(&self) -> PathBuf {
        self.vd_dir.join("bin/odap-vdev")
    }

    fn run(&self, args: &[&str]) -> Result<Output, Error> {
        require_tool("python3", "runs odap-vdev")?;
        Command::new(self.tool())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| Error::new(&codes::HOST_IO, format!("cannot run odap-vdev: {e}")))
    }

    /// `odap-vdev status --json`, the documented state and endpoints.
    pub fn status(&self) -> Value {
        self.run(&["status", "--json"])
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| serde_json::from_slice(&o.stdout).ok())
            .unwrap_or(Value::Null)
    }

    fn status_str(st: &Value, path: &[&str]) -> String {
        let mut v = st;
        for p in path {
            v = &v[*p];
        }
        v.as_str().unwrap_or_default().to_string()
    }

    fn ssh_port(st: &Value) -> u16 {
        Self::status_str(st, &["endpoints", "ssh"])
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(2222)
    }

    /// `odap-vdev ssh --`, without the remote command. Feed it to `opendap_transport_ssh::SshExec`.
    pub fn ssh_command(&self) -> Command {
        let mut c = Command::new(self.tool());
        c.args(["ssh", "--"]);
        c
    }

    fn checked(&self, args: &[&str]) -> Result<String, Error> {
        let out = self.run(args)?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            let last = err.lines().last().unwrap_or("no output").trim().to_string();
            // odap-vdev exits 2 on a usage or data error: a role or pixel the device does not have.
            let code = if out.status.code() == Some(2) {
                &codes::INPUT_INVALID
            } else {
                &codes::TRANSPORT_QMP_FAILED
            };
            Err(Error::new(code, last).details(serde_json::json!({ "command": format!("odap-vdev {}", args.join(" ")), "stderr": err })))
        }
    }

    fn input_args(event: &InputEvent) -> Vec<String> {
        match event {
            InputEvent::Key { role, hold_ms } => {
                vec![
                    "input".into(),
                    "press".into(),
                    role.as_str().into(),
                    "--hold".into(),
                    hold_ms.to_string(),
                ]
            }
            InputEvent::KeyDown { role } => {
                vec![
                    "input".into(),
                    "key".into(),
                    role.as_str().into(),
                    "down".into(),
                ]
            }
            InputEvent::KeyUp { role } => {
                vec![
                    "input".into(),
                    "key".into(),
                    role.as_str().into(),
                    "up".into(),
                ]
            }
            InputEvent::Tap { x, y } => {
                vec!["input".into(), "tap".into(), x.to_string(), y.to_string()]
            }
        }
    }

    fn hardware_args(change: &HardwareChange) -> Vec<String> {
        let a: &[&str] = match *change {
            HardwareChange::Attach { attachment } => match attachment {
                Attachment::Microsd => &["sd", "insert"],
                Attachment::UsbStorage => &["usb", "plug", "storage"],
                Attachment::UsbAudio => &["usb", "plug", "audio"],
                Attachment::Charger => &["power", "charger", "on"],
            },
            HardwareChange::Detach { attachment } => match attachment {
                Attachment::Microsd => &["sd", "eject"],
                Attachment::UsbStorage => &["usb", "unplug", "usbdisk"],
                Attachment::UsbAudio => &["usb", "unplug", "usbaudio"],
                Attachment::Charger => &["power", "charger", "off"],
            },
            HardwareChange::Battery { percent } => {
                return vec!["power".into(), "battery".into(), percent.to_string()];
            }
        };
        a.iter().map(|s| s.to_string()).collect()
    }

    /// `odap-vdev hw --json` (`opendap.vdev-hardware/v1`) as the model has it.
    fn parse_hardware(target: &Transport, v: &Value) -> Result<HardwareState, Error> {
        if v["schema"] != "opendap.vdev-hardware/v1" {
            return Err(Error::new(
                &codes::TRANSPORT_QMP_FAILED,
                "odap-vdev hw: not an opendap.vdev-hardware/v1 document",
            ));
        }
        let usb: Vec<&str> = v["usb"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let attached = Attachment::ALL
            .into_iter()
            .filter(|a| match a {
                Attachment::Microsd => v["sd"].as_bool() == Some(true),
                Attachment::UsbStorage => usb.contains(&"usbdisk"),
                Attachment::UsbAudio => usb.contains(&"usbaudio"),
                Attachment::Charger => v["charger"].as_bool() == Some(true),
            })
            .collect();
        let battery = v["battery"]
            .as_u64()
            .and_then(|p| u8::try_from(p).ok())
            .filter(|p| *p <= 100);
        Ok(HardwareState {
            target: target.clone(),
            attached,
            battery,
        })
    }
}

impl VmBackend for VirtualDevice {
    fn pid(&self) -> Option<u32> {
        self.status()["pid"].as_u64().map(|p| p as u32)
    }

    /// Whether sshd in the guest answers with its banner. QEMU's user-mode network accepts the TCP
    /// connection before the guest is up, so a successful connect alone proves nothing.
    fn ssh_ready(&self) -> bool {
        let addr = SocketAddr::from(([127, 0, 0, 1], Self::ssh_port(&self.status())));
        let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_secs(1)) else {
            return false;
        };
        let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).is_ok() && &buf == b"SSH-"
    }

    fn info(&self) -> VmInfo {
        let st = self.status();
        let pid = st["pid"].as_u64().map(|p| p as u32);
        let state = opendap_core::vm::state_of(pid, || self.ssh_ready());
        let image = Self::status_str(&st, &["image", "path"]);
        VmInfo {
            id: format!("vm:{DEFAULT_NAME}"),
            name: DEFAULT_NAME.into(),
            state,
            pid,
            endpoints: Endpoints {
                ssh: Self::status_str(&st, &["endpoints", "ssh"]),
                vnc: Self::status_str(&st, &["endpoints", "vnc"]),
                qmp: Self::status_str(&st, &["endpoints", "qmp"]),
            },
            image: ImageInfo {
                bytes: std::fs::metadata(&image).ok().map(|m| m.len()),
                present: st["image"]["present"].as_bool().unwrap_or(false),
                path: image,
            },
            console_log: Self::status_str(&st, &["console_log"]),
            runner_dir: self.vd_dir.display().to_string(),
        }
    }

    fn ssh_endpoint(&self) -> String {
        Self::status_str(&self.status(), &["endpoints", "ssh"])
    }

    fn image_present(&self) -> bool {
        self.status()["image"]["present"].as_bool().unwrap_or(false)
    }

    fn image_path(&self) -> String {
        Self::status_str(&self.status(), &["image", "path"])
    }

    fn image_building(&self) -> bool {
        self.status()["image"]["building"]
            .as_bool()
            .unwrap_or(false)
    }

    fn image_hint(&self) -> String {
        IMAGE_HINT.into()
    }

    fn start_command(&self) -> String {
        format!("{} start", self.tool().display())
    }

    fn stop_command(&self) -> String {
        format!("{} stop", self.tool().display())
    }

    fn launch(&self) -> Result<(), Error> {
        require_tool("qemu-system-aarch64", "runs the virtual device")?;
        require_tool("qemu-img", "creates the virtual device's disk")?;
        let out = self.run(&["start"])?;
        if !out.status.success() || self.pid().is_none() {
            let st = self.status();
            return Err(
                Error::new(&codes::VM_START_FAILED, "odap-vdev start failed").details(
                    serde_json::json!({
                        "stderr": String::from_utf8_lossy(&out.stderr),
                        "qemu_log": tail(Path::new(&Self::status_str(&st, &["qemu_log"])), 20),
                    }),
                ),
            );
        }
        Ok(())
    }

    fn request_stop(&self) -> Result<(), Error> {
        self.run(&["stop"]).map(|_| ())
    }

    fn emulator_log_tail(&self, lines: usize) -> String {
        tail(
            Path::new(&Self::status_str(&self.status(), &["qemu_log"])),
            lines,
        )
    }

    fn console_log_tail(&self, lines: usize) -> String {
        tail(
            Path::new(&Self::status_str(&self.status(), &["console_log"])),
            lines,
        )
    }
}

impl ScreenSource for VirtualDevice {
    fn describe(&self, target: &Transport, path: &str) -> String {
        format!("odap-vdev screen capture of {target} to {path}")
    }

    fn capture_png(&self, path: &str) -> Result<Vec<u8>, Error> {
        self.checked(&["screen", "capture", path])?;
        std::fs::read(path)
            .map_err(|e| Error::new(&codes::HOST_IO, format!("cannot read {path}: {e}")))
    }
}

impl InputSink for VirtualDevice {
    fn describe(&self, event: &InputEvent) -> String {
        format!("odap-vdev {}", Self::input_args(event).join(" "))
    }

    fn send(&self, event: &InputEvent) -> Result<(), Error> {
        let args = Self::input_args(event);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.checked(&refs).map(|_| ())
    }
}

impl HardwareBench for VirtualDevice {
    fn describe(&self, change: &HardwareChange) -> String {
        format!("odap-vdev {}", Self::hardware_args(change).join(" "))
    }

    fn hardware(&self, target: &Transport) -> Result<HardwareState, Error> {
        let out = self.checked(&["hw", "--json"])?;
        let v: Value = serde_json::from_str(&out)
            .map_err(|e| Error::new(&codes::TRANSPORT_QMP_FAILED, format!("odap-vdev hw: {e}")))?;
        Self::parse_hardware(target, &v)
    }

    fn apply(&self, change: &HardwareChange) -> Result<(), Error> {
        let args = Self::hardware_args(change);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.checked(&refs).map(|_| ())
    }
}

/// `doctor` checks of the virtual device: `workspace.virtual_devices`, `auth.dev_key`, `vm.image`.
#[derive(Debug, Default, Clone, Copy)]
pub struct CheckoutProbe;

impl HostProbe for CheckoutProbe {
    fn check(&self) -> Vec<Finding> {
        match workspace::vd_checkout() {
            Ok(dir) => checkout_findings(&VirtualDevice { vd_dir: dir }),
            Err(e) => {
                let skip = |id: &str, title: &str| {
                    finding(id, title, Status::Skip, "no virtual-devices checkout", None)
                };
                vec![
                    finding(
                        "workspace.virtual_devices",
                        "virtual-devices checkout",
                        Status::Error,
                        e.message,
                        Some(Fix::describe(format!(
                            "Run from inside the OpenDAP workspace (scripts/bootstrap.sh), or set {}.",
                            workspace::VD_DIR_VAR
                        ))),
                    ),
                    skip("auth.dev_key", "Development SSH key"),
                    skip("vm.image", "Virtual device image"),
                ]
            }
        }
    }
}

fn checkout_findings(vm: &VirtualDevice) -> Vec<Finding> {
    let st = vm.status();
    let mut out = vec![finding(
        "workspace.virtual_devices",
        "virtual-devices checkout",
        Status::Ok,
        vm.vd_dir.display().to_string(),
        None,
    )];

    let key = PathBuf::from(VirtualDevice::status_str(&st, &["dev_key"]));
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
            format!("{} is missing", key.display()),
            Some(Fix::describe(
                "Point ODAP_DEV_KEY at a key the image trusts. Device pairing (opendap device pair, not built yet) will add yours to any image.",
            )),
        ),
    });

    let image = VirtualDevice::status_str(&st, &["image", "path"]);
    out.push(match std::fs::metadata(&image) {
        Ok(m) => finding(
            "vm.image",
            "Virtual device image",
            Status::Ok,
            format!("{image} ({:.1} GB)", m.len() as f64 / 1e9),
            None,
        ),
        Err(_) if st["image"]["building"].as_bool().unwrap_or(false) => finding(
            "vm.image",
            "Virtual device image",
            Status::Warn,
            format!("{image} is missing: it is being rebuilt"),
            Some(Fix::describe(
                "Wait for the build to finish, then run `opendap doctor` again.",
            )),
        ),
        Err(_) => finding(
            "vm.image",
            "Virtual device image",
            Status::Error,
            format!("{image} is missing"),
            Some(Fix::describe(IMAGE_HINT)),
        ),
    });
    out
}

#[cfg(test)]
mod tests {
    use opendap_core::contract;
    use opendap_model::{Role, VmState};

    use super::*;

    struct Tmp(PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A virtual-devices checkout whose odap-vdev reports a stopped device with no image.
    fn checkout(name: &str) -> (Tmp, VirtualDevice) {
        let p = std::env::temp_dir().join(format!("opendap-vm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("bin")).unwrap();
        let tool = p.join("bin/odap-vdev");
        std::fs::write(
            &tool,
            "#!/bin/sh\n[ \"$1 $2\" = \"status --json\" ] && echo '{\"state\":\"stopped\",\"pid\":null,\"endpoints\":{\"ssh\":\"127.0.0.1:2299\",\"vnc\":\"127.0.0.1:5999\",\"qmp\":\"/tmp/none.sock\"},\"image\":{\"path\":\"/nonexistent.raw\",\"present\":false},\"dev_key\":\"/nonexistent/key\",\"console_log\":\"/tmp/none.log\",\"qemu_log\":\"/tmp/none.log\"}'\n[ \"$1 $2\" = \"hw --json\" ] && echo '{\"schema\":\"opendap.vdev-hardware/v1\",\"sd\":true,\"usb\":[\"usbaudio\",\"other\"],\"charger\":false,\"battery\":64}'\nexit 0\n",
        )
        .unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        (
            Tmp(p.clone()),
            VirtualDevice {
                vd_dir: p.canonicalize().unwrap(),
            },
        )
    }

    #[test]
    fn passes_the_vm_backend_contract() {
        let (_t, vm) = checkout("contract");
        contract::vm_backend(&vm);
        assert_eq!(vm.state(), VmState::Stopped);
        assert!(!vm.image_present());
        assert_eq!(vm.ssh_endpoint(), "127.0.0.1:2299");
    }

    #[test]
    fn passes_the_input_sink_contract() {
        let (_t, vm) = checkout("input");
        contract::input_sink(&vm);
        let key = InputEvent::Key {
            role: Role::from("volume-up"),
            hold_ms: 100,
        };
        assert_eq!(
            InputSink::describe(&vm, &key),
            "odap-vdev input press volume-up --hold 100"
        );
        assert_eq!(
            InputSink::describe(&vm, &InputEvent::Tap { x: 1, y: 2 }),
            "odap-vdev input tap 1 2"
        );
    }

    #[test]
    fn passes_the_hardware_bench_contract() {
        let (_t, vm) = checkout("hardware");
        contract::hardware_bench(&vm);
        let st = vm
            .hardware(&Transport::Vm {
                name: DEFAULT_NAME.into(),
            })
            .unwrap();
        assert_eq!(st.attached, [Attachment::Microsd, Attachment::UsbAudio]);
        assert_eq!(st.battery, Some(64));
        let plug = HardwareChange::Attach {
            attachment: Attachment::UsbStorage,
        };
        assert_eq!(
            HardwareBench::describe(&vm, &plug),
            "odap-vdev usb plug storage"
        );
        assert_eq!(
            InputSink::describe(
                &vm,
                &InputEvent::KeyDown {
                    role: Role::from("main")
                }
            ),
            "odap-vdev input key main down"
        );
    }

    #[test]
    fn plans_name_the_tool_of_the_checkout() {
        let (_t, vm) = checkout("plans");
        let tool = vm.tool().display().to_string();
        assert_eq!(vm.start_command(), format!("{tool} start"));
        assert_eq!(vm.stop_command(), format!("{tool} stop"));
        assert_eq!(vm.image_hint(), IMAGE_HINT);
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
        assert_eq!(
            ids,
            ["workspace.virtual_devices", "auth.dev_key", "vm.image"]
        );
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
