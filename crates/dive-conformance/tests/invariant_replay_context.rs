//! A named invariant word does not certify a separate diagnostic replay.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test fixture construction"
)]

use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, check_file_engine};
use dive_conformance::compare::{CompareContext, diff_results, diff_verdict_and_class};
use dive_conformance::dive::{CheckMapping, check_result, check_result_with_model};
use dive_conformance::process::ProcessOutcome;
use dive_conformance::tlc::result::{OracleResult, RunFacts, Verdict};
use dive_conformance::tlc::{messages, result};
use dive_conformance::trace_check::validate_counterexample;
use dive_conformance::util::TempDir;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-check/tests/fixtures/errors")
}

fn native(file: &Path, engine: Engine, compat: bool) -> (Value, i32) {
    let report = check_file_engine(
        &CheckRequest {
            file: file.into(),
            config: Some(file.with_extension("cfg")),
            compat_tlc: compat,
            seed: Some(0),
            fp: Some(0),
            workers: Some(1),
            ..CheckRequest::default()
        },
        engine,
    )
    .expect("native check");
    (report.to_json(), i32::from(report.exit_code()))
}

fn mapping(report: &Value, exit: i32, compat: bool) -> CheckMapping {
    check_result(
        &ProcessOutcome {
            exit_code: Some(exit),
            signal: None,
            timed_out: false,
            stdout: report.to_string(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        },
        compat,
    )
}

fn mapped(report: &Value, exit: i32, compat: bool) -> OracleResult {
    let CheckMapping::Result(result) = mapping(report, exit, compat) else {
        panic!("native report not classified: {report}")
    };
    *result
}

fn source_mapping(report: &Value, exit: i32, compat: bool, spec: &SpecModel) -> CheckMapping {
    check_result_with_model(
        &ProcessOutcome {
            exit_code: Some(exit),
            signal: None,
            timed_out: false,
            stdout: report.to_string(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        },
        compat,
        spec,
    )
}
fn mapped_source(report: &Value, exit: i32, compat: bool, spec: &SpecModel) -> OracleResult {
    let CheckMapping::Result(result) = source_mapping(report, exit, compat, spec) else {
        panic!("diagnostic proof failed: {report}");
    };
    *result
}

#[test]
fn independent_named_cause_does_not_certify_the_deciding_wrapper() {
    let root = fixtures();
    let file = root.join("specs/ErInvReplayError/ErInvReplayError.tla");
    let clean = std::fs::read_to_string(root.join("tlc-2026.10.06.014338/ErInvReplayError.stdout"))
        .expect("pinned clean TLC stream");
    let oracle = result::classify(
        &messages::parse(&clean),
        &RunFacts {
            exit_code: Some(75),
            ..RunFacts::default()
        },
    );
    assert_eq!(oracle.verdict, Verdict::Unclassified);
    assert_eq!(oracle.error_codes, [2111, 2121, 2103]);
    for compat in [false, true] {
        let spec = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc: compat,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("model");
        for engine in [Engine::Reference, Engine::Core] {
            let (report, exit) = native(&file, engine, compat);
            let actual = mapped_source(&report, exit, compat, &spec);
            assert!(matches!(
                mapping(&report, exit, compat),
                CheckMapping::Refused(_)
            ));
            let doc = TraceDoc::from_json(&report["counterexample"]["trace"]).expect("word");
            assert_eq!(spec.validate(&doc).validation, Validation::Valid);
            assert!(validate_counterexample(&spec, &actual, &oracle).validated);
            // A valid word cannot turn the canonical unknown stream into
            // a pinned-TLC agreement or certify the diagnostic replay.
            assert_ne!(diff_verdict_and_class(&oracle, &actual), Vec::new());
            for (path, value) in [
                ("/tlc/reported", json!(1000)),
                ("/tlc/reported", json!(2113)),
                ("/tlc/reported", json!(null)),
                ("/error_site", json!("next-state")),
                ("/error_site", json!("constraint")),
            ] {
                let mut forged = report.clone();
                *forged.pointer_mut(path).expect("field") = value;
                assert!(matches!(
                    source_mapping(&forged, exit, compat, &spec),
                    CheckMapping::Refused(_)
                ));
            }
            let mut unverified = report.clone();
            unverified["tlc"]["verified"] = json!(false);
            assert!(matches!(
                mapping(&unverified, exit, compat),
                CheckMapping::Refused(_)
            ));

            for ending in [
                Ending::InvariantError {
                    name: "InvB".into(),
                    ec: "TLC_MODULE_DIVISION_BY_ZERO".into(),
                },
                Ending::InvariantError {
                    name: "InvA".into(),
                    ec: "GENERAL".into(),
                },
                Ending::InvariantError {
                    name: "InvA".into(),
                    ec: "TLC_NESTED_EXPRESSION".into(),
                },
                Ending::Error {
                    ec: Some("TLC_MODULE_DIVISION_BY_ZERO".into()),
                },
            ] {
                let mut forged = actual.clone();
                let doc = TraceDoc::from_json(&report["counterexample"]["trace"])
                    .expect("trace")
                    .with_ending(ending);
                forged.trace.as_mut().expect("word").json = Some(doc.to_json());
                assert!(!validate_counterexample(&spec, &forged, &oracle).validated);
            }
            let mut forged_replay = actual.clone();
            forged_replay.counts.distinct_states = Some(6);
            assert_ne!(
                diff_results(
                    &actual,
                    &forged_replay,
                    CompareContext {
                        workers: 1,
                        counts_comparable: true
                    }
                ),
                Vec::new()
            );
        }
    }
}

#[test]
fn secondary_replay_context_is_not_proven_by_the_invariant_word() {
    let file = fixtures().join("specs/ErInvReplayError/ErInvReplayError.tla");
    let source = std::fs::read_to_string(&file).expect("source");
    let config = std::fs::read_to_string(file.with_extension("cfg")).expect("config");
    let temp = TempDir::new("invariant-replay-context").expect("temporary source");
    let changed_file = temp.path().join("ErInvReplayError.tla");
    let changed = source.replace(
        "InvB == y = 0 \\/ x # 1 \\/ (CHOOSE i \\in {1} : i > 1) = 1",
        "InvB == TRUE",
    );
    assert_ne!(source, changed);
    std::fs::write(&changed_file, changed).expect("changed secondary context");
    std::fs::write(changed_file.with_extension("cfg"), config).expect("config");
    for compat in [false, true] {
        let (original_report, exit) = native(&file, Engine::Core, compat);
        let original_spec = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc: compat,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("original model");
        let original = mapped_source(&original_report, exit, compat, &original_spec);
        let (mut changed_report, exit) = native(&changed_file, Engine::Reference, compat);
        let changed_result = mapped(&changed_report, exit, compat);
        assert_eq!(changed_result.verdict_code, Some(2111));
        assert_eq!(
            original_report["counterexample"]["trace"],
            changed_report["counterexample"]["trace"]
        );
        let spec = SpecModel::load(
            &changed_file,
            &changed_file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc: compat,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("changed model");
        // InvA's exact original cause remains real after removing the
        // secondary error. It cannot certify forged diagnostic metadata,
        // even if both engines were to emit the same false context.
        let doc = TraceDoc::from_json(&original_report["counterexample"]["trace"])
            .expect("original invariant word");
        assert_eq!(spec.validate(&doc).validation, Validation::Valid);
        assert_ne!(
            diff_verdict_and_class(&changed_result, &original),
            Vec::new()
        );
        changed_report["error_site"] = json!("invariant");
        changed_report["tlc"]["reported"] = json!(2111);
        changed_report["tlc"]["code"] = json!(2103);
        changed_report["tlc"]["name"] = json!("TLC_NESTED_EXPRESSION");
        changed_report["tlc"]["exit_status"] = json!(75);
        changed_report["tlc"]["verified"] = json!(true);
        assert!(matches!(
            source_mapping(
                &changed_report,
                if compat { 75 } else { exit },
                compat,
                &spec
            ),
            CheckMapping::Refused(_)
        ));
    }
}

#[test]
fn qualified_next_word_preserves_legacy_refusal_and_unclassified_oracle() {
    let file = fixtures().join("specs/ErWrapFcnGuard/ErWrapFcnGuard.tla");
    let stream =
        std::fs::read_to_string(fixtures().join("tlc-2026.10.06.014338/ErWrapFcnGuard.stdout"))
            .expect("pinned TLC stream");
    let oracle = result::classify(
        &messages::parse(&stream),
        &RunFacts {
            exit_code: Some(75),
            ..RunFacts::default()
        },
    );
    assert_eq!(oracle.verdict, Verdict::Unclassified);
    assert_eq!(oracle.error_codes, [1000, 2121, 2103]);
    for compat in [false, true] {
        let spec = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc: compat,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("model");
        for engine in [Engine::Reference, Engine::Core] {
            let (report, exit) = native(&file, engine, compat);
            let actual = mapped(&report, exit, compat);
            let doc = TraceDoc::from_json(&report["counterexample"]["trace"]).expect("trace");
            // The borrowed legacy API has no private source replay authority.
            assert!(matches!(
                spec.validate(&doc).validation,
                Validation::Unsupported(_)
            ));
            let proof = validate_counterexample(&spec, &actual, &oracle);
            assert!(proof.validated && !proof.refuted, "{proof:?}");
            // A valid word does not classify the canonical unknown TLC stream.
            assert_ne!(diff_verdict_and_class(&oracle, &actual), Vec::new());

            let mut anonymous = actual.clone();
            anonymous
                .trace
                .as_mut()
                .expect("word")
                .json
                .as_mut()
                .expect("document")["ending"] = json!({"kind":"error","ec":"GENERAL"});
            assert!(validate_counterexample(&spec, &anonymous, &oracle).refuted);
            for (pointer, value) in [
                ("/ending/ec", json!("TLC_NESTED_EXPRESSION")),
                ("/ending/endpoint/reported", json!(2179)),
                ("/ending/endpoint/deciding", json!(2179)),
            ] {
                let mut forged = actual.clone();
                *forged
                    .trace
                    .as_mut()
                    .expect("word")
                    .json
                    .as_mut()
                    .expect("document")
                    .pointer_mut(pointer)
                    .expect("endpoint field") = value;
                assert!(validate_counterexample(&spec, &forged, &oracle).refuted);
            }
            let mut coordinated = actual.clone();
            coordinated.verdict_code = Some(2132);
            coordinated.error_class = Some("TLC_VALUE_ASSERT_FAILED".into());
            coordinated.exit_code = Some(255);
            coordinated
                .trace
                .as_mut()
                .expect("word")
                .json
                .as_mut()
                .expect("document")["ending"]["endpoint"]["deciding"] = json!(2132);
            assert!(validate_counterexample(&spec, &coordinated, &oracle).refuted);
        }
    }
}
