//! Locating the virtual-devices checkout that runs virtual devices (`bin/odap-vdev`):
//! `$ODAP_VIRTUAL_DEVICES_DIR`, else the workspace's `repos.tsv` above the current directory
//! (RFC 0008). Never a guessed relative path.

use std::path::{Path, PathBuf};

use opendap_model::Error;
use opendap_model::error as codes;

/// Environment variable that points at a virtual-devices checkout.
pub const VD_DIR_VAR: &str = "ODAP_VIRTUAL_DEVICES_DIR";

/// The repository's name in `repos.tsv`.
pub const VD_REPO: &str = "virtual-devices";

/// A directory is a virtual-devices checkout when it has the tool.
pub fn is_vd_checkout(dir: &Path) -> bool {
    dir.join("bin/odap-vdev").is_file()
}

/// `$ODAP_VIRTUAL_DEVICES_DIR` when set (it must be a checkout), otherwise the workspace's entry.
pub fn find_vd_checkout(env_value: Option<&str>, cwd: &Path) -> Result<PathBuf, Error> {
    if let Some(v) = env_value.filter(|v| !v.is_empty()) {
        let p = PathBuf::from(v);
        let p = if p.is_relative() { cwd.join(p) } else { p };
        return if is_vd_checkout(&p) {
            Ok(p.canonicalize().unwrap_or(p))
        } else {
            Err(Error::new(
                &codes::WORKSPACE_NOT_FOUND,
                format!("{VD_DIR_VAR}={v} is not a virtual-devices checkout (no bin/odap-vdev)"),
            )
            .hint(format!(
                "Point {VD_DIR_VAR} at the virtual-devices repository, or unset it."
            )))
        };
    }
    for dir in cwd.ancestors() {
        let Ok(tsv) = std::fs::read_to_string(dir.join("repos.tsv")) else {
            continue;
        };
        let found = tsv.lines().skip(1).find_map(|line| {
            let (path, slug) = line.split_once('\t')?;
            (slug.trim().rsplit('/').next() == Some(VD_REPO)).then(|| dir.join(path))
        });
        return match found {
            Some(p) if is_vd_checkout(&p) => Ok(p.canonicalize().unwrap_or(p)),
            _ => Err(Error::new(
                &codes::WORKSPACE_NOT_FOUND,
                format!("{VD_REPO} is not checked out in {}", dir.display()),
            )
            .fix("scripts/bootstrap.sh")),
        };
    }
    Err(Error::new(
        &codes::WORKSPACE_NOT_FOUND,
        format!("no OpenDAP workspace (repos.tsv) above {}", cwd.display()),
    )
    .hint(format!(
        "Run from inside the workspace, or set {VD_DIR_VAR}."
    )))
}

/// Same as [`find_vd_checkout`], from the process environment and directory.
pub fn vd_checkout() -> Result<PathBuf, Error> {
    let cwd = std::env::current_dir().map_err(|e| {
        Error::new(
            &codes::HOST_IO,
            format!("cannot read the current directory: {e}"),
        )
    })?;
    find_vd_checkout(std::env::var(VD_DIR_VAR).ok().as_deref(), &cwd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_checkout_through_repos_tsv() {
        let root = std::env::temp_dir().join(format!("opendap-ws-{}", std::process::id()));
        std::fs::create_dir_all(root.join("public/virtual-devices/bin")).unwrap();
        std::fs::create_dir_all(root.join("public/cli/x")).unwrap();
        std::fs::write(root.join("public/virtual-devices/bin/odap-vdev"), "").unwrap();
        std::fs::write(
            root.join("repos.tsv"),
            "path\trepository\npublic/virtual-devices\topendap-project/virtual-devices\n",
        )
        .unwrap();
        let found = find_vd_checkout(None, &root.join("public/cli/x")).unwrap();
        assert!(found.ends_with("public/virtual-devices"));
        assert!(find_vd_checkout(Some("/nonexistent"), &root).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reports_a_missing_workspace() {
        let e = find_vd_checkout(None, Path::new("/")).unwrap_err();
        assert!(e.message.contains("repos.tsv"));
    }
}
