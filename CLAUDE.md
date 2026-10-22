# Toolkit CLI

The `opendap` command-line tool: an ADB-style, agentic-first CLI for the T1 and OpenDAP OS, written in Rust.

Part of [OpenDAP](https://github.com/opendap-project). This repository is open source.

## Working here

- Write everything in English: code, docs, commit messages.
- Record only information the maintainers confirmed. Do not infer specs, numbers or wording.
- Project context is in `docs/`: requirements in `docs/requirements/`. Read them before changing anything; update them with the `/opendap:requirement` and `/opendap:adr` skills.
- Licensing follows REUSE (`REUSE.toml`, `LICENSES/`). After adding files or folders, run the `/opendap:license-check` skill.
- The toolkit is one repository per tool domain under `public/toolkit/` (`cli`, `hooks`, `host`, `hardware`, `verify`). Use another toolkit repository through its documented interface, not its internals.
