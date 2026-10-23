# CLI

The `opendap` command-line tool: OpenDAP's adb, agentic-first, for advanced users and package developers working with OpenDAP devices, real or virtual. Written in Rust; it sits next to OpenDAP Desktop, not in the Developer Toolkit.

Part of [OpenDAP](https://github.com/opendap-project). This repository is open source.

## Working here

- Write everything in English: code, docs, commit messages.
- Record only information the maintainers confirmed. Do not infer specs, numbers or wording.
- Project context is in `docs/`: requirements in `docs/requirements/`. Read them before changing anything; update them with the `/opendap:requirement` and `/opendap:adr` skills.
- Licensing follows REUSE (`REUSE.toml`, `LICENSES/`). After adding files or folders, run the `/opendap:license-check` skill.
- This is the user-side CLI (like adb), not the Developer Toolkit: maintainer tooling belongs in the toolkit repositories. Use other repositories (virtual-devices `odap-vdev`, toolkit tools) through their documented interfaces, not their internals.
