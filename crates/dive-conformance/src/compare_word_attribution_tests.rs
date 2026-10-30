//! Actual independent replay is the only permission for word-relative names.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "small pinned attribution fixtures"
)]

use super::*;
use crate::golden::ModelGolden;
use crate::trace_check::{TraceCheck, validate_counterexample};
use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions};
use serde_json::json;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn case(tlc: bool) -> (SpecModel, OracleResult, OracleResult) {
    let base = root().join("conformance/corpus/word-relative-liveness/specifications/Mutual");
    let model = SpecModel::load(
        &base.join("Mutual.tla"),
        &base.join("Mutual.cfg"),
        &TraceOptions {
            compat_tlc: tlc,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("independent model");
    let path = root().join("conformance/golden/tlc-2026.10.06.014338/word-relative-liveness/specifications/Mutual/Mutual.cfg.json");
    let golden: ModelGolden = serde_json::from_str(&std::fs::read_to_string(path).expect("golden"))
        .expect("pinned model golden");
    assert_eq!(golden.result.violated_property.as_deref(), Some("P"));
    let mut observed = golden.result.clone();
    let doc = TraceDoc::behavior(vec![json!({"x":0})])
        .expect("word")
        .with_ending(Ending::Lasso { back_to: 0 })
        .with_properties(vec!["P".into(), "Q".into()]);
    let trace = observed.trace.as_mut().expect("primary word");
    trace.length = 1;
    trace.ending = TraceEnding::Lasso { back_to: Some(1) };
    trace.steps = None;
    trace.json = Some(doc.to_json());
    observed.violated_property = Some("P and Q".into());
    (model, golden.result, observed)
}

fn differences(golden: &OracleResult, observed: &OracleResult) -> Vec<FieldDiff> {
    diff_results(
        golden,
        observed,
        CompareContext {
            workers: 1,
            counts_comparable: true,
        },
    )
}

#[test]
fn different_valid_word_names_need_real_complete_attribution() {
    for tlc in [false, true] {
        let (model, golden, observed) = case(tlc);
        let mut diffs = differences(&golden, &observed);
        assert_eq!(
            diffs.iter().map(|d| d.field.as_str()).collect::<Vec<_>>(),
            ["violated_property"]
        );
        // A generic hook's successful validation claim supplies no attribution.
        assert!(!apply_word_attribution(
            &mut diffs,
            &golden,
            &observed,
            &TraceCheck::yes("claimed valid")
        ));
        assert_eq!(diffs.len(), 1);
        let proof = validate_counterexample(&model, &observed, &golden);
        assert!(proof.validated, "{proof:?}");
        assert!(apply_word_attribution(
            &mut diffs, &golden, &observed, &proof
        ));
        assert_eq!(diffs, Vec::<FieldDiff>::new());
        let mut altered = observed.clone();
        altered.violated_property = Some("Q".into());
        let mut diffs = differences(&golden, &altered);
        assert!(!apply_word_attribution(
            &mut diffs, &golden, &altered, &proof
        ));
        assert_eq!(diffs.len(), 1);
    }
}

#[test]
fn missing_forged_omitted_or_misreported_names_never_qualify() {
    for tlc in [false, true] {
        let (model, golden, observed) = case(tlc);
        for mutation in [
            "absent",
            "no-names",
            "omitted",
            "false",
            "wrong-report",
            "unknown",
            "duplicate",
            "reordered",
            "init",
        ] {
            let mut bad = observed.clone();
            if mutation == "absent" {
                bad.trace = None;
            } else if mutation == "wrong-report" {
                bad.violated_property = Some("Q".into());
            } else {
                let mut doc = TraceDoc::from_json(
                    bad.trace
                        .as_ref()
                        .expect("word")
                        .json
                        .as_ref()
                        .expect("JSON"),
                )
                .expect("read word");
                match mutation {
                    "no-names" => {
                        doc.properties.clear();
                        bad.violated_property = None;
                    }
                    "omitted" => {
                        doc.properties = vec!["P".into()];
                        bad.violated_property = Some("P".into());
                    }
                    "false" => {
                        doc.states
                            .push(json!({"x":2}).as_object().expect("state").clone());
                        doc.length = Some(2);
                        doc.ending = Some(Ending::Lasso { back_to: 1 });
                        bad.trace.as_mut().expect("word").length = 2;
                        bad.trace.as_mut().expect("word").ending =
                            TraceEnding::Lasso { back_to: Some(2) };
                    }
                    "unknown" => {
                        doc.properties = vec!["Missing".into()];
                        bad.violated_property = Some("Missing".into());
                    }
                    "duplicate" => {
                        doc.properties = vec!["P".into(), "P".into()];
                        bad.violated_property = Some("P and P".into());
                    }
                    "reordered" => {
                        doc.properties = vec!["Q".into(), "P".into()];
                        bad.violated_property = Some("Q and P".into());
                    }
                    "init" => {
                        doc.states[0] = json!({"x":9}).as_object().expect("state").clone();
                    }
                    _ => unreachable!(),
                }
                bad.trace.as_mut().expect("word").json = Some(doc.to_json());
            }
            let proof = validate_counterexample(&model, &bad, &golden);
            assert!(!proof.validated, "{mutation}: {proof:?}");
            let mut diffs = differences(&golden, &bad);
            assert!(
                !apply_word_attribution(&mut diffs, &golden, &bad, &proof),
                "{mutation}"
            );
        }
        // Independent replay refusing VIEW supplies no permission either.
        let source = std::fs::read_to_string(
            root()
                .join("conformance/corpus/word-relative-liveness/specifications/Mutual/Mutual.tla"),
        )
        .expect("source")
        .replace("====", "View == x\n====");
        let refused = SpecModel::from_source(
            "Mutual.tla",
            &source,
            "INIT Init\nNEXT Next\nPROPERTY P Q\nVIEW View\n",
            &TraceOptions {
                compat_tlc: tlc,
                ..TraceOptions::default()
            },
        )
        .expect("VIEW model");
        let proof = validate_counterexample(&refused, &observed, &golden);
        assert!(!proof.validated && !proof.refuted, "{proof:?}");
        assert!(!apply_word_attribution(
            &mut differences(&golden, &observed),
            &golden,
            &observed,
            &proof
        ));
    }
}

#[test]
fn attribution_never_waives_other_fields_or_safety_names() {
    let (model, golden, observed) = case(true);
    let proof = validate_counterexample(&model, &observed, &golden);
    assert!(proof.validated);
    let mut bad = observed.clone();
    bad.counts.distinct_states = Some(999);
    bad.counts.states_generated = Some(998);
    bad.counts.depth = Some(997);
    bad.error_class = Some("GENERAL".into());
    bad.exit_code = Some(255);
    let mut diffs = differences(&golden, &bad);
    apply_word_attribution(&mut diffs, &golden, &bad, &proof);
    for field in [
        "counts.distinct_states",
        "counts.states_generated",
        "counts.depth",
        "error_class",
        "exit_code",
    ] {
        assert!(diffs.iter().any(|d| d.field == field), "{field}: {diffs:?}");
    }
    let mut safety = golden.clone();
    safety.verdict = Verdict::SafetyViolation;
    let mut other = observed;
    other.verdict = Verdict::SafetyViolation;
    let mut diffs = differences(&safety, &other);
    assert!(!apply_word_attribution(&mut diffs, &safety, &other, &proof));
    assert!(diffs.iter().any(|d| d.field == "violated_property"));
}
