//! Shared conventions of the OpenDAP toolkit CLIs (spec §2, research §8):
//! output modes, the error envelope and exit codes, command metadata,
//! agent detection. Interface layer: no use case lives here.

pub mod env;
pub mod error;
pub mod meta;
pub mod output;

pub use error::CliError;
pub use meta::{Category, CommandMeta};
pub use output::{Output, OutputMode};
