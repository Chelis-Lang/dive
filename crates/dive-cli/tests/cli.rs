//! Smoke tests for the `dive` binary's command surface.

use std::process::Command;

fn dive(args: &[&str]) -> std::process::Output {
    match Command::new(env!("CARGO_BIN_EXE_dive")).args(args).output() {
        Ok(output) => output,
        Err(e) => unreachable!("cannot run dive binary: {e}"),
    }
}

/// Writes `contents` to a fresh file named `name` and returns its path.
fn temp_file(name: &str, contents: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dive-cli-test-{}-{}",
        std::process::id(),
        name.replace('.', "-")
    ));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        unreachable!("cannot create {}: {e}", dir.display());
    }
    let path = dir.join(name);
    if let Err(e) = std::fs::write(&path, contents) {
        unreachable!("cannot write {}: {e}", path.display());
    }
    path
}

#[test]
fn version_succeeds() {
    let out = dive(&["--version"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("dive "));
}

#[test]
fn unimplemented_commands_refuse_explicitly() {
    for cmd in ["check", "simulate", "translate", "validate-trace", "lsp"] {
        let out = dive(&[cmd]);
        assert_eq!(out.status.code(), Some(3), "{cmd}");
        assert!(
            String::from_utf8_lossy(&out.stderr).starts_with("unsupported:"),
            "{cmd}"
        );
    }
}

#[test]
fn unknown_command_is_usage_error() {
    assert_eq!(dive(&["frobnicate"]).status.code(), Some(2));
}

#[test]
fn parse_without_file_is_usage_error() {
    assert_eq!(dive(&["parse"]).status.code(), Some(2));
    assert_eq!(
        dive(&["parse", "--output", "xml", "x.tla"]).status.code(),
        Some(2)
    );
}

#[test]
fn parse_accepts_a_valid_module() {
    let path = temp_file("Good.tla", "---- MODULE Good ----\nx == 1\n====\n");
    let out = dive(&["parse", &path.to_string_lossy()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("ok:"));
}

#[test]
fn parse_json_reports_diagnostics() {
    let path = temp_file("Bad.tla", "---- MODULE Bad ----\nx == 1 * -2\n====\n");
    let out = dive(&["parse", &path.to_string_lossy(), "--output", "json"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => unreachable!("invalid JSON: {e}"),
    };
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["accepted"], false);
    assert_eq!(v["module"], "Bad");
    assert_eq!(v["diagnostics"][0]["code"], "DIVE-S0012");
    assert!(v.get("cst").is_none());
}

#[test]
fn parse_json_can_include_the_cst() {
    let path = temp_file("Cst.tla", "---- MODULE Cst ----\nx == 1\n====\n");
    let out = dive(&[
        "parse",
        &path.to_string_lossy(),
        "--output",
        "json",
        "--cst",
    ]);
    assert!(out.status.success());
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => unreachable!("invalid JSON: {e}"),
    };
    assert_eq!(v["cst"]["kind"], "SOURCE_FILE");
}

#[test]
fn parse_reads_a_config() {
    let tla = temp_file("Cfg.tla", "---- MODULE Cfg ----\nx == 1\n====\n");
    let cfg = temp_file("Cfg.cfg", "CONSTANT N = 3\nINIT Init\nINVARIANT Inv\n");
    let out = dive(&[
        "parse",
        &tla.to_string_lossy(),
        "--config",
        &cfg.to_string_lossy(),
        "--output",
        "json",
    ]);
    assert!(out.status.success());
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => unreachable!("invalid JSON: {e}"),
    };
    assert_eq!(v["accepted"], true);
    assert_eq!(v["config"]["accepted"], true);
    assert_eq!(v["config"]["model"]["invariants"][0]["text"], "Inv");
}

#[test]
fn parse_reports_config_errors() {
    let cfg = temp_file("Bad.cfg", "CONSTANT N = -1\nINIT Init\n");
    let out = dive(&["parse", "--config", &cfg.to_string_lossy()]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("error[DIVE-C0013]"), "{err}");
    assert!(
        err.contains("CFG_EXPECTED_SYMBOL (5006), exit 151, line 2"),
        "{err}"
    );
}

#[test]
fn parse_refuses_embedded_configs() {
    let tla = temp_file("Mono.tla", "---- MODULE Mono ----\n====\n");
    let out = dive(&["parse", "--config", &tla.to_string_lossy()]);
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("unsupported:"));
}

#[test]
fn parse_missing_file_is_usage_error() {
    assert_eq!(
        dive(&["parse", "/nonexistent/dive/Missing.tla"])
            .status
            .code(),
        Some(2)
    );
}
