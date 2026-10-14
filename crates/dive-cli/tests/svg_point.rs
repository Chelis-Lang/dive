//! Public checking and independent replay of the bounded numeric override.
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
fn point_check_and_trace_validation_in_both_profiles() {
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
                &[
                    "--store",
                    "exact",
                    "--workers",
                    "1",
                    "--seed",
                    "0",
                    "--fp",
                    "0",
                ],
                compat,
                native_status,
            );
            let expected = if native_status { 10 } else { 12 };
            assert_eq!(
                checked.status.code(),
                Some(expected),
                "{}",
                String::from_utf8_lossy(&checked.stdout)
            );
            let json: serde_json::Value = serde_json::from_slice(&checked.stdout).expect("report");
            assert_eq!(json["exit_code"], expected);
            assert_eq!(json["tlc"]["code"], 2110);
            assert_eq!(json["property"], "Inv");
            let trace = std::env::temp_dir().join(format!(
                "dive-svg-point-{}-{compat}-{native_status}.json",
                std::process::id()
            ));
            std::fs::write(&trace, &checked.stdout).expect("native trace");
            let valid = run(
                &file,
                &modules,
                "validate-trace",
                &["--trace", trace.to_str().expect("path")],
                compat,
                native_status,
            );
            assert_eq!(valid.status.code(), Some(0));
            let mut doc = dive_api::trace::TraceDoc::from_json(&json).expect("trace transport");
            doc.states[0]["p"]["record"]["x"] = serde_json::json!(-1);
            std::fs::write(&trace, serde_json::to_vec(&doc.to_json()).expect("JSON"))
                .expect("forged trace");
            let invalid = run(
                &file,
                &modules,
                "validate-trace",
                &["--trace", trace.to_str().expect("path")],
                compat,
                native_status,
            );
            assert_eq!(invalid.status.code(), Some(1));
            std::fs::remove_file(trace).expect("cleanup");
        }
    }
}
