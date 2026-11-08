//! Actual ordinary final reports retain strict facts and independent replay.
use std::path::PathBuf;
use std::time::Duration;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions};
use dive_api::{CheckRequest, ExitConvention};
use dive_conformance::compare::{CompareContext, diff_results};
use dive_conformance::dive::{CheckMapping, check_result};
use dive_conformance::process::ProcessOutcome;
use dive_conformance::tlc::result::{OracleResult, TraceEnding, Verdict};
use dive_conformance::trace_check::validate_counterexample;
use dive_conformance::util::TempDir;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A temporal report the retired reference engine produced (#791) for a run
/// Core now refuses by name; it remains real producer output for the mapping
/// checks. `core` is Core's report of the same run, which must be a refusal
/// that publishes no word.
fn recorded_after_core_refusal(
    core: &Value,
    key: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    assert_eq!(core["verdict"], "unsupported", "{core}");
    assert_eq!(core["exit_code"], 3, "{core}");
    assert!(core["counterexample"].is_null() && core["property"].is_null());
    let recorded: Value = serde_json::from_str(include_str!(
        "fixtures/retired-reference/ordinary-temporal-refusals.json"
    ))?;
    Ok(recorded
        .get(key)
        .ok_or_else(|| format!("no recorded report for {key}"))?
        .clone())
}
struct Spec {
    file: PathBuf,
    _directory: TempDir,
}
impl Spec {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let directory = TempDir::new("dive-transport-final")?;
        let file = directory.path().join("Ordinary.tla");
        std::fs::write(
            &file,
            "---- MODULE Ordinary ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nP == <>(x=3)\n====\n",
        )?;
        std::fs::write(
            file.with_extension("cfg"),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )?;
        Ok(Self {
            file,
            _directory: directory,
        })
    }
    fn report(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let request = CheckRequest {
            file: self.file.clone(),
            compat_tlc: true,
            fp: Some(0),
            seed: Some(0),
            workers: Some(1),
            ..CheckRequest::default()
        };
        Ok(
            dive_api::check_file_engine(&request, dive_api::Engine::Core)?
                .to_json_with_exit_convention(ExitConvention::Tlc),
        )
    }
    fn model(&self) -> Result<SpecModel, Box<dyn std::error::Error>> {
        Ok(SpecModel::load(
            &self.file,
            &self.file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc: true,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )?)
    }
}
fn mapped(value: &Value, status: Option<i32>, profile: bool) -> CheckMapping {
    check_result(
        &ProcessOutcome {
            exit_code: status,
            signal: None,
            timed_out: false,
            stdout: value.to_string(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        },
        profile,
    )
}
fn fact(value: &Value) -> Result<OracleResult, Box<dyn std::error::Error>> {
    match mapped(value, Some(13), true) {
        CheckMapping::Result(result) if result.verdict == Verdict::LivenessViolation => Ok(*result),
        _ => Err("expected a classified liveness result".into()),
    }
}
fn rejected(value: &Value, status: Option<i32>, profile: bool) {
    match mapped(value, status, profile) {
        CheckMapping::Refused(_) => {}
        CheckMapping::Result(result) => assert_eq!(result.verdict, Verdict::Unclassified),
    }
}

#[test]
fn actual_capture_counts_survive_the_print_marker_and_words_replay_independently() -> TestResult {
    let spec = Spec::new()?;
    let value = spec.report()?;
    assert_eq!(value["statistics"]["reported_by_tlc"], false);
    let result = fact(&value)?;
    assert_eq!(result.counts.states_generated, Some(4));
    assert_eq!(result.counts.distinct_states, Some(3));
    assert_eq!(result.counts.states_left_on_queue, Some(0));
    assert_eq!(result.counts.depth, Some(3));
    assert_eq!(
        result.error_class.as_deref(),
        Some("TLC_TEMPORAL_PROPERTY_VIOLATED")
    );
    assert_eq!(result.exit_code, Some(13));
    assert_eq!(result.violated_property.as_deref(), Some("P"));
    let model = spec.model()?;
    assert!(validate_counterexample(&model, &result, &result).validated);
    // A coherent transport with a producer's "valid" marker is still no proof.
    let mut forged = value.clone();
    let mut document = TraceDoc::from_json(&forged["counterexample"]["trace"])?;
    document.states[0].insert("x".into(), json!(99));
    forged["counterexample"]["trace"] = document.to_json();
    forged["counterexample"]["states"][0]["values"]["x"] = json!(99);
    let forged = fact(&forged)?;
    let validation = validate_counterexample(&model, &forged, &result);
    assert!(validation.refuted);
    assert!(!validation.validated);
    Ok(())
}

#[test]
fn partial_facts_wrong_statuses_and_contradictory_words_are_never_oracle_facts() -> TestResult {
    let spec = Spec::new()?;
    let value = spec.report()?;
    for (pointer, replacement) in [
        ("/execution/capture_completed", json!(false)),
        ("/execution/exhausted", json!(false)),
        ("/execution/stage", json!("capture")),
        ("/statistics/distinct", Value::Null),
        ("/statistics/generated", json!(-1)),
        ("/statistics/depth", Value::Null),
        ("/statistics/left_on_queue", json!(1)),
        ("/options/profile", json!("tlc+overrides")),
        ("/options/lncheck", json!("default")),
        ("/options/lasso", json!("tlc")),
        ("/options/exit_codes", json!("dive")),
        ("/tlc/code", json!(2193)),
        ("/tlc/name", json!("TLC_SUCCESS")),
        ("/tlc/verified", json!(false)),
        ("/tlc/exit_status", json!(12)),
        ("/exit_code", json!(12)),
        ("/counterexample/back_to", json!(0)),
        ("/counterexample/trace", Value::Null),
        ("/counterexample/states/0/values/x", json!(99)),
        ("/properties", json!(["P", "P"])),
        ("/property", json!("Other")),
    ] {
        let mut changed = value.clone();
        *changed
            .pointer_mut(pointer)
            .ok_or("mutation field must exist")? = replacement;
        rejected(&changed, Some(13), true);
    }
    for status in [None, Some(0), Some(12), Some(255)] {
        rejected(&value, status, true);
    }
    rejected(&value, Some(13), false);
    Ok(())
}

#[test]
fn different_valid_periodic_words_do_not_waive_any_observable_fact() -> TestResult {
    let spec = Spec::new()?;
    let value = spec.report()?;
    let original = fact(&value)?;
    let mut alternate = original.clone();
    let trace = alternate.trace.as_mut().ok_or("periodic word")?;
    let mut document = TraceDoc::from_json(trace.json.as_ref().ok_or("lossless word")?)?;
    // This model can reach 2 and stutter there as well as stutter initially.
    document.states = (0..=2)
        .map(|x| {
            let mut state = serde_json::Map::new();
            state.insert("x".into(), json!(x));
            state
        })
        .collect();
    document.labels = vec![None; 3];
    document.ending = Some(dive_api::trace::Ending::Lasso { back_to: 2 });
    document.length = Some(3);
    trace.length = 3;
    trace.ending = TraceEnding::Lasso { back_to: Some(3) };
    trace.json = Some(document.to_json());
    let model = spec.model()?;
    assert!(validate_counterexample(&model, &original, &original).validated);
    assert!(validate_counterexample(&model, &alternate, &original).validated);
    let context = CompareContext {
        workers: 1,
        counts_comparable: true,
    };
    assert_eq!(diff_results(&original, &alternate, context), Vec::new());
    for field in [
        "generated",
        "distinct",
        "verdict",
        "depth",
        "class",
        "exit",
        "property",
    ] {
        let mut changed = alternate.clone();
        match field {
            "generated" => changed.counts.states_generated = Some(999),
            "distinct" => changed.counts.distinct_states = Some(999),
            "verdict" => changed.verdict = Verdict::Success,
            "depth" => changed.counts.depth = Some(999),
            "class" => changed.error_class = Some("TLC_SUCCESS".into()),
            "exit" => changed.exit_code = Some(12),
            _ => changed.violated_property = Some("Other".into()),
        }
        assert!(
            !diff_results(&original, &changed, context).is_empty(),
            "{field}"
        );
    }
    let mut different_queue = alternate.clone();
    different_queue.counts.states_left_on_queue = Some(999);
    assert_eq!(
        diff_results(&original, &different_queue, context),
        Vec::new()
    );
    let mut finite = alternate;
    finite.trace.as_mut().ok_or("trace")?.ending = TraceEnding::Finite;
    assert_ne!(diff_results(&original, &finite, context), Vec::new());
    Ok(())
}

#[test]
fn actual_pure_state_property_dispatch_has_no_unused_temporal_policy() -> TestResult {
    let spec = Spec::new()?;
    let source = std::fs::read_to_string(&spec.file)?.replace("P == <>(x=3)", "P == x<=2");
    std::fs::write(&spec.file, source)?;
    for compat_tlc in [false, true] {
        let request = CheckRequest {
            file: spec.file.clone(),
            compat_tlc,
            fp: Some(0),
            seed: Some(0),
            workers: Some(1),
            ..CheckRequest::default()
        };
        let report = dive_api::check_file_engine(&request, dive_api::Engine::Core)?
            .to_json_with_exit_convention(ExitConvention::Tlc);
        assert_eq!(report["schema_version"], 3);
        assert_eq!(report["check_kind"], "safety");
        assert_eq!(report["verdict"], "success");
        for field in ["temporal_bounds", "lncheck", "lasso"] {
            assert!(report["options"].get(field).is_none(), "{field}");
        }
        assert!(report.get("execution").is_none());
        let CheckMapping::Result(result) = mapped(&report, Some(0), compat_tlc) else {
            return Err("classified pure-state safety result".into());
        };
        assert_eq!(result.verdict, Verdict::Success);
        assert_eq!(result.counts.distinct_states, Some(3));
        assert_eq!(result.counts.states_generated, Some(4));
        assert_eq!(result.counts.depth, Some(3));
        let mut forged = report.clone();
        forged["options"]["temporal_bounds"] = json!({});
        rejected(&forged, Some(0), compat_tlc);
        let mut missing_kind = report;
        missing_kind
            .as_object_mut()
            .ok_or("object")?
            .remove("check_kind");
        rejected(&missing_kind, Some(0), compat_tlc);
    }
    Ok(())
}

#[test]
fn actual_finite_safety_words_keep_strict_length_and_ending_comparisons() -> TestResult {
    let spec = Spec::new()?;
    let source = std::fs::read_to_string(&spec.file)?.replace("P == <>(x=3)", "P == [](x<2)");
    std::fs::write(&spec.file, source)?;
    let report = spec.report()?;
    assert_eq!(report["schema_version"], 3);
    assert_eq!(report["check_kind"], "safety");
    let CheckMapping::Result(result) = mapped(&report, Some(12), true) else {
        return Err("finite safety result".into());
    };
    assert_eq!(result.verdict, Verdict::SafetyViolation);
    let context = CompareContext {
        workers: 1,
        counts_comparable: true,
    };
    let mut wrong_length = (*result).clone();
    wrong_length.trace.as_mut().ok_or("finite trace")?.length += 1;
    assert!(
        diff_results(&result, &wrong_length, context)
            .iter()
            .any(|diff| diff.field == "trace.length")
    );
    let mut wrong_ending = (*result).clone();
    wrong_ending.trace.as_mut().ok_or("finite trace")?.ending = TraceEnding::Stuttering;
    assert!(
        diff_results(&result, &wrong_ending, context)
            .iter()
            .any(|diff| diff.field == "trace.ending")
    );
    Ok(())
}

#[test]
fn actual_error_transport_keeps_null_counts_and_cannot_claim_success() -> TestResult {
    let spec = Spec::new()?;
    let source = std::fs::read_to_string(&spec.file)?.replace("P == <>(x=3)", "P == <>1");
    std::fs::write(&spec.file, source)?;
    let report = recorded_after_core_refusal(&spec.report()?, "final-planning-true")?;
    assert!(report["statistics"].is_null());
    let CheckMapping::Result(result) = mapped(&report, Some(255), true) else {
        return Err("classified planning evaluation error".into());
    };
    assert_eq!(result.verdict, Verdict::EvaluationError);
    assert_eq!(result.error_class.as_deref(), Some("TLC_EXPECTED_VALUE"));
    assert_eq!(
        result.counts,
        dive_conformance::tlc::result::Counts::default()
    );
    assert!(result.trace.is_none());
    let mut forged = report;
    forged["tlc"] = json!({"code":2193,"name":"TLC_SUCCESS","verified":true,"exit_status":0});
    forged["exit_code"] = json!(0);
    rejected(&forged, Some(0), true);
    Ok(())
}

#[test]
fn native_multiworker_generated_variation_keeps_the_other_required_fields_strict() -> TestResult {
    let spec = Spec::new()?;
    let original = fact(&spec.report()?)?;
    let mut changed = original.clone();
    changed.counts.states_generated = Some(999);
    let one = CompareContext {
        workers: 1,
        counts_comparable: true,
    };
    let four = CompareContext {
        workers: 4,
        counts_comparable: true,
    };
    assert!(
        diff_results(&original, &changed, one)
            .iter()
            .any(|diff| diff.field == "counts.states_generated")
    );
    assert_eq!(diff_results(&original, &changed, four), Vec::new());
    for field in ["distinct", "depth", "verdict", "class", "exit", "property"] {
        let mut changed = original.clone();
        match field {
            "distinct" => changed.counts.distinct_states = Some(999),
            "depth" => changed.counts.depth = Some(999),
            "verdict" => changed.verdict = Verdict::Success,
            "class" => changed.error_class = Some("TLC_SUCCESS".into()),
            "exit" => changed.exit_code = Some(12),
            _ => changed.violated_property = Some("Other".into()),
        }
        assert!(
            !diff_results(&original, &changed, four).is_empty(),
            "{field}"
        );
    }
    Ok(())
}

#[test]
fn ordinary_route_matches_read_historical_pinned_final_controls() -> TestResult {
    use dive_conformance::tlc::{
        messages,
        result::{RunFacts, classify},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes");
    for name in ["LvNoFair", "LvWeakFair", "LvWFIntermittent", "LvActionProp"] {
        let recorded = root.join("tlc-2026.10.06.014338");
        let command = std::fs::read_to_string(recorded.join(format!("{name}.command")))?;
        assert!(command.contains("-workers 1 -lncheck final"));
        assert!(command.contains("-fp 0 -seed 0"));
        let stdout = std::fs::read_to_string(recorded.join(format!("{name}.stdout")))?;
        let status = std::fs::read_to_string(recorded.join(format!("{name}.exit")))?
            .trim()
            .parse()?;
        let oracle = classify(
            &messages::parse(&stdout),
            &RunFacts {
                exit_code: Some(status),
                ..RunFacts::default()
            },
        );
        assert!(oracle.verdict.is_oracle_fact());
        let file = root.join("specs").join(name).join(format!("{name}.tla"));
        for workers in [1, 4] {
            let request = CheckRequest {
                file: file.clone(),
                compat_tlc: true,
                fp: Some(0),
                seed: Some(0),
                workers: Some(workers),
                ..CheckRequest::default()
            };
            let report = dive_api::check_file_engine(&request, dive_api::Engine::Core)?;
            let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
            let CheckMapping::Result(result) = mapped(
                &value,
                Some(i32::from(report.exit_code_for(ExitConvention::Tlc))),
                true,
            ) else {
                return Err(format!("{name}: ordinary route refused").into());
            };
            let diffs = diff_results(
                &oracle,
                &result,
                CompareContext {
                    workers: u32::try_from(workers)?,
                    counts_comparable: true,
                },
            );
            assert!(diffs.is_empty(), "{name} w{workers}: {diffs:?}");
            if result.verdict == Verdict::LivenessViolation {
                let model = SpecModel::load(
                    &file,
                    &file.with_extension("cfg"),
                    &TraceOptions {
                        compat_tlc: true,
                        run: Some((0, 0)),
                        ..TraceOptions::default()
                    },
                )?;
                assert!(
                    validate_counterexample(&model, &result, &oracle).validated,
                    "{name}"
                );
            }
        }
    }
    Ok(())
}
