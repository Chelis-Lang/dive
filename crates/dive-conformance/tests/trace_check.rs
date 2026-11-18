//! The harness computes `trace_validated` itself (ADR 0016 §3): dive's
//! counterexample is read from the result and validated in-process, and a
//! deadlock is accepted only when the oracle's own counterexample ends in
//! the same state.

#![allow(clippy::expect_used, clippy::panic, reason = "test code")]

use std::path::PathBuf;

use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions};
use dive_conformance::tlc::result::OracleResult;
use dive_conformance::trace_check::{tlc_document, validate_counterexample};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tlc() -> TraceOptions {
    TraceOptions {
        compat_tlc: true,
        ..TraceOptions::default()
    }
}

fn golden(path: &str) -> OracleResult {
    let text = std::fs::read_to_string(repo().join(path)).expect("golden");
    let v: serde_json::Value = serde_json::from_str(&text).expect("json");
    serde_json::from_value(v["result"].clone()).expect("result")
}

/// `result` with its counterexample replaced by `doc` (as dive would emit it).
fn with_doc(result: &OracleResult, doc: &TraceDoc) -> OracleResult {
    let mut r = result.clone();
    if let Some(t) = r.trace.as_mut() {
        t.json = Some(doc.to_json());
    }
    r
}

#[test]
fn an_invariant_counterexample_is_validated_by_the_harness() {
    let g = golden(
        "conformance/golden/tlc-2026.10.06.014338/examples/specifications/DieHard/DieHard.cfg.json",
    );
    let dir = repo().join("crates/dive-eval/tests/fixtures/examples/specifications/DieHard");
    let spec =
        SpecModel::load(&dir.join("DieHard.tla"), &dir.join("DieHard.cfg"), &tlc()).expect("model");
    let (_, trace) = spec.read(&tlc_document(&g).expect("TLC document"));
    let trace = trace.expect("trace");
    let doc = spec.to_doc(&trace).expect("doc");

    let ok = validate_counterexample(&spec, &with_doc(&g, &doc), &g);
    assert!(ok.validated, "{}", ok.detail);

    // A tampered state.
    let mut bad = doc.clone();
    bad.states[3].insert("big".to_owned(), serde_json::json!(4));
    let no = validate_counterexample(&spec, &with_doc(&g, &bad), &g);
    assert!(!no.validated);
    assert!(no.detail.contains("invalid"), "{}", no.detail);

    // A document that claims something else than the result reports.
    let other = doc.clone().with_ending(Ending::Invariant("TypeOK".into()));
    assert!(!validate_counterexample(&spec, &with_doc(&g, &other), &g).validated);

    // TLC's own document is not dive's: it carries no claim.
    assert!(!validate_counterexample(&spec, &g, &g).validated);
}

#[test]
fn finite_deadlocks_validate_independently_and_false_claims_are_rejected() {
    let probes = repo().join("crates/dive-api/tests/fixtures/checker/tlc-probes");
    let dir = probes.join("specs/DeadlockMidLevel");
    let spec = SpecModel::load(
        &dir.join("DeadlockMidLevel.tla"),
        &dir.join("DeadlockMidLevel.cfg"),
        &tlc(),
    )
    .expect("model");
    let tlc_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(probes.join("tlc-2026.10.06.014338/DeadlockMidLevel.trace.json"))
            .expect("trace"),
    )
    .expect("json");
    // The probe's result, in the golden vocabulary.
    let mut g = golden(
        "conformance/golden/tlc-2026.10.06.014338/examples/specifications/DieHard/DieHard.cfg.json",
    );
    g.error_class = Some("TLC_DEADLOCK_REACHED".to_owned());
    g.violated_property = None;
    let t = g.trace.as_mut().expect("trace");
    t.length = 2;
    t.json = Some(tlc_json);

    let (_, trace) = spec.read(&tlc_document(&g).expect("TLC document"));
    let doc = spec.to_doc(&trace.expect("trace")).expect("doc");
    let ok = validate_counterexample(&spec, &with_doc(&g, &doc), &g);
    assert!(ok.validated, "{}", ok.detail);

    // `0 -> 1` is a behavior, but 1 has a successor. The independent finite
    // cover now refutes the claim without the oracle's ending state.
    let mut wrong = doc.clone();
    wrong.states[1].insert("x".to_owned(), serde_json::json!(1));
    let no = validate_counterexample(&spec, &with_doc(&g, &wrong), &g);
    assert!(!no.validated);
    assert!(no.detail.contains("has a successor"), "{}", no.detail);

    // A finite proof needs no oracle document.
    let mut blind = g.clone();
    if let Some(t) = blind.trace.as_mut() {
        t.json = None;
        t.json_unavailable = Some("issue #35".to_owned());
    }
    let independently_valid = validate_counterexample(&spec, &with_doc(&g, &doc), &blind);
    assert!(
        independently_valid.validated,
        "{}",
        independently_valid.detail
    );
}

#[test]
fn golden_controls_route_the_actual_mode_and_worker_contract() {
    use dive_api::trace::Validation;
    use dive_conformance::corpus::{ManifestExpectation, ModelEntry, ModelMode, Tier};
    use dive_conformance::tlc::{messages, result};
    use dive_conformance::trace_check::{
        ControlOutcome, ControlRow, ControlsSummary, golden_controls,
    };

    let root = repo().join("crates/dive-api/tests/fixtures/mode-controls");
    let mut model = ModelEntry {
        id: "ModeMutation.cfg".to_owned(),
        module: "ModeMutation.tla".to_owned(),
        mode: ModelMode::Simulate { traces: 100 },
        features: Vec::new(),
        tier: Tier::Deep,
        out_of_scope: None,
        counts_comparable: false,
        expected: ManifestExpectation {
            result: "safety-violation".to_owned(),
            runtime_seconds: None,
            distinct_states: None,
            total_states: None,
            state_depth: None,
            workers: None,
        },
        worker_introspecting: false,
    };
    let mut golden = result::classify(
        &messages::parse(
            "@!@!@STARTMSG 2110:1 @!@!@\nInvariant Safe is violated.\n@!@!@ENDMSG 2110 @!@!@\n",
        ),
        &result::RunFacts {
            exit_code: Some(12),
            ..Default::default()
        },
    );
    for (file, length) in [("simulation.json", 5), ("bfs.json", 3)] {
        golden.trace = Some(result::Trace {
            length,
            ending: result::TraceEnding::Finite,
            json: Some(
                serde_json::from_str(&std::fs::read_to_string(root.join(file)).expect("read"))
                    .expect("json"),
            ),
            steps: None,
            omitted: None,
            json_unavailable: None,
        });
        for (mode, workers, expected_mutants, findings) in [
            (ModelMode::Simulate { traces: 100 }, 1, 3, 0),
            (ModelMode::Exhaustive, 4, 3, 0),
            (ModelMode::Exhaustive, 1, 6, usize::from(length == 5)),
        ] {
            model.mode = mode;
            let outcome = golden_controls(&root, &model, &[], &golden, workers);
            let ControlOutcome::Ran(controls) = &outcome else {
                panic!("{outcome:?}")
            };
            assert_eq!(controls.positive.validation, Validation::Valid);
            assert_eq!(controls.dive_format, Some(Validation::Valid));
            assert_eq!(controls.mutants.len(), expected_mutants);
            assert_eq!(controls.failures().len(), findings);
            if findings == 0 {
                assert!(
                    controls
                        .mutants
                        .iter()
                        .all(|(_, v)| matches!(v, Validation::Invalid(_)))
                );
            }
            let summary = ControlsSummary {
                rows: vec![ControlRow {
                    id: model.id.clone(),
                    outcome,
                }],
            };
            assert_eq!(summary.findings().len(), findings);
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "three oracle-backed producer/normalizer/validator pipelines and forged claims"
)]
fn named_invariant_error_claims_require_independent_cause_and_order_proof() {
    use dive_api::{CheckRequest, check_file};
    use dive_conformance::dive::{CheckMapping, check_result_with_model};
    use dive_conformance::process::ProcessOutcome;
    use dive_conformance::tlc::{messages, result};
    use std::time::Duration;

    let fixtures = repo().join("crates/dive-api/tests/fixtures/core-lib");
    for name in [
        "ErAssertInv",
        "ErBagCardNonBag",
        "ErBagCupNonBag",
        "ErBagSqNonFcn",
    ] {
        let file = fixtures
            .join("specs")
            .join(name)
            .join(format!("{name}.tla"));
        let stdout = std::fs::read_to_string(
            fixtures
                .join("tlc-2026.10.06.014338")
                .join(format!("{name}.stdout")),
        )
        .expect("pinned TLC transcript");
        let golden = result::classify(
            &messages::parse(&stdout),
            &result::RunFacts {
                exit_code: Some(76),
                ..Default::default()
            },
        );
        assert_eq!(
            golden.error_class.as_deref(),
            Some("TLC_INVARIANT_EVALUATION_FAILED")
        );
        for compat_tlc in [false, true] {
            let native = check_file(&CheckRequest {
                file: file.clone(),
                config: Some(file.with_extension("cfg")),
                compat_tlc,
                seed: Some(0),
                fp: Some(0),
                workers: Some(1),
                ..CheckRequest::default()
            })
            .expect("native check");
            let mut report = native.to_json_with_exit_convention(dive_api::ExitConvention::Tlc);
            let exit_code = native.exit_code_for(dive_api::ExitConvention::Tlc);
            report["exit_code"] = serde_json::json!(exit_code);
            let outcome = ProcessOutcome {
                exit_code: Some(i32::from(exit_code)),
                signal: None,
                timed_out: false,
                stdout: report.to_string(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
            };
            let spec = SpecModel::load(
                &file,
                &file.with_extension("cfg"),
                &TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )
            .expect("model");
            let CheckMapping::Result(result) = check_result_with_model(&outcome, compat_tlc, &spec)
            else {
                panic!("native report not classified: {report}")
            };
            let valid = validate_counterexample(&spec, &result, &golden);
            assert!(valid.validated, "{name}: {}", valid.detail);
            let doc = TraceDoc::from_json(&report["counterexample"]["trace"]).expect("trace");
            let Some(Ending::InvariantError {
                name: invariant,
                ec,
            }) = &doc.ending
            else {
                panic!("named error")
            };
            for ending in [
                Ending::InvariantError {
                    name: "absent".into(),
                    ec: ec.clone(),
                },
                Ending::InvariantError {
                    name: invariant.clone(),
                    ec: "TLC_INVARIANT_EVALUATION_FAILED".into(),
                },
                Ending::InvariantError {
                    name: invariant.clone(),
                    ec: "TLC_MODULE_DIVISION_BY_ZERO".into(),
                },
                Ending::Error {
                    ec: Some(ec.clone()),
                },
                Ending::Error {
                    ec: Some("TLC_INVARIANT_EVALUATION_FAILED".into()),
                },
            ] {
                let forged = doc.clone().with_ending(ending);
                assert!(
                    !validate_counterexample(&spec, &with_doc(&result, &forged), &golden).validated
                );
            }
            let mut wrong_report = (*result).clone();
            wrong_report.error_class = Some("TLC_NESTED_EXPRESSION".into());
            assert!(!validate_counterexample(&spec, &wrong_report, &golden).validated);
        }
    }
}
