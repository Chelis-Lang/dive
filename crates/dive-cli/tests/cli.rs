//! Smoke tests for the `dive` binary's command surface.

use std::process::Command;

fn dive(args: &[&str]) -> std::process::Output {
    match Command::new(env!("CARGO_BIN_EXE_dive")).args(args).output() {
        Ok(output) => output,
        Err(e) => unreachable!("cannot run dive binary: {e}"),
    }
}

#[test]
fn version_succeeds() {
    let out = dive(&["--version"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("dive "));
}

#[test]
fn unimplemented_commands_refuse_explicitly() {
    for cmd in [
        "check",
        "simulate",
        "parse",
        "translate",
        "validate-trace",
        "lsp",
    ] {
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
