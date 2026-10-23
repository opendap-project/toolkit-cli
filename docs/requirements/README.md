# Toolkit CLI requirements

Requirements for the `opendap` command-line tool. TK-001, TK-009, TK-010 and TK-011 are shared by every toolkit repository; the IDs keep the `TK` prefix of the original toolkit repository.

| ID | Requirement | Priority | Status |
|---|---|---|---|
| TK-001 | The toolkit runs on the developer host and is not shipped in Blackbird OS. | must | Confirmed |
| TK-002 | The T1 shall have its own ADB-style command-line tool covering everything an advanced user may want to do with the device. | must | Confirmed |
| TK-003 | The CLI command shall be `opendap`, with the aliases `dap` and `odap` installed only when those names are free on the host. | must | Confirmed |
| TK-004 | The CLI shall be a single Rust binary, portable or built per operating system. | must | Confirmed |
| TK-005 | The CLI shall install and uninstall through a `curl … \| sh` script that detects the environment and selects the right binary. | must | Confirmed |
| TK-006 | The install script shall install the latest version by default and allow pinning a specific version. | must | Confirmed |
| TK-007 | The install script shall support the usual options: uninstall, verbose output and non-interactive mode, among others. | must | Confirmed |
| TK-008 | The CLI shall include a `doctor` command that checks its environment. | must | Confirmed |
| TK-009 | Every toolkit tool shall be agentic-first and human-friendly, and all of them shall share concepts, conventions and one domain model. | must | Confirmed |
| TK-010 | Toolkit tools and development in general shall officially support only UNIX / POSIX systems. | must | Confirmed |
| TK-011 | Toolkit tools shall be tested on macOS only. | must | Confirmed |

## Context

- TK-002: ADB (Android Debug Bridge) is the reference for scope: shell, file transfer, installing and removing software, logs, port forwarding, screenshots and input, rebooting into recovery or flashing modes. The command set is not defined yet.
- TK-009: references for CLI design suggested by the maintainers: Cloudflare's new `cf` CLI and Forge. Research: `docs/research/2026-10-agentic-cli-design.md` (in progress).
