//! Sequential graph capture combines name ordering and TLC composition.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{ActionComposition, CheckRequest, ExitConvention, TemporalMode};
use serde_json::json;
use std::path::PathBuf;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/name-cdot")
}
fn check_primary_cases(temporal: TemporalMode) -> TestResult {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/name-cdot/oracle.json"))?;
    for case in oracle["cases"].as_array().ok_or("cases")? {
        for workers in [1, 4] {
            let mut request = CheckRequest {
                file: fixtures().join("NameCdot.tla"),
                config: Some(
                    fixtures().join(format!("{}.cfg", case["case"].as_str().ok_or("case")?)),
                ),
                compat_tlc: true,
                action_composition: Some(ActionComposition::Tlc),
                seed: Some(0),
                fp: Some(0),
                workers: Some(workers),
                ..Default::default()
            };
            assert_eq!(request.temporal, TemporalMode::Ordinary);
            if temporal == TemporalMode::FinalShort {
                request.temporal = TemporalMode::FinalShort;
            }
            let report = dive_api::check_file(&request)?;
            let doc = report.to_json_with_exit_convention(ExitConvention::Tlc);
            assert_eq!(doc["execution"]["capture_completed"], true, "{doc}");
            assert_eq!(doc["options"]["action_composition"], "tlc");
            assert_eq!(doc["options"]["lncheck"], "final");
            assert_eq!(doc["options"]["lasso"], "short");
            if temporal == TemporalMode::Ordinary {
                assert_eq!(doc["options"]["profile"], "tlc");
            }
            for field in ["generated", "distinct", "depth"] {
                assert_eq!(doc["statistics"][field], case[field]);
            }
            assert_eq!(doc["options"]["workers"]["requested"], workers);
            assert_eq!(doc["options"]["workers"]["used"], 1);
            assert_eq!(doc["options"]["workers"]["sequential"], true);
            if case["case"] == "hold" {
                assert_eq!(report.verdict(), "success", "{doc}");
                assert_eq!(doc["exit_code"], case["physical_exit"]);
                let dive_api::temporal::Outcome::Holds(captured) =
                    report.temporal_result().ok_or("temporal result")?.outcome()
                else {
                    return Err("holding graph is required".into());
                };
                let actual = captured
                    .graph
                    .states
                    .iter()
                    .map(|state| match &state[0] {
                        Some(dive_value::Value::Str(value)) => Ok(value.as_str()),
                        _ => Err("expected string state"),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                assert_eq!(
                    serde_json::to_value(actual)?,
                    oracle["word"],
                    "actual primary values"
                );
            } else {
                assert_eq!(report.verdict(), "unsupported", "{doc}");
                assert_eq!(doc["execution"]["stage"], "validation");
                assert_eq!(doc["exit_code"], 3);
                assert!(
                    doc["message"]
                        .as_str()
                        .is_some_and(|message| !message.is_empty())
                );
            }
            assert!(doc["counterexample"].is_null() && doc["property"].is_null());
        }
    }
    Ok(())
}
#[test]
fn explicit_final_keeps_primary_hold_and_safe_failure_without_ordinary_admission() -> TestResult {
    check_primary_cases(TemporalMode::FinalShort)
}
#[test]
fn ordinary_tlc_keeps_mixed_name_composition_primary_values_and_safe_refusal() -> TestResult {
    check_primary_cases(TemporalMode::Ordinary)
}
#[test]
fn independent_composed_word_refusal_keeps_noncomposed_valid_and_forged_controls() -> TestResult {
    let composed =
        "---- MODULE Control ----\nVARIABLE x\nInit==x=0\nNext==(x'=1) \\cdot (x'=2)\n====\n";
    for compat_tlc in [false, true] {
        let opts = TraceOptions {
            compat_tlc,
            ..Default::default()
        };
        let analysis =
            dive_sema::analyze_source("Control.tla", composed, &dive_sema::Options::default());
        assert!(analysis.accepted());
        let ir = std::sync::Arc::new(dive_ir::lower(&analysis)?);
        let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
            .into_config()
            .ok_or("config")?;
        let mut bind_opts = if compat_tlc {
            dive_model::BindOptions::tlc()
        } else {
            dive_model::BindOptions::default()
        };
        bind_opts.eval.action_composition = if compat_tlc {
            dive_eval::ActionComposition::Tlc
        } else {
            dive_eval::ActionComposition::Tla
        };
        let model = dive_model::bind(ir, &cfg, &bind_opts).map_err(|f| format!("{f:?}"))?;
        for x in [2, 9] {
            let doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":x})])?;
            assert_eq!(
                dive_trace::validate_doc(&model, &doc, &[]).0.validation,
                Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
            );
            assert_eq!(model.evaluator.take_output(), [] as [String; 0]);
            // Public TLC TraceOptions retain composition=Off; the default
            // profile enables TLA composition and reaches its replay guard.
            if !compat_tlc {
                let public = SpecModel::from_source(
                    "Control.tla",
                    composed,
                    "INIT Init\nNEXT Next\n",
                    &opts,
                )?;
                assert_eq!(
                    public.validate(&doc).validation,
                    Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
                );
            }
        }
        let plain = SpecModel::from_source(
            "Control.tla",
            &composed.replace("(x'=1) \\cdot (x'=2)", "x'=2"),
            "INIT Init\nNEXT Next\n",
            &opts,
        )?;
        let doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":2})])?;
        assert_eq!(plain.validate(&doc).validation, Validation::Valid);
        let forged = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":9})])?;
        assert!(matches!(
            plain.validate(&forged).validation,
            Validation::Invalid(_)
        ));
    }
    Ok(())
}

/// The one-character fragment's `SubSeq(s,2,2)` sits in a `CHOOSE` domain
/// behind `IF Len(s)=4`, which no reachable one-character state takes. Pinned
/// TLC `tlc-2026.10.06.014338` (`-workers 1 -fp 0 -seed 0`, action
/// composition on) reports success with 5 generated, 2 distinct states and
/// depth 2. dive formerly refused the WF atom with a `SubSeq` error (2183)
/// because its finite-binding preflight evaluated that untaken domain.
#[test]
fn original_short_fragment_matches_the_pinned_success() -> TestResult {
    let report = dive_api::check_file(&CheckRequest {
        file: fixtures().join("original/NameCdot.tla"),
        config: Some(fixtures().join("hold.cfg")),
        compat_tlc: true,
        action_composition: Some(ActionComposition::Tlc),
        temporal: TemporalMode::FinalShort,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..Default::default()
    })?;
    let doc = report.to_json();
    assert_eq!(doc["verdict"], "success", "{doc}");
    assert_eq!(doc["execution"]["capture_completed"], true);
    assert_eq!(doc["statistics"]["generated"], 5);
    assert_eq!(doc["statistics"]["distinct"], 2);
    assert_eq!(doc["statistics"]["depth"], 2);
    assert!(doc["counterexample"].is_null());
    Ok(())
}
