//! Owned native results release graph storage while keeping actual word/facts.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "actual result ownership controls"
)]

use std::sync::{Arc, Weak};

use dive_check::CheckOptions;
use dive_eval::next::StateVals;
use dive_liveness::analyze::{
    self, Analysis, Limits,
    periodic::{
        self, PeriodicAnalysis,
        external::{NativeAnalysis, NativeFinal, NativeWord},
    },
};
use dive_liveness::product::AcceptancePolicy;
use dive_model::Model;

fn model(property: &str, options: &CheckOptions) -> Model {
    let source = format!(
        "---- MODULE NativeResults ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<3 THEN x+1 ELSE x\nP == {property}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("NativeResults.tla", &source, &dive_sema::Options::default());
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

type WeakState = Weak<[Option<dive_value::Value>]>;

fn weak_rows(states: &[StateVals]) -> Vec<WeakState> {
    states.iter().map(Arc::downgrade).collect()
}

fn no_rows(rows: &[WeakState]) {
    assert!(
        rows.iter().all(|row| row.upgrade().is_none()),
        "graph state rows released"
    );
}

fn same_word(actual: &NativeWord, expected: &NativeWord) {
    assert_eq!(actual.target_property, expected.target_property);
    assert_eq!(actual.properties, expected.properties);
    assert_eq!(actual.back_to, expected.back_to);
    assert_eq!(actual.edges, expected.edges);
    assert_eq!(actual.states, expected.states);
    for (actual, expected) in actual.states.iter().zip(&expected.states) {
        assert!(
            Arc::ptr_eq(actual, expected),
            "word rows move without reconversion or cloning values"
        );
    }
}

#[test]
fn final_results_keep_the_same_selected_word_and_release_every_other_graph_row() {
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        for (property, holding) in [("<>[](x<4)", true), ("<>(x=4)", false)] {
            let model = model(property, &options);
            let result = analyze::analyze_final(&model, &options, Limits::default())
                .expect("actual final analysis");
            let capture = match &result {
                Analysis::Holds(captured) => captured,
                Analysis::Candidate(word) => &word.exploration,
            };
            let weak = weak_rows(&capture.graph.states);
            let facts = capture.evidence().clone();
            let expected = match &result {
                Analysis::Holds(_) => None,
                Analysis::Candidate(word) => Some(NativeWord {
                    target_property: word.target_property.clone(),
                    properties: word.properties.clone(),
                    states: word.states.clone(),
                    back_to: word.back_to,
                    edges: word.edges.clone(),
                }),
            };
            let retained = capture
                .graph
                .states
                .iter()
                .map(|row| {
                    expected.as_ref().is_some_and(|word| {
                        word.states
                            .iter()
                            .any(|selected| Arc::ptr_eq(row, selected))
                    })
                })
                .collect::<Vec<_>>();
            let result = result.into_native();
            match &result {
                NativeFinal::Holds(evidence) => {
                    assert!(holding);
                    assert_eq!(evidence.completion(), facts.completion());
                    assert_eq!(evidence.stats(), facts.stats());
                }
                NativeFinal::Candidate(word, evidence) => {
                    assert!(!holding);
                    same_word(word, expected.as_ref().expect("selected word"));
                    assert_eq!(evidence.completion(), facts.completion());
                    assert_eq!(evidence.stats(), facts.stats());
                }
            }
            for (weak, retained) in weak.iter().zip(retained) {
                assert_eq!(weak.upgrade().is_some(), retained);
            }
            drop(result);
            drop(expected);
            no_rows(&weak);
        }
    }
}

#[test]
fn periodic_final_and_real_prefix_results_preserve_distinct_producer_facts() {
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        let holding_model = model("<>[](x<4)", &options);
        let (final_result, checks) = periodic::analyze_periodic(
            &holding_model,
            &options,
            Limits::default(),
            AcceptancePolicy::Legacy,
            periodic::Schedule {
                min_states: u64::MAX,
                growth_factor: 2,
            },
        )
        .expect("actual final result");
        let PeriodicAnalysis::Final(Analysis::Holds(captured)) = &final_result else {
            panic!("actual completed holding model")
        };
        let weak = weak_rows(&captured.graph.states);
        let expected = captured.evidence().clone();
        let NativeAnalysis::Final(NativeFinal::Holds(actual)) = final_result.into_native() else {
            panic!("complete remains complete")
        };
        assert_eq!(actual.stats(), expected.stats());
        assert_eq!(actual.options().seed, expected.options().seed);
        assert!(checks.exhausted && checks.final_check);
        no_rows(&weak);

        let failing_model = model("<>(x=4)", &options);
        let (prefix, checks) = periodic::analyze_periodic(
            &failing_model,
            &options,
            Limits::default(),
            AcceptancePolicy::Legacy,
            periodic::Schedule {
                min_states: 2,
                growth_factor: 1,
            },
        )
        .expect("actual prefix result");
        let PeriodicAnalysis::Prefix(candidate) = &prefix else {
            panic!("actual incomplete prefix")
        };
        let weak = weak_rows(&candidate.exploration.graph.states);
        let facts = candidate.exploration.evidence().clone();
        let expected = NativeWord {
            target_property: candidate.target_property.clone(),
            properties: candidate.properties.clone(),
            states: candidate.states.clone(),
            back_to: candidate.back_to,
            edges: candidate.edges.clone(),
        };
        let NativeAnalysis::Prefix(actual) = prefix.into_native() else {
            panic!("prefix cannot become final Holds")
        };
        same_word(&actual.word, &expected);
        assert_eq!(actual.exploration.stats(), facts.stats());
        assert_eq!(actual.exploration.level(), facts.level());
        assert!(actual.exploration.stats().left_on_queue > 0);
        assert!(!checks.exhausted && !checks.final_check);
        drop(actual);
        drop(expected);
        no_rows(&weak);
    }
}

#[test]
fn actual_final_and_prefix_failures_keep_their_error_and_exploration_evidence() {
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        let model = model("[]<>(1 \\div (x-x)=0)", &options);
        let result = analyze::analyze_final_with_evidence(&model, &options, Limits::default());
        let error = result.as_ref().expect_err("actual state atom failure");
        let expected = (
            error.error.code(),
            format!("{:?}", error.stage),
            error
                .exploration
                .as_ref()
                .expect("completed exploration")
                .stats,
        );
        let error = result
            .map(Analysis::into_native)
            .expect_err("failure stays failure");
        assert_eq!(
            (
                error.error.code(),
                format!("{:?}", error.stage),
                error
                    .exploration
                    .as_ref()
                    .expect("same actual exploration")
                    .stats
            ),
            expected
        );

        let result = periodic::analyze_periodic(
            &model,
            &options,
            Limits::default(),
            AcceptancePolicy::Legacy,
            periodic::Schedule {
                min_states: 2,
                growth_factor: 1,
            },
        );
        let error = result.as_ref().expect_err("actual prefix atom failure");
        let expected = (
            error.failure.failure().error.code(),
            error.capture_completed,
            error.prefix.as_ref().expect("prefix facts").stats,
            error.checks,
        );
        let error = result
            .map(|(analysis, checks)| (analysis.into_native(), checks))
            .expect_err("prefix failure stays failure");
        assert_eq!(
            (
                error.failure.failure().error.code(),
                error.capture_completed,
                error.prefix.as_ref().expect("same prefix facts").stats,
                error.checks
            ),
            expected
        );
    }
}
