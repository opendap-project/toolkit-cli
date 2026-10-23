//! `doctor` probes for the host: platform, programs, QEMU, Lima, pre-commit.

use std::path::Path;

use opendap_core::HostProbe;
use opendap_model::{Finding, Fix, Status};

use crate::{first_line, which};

/// A finding with its fields in the usual order.
pub fn finding(
    id: &str,
    title: &str,
    status: Status,
    message: impl Into<String>,
    fix: Option<Fix>,
) -> Finding {
    Finding {
        id: id.into(),
        status,
        title: title.into(),
        message: message.into(),
        fix,
    }
}

fn uname(flag: &str) -> String {
    first_line(Path::new("uname"), &[flag]).unwrap_or_else(|| "unknown".into())
}

/// `host.platform`: macOS is tested, Linux is supported, the rest is not.
#[derive(Debug, Default, Clone, Copy)]
pub struct HostPlatform;

impl HostProbe for HostPlatform {
    fn check(&self) -> Vec<Finding> {
        let (os, arch) = (uname("-s"), uname("-m"));
        let title = "Host platform";
        vec![match os.as_str() {
            "Darwin" => {
                let ver =
                    first_line(Path::new("sw_vers"), &["-productVersion"]).unwrap_or_default();
                finding(
                    "host.platform",
                    title,
                    Status::Ok,
                    format!("macOS {ver} {arch}"),
                    None,
                )
            }
            "Linux" => finding(
                "host.platform",
                title,
                Status::Warn,
                format!("Linux {arch}: supported, but the toolkit is tested on macOS only"),
                Some(Fix::describe(
                    "Report problems with the output of `opendap doctor --json`.",
                )),
            ),
            other => finding(
                "host.platform",
                title,
                Status::Error,
                format!(
                    "{other} is not a supported host; the toolkit supports UNIX / POSIX systems"
                ),
                Some(Fix::describe("Use macOS or Linux.")),
            ),
        }]
    }
}

/// `host.tools`: `ssh` and `python3`.
#[derive(Debug, Default, Clone, Copy)]
pub struct HostTools;

impl HostProbe for HostTools {
    fn check(&self) -> Vec<Finding> {
        let missing: Vec<&str> = ["ssh", "python3"]
            .into_iter()
            .filter(|t| which(t).is_none())
            .collect();
        vec![if missing.is_empty() {
            finding(
                "host.tools",
                "ssh and python3",
                Status::Ok,
                "both on PATH (exec; screen and input over QMP)",
                None,
            )
        } else {
            finding(
                "host.tools",
                "ssh and python3",
                Status::Error,
                format!("not on PATH: {}", missing.join(", ")),
                Some(Fix::describe("Install the missing programs.")),
            )
        }]
    }
}

/// `vm.qemu`: QEMU, its EDK2 firmware and hardware acceleration.
#[derive(Debug, Default, Clone, Copy)]
pub struct Qemu;

impl HostProbe for Qemu {
    fn check(&self) -> Vec<Finding> {
        let (id, title) = ("vm.qemu", "QEMU");
        let Some(qemu) = which("qemu-system-aarch64") else {
            return vec![finding(
                id,
                title,
                Status::Error,
                "qemu-system-aarch64 is not on PATH",
                Some(Fix {
                    description: Some("Install QEMU".into()),
                    command: Some("brew install qemu".into()),
                }),
            )];
        };
        let version = first_line(&qemu, &["--version"]).unwrap_or_else(|| "version unknown".into());
        let mut problems = Vec::new();
        if which("qemu-img").is_none() {
            problems.push("qemu-img is missing".to_string());
        }
        // dap-qemu.sh boots the virtual T1 with the EDK2 firmware shipped with QEMU.
        let firmware = qemu
            .parent()
            .and_then(Path::parent)
            .map(|p| p.join("share/qemu/edk2-aarch64-code.fd"));
        if !firmware.as_deref().is_some_and(Path::is_file) {
            problems.push("edk2-aarch64-code.fd firmware not found next to QEMU".into());
        }
        let accel = if uname("-s") == "Darwin" {
            match first_line(Path::new("sysctl"), &["-n", "kern.hv_support"]).as_deref() {
                Some("1") => "HVF available",
                _ => {
                    problems.push("Hypervisor.framework (HVF) is not available".into());
                    "HVF unavailable"
                }
            }
        } else if Path::new("/dev/kvm").exists() {
            "KVM available"
        } else {
            problems.push("/dev/kvm is missing".into());
            "KVM unavailable"
        };
        vec![if problems.is_empty() {
            finding(id, title, Status::Ok, format!("{version}; {accel}"), None)
        } else {
            finding(
                id,
                title,
                Status::Error,
                format!("{version}; {}", problems.join("; ")),
                Some(Fix {
                    description: Some("Reinstall QEMU with its firmware".into()),
                    command: Some("brew reinstall qemu".into()),
                }),
            )
        }]
    }
}

/// `vm.lima`: Lima and the image build VM. Only needed to build images.
#[derive(Debug, Default, Clone, Copy)]
pub struct Lima;

impl HostProbe for Lima {
    fn check(&self) -> Vec<Finding> {
        let (id, title) = ("vm.lima", "Lima (image build VM)");
        let Some(lima) = which("limactl") else {
            return vec![finding(
                id,
                title,
                Status::Warn,
                "limactl is not on PATH; it is needed only to build Blackbird OS images",
                Some(Fix {
                    description: Some("Install Lima".into()),
                    command: Some("brew install lima".into()),
                }),
            )];
        };
        let version = first_line(&lima, &["--version"]).unwrap_or_else(|| "version unknown".into());
        let instance = first_line(
            &lima,
            &["list", "--format", "{{.Status}}", "blackbird-os-build"],
        )
        .filter(|s| !s.is_empty())
        .map(|s| format!("build VM blackbird-os-build: {}", s.to_lowercase()))
        .unwrap_or_else(|| {
            "build VM blackbird-os-build not created yet (dap-image.sh creates it)".into()
        });
        vec![finding(
            id,
            title,
            Status::Ok,
            format!("{version}; {instance}"),
            None,
        )]
    }
}

/// `tools.pre_commit`: every repository runs pre-commit.
#[derive(Debug, Default, Clone, Copy)]
pub struct PreCommit;

impl HostProbe for PreCommit {
    fn check(&self) -> Vec<Finding> {
        let (id, title) = ("tools.pre_commit", "pre-commit");
        vec![
            match which("pre-commit").and_then(|p| first_line(&p, &["--version"])) {
                Some(v) => finding(id, title, Status::Ok, v, None),
                None => finding(
                    id,
                    title,
                    Status::Error,
                    "pre-commit is not on PATH; every repository runs it",
                    Some(Fix {
                        description: Some("Install the host tools".into()),
                        command: Some("public/toolkit/host/setup.sh".into()),
                    }),
                ),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use opendap_core::contract;

    use super::*;

    #[test]
    fn every_host_probe_passes_the_contract() {
        let probes: [&dyn HostProbe; 5] = [&HostPlatform, &HostTools, &Qemu, &Lima, &PreCommit];
        for p in probes {
            contract::host_probe(p);
        }
    }

    #[test]
    fn each_probe_reports_its_own_id() {
        assert_eq!(HostPlatform.check()[0].id, "host.platform");
        assert_eq!(HostTools.check()[0].id, "host.tools");
        assert_eq!(Qemu.check()[0].id, "vm.qemu");
        assert_eq!(Lima.check()[0].id, "vm.lima");
        assert_eq!(PreCommit.check()[0].id, "tools.pre_commit");
    }
}
