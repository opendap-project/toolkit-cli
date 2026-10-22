//! `opendap doctor`: host checks, each with a status and a fix.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use opendap_cli_kit::CliError;
use opendap_cli_kit::workspace;
use opendap_model::{DoctorReport, Finding, Fix, Status};

use super::Ctx;
use crate::virt::{VirtualT1, which};

fn finding(
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

/// First line of `<program> <args…>` stdout, when it runs.
fn first_line(program: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().next().unwrap_or("").trim().to_string())
}

fn uname(flag: &str) -> String {
    first_line(Path::new("uname"), &[flag]).unwrap_or_else(|| "unknown".into())
}

fn host_platform() -> Finding {
    let (os, arch) = (uname("-s"), uname("-m"));
    let title = "Host platform";
    match os.as_str() {
        "Darwin" => {
            let ver = first_line(Path::new("sw_vers"), &["-productVersion"]).unwrap_or_default();
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
            format!("{other} is not a supported host; the toolkit supports UNIX / POSIX systems"),
            Some(Fix::describe("Use macOS or Linux.")),
        ),
    }
}

fn vm_qemu() -> Finding {
    let (id, title) = ("vm.qemu", "QEMU");
    let Some(qemu) = which("qemu-system-aarch64") else {
        return finding(
            id,
            title,
            Status::Error,
            "qemu-system-aarch64 is not on PATH",
            Some(Fix {
                description: Some("Install QEMU".into()),
                command: Some("brew install qemu".into()),
            }),
        );
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
    if problems.is_empty() {
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
    }
}

fn vm_lima() -> Finding {
    let (id, title) = ("vm.lima", "Lima (image build VM)");
    let Some(lima) = which("limactl") else {
        return finding(
            id,
            title,
            Status::Warn,
            "limactl is not on PATH; it is needed only to build OpenDAP OS images",
            Some(Fix {
                description: Some("Install Lima".into()),
                command: Some("brew install lima".into()),
            }),
        );
    };
    let version = first_line(&lima, &["--version"]).unwrap_or_else(|| "version unknown".into());
    let instance = first_line(
        &lima,
        &["list", "--format", "{{.Status}}", "opendap-os-build"],
    )
    .filter(|s| !s.is_empty())
    .map(|s| format!("build VM opendap-os-build: {}", s.to_lowercase()))
    .unwrap_or_else(|| {
        "build VM opendap-os-build not created yet (dap-image.sh creates it)".into()
    });
    finding(
        id,
        title,
        Status::Ok,
        format!("{version}; {instance}"),
        None,
    )
}

fn pre_commit() -> Finding {
    let (id, title) = ("tools.pre_commit", "pre-commit");
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
    }
}

fn host_tools() -> Finding {
    let missing: Vec<&str> = ["ssh", "python3"]
        .into_iter()
        .filter(|t| which(t).is_none())
        .collect();
    if missing.is_empty() {
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
    }
}

fn checkout_checks() -> Vec<Finding> {
    let vm = match workspace::os_checkout() {
        Ok(dir) => VirtualT1 { os_dir: dir },
        Err(e) => {
            let skip = |id: &str, title: &str| {
                finding(id, title, Status::Skip, "no opendap-os checkout", None)
            };
            return vec![
                finding(
                    "workspace.opendap_os",
                    "opendap-os checkout",
                    Status::Error,
                    e.message,
                    Some(Fix::describe(format!(
                        "Run from inside the OpenDAP workspace (scripts/bootstrap.sh), or set {}.",
                        workspace::OS_DIR_VAR
                    ))),
                ),
                skip("auth.dev_key", "Development SSH key"),
                skip("vm.image", "Virtual T1 image"),
            ];
        }
    };
    let os_dir = vm.os_dir.display().to_string();
    let build_cmd = format!("sh {}/build/dap-image.sh qemu", os_dir);
    let mut out = vec![finding(
        "workspace.opendap_os",
        "opendap-os checkout",
        Status::Ok,
        os_dir.clone(),
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
        Err(_) if VirtualT1::image_build_running() => finding(
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

fn mark(s: Status) -> &'static str {
    match s {
        Status::Ok => "[✓]",
        Status::Warn => "[!]",
        Status::Error => "[✗]",
        Status::Skip => "[-]",
    }
}

fn human(r: &DoctorReport) -> String {
    let mut s = String::new();
    for c in &r.checks {
        s.push_str(&format!("{} {}: {}\n", mark(c.status), c.title, c.message));
        if let Some(f) = c.fix.as_ref().filter(|_| c.status != Status::Ok) {
            if let Some(d) = &f.description {
                s.push_str(&format!("    fix: {d}\n"));
            }
            if let Some(cmd) = &f.command {
                s.push_str(&format!("    run: {cmd}\n"));
            }
        }
    }
    let m = &r.summary;
    s.push_str(&format!(
        "\n{} ok, {} warning(s), {} error(s), {} skipped\n",
        m.ok, m.warn, m.error, m.skip
    ));
    s
}

pub fn run(ctx: &Ctx) -> Result<u8, CliError> {
    let mut checks = vec![host_platform(), host_tools(), vm_qemu(), vm_lima()];
    checks.extend(checkout_checks());
    checks.push(pre_commit());
    let report = DoctorReport::new(checks);
    ctx.out.emit(&report, || human(&report));
    Ok(if report.ok { 0 } else { 1 })
}
