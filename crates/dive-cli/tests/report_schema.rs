//! Schema-3 reports for the seven public checking outcomes.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn normalize(value: &mut Value, fixture_prefix: &str) {
    match value {
        Value::String(text) => *text = text.replace(fixture_prefix, "<fixtures>"),
        Value::Array(items) => {
            for item in items {
                normalize(item, fixture_prefix);
            }
        }
        Value::Object(fields) => {
            for (key, item) in fields {
                if matches!(
                    key.as_str(),
                    "elapsed_ms"
                        | "convert_ms"
                        | "compile_ms"
                        | "explore_ms"
                        | "expand_ms"
                        | "commit_ms"
                        | "fold_ms"
                        | "startup_ms"
                ) && item.is_number()
                {
                    *item = if item.is_f64() {
                        serde_json::json!(0.0)
                    } else {
                        serde_json::json!(0)
                    };
                } else {
                    normalize(item, fixture_prefix);
                }
            }
        }
        _ => {}
    }
}

fn report(name: &str, config: &str, extra: &[&str], exit: i32, verdict: &str) -> TestResult {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/report-schema");
    let output = Command::new(env!("CARGO_BIN_EXE_dive"))
        .env_remove("DIVE_COMPAT")
        .env_remove("DIVE_ENGINE")
        .env_remove("DIVE_CORE_OPT")
        .args([
            "check",
            &fixtures.join("Reports.tla").to_string_lossy(),
            "--config",
            &fixtures.join(format!("{config}.cfg")).to_string_lossy(),
        ])
        .args([
            "--engine",
            "core",
            "--workers",
            "1",
            "--seed",
            "0",
            "--fp",
            "0",
            "--time-limit",
            "none",
            "--max-memory",
            "none",
            "--output",
            "json",
        ])
        .args(extra)
        .output()?;
    assert_eq!(
        output.status.code(),
        Some(exit),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{name}: {:?}", output.stderr);
    let mut document: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["schema_version"], 3);
    assert_eq!(document["schema_version"], dive_api::REPORT_SCHEMA_VERSION);
    assert_eq!(document["verdict"], verdict);
    assert_eq!(document["engine"], "core");
    let metric = &mut document["resources"]["memory"]["metric"];
    assert!(matches!(
        metric.as_str(),
        Some("rss+swap" | "rss" | "unavailable")
    ));
    *metric = Value::String("<platform-memory-metric>".into());
    normalize(&mut document, &fixtures.to_string_lossy());
    let golden = fixtures.join(format!("{name}.json"));
    if std::env::var("DIVE_UPDATE_REPORT_GOLDENS").as_deref() == Ok("1") {
        std::fs::write(
            &golden,
            format!("{}\n", serde_json::to_string_pretty(&document)?),
        )?;
    }
    let expected: Value = serde_json::from_slice(&std::fs::read(&golden)?)?;
    assert!(
        document == expected,
        "{name}: report differs from {}",
        golden.display()
    );
    Ok(())
}

#[test]
fn success() -> TestResult {
    report("success", "Success", &[], 0, "success")
}
#[test]
fn invariant_violation() -> TestResult {
    report("invariant", "Invariant", &[], 12, "invariant-violation")
}
#[test]
fn deadlock() -> TestResult {
    report("deadlock", "Deadlock", &[], 11, "deadlock")
}
#[test]
fn liveness_violation() -> TestResult {
    report("liveness", "Liveness", &[], 13, "liveness-violation")
}
#[test]
fn evaluation_error() -> TestResult {
    report(
        "evaluation-error",
        "EvaluationError",
        &[],
        76,
        "evaluation-error",
    )
}
#[test]
fn refusal() -> TestResult {
    report(
        "refusal",
        "Liveness",
        &["--store", "exact"],
        3,
        "unsupported",
    )
}
#[test]
fn budget_stop() -> TestResult {
    report(
        "budget-stop",
        "Success",
        &["--max-states", "2"],
        5,
        "stopped",
    )
}
