//! Locating the opendap-os checkout that provides the virtual T1 backend
//! (`build/dap-qemu.sh`, `build/dap-qmp.py`).

use std::path::{Path, PathBuf};

use opendap_model::Error;
use opendap_model::error as codes;

/// Environment variable that points at an opendap-os checkout.
pub const OS_DIR_VAR: &str = "OPENDAP_OS_DIR";

/// Where the opendap-os checkout sits inside the workspace.
pub const OS_IN_WORKSPACE: &str = "public/opendap-os";

/// A directory is an opendap-os checkout when it has the virtual T1 runner.
pub fn is_os_checkout(dir: &Path) -> bool {
    dir.join("build/dap-qemu.sh").is_file()
}

/// `$OPENDAP_OS_DIR` when set (it must be a checkout), otherwise walk up from
/// `cwd` to the folder that contains `public/opendap-os`.
pub fn find_os_checkout(env_value: Option<&str>, cwd: &Path) -> Result<PathBuf, Error> {
    if let Some(v) = env_value.filter(|v| !v.is_empty()) {
        let p = PathBuf::from(v);
        let p = if p.is_relative() { cwd.join(p) } else { p };
        return if is_os_checkout(&p) {
            Ok(p.canonicalize().unwrap_or(p))
        } else {
            Err(Error::new(
                &codes::WORKSPACE_NOT_FOUND,
                format!("{OS_DIR_VAR}={v} is not an opendap-os checkout (no build/dap-qemu.sh)"),
            )
            .hint(format!(
                "Point {OS_DIR_VAR} at the opendap-os repository, or unset it."
            )))
        };
    }
    for dir in cwd.ancestors() {
        let candidate = dir.join(OS_IN_WORKSPACE);
        if is_os_checkout(&candidate) {
            return Ok(candidate.canonicalize().unwrap_or(candidate));
        }
    }
    Err(Error::new(
        &codes::WORKSPACE_NOT_FOUND,
        format!("no {OS_IN_WORKSPACE} found above {}", cwd.display()),
    )
    .fix("scripts/bootstrap.sh"))
}

/// Same as [`find_os_checkout`], from the process environment and directory.
pub fn os_checkout() -> Result<PathBuf, Error> {
    let cwd = std::env::current_dir().map_err(|e| {
        Error::new(
            &codes::HOST_IO,
            format!("cannot read the current directory: {e}"),
        )
    })?;
    find_os_checkout(std::env::var(OS_DIR_VAR).ok().as_deref(), &cwd)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tmp(PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn tmp(name: &str) -> Tmp {
        let p = std::env::temp_dir().join(format!("opendap-ws-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Tmp(p.canonicalize().unwrap())
    }

    fn make_checkout(dir: &Path) {
        std::fs::create_dir_all(dir.join("build")).unwrap();
        std::fs::write(dir.join("build/dap-qemu.sh"), "#!/bin/sh\n").unwrap();
    }

    #[test]
    fn walks_up_to_the_workspace() {
        let t = tmp("walk");
        let os = t.0.join("public/opendap-os");
        make_checkout(&os);
        let deep = t.0.join("public/toolkit/cli/crates");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(find_os_checkout(None, &deep).unwrap(), os);
        assert_eq!(find_os_checkout(Some(""), &t.0).unwrap(), os);
    }

    #[test]
    fn env_variable_wins_and_is_validated() {
        let t = tmp("env");
        let ws_os = t.0.join("public/opendap-os");
        make_checkout(&ws_os);
        let other = t.0.join("elsewhere/opendap-os");
        make_checkout(&other);
        let found = find_os_checkout(Some(other.to_str().unwrap()), &t.0).unwrap();
        assert_eq!(found, other);

        let err = find_os_checkout(Some(t.0.join("nope").to_str().unwrap()), &t.0).unwrap_err();
        assert_eq!(err.code.code, "workspace.not_found");
    }

    #[test]
    fn reports_missing_workspace() {
        let t = tmp("none");
        let err = find_os_checkout(None, &t.0).unwrap_err();
        assert_eq!(err.code.code, "workspace.not_found");
        assert_eq!(err.exit_code(), 10);
    }
}
