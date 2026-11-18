//! A genuine independently replayed Next error grants no wrapper exemption.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test fixture construction"
)]

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file};
use dive_conformance::compare::{CompareContext, diff_results};
use dive_conformance::dive::{CheckMapping, check_result, check_result_with_model};
use dive_conformance::process::ProcessOutcome;
use dive_conformance::tlc::result::{OracleResult, RunFacts, Verdict};
use dive_conformance::tlc::{messages, result};
use dive_conformance::trace_check::validate_counterexample;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn native(file: &Path, compat: bool) -> OracleResult {
    let report = check_file(&CheckRequest {
        file: file.into(),
        config: Some(file.with_extension("cfg")),
        compat_tlc: compat,
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    })
    .expect("native check");
    let outcome = ProcessOutcome {
        exit_code: Some(i32::from(
            report.exit_code_for(dive_api::ExitConvention::Tlc),
        )),
        signal: None,
        timed_out: false,
        stdout: report.to_json().to_string(),
        stderr: String::new(),
        elapsed: Duration::ZERO,
    };
    let CheckMapping::Result(result) = check_result(&outcome, compat) else {
        panic!("native classification")
    };
    *result
}
fn model(file: &Path, compat: bool) -> SpecModel {
    SpecModel::load(
        file,
        &file.with_extension("cfg"),
        &TraceOptions {
            compat_tlc: compat,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )
    .expect("independent model")
}

#[test]
fn pinned_direct_cause_requires_the_real_independent_word() {
    let file=repo().join("conformance/corpus/interval-streaming/specifications/EnabledIntervalFirstError/EnabledIntervalFirstError.tla");
    let text=std::fs::read_to_string(repo().join("conformance/golden/tlc-2026.10.06.014338/interval-streaming/specifications/EnabledIntervalFirstError/EnabledIntervalFirstError.cfg.json")).expect("pinned golden");
    let golden: Value = serde_json::from_str(&text).expect("golden JSON");
    let golden: OracleResult = serde_json::from_value(golden["result"].clone()).expect("result");
    assert_eq!(golden.verdict_code, Some(2179));
    for compat in [false, true] {
        let spec = model(&file, compat);
        {
            let actual = native(&file, compat);
            assert_eq!(
                diff_results(
                    &golden,
                    &actual,
                    CompareContext {
                        workers: 1,
                        counts_comparable: true
                    }
                ),
                Vec::new()
            );
            let proof = validate_counterexample(&spec, &actual, &golden);
            assert!(proof.validated, "{proof:?}");
            let mut forged = actual.clone();
            let wire = forged
                .trace
                .as_mut()
                .and_then(|trace| trace.json.as_mut())
                .expect("native word");
            wire["ending"]["endpoint"]["source"]["module"] = json!("Forged");
            assert!(validate_counterexample(&spec, &forged, &golden).refuted);
            forged.trace.as_mut().expect("trace").json = None;
            assert!(!validate_counterexample(&spec, &forged, &golden).validated);
        }
    }
}

#[test]
fn next_word_validity_does_not_waive_an_unknown_or_mismatched_wrapper() {
    let root = repo().join("crates/dive-api/tests/fixtures/checker/errors");
    let file = root.join("specs/ErWrapFcnGuard/ErWrapFcnGuard.tla");
    let clean = std::fs::read_to_string(root.join("tlc-2026.10.06.014338/ErWrapFcnGuard.stdout"))
        .expect("pinned clean stream");
    let oracle = result::classify(
        &messages::parse(&clean),
        &RunFacts {
            exit_code: Some(75),
            ..RunFacts::default()
        },
    );
    assert_eq!(oracle.verdict, Verdict::Unclassified);
    for compat in [false, true] {
        let spec = model(&file, compat);
        {
            let actual = native(&file, compat);
            let doc = TraceDoc::from_json(
                actual
                    .trace
                    .as_ref()
                    .and_then(|trace| trace.json.as_ref())
                    .expect("word"),
            )
            .expect("decode");
            assert_eq!(
                spec.try_validate(&doc).expect("fresh replay").validation,
                Validation::Valid
            );
            assert!(validate_counterexample(&spec, &actual, &actual).validated);
            // Word/diagnostic proof does not turn the canonical unclassified
            // pinned stream into TLC agreement.
            assert_ne!(
                diff_results(
                    &oracle,
                    &actual,
                    CompareContext {
                        workers: 1,
                        counts_comparable: true
                    }
                ),
                Vec::new()
            );
            let mut legacy = actual.clone();
            let endpoint = &mut legacy
                .trace
                .as_mut()
                .and_then(|trace| trace.json.as_mut())
                .expect("word")["ending"]["endpoint"];
            endpoint["stage"] = json!("next-before-successor");
            let object = endpoint.as_object_mut().expect("endpoint");
            object.remove("reported");
            object.remove("deciding");
            let legacy_doc = TraceDoc::from_json(
                legacy
                    .trace
                    .as_ref()
                    .and_then(|trace| trace.json.as_ref())
                    .expect("legacy word"),
            )
            .expect("legacy decode");
            assert_eq!(
                spec.try_validate(&legacy_doc)
                    .expect("logical-only replay")
                    .validation,
                Validation::Valid
            );
            assert!(validate_counterexample(&spec, &legacy, &actual).refuted);
            let mut forged = actual.clone();
            forged
                .trace
                .as_mut()
                .and_then(|trace| trace.json.as_mut())
                .expect("word")["ending"]["ec"] = json!("TLC_NESTED_EXPRESSION");
            assert!(validate_counterexample(&spec, &forged, &forged).refuted);
        }
    }
}

#[test]
fn pinned_partial_candidate_requires_the_exact_independent_terminal() {
    let root = repo().join("crates/dive-api/tests/fixtures/checker/errors");
    let file = root.join("specs/ErNextIncomplete/ErNextIncomplete.tla");
    let clean = std::fs::read_to_string(root.join("tlc-2026.10.06.014338/ErNextIncomplete.stdout"))
        .expect("pinned clean BFS stream");
    let oracle = result::classify(
        &messages::parse(&clean),
        &RunFacts {
            exit_code: Some(75),
            ..RunFacts::default()
        },
    );
    assert_eq!(oracle.verdict_code, Some(2109));
    for compat in [false, true] {
        let spec = model(&file, compat);
        {
            let actual = native(&file, compat);
            assert_eq!(
                diff_results(
                    &oracle,
                    &actual,
                    CompareContext {
                        workers: 1,
                        counts_comparable: true
                    }
                ),
                Vec::new()
            );
            let proof = validate_counterexample(&spec, &actual, &oracle);
            assert!(proof.validated, "{proof:?}");
            for (pointer, value) in [
                ("/ending/endpoint/assignments/x", json!(4)),
                ("/ending/endpoint/source/module", json!("Forged")),
            ] {
                let mut forged = actual.clone();
                let wire = forged
                    .trace
                    .as_mut()
                    .and_then(|trace| trace.json.as_mut())
                    .expect("partial word");
                *wire.pointer_mut(pointer).expect("field") = value;
                assert!(validate_counterexample(&spec, &forged, &oracle).refuted);
            }
            let mut forged = actual.clone();
            forged.error_class = Some("TLC_NESTED_EXPRESSION".into());
            assert!(validate_counterexample(&spec, &forged, &oracle).refuted);
        }
    }
}

#[test]
fn candidate_wrapper_replay_preserves_pinned_primary_headers_and_strict_report_checks() {
    let root = repo().join("crates/dive-api/tests/fixtures/checker/errors");
    let cases = ["Constr", "ActConstr", "Prop"]
        .into_iter()
        .flat_map(|site| {
            ["Choose", "Domain", "NonBoolIf", "NonEnum", "TypeErr"].map(|kind| (site, kind))
        })
        .chain([("Prop", "NonBool")]);
    for (site, kind) in cases {
        let name = format!("Er{site}{kind}");
        let file = root.join(format!("specs/{name}/{name}.tla"));
        let clean =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("pinned stream");
        let transcript = messages::parse(&clean);
        assert_eq!(transcript.anomalies, Vec::<String>::new());
        let first = transcript
            .messages()
            .find(|m| m.class == messages::class::ERROR)
            .expect("first primary error");
        assert!(
            transcript
                .messages()
                .any(|m| m.code == 2103 && m.class == messages::class::ERROR)
        );
        let oracle = result::classify(
            &transcript,
            &RunFacts {
                exit_code: Some(75),
                ..RunFacts::default()
            },
        );
        for compat in [false, true] {
            let spec = model(&file, compat);
            {
                let actual = native(&file, compat);
                let wire = actual
                    .trace
                    .as_ref()
                    .and_then(|t| t.json.as_ref())
                    .expect("actual word");
                assert_eq!(wire["ending"]["endpoint"]["reported"], first.code);
                assert_eq!(actual.verdict_code, Some(2103));
                assert!(
                    validate_counterexample(&spec, &actual, &actual).validated,
                    "{name}"
                );
                if oracle.verdict == Verdict::Unclassified {
                    assert_ne!(
                        diff_results(
                            &oracle,
                            &actual,
                            CompareContext {
                                workers: 1,
                                counts_comparable: true
                            }
                        ),
                        Vec::new()
                    );
                }
                let mut bad = actual.clone();
                bad.error_class = Some("GENERAL".into());
                assert!(validate_counterexample(&spec, &bad, &actual).refuted);
                let mut bad = actual.clone();
                bad.trace
                    .as_mut()
                    .expect("trace")
                    .json
                    .as_mut()
                    .expect("word")["ending"]["endpoint"]["deciding"] = json!(2179);
                assert!(validate_counterexample(&spec, &bad, &actual).refuted);
                let mut bad = actual.clone();
                bad.trace
                    .as_mut()
                    .expect("trace")
                    .json
                    .as_mut()
                    .expect("word")["ending"]["endpoint"]["source"]["module"] = json!("Forged");
                assert!(validate_counterexample(&spec, &bad, &actual).refuted);
            }
        }
    }
}

#[test]
fn actual_next_receiver_wrapper_requires_the_original_source_word_and_diagnostic_proof() {
    for (name, raw, hash) in [
        (
            "EmptyLazyQuery",
            include_str!("fixtures/next-receiver-lifetime/EmptyLazyQuery.stdout"),
            "a37ec7307f5360e6c998a07e0bcdef26a242d3a9449c04f1d6de4c0b5a500144",
        ),
        (
            "DefinedFunction",
            include_str!("fixtures/next-receiver-lifetime/DefinedFunction.stdout"),
            "db7ee058068f983b84139c160a21a7c63945f3d99d1ecd382509f0368e3e03bd",
        ),
    ] {
        let file = repo().join(format!(
            "crates/dive-api/tests/fixtures/next-receiver-lifetime/{name}.tla"
        ));
        assert_eq!(dive_conformance::hash::sha256_bytes(raw.as_bytes()), hash);
        let oracle = result::classify(
            &messages::parse(raw),
            &RunFacts {
                exit_code: Some(75),
                ..RunFacts::default()
            },
        );
        assert_eq!(oracle.verdict_code, Some(2147));
        assert_eq!(oracle.exit_code, Some(75));
        for compat in [false, true] {
            let source = model(&file, compat);
            {
                for workers in [1, 4] {
                    let report = check_file(&CheckRequest {
                        file: file.clone(),
                        config: Some(file.with_extension("cfg")),
                        compat_tlc: compat,
                        workers: Some(workers),
                        seed: Some(0),
                        fp: Some(0),
                        ..CheckRequest::default()
                    })
                    .expect("actual producer");
                    let value = report.to_json();
                    let outcome = |value: &Value, exit| ProcessOutcome {
                        exit_code: Some(exit),
                        signal: None,
                        timed_out: false,
                        stdout: value.to_string(),
                        stderr: String::new(),
                        elapsed: Duration::ZERO,
                    };
                    let actual_outcome = outcome(
                        &value,
                        i32::from(report.exit_code_for(dive_api::ExitConvention::Tlc)),
                    );
                    assert!(matches!(
                        check_result(&actual_outcome, compat),
                        CheckMapping::Refused(_)
                    ));
                    let CheckMapping::Result(actual) =
                        check_result_with_model(&actual_outcome, compat, &source)
                    else {
                        panic!("source-bound report must prove actual receiver: {value}")
                    };
                    assert_eq!(
                        diff_results(
                            &oracle,
                            &actual,
                            CompareContext {
                                workers: u32::try_from(workers).expect("small worker count"),
                                counts_comparable: true
                            }
                        ),
                        Vec::new(),
                        "{value}"
                    );
                    assert!(validate_counterexample(&source, &actual, &oracle).validated);
                    reject_next_receiver_report_mutations(&value, compat, &source);
                }
            }
        }
    }
}

fn reject_next_receiver_report_mutations(value: &Value, compat: bool, source: &SpecModel) {
    let outcome = |value: &Value, exit| ProcessOutcome {
        exit_code: Some(exit),
        signal: None,
        timed_out: false,
        stdout: value.to_string(),
        stderr: String::new(),
        elapsed: Duration::ZERO,
    };
    for (pointer, replacement) in [
        ("/counterexample", Value::Null),
        (
            "/counterexample/trace/ending/endpoint/deciding",
            json!(2179),
        ),
        (
            "/counterexample/trace/ending/endpoint/reported",
            json!(1000),
        ),
        (
            "/counterexample/trace/ending/endpoint/source/module",
            json!("Forged"),
        ),
        (
            "/counterexample/trace/ending/ec",
            json!("TLC_MODULE_OVERFLOW"),
        ),
        ("/tlc/reported", json!(1000)),
        ("/error_site", json!("initial-states")),
    ] {
        let mut bad = value.clone();
        *bad.pointer_mut(pointer).expect("mutation field") = replacement;
        assert!(
            matches!(
                check_result_with_model(&outcome(&bad, 75), compat, source),
                CheckMapping::Refused(_)
            ),
            "{bad}"
        );
    }
    let mut auxiliary = value.clone();
    auxiliary["diagnostic_witness"] = json!({"trace":value["counterexample"]["trace"]});
    assert!(matches!(
        check_result_with_model(&outcome(&auxiliary, 75), compat, source),
        CheckMapping::Refused(_)
    ));
    assert!(matches!(
        check_result_with_model(&outcome(value, 255), compat, source),
        CheckMapping::Refused(_)
    ));
}
