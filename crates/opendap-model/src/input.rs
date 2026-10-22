//! Input events sent to a device (`opendap.input/v1`).

use serde::{Deserialize, Serialize};

use crate::device::Transport;

/// A hardware button of the T1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Button {
    Action,
    VolUp,
    VolDown,
    Power,
}

impl Button {
    /// Every button.
    pub const ALL: [Button; 4] = [
        Button::Action,
        Button::VolUp,
        Button::VolDown,
        Button::Power,
    ];

    /// The name used in JSON and on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            Button::Action => "action",
            Button::VolUp => "vol-up",
            Button::VolDown => "vol-down",
            Button::Power => "power",
        }
    }
}

/// One input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InputEvent {
    /// Press a button for `hold_ms` milliseconds.
    Key { button: Button, hold_ms: u64 },
    /// Touch the display at a pixel.
    Tap { x: u32, y: u32 },
}

/// An event that reached the device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputSent {
    pub target: Transport,
    pub event: InputEvent,
    pub changed: bool,
}
