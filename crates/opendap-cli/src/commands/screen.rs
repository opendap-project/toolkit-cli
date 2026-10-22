use std::path::PathBuf;

use opendap_cli_kit::CliError;
use opendap_core::{Need, Outcome, resolve_target};
use opendap_model::error as codes;
use opendap_transport_vm::LocalVms;

use super::{Ctx, Doc, emit_plan};
use crate::cli::ScreenCmd;

pub fn run(ctx: &Ctx, cmd: ScreenCmd) -> Result<u8, CliError> {
    let ScreenCmd::Capture { file } = cmd;
    let (vm, target) = resolve_target(&LocalVms, ctx.target.as_ref(), Need::Qmp)?;
    let cwd = std::env::current_dir().map_err(|e| {
        CliError::new(
            &codes::HOST_IO,
            format!("cannot read the current directory: {e}"),
        )
    })?;
    let file = file.unwrap_or_else(|| {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        PathBuf::from(format!("screen-{secs}.png"))
    });
    let path = if file.is_relative() {
        cwd.join(file)
    } else {
        file
    };
    if let Some(parent) = path.parent().filter(|p| !p.is_dir()) {
        return Err(CliError::new(
            &codes::HOST_IO,
            format!("no such directory: {}", parent.display()),
        ));
    }
    let path = path.display().to_string();
    let doc = match opendap_core::screen::capture(&vm, target, &path, ctx.dry_run)? {
        Outcome::Plan(plan) => return emit_plan(ctx, "opendap.screen_capture/v1", plan),
        Outcome::Done(doc) => doc,
    };
    ctx.out.emit(
        &Doc {
            schema: "opendap.screen_capture/v1",
            body: &doc,
        },
        || format!("{}\n", doc.path),
    );
    Ok(0)
}
