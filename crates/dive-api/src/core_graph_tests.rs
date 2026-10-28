//! The core's behavior graph equals the reference capture's
//! (`docs/developer/core/11-liveness.md` §8), at every worker count and in
//! both profiles.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::sync::Arc;

use dive_check::CheckOptions;
use dive_check::capture::{self, BehaviorGraph};

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
