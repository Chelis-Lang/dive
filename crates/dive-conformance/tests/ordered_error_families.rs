//! Word/report proof does not classify an ambiguous pinned oracle stream.
use dive_api::trace::{SpecModel, TraceOptions};
use dive_api::{CheckRequest, Engine, check_file_engine};
use dive_conformance::compare::{CompareContext, diff_results};
use dive_conformance::dive::{CheckMapping, check_result_with_model};
use dive_conformance::process::ProcessOutcome;
use dive_conformance::tlc::result::{RunFacts, Verdict};
use dive_conformance::tlc::{messages, result};
use dive_conformance::trace_check::validate_counterexample;
use std::path::PathBuf;
use std::time::Duration;
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one fixture matrix checks both profiles, the agreed record, exit conventions and independent words"
)]
fn error_family_shadows_preserve_the_pinned_oracle_classification() -> TestResult {
    // The results the retired reference engine and Core agreed on before the
    // reference engine was removed (#791): Core must still equal them.
    let agreed: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/agreed/ordered-error-families.json"))?;
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-api/tests/fixtures/checker/errors");
    for name in [
        "ErViewChoose",
        "ErViewDomain",
        "ErViewNonBool",
        "ErViewNonBoolIf",
        "ErViewNonEnum",
        "ErViewTypeErr",
        "ErViewAssert",
        "ErViewDivZero",
        "ErViewEnabled",
        "ErViewOverflow",
        "ErInvConstrained",
        "ErInvReplayError",
        "ErConstrNonBool",
        "ErActConstrNonBool",
    ] {
        let file = root.join(format!("specs/{name}/{name}.tla"));
        let clean =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))?;
        let exit =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))?
                .trim()
                .parse()?;
        let oracle = result::classify(
            &messages::parse(&clean),
            &RunFacts {
                exit_code: Some(exit),
                ..RunFacts::default()
            },
        );
        assert_eq!(
            oracle.verdict,
            match name {
                "ErViewAssert" => Verdict::AssertionFailure,
                "ErViewDivZero" | "ErViewEnabled" => Verdict::EvaluationError,
                "ErViewOverflow" => Verdict::Overflow,
                _ => Verdict::Unclassified,
            },
            "{name}"
        );
        for compat_tlc in [false, true] {
            if name == "ErViewOverflow" && !compat_tlc {
                continue;
            }
            let model = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            for workers in [1, 4] {
                let key = format!("{name}/{compat_tlc}/{workers}");
                let recorded: dive_conformance::tlc::result::OracleResult = serde_json::from_value(
                    agreed
                        .get(&key)
                        .ok_or_else(|| format!("no agreed result for {key}"))?
                        .clone(),
                )?;
                let mut pair = vec![recorded];
                {
                    let engine = Engine::Core;
                    let report = check_file_engine(
                        &CheckRequest {
                            file: file.clone(),
                            config: Some(file.with_extension("cfg")),
                            compat_tlc,
                            workers: Some(workers),
                            seed: Some(0),
                            fp: Some(0),
                            ..CheckRequest::default()
                        },
                        engine,
                    )?;
                    let wire = report.to_json_with_exit_convention(dive_api::ExitConvention::Tlc);
                    let outcome = ProcessOutcome {
                        exit_code: Some(i32::from(
                            report.exit_code_for(dive_api::ExitConvention::Tlc),
                        )),
                        signal: None,
                        timed_out: false,
                        stdout: wire.to_string(),
                        stderr: String::new(),
                        elapsed: Duration::ZERO,
                    };
                    let CheckMapping::Result(actual) =
                        check_result_with_model(&outcome, compat_tlc, &model)
                    else {
                        return Err(format!(
                            "source-bound mapping refused {name}/{engine:?}: {wire}"
                        )
                        .into());
                    };
                    assert!(
                        validate_counterexample(&model, &actual, &oracle).validated,
                        "{name}/{engine:?}"
                    );
                    if name.starts_with("ErView") {
                        assert!(matches!(
                            dive_conformance::dive::check_result(&outcome, compat_tlc),
                            CheckMapping::Refused(_)
                        ));
                        let mut forged_wire = wire.clone();
                        forged_wire["tlc"]["exit_status"] = serde_json::json!(76);
                        forged_wire["exit_code"] = serde_json::json!(76);
                        let mut forged_outcome = outcome.clone();
                        forged_outcome.exit_code = Some(76);
                        forged_outcome.stdout = forged_wire.to_string();
                        assert!(matches!(
                            check_result_with_model(&forged_outcome, compat_tlc, &model),
                            CheckMapping::Refused(_)
                        ));
                        let mut forged_result = actual.clone();
                        forged_result.exit_code = Some(76);
                        forged_result.exit_status = Some("FAILURE_SAFETY_EVAL".into());
                        assert!(validate_counterexample(&model, &forged_result, &oracle).refuted);

                        let native_wire =
                            report.to_json_with_exit_convention(dive_api::ExitConvention::Native);
                        let mut native_outcome = outcome.clone();
                        native_outcome.exit_code = Some(13);
                        native_outcome.stdout = native_wire.to_string();
                        let CheckMapping::Result(native) =
                            dive_conformance::dive::check_result_with_model_and_exit_convention(
                                &native_outcome,
                                compat_tlc,
                                dive_api::ExitConvention::Native,
                                &model,
                            )
                        else {
                            return Err(format!("{name}: explicit native report refused").into());
                        };
                        assert!(dive_conformance::trace_check::validate_counterexample_with_exit_convention(
                            &model, &native, &oracle, dive_api::ExitConvention::Native
                        ).validated);
                        assert!(matches!(
                            check_result_with_model(&native_outcome, compat_tlc, &model),
                            CheckMapping::Refused(_)
                        ));
                    }
                    if oracle.verdict != Verdict::Unclassified {
                        assert!(
                            diff_results(
                                &oracle,
                                &actual,
                                CompareContext {
                                    workers: u32::try_from(workers)?,
                                    counts_comparable: true,
                                }
                            )
                            .is_empty(),
                            "{name}/{engine:?}: {:?}",
                            diff_results(
                                &oracle,
                                &actual,
                                CompareContext {
                                    workers: u32::try_from(workers)?,
                                    counts_comparable: true
                                }
                            )
                        );
                    }
                    pair.push(*actual);
                }
                assert!(
                    diff_results(
                        &pair[0],
                        &pair[1],
                        CompareContext {
                            workers: u32::try_from(workers)?,
                            counts_comparable: true
                        }
                    )
                    .is_empty(),
                    "{name}/{compat_tlc}/{workers}"
                );
            }
        }
    }
    Ok(())
}
