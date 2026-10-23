//! Restarting and powering off a device (`opendap.power/v1`).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::device::Transport;

/// A mode a device boots into. The device says which it has (blackbird-os `odap-reboot --list`):
/// `normal` always; `debug` on images with the debug boot entry; `recovery` and `maskrom` from the
/// board's support package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BootMode {
    Normal,
    Debug,
    Recovery,
    Maskrom,
}

impl BootMode {
    pub fn as_str(self) -> &'static str {
        match self {
            BootMode::Normal => "normal",
            BootMode::Debug => "debug",
            BootMode::Recovery => "recovery",
            BootMode::Maskrom => "maskrom",
        }
    }
}

impl fmt::Display for BootMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What was asked of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum PowerAction {
    /// Restart, into `mode`.
    Reboot { mode: BootMode },
    /// Shut down in order.
    Poweroff,
}

/// The device accepted the request and is going down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerDone {
    pub target: Transport,
    #[serde(flatten)]
    pub action: PowerAction,
}
