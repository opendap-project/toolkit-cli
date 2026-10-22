//! Host adapter of the OpenDAP toolkit: finding programs, the system clock and
//! the `doctor` probes for the host itself. POSIX hosts (macOS, Linux).

pub mod probes;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use opendap_core::Clock;
use opendap_model::Error;
use opendap_model::error as codes;

pub use probes::{HostPlatform, HostTools, Lima, PreCommit, Qemu};

/// Find a program on PATH.
pub fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(program))
        .find(|p| p.is_file())
}

/// Find a program on PATH, or fail with `host.tool_missing`.
pub fn require_tool(program: &str, purpose: &str) -> Result<PathBuf, Error> {
    which(program).ok_or_else(|| {
        Error::new(
            &codes::HOST_TOOL_MISSING,
            format!("`{program}` is not on PATH ({purpose})"),
        )
        .fix("opendap doctor")
    })
}

/// First line of `<program> <args…>` stdout, when it runs and succeeds.
pub fn first_line(program: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(text.lines().next().unwrap_or("").trim().to_string())
}

/// The real monotonic clock.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        SystemClock {
            origin: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }
    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_programs_and_reports_missing_ones() {
        assert!(which("sh").is_some());
        let err = require_tool("opendap-no-such-program", "a test").unwrap_err();
        assert_eq!(err.code.code, "host.tool_missing");
        assert_eq!(
            err.message,
            "`opendap-no-such-program` is not on PATH (a test)"
        );
        assert_eq!(
            first_line(Path::new("sh"), &["-c", "echo a; echo b"]).as_deref(),
            Some("a")
        );
        assert_eq!(first_line(Path::new("sh"), &["-c", "exit 1"]), None);
    }

    #[test]
    fn the_system_clock_moves() {
        let c = SystemClock::default();
        let t = c.now();
        c.sleep(Duration::from_millis(5));
        assert!(c.now() >= t + Duration::from_millis(5));
    }
}
