use std::path::PathBuf;

use opendap_cli_kit::CliError;
use opendap_model::Transport;
use opendap_model::error as codes;
use serde::Serialize;

use super::{Ctx, Doc, Need, emit_plan, resolve_target};
use crate::cli::ScreenCmd;
use crate::virt::png_size;

#[derive(Serialize)]
struct Capture {
    target: Transport,
    path: String,
    format: &'static str,
    width: u32,
    height: u32,
    bytes: u64,
}

pub fn run(ctx: &Ctx, cmd: ScreenCmd) -> Result<u8, CliError> {
    let ScreenCmd::Capture { file } = cmd;
    let (vm, target) = resolve_target(ctx, Need::Qmp)?;
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
    if ctx.dry_run {
        return emit_plan(
            ctx,
            "opendap.screen_capture/v1",
            vec![format!("QMP screendump of {target} to {}", path.display())],
        );
    }
    vm.qmp(&["screenshot".into(), path.display().to_string()])?;
    let bytes = std::fs::read(&path).map_err(|e| {
        CliError::new(
            &codes::HOST_IO,
            format!("cannot read {}: {e}", path.display()),
        )
    })?;
    let (width, height) = png_size(&bytes).ok_or_else(|| {
        CliError::new(
            &codes::TRANSPORT_QMP_FAILED,
            format!("{} is not a PNG", path.display()),
        )
    })?;
    let doc = Capture {
        target,
        path: path.display().to_string(),
        format: "png",
        width,
        height,
        bytes: bytes.len() as u64,
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
