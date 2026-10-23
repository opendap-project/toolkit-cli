# CLI

The `opendap` command-line tool: OpenDAP's adb, agentic-first, for advanced users and package developers working with OpenDAP devices, real or virtual. Written in Rust; it sits next to OpenDAP Desktop, not in the Developer Toolkit.

Part of [OpenDAP](https://github.com/opendap-project).

## Status

A first slice of the [CLI spec](docs/specs/opendap-cli.md) is implemented. It works against the virtual T1 only:

| Command | What it does |
|---|---|
| `version` | Version, commit, build target and model schema IDs |
| `doctor` | Checks the host platform, `ssh` and `python3`, QEMU, the virtual-devices checkout, the development SSH key and the virtual device's image. Exits 1 when any check is an error |
| `schema [<command>]` | Command metadata as JSON: arguments, flags, category, idempotency, output schema, error codes, examples |
| `vm list`, `vm get [<name>]` | The virtual T1 instance, its state (`stopped`, `booting`, `running`) and endpoints |
| `vm start [<name>]`, `vm stop [<name>]` | Boots the virtual T1 and waits until SSH answers (`--detach` returns at once, `--wait-timeout` defaults to `180s`); stops it. Both are idempotent |
| `device list` | Every known T1 with its transport, state and mode. Today only `vm:default` |
| `exec [-- cmd…]` | Runs a command on the target over SSH, or opens a shell when none is given |
| `screen capture [file]` | Saves the display as a 320 × 480 PNG |
| `input send key <button>` | Presses `action`, `vol-up`, `vol-down` or `power` (`--hold-ms`, default 100) |
| `input send tap <x> <y>` | Touches the display at a pixel |

Every other command in the spec is not implemented yet.

`input send tap` goes through the virtual device's tablet, which the shell treats as one finger.

## Build

```sh
cargo build            # target/debug/opendap
cargo test
```

## Usage

```sh
opendap doctor
opendap vm start
opendap device list
opendap exec -- uname -a
opendap screen capture /tmp/t1.png
opendap input send key vol-up
opendap vm stop
```

### Virtual T1 backend

Virtual devices run from a virtual-devices checkout, through its tool `odap-vdev` (status as JSON, lifecycle, management link, screen and input). The CLI finds the checkout in `$ODAP_VIRTUAL_DEVICES_DIR`, or through the workspace's `repos.tsv` above the current directory.

The image is built by maintainers with toolkit-image `odap-image build qemu` (releases will come through `opendap image pull`). While it is missing, `doctor` reports it and `vm start` fails with `vm.image_missing`.

### Shared conventions

- **Global flags:** `-t/--target <selector>` (env `OPENDAP_TARGET`), `-o/--output human|json|ndjson|plain` (env `OPENDAP_OUTPUT`), `--json`, `--no-input` (env `OPENDAP_NO_INPUT=1`), `-n/--dry-run`, `-V/--version` (bare version).
- **Output:** stdout carries data only; notes and errors in text go to stderr. Without `--output`, the output is human on a TTY and JSON when stdout is piped or an agent is detected. Every JSON document has a `schema` (`opendap.<kind>/v1`); lists are `{"items":[…]}`; mutations carry `changed`; `--dry-run` prints a `plan` and changes nothing. Human output may change between versions; scripts should use JSON.
- **Errors:** in JSON modes, stdout carries `{"schema":"opendap.error/v1","ok":false,"error":{code, message, hint, fix, retryable, details, docs}}`. Codes are stable and listed per command by `opendap schema`.
- **Exit codes:** 0 success, 1 failure, 2 usage, 3 not found, 8 timeout, 10 precondition failed, 11 unsupported (the full table is in the research report, §8.4).
- **Targets:** selectors are `vm:<name>`, `usb:<serial>`, `net:<host>[:port]`, `serial:<path>` and `maskrom:<bus>-<addr>`. Only `vm:default` is reachable today; the other kinds fail with `target.unsupported`. Without `--target`, the single running T1 is used.
- **`exec`:** each argument reaches the device unchanged (it is quoted for the remote shell); use `exec -- sh -c '…'` for pipelines. In human and plain modes the remote exit code is returned. In JSON mode the result carries `exit_code`, `stdout` and `stderr`, and the process exits 1 when the remote command failed. A shell without a command needs a TTY; with `--no-input`, `OPENDAP_NO_INPUT=1`, JSON output or no TTY it fails with `input.missing`.
- **Prompts:** none in this slice. Interactive sessions are allowed only when stdin and stderr are TTYs, no agent is detected, `CI` is unset and `--no-input` is not set.

## Layout

- `Cargo.toml`, `crates/` — the Rust workspace, layered as the [Rust crate architecture](https://github.com/opendap-project/system/blob/main/docs/specs/rust-crate-architecture.md) sets out
- `templates/` — project templates the CLI generates
- `docs/` — requirements, the [CLI spec](docs/specs/opendap-cli.md) and research

### Crates

| Crate | Layer | Contents |
|---|---|---|
| `opendap-model` | Domain model | Types and schema IDs: devices, targets, virtual T1 instances, exec results, captures, input events, findings, error codes and errors as data |
| `opendap-core` | Application core | Use cases (resolve a target, list devices, list, get, start and stop the virtual T1, exec, capture the screen, send input, doctor) written against ports: `VmProvider`, `VmBackend`, `DeviceTransport`, `ScreenSource`, `InputSink`, `HostProbe`, `Clock`. The `contract` feature exposes one test suite per port |
| `opendap-transport-vm` | Adapter | Virtual devices through virtual-devices' `odap-vdev`, discovery of its checkout, and their `doctor` checks |
| `opendap-transport-ssh` | Adapter | Commands over SSH, each argument quoted for the remote shell |
| `opendap-host` | Adapter | Program lookup, the system clock and the host's `doctor` checks (platform, programs, QEMU) |
| `opendap-cli-kit` | Interface | Conventions of the toolkit CLIs: output modes, the error envelope, exit codes, command metadata, agent detection |
| `opendap-cli` | Interface | The `opendap` binary: the command tree, command metadata (`src/meta.rs`), wiring of the adapters into the core, rendering |

Dependencies point inward only: the model depends on no other crate here; the core on the model; adapters on the core and the model (the transports also use `opendap-host` to find programs); the interface crates on everything above. The model and the core do no I/O and hold no platform code. Platform-specific code lives in the adapters, which support POSIX hosts.

Tests: unit tests of the core run against fake adapters; each adapter runs the contract suite of the ports it implements; `crates/opendap-cli/tests/` checks the binary. End-to-end checks run against the virtual T1 by hand (see [Usage](#usage)).

## License

This repository follows the [REUSE](https://reuse.software) layout: license texts live in `LICENSES/` and `REUSE.toml` maps each path to its license.

- Default (the `opendap` binary): **GNU GPL v3.0 or later** (`GPL-3.0-or-later`)
- The library crates (`opendap-model`, `opendap-core`, `opendap-transport-vm`, `opendap-transport-ssh`, `opendap-host`, `opendap-cli-kit`): **Mozilla Public License 2.0** (`MPL-2.0`), libraries shared with OpenDAP Desktop and plugins
- Markdown files and `docs/`: **Creative Commons Attribution-ShareAlike 4.0** (`CC-BY-SA-4.0`)
- `templates/`: **MIT, so generated projects are not bound to the toolkit's license** (`MIT`)

Copyright © 2026 The OpenDAP Project.
