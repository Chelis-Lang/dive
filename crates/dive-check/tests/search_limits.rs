//! Explicit search ceilings retain exact admission and error boundaries.
#![allow(clippy::expect_used, clippy::panic, reason = "finite search controls")]

use dive_check::{
    CheckOptions, Verdict,
    budget::{Phase, Resource},
};
use dive_store::StoreMode;
use std::sync::Arc;

fn input(
    init: &str,
    next: &str,
    invariant: Option<&str>,
) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let source = format!(
        "---- MODULE Bound ----\nEXTENDS Naturals\nVARIABLE x\nInit == {init}\nNext == {next}\nInv == {}\n====\n",
        invariant.unwrap_or("TRUE")
    );
    let analysis = dive_sema::analyze_source("Bound.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let config = dive_config::parse(&format!(
        "INIT Init\nNEXT Next\n{}",
        if invariant.is_some() {
            "INVARIANT Inv\n"
        } else {
            ""
        }
    ))
    .into_config()
    .expect("config");
    (Arc::new(dive_ir::lower(&analysis).expect("IR")), config)
}

fn options() -> Vec<CheckOptions> {
    [CheckOptions::tlc(0, 0), CheckOptions::dive(0)]
        .into_iter()
        .flat_map(|base| {
            [StoreMode::Fingerprint, StoreMode::Exact]
                .into_iter()
                .flat_map(move |store| {
                    let base = base.clone();
                    [1, 4, 8].map(|workers| CheckOptions {
                        store,
                        workers,
                        ..base.clone()
                    })
                })
        })
        .collect()
}

#[test]
fn exact_state_and_depth_bounds_may_exhaust_but_a_truncated_frontier_never_proves() {
    let (ir, config) = input("x = 0", "x' = IF x < 3 THEN x + 1 ELSE x", None);
    for base in options() {
        for (states, depth, expected, distinct, observed) in [
            (Some(0), None, Some(Resource::States), 0, 1),
            (Some(1), None, Some(Resource::States), 1, 2),
            (Some(4), None, None, 4, 0),
            (None, Some(0), Some(Resource::Depth), 0, 1),
            (None, Some(3), Some(Resource::Depth), 3, 4),
            (None, Some(4), None, 4, 0),
        ] {
            let opts = CheckOptions {
                max_states: states,
                max_depth: depth,
                ..base.clone()
            };
            let result = dive_check::check(&ir, &config, &opts);
            assert_eq!(
                result.stats.expect("progress").distinct,
                distinct,
                "{result:?}"
            );
            match (&result.verdict, expected) {
                (Verdict::Success, None) => {
                    assert_eq!(result.stats.expect("complete").generated, 5);
                }
                (Verdict::Stopped(stop), Some(resource)) => {
                    assert_eq!(
                        (stop.resource, stop.phase, stop.observed),
                        (resource, Phase::Search, observed)
                    );
                    assert!(result.counterexample.is_none(), "{result:?}");
                    assert_eq!(result.stats.expect("progress").depth, distinct.min(4));
                }
                other => panic!("unexpected boundary: {other:?}, {result:?}"),
            }
        }
    }
}

#[test]
fn initial_admission_and_parallel_winners_preserve_the_first_error() {
    for base in options() {
        let (ir, config) = input("x \\in 0..10", "UNCHANGED x", None);
        let opts = CheckOptions {
            max_states: Some(2),
            ..base.clone()
        };
        let result = dive_check::check(&ir, &config, &opts);
        assert!(matches!(result.verdict, Verdict::Stopped(_)), "{result:?}");
        let stats = result.stats.expect("initial progress");
        assert_eq!(
            (
                stats.distinct,
                stats.generated,
                stats.initial_generated,
                stats.depth
            ),
            (2, 3, 3, 1)
        );
        for (invariant, stopped) in [("x # 9", false), ("x # 10", true)] {
            let (ir, config) = input("x = 0", "x' \\in 1..100", Some(invariant));
            let opts = CheckOptions {
                max_states: Some(10),
                ..base.clone()
            };
            let result = dive_check::check(&ir, &config, &opts);
            assert_eq!(
                matches!(result.verdict, Verdict::Stopped(_)),
                stopped,
                "{result:?}"
            );
            assert_eq!(
                matches!(result.verdict, Verdict::InvariantViolated { .. }),
                !stopped,
                "{result:?}"
            );
            assert_eq!(result.stats.expect("progress").distinct, 10);
            if stopped {
                assert!(result.counterexample.is_none());
            }
        }
    }
}

#[test]
fn capture_cannot_certify_a_graph_cut_by_an_explicit_search_limit() {
    let (ir, config) = input("x = 0", "x' = IF x < 3 THEN x + 1 ELSE x", None);
    for base in options() {
        let binding = if base.profile == dive_eval::Profile::Tlc {
            dive_model::BindOptions::tlc()
        } else {
            dive_model::BindOptions::dive()
        };
        let model = dive_model::bind(ir.clone(), &config, &binding).expect("model");
        let opts = CheckOptions {
            max_states: Some(2),
            ..base
        };
        match dive_check::capture::explore(&model, &opts, dive_check::capture::Limits::default()) {
            dive_check::capture::Exploration::Stopped(result) => {
                let Verdict::Stopped(stop) = &result.verdict else {
                    panic!("{result:?}")
                };
                assert_eq!(
                    (stop.resource, stop.phase, stop.limit, stop.observed),
                    (Resource::States, Phase::Capture, 2, 3)
                );
                assert_eq!(result.stats.expect("progress").distinct, 2);
                assert!(result.counterexample.is_none());
            }
            other @ dive_check::capture::Exploration::Complete(_) => {
                panic!("truncated graph cannot complete: {other:?}");
            }
        }
    }
}

#[test]
fn constraints_views_and_key_errors_keep_their_original_admission_order() {
    for base in options() {
        for config in [
            "INIT Init\nNEXT Next\nVIEW V\n",
            "INIT Init\nNEXT Next\nCONSTRAINT C\n",
        ] {
            let (ir, _) = input("x = 0", "x' \\in 1..10\nV == 0\nC == x < 2", None);
            let config = dive_config::parse(config).into_config().expect("config");
            let opts = CheckOptions {
                max_states: Some(if config.view.is_some() { 1 } else { 2 }),
                max_depth: Some(2),
                ..base.clone()
            };
            let result = dive_check::check(&ir, &config, &opts);
            assert!(matches!(result.verdict, Verdict::Success), "{result:?}");
            assert_eq!(
                result.stats.expect("complete").distinct,
                opts.max_states.expect("cap")
            );
        }
        let (ir, _) = input("x = 0", "x' = 1\nV == IF x = 0 THEN 0 ELSE 1 \\div 0", None);
        let config = dive_config::parse("INIT Init\nNEXT Next\nVIEW V\n")
            .into_config()
            .expect("config");
        let result = dive_check::check(
            &ir,
            &config,
            &CheckOptions {
                max_states: Some(1),
                ..base.clone()
            },
        );
        assert!(
            matches!(
                result.verdict,
                Verdict::EvaluationError {
                    site: dive_check::ErrorSite::View,
                    ..
                }
            ),
            "{result:?}"
        );
        let (ir, config) = input("x = 0", "x' = x \\/ TRUE", None);
        let result = dive_check::check(
            &ir,
            &config,
            &CheckOptions {
                max_states: Some(1),
                ..base
            },
        );
        assert!(
            matches!(
                result.verdict,
                Verdict::NotCompletelySpecified { initial: false, .. }
            ),
            "{result:?}"
        );
    }
}

#[test]
fn earlier_evaluation_errors_bound_diagnostic_replay_before_further_admission() {
    let (ir, config) = input(
        "x = 0",
        "x' = 1 \\/ x' = 2 \\/ x' = 3",
        Some("IF x = 1 THEN (1 \\div 0) = 0 ELSE TRUE"),
    );
    for base in options() {
        for disk in [false, true] {
            for cap in [2, 3, 4] {
                let opts = CheckOptions {
                    max_states: Some(cap),
                    seen_disk: disk.then(Default::default),
                    ..base.clone()
                };
                let result = dive_check::check(&ir, &config, &opts);
                assert!(
                    matches!(
                        result.verdict,
                        Verdict::EvaluationError {
                            site: dive_check::ErrorSite::Invariant,
                            ..
                        }
                    ),
                    "{result:?}"
                );
                assert_eq!(
                    result.stats.expect("error prefix").distinct,
                    cap,
                    "{result:?}"
                );
                if let Some(usage) = result.seen {
                    assert_eq!(usage.admitted_keys, cap);
                    assert_eq!(
                        usage
                            .before_cleanup
                            .expect("physical keys")
                            .physical_entries,
                        cap
                    );
                    assert!(usage.after_cleanup.expect("cleanup").cleanup_completed);
                }
            }
        }
    }
}

#[test]
fn disk_admission_never_inserts_the_first_over_budget_identity() {
    let (ir, config) = input("x = 0", "x' = IF x < 3 THEN x + 1 ELSE x", None);
    for base in options() {
        for cap in [0, 1, 3, 4] {
            let result = dive_check::check(
                &ir,
                &config,
                &CheckOptions {
                    max_states: Some(cap),
                    seen_disk: Some(dive_check::SeenOptions::default()),
                    ..base.clone()
                },
            );
            assert_eq!(
                matches!(result.verdict, Verdict::Success),
                cap == 4,
                "{result:?}"
            );
            let usage = result.seen.expect("disk evidence");
            assert_eq!(usage.admitted_keys, cap);
            assert_eq!(
                usage
                    .before_cleanup
                    .expect("physical keys")
                    .physical_entries,
                cap
            );
            assert!(usage.after_cleanup.expect("cleanup").cleanup_completed);
        }
    }
}

#[test]
fn bounded_parallel_publication_is_replayable_across_claim_orders() {
    let (ir, config) = input("x = 0", "x' \\in 1..100", None);
    for base in options().into_iter().filter(|o| o.workers > 1) {
        for chaos in [None, Some(0), Some(17)] {
            let result = dive_check::check(
                &ir,
                &config,
                &CheckOptions {
                    max_states: Some(10),
                    chaos,
                    ..base.clone()
                },
            );
            let Verdict::Stopped(stop) = &result.verdict else {
                panic!("{result:?}")
            };
            assert_eq!(
                (stop.resource, stop.limit, stop.observed),
                (Resource::States, 10, 11)
            );
            let stats = result.stats.expect("reference prefix");
            assert_eq!((stats.generated, stats.distinct, stats.depth), (11, 10, 2));
            assert_eq!(result.workers.used, base.workers);
            assert!(!result.workers.sequential);
        }
    }
}
