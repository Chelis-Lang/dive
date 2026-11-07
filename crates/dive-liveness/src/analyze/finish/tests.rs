//! Completion facts and result assembly do not require retained graph rows.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "producer/result controls"
)]

use std::sync::Arc;

use super::*;
use crate::analyze::{self, Limits};
use dive_model::Model;
use dive_runtime::{CheckOptions, Stats, Verdict};

fn fixture(options: &CheckOptions) -> Model {
    let source = "---- MODULE Evidence ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<3 THEN x+1 ELSE x\nP == <>(x=4)\n====\n";
    let analysis =
        dive_sema::analyze_source("Evidence.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}

fn capture(model: &Model, options: &CheckOptions) -> capture::Complete {
    match dive_check::capture::explore(model, options, capture::Limits::default()) {
        capture::Exploration::Complete(complete) => *complete,
        capture::Exploration::Stopped(result) => panic!("finite capture: {result:?}"),
    }
}

struct Stop;
impl capture::Observer for Stop {
    fn level_done(&mut self, _: u64, _: &capture::BehaviorGraph, _: Stats) -> Result<(), String> {
        Ok(())
    }
    fn stop_after_level(&self) -> bool {
        true
    }
}

fn captured_prefix(model: &Model, options: &CheckOptions) -> capture::Prefix {
    match dive_check::capture::explore_controlled_with_evidence(
        model,
        options,
        capture::Limits::default(),
        &mut Stop,
    )
    .expect("capture thread")
    {
        capture::ControlledExploration::Prefix(prefix) => *prefix,
        other => panic!("real prefix: {other:?}"),
    }
}

fn word(state: dive_eval::next::StateVals) -> CandidateWord {
    CandidateWord {
        target_property: "P".into(),
        properties: vec!["P".into()],
        states: vec![state],
        back_to: 0,
        edges: vec![capture::Edge {
            to: 0,
            ordinal: None,
            label: None,
        }],
    }
}

#[test]
fn neutral_final_and_prefix_results_release_graph_rows_and_keep_actual_facts() {
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        let model = fixture(&options);
        let complete = capture(&model, &options);
        let expected = complete.stats;
        let receipt = complete.completion().clone();
        let weak = Arc::downgrade(&complete.graph.states[0]);
        let result = complete.into_evidence();
        assert!(
            weak.upgrade().is_none(),
            "detached producer facts retain no state rows"
        );
        let Completed::Holds(evidence) = super::complete(Ok(None), result).expect("final assembly")
        else {
            panic!("holding decision")
        };
        assert_eq!(evidence.stats(), expected);
        assert_eq!(evidence.completion(), &receipt);
        assert_eq!(evidence.options().seed, 7);
        assert_eq!(evidence.safety_result().verdict, Verdict::Success);

        let prefix = captured_prefix(&model, &options);
        let expected = prefix.stats;
        let level = prefix.level();
        let weak = Arc::downgrade(&prefix.graph.states[1]);
        let candidate = word(prefix.graph.states[0].clone());
        let Prefix {
            word: actual,
            exploration,
        } = super::prefix(Ok(candidate), prefix.into_evidence()).expect("prefix assembly");
        assert!(
            weak.upgrade().is_none(),
            "only selected word values survive"
        );
        assert_eq!(exploration.stats(), expected);
        assert!(exploration.stats().left_on_queue > 0);
        assert_eq!(exploration.level(), level);
        assert_eq!(actual.target_property, "P");
    }
}

#[test]
fn legacy_adapters_retain_original_graph_allocations_and_selected_word() {
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        let model = fixture(&options);
        for candidate in [false, true] {
            let captured = capture(&model, &options);
            let pointers = (
                captured.graph.states.as_ptr(),
                captured.graph.edges.as_ptr(),
                captured.graph.concrete.as_ptr(),
                captured.graph.roots.as_ptr(),
            );
            let selected = candidate.then(|| word(captured.graph.states[0].clone()));
            let result = super::complete(Ok(selected), captured.evidence().clone())
                .expect("result")
                .reference(captured);
            let retained = match &result {
                Analysis::Holds(capture) => {
                    assert!(!candidate);
                    capture
                }
                Analysis::Candidate(word) => {
                    assert!(candidate);
                    assert_eq!(word.target_property, "P");
                    assert_eq!(word.properties, ["P"]);
                    assert_eq!(word.back_to, 0);
                    assert_eq!(word.edges[0].ordinal, None);
                    &word.exploration
                }
            };
            assert_eq!(
                pointers,
                (
                    retained.graph.states.as_ptr(),
                    retained.graph.edges.as_ptr(),
                    retained.graph.concrete.as_ptr(),
                    retained.graph.roots.as_ptr()
                )
            );
            assert_eq!(retained.stats, retained.evidence().stats());
        }
        let captured = captured_prefix(&model, &options);
        let pointers = (
            captured.graph.states.as_ptr(),
            captured.graph.edges.as_ptr(),
            captured.graph.concrete.as_ptr(),
        );
        let candidate = word(captured.graph.states[0].clone());
        let result = super::prefix(Ok(candidate), captured.evidence().clone())
            .expect("prefix")
            .reference(captured);
        assert_eq!(
            pointers,
            (
                result.exploration.graph.states.as_ptr(),
                result.exploration.graph.edges.as_ptr(),
                result.exploration.graph.concrete.as_ptr()
            )
        );
    }
}

#[test]
fn analysis_failures_and_typed_stops_never_become_absence_of_acceptance() {
    let options = CheckOptions::tlc(0, 7);
    let model = fixture(&options);
    let complete = capture(&model, &options).into_evidence();
    let prefix = captured_prefix(&model, &options).into_evidence();
    let stopped = dive_runtime::budget::Stopped {
        resource: dive_runtime::budget::Resource::Time,
        limit: 10,
        observed: 11,
        elapsed_ms: 11,
        phase: dive_runtime::budget::Phase::Temporal,
    };
    let mut safety = complete.safety_result().clone();
    safety.verdict = Verdict::Stopped(Box::new(stopped.clone()));
    let errors = [
        AnalysisError::Limit("test source allocation"),
        AnalysisError::Inconsistent("test shape"),
        AnalysisError::Stopped(Box::new(stopped)),
        AnalysisError::Exploration(Box::new(safety)),
    ];
    for error in errors {
        let expected = format!("{error:?}");
        let actual =
            super::complete(Err(error.clone()), complete.clone()).expect_err("never Holds");
        assert_eq!(format!("{actual:?}"), expected);
        let actual = super::prefix(Err(error), prefix.clone()).expect_err("never prefix candidate");
        assert_eq!(format!("{actual:?}"), expected);
    }
    // Exercise the production final-only and periodic adapters with the same
    // real model, rather than only constructing neutral carriers in tests.
    let final_result = analyze::analyze_final(&model, &options, Limits::default())
        .expect("reference final analysis");
    assert!(matches!(final_result, Analysis::Candidate(_)));
    let periodic = analyze::periodic::analyze_periodic(
        &model,
        &options,
        Limits::default(),
        crate::product::AcceptancePolicy::Legacy,
        analyze::periodic::Schedule {
            min_states: 2,
            growth_factor: 1,
        },
    )
    .expect("reference periodic");
    assert!(matches!(
        periodic.0,
        analyze::periodic::PeriodicAnalysis::Prefix(_)
    ));
}
