use opendap_cli_kit::CliError;
use serde::Serialize;

use super::{Ctx, Doc};

#[derive(Serialize)]
struct Version {
    version: &'static str,
    commit: Option<&'static str>,
    target: &'static str,
    model_schemas: &'static [&'static str],
}

pub fn run(ctx: &Ctx) -> Result<u8, CliError> {
    let commit = Some(env!("OPENDAP_BUILD_COMMIT")).filter(|c| !c.is_empty());
    let v = Version {
        version: env!("CARGO_PKG_VERSION"),
        commit,
        target: env!("OPENDAP_BUILD_TARGET"),
        model_schemas: opendap_model::SCHEMAS,
    };
    ctx.out.emit(
        &Doc {
            schema: "opendap.version/v1",
            body: &v,
        },
        || {
            format!(
                "opendap {} ({}, {})\n",
                v.version,
                commit.unwrap_or("unknown commit"),
                v.target
            )
        },
    );
    Ok(0)
}
