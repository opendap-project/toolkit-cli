//! Running a command on a device (`opendap.exec/v1`).

use serde::{Deserialize, Serialize};

use crate::device::Transport;

/// A finished, captured remote command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecResult {
    pub target: Transport,
    /// The argv as given; each argument reached the device unchanged.
    pub command: Vec<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}
