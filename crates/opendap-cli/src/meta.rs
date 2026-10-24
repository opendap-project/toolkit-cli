//! Command metadata, in one place. `opendap schema` merges it with the clap
//! tree; the tests below keep the two in sync.

use opendap_cli_kit::meta::{Category, CommandMeta};

const VM_BACKEND: &[&str] = &["workspace.not_found", "host.io"];

pub static COMMANDS: &[CommandMeta] = &[
    CommandMeta {
        path: "version",
        summary: "Version details: version, commit, build target and model schema IDs",
        category: Category::Read,
        idempotent: true,
        targeted: false,
        output_schema: "opendap.version/v1",
        errors: &[],
        examples: &["opendap version", "opendap version --json"],
    },
    CommandMeta {
        path: "doctor",
        summary: "Check the host; exits 1 when any check is an error",
        category: Category::Read,
        idempotent: true,
        targeted: false,
        output_schema: "opendap.doctor/v1",
        errors: &[],
        examples: &["opendap doctor", "opendap doctor --json"],
    },
    CommandMeta {
        path: "schema",
        summary: "Machine-readable description of every command, or of one",
        category: Category::Read,
        idempotent: true,
        targeted: false,
        output_schema: "opendap.schema/v1",
        errors: &["command.unknown"],
        examples: &["opendap schema", "opendap schema vm start"],
    },
    CommandMeta {
        path: "vm list",
        summary: "Virtual T1 instances and their endpoints (SSH, VNC, QMP)",
        category: Category::Read,
        idempotent: true,
        targeted: false,
        output_schema: "opendap.vm_list/v1",
        errors: VM_BACKEND,
        examples: &["opendap vm list", "opendap vm list --json"],
    },
    CommandMeta {
        path: "vm get",
        summary: "One virtual T1 instance: state, PID, endpoints, image",
        category: Category::Read,
        idempotent: true,
        targeted: false,
        output_schema: "opendap.vm/v1",
        errors: &["workspace.not_found", "host.io", "vm.not_found"],
        examples: &["opendap vm get", "opendap vm get default --json"],
    },
    CommandMeta {
        path: "vm start",
        summary: "Boot the virtual T1 headless; blocks until SSH answers unless --detach",
        category: Category::Mutate,
        idempotent: true,
        targeted: false,
        output_schema: "opendap.vm/v1",
        errors: &[
            "workspace.not_found",
            "host.io",
            "host.tool_missing",
            "vm.not_found",
            "vm.image_missing",
            "vm.start_failed",
            "vm.boot_timeout",
        ],
        examples: &["opendap vm start", "opendap vm start --detach --json"],
    },
    CommandMeta {
        path: "vm stop",
        summary: "Stop the virtual T1",
        category: Category::Mutate,
        idempotent: true,
        targeted: false,
        output_schema: "opendap.vm/v1",
        errors: &[
            "workspace.not_found",
            "host.io",
            "vm.not_found",
            "vm.stop_timeout",
        ],
        examples: &["opendap vm stop", "opendap vm stop --json"],
    },
    CommandMeta {
        path: "hardware get",
        summary: "What is attached to the target by hand (card, USB, charger), its battery's charge, its status LED and the light on its sensor",
        category: Category::Read,
        idempotent: true,
        targeted: true,
        output_schema: "opendap.hardware/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.io",
            "host.tool_missing",
            "transport.qmp_failed",
        ],
        examples: &["opendap hardware get", "opendap hardware get --json"],
    },
    CommandMeta {
        path: "hardware update",
        summary: "Attach or detach a microSD card, a USB disk or DAC, or a charger, or set the battery's charge or the light on the sensor (virtual devices)",
        category: Category::Mutate,
        idempotent: true,
        targeted: true,
        output_schema: "opendap.hardware/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.io",
            "host.tool_missing",
            "transport.qmp_failed",
        ],
        examples: &[
            "opendap vm attach microsd",
            "opendap vm attach usb-audio --json",
        ],
    },
    CommandMeta {
        path: "device list",
        summary: "Every known T1 with its transport, state and mode (today: the virtual T1)",
        category: Category::Read,
        idempotent: true,
        targeted: false,
        output_schema: "opendap.device_list/v1",
        errors: &["host.io"],
        examples: &["opendap device list", "opendap device list --json"],
    },
    CommandMeta {
        path: "device reboot",
        summary: "Restart the device, optionally into another boot mode (the device says which it has)",
        category: Category::Mutate,
        idempotent: false,
        targeted: true,
        output_schema: "opendap.power/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.tool_missing",
            "device.mode_unsupported",
            "device.power_failed",
            "transport.ssh_failed",
        ],
        examples: &[
            "opendap device reboot",
            "opendap device reboot --mode debug",
            "opendap device reboot --mode recovery --json",
        ],
    },
    CommandMeta {
        path: "device poweroff",
        summary: "Shut the device down in order",
        category: Category::Mutate,
        idempotent: false,
        targeted: true,
        output_schema: "opendap.power/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.tool_missing",
            "device.power_failed",
            "transport.ssh_failed",
        ],
        examples: &["opendap device poweroff", "opendap device poweroff --json"],
    },
    CommandMeta {
        path: "exec",
        summary: "Run a command on the target over SSH, or open a shell when none is given",
        category: Category::Mutate,
        idempotent: false,
        targeted: true,
        output_schema: "opendap.exec/v1",
        errors: &[
            "input.missing",
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.tool_missing",
            "transport.ssh_failed",
        ],
        examples: &[
            "opendap exec -- uname -a",
            "opendap exec --json -- cat /etc/os-release",
            "opendap exec",
        ],
    },
    CommandMeta {
        path: "screen capture",
        summary: "Capture the target's display to a PNG file",
        category: Category::Read,
        idempotent: true,
        targeted: true,
        output_schema: "opendap.screen_capture/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.tool_missing",
            "host.io",
            "transport.qmp_failed",
        ],
        examples: &[
            "opendap screen capture",
            "opendap screen capture /tmp/t1.png --json",
        ],
    },
    CommandMeta {
        path: "input send key",
        summary: "Press a control by its role (the T1: action, volume-up, volume-down, main)",
        category: Category::Mutate,
        idempotent: false,
        targeted: true,
        output_schema: "opendap.input/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.tool_missing",
            "transport.qmp_failed",
        ],
        examples: &[
            "opendap input send key volume-up",
            "opendap input send key main --hold-ms 3000 --json",
        ],
    },
    CommandMeta {
        path: "input send key-down",
        summary: "Put a control down and leave it there, until input send key-up",
        category: Category::Mutate,
        idempotent: false,
        targeted: true,
        output_schema: "opendap.input/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.tool_missing",
            "transport.qmp_failed",
        ],
        examples: &[
            "opendap input send key-down main",
            "opendap input send key-down volume-down --json",
        ],
    },
    CommandMeta {
        path: "input send key-up",
        summary: "Release a control put down with input send key-down",
        category: Category::Mutate,
        idempotent: false,
        targeted: true,
        output_schema: "opendap.input/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.tool_missing",
            "transport.qmp_failed",
        ],
        examples: &[
            "opendap input send key-up main",
            "opendap input send key-up volume-down --json",
        ],
    },
    CommandMeta {
        path: "input send keyboard",
        summary: "A key of a keyboard attached to the device, by its Linux name; it reaches the activity as a keyboard's key",
        category: Category::Mutate,
        idempotent: false,
        targeted: true,
        output_schema: "opendap.input/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.tool_missing",
            "transport.qmp_failed",
        ],
        examples: &[
            "opendap input send keyboard right",
            "opendap input send keyboard enter --down",
        ],
    },
    CommandMeta {
        path: "input send tap",
        summary: "Touch the display at a pixel",
        category: Category::Mutate,
        idempotent: false,
        targeted: true,
        output_schema: "opendap.input/v1",
        errors: &[
            "target.invalid",
            "target.unsupported",
            "device.not_found",
            "vm.not_found",
            "vm.not_running",
            "workspace.not_found",
            "host.tool_missing",
            "transport.qmp_failed",
        ],
        examples: &[
            "opendap input send tap 160 240",
            "opendap input send tap 10 10 --json",
        ],
    },
];

pub fn find(path: &str) -> Option<&'static CommandMeta> {
    COMMANDS.iter().find(|c| c.path == path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::CommandFactory;

    /// Leaf command paths of the clap tree.
    fn leaves(cmd: &clap::Command, prefix: &str, out: &mut Vec<String>) {
        for sub in cmd.get_subcommands() {
            let path = if prefix.is_empty() {
                sub.get_name().to_string()
            } else {
                format!("{prefix} {}", sub.get_name())
            };
            if sub.get_subcommands().next().is_none() {
                out.push(path);
            } else {
                leaves(sub, &path, out);
            }
        }
    }

    #[test]
    fn every_command_has_metadata_and_back() {
        let mut paths = Vec::new();
        leaves(&Cli::command(), "", &mut paths);
        for p in &paths {
            assert!(find(p).is_some(), "no metadata for `{p}`");
        }
        for m in COMMANDS {
            assert!(
                paths.iter().any(|p| p == m.path),
                "metadata for unknown command `{}`",
                m.path
            );
        }
    }

    #[test]
    fn metadata_is_complete() {
        // Verbs from research §8.1, plus `send` and `capture` from the accepted spec §5.
        const VERBS: &[&str] = &[
            "list",
            "get",
            "create",
            "update",
            "delete",
            "install",
            "uninstall",
            "start",
            "stop",
            "restart",
            "push",
            "pull",
            "flash",
            "boot",
            "reboot",
            "poweroff",
            "exec",
            "watch",
            "wait",
            "pair",
            "unpair",
            "use",
            "check",
            "fix",
            "search",
            "send",
            "capture",
        ];
        const TOP_LEVEL: &[&str] = &["version", "doctor", "schema", "exec"];
        for m in COMMANDS {
            let words: Vec<&str> = m.path.split(' ').collect();
            let ok = match words.as_slice() {
                [single] => TOP_LEVEL.contains(single),
                [_, verb] => VERBS.contains(verb),
                [_, verb, _] => VERBS.contains(verb),
                _ => false,
            };
            assert!(ok, "`{}` does not follow the grammar", m.path);
            assert!(m.examples.len() >= 2, "`{}` needs two examples", m.path);
            assert!(m.output_schema.starts_with("opendap.") && m.output_schema.ends_with("/v1"));
            assert!(
                opendap_model::SCHEMAS.contains(&m.output_schema),
                "{}",
                m.output_schema
            );
            for code in m.errors {
                assert!(
                    opendap_model::error::lookup(code).is_some(),
                    "`{}`: unknown code {code}",
                    m.path
                );
            }
        }
    }
}
