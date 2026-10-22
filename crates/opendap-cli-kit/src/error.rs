//! The error every command returns, rendered as the `opendap.error/v1`
//! envelope on stdout (JSON modes) and as text on stderr (always).
//!
//! The error itself is domain data ([`opendap_model::Error`]); this module
//! only adds its human rendering.

/// The error every command returns.
pub use opendap_model::Error as CliError;

/// Human rendering of an error, for stderr.
pub fn human(err: &CliError) -> String {
    let b = err.body();
    let mut s = format!("error[{}]: {}\n  hint: {}\n", b.code, b.message, b.hint);
    for f in &b.fix {
        if let Some(c) = &f.command {
            s.push_str(&format!("  fix: {c}\n"));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendap_model::error as codes;

    #[test]
    fn human_rendering_lists_hint_and_command_fixes() {
        let e = CliError::new(&codes::VM_NOT_RUNNING, "vm:default is stopped")
            .fix("opendap vm start")
            .hint("Start it first.");
        assert_eq!(
            human(&e),
            "error[vm.not_running]: vm:default is stopped\n  hint: Start it first.\n  fix: opendap vm start\n"
        );
    }
}
