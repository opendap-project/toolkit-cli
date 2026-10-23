# `opendap` CLI

- **Status:** Accepted (2026-10-22), with the open questions resolved as recommended (§7)
- **Date:** 2026-10-21
- **Requirements:** TK-002 to TK-011
- **Research:** [`../research/2026-10-agentic-cli-design.md`](../research/2026-10-agentic-cli-design.md)

The ADB-style tool for everything an advanced user, a developer or an agent does with a T1, real or virtual. It is also the front door of every toolkit tool.

## 1. Principles

- **Agentic-first, human-friendly.** Every command works without a TTY, has JSON output, a `--dry-run`, stable error codes with a hint and a fix, and machine-readable help. Humans get readable tables, colour and confirmations.
- **One domain model.** Device, target, transport, image, capability, chip, job and finding are shared types (crate `opendap-model`, versioned JSON schemas `opendap.<kind>/v1`). Help, schemas, MCP tools, docs and the conformance test are generated from command metadata, following Cloudflare Forge.
- **Standard and trick-less.** SSH, USB, rockusb, QMP, journald, systemd and RAUC underneath. No proprietary daemon is required in v0.
- **UNIX / POSIX only, tested on macOS** (TK-010, TK-011).

## 2. Shared conventions

Every toolkit tool adopts §8 of the research report as written: grammar, global flags, output, errors and exit codes, interactivity, targets, configuration, authentication, logging, help for agents, versioning and install. In short:

- Grammar `opendap <noun> <verb> [subject] [--flags]`, with a closed verb vocabulary. ADB-style shortcuts (`shell`, `push`, `pull`, `logs`, `devices`, `flash`, `screenshot`) are documented aliases of canonical commands.
- Global flags: `-t/--target`, `--context`, `-o/--output human|json|ndjson|plain` (`--json`), `--fields`, `-n/--dry-run`, `-f/--force` (the only way to skip confirmations), `--no-input`, `-q`/`-v`, `--log-format`, `--color`, `--config`, `--offline`, `--timeout`.
- stdout carries only data. JSON lists are `{"items":[…]}`, and every document has a `schema`. Streams are NDJSON whose last line is `result` or `error`.
- Errors carry a stable dotted `code`, `hint`, `fix`, `retryable` and `docs`. There are 13 coarse exit codes (0–11, 130).
- Target selectors: `usb:<serial>`, `serial:<path>`, `net:<host>`, `vm:<name>`, `maskrom:<bus>-<addr>`, or a context name. A single candidate is used automatically, except for destructive operations.
- XDG paths on macOS too. One shared `config.toml`, with a table per tool.

## 3. Architecture

- **One front door, external plugins.** `opendap` is a single Rust binary. Heavier toolkit tools ship as `opendap-<tool>` binaries and run as external subcommands (the cargo and git pattern): `opendap image build` → `opendap-image build`. The device CLI stays small, and every tool shares one grammar and one `cli search`.
- **Crates:** `opendap-model` (types and schemas), `opendap-cli-kit` (flags, output, errors, targets, config, logging, agent detection, help, schema, MCP, `doctor` framework, self-update), and one crate per binary. Use cases live in `opendap-core` and its adapter crates, layered as the [Rust crate architecture](https://github.com/opendap-project/system/blob/main/docs/specs/rust-crate-architecture.md) sets out.
- **Transports in v0:**
  - SSH, over the network or over USB networking from the main USB-C in gadget mode.
  - Serial console, over the same USB-C path (BRD-039).
  - rockusb, for MaskROM flashing.
  - QMP, VNC and SSH, for the virtual T1.
- **No host daemon in v0.** Each invocation talks to the device directly.

## 4. Installer

```sh
curl --proto '=https' --tlsv1.2 -fsSL "$OPENDAP_INSTALL_BASE/install.sh" | sh -s -- [options]
```

| Option | Env | Behavior |
|---|---|---|
| (none) | | Install the latest release for the detected OS and architecture into `~/.local/bin` |
| `--version X.Y.Z` | `OPENDAP_VERSION` | Pin a specific release instead of the latest |
| `--uninstall` | | Remove the binary, the aliases it created and its receipt. `--purge` also removes config, state and caches |
| `--yes` | `OPENDAP_NO_INPUT=1` | Non-interactive: never prompt |
| `--verbose` | | Print every step |
| `--install-dir DIR` | `OPENDAP_INSTALL_DIR` | Install location |
| `--no-modify-path` | | Do not edit shell profiles |
| `--aliases auto\|none` | | `auto` (default): create `dap` and `odap` only if those names are free on the host; never overwrite |
| `--json` | | Machine-readable result for agents |

Behavior:

- POSIX `sh`, wrapped in a function so a partial download cannot run.
- Detects OS (macOS, Linux), architecture (arm64, x86_64) and libc. Linux binaries are static musl.
- Verifies the sha256 checksum and the minisign signature of the checksum file before installing.
- Writes a receipt that `opendap self update` and `opendap self uninstall` use.

## 5. Essential commands (v0)

Canonical form first, then the ADB-style shortcut where one exists.

### Targets and devices

| Command | Shortcut | What it does |
|---|---|---|
| `device list` | `devices` | Every reachable T1 (USB, network, MaskROM) and virtual T1, with mode and transport |
| `device get` | | Model, revision, serial, Blackbird OS version, mode, battery, storage, attached faceplate and HAT |
| `device wait --mode <mode>` | | Waits until the target reaches a mode (`normal`, `recovery`, `maskrom`, `gadget:*`) |
| `device reboot [--mode normal\|recovery\|maskrom]` | `reboot` | Reboots, optionally into another mode |
| `device pair` / `device unpair` | | Exchanges the host key with the device, which confirms on screen or over the console |
| `context list\|get\|create\|use\|delete` | | Named target and defaults bundles |

### Access

| Command | Shortcut | What it does |
|---|---|---|
| `exec [-- cmd…]` | `shell` | Interactive shell, or runs a command and returns its output and exit code |
| `file push <local> <remote>` / `file pull <remote> <local>` | `push` / `pull` | Copies files; idempotent by hash |
| `file list <path>` | | Lists a remote directory |
| `log watch` / `log get` | `logs` | Streams or fetches the system journal, filterable by service and priority |
| `forward create <local> <remote>` / `forward list` / `forward delete` | | Port forwarding between host and device |
| `screen capture [file]` | `screenshot` | Captures the display |
| `input send key <button>` / `input send tap <x> <y>` | | Injects button presses (`action`, `vol-up`, `vol-down`, `power`) and touches |

### Software and system

| Command | Shortcut | What it does |
|---|---|---|
| `capability list\|get\|install\|uninstall\|update` | | Third-party capabilities, the T1's equivalent of apps |
| `os get` | | Blackbird OS version, A/B slot, pending update |
| `os check` / `os install <bundle\|version>` | | Looks for an update / applies an OTA bundle, with rollback |
| `service list\|get\|start\|stop\|restart` | | System services |

### Images and flashing

| Command | Shortcut | What it does |
|---|---|---|
| `image list` | | Available Blackbird OS releases |
| `image pull <version>` | | Downloads and verifies a release image |
| `image flash <file\|version>` | `flash` | Flashes over MaskROM / loader or USB mass storage. Destructive: requires an explicit target and confirmation (type the serial) or `--force` |

### Peripheral hosting (OS-008)

| Command | What it does |
|---|---|
| `chip list` | Every programmable chip on the board and on attached HATs: RP2350 coprocessor, HAT microcontrollers, FPGAs, other SoCs |
| `chip get <chip>` | Identity, firmware version, how it is reached (SWD, JTAG, USB, SPI) |
| `chip flash <chip> <firmware>` | Flashes firmware from Blackbird OS. Destructive |
| `chip restart <chip>` | Resets a chip, optionally into its boot mode |
| `chip start <chip> --debug` | Starts a debug server (GDB) on the device and forwards it to the host |
| `hat get` | EEPROM contents, capability manifest and signature status |
| `faceplate get` | Identity and controls (HID descriptor) |

### Virtual T1

| Command | What it does |
|---|---|
| `vm list` / `vm get` | Virtual T1 instances and their endpoints (SSH, VNC, QMP) |
| `vm start` / `vm stop` | Boots or stops the virtual T1 headless. Every other command works against it with `--target vm:<name>` |

### Host and CLI

| Command | What it does |
|---|---|
| `doctor` | Checks the host: binary and plugins, USB permissions, transports, SSH keys, QEMU, Lima, KiCad, FreeCAD, Blender, pre-commit and toolkit versions. Every finding has a severity and a fix, plus `--json` and `--report` |
| `self update` / `self uninstall` / `self alias` | Same behavior as the installer, using its receipt |
| `cli search "<task>"`, `schema [<command>]`, `tools`, `help errors [<code>]` | Discovery for agents and humans, generated from command metadata |
| `mcp serve` | MCP server over stdio, with a small tool set: search, describe, run, plus `device_list` and `doctor` |
| `version`, `completion <shell>` | Version details (`--json`) and shell completions |

### Plugins (existing toolkit scripts, to be migrated)

| Command | Binary | Replaces |
|---|---|---|
| `image build [--profile qemu\|oci\|t1]` | `opendap-image` | `blackbird-os/build/dap-image.sh` |
| `vm start\|stop` (backend) | `opendap-image` | virtual-devices `build/dap-qemu.sh`, `dap-qmp.py` |
| `fab package <project>` | `opendap-fab` | `toolkit/hardware/tools/fab-package.sh` |
| `export build` | `opendap-export` | `toolkit/hardware/tools/freecad-export.py` |
| `security check` | `opendap-image` | `blackbird-os/build/security-check.py` |
| `verify …` | to be defined | verification instrumentation (in progress) |

## 6. Release

Built locally (no CI) for macOS arm64 and x86_64 and Linux arm64 and x86_64 (static musl). Packaged with `dist` in local mode, signed with minisign, and published as an immutable GitHub release (draft, upload, publish) with `gh`, through `/opendap:release`.

## 7. Decisions (2026-10-22)

1. Piped stdout defaults to JSON (agent-first, like `cf`); humans pass `-o plain` for greppable text.
2. One front door, `opendap`, with external `opendap-<tool>` plugins.
3. Every destructive operation needs an explicit target, even when only one device is present.
4. v0 has no dedicated device-side agent: SSH and standard tools on the device.
5. No Apple Developer ID for now; revisit for browser downloads, a `.pkg` or a Homebrew cask.
6. Packaging with `dist` in local mode.
