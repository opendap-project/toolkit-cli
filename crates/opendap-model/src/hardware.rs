//! The physical side of a virtual device (`opendap.hardware/v1`): what a person does with their
//! hands to a real one, a card in the socket, a USB device or charger on the port, and what the
//! board's battery holds. It is the hardware's state, never what the device's system makes of it
//! (a card is in the socket from the moment it goes in, mounted or not).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::device::Transport;

/// Something attached to the device from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Attachment {
    /// A microSD card in the socket.
    Microsd,
    /// A USB mass-storage device on the USB port.
    UsbStorage,
    /// A USB audio device (a DAC) on the USB port.
    UsbAudio,
    /// A charger on the USB port.
    Charger,
}

impl Attachment {
    pub const ALL: [Attachment; 4] = [
        Attachment::Microsd,
        Attachment::UsbStorage,
        Attachment::UsbAudio,
        Attachment::Charger,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Attachment::Microsd => "microsd",
            Attachment::UsbStorage => "usb-storage",
            Attachment::UsbAudio => "usb-audio",
            Attachment::Charger => "charger",
        }
    }
}

impl fmt::Display for Attachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The hardware's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareState {
    pub target: Transport,
    /// What is attached, in [`Attachment::ALL`] order.
    pub attached: Vec<Attachment>,
    /// The battery's charge in percent, when the board's battery model can be read (not while
    /// the device boots).
    pub battery: Option<u8>,
    /// The colour the status LED shows, red, green and blue from 0 to 255, when the board has one
    /// that can be read. What the device's system set it to, as an eye would see it.
    #[serde(default)]
    pub status_led: Option<[u8; 3]>,
    /// The light on the ambient light sensor, in lux, when the board has one that can be read.
    #[serde(default)]
    pub light: Option<u32>,
}

impl HardwareState {
    pub fn has(&self, a: Attachment) -> bool {
        self.attached.contains(&a)
    }
}

/// A change made by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "lowercase")]
pub enum HardwareChange {
    Attach {
        attachment: Attachment,
    },
    Detach {
        attachment: Attachment,
    },
    /// Set the battery's charge, in percent (0–100).
    Battery {
        percent: u8,
    },
    /// Set the light that falls on the ambient light sensor, in lux (0–100 000).
    Light {
        lux: u32,
    },
}

/// The brightest light the ambient light sensor is set to, in lux (direct sunlight).
pub const LIGHT_MAX_LUX: u32 = 100_000;

/// A change that reached the hardware; `changed` is false when it was already so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareChanged {
    #[serde(flatten)]
    pub state: HardwareState,
    #[serde(flatten)]
    pub change: HardwareChange,
    pub changed: bool,
}
