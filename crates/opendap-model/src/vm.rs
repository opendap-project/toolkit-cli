//! Virtual T1 instances (`opendap.vm/v1`, `opendap.vm_list/v1`).

use serde::{Deserialize, Serialize};

/// The only virtual T1 instance today, addressed as `vm:default`.
pub const DEFAULT_NAME: &str = "default";

/// Lifecycle state of a virtual T1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VmState {
    Stopped,
    /// The emulator runs, SSH does not answer yet.
    Booting,
    /// SSH answers.
    Running,
}

impl VmState {
    /// The state as it appears in JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            VmState::Stopped => "stopped",
            VmState::Booting => "booting",
            VmState::Running => "running",
        }
    }
}

/// Where a virtual T1 can be reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoints {
    pub ssh: String,
    pub vnc: String,
    pub qmp: String,
}

/// The disk image a virtual T1 boots from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageInfo {
    pub path: String,
    pub present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

/// One virtual T1 instance (`opendap.vm/v1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmInfo {
    pub id: String,
    pub name: String,
    pub state: VmState,
    pub pid: Option<u32>,
    pub endpoints: Endpoints,
    pub image: ImageInfo,
    pub console_log: String,
    /// The virtual-devices checkout that runs it.
    pub runner_dir: String,
}
