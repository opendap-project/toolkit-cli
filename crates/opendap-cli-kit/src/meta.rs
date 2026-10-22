//! Command metadata, declared once per command. `schema`, the conformance
//! tests and, later, help, MCP tools and docs are generated from it.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// Reads state; changes nothing.
    Read,
    /// Changes state on the host or the target.
    Mutate,
    /// Destroys data; needs an explicit target and confirmation or --force.
    Destructive,
    /// Streams events (NDJSON).
    Stream,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandMeta {
    /// Canonical path without the binary name, for example `vm start`.
    pub path: &'static str,
    pub summary: &'static str,
    pub category: Category,
    /// Running it twice leaves the same state and reports `changed:false` the second time.
    pub idempotent: bool,
    /// Whether the command acts on a device and honours `--target`.
    pub targeted: bool,
    /// Schema ID of the success document.
    pub output_schema: &'static str,
    /// Error codes the command can return, besides the generic usage errors.
    pub errors: &'static [&'static str],
    pub examples: &'static [&'static str],
}
