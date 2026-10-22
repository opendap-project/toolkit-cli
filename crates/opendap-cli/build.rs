// Records the git commit and the build target for `opendap version`.
use std::process::Command;

fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=OPENDAP_BUILD_COMMIT={commit}");
    println!(
        "cargo:rustc-env=OPENDAP_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap_or_default()
    );
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
}
