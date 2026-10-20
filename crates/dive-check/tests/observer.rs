//! Committed graph events preserve actual safety results and boundary counters.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test assertions and controlled callback panic"
)]

use std::sync::Arc;

use dive_check::{
    CheckOptions, Stats, Verdict,
    capture::{self, BehaviorGraph, Edge, Exploration, Limits, Observer},
};
use dive_eval::next::StateVals;
use dive_store::StoreMode;
use dive_value::Value;

fn model(body: &str, config: &str, opts: &CheckOptions) -> dive_model::Model {
    let source =
        format!("---- MODULE Observe ----\nEXTENDS Naturals, TLC\nVARIABLE x\n{body}\n====\n");
    let analysis =
        dive_sema::analyze_source("Observe.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            no_deadlock: opts.no_deadlock,
            host: None,
        },
    )
    .expect("model")
}

#[derive(Default)]
struct Events {
    roots: Vec<(usize, StateVals, Stats)>,
    edges: Vec<(usize, Edge, StateVals, Stats)>,
    levels: Vec<(u64, BehaviorGraph, Stats)>,
}
impl Observer for Events {
    fn init(&mut self, id: usize, vals: &StateVals, stats: Stats) -> Result<(), String> {
        self.roots.push((id, vals.clone(), stats));
        Ok(())
    }
    fn edge(
        &mut self,
        from: usize,
        edge: &Edge,
        successor: &StateVals,
        stats: Stats,
    ) -> Result<(), String> {
        self.edges
            .push((from, edge.clone(), successor.clone(), stats));
        Ok(())
    }
    fn level_done(
        &mut self,
        level: u64,
        graph: &BehaviorGraph,
        stats: Stats,
    ) -> Result<(), String> {
        self.levels.push((level, graph.clone(), stats));
        Ok(())
    }
}

fn observed(
    m: &dive_model::Model,
    opts: &CheckOptions,
    limits: Limits,
    events: &mut dyn Observer,
) -> Exploration {
    capture::explore_observed_with_evidence(m, opts, limits, events).expect("execution evidence")
}

fn complete(result: Exploration) -> capture::Complete {
    let Exploration::Complete(result) = result else {
        panic!("{result:?}")
    };
    *result
}

#[test]
fn inert_observer_preserves_plain_capture_in_both_executors_and_profiles() {
    struct Inert;
    impl Observer for Inert {}
    for mut opts in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        for store in [StoreMode::Exact, StoreMode::Fingerprint] {
            for inline in [false, true] {
                opts.store = store;
                opts.sequential = inline;
                let body = "Init == x=0\nNext == (x<2 /\\ x'=x+1) \\/ x'=x";
                let cfg = "INIT Init\nNEXT Next\n";
                let plain = complete(capture::explore(
                    &model(body, cfg, &opts),
                    &opts,
                    Limits::default(),
                ));
                let hooked = complete(observed(
                    &model(body, cfg, &opts),
                    &opts,
                    Limits::default(),
                    &mut Inert,
                ));
                assert_eq!(hooked.stats, plain.stats);
                assert_eq!(
                    hooked.safety_result().verdict,
                    plain.safety_result().verdict
                );
                assert_eq!(hooked.workers, plain.workers);
                assert_eq!(hooked.graph.edges, plain.graph.edges);
                assert_eq!(hooked.graph.states, plain.graph.states);
                assert_eq!(hooked.graph.roots, plain.graph.roots);
            }
        }
    }
}

#[test]
fn completed_levels_have_appended_frontiers_and_real_concrete_edge_order() {
    for inline in [false, true] {
        let opts = CheckOptions {
            sequential: inline,
            ..CheckOptions::tlc(0, 0)
        };
        let m = model(
            "Init == x=0\nNext == (x<2 /\\ x'=x+1) \\/ x'=x",
            "INIT Init\nNEXT Next\n",
            &opts,
        );
        let mut events = Events::default();
        let result = complete(observed(&m, &opts, Limits::default(), &mut events));
        assert_eq!(events.roots.len(), 1);
        assert_eq!(
            (
                events.roots[0].0,
                events.roots[0].2.generated,
                events.roots[0].2.left_on_queue
            ),
            (0, 1, 1)
        );
        assert_eq!(
            events
                .levels
                .iter()
                .map(|(level, graph, stats)| (
                    *level,
                    graph.states.len(),
                    stats.generated,
                    stats.left_on_queue
                ))
                .collect::<Vec<_>>(),
            [(1, 2, 3, 1), (2, 3, 5, 1), (3, 3, 6, 0)]
        );
        assert_eq!(events.levels[0].1.edges[1].len(), 0);
        assert_eq!(events.edges.len(), 8);
        assert_eq!(
            events
                .edges
                .iter()
                .map(|(from, edge, _, _)| (*from, edge.to, edge.ordinal))
                .collect::<Vec<_>>(),
            [
                (0, 1, Some(0)),
                (0, 0, Some(1)),
                (0, 0, None),
                (1, 2, Some(0)),
                (1, 1, Some(1)),
                (1, 1, None),
                (2, 2, Some(0)),
                (2, 2, None)
            ]
        );
        for (_, edge, successor, _) in &events.edges {
            assert_eq!(successor, &result.graph.states[edge.to]);
        }
    }
}

#[test]
fn constraints_and_duplicates_count_generated_candidates_separately_from_edges() {
    let opts = CheckOptions::tlc(0, 0);
    let m = model(
        "Init == x \\in {0,1,2}\nNext == x'=0 \\/ x'=0 \\/ x'=2\nKeep == x<2",
        "INIT Init\nNEXT Next\nCONSTRAINT Keep\n",
        &opts,
    );
    let mut events = Events::default();
    let result = complete(observed(&m, &opts, Limits::default(), &mut events));
    assert_eq!(result.stats.generated, 9);
    assert_eq!(result.stats.distinct, 2);
    assert_eq!(
        events.roots.iter().map(|root| root.0).collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(events.roots.iter().all(|root| root.2.generated == 3));
    assert_eq!(events.edges.len(), 6);
    assert!(
        events
            .edges
            .iter()
            .all(|(_, _, state, stats)| state[0] != Some(Value::Int(2)) && stats.generated == 9)
    );
}

#[test]
fn edge_events_keep_original_values_before_equal_representative_replacement() {
    for inline in [false, true] {
        let opts = CheckOptions {
            sequential: inline,
            ..CheckOptions::tlc(0, 0)
        };
        let m = model(
            "Init == x = <<1>>\nNext == x'=[i \\in 1..1 |-> i]",
            "INIT Init\nNEXT Next\n",
            &opts,
        );
        let mut events = Events::default();
        let result = complete(observed(&m, &opts, Limits::default(), &mut events));
        let Value::Fcn(stored) = result.graph.states[0][0].as_ref().expect("representative") else {
            panic!("tuple representative")
        };
        let Value::Fcn(original) = events.edges[0].2[0].as_ref().expect("concrete event") else {
            panic!("function event")
        };
        assert!(stored.is_tuple_literal());
        assert!(!original.is_tuple_literal());
        assert_eq!(events.edges[0].2, result.graph.concrete[0][0].state);
        assert!(events.edges[1].1.ordinal.is_none());
        assert_eq!(events.edges[1].2, result.graph.states[0]);
    }
}

#[test]
fn later_partial_level_preserves_only_the_completed_graph_prefix() {
    for inline in [false, true] {
        let opts = CheckOptions {
            sequential: inline,
            ..CheckOptions::tlc(0, 0)
        };
        let body = "Init == x=0\nNext == (x=0 /\\ x'=1) \\/ (x=1 /\\ x'=2)\nInv == x<2";
        let cfg = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
        let mut events = Events::default();
        let plain = capture::explore(&model(body, cfg, &opts), &opts, Limits::default());
        let result = observed(
            &model(body, cfg, &opts),
            &opts,
            Limits::default(),
            &mut events,
        );
        let (Exploration::Stopped(plain), Exploration::Stopped(result)) = (plain, result) else {
            panic!("later safety stop")
        };
        assert_eq!(plain.verdict, result.verdict);
        assert_eq!(plain.stats, result.stats);
        assert_eq!(events.levels.len(), 1);
        assert_eq!(events.levels[0].0, 1);
        assert_eq!(events.levels[0].1.states.len(), 2);
        assert_eq!(events.levels[0].1.edges[1], []);
        assert_eq!(events.edges.len(), 2);
        assert!(events.edges.iter().all(|(from, _, _, _)| *from == 0));
    }
}

#[derive(Clone, Copy)]
enum FaultAt {
    Init,
    Edge,
    Level,
}
struct Fault {
    at: FaultAt,
    panic: bool,
    calls: usize,
}
impl Fault {
    fn fail(&mut self) -> Result<(), String> {
        self.calls += 1;
        assert!(!self.panic, "controlled observer failure");
        Err("control stop".into())
    }
}
impl Observer for Fault {
    fn init(&mut self, _: usize, _: &StateVals, _: Stats) -> Result<(), String> {
        if matches!(self.at, FaultAt::Init) {
            self.fail()
        } else {
            Ok(())
        }
    }
    fn edge(&mut self, _: usize, _: &Edge, _: &StateVals, _: Stats) -> Result<(), String> {
        if matches!(self.at, FaultAt::Edge) {
            self.fail()
        } else {
            Ok(())
        }
    }
    fn level_done(&mut self, _: u64, _: &BehaviorGraph, _: Stats) -> Result<(), String> {
        if matches!(self.at, FaultAt::Level) {
            self.fail()
        } else {
            Ok(())
        }
    }
}

#[test]
fn callback_errors_and_unwinds_retain_real_boundary_counters_without_completion() {
    for inline in [false, true] {
        for at in [FaultAt::Init, FaultAt::Edge, FaultAt::Level] {
            for panic in [false, true] {
                let opts = CheckOptions {
                    sequential: inline,
                    ..CheckOptions::tlc(0, 0)
                };
                let m = model(
                    "Init == x=0\nNext == (x<2 /\\ x'=x+1) \\/ x'=x",
                    "INIT Init\nNEXT Next\n",
                    &opts,
                );
                let mut fault = Fault {
                    at,
                    panic,
                    calls: 0,
                };
                let Exploration::Stopped(result) =
                    observed(&m, &opts, Limits::default(), &mut fault)
                else {
                    panic!("callback stop")
                };
                assert_eq!(fault.calls, 1);
                assert!(
                    matches!(result.verdict, Verdict::Unsupported { .. })
                        || (panic && matches!(result.verdict, Verdict::InternalError { .. }))
                );
                let stats = result.stats.expect("real counters");
                assert_eq!(
                    (
                        stats.generated,
                        stats.distinct,
                        stats.left_on_queue,
                        stats.depth
                    ),
                    if matches!(at, FaultAt::Init) {
                        (1, 1, 1, 1)
                    } else {
                        (3, 2, 1, 2)
                    }
                );
                assert!(result.counterexample.is_none());
            }
        }
    }
}

#[test]
fn failed_initialization_emits_no_event_and_retains_the_deciding_result() {
    for body in [
        "Init == x=0\nNext == x'=x\nInv == x#0",
        "Init == x=0 \\/ TRUE\nNext == x'=x\nInv == x=x",
        "Init == x=0\nNext == x'=x\nInv == x=1\\div 0",
    ] {
        let opts = CheckOptions::tlc(0, 0);
        let cfg = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
        let mut events = Events::default();
        let plain = capture::explore(&model(body, cfg, &opts), &opts, Limits::default());
        let result = observed(
            &model(body, cfg, &opts),
            &opts,
            Limits::default(),
            &mut events,
        );
        let (Exploration::Stopped(plain), Exploration::Stopped(result)) = (plain, result) else {
            panic!("Init stop")
        };
        assert_eq!(plain.verdict, result.verdict);
        assert_eq!(plain.stats, result.stats);
        assert!(events.roots.is_empty() && events.edges.is_empty() && events.levels.is_empty());
    }
}

#[test]
fn safety_and_resource_stops_in_partial_levels_prevent_failing_callbacks() {
    for inline in [false, true] {
        for (body, cfg, limits) in [
            (
                "Init == x=0\nNext == x'=1\nInv == x=0",
                "INIT Init\nNEXT Next\nINVARIANT Inv\n",
                Limits::default(),
            ),
            (
                "Init == x=0\nNext == x'=1\\div 0",
                "INIT Init\nNEXT Next\n",
                Limits::default(),
            ),
            (
                "Init == x=0\nNext == x=1 /\\ x'=x",
                "INIT Init\nNEXT Next\n",
                Limits::default(),
            ),
            (
                "Init == x=0\nNext == x'=1",
                "INIT Init\nNEXT Next\n",
                Limits {
                    edges: 0,
                    ..Limits::default()
                },
            ),
            (
                "Init == x=0\nNext == x'=1",
                "INIT Init\nNEXT Next\n",
                Limits {
                    generated: 1,
                    ..Limits::default()
                },
            ),
        ] {
            let opts = CheckOptions {
                sequential: inline,
                ..CheckOptions::tlc(0, 0)
            };
            let plain = capture::explore(&model(body, cfg, &opts), &opts, limits);
            let mut fault = Fault {
                at: FaultAt::Edge,
                panic: true,
                calls: 0,
            };
            let result = observed(&model(body, cfg, &opts), &opts, limits, &mut fault);
            let (Exploration::Stopped(plain), Exploration::Stopped(result)) = (plain, result)
            else {
                panic!("source stop")
            };
            assert_eq!(plain.verdict, result.verdict);
            assert_eq!(plain.stats, result.stats);
            assert_eq!(plain.output, result.output);
            assert_eq!(
                plain.counterexample.as_ref().map(|t| t
                    .states
                    .iter()
                    .map(|s| &s.values)
                    .collect::<Vec<_>>()),
                result.counterexample.as_ref().map(|t| t
                    .states
                    .iter()
                    .map(|s| &s.values)
                    .collect::<Vec<_>>())
            );
            assert_eq!(fault.calls, 0);
        }
    }
}

#[test]
fn unsupported_observation_width_runs_no_callback() {
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let m = model("Init == x=0\nNext == x'=x", "INIT Init\nNEXT Next\n", &opts);
    let mut events = Events::default();
    assert!(
        matches!(observed(&m,&opts,Limits::default(),&mut events),Exploration::Stopped(result) if matches!(result.verdict,Verdict::Unsupported { .. }))
    );
    assert!(events.roots.is_empty() && events.edges.is_empty() && events.levels.is_empty());
}

struct StopLevel {
    events: Events,
    fail: bool,
}
impl Observer for StopLevel {
    fn level_done(
        &mut self,
        level: u64,
        graph: &BehaviorGraph,
        stats: Stats,
    ) -> Result<(), String> {
        self.events.level_done(level, graph, stats)?;
        if self.fail {
            Err("controlled level failure".into())
        } else {
            Ok(())
        }
    }
    fn stop_after_level(&self) -> bool {
        !self.events.levels.is_empty()
    }
}

#[test]
fn controlled_prefix_is_committed_joined_and_never_complete_at_all_widths() {
    for mut opts in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        for width in [1, 4, 8] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                opts.workers = width;
                opts.store = store;
                let m = model(
                    "Init == x \\in 0..3\nNext == x'=x+4",
                    "INIT Init\nNEXT Next\n",
                    &opts,
                );
                let mut observer = StopLevel {
                    events: Events::default(),
                    fail: false,
                };
                let result = capture::explore_controlled_with_evidence(
                    &m,
                    &opts,
                    Limits::default(),
                    &mut observer,
                )
                .expect("capture thread");
                let capture::ControlledExploration::Prefix(prefix) = result else {
                    panic!("must stop at real prefix: {opts:?}: {result:?}")
                };
                assert_eq!(prefix.level(), 1);
                assert_eq!(
                    (
                        prefix.stats.generated,
                        prefix.stats.distinct,
                        prefix.stats.depth,
                        prefix.stats.left_on_queue
                    ),
                    (8, 8, 2, 4)
                );
                assert_eq!(prefix.workers.requested, width);
                assert_eq!(prefix.workers.used, width);
                assert_eq!(prefix.options.seed, 7);
                assert_eq!(observer.events.levels.len(), 1);
                assert!(prefix.graph.edges[..4].iter().all(|row| row.len() == 2));
                assert!(prefix.graph.edges[4..].iter().all(Vec::is_empty));
                assert!(prefix.graph.concrete[4..].iter().all(Vec::is_empty));
                assert_eq!(prefix.collision.is_some(), store == StoreMode::Fingerprint);
            }
        }
    }
}

#[test]
fn controlled_callback_error_and_failed_partial_level_never_become_prefixes() {
    for width in [1, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::dive(0)
        };
        let m = model(
            "Init == x \\in 0..3\nNext == x'=x+4",
            "INIT Init\nNEXT Next\n",
            &opts,
        );
        let mut observer = StopLevel {
            events: Events::default(),
            fail: true,
        };
        let result =
            capture::explore_controlled_with_evidence(&m, &opts, Limits::default(), &mut observer)
                .expect("thread");
        assert!(
            matches!(result, capture::ControlledExploration::Stopped(stop) if matches!(stop.verdict, Verdict::Unsupported { ref feature } if feature.contains("controlled level failure")))
        );
        assert_eq!(observer.events.levels.len(), 1);
        let m = model(
            "Init == x \\in 0..3\nNext == x'=x+4\nSafe == x<4",
            "INIT Init\nNEXT Next\nINVARIANT Safe\n",
            &opts,
        );
        let mut observer = StopLevel {
            events: Events::default(),
            fail: false,
        };
        let result =
            capture::explore_controlled_with_evidence(&m, &opts, Limits::default(), &mut observer)
                .expect("thread");
        assert!(
            matches!(result, capture::ControlledExploration::Stopped(stop) if matches!(stop.verdict, Verdict::InvariantViolated { .. }))
        );
        assert!(observer.events.levels.is_empty());
    }
}

#[test]
fn controlled_prefix_skips_postconditions_and_legacy_entrypoint_cannot_publish_it() {
    let opts = CheckOptions {
        sequential: true,
        ..CheckOptions::dive(0)
    };
    let m = model(
        "Init == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nPost == FALSE",
        "INIT Init\nNEXT Next\nPOSTCONDITION Post\n",
        &opts,
    );
    let mut observer = StopLevel {
        events: Events::default(),
        fail: false,
    };
    assert!(matches!(
        capture::explore_controlled_with_evidence(&m, &opts, Limits::default(), &mut observer)
            .expect("thread"),
        capture::ControlledExploration::Prefix(_)
    ));
    let mut observer = StopLevel {
        events: Events::default(),
        fail: false,
    };
    assert!(
        matches!(observed(&m, &opts, Limits::default(), &mut observer), Exploration::Stopped(stop) if matches!(stop.verdict, Verdict::Unsupported { .. }))
    );
    let result = capture::explore(&m, &opts, Limits::default());
    assert!(
        matches!(&result, Exploration::Stopped(stop) if matches!(&stop.verdict, Verdict::ModelError { code: 1000, message, .. } if message.contains("The post condition Post cannot be a constant."))),
        "{result:?}"
    );
}
