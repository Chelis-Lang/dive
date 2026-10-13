//! Mode-aware controls on genuine pinned TLC simulation and BFS witnesses.
#![allow(clippy::expect_used, reason = "test fixtures and assertions")]

use std::path::PathBuf;

use dive_api::trace::{
    ControlMode, Controls, Ending, SpecModel, TraceDoc, TraceOptions, Validation,
};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mode-controls")
}

fn doc(mode: &str) -> TraceDoc {
    TraceDoc::parse(&std::fs::read_to_string(fixture().join(format!("{mode}.json"))).expect("read"))
        .expect("TLC document")
        .with_ending(Ending::Invariant("Safe".to_owned()))
}

fn assert_controls(model: &SpecModel, doc: &TraceDoc, mode: ControlMode, expected: &[&str]) {
    let (positive, trace) = model.read(doc);
    assert_eq!(positive.validation, Validation::Valid);
    let trace = trace.expect("decoded trace");
    assert_eq!(model.validate_trace(&trace).validation, Validation::Valid);
    assert_eq!(
        model
            .validate(&model.to_doc(&trace).expect("document"))
            .validation,
        Validation::Valid
    );
    let mutants = model.mutants_with_mode(&trace, mode);
    assert_eq!(
        mutants.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        expected
    );
    for mutant in mutants {
        assert!(
            matches!(
                model.validate_trace(&mutant.trace).validation,
                Validation::Invalid(_)
            ),
            "direct: {}",
            mutant.name
        );
        assert!(
            matches!(
                model
                    .validate(&model.to_doc(&mutant.trace).expect("mutant document"))
                    .validation,
                Validation::Invalid(_)
            ),
            "document: {}",
            mutant.name
        );
    }
    let controls = model.controls_with_mode(doc, mode);
    assert_eq!(controls.dive_format, Some(Validation::Valid));
    assert_eq!(controls.mutants.len(), expected.len());
    assert!(controls.failures().is_empty(), "{:?}", controls.failures());
}

#[test]
fn simulation_uses_non_distance_controls_and_bfs_preserves_every_family() {
    for compat_tlc in [false, true] {
        let model = SpecModel::load(
            &fixture().join("ModeMutation.tla"),
            &fixture().join("ModeMutation.cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("model");
        let simulation = doc("simulation");
        assert_eq!(simulation.states.len(), 5);
        assert_controls(
            &model,
            &simulation,
            ControlMode::Counterexample,
            &["change-2-x", "truncate", "claim-Safe_dive_trace_mutant"],
        );

        // The compatibility API keeps its explicit shortest-BFS premise.
        // Violating that premise exposes a valid path mutation, not a validator bug.
        let (_, trace) = model.read(&simulation);
        let trace = trace.expect("trace");
        let legacy = model.mutants(&trace);
        let explicit = model.mutants_with_mode(&trace, ControlMode::ShortestBfs);
        assert_eq!(
            legacy.iter().map(|m| &m.name).collect::<Vec<_>>(),
            explicit.iter().map(|m| &m.name).collect::<Vec<_>>()
        );
        let drop = legacy
            .iter()
            .find(|m| m.name == "drop-state-2")
            .expect("drop control");
        assert_eq!(
            model.validate_trace(&drop.trace).validation,
            Validation::Valid
        );
        assert_eq!(
            model
                .validate(&model.to_doc(&drop.trace).expect("drop document"))
                .validation,
            Validation::Valid
        );
        assert_eq!(
            model.controls(&simulation).failures(),
            vec!["mutant drop-state-2 is not rejected: valid"]
        );

        let bfs = doc("bfs");
        assert_eq!(bfs.states.len(), 3);
        assert_controls(
            &model,
            &bfs,
            ControlMode::ShortestBfs,
            &[
                "drop-state-1",
                "drop-first",
                "swap-1",
                "change-1-x",
                "truncate",
                "claim-Safe_dive_trace_mutant",
            ],
        );
        assert_eq!(model.controls(&bfs).failures(), Vec::<String>::new());
        // Selection comes from the producer contract, even when its word is shortest.
        assert_controls(
            &model,
            &bfs,
            ControlMode::Counterexample,
            &["change-1-x", "truncate", "claim-Safe_dive_trace_mutant"],
        );
    }
}

#[test]
fn empty_negative_suite_receives_no_coverage_credit() {
    let controls = Controls {
        positive: dive_api::trace::Report {
            validation: Validation::Valid,
            notes: Vec::new(),
            length: 1,
        },
        dive_format: Some(Validation::Valid),
        mutants: Vec::new(),
    };
    assert_eq!(controls.failures(), vec!["no negative controls generated"]);
}

#[test]
fn non_shortest_controls_keep_false_action_labels() {
    let source = "---- MODULE LabelControls ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nUp == x'=x+1\nDown == x>0 /\\ x'=x-1\nNext == Up \\/ Down\nInv == x<2\n====\n";
    let doc = TraceDoc::parse(
        r#"{"format":"dive-trace","version":1,"variables":["x"],
        "states":[{"action":null,"values":{"x":0}},
                  {"action":{"name":"Up"},"values":{"x":1}},
                  {"action":{"name":"Down"},"values":{"x":0}},
                  {"action":{"name":"Up"},"values":{"x":1}},
                  {"action":{"name":"Up"},"values":{"x":2}}],
        "ending":{"kind":"invariant","name":"Inv"}}"#,
    )
    .expect("document");
    for compat_tlc in [false, true] {
        let model = SpecModel::from_source(
            "LabelControls.tla",
            source,
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            &TraceOptions {
                compat_tlc,
                ..TraceOptions::default()
            },
        )
        .expect("model");
        assert_controls(
            &model,
            &doc,
            ControlMode::Counterexample,
            &[
                "change-2-x",
                "truncate",
                "claim-Inv_dive_trace_mutant",
                "relabel-1-Down",
            ],
        );
    }
}
