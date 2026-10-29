//! The core's behavior graph equals the reference capture's
//! (`docs/developer/core/11-liveness.md` §8), at every worker count and in
//! both profiles.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::sync::Arc;

use dive_check::CheckOptions;
use dive_check::capture::{self, BehaviorGraph};
use dive_core::node::{Kind, flags};
use dive_explore::graph::{GraphSink, Step, Stopped};

use super::Graph;

/// Specifications (body, configuration) with temporal properties.
const SPECS: &[(&str, &str)] = &[
    // Two actions, seen states and a stutter-only end.
    (
        "VARIABLES x, y\nInit == x = 0 /\\ y = 0\nA == x < 3 /\\ x' = x + 1 /\\ y' = y\nB == y' = 1 - y /\\ x' = x\nNext == A \\/ B\nP == <>(x = 3)",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    ),
    // Several initial states, a duplicate within a level and compound values.
    (
        "VARIABLES s, f\nInit == s \\in SUBSET {1, 2} /\\ f = [i \\in {1, 2} |-> 0]\nNext == \\E i \\in {1, 2} : s' = s \\cup {i} /\\ f' = [f EXCEPT ![i] = (@ + 1) % 2]\nP == []<>(s = {1, 2})",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    ),
    // A state constraint and an action constraint: states outside the model
    // are not vertices.
    (
        "VARIABLES x, r\nInit == x = 0 /\\ r = [a |-> 0, b |-> <<>>]\nNext == \\/ x' = x + 1 /\\ r' = [r EXCEPT !.a = x]\n        \\/ x' = x /\\ r' = [r EXCEPT !.b = Append(@, x)]\nC == x < 4 /\\ Len(r.b) < 3\nAC == x' # 2 \\/ x = 2\nP == <>[](x = 3)",
        "INIT Init\nNEXT Next\nCONSTRAINT C\nACTION_CONSTRAINT AC\nPROPERTY P\n",
    ),
    // Wide levels (several parallel chunks) with fairness.
    (
        "VARIABLES a, b\nInit == a = 0 /\\ b = 0\nIncA == a' = (a + 1) % 150 /\\ b' = b\nIncB == b' = (b + 7) % 160 /\\ a' = a\nSpec == Init /\\ [][IncA \\/ IncB]_<<a, b>> /\\ WF_<<a, b>>(IncA)\nP == []<>(a = 0)",
        "SPECIFICATION Spec\nPROPERTY P\n",
    ),
];

fn ir(body: &str) -> Arc<dive_ir::Ir> {
    let source = format!("---- MODULE G ----\nEXTENDS Naturals, Sequences\n{body}\n====\n");
    let analysis = dive_sema::analyze_source("G.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    Arc::new(dive_ir::lower(&analysis).expect("IR"))
}

fn reference(
    ir: &Arc<dive_ir::Ir>,
    cfg: &dive_config::ModelConfig,
    options: &CheckOptions,
) -> BehaviorGraph {
    let model = dive_model::bind(
        Arc::clone(ir),
        cfg,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )
    .expect("model");
    match capture::explore(&model, options, capture::Limits::default()) {
        capture::Exploration::Complete(c) => c.graph,
        other @ capture::Exploration::Stopped(_) => {
            panic!("reference capture did not complete: {other:?}")
        }
    }
}

fn core(
    ir: &Arc<dive_ir::Ir>,
    cfg: &dive_config::ModelConfig,
    options: &CheckOptions,
    profile_tlc: bool,
) -> BehaviorGraph {
    let opts = dive_core::Opts::default();
    let prepared = crate::core_engine::prepare(ir, cfg, options, profile_tlc, opts, true)
        .unwrap_or_else(|r| panic!("core refused: {:?}", r.verdict));
    let mut graph = Graph::new(&prepared.model, capture::Limits::default(), None);
    let poll = || false;
    let out = dive_explore::explore_with(
        &prepared.plan,
        &prepared.store,
        dive_explore::Config {
            opts,
            max_states: None,
            max_depth: None,
            workers: options.workers,
            warn: options.eval.warnings,
            sample: None,
        },
        &poll,
        Some(&mut graph),
    );
    assert_eq!(
        out.verdict,
        dive_explore::Verdict::Success,
        "{:?}",
        graph.take_error()
    );
    let (builder, metrics) = graph.finish();
    assert!(metrics.converted > 0);
    builder.graph().clone()
}

fn same(a: &BehaviorGraph, b: &BehaviorGraph, what: &str) {
    assert_eq!(a.roots, b.roots, "{what}: roots");
    assert_eq!(a.states, b.states, "{what}: states");
    assert_eq!(a.edges, b.edges, "{what}: edges");
    let rows = |g: &BehaviorGraph| {
        g.concrete
            .iter()
            .map(|r| {
                r.iter()
                    .map(|c| (c.state.clone(), c.ordinal, c.label.clone()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(a), rows(b), "{what}: concrete rows");
}

#[test]
fn the_core_graph_is_the_reference_capture() {
    for (body, cfg) in SPECS {
        let ir = ir(body);
        let cfg = dive_config::parse(cfg).into_config().expect("config");
        for profile_tlc in [false, true] {
            let base = if profile_tlc {
                CheckOptions::tlc(0, 0)
            } else {
                CheckOptions::dive(0)
            };
            let expected = reference(&ir, &cfg, &base);
            assert!(expected.states.len() > 3, "{body}");
            for workers in [1, 2, 3, 8] {
                let options = CheckOptions {
                    workers,
                    ..base.clone()
                };
                let got = core(&ir, &cfg, &options, profile_tlc);
                same(
                    &got,
                    &expected,
                    &format!("{body} tlc={profile_tlc} w={workers}"),
                );
            }
        }
    }
}

const ALIASES: &str = "VARIABLES k, x\n\
Init == k \\in 0..15 /\\ x = <<0>>\n\
A == k' = k /\\ x' = <<2>>\n\
Rejected == k' = k /\\ x' = <<9>>\n\
B == k' = k /\\ x' = [i \\in 1..1 |-> 4]\n\
Again == k' = k /\\ x' = <<2>>\n\
Literal == k' = k /\\ x' = <<4>>\n\
Next == A \\/ Rejected \\/ B \\/ Again \\/ Literal\n\
C == x[1] # 9\nV == k";
const ALIASES_CFG: &str = "INIT Init\nNEXT Next\nCONSTRAINT C\nVIEW V\n";

struct Originals<'a> {
    graph: Graph<'a>,
    /// (source id, ordinal, action, tuple-literal bit) for accepted originals.
    rows: Vec<(u64, u64, usize, bool)>,
    variable: usize,
}

impl GraphSink for Originals<'_> {
    fn max_expansion_units(&self) -> usize {
        self.graph.max_expansion_units()
    }

    fn max_successors_per_unit(&self) -> u64 {
        self.graph.max_successors_per_unit()
    }

    fn emitted(&mut self, generated: u64, ordinal: Option<u64>) -> Result<(), Stopped> {
        self.graph.emitted(generated, ordinal)
    }

    fn state(&mut self, heap: &mut dive_core::Heap, step: Step<'_>) -> Result<(), Stopped> {
        let original = step.original[self.variable];
        assert!(
            !original.is_transient(),
            "original outlives the worker arena"
        );
        assert_eq!(heap.view(original).kind, Kind::IntFcn);
        let tuple = heap.view(original).flags & flags::TUPLE != 0;
        if let Some(parent) = step.parent {
            self.rows
                .push((parent, step.ordinal, step.action.expect("action"), tuple));
        } else {
            assert!(tuple, "initial literal keeps its representation");
            let (_, identities) = step.new.expect("new representative");
            assert_eq!(heap.view(identities[self.variable]).flags & flags::VIEW, 0);
        }
        self.graph.state(heap, step)
    }

    fn stutter(&mut self, source: u64) -> Result<(), Stopped> {
        self.graph.stutter(source)
    }

    fn level_done(&mut self, level: u64, stats: dive_explore::Stats) -> Result<bool, Stopped> {
        self.graph.level_done(level, stats)
    }
}

fn prepared_aliases(tlc: bool, opts: dive_core::Opts) -> crate::core_engine::Prepared {
    let options = if tlc {
        CheckOptions::tlc(0, 0)
    } else {
        CheckOptions::dive(0)
    };
    let cfg = dive_config::parse(ALIASES_CFG)
        .into_config()
        .expect("config");
    crate::core_engine::prepare(&ir(ALIASES), &cfg, &options, tlc, opts, false)
        .unwrap_or_else(|r| panic!("core refused: {:?}", r.verdict))
}

fn config(opts: dive_core::Opts, workers: usize) -> dive_explore::Config {
    dive_explore::Config {
        opts,
        workers,
        warn: true,
        max_states: None,
        max_depth: None,
        sample: None,
    }
}

#[test]
fn seen_targets_keep_original_values_ordinals_and_representations() {
    for tlc in [false, true] {
        for hashcons in [false, true] {
            for early_probe in [false, true] {
                let opts = dive_core::Opts {
                    hashcons,
                    early_probe,
                    fold: false,
                    ..dive_core::Opts::default()
                };
                for workers in [1, 2, 3, 8] {
                    let prepared = prepared_aliases(tlc, opts);
                    let variable = prepared.plan.vars.iter().position(|v| v == "x").expect("x");
                    let mut sink = Originals {
                        graph: Graph::new(
                            &prepared.model,
                            capture::Limits {
                                states: 16,
                                ..capture::Limits::default()
                            },
                            None,
                        ),
                        rows: vec![],
                        variable,
                    };
                    let out = dive_explore::explore_with(
                        &prepared.plan,
                        &prepared.store,
                        config(opts, workers),
                        &|| false,
                        Some(&mut sink),
                    );
                    assert_eq!(out.verdict, dive_explore::Verdict::Success);
                    assert_eq!((out.stats.generated, out.stats.distinct), (96, 16));
                    let (builder, metrics) = sink.graph.finish();
                    assert!(
                        metrics.memo_entries <= 16 * 2,
                        "original conversion must not grow the cache past its state/variable allowance"
                    );
                    let graph = builder.graph();
                    assert_eq!(graph.states.len(), 16);
                    let expected = [2, 4, 2, 4]
                        .map(|x| Some(dive_value::Value::tuple(vec![dive_value::Value::Int(x)])));
                    for (source, row) in graph.concrete.iter().enumerate() {
                        assert_eq!(row.len(), 4);
                        let values: Vec<_> =
                            row.iter().map(|c| c.state[variable].clone()).collect();
                        assert_eq!(
                            values, expected,
                            "tlc={tlc} hc={hashcons} ep={early_probe} w={workers}"
                        );
                        for (target, literal) in row.iter().zip([true, false, true, true]) {
                            assert_eq!(
                                target.state[variable]
                                    .as_ref()
                                    .expect("assigned")
                                    .as_fcn()
                                    .expect("function")
                                    .is_tuple_literal(),
                                literal,
                                "the converted value retains the tuple/function class too"
                            );
                        }
                        assert!(
                            row.iter().all(|c| c.state != graph.states[source]),
                            "representative substitution must fail"
                        );
                        assert_eq!(
                            sink.rows[source * 4..source * 4 + 4],
                            [
                                (source as u64, 0, 0, true),
                                (source as u64, 2, 2, false),
                                (source as u64, 3, 3, true),
                                (source as u64, 4, 4, true),
                            ],
                            "tlc={tlc} hc={hashcons} ep={early_probe} w={workers}"
                        );
                        assert_eq!(graph.edges[source].last().expect("stutter").ordinal, None);
                    }
                    assert_eq!(
                        out.metrics.store,
                        prepared.store.stats(),
                        "speculative originals stay accounted in the store"
                    );
                }
            }
        }
    }
}

#[test]
fn converted_new_and_seen_originals_preserve_checked_equality() {
    for nested in [false, true] {
        for imported in [false, true] {
            let wrap = |value: &str| {
                if nested {
                    format!("<<{value}>>")
                } else {
                    value.to_owned()
                }
            };
            let general = wrap("[i \\in {\"a\"} |-> 0]");
            let literal = wrap("[a |-> 0]");
            let literal_action = if imported {
                format!("\\E r \\in {{{literal}}} : k' = k /\\ x' = r")
            } else {
                format!("k' = k /\\ x' = {literal}")
            };
            let body = format!(
                "VARIABLES k, x\nInit == k \\in 0..3 /\\ x = {general}\nA == k' = k /\\ x' = {general}\nB == {literal_action}\nNext == A \\/ B\nV == k"
            );
            let cfg = dive_config::parse("INIT Init\nNEXT Next\nVIEW V\n")
                .into_config()
                .expect("config");
            for tlc in [false, true] {
                let options = if tlc {
                    CheckOptions::tlc(0, 0)
                } else {
                    CheckOptions::dive(0)
                };
                for workers in [1, 2, 3, 8] {
                    let opts = dive_core::Opts {
                        fold: false,
                        ..dive_core::Opts::default()
                    };
                    let prepared =
                        crate::core_engine::prepare(&ir(&body), &cfg, &options, tlc, opts, false)
                            .unwrap_or_else(|r| panic!("core refused: {:?}", r.verdict));
                    let variable = prepared.plan.vars.iter().position(|v| v == "x").expect("x");
                    let mut graph = Graph::new(&prepared.model, capture::Limits::default(), None);
                    let out = dive_explore::explore_with(
                        &prepared.plan,
                        &prepared.store,
                        config(opts, workers),
                        &|| false,
                        Some(&mut graph),
                    );
                    assert_eq!(out.verdict, dive_explore::Verdict::Success);
                    let (builder, _) = graph.finish();
                    let graph = builder.graph();
                    let tuple = dive_value::Value::tuple(vec![
                        dive_value::Value::Int(0),
                        dive_value::Value::Int(0),
                    ]);
                    let rhs = if nested {
                        dive_value::Value::tuple(vec![tuple])
                    } else {
                        tuple
                    };
                    for (source, row) in graph.concrete.iter().enumerate() {
                        let original = graph.states[source][variable].as_ref().expect("new state");
                        assert!(
                            !dive_value::tlc::equals(original, &rhs, options.eval.ctx)
                                .expect("general function differs")
                        );
                        assert_eq!(row.len(), 2);
                        assert!(
                            !dive_value::tlc::equals(
                                row[0].state[variable].as_ref().expect("seen general"),
                                &rhs,
                                options.eval.ctx
                            )
                            .expect("seen general function differs")
                        );
                        assert!(
                            dive_value::tlc::equals(
                                row[1].state[variable].as_ref().expect("seen literal"),
                                &rhs,
                                options.eval.ctx
                            )
                            .is_err(),
                            "nested={nested} tlc={tlc} w={workers}: literal record must remain incomparable"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn capture_scratch_limit_stops_before_retaining_the_overflow_target() {
    for workers in [1, 2, 3, 8] {
        let opts = dive_core::Opts {
            hashcons: false,
            fold: false,
            ..dive_core::Opts::default()
        };
        let prepared = prepared_aliases(false, opts);
        let initial = prepared.store.stats();
        let mut graph = Graph::new(
            &prepared.model,
            capture::Limits {
                successors_per_unit: 2,
                ..capture::Limits::default()
            },
            None,
        );
        let out = dive_explore::explore_with(
            &prepared.plan,
            &prepared.store,
            config(opts, workers),
            &|| false,
            Some(&mut graph),
        );
        assert_eq!(out.verdict, dive_explore::Verdict::GraphStop);
        assert!(format!("{:?}", graph.take_error()).contains("successors per source"));
        let (builder, _) = graph.finish();
        assert_eq!(
            builder.graph().concrete.iter().map(Vec::len).sum::<usize>(),
            1
        );
        assert_eq!(
            builder.graph().edges.iter().map(Vec::len).sum::<usize>(),
            1,
            "no stutter/completion for the interrupted source"
        );
        assert_eq!(out.metrics.store, prepared.store.stats());
        // Each initial tuple has an identity and exact original (32 nodes).
        // At most eight sources retain two originals plus the VIEW-normalized
        // identity of their first accepted tuple each.
        // The overflow emission constructs a transient function, but must not
        // intern it. Speculative rows remain in Store even after wave discard.
        assert!(
            out.metrics.store.nodes - initial.nodes <= 32 + capture::MAX_EXPANSION_UNITS as u64 * 3
        );
    }
}

#[test]
fn ordinary_safety_keeps_duplicates_uninterned() {
    for workers in [1, 2, 3, 8] {
        let opts = dive_core::Opts {
            hashcons: false,
            fold: false,
            ..dive_core::Opts::default()
        };
        let body = "VARIABLES k, x\nInit == k \\in 0..15 /\\ x = <<0>>\nNext == k' = k /\\ (x' = <<0>> \\/ x' = <<0>> \\/ x' = <<0>> \\/ x' = <<0>>)";
        let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
            .into_config()
            .expect("config");
        let prepared = crate::core_engine::prepare(
            &ir(body),
            &cfg,
            &CheckOptions::dive(0),
            false,
            opts,
            false,
        )
        .unwrap_or_else(|r| panic!("core refused: {:?}", r.verdict));
        let initial = prepared.store.stats();
        let out = dive_explore::explore(
            &prepared.plan,
            &prepared.store,
            config(opts, workers),
            &|| false,
        );
        assert_eq!(out.verdict, dive_explore::Verdict::Success);
        assert_eq!(out.metrics.discarded_before_intern, 64);
        assert_eq!(
            out.metrics.store.nodes - initial.nodes,
            16,
            "only sixteen initial tuples are retained; no seen original is interned"
        );
    }
}

#[test]
fn capture_polls_the_run_budget_before_retaining_each_original() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    for workers in [1, 2, 3, 8] {
        let opts = dive_core::Opts {
            hashcons: false,
            fold: false,
            ..dive_core::Opts::default()
        };
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nVIEW V\n")
            .into_config()
            .expect("config");
        let prepared = crate::core_engine::prepare(&ir("VARIABLE x\nInit == x = <<0>>\nNext == x' = <<1>> \\/ x' = <<2>> \\/ x' = <<3>>\nV == 0"), &cfg, &CheckOptions::dive(0), false, opts, false)
        .unwrap_or_else(|r| panic!("core refused: {:?}", r.verdict));
        let polls = AtomicUsize::new(0);
        // Init, before source expansion, then each original. One source makes
        // these calls ordered at every width, including inside the worker pool.
        let poll = || polls.fetch_add(1, Ordering::Relaxed) >= 3;
        let mut graph = Graph::new(&prepared.model, capture::Limits::default(), None);
        let out = dive_explore::explore_with(
            &prepared.plan,
            &prepared.store,
            config(opts, workers),
            &poll,
            Some(&mut graph),
        );
        assert_eq!(out.verdict, dive_explore::Verdict::Budget);
        assert_eq!(
            out.stats.generated, 3,
            "initial, first original, payload-free budget stop"
        );
        assert!(
            graph.take_error().is_none(),
            "this is the armed run budget, not a capture refusal"
        );
        let (builder, _) = graph.finish();
        assert_eq!(builder.graph().concrete[0].len(), 1);
        assert_eq!(
            builder.graph().edges[0].len(),
            1,
            "no implicit stutter or complete source"
        );
    }
}
