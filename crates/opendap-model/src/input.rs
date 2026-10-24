//! Input events sent to a device (`opendap.input/v1`).

use serde::{Deserialize, Serialize};

use crate::device::Transport;

/// The role of a control (system opendap-device): what it means on the device, never which key or
/// button produces it. The device's description names the roles it has; the device rejects others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Role(pub String);

impl Role {
    /// The roles of the T1 reference, for examples and completion only; a device may have others.
    pub const COMMON: [&'static str; 4] = ["action", "volume-up", "volume-down", "main"];

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Role {
    fn from(s: &str) -> Self {
        Role(s.to_string())
    }
}

/// One input event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum InputEvent {
    /// Press the control with `role` for `hold_ms` milliseconds.
    Key { role: Role, hold_ms: u64 },
    /// Put the control with `role` down and leave it there, as a finger resting on it: for
    /// surfaces that follow a person's hand (the simulator window), paired with [`Self::KeyUp`].
    #[serde(rename = "key-down")]
    KeyDown { role: Role },
    /// Release the control with `role`.
    #[serde(rename = "key-up")]
    KeyUp { role: Role },
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
