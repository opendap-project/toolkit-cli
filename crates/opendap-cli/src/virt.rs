//! The virtual T1 backend: wraps `build/dap-qemu.sh` and `build/dap-qmp.py`
//! from the opendap-os checkout. It manages a single instance, `vm:default`.

use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use opendap_cli_kit::CliError;
use opendap_cli_kit::workspace;
use opendap_model::error as codes;
use serde::Serialize;

/// The only instance `dap-qemu.sh` manages.
pub const DEFAULT_NAME: &str = "default";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VmState {
    Stopped,
    /// QEMU runs, SSH does not answer yet.
    Booting,
    /// SSH answers.
    Running,
}

#[derive(Debug, Clone, Serialize)]
pub struct Endpoints {
    pub ssh: String,
    pub vnc: String,
    pub qmp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageInfo {
    pub path: String,
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// One virtual T1 instance (`opendap.vm/v1`).
#[derive(Debug, Clone, Serialize)]
pub struct VmInfo {
    pub id: String,
    pub name: String,
    pub state: VmState,
    pub pid: Option<u32>,
    pub endpoints: Endpoints,
    pub image: ImageInfo,
    pub console_log: String,
    pub os_dir: String,
}

#[derive(Debug, Clone)]
pub struct VirtualT1 {
    pub os_dir: PathBuf,
}

fn env_num<T: std::str::FromStr>(var: &str, default: T) -> T {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Check a VM name; only `default` exists.
pub fn check_name(name: &str) -> Result<(), CliError> {
    if name == DEFAULT_NAME {
        Ok(())
    } else {
        Err(CliError::new(
            &codes::VM_NOT_FOUND,
            format!("no virtual T1 named `{name}`"),
        )
        .fix("opendap vm list"))
    }
}

/// Find a program on PATH.
pub fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(program))
        .find(|p| p.is_file())
}

pub fn require_tool(program: &str, purpose: &str) -> Result<PathBuf, CliError> {
    which(program).ok_or_else(|| {
        CliError::new(
            &codes::HOST_TOOL_MISSING,
            format!("`{program}` is not on PATH ({purpose})"),
        )
        .fix("opendap doctor")
    })
}

impl VirtualT1 {
    pub fn locate() -> Result<Self, CliError> {
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
        self.os_dir.join("image/mkosi.output/opendap-os-qemu.raw")
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

    /// The QEMU PID, when the PID file names a live QEMU process.
    pub fn pid(&self) -> Option<u32> {
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
    pub fn ssh_ready(&self) -> bool {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.ssh_port()));
        let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_secs(1)) else {
            return false;
        };
        let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).is_ok() && &buf == b"SSH-"
    }

    pub fn state(&self) -> VmState {
        self.state_for(self.pid())
    }

    fn state_for(&self, pid: Option<u32>) -> VmState {
        match pid {
            None => VmState::Stopped,
            Some(_) if self.ssh_ready() => VmState::Running,
            Some(_) => VmState::Booting,
        }
    }

    pub fn info(&self) -> VmInfo {
        let pid = self.pid();
        let state = self.state_for(pid);
        let image = self.image();
        let meta = std::fs::metadata(&image).ok();
        VmInfo {
            id: format!("vm:{DEFAULT_NAME}"),
            name: DEFAULT_NAME.into(),
            state,
            pid,
            endpoints: Endpoints {
                ssh: format!("127.0.0.1:{}", self.ssh_port()),
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

    /// `sh build/dap-qemu.sh <args…>` with stdin closed and output captured.
    pub fn run_qemu_script(&self, args: &[&str]) -> Result<Output, CliError> {
        Command::new("sh")
            .arg(self.qemu_script())
            .args(args)
            .current_dir(&self.os_dir)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| CliError::new(&codes::HOST_IO, format!("cannot run dap-qemu.sh: {e}")))
    }

    /// The `dap-qemu.sh ssh` command line, for exec.
    pub fn ssh_command(&self, remote: Option<String>) -> Command {
        let mut c = Command::new("sh");
        c.arg(self.qemu_script())
            .arg("ssh")
            .current_dir(&self.os_dir);
        if let Some(r) = remote {
            c.arg(r);
        }
        c
    }

    /// `python3 build/dap-qmp.py <args…>`.
    pub fn qmp(&self, args: &[String]) -> Result<String, CliError> {
        let python = require_tool("python3", "drives QMP through dap-qmp.py")?;
        let out = Command::new(python)
            .arg(self.qmp_script())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| CliError::new(&codes::HOST_IO, format!("cannot run dap-qmp.py: {e}")))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            let last = err.lines().last().unwrap_or("no output").trim().to_string();
            Err(CliError::new(
                &codes::TRANSPORT_QMP_FAILED,
                format!("dap-qmp.py {}: {last}", args[0]),
            )
            .details(serde_json::json!({ "stderr": err })))
        }
    }

    /// Whether an image build is running (`dap-image.sh`).
    pub fn image_build_running() -> bool {
        Command::new("pgrep")
            .args(["-f", "dap-image.sh"])
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// Last lines of a log file, for error details.
pub fn tail(path: &Path, lines: usize) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

/// Quote one argument for the remote POSIX shell, so argv reaches the device unchanged.
pub fn shell_quote(arg: &str) -> String {
    // An argument starting with '-' is always quoted, so the remote command can never start with
    // something ssh could read as an option (dap-qemu.sh also ends ssh options with `--`).
    let safe = !arg.is_empty()
        && !arg.starts_with('-')
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c));
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', r"'\''"))
    }
}

/// Width and height from a PNG header.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != SIG || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_preserves_argv() {
        assert_eq!(shell_quote("uname"), "uname");
        assert_eq!(shell_quote("-a"), "'-a'");
        assert_eq!(shell_quote("ls | wc"), "'ls | wc'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("-oProxyCommand=x"), "'-oProxyCommand=x'");
    }

    #[test]
    fn reads_png_size() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&320u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(png_size(&png), Some((320, 480)));
        assert_eq!(png_size(b"not a png at all, really no"), None);
    }

    #[test]
    fn only_the_default_instance_exists() {
        assert!(check_name("default").is_ok());
        assert_eq!(check_name("other").unwrap_err().code.code, "vm.not_found");
    }
}
