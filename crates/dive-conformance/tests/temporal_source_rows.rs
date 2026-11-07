//! Source-row reports earn agreement only with their actual independently replayed word.

use std::path::PathBuf;
use std::time::Duration;

use dive_api::trace::{SpecModel, TraceOptions};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_engine};
use dive_conformance::compare::{CompareContext, diff_results};
use dive_conformance::dive::{CheckMapping, check_result};
use dive_conformance::process::ProcessOutcome;
use dive_conformance::tlc::result::{OracleResult, Verdict};
use dive_conformance::trace_check::validate_counterexample;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn mapped(value: &Value, profile: bool) -> CheckMapping {
    check_result(
        &ProcessOutcome {
            exit_code: Some(255),
            signal: None,
            timed_out: false,
            stdout: value.to_string(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        },
        profile,
    )
}

fn native(value: &Value, profile: bool) -> Result<OracleResult, Box<dyn std::error::Error>> {
    match mapped(value, profile) {
        CheckMapping::Result(result) if result.verdict == Verdict::OtherError => Ok(*result),
        result => Err(format!("expected classified source-row report: {result:?}").into()),
    }
}

fn rejected(value: &Value, profile: bool) {
    if let CheckMapping::Result(result) = mapped(value, profile) {
        assert_eq!(result.verdict, Verdict::Unclassified);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one actual producer/oracle/independent-reader matrix with adversarial transport controls"
)]
fn real_source_rows_match_official_goldens_and_required_words_cannot_be_forged() -> TestResult {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in [
        "MinChooseFair",
        "MinChooseFairSafeNext",
        "RowActionBeforeState",
        "RowStateAfterQuietAction",
        "RowSelfThenError",
    ] {
        let file = root.join(format!(
            "crates/dive-api/tests/fixtures/fairness-primary/{name}.tla"
        ));
        let corpus = root.join(format!(
            "conformance/corpus/fairness-primary/specifications/{name}/{name}.tla"
        ));
        assert_eq!(
            std::fs::read(&file)?,
            std::fs::read(&corpus)?,
            "API and corpus source identity"
        );
        assert_eq!(
            std::fs::read(file.with_extension("cfg"))?,
            std::fs::read(corpus.with_extension("cfg"))?,
            "actual configuration identity"
        );
        let golden: Value = serde_json::from_str(&std::fs::read_to_string(root.join(format!(
            "conformance/golden/tlc-2026.10.06.014338/fairness-primary/specifications/{name}/{name}.cfg.json"
        )))?)?;
        let oracle: OracleResult = serde_json::from_value(golden["result"].clone())?;
        assert_eq!(oracle.verdict, Verdict::OtherError);
        assert_eq!(oracle.verdict_code, Some(1000));
        assert_eq!(oracle.exit_code, Some(255));
        for compat_tlc in [false, true] {
            let spec = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            for engine in [Engine::Reference, Engine::Core] {
                let value = check_file_engine(
                    &CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc,
                        seed: Some(0),
                        fp: Some(0),
                        workers: Some(1),
                        ..CheckRequest::default()
                    },
                    engine,
                )?
                .to_json_with_exit_convention(ExitConvention::Tlc);
                let result = native(&value, compat_tlc)?;
                assert_eq!(
                    diff_results(
                        &oracle,
                        &result,
                        CompareContext {
                            workers: 1,
                            counts_comparable: true
                        }
                    ),
                    Vec::new(),
                    "{name}/{compat_tlc}/{engine:?}"
                );
                let validation = validate_counterexample(&spec, &result, &oracle);
                assert!(validation.validated, "{name}/{engine:?}: {validation:?}");

                let mut bad = result.clone();
                bad.trace = None;
                assert!(
                    !validate_counterexample(&spec, &bad, &oracle).validated,
                    "verified header cannot replace a required word"
                );
                let mut bad = result.clone();
                bad.verdict_code = Some(2103);
                bad.error_class = Some("TLC_NESTED_EXPRESSION".into());
                bad.exit_code = Some(75);
                assert!(validate_counterexample(&spec, &bad, &oracle).refuted);
                let mut bad = value.clone();
                bad["counterexample"]["trace"]["ending"]["endpoint"]["source"]["module"] =
                    json!("Foreign");
                let bad = native(&bad, compat_tlc)?;
                assert!(
                    validate_counterexample(&spec, &bad, &oracle).refuted,
                    "producer valid marker is no proof"
                );
                for (pointer, replacement) in [
                    ("/counterexample", Value::Null),
                    ("/execution/capture_completed", json!(true)),
                    ("/tlc/exit_status", json!(75)),
                    ("/property", json!("Live")),
                    ("/counterexample/back_to", json!(1)),
                    ("/counterexample/trace/version", json!(4)),
                ] {
                    let mut bad = value.clone();
                    let target = bad.pointer_mut(pointer);
                    if let Some(target) = target {
                        *target = replacement;
                    } else if pointer == "/counterexample/back_to" {
                        bad["counterexample"]["back_to"] = replacement;
                    } else {
                        return Err(format!("missing mutation field {pointer}").into());
                    }
                    rejected(&bad, compat_tlc);
                }
            }
        }
    }
    Ok(())
}
