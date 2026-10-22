//! Display captures (`opendap.screen_capture/v1`).

use serde::{Deserialize, Serialize};

use crate::device::Transport;

/// A saved capture of a device display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenCapture {
    pub target: Transport,
    pub path: String,
    /// Always `png` today.
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}
