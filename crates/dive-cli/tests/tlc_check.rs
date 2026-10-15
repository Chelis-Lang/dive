//! Checking entrypoints, transport, and retained refusal/validation boundaries.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tlc-check")
}

fn run(binary: &str, args: &[String], cwd: &Path) -> std::io::Result<Output> {
    Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .env("DIVE_COMPAT", "dive")
        .env("DIVE_CHAOS", "99")
        .output()
}

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).into()).collect()
}

fn adapter(args: &[String], cwd: &Path) -> std::io::Result<Output> {
    let mut all = vec!["tlc".into()];
    all.extend_from_slice(args);
    run(env!("CARGO_BIN_EXE_dive"), &all, cwd)
}

#[test]
fn supported_safety_outcomes_retain_historical_primary_fields() -> TestResult {
    // Retained old oracle data is a regression expectation, not a new primary
    // measurement or a passing result for this source before the test is run.
    let primary: Value = serde_json::from_str(include_str!("fixtures/tlc-check/primary.json"))?;
    for case in primary["cases"].as_array().ok_or("primary cases")? {
        let mut args = words(&["-seed", "7", "-fp", "0", "--output", "json"]);
        for arg in case["args"].as_array().ok_or("argv")? {
            args.push(arg.as_str().ok_or("argument")?.into());
        }
        args.push(case["module"].as_str().ok_or("module")?.into());
        for shim in [false, true] {
            let output = if shim {
                run(env!("CARGO_BIN_EXE_tlc"), &args, &fixtures())?
            } else {
                adapter(&args, &fixtures())?
            };
            assert_eq!(output.status.code().map(i64::from), case["exit"].as_i64());
            assert!(output.stderr.is_empty(), "{output:?}");
            let doc: Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(doc["verdict"], case["verdict"]);
            assert_eq!(doc["property"], case["property"]);
            assert_eq!(doc["tlc"]["code"], case["code"]);
            assert_eq!(doc["exit_code"], case["exit"]);
            assert_eq!(doc["options"]["profile"], "tlc");
            assert_eq!(doc["options"]["workers"]["requested"], 1);
            assert_eq!(doc["options"]["seed"], 7);
            if case["statistics"].is_object() {
                for key in ["generated", "distinct", "left_on_queue", "depth"] {
                    assert_eq!(doc["statistics"][key], case["statistics"][key]);
                }
            } else {
                assert_ne!(doc["statistics"]["reported_by_tlc"], true);
            }
            let states = doc["counterexample"]["states"].as_array();
            let values: Vec<_> = states
                .into_iter()
                .flatten()
                .map(|s| s["values"]["x"].clone())
                .collect();
            assert_eq!(json!(values), case["trace_values"]);
            if !values.is_empty() {
                assert_eq!(doc["counterexample"]["validation"], "valid");
            }
        }
    }
    Ok(())
}

#[test]
fn absolute_root_relative_config_and_existing_aliases_transport_options() -> TestResult {
    for (module, config) in [("Probe", "Dead"), ("Probe.tla", "Dead.cfg")] {
        let args = vec![
            fixtures().join(module).display().to_string(),
            "--config".into(),
            config.into(),
            "-deadlock".into(),
            "--seed=-9223372036854775808".into(),
            "--fp=130".into(),
            "--workers=2".into(),
            "--output=json".into(),
        ];
        let output = adapter(&args, Path::new(env!("CARGO_MANIFEST_DIR")))?;
        assert!(output.status.success(), "{output:?}");
        let doc: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(
            doc["config"],
            fixtures().join("Dead.cfg").display().to_string()
        );
        assert_eq!(doc["options"]["seed"], i64::MIN);
        assert_eq!(doc["options"]["fingerprint"]["polynomial_index"], 130);
        assert_eq!(doc["options"]["workers"]["requested"], 2);
        assert_eq!(doc["options"]["deadlock_flag"], true);
    }
    // Adapter normalization must not change file-oriented public API semantics.
    let request = dive_api::CheckRequest {
        file: fixtures().join("Probe"),
        ..Default::default()
    };
    assert!(matches!(
        dive_api::check_file(&request),
        Err(dive_api::ApiError::Io { .. })
    ));
    Ok(())
}

#[test]
fn final_spelling_uses_existing_qualified_backend_without_extra_policy_flag() -> TestResult {
    for (config, exit, verdict) in [
        ("FailLive", 13, "liveness-violation"),
        ("HoldLive", 0, "success"),
    ] {
        let args = words(&[
            "Temporal", "-config", config, "-lncheck", "final", "-seed", "0", "-fp", "0",
            "--output", "json",
        ]);
        let output = adapter(&args, &fixtures())?;
        assert_eq!(output.status.code(), Some(exit), "{output:?}");
        let actual: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(actual["verdict"], verdict);
        assert_eq!(actual["options"]["profile"], "tlc+overrides");
        let native = run(
            env!("CARGO_BIN_EXE_dive"),
            &words(&[
                "check",
                "Temporal.tla",
                "--config",
                &format!("{config}.cfg"),
                "--compat=tlc",
                "--lncheck",
                "final",
                "--lasso",
                "short",
                "--seed",
                "0",
                "--fp",
                "0",
                "--workers",
                "1",
                "--output",
                "json",
            ]),
            &fixtures(),
        )?;
        let reference: Value = serde_json::from_slice(&native.stdout)?;
        for key in ["verdict", "property", "statistics", "execution"] {
            assert_eq!(actual[key], reference[key]);
        }
        if exit == 13 {
            assert_eq!(actual["counterexample"]["validation"], "valid");
            assert_eq!(
                actual["counterexample"]["trace"],
                reference["counterexample"]["trace"]
            );
        }
    }
    let output = adapter(
        &words(&[
            "Temporal", "-config", "FailLive", "-lncheck", "default", "--output", "json",
        ]),
        &fixtures(),
    )?;
    assert_eq!(output.status.code(), Some(3));
    let doc: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(doc["verdict"], "unsupported");
    assert!(doc["counterexample"].is_null());
    Ok(())
}

#[test]
fn unsupported_modes_and_tool_flags_refuse_before_input_access() -> TestResult {
    for flag in [
        "-tool",
        "-simulate",
        "-generate",
        "-dfid",
        "-checkpoint",
        "-recover",
        "-metadir",
        "-cleanup",
        "-debugger",
        "-coverage",
        "-dumpTrace",
    ] {
        let output = adapter(&words(&["missing-model", flag]), &fixtures())?;
        assert_eq!(output.status.code(), Some(3), "{flag}: {output:?}");
        assert_eq!(output.stdout, [] as [u8; 0]);
        let error = String::from_utf8(output.stderr)?;
        assert!(error.contains("DIVE-X0002") && error.contains(flag));
    }
    for args in [
        words(&["-workers", "0"]),
        words(&["-fp", "131"]),
        words(&["-seed", "9223372036854775808"]),
        words(&["-unknown"]),
        words(&[]),
    ] {
        let output = adapter(&args, &fixtures())?;
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(output.stdout, [] as [u8; 0]);
        assert!(String::from_utf8(output.stderr)?.contains("DIVE-X0001"));
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn argv_zero_basename_selects_adapter_on_the_dive_binary() -> TestResult {
    use std::os::unix::process::CommandExt as _;
    let output = Command::new(env!("CARGO_BIN_EXE_dive"))
        .arg0("/renamed/location/tlc")
        .args(["Probe", "-seed", "7", "-fp", "0", "--output", "json"])
        .current_dir(fixtures())
        .output()?;
    assert!(output.status.success(), "{output:?}");
    let doc: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(doc["options"]["profile"], "tlc");
    assert_eq!(doc["options"]["workers"]["requested"], 1);
    Ok(())
}
