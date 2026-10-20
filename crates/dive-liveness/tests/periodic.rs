//! Prefix checks reuse the final kernel without claiming frontier exhaustion.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite regression assertions"
)]
use dive_check::{CheckOptions, Verdict};
use dive_liveness::analyze::{
    Analysis, AnalysisError, Limits, Stage,
    periodic::{PeriodicAnalysis, Schedule, analyze_periodic},
};
use dive_trace::{Ending, Trace, Validation};
use std::sync::Arc;

/// Checks every grown completed level, so small graphs exercise prefixes.
const EVERY_LEVEL: Schedule = Schedule {
    min_states: 2,
    growth_factor: 1,
};

fn model(body: &str, options: &CheckOptions) -> dive_model::Model {
    model_with(body, "PROPERTY P\n", options)
}

fn model_with(body: &str, properties: &str, options: &CheckOptions) -> dive_model::Model {
    let source = format!("---- MODULE Periodic ----\nEXTENDS Naturals\nVARIABLE x\n{body}\n====\n");
    let analysis =
        dive_sema::analyze_source("Periodic.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let cfg = dive_config::parse(&format!("INIT Init\nNEXT Next\n{properties}"))
        .into_config()
        .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &cfg,
        &dive_model::BindOptions {
            eval: options.eval,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}
const CHAIN: &str = "Init == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\n";

#[test]
fn early_word_has_actual_prefix_counts_and_fresh_independent_validation() {
    for base in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        for width in [1, 4, 8] {
            let options = CheckOptions {
                workers: width,
                ..base.clone()
            };
            let body = format!("{CHAIN}P == <>(x=3)");
            let (result, checks) = analyze_periodic(
                &model(&body, &options),
                &options,
                Limits::default(),
                dive_liveness::product::AcceptancePolicy::compact_default(),
                EVERY_LEVEL,
            )
            .expect("prefix analysis");
            let PeriodicAnalysis::Prefix(candidate) = result else {
                panic!("must find early stuttering word")
            };
            let capture = &candidate.exploration;
            assert_eq!(
                (
                    capture.stats.generated,
                    capture.stats.distinct,
                    capture.stats.depth,
                    capture.stats.left_on_queue
                ),
                (2, 2, 2, 1)
            );
            assert_eq!(
                (checks.prefixes, checks.last_level, checks.last_size),
                (1, Some(1), 2)
            );
            assert!(!checks.exhausted && !checks.final_check);
            assert_eq!(
                (capture.workers.requested, capture.workers.used),
                (width, width)
            );
            assert_eq!(candidate.target_property, "P");
            assert_eq!(candidate.properties, ["P"]);
            assert_eq!(capture.graph.edges[1], [] as [dive_check::capture::Edge; 0]);
            let independent = model(&body, &options);
            let trace = Trace {
                labels: vec![None; candidate.states.len()],
                length: Some(candidate.states.len()),
                states: candidate.states,
                ending: Ending::Lasso {
                    back_to: candidate.back_to,
                },
                properties: candidate.properties,
            };
            assert_eq!(
                dive_trace::validate(&independent, &trace).validation,
                Validation::Valid
            );
            let document = trace.to_doc(&independent).expect("lossless word");
            assert_eq!(
                dive_trace::validate_doc(&independent, &document, &[])
                    .0
                    .validation,
                Validation::Valid
            );
        }
    }
}

#[test]
fn holding_requires_real_exhaustion_and_drained_levels_never_trigger_prefix_checks() {
    for width in [1, 4, 8] {
        let options = CheckOptions {
            workers: width,
            ..CheckOptions::dive(0)
        };
        for (body, expected) in [
            (format!("{CHAIN}P == []<>(x<=2)"), (4, 3, 3, 2)),
            (
                "Init == x=0\nNext == x'=x\nP == []<>(x=0)".into(),
                (2, 1, 1, 0),
            ),
        ] {
            let (result, checks) = analyze_periodic(
                &model(&body, &options),
                &options,
                Limits::default(),
                dive_liveness::product::AcceptancePolicy::compact_default(),
                EVERY_LEVEL,
            )
            .expect("complete analysis");
            let PeriodicAnalysis::Final(Analysis::Holds(capture)) = result else {
                panic!("only completed holding result allowed")
            };
            assert_eq!(
                (
                    capture.stats.generated,
                    capture.stats.distinct,
                    capture.stats.depth,
                    checks.prefixes
                ),
                expected
            );
            assert_eq!(capture.stats.left_on_queue, 0);
            assert!(checks.exhausted && checks.final_check);
            assert_eq!(
                capture.completion().entries().0,
                usize::try_from(expected.1).expect("small count")
            );
        }
    }
}

#[test]
fn prefix_atom_limits_keep_actual_stage_and_incomplete_capture_without_holds() {
    for width in [1, 4, 8] {
        let options = CheckOptions {
            workers: width,
            ..CheckOptions::dive(0)
        };
        let body = format!("{CHAIN}P == []<>(x<=2)");
        let failure = analyze_periodic(
            &model(&body, &options),
            &options,
            Limits {
                atom_cells: 0,
                ..Limits::default()
            },
            dive_liveness::product::AcceptancePolicy::compact_default(),
            EVERY_LEVEL,
        )
        .expect_err("label bound");
        assert_eq!(failure.capture_completed, Some(false));
        assert_eq!(failure.failure.failure().stage, Stage::Atoms);
        assert!(matches!(
            failure.failure.failure().error,
            AnalysisError::Limit("atom cells")
        ));
        assert!(failure.failure.failure().exploration.is_none());
        let stats = failure.prefix.as_ref().expect("separate prefix work").stats;
        assert_eq!(
            (stats.generated, stats.distinct, stats.left_on_queue),
            (2, 2, 1)
        );
        assert_eq!(failure.checks.prefixes, 1);
        assert!(!failure.checks.exhausted && !failure.checks.final_check);
        let failure = analyze_periodic(
            &model(&body, &options),
            &options,
            Limits {
                capture: dive_check::capture::Limits {
                    states: 1,
                    ..Default::default()
                },
                ..Limits::default()
            },
            dive_liveness::product::AcceptancePolicy::compact_default(),
            EVERY_LEVEL,
        )
        .expect_err("capture bound");
        assert_eq!(failure.failure.failure().stage, Stage::Capture);
        assert!(
            matches!(&failure.failure.failure().error, AnalysisError::Exploration(result) if matches!(result.verdict, Verdict::ResourceLimit { .. }))
        );
        assert_eq!(failure.checks.prefixes, 0);
    }
}

#[test]
fn selected_original_constructor_error_keeps_typed_cause_on_prefix() {
    let options = CheckOptions::tlc(0, 0);
    let body = "Init == x=[k \\in {\"a\"} |-> 0]\nNext == x'=[a |-> 0] \\/ x'=0\nP == <>[][~(x' = <<0,0>>)]_x";
    let failure = analyze_periodic(
        &model(body, &options),
        &options,
        Limits::default(),
        dive_liveness::product::AcceptancePolicy::compact_default(),
        EVERY_LEVEL,
    )
    .expect_err("actual original record target");
    assert_eq!(failure.capture_completed, Some(false));
    assert_eq!(failure.failure.failure().stage, Stage::Atoms);
    let action = failure
        .failure
        .action_evaluation()
        .expect("actual action cause");
    assert_eq!(action.cause().ec(), dive_eval::Ec::General);
    assert_eq!(action.cause().class(), dive_eval::ErrorClass::Eval);
    assert!(action.core_cause().is_some() && action.direct_operand());
    assert_eq!(action.property(), "P");
    assert_eq!(failure.checks.prefixes, 1);
}

#[test]
fn foreign_model_context_is_refused_before_planning_or_capture() {
    let options = CheckOptions::dive(0);
    let mut foreign = model(&format!("{CHAIN}P == []<>TRUE"), &options);
    let other = model(&format!("{CHAIN}P == []<>FALSE"), &options);
    foreign.names = other.names;
    let failure = analyze_periodic(
        &foreign,
        &options,
        Limits::default(),
        dive_liveness::product::AcceptancePolicy::compact_default(),
        EVERY_LEVEL,
    )
    .expect_err("foreign names");
    assert_eq!(failure.failure.failure().stage, Stage::Planning);
    assert_eq!(failure.capture_completed, Some(false));
    assert!(failure.failure.failure().exploration.is_none());
    assert_eq!(failure.checks.prefixes, 0);
    assert!(failure.failure.to_string().contains("DIVE-EVAL-C"));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "retain both reference words and per-width typed and serialized validation assertions"
)]
fn existing_pinned_fair_and_unfair_words_anchor_both_periodic_profiles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tlc-probes");
    for name in ["LvNoFair", "LvWFIntermittent"] {
        let directory = root.join("specs").join(name);
        let source = std::fs::read_to_string(directory.join(format!("{name}.tla")))
            .expect("existing source");
        let config = dive_config::parse(
            &std::fs::read_to_string(directory.join(format!("{name}.cfg")))
                .expect("existing config"),
        )
        .into_config()
        .expect("config");
        let oracle = root.join("tlc-2026.10.06.014338");
        assert_eq!(
            std::fs::read_to_string(oracle.join(format!("{name}.exit")))
                .expect("pinned status")
                .trim(),
            "13"
        );
        let raw = std::fs::read_to_string(oracle.join(format!("{name}.trace.json")))
            .expect("existing reference word");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("reference JSON");
        let actions = json["counterexample"]["action"]
            .as_array()
            .expect("original actions");
        let back_to = usize::try_from(
            actions.last().expect("closing action")[2][0]
                .as_u64()
                .expect("target")
                - 1,
        )
        .expect("small reference word");
        let reference = dive_trace::TraceDoc::parse(&raw)
            .expect("reference document")
            .with_ending(Ending::Lasso { back_to })
            .with_properties(vec!["Prop".into()]);
        for base in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
            for width in [1, 4, 8] {
                let options = CheckOptions {
                    workers: width,
                    ..base.clone()
                };
                let fresh = || {
                    let analysis = dive_sema::analyze_source(
                        &format!("{name}.tla"),
                        &source,
                        &dive_sema::Options::default(),
                    );
                    assert!(analysis.accepted());
                    dive_model::bind(
                        Arc::new(dive_ir::lower(&analysis).expect("IR")),
                        &config,
                        &dive_model::BindOptions {
                            eval: options.eval,
                            no_deadlock: false,
                            host: None,
                        },
                    )
                    .expect("fresh model")
                };
                let (analysis, checks) = analyze_periodic(
                    &fresh(),
                    &options,
                    Limits::default(),
                    dive_liveness::product::AcceptancePolicy::compact_default(),
                    EVERY_LEVEL,
                )
                .expect("periodic analysis");
                let (states, properties, back_to) = match analysis {
                    PeriodicAnalysis::Prefix(candidate) => {
                        assert!(!checks.exhausted && !checks.final_check);
                        assert!(candidate.exploration.stats.left_on_queue > 0);
                        (candidate.states, candidate.properties, candidate.back_to)
                    }
                    PeriodicAnalysis::Final(Analysis::Candidate(candidate)) => {
                        assert!(checks.exhausted && checks.final_check);
                        assert_eq!(candidate.exploration.stats.left_on_queue, 0);
                        (candidate.states, candidate.properties, candidate.back_to)
                    }
                    PeriodicAnalysis::Final(Analysis::Holds(_)) => {
                        panic!("pinned violated property cannot hold")
                    }
                };
                let independent = fresh();
                assert_eq!(
                    dive_trace::validate_doc(&independent, &reference, &config.model_values)
                        .0
                        .validation,
                    Validation::Valid,
                    "{name}: existing primary word"
                );
                assert_eq!(properties, ["Prop"]);
                let trace = Trace {
                    labels: vec![None; states.len()],
                    length: Some(states.len()),
                    states,
                    ending: Ending::Lasso { back_to },
                    properties,
                };
                assert_eq!(
                    dive_trace::validate(&independent, &trace).validation,
                    Validation::Valid,
                    "{name}: typed produced word"
                );
                let document = trace.to_doc(&independent).expect("lossless document");
                assert_eq!(
                    dive_trace::validate_doc(&independent, &document, &config.model_values)
                        .0
                        .validation,
                    Validation::Valid,
                    "{name}: serialized produced word"
                );
            }
        }
    }
}

#[test]
fn prefixes_decide_only_the_first_property_so_attribution_matches_final_checking() {
    // P2 fails on the first prefix by stuttering at x=0; P1 fails only once
    // x reaches 5. A final-only search names P1 first, so the prefix must not
    // stop with P2's word.
    let body = "Init == x=0\nNext == x'=IF x<6 THEN x+1 ELSE x\n\
                P1 == [](x>=5 => <>(x=9))\nP2 == <>(x=9)";
    for width in [1, 4] {
        let options = CheckOptions {
            workers: width,
            ..CheckOptions::dive(0)
        };
        let (result, checks) = analyze_periodic(
            &model_with(body, "PROPERTY P1\nPROPERTY P2\n", &options),
            &options,
            Limits::default(),
            dive_liveness::product::AcceptancePolicy::compact_default(),
            EVERY_LEVEL,
        )
        .expect("periodic analysis");
        // P2's word exists from level 1, but only P1's word, first reachable
        // at level 6 with x=6 still queued, may stop the run.
        let PeriodicAnalysis::Prefix(word) = result else {
            panic!("P1 fails on the level-6 prefix: {result:?}")
        };
        assert_eq!(word.exploration.level(), 6);
        assert_eq!((checks.prefixes, checks.last_level), (6, Some(6)));
        assert!(!checks.exhausted && !checks.final_check);
        assert_eq!(word.target_property, "P1");
        assert_eq!(word.properties, ["P1", "P2"]);
    }
}
