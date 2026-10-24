//! The clap command tree. Behaviour metadata lives in `meta.rs`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "opendap",
    about = "The command-line tool for the T1 and Blackbird OS",
    long_about = "The command-line tool for the T1 and Blackbird OS.\n\n\
        Agents: run `opendap schema` for every command with its flags, \
        output schema and error codes, then `opendap schema <command>`.",
    disable_version_flag = true,
    arg_required_else_help = true,
    subcommand_required = false
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,

    /// Print the bare version and exit (`opendap version --json` gives details)
    #[arg(short = 'V', long = "version")]
    pub version: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Flags shared by every command.
#[derive(Debug, Args)]
pub struct Global {
    /// Device or VM selector: vm:<name>, usb:<serial>, net:<host>[:port], serial:<path>, maskrom:<bus>-<addr>
    #[arg(
        short = 't',
        long,
        global = true,
        env = "OPENDAP_TARGET",
        value_name = "SELECTOR"
    )]
    pub target: Option<String>,

    /// Output mode (default: human on a TTY, JSON when piped or under an agent; env OPENDAP_OUTPUT)
    #[arg(short = 'o', long, global = true, value_name = "MODE",
          value_parser = ["human", "json", "ndjson", "plain"])]
    pub output: Option<String>,

    /// Shorthand for --output json
    #[arg(long, global = true)]
    pub json: bool,

    /// Never prompt or open an interactive session
    #[arg(long, global = true, env = "OPENDAP_NO_INPUT",
          value_parser = clap::builder::BoolishValueParser::new())]
    pub no_input: bool,

    /// Print what the command would do and change nothing
    #[arg(short = 'n', long, global = true)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Version details
    Version,
    /// Check the host: platform, tools, QEMU for the emulator, the virtual-devices checkout, the dev SSH key, the image
    Doctor,
    /// Machine-readable description of every command, or of one
    Schema {
        /// Command path, for example `vm start` (omit for every command)
        #[arg(value_name = "COMMAND")]
        command: Vec<String>,
    },
    /// Virtual T1 instances
    Vm {
        #[command(subcommand)]
        cmd: VmCmd,
    },
    /// Physical and virtual T1 devices
    Device {
        #[command(subcommand)]
        cmd: DeviceCmd,
    },
    /// Run a command on the target over SSH, or open a shell when none is given
    Exec {
        /// Command and arguments, after `--`; each argument reaches the device unchanged
        #[arg(last = true, value_name = "CMD")]
        argv: Vec<String>,
    },
    /// The target's display
    Screen {
        #[command(subcommand)]
        cmd: ScreenCmd,
    },
    /// Button and touch input
    Input {
        #[command(subcommand)]
        cmd: InputCmd,
    },
    /// The target's physical side: what is attached by hand, and its battery
    Hardware {
        #[command(subcommand)]
        cmd: HardwareCmd,
    },
}

#[derive(Debug, Subcommand)]
pub enum VmCmd {
    /// Virtual T1 instances and their endpoints (SSH, VNC, QMP)
    List,
    /// One virtual T1 instance
    Get {
        /// Instance name
        #[arg(default_value = "default")]
        name: String,
    },
    /// Boot the virtual T1 headless and wait until SSH answers
    Start {
        /// Instance name
        #[arg(default_value = "default")]
        name: String,
        /// Return once QEMU runs, without waiting for the boot
        #[arg(long)]
        detach: bool,
        /// How long to wait for SSH, in seconds (`90`, `90s`, `3m`)
        #[arg(long, value_name = "DURATION", default_value = "180s", value_parser = parse_duration)]
        wait_timeout: u64,
    },
    /// Stop the virtual T1
    Stop {
        /// Instance name
        #[arg(default_value = "default")]
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum HardwareCmd {
    /// What is attached to the target by hand (card, USB, charger) and its battery's charge
    Get,
    /// Attach or detach something by hand, or set the battery's charge (virtual devices)
    #[command(group = clap::ArgGroup::new("change").required(true).multiple(false))]
    Update {
        /// Attach this, as a hand does (a card goes in its socket)
        #[arg(long, value_name = "WHAT", group = "change")]
        attach: Option<AttachmentArg>,
        /// Detach this, as a hand does
        #[arg(long, value_name = "WHAT", group = "change")]
        detach: Option<AttachmentArg>,
        /// Set the battery's charge, in percent
        #[arg(long, value_name = "PERCENT", group = "change",
              value_parser = clap::value_parser!(u8).range(0..=100))]
        battery: Option<u8>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum AttachmentArg {
    /// A microSD card (the board's card size; the same card each time)
    Microsd,
    /// A USB disk (1 GiB, the same disk each time)
    UsbStorage,
    /// A USB DAC
    UsbAudio,
    /// A charger on the USB port
    Charger,
}

impl From<AttachmentArg> for opendap_model::Attachment {
    fn from(a: AttachmentArg) -> Self {
        match a {
            AttachmentArg::Microsd => Self::Microsd,
            AttachmentArg::UsbStorage => Self::UsbStorage,
            AttachmentArg::UsbAudio => Self::UsbAudio,
            AttachmentArg::Charger => Self::Charger,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum DeviceCmd {
    /// Every known T1 with its transport, state and mode (today: the virtual T1)
    List,
    /// Restart the device, optionally into another boot mode (the device says which it has)
    Reboot {
        /// The mode to boot into: normal, debug (once), recovery, maskrom
        #[arg(long, value_enum, default_value_t = BootModeArg::Normal)]
        mode: BootModeArg,
    },
    /// Shut the device down in order
    Poweroff,
}

/// The boot modes of `device reboot --mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BootModeArg {
    Normal,
    Debug,
    Recovery,
    Maskrom,
}

impl From<BootModeArg> for opendap_model::BootMode {
    fn from(m: BootModeArg) -> Self {
        match m {
            BootModeArg::Normal => Self::Normal,
            BootModeArg::Debug => Self::Debug,
            BootModeArg::Recovery => Self::Recovery,
            BootModeArg::Maskrom => Self::Maskrom,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum ScreenCmd {
    /// Capture the display to a PNG file
    Capture {
        /// Output file (default: screen-<unix time>.png in the current directory)
        file: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum InputCmd {
    /// Inject an input event
    Send {
        #[command(subcommand)]
        event: InputEvent,
    },
}

#[derive(Debug, Subcommand)]
pub enum InputEvent {
    /// Press a control by its role
    Key {
        /// The control's role on the device (the T1: action, volume-up, volume-down, main)
        role: String,
        /// How long the control stays down, in milliseconds
        #[arg(long, value_name = "MS", default_value_t = 100,
              value_parser = clap::value_parser!(u64).range(1..=60_000))]
        hold_ms: u64,
    },
    /// Put a control down and leave it there (release it with key-up)
    KeyDown {
        /// The control's role on the device
        role: String,
    },
    /// Release a control put down with key-down
    KeyUp {
        /// The control's role on the device
        role: String,
    },
    /// Touch the display at a pixel (origin top left; the device checks it is on its display)
    Tap {
        /// Horizontal pixel
        x: u32,
        /// Vertical pixel
        y: u32,
    },
}

/// Seconds from `90`, `90s` or `3m`.
pub fn parse_duration(s: &str) -> Result<u64, String> {
    let (num, mult) = match s.strip_suffix('m') {
        Some(n) => (n, 60),
        None => (s.strip_suffix('s').unwrap_or(s), 1),
    };
    match num.parse::<u64>() {
        Ok(n) if n > 0 => Ok(n * mult),
        _ => Err(format!("`{s}` is not a duration like 90, 90s or 3m")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("opendap").chain(args.iter().copied()))
    }

    #[test]
    fn tree_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn global_flags_work_after_the_subcommand() {
        let cli = parse(&["device", "list", "--json", "-t", "vm:default", "-n"]).unwrap();
        assert!(cli.global.json && cli.global.dry_run);
        assert_eq!(cli.global.target.as_deref(), Some("vm:default"));
        let cli = parse(&["-o", "plain", "vm", "list"]).unwrap();
        assert_eq!(cli.global.output.as_deref(), Some("plain"));
        assert!(parse(&["-o", "yaml", "vm", "list"]).is_err());
    }

    #[test]
    fn exec_takes_everything_after_double_dash() {
        let cli = parse(&["exec", "--", "uname", "-a"]).unwrap();
        match cli.command {
            Some(Command::Exec { argv }) => assert_eq!(argv, ["uname", "-a"]),
            other => panic!("{other:?}"),
        }
        let cli = parse(&["exec", "--target", "vm:default"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Exec { ref argv }) if argv.is_empty()));
        assert!(parse(&["exec", "uname"]).is_err());
    }

    #[test]
    fn vm_defaults_and_durations() {
        match parse(&["vm", "start"]).unwrap().command {
            Some(Command::Vm {
                cmd:
                    VmCmd::Start {
                        name,
                        detach,
                        wait_timeout,
                    },
            }) => {
                assert_eq!(
                    (name.as_str(), detach, wait_timeout),
                    ("default", false, 180)
                );
            }
            other => panic!("{other:?}"),
        }
        match parse(&["vm", "start", "--wait-timeout", "3m", "--detach"])
            .unwrap()
            .command
        {
            Some(Command::Vm {
                cmd:
                    VmCmd::Start {
                        wait_timeout,
                        detach,
                        ..
                    },
            }) => {
                assert_eq!((wait_timeout, detach), (180, true));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(parse_duration("90"), Ok(90));
        assert!(parse_duration("0s").is_err() && parse_duration("x").is_err());
    }

    #[test]
    fn input_takes_roles_and_pixels() {
        match parse(&["input", "send", "key", "volume-up"])
            .unwrap()
            .command
        {
            Some(Command::Input {
                cmd:
                    InputCmd::Send {
                        event: InputEvent::Key { role, hold_ms },
                    },
            }) => {
                assert_eq!((role.as_str(), hold_ms), ("volume-up", 100));
            }
            other => panic!("{other:?}"),
        }
        assert!(parse(&["input", "send", "tap", "319", "479"]).is_ok());
        assert!(parse(&["input", "send", "tap", "-1", "0"]).is_err());
    }

    #[test]
    fn bare_version_flag_and_schema_path() {
        assert!(parse(&["-V"]).unwrap().version);
        match parse(&["schema", "vm", "start"]).unwrap().command {
            Some(Command::Schema { command }) => assert_eq!(command, ["vm", "start"]),
            other => panic!("{other:?}"),
        }
    }
}
