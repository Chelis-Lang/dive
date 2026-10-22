//! Composition counts and concrete edges survive the shared observer kernel.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::sync::Arc;

use dive_check::{
    CheckOptions, Stats, Verdict,
    capture::{self, BehaviorGraph, Edge, Exploration, Limits, Observer},
};
use dive_eval::{ActionComposition, next::StateVals};
use dive_store::StoreMode;
use dive_value::Value;

fn model(source: &str, config: &str, opts: &CheckOptions) -> dive_model::Model {
    let name = source.split_whitespace().nth(2).expect("module");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(
        ir,
        &config,
        &dive_model::BindOptions {
            eval: opts.eval,
            symmetry_liveness: opts.symmetry_liveness,
            no_deadlock: opts.no_deadlock,
            host: None,
        },
    )
    .expect("model")
}

fn complete(result: Exploration) -> capture::Complete {
    let Exploration::Complete(result) = result else {
        panic!("{result:?}")
    };
    *result
}

#[derive(Default)]
struct Events {
    initial: Vec<Stats>,
    edges: Vec<(usize, Edge, StateVals, Stats)>,
    levels: Vec<(u64, Stats, usize, usize)>,
}
impl Observer for Events {
    fn init(&mut self, _: usize, _: &StateVals, counters: Stats) -> Result<(), String> {
        self.initial.push(counters);
        Ok(())
    }
    fn edge(
        &mut self,
        from: usize,
        edge: &Edge,
        successor: &StateVals,
        counters: Stats,
    ) -> Result<(), String> {
        self.edges
            .push((from, edge.clone(), successor.clone(), counters));
        Ok(())
    }
    fn level_done(
        &mut self,
        level: u64,
        graph: &BehaviorGraph,
        counters: Stats,
    ) -> Result<(), String> {
        self.levels.push((
            level,
            counters,
            graph.edges.iter().map(Vec::len).sum(),
            graph.concrete.iter().map(Vec::len).sum(),
        ));
        Ok(())
    }
}

#[test]
fn pinned_tlc_intermediates_count_in_completed_levels_but_never_become_edges() {
    let source = include_str!("../../dive-eval/tests/fixtures/cdot/specs/CdNext/CdNext.tla");
    let cfg = include_str!("../../dive-eval/tests/fixtures/cdot/specs/CdNext/CdNext.cfg");
    for mut opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        opts.eval.action_composition = ActionComposition::Tlc;
        for store in [StoreMode::Exact, StoreMode::Fingerprint] {
            opts.store = store;
            for width in [1, 4, 8] {
                opts.workers = width;
                let captured = complete(capture::explore(
                    &model(source, cfg, &opts),
                    &opts,
                    Limits::default(),
                ));
                assert_eq!(captured.safety_result().verdict, Verdict::Success);
                assert_eq!(
                    (
                        captured.stats.generated,
                        captured.stats.distinct,
                        captured.stats.depth
                    ),
                    (65, 18, 4)
                );
                assert_eq!(captured.workers.used, 1);
                assert!(captured.workers.sequential);
                assert!(
                    captured
                        .workers
                        .reason
                        .as_ref()
                        .expect("inline reason")
                        .contains("action composition")
                );
                let emitted = captured.graph.concrete.iter().map(Vec::len).sum::<usize>();
                let edges = captured.graph.edges.iter().map(Vec::len).sum::<usize>();
                assert_eq!((emitted, edges), (44, 62));
                if width == 1 {
                    let mut events = Events::default();
                    let hooked = complete(
                        capture::explore_observed_with_evidence(
                            &model(source, cfg, &opts),
                            &opts,
                            Limits::default(),
                            &mut events,
                        )
                        .expect("observer"),
                    );
                    assert_eq!(hooked.stats, captured.stats);
                    assert_eq!(events.initial.len(), 1);
                    assert_eq!(events.initial[0].generated, 1);
                    assert_eq!(events.edges.len(), 62);
                    assert_eq!(
                        events
                            .levels
                            .iter()
                            .map(|(level, counters, _, _)| (
                                *level,
                                counters.generated,
                                counters.distinct,
                                counters.left_on_queue
                            ))
                            .collect::<Vec<_>>(),
                        [(1, 7, 5, 4), (2, 31, 12, 7), (3, 58, 18, 6), (4, 65, 18, 0)]
                    );
                    assert_eq!(
                        events
                            .levels
                            .iter()
                            .map(|(_, _, edges, emitted)| (*edges, *emitted))
                            .collect::<Vec<_>>(),
                        [(5, 4), (25, 20), (51, 39), (62, 44)]
                    );
                    assert_eq!(
                        events
                            .edges
                            .iter()
                            .filter(|(_, edge, _, _)| edge.ordinal.is_some())
                            .count(),
                        44
                    );
                    assert_eq!(
                        events.edges[..4]
                            .iter()
                            .map(|(_, edge, _, _)| edge.ordinal)
                            .collect::<Vec<_>>(),
                        [Some(0), Some(1), Some(2), Some(3)]
                    );
                }
            }
        }
    }
}

#[test]
fn composition_events_keep_original_equal_values_and_mode_specific_counts() {
    let source = "---- MODULE ConcreteComposition ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = <<1>>\nNext == (x'=x) \\cdot (x'=[i \\in 1..1 |-> i])\n====\n";
    let cfg = "INIT Init\nNEXT Next\n";
    for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for mode in [ActionComposition::Tlc, ActionComposition::Tla] {
            opts.eval.action_composition = mode;
            for store in [StoreMode::Exact, StoreMode::Fingerprint] {
                opts.store = store;
                for width in [1, 4, 8] {
                    opts.workers = width;
                    let captured = complete(capture::explore(
                        &model(source, cfg, &opts),
                        &opts,
                        Limits::default(),
                    ));
                    assert_eq!(
                        (captured.stats.generated, captured.stats.distinct),
                        if mode == ActionComposition::Tlc {
                            (3, 1)
                        } else {
                            (2, 1)
                        }
                    );
                    assert_eq!(captured.graph.edges[0].len(), 2);
                    assert_eq!(captured.graph.concrete[0].len(), 1);
                    assert_eq!(
                        captured.workers.used,
                        if mode == ActionComposition::Tlc {
                            1
                        } else {
                            width
                        }
                    );
                    if width == 1 {
                        let mut events = Events::default();
                        let hooked = complete(
                            capture::explore_observed_with_evidence(
                                &model(source, cfg, &opts),
                                &opts,
                                Limits::default(),
                                &mut events,
                            )
                            .expect("observer"),
                        );
                        assert_eq!(hooked.stats, captured.stats);
                        assert_eq!(events.edges.len(), 2);
                        let Value::Fcn(stored) =
                            hooked.graph.states[0][0].as_ref().expect("stored")
                        else {
                            panic!("tuple")
                        };
                        let Value::Fcn(original) = events.edges[0].2[0].as_ref().expect("original")
                        else {
                            panic!("function")
                        };
                        assert!(stored.is_tuple_literal());
                        assert!(!original.is_tuple_literal());
                        assert_eq!(events.edges[0].1.ordinal, Some(0));
                        assert_eq!(events.edges[1].1.ordinal, None);
                        assert_eq!(events.levels[0].1, hooked.stats);
                    }
                }
            }
        }
    }
}

#[test]
fn the_composition_accounting_reason_does_not_admit_live_state_or_effects() {
    let source = "---- MODULE CompositionEffects ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == TLCSet(0,1) /\\ ((x'=x) \\cdot (x'=x))\n====\n";
    for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        opts.eval.action_composition = ActionComposition::Tlc;
        let mut events = Events::default();
        let m = model(source, "INIT Init\nNEXT Next\n", &opts);
        let result =
            capture::explore_observed_with_evidence(&m, &opts, Limits::default(), &mut events)
                .expect("refused before execution");
        assert!(
            matches!(result,Exploration::Stopped(result) if matches!(result.verdict,Verdict::Unsupported{ref feature} if feature.contains("stateful checking")))
        );
        assert_eq!(events.initial, []);
        assert_eq!(events.edges.len(), 0);
        assert_eq!(events.levels.len(), 0);
        assert_eq!(m.evaluator.take_output(), [] as [String; 0]);
    }
}

#[test]
fn intermediate_budget_excess_prevents_level_events_without_overriding_safety() {
    let deadlock =
        include_str!("../../dive-eval/tests/fixtures/cdot/specs/CdDeadlock/CdDeadlock.tla");
    let violation = "---- MODULE CompositionStop ----\nVARIABLE x\nInit == x=0\nNext == (x'=1) \\cdot (x'=2)\nInv == x=0\n====\n";
    for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        opts.eval.action_composition = ActionComposition::Tlc;
        for (source, config, limit, expected, generated) in [
            (deadlock, "INIT Init\nNEXT Next\n", 1, None, 2),
            (
                violation,
                "INIT Init\nNEXT Next\nINVARIANT Inv\n",
                2,
                Some("Inv"),
                3,
            ),
        ] {
            let mut events = Events::default();
            let m = model(source, config, &opts);
            let limits = Limits {
                generated: limit,
                ..Limits::default()
            };
            let Exploration::Stopped(result) =
                capture::explore_observed_with_evidence(&m, &opts, limits, &mut events)
                    .expect("bounded exploration")
            else {
                panic!("must stop")
            };
            if let Some(property) = expected {
                assert!(
                    matches!(result.verdict,Verdict::InvariantViolated{property:ref name,..} if name==property)
                );
            } else {
                assert!(matches!(result.verdict, Verdict::ResourceLimit { .. }));
            }
            assert_eq!(result.stats.expect("actual counts").generated, generated);
            assert_eq!(events.initial.len(), 1);
            assert_eq!(events.edges.len(), 0);
            assert_eq!(events.levels.len(), 0);
        }
    }
}
