//! Core refuses the bounded numeric `SVG!PointOnLine` override by name.
#![allow(clippy::expect_used, reason = "functional assertions")]
use std::{path::Path, path::PathBuf, process::Command, process::Output};

fn run(
    file: &Path,
    modules: &Path,
    command: &str,
    extra: &[&str],
    compat: bool,
    native_status: bool,
) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dive"));
    cmd.env_remove("DIVE_COMPAT")
        .env_remove("DIVE_ENGINE")
        .arg(command)
        .arg(file)
        .arg("--output")
        .arg("json")
        .arg("--module-path")
        .arg(modules)
        .args(extra);
    if compat {
        cmd.arg("--compat=tlc");
    }
    if command == "check" && native_status {
        cmd.arg("--exit-codes=native");
    }
    cmd.output().expect("CLI")
}

#[test]
fn point_check_is_a_named_core_refusal_in_both_profiles() {
    // Independent validation of `SVG!PointOnLine` traces is covered by
    // `dive-trace`'s own tests; Core does not implement the operator.
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-api/tests/fixtures/svg-point/Point.tla");
    let modules =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-eval/tests/fixtures/svg-point");
    for compat in [true, false] {
        for native_status in [false, true] {
            let checked = run(
                &file,
                &modules,
                "check",
                &["--workers", "1", "--seed", "0", "--fp", "0"],
                compat,
                native_status,
            );
            assert_eq!(checked.status.code(), Some(3));
            let json: serde_json::Value = serde_json::from_slice(&checked.stdout).expect("report");
            assert_eq!(json["verdict"], "unsupported");
            assert_eq!(
                json["message"],
                "the standard operator SVG!PointOnLine (M0 core)"
            );
            assert!(json["counterexample"].is_null());
        }
    }
}
