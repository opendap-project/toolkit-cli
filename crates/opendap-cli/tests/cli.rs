//! End-to-end checks of the binary that need no virtual T1.

use std::process::{Command, Output};

fn opendap(args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_opendap"));
    c.args(args)
        .env_remove("OPENDAP_TARGET")
        .env_remove("OPENDAP_OUTPUT")
        .env_remove("OPENDAP_NO_INPUT");
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().expect("runs")
}

fn json(out: &Output) -> serde_json::Value {
    serde_json::from_slice(&out.stdout).expect("stdout is one JSON document")
}

#[test]
fn bare_version_flag() {
    let out = opendap(&["-V"], &[]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn piped_output_defaults_to_json() {
    let out = opendap(&["version"], &[("OPENDAP_AGENT", "0")]);
    assert_eq!(json(&out)["schema"], "opendap.version/v1");
    let out = opendap(&["version", "-o", "plain"], &[]);
    assert!(serde_json::from_slice::<serde_json::Value>(&out.stdout).is_err());
}

#[test]
fn no_input_env_refuses_an_interactive_shell() {
    for value in ["1", "true"] {
        let out = opendap(&["exec", "-o", "json"], &[("OPENDAP_NO_INPUT", value)]);
        assert_eq!(out.status.code(), Some(2));
        let v = json(&out);
        assert_eq!(v["schema"], "opendap.error/v1");
        assert_eq!(v["error"]["code"], "input.missing");
        assert!(!out.stderr.is_empty(), "the error is also on stderr");
    }
}

#[test]
fn usage_errors_use_the_envelope_and_exit_2() {
    let out = opendap(&["input", "send", "key", "home", "--json"], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(json(&out)["error"]["code"], "input.invalid");
}

#[test]
fn invalid_and_unsupported_targets() {
    let out = opendap(&["screen", "capture", "--json", "-t", "nope:x"], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(json(&out)["error"]["code"], "target.invalid");
    let out = opendap(
        &["screen", "capture", "--json"],
        &[("OPENDAP_TARGET", "maskrom:1-4")],
    );
    assert_eq!(out.status.code(), Some(11));
    assert_eq!(json(&out)["error"]["code"], "target.unsupported");
}

#[test]
fn schema_lists_every_command_and_rejects_unknown_ones() {
    let v = json(&opendap(&["schema", "--json"], &[]));
    assert_eq!(v["schema"], "opendap.schema/v1");
    let paths: Vec<&str> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"vm start") && paths.contains(&"input send tap"));
    let out = opendap(&["schema", "vm", "reboot", "--json"], &[]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(json(&out)["error"]["code"], "command.unknown");
}
