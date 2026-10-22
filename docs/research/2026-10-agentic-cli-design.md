# Research: agentic-first CLI design for the OpenDAP toolkit

- **Date:** 2026-10-21
- **Status:** research input, not a decision. Every item labelled **Recommendation** is a proposal for the maintainers to accept or reject.
- **Scope:** the developer and power-user CLI `opendap` (ADB-like, single Rust binary, aliases `dap` and `odap` only when free), and the conventions it should share with the other toolkit tools (image builder, fab package, export, security check, and later tools).
- **Host scope (maintainers' instruction, 2026-10-21):** UNIX/POSIX hosts only. macOS is the only tested host. Linux is supported but untested. Windows and PowerShell are out of scope, so Windows builds and installers are left out of every recommendation below.
- **Placeholders:** this report uses `$OPENDAP_INSTALL_BASE` for the install and download base URL and `<org>/<repo>` for the release repository. It names no project domain.
- **Marking:** **UNVERIFIED** means a claim could not be confirmed from a primary source in this pass. Versions are the latest found on 2026-10-21, mostly from the GitHub releases API, crates.io or npm.

---

## 0. Executive summary

1. **Cloudflare `cf` is the clearest public reference for an agent-first CLI in 2026.** It was a technical preview in April 2026 and was relaunched in late September 2026 as "the agentic CLI for the entire Cloudflare API". What to take from it:
   - one schema generates every interface;
   - the grammar is fixed: `get` and never `info`, `--force` and never `--yes`;
   - JSON is the default output;
   - `--dry-run` is available on every generated command;
   - stdout carries data only and stderr carries everything else;
   - agents are detected through environment variables and given a different help preamble;
   - commands can be searched by intent (`cf cli search`);
   - a machine-readable `cf schema` and `cf tools` (MCP tool definitions) are built in;
   - non-interactive runs never block on a prompt.

   Its weak spots are as instructive:
   - errors are human-rendered boxes with a single exit code 1;
   - there is no NDJSON;
   - telemetry is opt-out.
2. **"Forge" most likely means Cloudflare Forge.** It is Cloudflare's schema-first generation pipeline (Rust core, Apache-2.0, open-sourced 2026-09) that generates `cf`. It is the one candidate that is directly about "a shared domain model that generates many consistent interfaces". Atlassian Forge CLI is a distant second. Laravel Forge and Foundry `forge` are not relevant (§2).
3. **OpenDAP's problem is a device-and-target problem, not a cloud API problem.** Most of the command model therefore comes from:
   - `adb`: the host↔device transport model and serial selection;
   - `kubectl`: contexts, resource grammar, `-o`, `--dry-run`, `explain`;
   - `probe-rs` and `fastboot`.

   The output, error, help and agent conventions come from `cf`, `gh` and clig.dev.
4. **Distribution without CI is practical.** Build the binaries locally from the Mac:
   - macOS arm64 natively;
   - macOS x86_64 with `rustup target add`;
   - Linux x86_64 and aarch64 musl-static with `cargo-zigbuild`.

   Then publish with `gh release` as a **draft → upload → publish** sequence on an **immutable release**. GitHub then produces a release attestation with no Actions involved. Ship `SHA256SUMS` signed with **minisign**. Downloads made with curl carry no `com.apple.quarantine` attribute, so Gatekeeper does not assess them. Ad-hoc linker signatures are enough on Apple Silicon. Both points were verified locally (§5.3).
5. **A shared kit crate plus one schema.** Recommendation: build one `opendap-cli-kit` crate and one `opendap-model` crate (serde + schemars). The kit carries global flags, the output renderer, the error envelope and exit codes, target selection, the doctor framework, agent detection, schema and help export, the MCP adapter and self-update. Every toolkit tool depends on both crates. A conformance test walks each clap tree and enforces the grammar, the way Forge does (§7).

---

## 1. Cloudflare `cf` CLI

### 1.1 What it is and when it shipped

| Item | Fact | Source |
|---|---|---|
| Technical preview | Announced **2026-04-13** (Agents Week) by Matt "TK" Taylor and Dan Carter: "a new unified CLI designed for consistency across the Cloudflare platform", covering nearly 3,000 API operations. Installed with `npx cf` / `npm install -g cf`. | [blog.cloudflare.com/cf-cli-local-explorer](https://blog.cloudflare.com/cf-cli-local-explorer/) |
| Launch post | "Introducing cf" by Matt "TK" Taylor, dated **2026-09-28** on the fetched page. **UNVERIFIED:** the fetch tool returned the same date for the Forge post, so check it. The post claims to cover "the entirety of the Cloudflare API surface of over 3,000 operations" and offers `cf migrate` from Wrangler with 18 months of Wrangler maintenance. | [blog.cloudflare.com/cloudflare-cf-cli-launch](https://blog.cloudflare.com/cloudflare-cf-cli-launch) |
| Agent share of usage | "In March 2026, agents were responsible for a quarter of Wrangler use", rising to "48%" by the launch. Agents were "almost four times as likely to use six or more commands". | same |
| Repository | `cloudflare/cf`: "The agentic CLI for the entire Cloudflare API". Created 2026-09-21, MIT OR Apache-2.0. | [github.com/cloudflare/cf](https://github.com/cloudflare/cf) |
| Current version | npm `cf` **1.0.0-beta.20** (2026-10-20). Binaries `cf` and `cloudflare`. Node-based, with a single-binary build proposed but not shipped. | [npmjs.com/package/cf](https://www.npmjs.com/package/cf) |
| Relationship to Wrangler | "cf IS the next version of Wrangler". Wrangler is at 4.151.3. | `AGENTS.md` in the repo |

The design facts below come mainly from the repository's `AGENTS.md` and `packages/cli/AGENTS.md`, read on 2026-10-21. They describe the implemented behaviour, which is more reliable than the blog posts.

### 1.2 Design principles relevant to OpenDAP

**Motivation.** "Increasingly, agents are the primary customer of our APIs." Also: "Agents expect CLIs to be consistent. If one command uses `<command> info` … and another uses `<command> get`, the agent will expect one and call a non-existent command for the other." Manually enforced consistency "is Swiss cheese", so the rules are enforced in the schema layer ([April post](https://blog.cloudflare.com/cf-cli-local-explorer/)).

#### Command grammar

- The shape is `cf <product> [group…] <operation>`, which is noun…verb.
- The **last** path parameter is the *subject* and becomes a positional argument. Container parameters become required `--flags`. `list` and `bulk-*` operations take no positional argument.
- The verb vocabulary is fixed. `get` is used, never `info`. `.get`/`.list`, `.create`, `.delete` and `.edit` map to GET, POST, DELETE and PATCH/PUT.
- Deprecated operations are **removed from the CLI entirely**: "they'll break at sunset; surfacing them encourages users to adopt doomed paths."
- Strict parsing: unknown flags are errors (`yargs.strict()`).
- A small set of hand-written root commands (`init`, `dev`, `build`, `deploy`, `auth`, `docs`, `cli`, `schema`, `tools`) sits next to the generated tree. They must not collide with generated names.

#### Output

- The design moved during 2026. The April post said "Always `--json`, never `--format`." The September implementation makes **JSON the default, with no global `--json`, `--ndjson`, `--pretty` or `--format`**. Those flag names are reserved for the future. The README says JSON is "pretty printed for humans and condensed for agents for maximum context savings".
- stdout carries **only** command output. "Every message goes to stderr."
- A mutation that returns null prints **nothing** on stdout, so `jq` never sees `null`. A TTY gets a `✓ <label>` line on stderr.
- Binary and text responses are written verbatim to stdout. `--text` decodes them.
- Colour and syntax highlighting turn off for non-TTY or CI, even when `FORCE_COLOR` is set ("Non-interactive output must remain parseable"). `NO_COLOR` is honoured. Spinners need a TTY and stop under `CF_QUIET=1`.
- There is no NDJSON. Users are told to pipe through `jq -c '.[]'`.
- List commands return a single page, deliberately.

#### Non-interactive behaviour

- A missing required field opens a prompt **only when stdin is a TTY**. Otherwise it fails with `--<flag> is required`, and for enums the error lists the valid choices.
- Secrets marked `x-sensitive` may be piped on stdin (`echo … | cf workers secrets update X`).
- Destructive operations ask for confirmation. Only `--force`/`-f` skips it, and `--quiet` never confirms. Since a recent beta, a non-interactive run without `--force` **exits 1 without mutating anything** and prints the "pass --force" hint. Before, it reported success after aborting (`.changeset/noninteractive-abort-exit.md`).
- A destructive POST or PUT is flagged destructive by a schema annotation (`x-forge-require-confirmation: 'This operation …'`), not by guessing from the HTTP verb.
- `cf init` asks for a directory when interactive. Scripts must pass one (`cf init .`).

**Dry-run.** "Every generated command gets `--dry-run` (mutating and read alike — reads show URL/method/path params without a body preview)."

#### Input ergonomics

- `--body` is an escape hatch next to the per-field flags.
- `@path` ingestion works on any string flag (`--flag @file`, read as text, binary, base64 or JSON).
- Array flags accept several values (`--keys k1 k2`).

#### Authentication and context

- Token resolution order: `CLOUDFLARE_API_TOKEN` → OAuth profile (`--profile`, then the nearest directory binding, then the default).
- OAuth uses the **Device Authorization Grant (RFC 8628) by default**, so it works over SSH, in containers and in Codespaces. The localhost PKCE callback is an opt-out (`--no-device`). `--use-keyring` stores credentials encrypted in the OS keychain.
- There is deliberately **no `--api-token` flag**, which keeps secrets out of `ps` and shell history.
- Every value (account, zone, region) has a documented precedence chain.
- `--local` routes a command to a local simulator. "Commands with no local explorer equivalent return an error instead of falling through to production." Output "clearly signals whether commands are applied to remote or local resources."

**Error model.** This is the weakest part.

- A single central `handleError` renders a boxed report on stderr: `[code] message`, then a dim `HTTP <status> · <path>` line.
- The process exits **1** for every failure.
- There is no machine-readable error document on stdout.
- The repository forbids per-error-code switches in the CLI: "surface product knowledge via forge annotations or push fixes upstream into the canonical API error message."

The lesson for OpenDAP is to keep the remediation knowledge in the shared model, but to add the structured error envelope and the stable exit codes that `cf` lacks.

#### Discovery and help for agents

- `detectAgentContext()` reads `AI_AGENT`, `CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CODEX_THREAD_ID`, `CURSOR_AGENT`, `GEMINI_CLI`, `OPENCODE`, `COPILOT_AGENT`, `AMP_CURRENT_THREAD_ID`, `QWEN_CODE_*` and others (`packages/cli/src/lib/agent-context.ts`). In this research session's shell, Claude Code exported `AI_AGENT=claude-code_<version>_agent` and `CLAUDECODE=1`.
- When an agent is detected, **every help screen starts with a discovery banner**. The banner tells agents not to chain nested `--help` calls. It points them to `cf cli search "<task>"`, which returns 5 compact JSON matches, then to `<cmd> --help`, then to `cf schema <cmd>` for the exact API request. It also tells them to keep search queries free of identifying data.
- `cf docs search "<question>"` needs no login. Its results are fetched with `Accept: text/markdown`.
- `cf schema [command..]` is "for agent introspection. Always outputs JSON."
- `cf tools` emits **MCP tool definitions** (`{name, description, inputSchema}`) derived from the same command metadata (`_meta/commands.json`).
- Shell completion (`cf complete bash`) reads the same metadata.
- Cloudflare's MCP approach, "Code Mode", exposes the whole API through **two tools, `search()` and `execute()`, in about 1,000 tokens** instead of about 1.17 M tokens for one tool per endpoint ([Code Mode MCP, 2026-02-20](https://blog.cloudflare.com/code-mode-mcp/)).

#### Idempotency and safety

- Bulk operations split into batches automatically, driven by the schema's `maxItems`.
- Patches are deferred "so later failures remain retryable".
- Persistent state must stay cross-version compatible, because a global `cf` may delegate to a project-pinned `cf`.

#### Telemetry

- On by default, with an opt-out (`cf cli telemetry disable`, `CF_SEND_TELEMETRY=false`, `DO_NOT_TRACK=1`).
- Records the detected agent harness ([telemetry.md](https://github.com/cloudflare/cf/blob/main/packages/cli/telemetry.md)).

clig.dev disagrees and asks for opt-in. This report recommends opt-in (§8.11).

**Cross-platform rule.** `cf` treats signals, exit codes, quoting, paths and terminal behaviour per OS. For OpenDAP this narrows to macOS vs Linux differences in USB access, serial device names and `sed -i`.

**Distribution reasoning.** The repo's AGENTS.md explains why `cf` is **not** Rust or Go: its TypeScript dependencies. It estimates a Go or Rust binary at "~15–25 MB" against 245–265 MB for an embedded-Node build. OpenDAP has no such constraint, so Rust is the right choice.

### 1.3 Takeaways for OpenDAP

**Recommendation:** adopt these from `cf`:

- the fixed verb vocabulary;
- the stdout/stderr split;
- prompts only on a TTY;
- `--force` as the single bypass, failing closed when non-interactive;
- `--dry-run` everywhere;
- `@file` inputs;
- the agent-detection help banner;
- `search`, `schema` and `tools` introspection commands;
- device-flow-style pairing that works over SSH;
- local vs remote stated explicitly in output, which for OpenDAP means virtual T1 vs physical device.

Improve on `cf` in four places: a structured error envelope, distinct exit codes, NDJSON for streams, and opt-in telemetry.

---

## 2. "Forge": which one is meant

| Candidate | What it is | Relevance |
|---|---|---|
| **Cloudflare Forge** | "a schema-first OpenAPI code generation and surface tooling framework … resolving OpenAPI 3.x specifications, applying JSONPath-based overlays, and generating typed SDKs, CLI interfaces, runtime helpers, and documentation" ([README](https://github.com/cloudflare/forge)). Open-sourced in late September 2026 under Apache-2.0 ([blog post, dated 2026-09-28 on fetch](https://blog.cloudflare.com/forge-open-source-generation-pipeline)). The core is **Rust** (`Cargo.toml`: clap 4.6.7, rayon, serde, and **wasmtime 49 component-model plugins**). `cf` is its first output. Further targets (TypeScript, Rust, Python, Go and PHP SDKs, Terraform, docs, MCP servers) are planned. | **Highest.** It is the "shared domain model → many consistent interfaces" idea, it sits in the same family as `cf`, and it is written in Rust. |
| Atlassian Forge CLI | The CLI for Atlassian's app platform (`forge create/deploy/tunnel/install`). | Low. It is a deployment CLI for one platform and shows no shared-model design. Its non-interactive and CI flags could not be confirmed (**UNVERIFIED**). |
| Laravel Forge CLI | A server-provisioning SaaS CLI. | Not relevant. |
| Foundry `forge` | The Ethereum build and test tool. | Not relevant beyond being a well-made Rust CLI. |

**Conclusion:** the maintainers most likely means **Cloudflare Forge**, since it is mentioned with `cf`. The ideas to take from it:

1. **One schema, many targets.** The CLI, SDKs, docs and MCP tools are all generated from one source. Generated outputs can feed further generators ("Forge transformers can be chained: generate outputs from other outputs").
2. **"CLIs are different."** A CLI has local-only behaviours (wizards, confirmations, file ingestion, local simulators) that make no sense in an SDK. Forge expresses them as **method-level annotations** in the schema rather than as ad-hoc switches in CLI code:
   - `x-forge-require-confirmation`
   - `x-sensitive`
   - `x-forge-aliases`
   - `x-forge-hidden`
   - `x-forge-epilogue`
   - `paramOverride.positional`, `.fromFile`, `.default`
   - `status: deprecated`
3. **The decision rule** (quoted from the `cf` AGENTS.md):

   > Before hand-wiring a feature into cf, ask: "Should users of the API also have access to this?" If yes: it belongs at the platform/API layer … If no (truly CLI-only, like interactive wizards): it belongs in forge as a method-level annotation so every future generated CLI can emit the same affordance consistently.

4. **Consistency is linted, not reviewed.** Forge "lints every change, and then generates preview builds of the CLI, docs, and SDKs".
5. **Generated surface is never hand-edited.** Hand-written commands are registered in one typed registry, which is merged back into the same metadata (`_meta/commands.json`). That keeps help, completion, search, `schema` and `tools` complete.

**What OpenDAP should take:** not Forge itself. OpenDAP has no large HTTP API to generate from. The *pattern* is what transfers: one Rust model crate with annotations, from which the clap tree, JSON Schemas, MCP tool definitions, man pages, completions and `llms.txt` are all derived (§7).

---

## 3. Other references

| Tool (version, 2026-10-21) | Idea worth taking | Source |
|---|---|---|
| **GitHub `gh`** v2.103.0 | `--json <fields>`. Run `--json` with no fields to **list the available fields**. `--jq` has jq built in, so no `jq` install is needed. `--template` uses Go templates. Documented exit codes: **0 ok, 1 failure, 2 cancelled, 4 auth required**. | [formatting](https://cli.github.com/manual/gh_help_formatting), [exit codes](https://cli.github.com/manual/gh_help_exit-codes) |
| **kubectl** v1.37.1 | `kubectl <verb> <TYPE> [NAME]`. Types accept singular, plural and short forms. `-o json\|yaml\|wide\|name\|jsonpath=\|custom-columns=\|go-template=`. `--dry-run=client\|server`. `apply` is idempotent. Contexts live in kubeconfig (`KUBECONFIG`, `--kubeconfig`). `explain` and `api-resources` give self-description. | [kubectl reference](https://kubernetes.io/docs/reference/kubectl/) |
| **adb** | Three parts: client, **host server** (TCP 5037, started on demand) and **device daemon**. Selection: `-s SERIAL`, `-d` (USB), `-e` (emulator), `-t ID`, `$ANDROID_SERIAL`, with `-s` overriding the variable. `devices -l` shows state (`device`/`offline`/…). `wait-for-device`, `forward`/`reverse`. Wi-Fi pairing (`adb pair`) with mDNS discovery. `exec-out` returns raw binary stdout. An RSA trust prompt appears on first USB connection. | [developer.android.com/tools/adb](https://developer.android.com/tools/adb) |
| **fastboot** | A separate tool for the bootloader mode: `flashing unlock/lock`, `flashall -w`, `boot`, `-s`. Bootloader state is a *mode* of the same device. | [source.android.com](https://source.android.com/docs/setup/test/running) |
| **probe-rs** v0.32.0 | `--probe VID:PID[:Serial]` and `--chip`. `run` doubles as a cargo runner. Config files in TOML, JSON or YAML. `serve` is a remote probe server with token auth. | [probe.rs docs](https://probe.rs/docs/tools/probe-rs/) |
| **flyctl** v0.4.120 | `fly mcp server` (experimental) exposes flyctl as an MCP server over HTTP, SSE or streaming. `--claude/--cursor/--vscode/--zed` flags write client config, and `--inspector` launches the MCP inspector. The installer creates a `fly` symlink to `flyctl` (`ln -sf`), which is precedent for an alias created at install time. | [docs.fly.io/flyctl/mcp-server](https://docs.fly.io/flyctl/mcp-server), `fly.sh` |
| **wrangler** 4.151.3 | Being folded into `cf`. Its lessons are captured in §1. | npm |
| **clig.dev** | Primary output to stdout and messages to stderr. Exit 0 or non-zero. `--json`, `--plain`. **Prompt only if stdin is a TTY**, and support `--no-input`. `-n/--dry-run`, `-f/--force`, `-q/--quiet`. Disable colour on non-TTY, `NO_COLOR`, `TERM=dumb` or `--no-color`. Config precedence: **flags > env > project > user > system**. Follow XDG. noun-verb is "more common". **Do not allow arbitrary subcommand abbreviations.** Prefer additive changes. **Telemetry opt-in.** | [clig.dev](https://clig.dev/) |
| **Anthropic, "Writing effective tools for agents"** (2025-09-11) | Namespace by service and resource. Return high-signal context, not opaque IDs. Offer a `response_format` of concise vs detailed (72 vs 206 tokens in their example). Paginate and truncate with sensible defaults. Errors should "clearly communicate specific and actionable improvements, rather than opaque error codes". | [anthropic.com/engineering/writing-tools-for-agents](https://www.anthropic.com/engineering/writing-tools-for-agents) |
| **llms.txt** (v2, 2026-08-10) | A Markdown `/llms.txt` index (H1, blockquote summary, H2 link lists), with `.md` twins of every docs page discoverable through `rel="alternate" type="text/markdown"`. | [llmstxt.org](https://llmstxt.org/) |
| **MCP** spec 2026-07-28 | JSON-RPC 2.0. Tools, resources and prompts. Extensions: **Tasks** (long-running operations with polling) and Skills over MCP. Tool annotations are "untrusted unless obtained from a trusted server". | [modelcontextprotocol.io/specification](https://modelcontextprotocol.io/specification/latest) |
| **Homebrew** 7.0.10 | `brew doctor` "will exit with a non-zero status if any potential problems are found". `--list-checks` lets a single check run on its own. `brew config` gives a bug-report dump. `--json=v2` is a **versioned JSON schema**. Variables: `HOMEBREW_NO_AUTO_UPDATE`, `HOMEBREW_NO_ANALYTICS`. | [docs.brew.sh/Manpage](https://docs.brew.sh/Manpage) |

---

## 4. Install-script best practices

### 4.1 What existing installers actually do (read from the scripts on 2026-10-21)

| Script | Shell | Platform detection | Integrity | Install dir | PATH | Options |
|---|---|---|---|---|---|---|
| **uv** 0.13.5 (`astral.sh/uv/install.sh`, generated by `dist`) | POSIX `sh` (dash-tested). `local` is aliased to `typeset` on ksh. | `uname`. Checks glibc via `ldd --version` against a minimum (2.17, 2.28, 2.31). **Falls back to musl-static** when glibc is too old. | **sha256 of every artifact embedded in the script** (per-release installer) | `$UV_INSTALL_DIR` → `$XDG_BIN_HOME` → `~/.local/bin` | Writes an `env` script plus a fish variant and sources it from `.profile`, `.bashrc`, `.bash_profile`, `.bash_login`, `.zshrc`, `.zshenv` and fish `conf.d`. Appends to `$GITHUB_PATH` in Actions. | `-v/--verbose`, `-q/--quiet`, `UV_NO_MODIFY_PATH` (the `--no-modify-path` flag is deprecated in favour of the env var), `UV_DOWNLOAD_URL` / `INSTALLER_DOWNLOAD_URL` (mirror), `UV_UNMANAGED_INSTALL`, `UV_DISABLE_UPDATE`. Writes an **install receipt** JSON used by the updater. The whole body lives in functions, and the last line is `download_binary_and_run_installer "$@" \|\| exit 1`. |
| **rustup** (`sh.rustup.rs`) | POSIX `sh` | `uname -m`, and on Darwin **`sysctl hw.optional.arm64` / `hw.optional.x86_64`**, because "`uname -m` can lie due to Rosetta" | TLS hardening: `curl --proto '=https' --tlsv1.2` with restricted ciphers | `$CARGO_HOME` | `--no-modify-path` | `-y/--yes`, `-q`, `-v`. `RUSTUP_VERSION` pins the version and `RUSTUP_UPDATE_ROOT` sets a mirror. Prints "Unable to run interactively. Run with -y" when there is no TTY. Wrapped in `main "$@" \|\| exit 1`. |
| **deno** 2.9.8 | `sh` | `uname -sm` | **none** | `$DENO_INSTALL` (`~/.deno`) | delegates to `deno` itself | `-y/--yes`, `--no-modify-path`, positional version. Latest comes from a `release-latest.txt` endpoint. |
| **bun** 1.4.3 | **bash** (`set -euo pipefail`) | `uname -ms`. On Rosetta, `sysctl -n sysctl.proc_translated`. AVX2 probe picks a `-baseline` build. musl detection. | **none** | `$BUN_INSTALL` (`~/.bun`) | Edits per-shell rc files and installs completions | positional tag, `GITHUB` mirror variable |
| **flyctl** 0.4.120 | `sh` | `uname -s/-m`, and **asks a server endpoint** for the right artifact | **none** | `$FLYCTL_INSTALL` (`~/.fly`) | Writes rc.d snippets | positional version, `prerel`. Extracts to a temp dir and then **`mv` atomically** so the running executable is never opened for writing. Creates the `fly` alias symlink. |

Lessons:

- Only the `dist`-generated installer (uv) verifies checksums.
- Only uv and rustup are POSIX `sh` with full function wrapping.
- bun and rustup handle Rosetta.
- uv has the most complete PATH handling.

### 4.2 Recommended installer behaviour (`$OPENDAP_INSTALL_BASE/install.sh`)

#### Recommendation, structure and safety

1. Write it in POSIX `sh` and keep `shellcheck --shell=dash` clean. Start with `set -eu`. Put everything inside functions and call `main "$@" || exit 1` on the **last line**, so a download truncated mid-stream executes nothing.
2. Use `curl --proto '=https' --tlsv1.2 -fsSL` with retries. Fall back to `wget --https-only`. Fail when neither exists.
3. Download into `mktemp -d` and clean up with `trap … EXIT INT TERM`. Verify before extracting. Extract with `--no-same-owner`, then `chmod +x`, then `mv` atomically into place. Never modify the binary after it is placed, because that would break the macOS signature (§5.3).
4. Never use `sudo` implicitly. When the target directory is not writable, stop and print the exact command to run, or suggest `--install-dir`.

**Recommendation, platform detection** (UNIX only)

- OS: `uname -s`, either `Darwin` or `Linux`. Anything else fails with a clear "unsupported host" error code. **WSL2** reports `Linux` and gets the Linux musl build, flagged as untested. Detect it from `/proc/sys/fs/binfmt_misc/WSLInterop` or a `microsoft` string in `/proc/version`, and print a note: USB passthrough needs `usbipd`, **UNVERIFIED** for MaskROM.
- Arch: `uname -m`, normalised as `arm64|aarch64 → aarch64` and `x86_64|amd64 → x86_64`. On Darwin, `sysctl -n hw.optional.arm64` gives the real CPU even under Rosetta (rustup and bun pattern).
- libc: **ship only musl-static Linux builds**, so the installer needs no glibc probe. This avoids uv's `ldd` logic, and the static binary runs on glibc and musl (Alpine) hosts alike.

**Recommendation, options.** Every flag has an environment variable, so `curl … | sh -s -- <flags>` and `VAR=… sh` both work.

| Flag | Env | Meaning |
|---|---|---|
| `--version <semver\|latest>` | `OPENDAP_VERSION` | Pin a version. The default is `latest`. |
| `--install-dir <dir>` | `OPENDAP_INSTALL_DIR` | Default order: `$XDG_BIN_HOME` → `~/.local/bin`. `/usr/local/bin` only on explicit request. |
| `--no-modify-path` | `OPENDAP_NO_MODIFY_PATH=1` | Skip rc-file edits and print the line to add instead. |
| `--aliases <auto\|none\|dap,odap>` | `OPENDAP_ALIASES` | `auto` creates `dap` and `odap` only when free (below). |
| `--yes` / `-y` | `OPENDAP_YES=1` | Assume yes. Without a TTY on stdin, never prompt and use the defaults. |
| `--uninstall` | — | Reverse everything recorded in the receipt. |
| `--verbose` / `--quiet` | `OPENDAP_VERBOSE`, `OPENDAP_QUIET` | |
| `--base-url <url>` | `OPENDAP_INSTALL_BASE` | Mirror or offline base. Also accepts `file://`. |
| `--archive <path>` | `OPENDAP_ARCHIVE` | Fully offline install from a local tarball, still checksum-verified. |
| `--json` | — | Print the final summary (version, path, aliases, PATH changes) as one JSON object on stdout, for agents. |
| `--dry-run` | — | Print the plan and change nothing. |

> Note on `--yes`: inside the CLI the confirmation bypass is `--force` (§8.5). In installers `-y/--yes` is the established convention (rustup, deno), so the script keeps it.

#### Recommendation, version resolution

- `latest` resolves through a small manifest, `$OPENDAP_INSTALL_BASE/latest.json` (`{"version":"x.y.z","date":…}`), or through the GitHub API `repos/<org>/<repo>/releases/latest`. The manifest avoids GitHub API rate limits and works with mirrors.
- Artifact URL: `$BASE/v$VERSION/opendap-$VERSION-$TARGET.tar.gz`.

**Recommendation, integrity** (layered)

1. Always: download `SHA256SUMS` from the same release and verify with `shasum -a 256` (macOS) or `sha256sum` (Linux). On mismatch, abort with exit code 1.
2. If `minisign` is on PATH: verify `SHA256SUMS.minisig` against the **public key embedded in the script** (`minisign -Vm SHA256SUMS -P <key>`). Otherwise print one line on how to verify manually. Signing works locally with minisign 0.12 and needs no CI.
3. If `gh` is present and the release is immutable: `gh release verify-asset vX.Y.Z <file>` is an optional extra check (§5.5).
4. The binary embeds the same minisign public key (`minisign-verify` crate 0.3.0). It **requires** a valid signature for every `opendap self update`, so after the first install updates no longer rely only on TLS and GitHub.

**Recommendation, PATH handling.** Copy uv's approach:

- Write `$INSTALL_DIR/env` (sh) and `env.fish`.
- Add one idempotent line (guarded by a marker comment) to `.profile`, `.bashrc`, `.zshrc` or `.zshenv`, or to fish `conf.d`, depending on `$SHELL` and which files exist.
- Skip all of it when the directory is already on PATH.
- Record every edited file in the receipt.
- Print `source …` for the current shell, because a child process cannot change its parent's PATH.

#### Recommendation, aliases created only when free

- For each of `dap` and `odap`: if `command -v <name>` resolves to anything **other than** our own binary, **or** a file exists at `$INSTALL_DIR/<name>`, skip it, report the collision (path and `--version` output when safe) and record `skipped`.
- Otherwise create a **symlink** to `opendap`, as flyctl does. The binary dispatches on `argv[0]` only for help text, so behaviour stays identical.
- Store the result in the receipt. `opendap doctor` re-checks collisions later, for example after another tool installs `dap`.
- Known risks found on 2026-10-21:
  - npm has a `dap` package (2.1.0, "Data access proxy for SSH hosts…"). Whether it installs a `dap` binary is **UNVERIFIED**.
  - crates.io `dap` is a Debug Adapter Protocol library and installs no binary.
  - No Homebrew formula is named `dap`, `odap` or `opendap`.
  - Unrelated "OPeNDAP" (the scientific data protocol; Homebrew `libdap` 3.21.1 ships `getdap` and `dap-config`) creates name and search confusion but no binary collision.

#### Recommendation, receipt and uninstall

- Write `${XDG_CONFIG_HOME:-~/.config}/opendap/install-receipt.json`: `{schema:"opendap.receipt/v1", version, target, install_dir, binary, aliases:[…], path_edits:[{file, line}], source:"install.sh", installed_at}`.
- `install.sh --uninstall` and `opendap self uninstall` read the receipt. They remove the binary, the aliases **only if they still point to us**, and the exact rc lines. They ask before removing `~/.config/opendap` and `~/.local/state/opendap`, and keep them by default; `--purge` removes them.

#### Recommendation, self-update vs package managers

- `opendap self update [--version X] [--check]` updates through the same manifest, with signature verification. It refuses, telling the user how to upgrade instead, when the receipt's `source` is not `install.sh`, for example a future Homebrew tap (`brew upgrade opendap`).
- There are two options here:
  - `axoupdater` 0.10.3 is the `dist` updater library and reads dist receipts.
  - `self_update` 1.3.1 is a generic crate.
- Recommendation: about 150 lines of own code using the shared kit. The signature check, the receipt format and the mirror support are all project-specific.
- **No automatic background update checks by default.** An optional `update.check = "weekly"` setting prints a notice on stderr and never on stdout, and is always off when an agent is detected or `CI` is set.

**Recommendation, telemetry:** none in the installer. If telemetry is ever added to the CLI, it is opt-in (§8.11).

---

## 5. Rust single-binary distribution without CI

### 5.1 Targets and naming

#### Recommendation (UNIX only)

| Target triple | Built on the Mac with | Notes |
|---|---|---|
| `aarch64-apple-darwin` | native `cargo build --release` | The primary, tested host |
| `x86_64-apple-darwin` | `rustup target add x86_64-apple-darwin` | Untested. Optionally combine both macOS builds with `lipo -create` into one universal file. |
| `x86_64-unknown-linux-musl` | `cargo zigbuild` (cargo-zigbuild **0.24.0**, 2026-10-21; needs `zig`) | Static binary. Supported, untested. |
| `aarch64-unknown-linux-musl` | `cargo zigbuild` | Static binary. Also useful for running the CLI on a DAP or Pi. |

- Artifact names: `opendap-<version>-<target>.tar.gz`, containing `opendap`, `LICENSE*`, `README.md`, and completions and man pages generated at build time. Release-level files: `SHA256SUMS`, `SHA256SUMS.minisig`, `latest.json`, `install.sh` and, optionally, `sbom.cdx.json`.
- `cross` is still at v0.2.5 (2023-02-04) and needs Docker, so cargo-zigbuild is the better local tool.
- **`dist`** (formerly cargo-dist) **v0.33.1** (2026-10-21) can do the same locally. Its docs say: "if you have both cargo-zigbuild and cargo-xwin installed on a macOS machine, you can build pretty much every target dist supports, by running `dist build --artifacts all`." Its config already models `install-path`, `bin-aliases`, `checksum` (sha256 … blake2b), `install-updater`, `min-glibc-version`, `cargo-auditable`, `cargo-cyclonedx` and `hosting = "github" | "simple"`. ([config reference](https://axodotdev.github.io/cargo-dist/book/reference/config.html)). Its shell-installer docs list a limitation, "Cannot detect situations where musl-based builds are appropriate", which looks outdated: the generated uv script does fall back to musl.
- **Recommendation:** start with a `justfile` or `xtask` (`cargo xtask dist`) that calls `cargo build` / `cargo zigbuild`, packages, hashes, signs and runs `gh release`. That gives full control over the bootstrap installer with `--version`, `--uninstall` and alias logic, which dist's per-release installer does not cover. Reassess dist once CI exists.

### 5.2 Build profile and size

#### Recommendation

- `[profile.dist]` inherits release, with `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"` and `panic = "abort"`. Keep `opt-level = 3` unless size matters more than speed.
- Use `rustls` (no OpenSSL) so the musl build stays static.
- Rough size: 5–15 MB with TLS, USB (`nusb` 0.2.8, `rockusb` 0.4.0), SSH client, serde and MCP (`rmcp` 3.6.0). **UNVERIFIED:** measure it.
- Embed `cargo-auditable` metadata so `cargo audit bin` works on releases.

### 5.3 macOS signing, notarisation and Gatekeeper with curl installs (verified locally)

- **No quarantine flag from curl.** Downloaded on this machine (macOS **27.0.1**, curl 8.7.1), a curl-fetched file carried only `com.apple.provenance` and **no `com.apple.quarantine`**. Gatekeeper's notarisation check applies to quarantined items, so a binary installed through `curl | sh` runs without Developer ID signing or notarisation. **UNVERIFIED:** whether `com.apple.provenance` will drive policy in later macOS releases. Re-test on each major macOS release.
- **Apple Silicon needs a code signature, and an ad-hoc one is enough.** A `rustc 1.98.1` build here showed `flags=0x20002(adhoc,linker-signed)`, `Signature=adhoc`. The linker signs automatically. Cross-built `x86_64-apple-darwin` binaries are also linker-signed (**UNVERIFIED** here). After `lipo`, `strip` or `install_name_tool`, re-sign with `codesign -s - -f`.
- **When signing becomes necessary:** browser downloads (quarantined), a Homebrew *cask*, or a `.pkg`. Then you need a Developer ID Application certificate, `codesign --options runtime --timestamp`, and `xcrun notarytool submit … --wait` on a zip. A bare Mach-O file cannot carry a stapled ticket, so Gatekeeper checks it online (**UNVERIFIED** against Apple's page in this pass, which did not render). A Homebrew *formula* that builds from source or pours bottles avoids the question.
- **Recommendation:** ad-hoc signatures for v0. Document that tarballs should be installed with the script or `curl`, not through a browser. Budget a Developer ID before any `.pkg` or cask.

### 5.4 Release flow without CI (`gh` 2.103.0)

#### Recommendation

```sh
# 1. build + package + hash + sign (local)
cargo xtask dist --targets aarch64-apple-darwin,x86_64-apple-darwin,x86_64-unknown-linux-musl,aarch64-unknown-linux-musl
(cd dist && shasum -a 256 *.tar.gz install.sh > SHA256SUMS && minisign -Sm SHA256SUMS)
# 2. draft first, upload everything, then publish (assets of an immutable release cannot change after publication)
gh release create vX.Y.Z --draft --verify-tag --notes-file CHANGELOG-X.Y.Z.md dist/*
gh release edit vX.Y.Z --draft=false --latest
# 3. update the bootstrap manifest
#    $OPENDAP_INSTALL_BASE/latest.json  -> {"version":"X.Y.Z", ...}
```

- `--verify-tag` aborts when the tag is not on the remote. `--fail-on-no-commits` is also available ([gh release create](https://cli.github.com/manual/gh_release_create)).

### 5.5 GitHub immutable releases

- These are attestations without Actions. "The assets and associated Git tag cannot be changed after publication." GitHub generates a **release attestation** covering the tag, the commit SHA and the assets.
- Consumers verify with `gh release verify <tag>` and `gh release verify-asset <tag> <file>` ([docs](https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases), [verify](https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/verifying-the-integrity-of-a-release)).
- Because this needs no CI, it fits the local `gh release` workflow. **Recommendation:** turn on immutable releases for the release repository.
- **cosign** v3.1.4 (`sign-blob`) is an alternative to minisign. Keyless mode needs an interactive OIDC login, which works locally, and puts a record in the Rekor transparency log. minisign is simpler, and its verification fits into the binary. **Recommendation:** use minisign now and consider cosign later.

---

## 6. `doctor` command patterns

| Tool | Pattern | Source |
|---|---|---|
| `flutter doctor` | Validators with results **success / partial / missing / notAvailable**, shown as `[✓]`, `[!]`, `[✗]`, and merged per group. `-v` for detail. `--android-licenses` is a *fix action*. **`flutter doctor` has no `--machine` flag** (only `flutter devices --machine` does), which is a gap. | flutter `packages/flutter_tools/lib/src/commands/doctor.dart`, `doctor_validator.dart` (read 2026-10-21) |
| `brew doctor` (`dr`) | Exits non-zero when problems are found. `--list-checks` and running a single check. `brew config` gives a separate environment dump for bug reports. | [Homebrew manpage](https://docs.brew.sh/Manpage) |
| `npx expo-doctor` | Checks project config and dependency compatibility. Each issue comes with "advice on how to fix it or where to find help". | [Expo docs](https://docs.expo.dev/develop/tools/) |

### 6.1 Recommended `opendap doctor`

**Checks** (each has a stable ID; this is an initial set, not a commitment):

| ID | Checks |
|---|---|
| `host.platform` | OS, arch, Rosetta, WSL. Untested-host warning on Linux. |
| `install.version` | Installed version vs `latest.json`. Skipped with `--offline`. |
| `install.path` | Install directory on PATH. Shadowing by another `opendap` earlier in PATH. |
| `install.aliases` | `dap`/`odap` state: ours, skipped because of a collision, or stale. |
| `install.integrity` | Binary hash vs receipt. Signature of the last update. |
| `config.valid` | Config files parse. Unknown keys produce a warning. |
| `usb.access` | macOS: no driver needed (`nusb`). Linux: udev rule for the OpenDAP VID:PIDs, `plugdev`/`dialout` group membership. The fix shows the exact rule file. |
| `usb.devices` | Enumerates T1 devices and their *mode*: normal (gadget: serial, network, storage), MaskROM (`rockusb`) or recovery. |
| `serial.console` | Port present and readable. Not held by another process. |
| `target.<name>.reachable` | Each configured target: SSH reachability, host-key match, device-agent version compatibility. |
| `vm.qemu` | `qemu-system-aarch64` present with version ≥ the minimum. HVF acceleration (macOS) or KVM (Linux) available. VNC and QMP ports free. Image present. |
| `disk.space` | Free space for images and caches. |
| `auth.keys` | Key file permissions (`0600`). Agent keys loaded. |

#### Recommendation, output and behaviour

- Human mode prints flutter-style lines, `[✓] [!] [✗] [-]`. Every non-OK line carries a one-line **fix** and an `opendap help errors <code>` reference.
- `--json` prints one document:

```json
{"schema":"opendap.doctor/v1","ok":false,"summary":{"ok":9,"warn":2,"error":1,"skip":1},
 "checks":[{"id":"usb.access","status":"error","title":"USB access",
   "message":"No udev rule grants access to 2207:350b (MaskROM)",
   "fix":{"description":"Install the OpenDAP udev rules","command":"opendap doctor --fix usb.access","auto":true,"needs_root":true},
   "docs":"opendap help errors usb.permission_denied","duration_ms":4}]}
```

- Exit code: **0** when there is no `error`, **1** otherwise. `--strict` also fails on `warn`. Use this in pre-commit hooks and scripts.
- `--check <id>[,<id>]`, `--list-checks`, `--offline`, `--target <sel>`.
- `--fix [<id>…]` applies only fixes marked `auto`. It respects `--dry-run`, prints any `sudo` command instead of running it, and is non-interactive with `--force`.
- `opendap doctor --report` (the `brew config` analogue) writes a redacted Markdown and JSON bundle for bug reports. Tokens, serials and IPs are redacted by default.
- **Shared framework:** the kit provides a `Check` trait (`id`, `title`, `run(ctx) -> Finding`, an optional `fix`). Every toolkit tool adds its own checks: the image builder checks the Linux VM and container runtime, fab-package checks KiCad and FreeCAD versions. Each tool exposes them as `<tool> doctor`, and `opendap doctor --all` aggregates the toolkit tools installed on PATH.

---

## 7. A shared domain model for the toolkit

The Forge lesson applied to a device toolkit.

### Recommendation, crates

- **`opendap-model`**: plain serde types plus `schemars` (1.2.3) JSON Schema:
  - `Device` (id, serial, model, revision, mode, transports[], os_version, agent_version)
  - `Target`, a selector that resolves to a Device or a VM
  - `Transport`: `usb`, `serial`, `net`, `vm`, `maskrom`
  - `Image`, `Artifact`, `Partition`, `Package`, `Service`, `Job`
  - `Finding`, shared by `doctor` and the security check
  - `ErrorBody`

  Every type has a versioned schema ID (`opendap.device/v1`). These types are the domain model shared by the CLI, the device agent, the image builder and the docs.
- **`opendap-cli-kit`**: `clap` 4.6.7 (derive) plus:
  - global flags;
  - output rendering (human, JSON, NDJSON; `anstream` 1.0 for colour handling);
  - the error envelope and exit codes;
  - target and context resolution;
  - config loading;
  - `tracing` logging;
  - agent detection;
  - help, schema, completion and man-page export (`clap_complete` 4.6.11, `clap_mangen` 0.3.3);
  - the `doctor` framework;
  - an MCP adapter (`rmcp` 3.6.0, stdio);
  - self-update and receipts.
- **Command metadata in one place.** Each command declares `{verb, noun, category: read|mutate|destructive|stream, idempotent: bool, output_schema, errors: [codes], examples}`, attached to the clap command (for example through a derive or a registry). The kit generates the following from it:
  - `--help`;
  - `opendap schema`;
  - `opendap tools` (MCP);
  - completions;
  - man pages;
  - the docs Markdown and `llms.txt`;
  - `--dry-run` support checks.
- **Conformance test** (the Forge-style lint). A shared `#[test]` walks every tool's clap tree and fails on any of these:
  - a verb outside the allowlist;
  - `info`, `show`, `ls`, `rm` used as canonical names (allowed only as hidden aliases);
  - `--yes`/`--skip-*` flags;
  - a missing `--json` support;
  - a mutating command without `--dry-run`;
  - a destructive command without `--force`;
  - a flag name outside the kebab-case or shared-glossary rules;
  - a reused short flag with a different meaning;
  - an undocumented error code.

### Recommendation, one front door or many binaries

- `opendap` is the front door.
- Other toolkit tools ship as separate binaries named `opendap-<tool>`: `opendap-image`, `opendap-fab`, `opendap-export`, `opendap-seccheck` (names to be decided). They are discoverable as **external subcommands**, the cargo and git pattern: `opendap image build` executes `opendap-image build` when it is on PATH.
- This keeps the device CLI small. Heavy host dependencies (image building in a Linux VM, KiCad, FreeCAD) stay out of it, while users and agents still get one grammar and one `opendap cli search`.

---

## 8. Recommended shared conventions for all OpenDAP toolkit CLIs

> **All items in this section are recommendations.** Nothing here is confirmed by the maintainers.

### 8.1 Naming and grammar

- Binaries: `opendap` (front door) and `opendap-<tool>` (plugins). Aliases `dap` and `odap` are symlinks created only when free (§4.2). Lowercase and kebab-case only.
- Canonical form: **`opendap <noun> <verb> [subject] [--flags]`** (noun-verb, as in `cf`, `gh` and clig.dev). Nouns are singular: `device`, `target`, `app`, `file`, `service`, `image`, `vm`, `log`, `config`, `context`, `key`, `self`.
- **Verb vocabulary (closed set):** `list`, `get`, `create`, `update`, `delete`, `install`, `uninstall`, `start`, `stop`, `restart`, `push`, `pull`, `flash`, `boot`, `reboot`, `exec`, `watch`, `wait`, `pair`, `unpair`, `use`, `check`, `fix`, `search`. `get`, never `info` or `show`. `delete`, never `rm` or `remove`. `list`, never `ls`. The short forms exist only as hidden aliases.
- **ADB-style shortcuts** are documented aliases of canonical commands and never have their own semantics. `opendap shell` → `exec --interactive`. `opendap push`/`pull` → `file push`/`pull`. `opendap logs` → `log watch`. `opendap devices` → `device list`. `opendap flash` → `image flash`. `schema` and `tools` always print the canonical path.
- The subject is the last positional argument. Containers are flags. `list` takes no positional argument.
- No prefix abbreviation of subcommands (clig.dev). Unknown flags are errors.
- Long-running operations come in two forms: blocking by default with a `--detach` that returns a `Job`, and `opendap job get|watch|wait <id>`.

### 8.2 Global flags (identical in every tool)

| Flag | Env | Meaning |
|---|---|---|
| `-t, --target <sel>` | `OPENDAP_TARGET` | Device or VM selector (§8.6). The flag wins over the variable, as `-s` beats `ANDROID_SERIAL` in adb. |
| `--context <name>` | `OPENDAP_CONTEXT` | Named target and config bundle. |
| `-o, --output <human\|json\|ndjson\|plain>` | `OPENDAP_OUTPUT` | Output mode. `--json` is shorthand for `-o json`. |
| `--fields <a,b>` | — | Project the JSON fields. `--fields` with no value lists them (the `gh` pattern). |
| `-n, --dry-run` | — | Print the plan and change nothing. Accepted by every command; read-only commands show what they would query. |
| `-f, --force` | — | The **only** way to skip confirmation. Required for destructive operations when non-interactive. |
| `--no-input` | `OPENDAP_NO_INPUT=1` | Never prompt, even on a TTY. Implied when stdin is not a TTY, under `CI`, or when an agent is detected. |
| `-q, --quiet` / `-v…` | `OPENDAP_LOG` | Verbosity. Logs go to stderr only. |
| `--log-format <text\|json>` | `OPENDAP_LOG_FORMAT` | Structured logs on stderr. |
| `--color <auto\|always\|never>` | `NO_COLOR`, `TERM=dumb` | Follows clig.dev. Never colours JSON on non-TTY. |
| `--config <path>` | `OPENDAP_CONFIG` | Explicit config file. |
| `--offline` | `OPENDAP_OFFLINE=1` | No network except to the explicit target. |
| `--timeout <dur>` | — | A global ceiling. Commands that wait have their own `--wait-timeout`. |
| `-h, --help` / `-V, --version` | — | `--version` prints the bare version on stdout. `version --json` gives the details. |

- Reserve `--experimental-*` for feature gates.
- Never add credential flags (`--token`); use env or the keychain (§8.8).

### 8.3 Output

- **stdout is data only.** Progress, spinners, warnings, hints and `✓` lines go to stderr. A mutation with nothing to return prints nothing on stdout.
- **Default mode, in priority order:** `--output` > `OPENDAP_OUTPUT` > **`json` when an agent is detected** > `human` when stdout is a TTY > **`json` otherwise**. This deliberately differs from `gh`, where piping is still human-formatted, and is close to `cf`, where JSON is always the default. Humans who want greppable tables when piping use `-o plain`. (There is a trade-off here; the alternative is human everywhere unless `--json` is passed.)
- **JSON contract:**
  - single results are an object;
  - lists are `{"items":[…],"next_cursor":…}`, never a bare array, so pagination can be added later without breaking consumers;
  - every top-level document carries `"schema":"opendap.<kind>/v1"`;
  - mutations include `"changed": true|false` (idempotency signal) and, under `--dry-run`, `"plan":[…]`.
- JSON is compact (one line) when not a TTY or when an agent is detected, and pretty when it is a TTY. This follows `cf`'s "condensed for agents".
- **NDJSON for streams** (`log watch`, `device watch`, `image flash`, `vm start --follow`): one event per line, `{"type":"progress|log|state|warning|result|error", "ts":…, …}`. The last line is always `result` or `error`.
- Output schemas are published (`opendap schema --output-schemas`). Fields are only **added** within a `/v1`. Removing or renaming a field means `/v2`, with `/v1` still selectable for one minor-release cycle through `--schema-version`.
- The human mode can change freely. Scripts must not parse it, and the docs say so.

### 8.4 Errors and exit codes

- **Error envelope.** Under `json` or `ndjson` it goes to stdout; it always goes to stderr as human text.

```json
{"schema":"opendap.error/v1","ok":false,
 "error":{"code":"target.ambiguous","message":"3 devices match; pick one with --target",
  "hint":"Run `opendap device list` and pass --target usb:<serial>",
  "fix":[{"command":"opendap device list --json"}],
  "retryable":false,"details":{"candidates":["usb:T1A3F2","net:t1.local","vm:default"]},
  "docs":"opendap help errors target.ambiguous"}}
```

- `code` is a **stable dotted identifier**: `<area>.<condition>`, for example `usb.permission_denied`, `device.not_found`, `device.mode_mismatch`, `image.checksum_mismatch`, `auth.required`, `confirm.required`, `input.invalid`. Codes live in the model crate with a message template, a hint and a docs entry. This follows the Forge rule: remediation knowledge lives in the model, not in ad-hoc switches.
- `opendap help errors [<code>]` lists and explains codes and is generated from the model.

#### Exit codes (coarse, stable)

| Code | Meaning |
|---|---|
| 0 | Success, including a no-op with `changed:false` |
| 1 | Generic failure, or a doctor/check error-level finding |
| 2 | Usage error: bad flag or argument (clap default) |
| 3 | Not found (device, file, app, image) |
| 4 | Authentication or pairing required (same as `gh`) |
| 5 | Permission denied on the host (USB, serial, filesystem) |
| 6 | Target ambiguous: several matches and no `--target` |
| 7 | Target busy or locked, or a conflicting operation in progress |
| 8 | Timed out |
| 9 | Confirmation required: non-interactive and `--force` missing (`cf` uses 1; a distinct code tells agents exactly what to do) |
| 10 | Precondition failed or version incompatible (device agent too old, wrong mode) |
| 11 | Unsupported on this host or target |
| 130 | Interrupted (SIGINT) |

- The JSON `code` is the precise contract. Exit codes are for shells.

### 8.5 Interactivity, safety and idempotency

- Prompt only when stdin **and** stderr are TTYs, no agent is detected, and `--no-input` is not set. In every other case a missing value fails with `input.missing` (exit 2), and for enums the error lists the valid values.
- Destructive operations need either an interactive confirmation or `--force`. Destructive operations are: flashing, wiping, deleting user data, factory reset, `self uninstall --purge`. Interactive confirmation of a flash or wipe makes the user **type the device serial**. Non-interactive runs without `--force` fail with code 9 and change nothing.
- `--dry-run` is accepted by every command. For mutating commands it prints the exact plan: device, partitions, image hash, bytes. A plan is reproducible: `--plan-out plan.json` then `apply --plan plan.json` (optional, later).
- **Idempotent by design:** `install` of the version already installed, `push` of an identical file (hash compared), `vm start` of a running VM and `config set` to the same value all return `changed:false` with exit 0. Retrying after a timeout is safe. Commands that are not idempotent say so in their metadata.
- `@file` and `-` (stdin) work on every value flag that takes content. Secrets are read only from stdin or a file, never from argv.

### 8.6 Targets, transports and contexts

- **Selector grammar:**
  - `usb:<serial>`
  - `serial:<path>` (console)
  - `net:<host>[:port]` (SSH)
  - `vm:<name>` (virtual T1: SSH, VNC, QMP)
  - `maskrom:<bus>-<addr>` (rockusb)
  - or a bare name that resolves through contexts.

  Resolution: when exactly one candidate is present it is used automatically, *except* for destructive operations, which need an explicit target (selector or context). Otherwise the command fails with code 6 and lists the candidates.
- **Modes are part of the Device model**, as adb and fastboot treat them: `normal`, `gadget:{serial,net,storage}`, `maskrom`, `recovery`. `opendap device wait --mode maskrom --timeout 60s` replaces ad-hoc sleeps. A command run against the wrong mode fails with `device.mode_mismatch` (exit 10), and the hint names the transition command (`opendap device reboot --mode maskrom`).
- **Contexts**, kubectl style: `opendap context list|get|create|use|delete`. A context bundles target, defaults and credentials reference. They are stored in the user config. `OPENDAP_CONTEXT` sets one per shell. **Every output says whether it acted on a physical device or the virtual T1** (`"target":{"kind":"vm","name":"default"}`), which is the counterpart of `cf`'s local vs remote signal.
- **No persistent host server in v0.** Each invocation talks to devices directly (USB through `nusb`, SSH, QMP). Re-evaluate an adb-style background server only if port forwarding or multi-client arbitration needs one.

### 8.7 Configuration and files

- Precedence (clig.dev): **flags > env > project file (`opendap.toml`, found by walking up from cwd) > user (`$XDG_CONFIG_HOME/opendap/config.toml`) > system (`/etc/opendap/config.toml`)**. Use XDG paths **on macOS too**, not `~/Library/Application Support`, which is what CLI users expect. The `etcetera` crate (0.11) supports an XDG strategy on macOS; the `directories` crate defaults to Apple paths.
- Other locations:
  - state: `$XDG_STATE_HOME/opendap`
  - caches (images): `$XDG_CACHE_HOME/opendap`
  - data (keys): `$XDG_DATA_HOME/opendap`, with mode `0600`/`0700`
- `opendap config get|set|list|unset [--scope project|user]` and `opendap config path`. Unknown keys produce a warning, not an error. Each subsection is validated lazily, so a broken `vm` section does not break `device list`, matching `cf`'s "lazy config validation".
- All tools share one config file with a table per tool (`[image]`, `[fab]`) and shared `[targets]`/`[contexts]`.

### 8.8 Authentication and trust

- Device pairing follows adb's RSA-prompt model:
  - `opendap device pair` generates or uses a host key (ed25519) and sends it to the device;
  - the device confirms on screen or over the serial console;
  - the host key is stored under `$XDG_DATA_HOME/opendap/keys`;
  - SSH host keys are pinned per device.
- For VMs, the key is injected at image build time.
- Credentials for any remote service (for example release mirrors) come from `OPENDAP_TOKEN`, then the OS keychain (`keyring` 4.2, opt-in), then a `0600` file. There are never credential flags. Remote logins use the **device authorization grant**, as `cf` does, so they work over SSH and in containers.
- `opendap auth status --json` reports what is configured and **never prints secrets**.

### 8.9 Logging and diagnostics

- `tracing` with `OPENDAP_LOG` (EnvFilter syntax, for example `opendap=debug,rockusb=trace`). `-v` = info, `-vv` = debug, `-vvv` = trace. Logs go to stderr only. `--log-format json` gives agents and bug reports structured logs. `--log-file <path>` mirrors them.
- USB and serial protocol traces only at trace level, with payload bytes redacted unless `--trace-payload` is set.
- Every error under `-v` includes the `details` chain. `doctor --report` bundles the version, config (redacted), the last N log lines and device inventory.

### 8.10 Help and discoverability for agents

- `--help` is generated, concise and example-first. Every command has at least two examples, including one with `--json`.
- **Agent detection.** Detection uses `AI_AGENT`, `CLAUDECODE`, `CODEX_THREAD_ID`, `CURSOR_AGENT`, `GEMINI_CLI`, `OPENCODE`, `COPILOT_AGENT` and similar variables, the same list `cf` uses. `OPENDAP_AGENT=1|0` forces it on or off. When an agent is detected:
  - prompts are disabled;
  - JSON is compact;
  - help starts with a short discovery banner: use `opendap cli search "<task>"`, then `<cmd> --help`, then `opendap schema <cmd>`, and do not crawl nested `--help`;
  - update notices are suppressed.
- **Introspection commands** (front door, also available in each plugin):
  - `opendap cli search "<intent>"`: top 5 commands as compact JSON `{command, summary}` (`cf` pattern; offline index built into the binary).
  - `opendap schema [<command…>]`: JSON with arguments, flags (types, enums, defaults), category, idempotency, output schema ID and error codes. `opendap schema --all` dumps the whole tree.
  - `opendap tools`: MCP tool definitions derived from the same metadata.
  - `opendap help errors [<code>]`.
- **MCP server mode: `opendap mcp serve`** (stdio first, using `rmcp`).
  - It exposes a **small, curated tool set** rather than one tool per command, following Code Mode and Anthropic's advice on token budgets: `opendap_search`, `opendap_describe` (schema for one command) and `opendap_run` (runs any canonical command with schema-validated arguments and returns its JSON).
  - A few high-value direct tools such as `device_list` and `doctor`.
  - MCP tool annotations (`readOnlyHint`, `destructiveHint`, `idempotentHint`) come from command metadata. Destructive commands still need `force: true` in the arguments, and the host's consent flow applies.
  - Long operations (flash, image build) map to the MCP **Tasks** extension when the client supports it, and to `Job` handles otherwise.
- **Docs for LLMs.** The docs site publishes `llms.txt` and a Markdown twin of every page, including a generated CLI reference (from `schema --all`) and an error-code catalogue. These are served under `$OPENDAP_INSTALL_BASE`, or wherever the docs live.
- An optional `AGENTS.md` snippet in each repo explains how agents should drive the toolkit: target selection, `--json`, `--dry-run` before `--force`.
- Shell completions (`opendap completion bash|zsh|fish`) and man pages are shipped in the tarball.

### 8.11 Versioning, compatibility and telemetry

- **SemVer** for each binary. Pre-1.0 minors may break the human output, but **never** a published JSON schema version or an error code.
- `opendap version --json` returns `{version, commit, build_date, target, model_schema_versions, device_protocol: {min,max}}`. Before every command the device agent and host negotiate a protocol range, and a mismatch gives `device.incompatible` (exit 10) with an upgrade hint.
- Deprecations: a stderr warning plus a `warnings:[{code:"deprecated.flag",…}]` entry in the JSON, kept for at least one minor release. Deprecated commands then **disappear** from help, search and schema (the `cf` rule) but keep running until the next major.
- Toolkit tools share the kit crate version. A tool that finds an incompatible `opendap-<tool>` plugin warns from `doctor`.
- **Telemetry:** none by default. If it is ever added, it is opt-in (`opendap cli telemetry enable`), respects `DO_NOT_TRACK=1`, documents exactly what it sends, and offers a `status` command. Neither the CLI nor the installer phones home. The only network call is the optional, user-enabled update check.

### 8.12 Install, update and uninstall

- Bootstrap: `curl --proto '=https' --tlsv1.2 -fsSL $OPENDAP_INSTALL_BASE/install.sh | sh -s -- [--version X] [--yes] [--no-modify-path] [--aliases auto|none] [--uninstall] [--verbose] [--json]` (§4.2).
- `opendap self update|uninstall|alias` are the in-binary equivalents, reading the same receipt.
- Release artifacts per §5.1. Integrity per §4.2: sha256 plus a minisign signature, with an optional immutable-release attestation.
- Linux: musl-static only. macOS: per-arch, optionally universal. Ad-hoc signed. Windows: none.

---

## 9. Open questions for the maintainers

1. Default output when stdout is piped: JSON as recommended, which is agent-first and like `cf`, or human-plain, which is script-friendly and like `gh`?
2. One front-door binary with external `opendap-<tool>` plugins, or fully separate tool names that only share the kit?
3. Should MaskROM flashing require an explicit `maskrom:` selector even when only one device is present? Recommended: yes for every destructive operation.
4. The name of the device-side agent and its protocol (SSH plus JSON-RPC, or a custom USB gadget function). This report deliberately does not define it.
5. When to buy an Apple Developer ID, which only matters for browser downloads, a `.pkg` or a cask.
6. Whether to adopt `dist` now for build and packaging, or keep a custom `xtask` until CI exists.

## 10. Sources (accessed 2026-10-21)

- Cloudflare: [cf technical preview (2026-04-13)](https://blog.cloudflare.com/cf-cli-local-explorer/) · [cf launch](https://blog.cloudflare.com/cloudflare-cf-cli-launch) · [Forge open source](https://blog.cloudflare.com/forge-open-source-generation-pipeline) · [Code Mode MCP (2026-02-20)](https://blog.cloudflare.com/code-mode-mcp/) · [github.com/cloudflare/cf](https://github.com/cloudflare/cf) (`AGENTS.md`, `packages/cli/AGENTS.md`, `src/index.ts`, `src/lib/agent-context.ts`, `src/lib/errors.ts`, `src/lib/output.ts`, `src/commands/{schema,tools}.ts`, `src/commands/cli/search.ts`, `telemetry.md`, `.changeset/*`) · [github.com/cloudflare/forge](https://github.com/cloudflare/forge) · [npm cf 1.0.0-beta.20](https://www.npmjs.com/package/cf)
- GitHub CLI: [formatting](https://cli.github.com/manual/gh_help_formatting) · [exit codes](https://cli.github.com/manual/gh_help_exit-codes) · [release create](https://cli.github.com/manual/gh_release_create) · [immutable releases](https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases) · [verifying a release](https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/verifying-the-integrity-of-a-release)
- [kubectl reference](https://kubernetes.io/docs/reference/kubectl/) · [adb](https://developer.android.com/tools/adb) · [fastboot / AOSP](https://source.android.com/docs/setup/test/running) · [probe-rs](https://probe.rs/docs/tools/probe-rs/) · [fly mcp server](https://docs.fly.io/flyctl/mcp-server)
- [clig.dev](https://clig.dev/) · [Anthropic: Writing effective tools for agents (2025-09-11)](https://www.anthropic.com/engineering/writing-tools-for-agents) · [llmstxt.org](https://llmstxt.org/) · [MCP specification 2026-07-28](https://modelcontextprotocol.io/specification/latest)
- Installers read directly: `https://astral.sh/uv/install.sh` (uv 0.13.5), `https://sh.rustup.rs`, `https://deno.land/install.sh`, `https://bun.sh/install`, `https://fly.io/install.sh`
- [dist config reference](https://axodotdev.github.io/cargo-dist/book/reference/config.html) · [dist shell installer](https://axodotdev.github.io/cargo-dist/book/installers/shell.html) · [dist CI customising / cross builds](https://axodotdev.github.io/cargo-dist/book/ci/customizing.html)
- Doctor: [Homebrew manpage](https://docs.brew.sh/Manpage) · [Expo tools](https://docs.expo.dev/develop/tools/) · flutter `packages/flutter_tools/lib/src/commands/doctor.dart` and `doctor_validator.dart` on GitHub
- Versions from the GitHub releases API: dist v0.33.1, cargo-zigbuild v0.24.0, cross v0.2.5, minisign 0.12, cosign v3.1.4, gh v2.103.0, flyctl v0.4.120, probe-rs v0.32.0, axoupdater v0.10.3, kubernetes v1.37.1, bun v1.4.3, deno v2.9.8, Homebrew 7.0.10, clap v4.6.7, Rust 1.99.1. From crates.io: rockusb 0.4.0, nusb 0.2.8, minisign-verify 0.3.0, self_update 1.3.1, schemars 1.2.3, rmcp 3.6.0, clap_complete 4.6.11, clap_mangen 0.3.3, anstream 1.0.0, keyring 4.2.0, etcetera 0.11.0. From npm: wrangler 4.151.3.
- Local verification on this host (macOS 27.0.1, curl 8.7.1, rustc 1.98.1): `xattr -l` on a curl download showed no quarantine attribute, and `codesign -dv` on a rustc build showed an ad-hoc, linker-signed signature.
