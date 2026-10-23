//! Stable error codes and the error envelope (`opendap.error/v1`).
//!
//! Remediation knowledge lives here, not in ad-hoc switches: each code carries
//! its exit code, whether a retry can help, and a default hint.

use serde::{Deserialize, Serialize};

/// Coarse, stable exit codes for shells. The JSON `code` is the precise contract.
pub mod exit {
    pub const OK: u8 = 0;
    pub const FAILURE: u8 = 1;
    pub const USAGE: u8 = 2;
    pub const NOT_FOUND: u8 = 3;
    pub const AUTH: u8 = 4;
    pub const PERMISSION: u8 = 5;
    pub const AMBIGUOUS: u8 = 6;
    pub const BUSY: u8 = 7;
    pub const TIMEOUT: u8 = 8;
    pub const CONFIRM: u8 = 9;
    pub const PRECONDITION: u8 = 10;
    pub const UNSUPPORTED: u8 = 11;
    pub const INTERRUPTED: u8 = 130;
}

/// A stable error code: `<area>.<condition>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ErrorCode {
    pub code: &'static str,
    pub exit: u8,
    pub retryable: bool,
    /// What the code means, for `schema` and the error catalogue.
    pub summary: &'static str,
    /// Default hint when the error site gives none.
    pub hint: &'static str,
}

macro_rules! codes {
    ($( $name:ident = ($code:literal, $exit:expr, $retry:literal, $summary:literal, $hint:literal); )*) => {
        $( pub const $name: ErrorCode = ErrorCode {
            code: $code, exit: $exit, retryable: $retry, summary: $summary, hint: $hint,
        }; )*
        /// Every code this model version defines.
        pub const ALL: &[ErrorCode] = &[$($name),*];
    };
}

codes! {
    INPUT_INVALID = ("input.invalid", exit::USAGE, false,
        "An argument or flag value is invalid.",
        "Run the command with --help, or `opendap schema <command>` for the accepted values.");
    INPUT_MISSING = ("input.missing", exit::USAGE, false,
        "A value is required and prompting is disabled.",
        "Pass the value explicitly; prompts are disabled without a TTY, under OPENDAP_NO_INPUT=1 or --no-input.");
    COMMAND_UNKNOWN = ("command.unknown", exit::NOT_FOUND, false,
        "No command matches the given path.",
        "Run `opendap schema` to list every command.");
    TARGET_INVALID = ("target.invalid", exit::USAGE, false,
        "The target selector does not parse.",
        "Use vm:<name>, usb:<serial>, net:<host>[:port], serial:<path> or maskrom:<bus>-<addr>.");
    TARGET_UNSUPPORTED = ("target.unsupported", exit::UNSUPPORTED, false,
        "This build cannot reach that kind of target yet.",
        "Only the virtual T1 (vm:default) is supported in this version.");
    DEVICE_NOT_FOUND = ("device.not_found", exit::NOT_FOUND, false,
        "No device matches the target, or no device is running.",
        "Run `opendap device list`, or start the virtual T1 with `opendap vm start`.");
    VM_NOT_FOUND = ("vm.not_found", exit::NOT_FOUND, false,
        "No virtual T1 instance has that name.",
        "Run `opendap vm list`; the only instance is `default`.");
    VM_NOT_RUNNING = ("vm.not_running", exit::PRECONDITION, false,
        "The virtual device is stopped or still booting.",
        "Run `opendap vm start` (it waits for the boot to finish).");
    VM_IMAGE_MISSING = ("vm.image_missing", exit::PRECONDITION, true,
        "The virtual device's image has not been built, or is being rebuilt.",
        "A maintainer builds it with toolkit-image `odap-image build qemu`; or wait for the running build.");
    VM_START_FAILED = ("vm.start_failed", exit::FAILURE, true,
        "QEMU did not start.",
        "Run `odap-vdev status` for its QEMU log, and `opendap doctor`.");
    VM_BOOT_TIMEOUT = ("vm.boot_timeout", exit::TIMEOUT, true,
        "The virtual device started but SSH did not answer in time.",
        "Check the console log (`odap-vdev status`), or retry with a longer --wait-timeout.");
    VM_STOP_TIMEOUT = ("vm.stop_timeout", exit::TIMEOUT, true,
        "QEMU did not exit in time after the stop request.",
        "Retry `opendap vm stop`.");
    WORKSPACE_NOT_FOUND = ("workspace.not_found", exit::PRECONDITION, false,
        "The virtual-devices checkout that runs virtual devices was not found.",
        "Run from inside the OpenDAP workspace, or set ODAP_VIRTUAL_DEVICES_DIR to the virtual-devices checkout.");
    HOST_TOOL_MISSING = ("host.tool_missing", exit::PRECONDITION, false,
        "A host program the command needs is not installed.",
        "Run `opendap doctor` for the missing tool and how to install it.");
    HOST_IO = ("host.io", exit::FAILURE, false,
        "A file on the host could not be read or written.",
        "Check the path and its permissions.");
    DEVICE_MODE_UNSUPPORTED = ("device.mode_unsupported", exit::UNSUPPORTED, false,
        "The device has no such boot mode.",
        "Run `opendap exec -- odap-reboot --list` for the boot modes the device has.");
    DEVICE_POWER_FAILED = ("device.power_failed", exit::FAILURE, true,
        "The device did not accept the restart or power-off.",
        "Read the device's message in the error details, or its journal with `opendap exec -- journalctl -b`.");
    TRANSPORT_SSH_FAILED = ("transport.ssh_failed", exit::FAILURE, true,
        "The SSH connection to the target failed.",
        "Check that the target is online with `opendap device list`, then retry.");
    TRANSPORT_QMP_FAILED = ("transport.qmp_failed", exit::FAILURE, true,
        "The QMP command to the virtual T1 failed.",
        "Check that the virtual T1 is running with `opendap vm get`, then retry.");
}

/// Find a code by its identifier.
pub fn lookup(code: &str) -> Option<&'static ErrorCode> {
    ALL.iter().find(|c| c.code == code)
}

/// A remediation step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fix {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

impl Fix {
    pub fn command(command: impl Into<String>) -> Self {
        Fix {
            description: None,
            command: Some(command.into()),
        }
    }
    pub fn describe(description: impl Into<String>) -> Self {
        Fix {
            description: Some(description.into()),
            command: None,
        }
    }
}

/// The `error` member of the envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub hint: String,
    pub fix: Vec<Fix>,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    pub docs: String,
}

/// The error envelope (`opendap.error/v1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorDocument {
    pub schema: String,
    pub ok: bool,
    pub error: ErrorBody,
}

impl ErrorDocument {
    pub fn new(body: ErrorBody) -> Self {
        ErrorDocument {
            schema: "opendap.error/v1".into(),
            ok: false,
            error: body,
        }
    }
}

/// An error as data: a stable code with its message, hint, fixes and details.
/// Every library returns it; interfaces render it (the CLI as the
/// `opendap.error/v1` envelope).
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    pub code: &'static ErrorCode,
    pub message: String,
    /// Overrides the code's default hint.
    pub hint: Option<String>,
    pub fix: Vec<Fix>,
    pub details: Option<serde_json::Value>,
}

impl Error {
    pub fn new(code: &'static ErrorCode, message: impl Into<String>) -> Self {
        Error {
            code,
            message: message.into(),
            hint: None,
            fix: Vec::new(),
            details: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Add a fix that is a command to run.
    pub fn fix(mut self, command: impl Into<String>) -> Self {
        self.fix.push(Fix::command(command));
        self
    }

    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn exit_code(&self) -> u8 {
        self.code.exit
    }

    /// The `error` member of the envelope.
    pub fn body(&self) -> ErrorBody {
        ErrorBody {
            code: self.code.code.to_string(),
            message: self.message.clone(),
            hint: self
                .hint
                .clone()
                .unwrap_or_else(|| self.code.hint.to_string()),
            fix: self.fix.clone(),
            retryable: self.code.retryable,
            details: self.details.clone(),
            docs: format!("opendap help errors {}", self.code.code),
        }
    }

    /// The whole envelope.
    pub fn document(&self) -> ErrorDocument {
        ErrorDocument::new(self.body())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_has_the_documented_shape() {
        let e = Error::new(&VM_NOT_RUNNING, "vm:default is stopped")
            .fix("opendap vm start")
            .details(serde_json::json!({"target": "vm:default"}));
        let v = serde_json::to_value(e.document()).unwrap();
        assert_eq!(v["schema"], "opendap.error/v1");
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"]["code"], "vm.not_running");
        assert_eq!(v["error"]["message"], "vm:default is stopped");
        assert_eq!(v["error"]["hint"], VM_NOT_RUNNING.hint);
        assert_eq!(
            v["error"]["fix"],
            serde_json::json!([{"command": "opendap vm start"}])
        );
        assert_eq!(v["error"]["retryable"], false);
        assert_eq!(v["error"]["details"]["target"], "vm:default");
        assert_eq!(v["error"]["docs"], "opendap help errors vm.not_running");
        assert_eq!(e.exit_code(), 10);
    }

    #[test]
    fn explicit_hint_wins_and_details_are_optional() {
        let e = Error::new(&INPUT_INVALID, "bad").hint("try this");
        let v = serde_json::to_value(e.document()).unwrap();
        assert_eq!(v["error"]["hint"], "try this");
        assert!(v["error"].get("details").is_none());
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn codes_are_unique_dotted_and_documented() {
        let mut seen = std::collections::HashSet::new();
        for c in ALL {
            assert!(seen.insert(c.code), "duplicate code {}", c.code);
            let (area, cond) = c.code.split_once('.').expect("dotted");
            assert!(!area.is_empty() && !cond.is_empty());
            assert!(
                c.code
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch == '.' || ch == '_')
            );
            assert!(!c.summary.is_empty() && !c.hint.is_empty());
            assert!(c.exit <= 11 || c.exit == exit::INTERRUPTED);
        }
        assert_eq!(lookup("vm.not_running"), Some(&VM_NOT_RUNNING));
        assert_eq!(lookup("nope.nope"), None);
    }
}
