//! Devices, transports and target selectors.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// How the host reaches a device. Serialized as `{"kind":"vm","name":"default"}`.
///
/// Only `vm` is backed by an implementation today; the other kinds are part of
/// the selector grammar so USB, network, serial and MaskROM transports can be
/// added without changing the type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Transport {
    /// A virtual T1 instance.
    Vm { name: String },
    /// A T1 over USB, identified by its serial number.
    Usb { serial: String },
    /// A T1 over SSH on the network.
    Net {
        host: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        port: Option<u16>,
    },
    /// A serial console.
    Serial { path: String },
    /// A Rockchip SoC in MaskROM mode (rockusb), by USB bus and address.
    Maskrom { bus: u16, addr: u16 },
}

impl Transport {
    /// The selector string for this transport, for example `vm:default`.
    pub fn selector(&self) -> String {
        self.to_string()
    }

    /// The kind as it appears in selectors and JSON.
    pub fn kind(&self) -> &'static str {
        match self {
            Transport::Vm { .. } => "vm",
            Transport::Usb { .. } => "usb",
            Transport::Net { .. } => "net",
            Transport::Serial { .. } => "serial",
            Transport::Maskrom { .. } => "maskrom",
        }
    }
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Transport::Vm { name } => write!(f, "vm:{name}"),
            Transport::Usb { serial } => write!(f, "usb:{serial}"),
            Transport::Net {
                host,
                port: Some(p),
            } => write!(f, "net:{host}:{p}"),
            Transport::Net { host, port: None } => write!(f, "net:{host}"),
            Transport::Serial { path } => write!(f, "serial:{path}"),
            Transport::Maskrom { bus, addr } => write!(f, "maskrom:{bus}-{addr}"),
        }
    }
}

/// A target selector given with `--target` or `OPENDAP_TARGET`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// An explicit transport selector (`vm:default`, `usb:<serial>`…).
    Transport(Transport),
    /// A bare name, resolved through contexts.
    Context(String),
}

/// Why a selector could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorError(pub String);

impl fmt::Display for SelectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SelectorError {}

fn non_empty(kind: &str, value: &str) -> Result<String, SelectorError> {
    if value.is_empty() {
        Err(SelectorError(format!("`{kind}:` needs a value")))
    } else {
        Ok(value.to_string())
    }
}

impl FromStr for Target {
    type Err = SelectorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(SelectorError("empty target selector".into()));
        }
        let Some((kind, rest)) = s.split_once(':') else {
            if s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
            {
                return Ok(Target::Context(s.to_string()));
            }
            return Err(SelectorError(format!("invalid context name `{s}`")));
        };
        let transport = match kind {
            "vm" => Transport::Vm {
                name: non_empty(kind, rest)?,
            },
            "usb" => Transport::Usb {
                serial: non_empty(kind, rest)?,
            },
            "serial" => Transport::Serial {
                path: non_empty(kind, rest)?,
            },
            "net" => {
                let rest = non_empty(kind, rest)?;
                match rest.rsplit_once(':') {
                    Some((host, port)) if !host.is_empty() && !host.contains(':') => {
                        Transport::Net {
                            host: host.to_string(),
                            port: Some(port.parse().map_err(|_| {
                                SelectorError(format!("invalid port `{port}` in `{s}`"))
                            })?),
                        }
                    }
                    _ => Transport::Net {
                        host: rest,
                        port: None,
                    },
                }
            }
            "maskrom" => {
                let (bus, addr) = rest
                    .split_once('-')
                    .and_then(|(b, a)| Some((b.parse().ok()?, a.parse().ok()?)))
                    .ok_or_else(|| {
                        SelectorError(format!("`{s}`: expected maskrom:<bus>-<addr>"))
                    })?;
                Transport::Maskrom { bus, addr }
            }
            other => {
                return Err(SelectorError(format!(
                    "unknown target kind `{other}`; expected vm, usb, net, serial or maskrom"
                )));
            }
        };
        Ok(Target::Transport(transport))
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Transport(t) => t.fmt(f),
            Target::Context(name) => f.write_str(name),
        }
    }
}

/// Device mode, as adb and fastboot treat them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "recovery")]
    Recovery,
    #[serde(rename = "maskrom")]
    Maskrom,
    #[serde(rename = "gadget:serial")]
    GadgetSerial,
    #[serde(rename = "gadget:net")]
    GadgetNet,
    #[serde(rename = "gadget:storage")]
    GadgetStorage,
}

/// Whether the device answers on its transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceState {
    /// Reachable and ready for commands.
    Online,
    /// Present but not ready yet (a virtual T1 still booting).
    Booting,
    /// Known but not running or not reachable.
    Offline,
}

/// A T1, physical or virtual (`opendap.device/v1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// The selector that addresses this device, for example `vm:default`.
    pub id: String,
    pub model: String,
    /// True for a virtual T1.
    #[serde(rename = "virtual")]
    pub is_virtual: bool,
    pub transport: Transport,
    pub state: DeviceState,
    /// Known only when the device is online.
    pub mode: Option<Mode>,
    pub serial: Option<String>,
}

/// A non-fatal problem met while discovering devices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    /// An error code from [`crate::error`].
    pub code: String,
    pub message: String,
}

/// The result of device discovery (`opendap.device_list/v1`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceList {
    pub items: Vec<Device>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Target, SelectorError> {
        s.parse()
    }

    #[test]
    fn parses_every_selector_kind() {
        assert_eq!(
            parse("vm:default").unwrap(),
            Target::Transport(Transport::Vm {
                name: "default".into()
            })
        );
        assert_eq!(
            parse("usb:T1A3F2").unwrap(),
            Target::Transport(Transport::Usb {
                serial: "T1A3F2".into()
            })
        );
        assert_eq!(
            parse("net:t1.local").unwrap(),
            Target::Transport(Transport::Net {
                host: "t1.local".into(),
                port: None
            })
        );
        assert_eq!(
            parse("net:10.0.0.2:2222").unwrap(),
            Target::Transport(Transport::Net {
                host: "10.0.0.2".into(),
                port: Some(2222)
            })
        );
        assert_eq!(
            parse("serial:/dev/cu.usbmodem1").unwrap(),
            Target::Transport(Transport::Serial {
                path: "/dev/cu.usbmodem1".into()
            })
        );
        assert_eq!(
            parse("maskrom:1-4").unwrap(),
            Target::Transport(Transport::Maskrom { bus: 1, addr: 4 })
        );
        assert_eq!(parse("bench").unwrap(), Target::Context("bench".into()));
    }

    #[test]
    fn rejects_malformed_selectors() {
        for bad in [
            "",
            "vm:",
            "foo:bar",
            "maskrom:1",
            "maskrom:a-b",
            "net:host:port",
            "a b",
        ] {
            assert!(parse(bad).is_err(), "`{bad}` should not parse");
        }
    }

    #[test]
    fn selector_round_trips() {
        for s in [
            "vm:default",
            "usb:X1",
            "net:h",
            "net:h:22",
            "serial:/dev/x",
            "maskrom:3-7",
        ] {
            assert_eq!(parse(s).unwrap().to_string(), s);
        }
    }

    #[test]
    fn device_json_shape() {
        let d = Device {
            id: "vm:default".into(),
            model: "T1".into(),
            is_virtual: true,
            transport: Transport::Vm {
                name: "default".into(),
            },
            state: DeviceState::Online,
            mode: Some(Mode::Normal),
            serial: None,
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(
            v["transport"],
            serde_json::json!({"kind":"vm","name":"default"})
        );
        assert_eq!(v["virtual"], true);
        assert_eq!(v["state"], "online");
        assert_eq!(v["mode"], "normal");
        assert_eq!(serde_json::to_value(Mode::GadgetNet).unwrap(), "gadget:net");
    }
}
