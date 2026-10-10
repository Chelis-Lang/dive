//! Graph capture shares the safety engine without claiming temporal truth.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_check::{
    CheckOptions, Verdict,
    capture::{self, Exploration, Limits},
};
use dive_store::StoreMode;
use dive_value::Value;
use std::sync::Arc;

fn model(source: &str, config: &str, opts: &CheckOptions) -> dive_model::Model {
    let name = source.split_whitespace().nth(2).expect("module");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
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
    .expect("bind")
}

fn complete(model: &dive_model::Model, opts: &CheckOptions) -> capture::Complete {
    match capture::explore(model, opts, Limits::default()) {
        Exploration::Complete(c) => *c,
        Exploration::Stopped(v) => panic!("{v:?}"),
    }
}

#[test]
fn duplicates_seen_edges_roots_and_stutters_retain_reference_order() {
    let source = "---- MODULE Capture ----\nVARIABLE x\nInit == x \\in {0,1}\n\
        Next == x' = 0 \\/ x' = 0 \\/ x' = 1\nProp == []<>(x = 0)\n====\n";
    for store in [StoreMode::Fingerprint, StoreMode::Exact] {
        let opts = CheckOptions {
            store,
            workers: 8,
            ..CheckOptions::tlc(0, 0)
        };
        let m = model(source, "INIT Init\nNEXT Next\nPROPERTY Prop\n", &opts);
        assert!(
            m.unsupported
                .iter()
                .any(|u| u.kind == dive_model::UnsupportedKind::TemporalProperty)
        );
        let c = complete(&m, &opts);
        assert_eq!(
            (c.stats.generated, c.stats.distinct, c.stats.depth),
            (8, 2, 1)
        );
        assert_eq!(c.graph.roots, [0, 1]);
        assert_eq!(c.graph.states[0][0], Some(Value::Int(0)));
        assert_eq!(c.graph.states[1][0], Some(Value::Int(1)));
        for (from, row) in c.graph.edges.iter().enumerate() {
            assert_eq!(
                row.iter().map(|e| e.to).collect::<Vec<_>>(),
                [0, 0, 1, from]
            );
            assert_eq!(
                row.iter().map(|e| e.ordinal).collect::<Vec<_>>(),
                [Some(0), Some(1), Some(2), None]
            );
            assert!(row.last().expect("stutter").label.is_none());
        }
        assert_eq!(
            (c.workers.requested, c.workers.used, c.workers.sequential),
            (8, 1, true)
        );
        assert!(c.workers.reason.expect("reason").contains("capture"));
    }
}

#[test]
fn constraints_filter_generated_edges_without_filtering_implicit_stutters() {
    let opts = CheckOptions::tlc(0, 0);
    let m = model(
        "---- MODULE Capture ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
        Next == x' = (x + 1) % 3\nKeep == x < 2\nChanges == x' # x\n====\n",
        "INIT Init\nNEXT Next\nCONSTRAINT Keep\nACTION_CONSTRAINT Changes\n",
        &opts,
    );
    let c = complete(&m, &opts);
    assert_eq!(
        (c.stats.generated, c.stats.distinct, c.stats.depth),
        (3, 2, 2)
    );
    assert_eq!(
        c.graph.edges[0].iter().map(|e| e.to).collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(
        c.graph.edges[1].iter().map(|e| e.to).collect::<Vec<_>>(),
        [1]
    );
    assert!(c.graph.edges[1][0].ordinal.is_none());
}

#[test]
fn no_safety_stop_or_resource_bound_exposes_a_complete_graph() {
    let opts = CheckOptions::tlc(0, 0);
    let source = "---- MODULE Capture ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nInv == ~x\n====\n";
    let m = model(source, "INIT Init\nNEXT Next\nINVARIANT Inv\n", &opts);
    assert!(matches!(
        capture::explore(&m, &opts, Limits::default()),
        Exploration::Stopped(r) if matches!(r.verdict, Verdict::InvariantViolated { .. })
    ));
    for limits in [
        Limits {
            generated: 0,
            ..Limits::default()
        },
        Limits {
            states: 0,
            ..Limits::default()
        },
        Limits {
            edges: 0,
            ..Limits::default()
        },
    ] {
        let m = model(source, "INIT Init\nNEXT Next\n", &opts);
        assert!(matches!(
            capture::explore(&m, &opts, limits),
            Exploration::Stopped(r) if matches!(r.verdict, Verdict::ResourceLimit { .. })
        ));
    }
    let dead = model(
        "---- MODULE Capture ----\nVARIABLE x\nInit == x = TRUE\nNext == ~x /\\ x' = TRUE\n====\n",
        "INIT Init\nNEXT Next\n",
        &opts,
    );
    assert!(matches!(
        capture::explore(&dead, &opts, Limits::default()),
        Exploration::Stopped(r) if r.verdict == Verdict::Deadlock
    ));
}

#[test]
fn every_other_unsupported_reason_and_profile_mismatch_is_retained() {
    let opts = CheckOptions::tlc(0, 0);
    let mut m = model(
        "---- MODULE Capture ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\n====\n",
        "INIT Init\nNEXT Next\n",
        &opts,
    );
    // A matching-looking message is not a temporal capability certificate.
    m.unsupported.push(dive_model::Unsupported {
        feature: "temporal properties (liveness checking): Fake".into(),
        kind: dive_model::UnsupportedKind::Feature,
    });
    assert!(
        matches!(capture::explore(&m,&opts,Limits::default()),Exploration::Stopped(r) if matches!(&r.verdict, Verdict::Unsupported { feature } if feature.contains("Fake")))
    );
    assert!(
        matches!(capture::explore(&m,&CheckOptions::dive(0),Limits::default()),Exploration::Stopped(r) if matches!(&r.verdict, Verdict::Unsupported { feature } if feature.contains("profile mismatch")))
    );
}

#[test]
fn failing_assumptions_keep_prints_and_completed_safety_stops_keep_all_evidence() {
    let opts = CheckOptions::tlc(0, 17);
    for source in [
        "---- MODULE Capture ----\nEXTENDS TLC\nVARIABLE x\nASSUME PrintT(\"before-failure\") /\\ FALSE\nInit == x = TRUE\nNext == x' = x\n====\n",
        "---- MODULE Capture ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nInv == ~x\n====\n",
    ] {
        let cfg_text = if source.contains("Inv ==") {
            "INIT Init\nNEXT Next\nINVARIANT Inv\n"
        } else {
            "INIT Init\nNEXT Next\n"
        };
        let m = model(source, cfg_text, &opts);
        let cfg = dive_config::parse(cfg_text).into_config().expect("cfg");
        let normal = dive_check::check(&m.ir, &cfg, &opts);
        let Exploration::Stopped(captured) = capture::explore(&m, &opts, Limits::default()) else {
            panic!("safety stop must retain evidence, never a complete graph");
        };
        assert_eq!(captured.verdict, normal.verdict);
        assert_eq!(captured.stats, normal.stats);
        assert_eq!(captured.tlc_prints_stats, normal.tlc_prints_stats);
        assert_eq!(captured.output, normal.output);
        assert_eq!(captured.warnings, normal.warnings);
        assert_eq!(captured.var_names, normal.var_names);
        assert_eq!(captured.tlc_var_order, normal.tlc_var_order);
        assert_eq!(captured.fp_index, normal.fp_index);
        assert!(captured.elapsed > std::time::Duration::ZERO);
        let states = |r: &dive_check::CheckResult| {
            r.counterexample.as_ref().map(|c| {
                c.states
                    .iter()
                    .map(|s| s.values.clone())
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(states(&captured), states(&normal));
        if source.contains("PrintT") {
            assert_ne!(captured.output, Vec::<String>::new());
        }
    }
}

#[test]
fn concrete_parallel_capture_retains_equal_nonrepresentative_values_and_real_width() {
    let source = "---- MODULE Concrete ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = <<1>>\nNext == x' = [i \\in 1..1 |-> i]\n====\n";
    for width in [1, 3, 4, 8] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            let opts = CheckOptions {
                workers: width,
                store,
                ..CheckOptions::tlc(0, 0)
            };
            let m = model(source, "INIT Init\nNEXT Next\n", &opts);
            let Exploration::Complete(c) =
                capture::explore_concrete_parallel_with_evidence(&m, &opts, Limits::default())
                    .expect("worker evidence")
            else {
                panic!("finite model must complete")
            };
            assert_eq!(c.workers.used, width);
            assert_eq!(c.workers.requested, width);
            assert!(!c.workers.sequential);
            assert_eq!((c.stats.generated, c.stats.distinct), (2, 1));
            let Value::Fcn(stored) = c.graph.states[0][0].as_ref().expect("stored") else {
                panic!("tuple")
            };
            let Value::Fcn(actual) = c.graph.concrete[0][0].state[0].as_ref().expect("actual")
            else {
                panic!("function")
            };
            assert!(stored.is_tuple_literal());
            assert!(!actual.is_tuple_literal());
            assert_eq!(c.graph.concrete[0][0].ordinal, 0);
            assert_eq!(c.graph.edges[0].len(), 2);
        }
    }
}

#[test]
fn concrete_parallel_source_rows_equal_inline_across_scheduling_and_fixed_bounds() {
    let source = "---- MODULE Concrete ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..7\nNext == x' = (x+1)%8 \\/ x' = x \\/ x' = (x+1)%8\nKeep == x < 6\nChange == x' # 4\n====\n";
    let config =
        "INIT Init\nNEXT Next\nCONSTRAINT Keep\nACTION_CONSTRAINT Change\nCHECK_DEADLOCK FALSE\n";
    let baseline_opts = CheckOptions::tlc(0, 0);
    let baseline = complete(&model(source, config, &baseline_opts), &baseline_opts);
    for width in [1, 3, 4, 8] {
        for chaos in [None, Some(7), Some(19)] {
            let opts = CheckOptions {
                workers: width,
                chaos,
                ..CheckOptions::tlc(0, 0)
            };
            let m = model(source, config, &opts);
            let Exploration::Complete(c) =
                capture::explore_concrete_parallel_with_evidence(&m, &opts, Limits::default())
                    .expect("worker evidence")
            else {
                panic!("finite model")
            };
            assert_eq!(c.graph.states, baseline.graph.states);
            assert_eq!(c.graph.roots, baseline.graph.roots);
            assert_eq!(c.graph.edges, baseline.graph.edges);
            assert_eq!(c.stats, baseline.stats);
            for (row, want) in c.graph.concrete.iter().zip(&baseline.graph.concrete) {
                assert_eq!(row.len(), want.len());
                for (got, want) in row.iter().zip(want) {
                    assert_eq!(got.state, want.state);
                    assert_eq!(got.ordinal, want.ordinal);
                    assert_eq!(got.label, want.label);
                }
            }
        }
    }
    for edges in 0..24 {
        let mut outcomes = Vec::new();
        for width in [1, 3, 4, 8] {
            let opts = CheckOptions {
                workers: width,
                ..CheckOptions::tlc(0, 0)
            };
            let m = model(source, config, &opts);
            let result = capture::explore_concrete_parallel_with_evidence(
                &m,
                &opts,
                Limits {
                    edges,
                    ..Limits::default()
                },
            )
            .expect("evidence");
            match result {
                Exploration::Stopped(r) => {
                    assert!(matches!(r.verdict, Verdict::ResourceLimit { .. }));
                    outcomes.push(false);
                }
                Exploration::Complete(c) => {
                    assert_eq!(c.graph.edges, baseline.graph.edges);
                    outcomes.push(true);
                }
            }
        }
        assert!(outcomes.windows(2).all(|p| p[0] == p[1]));
    }
}

#[test]
fn actual_captured_scopes_are_proved_before_any_pool_evaluation() {
    let src = r"---- MODULE Captured ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Q(P(_)) == P(x)
R(y) == y
====
";
    for width in [1, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 7)
        };
        let mut m = model(src, "INIT Init\nNEXT Next\n", &opts);
        let d = m.ir.root_def("Q").expect("Q");
        let sc = m
            .evaluator
            .scope_apply(
                &m.evaluator.scope(d),
                d,
                &[dive_ir::Arg::Op {
                    op: dive_ir::OpRef::Std(dive_ir::StdOp::TlcPrintT),
                    level: dive_ir::Level::Constant,
                }],
            )
            .expect("scope");
        m.invariants.push(dive_model::Named {
            name: "hidden PrintT".into(),
            pred: dive_model::Pred::new(m.ir.def(d).body, sc),
        });
        let refusal = |m: &dive_model::Model| {
            let r = capture::explore_concrete_parallel_with_evidence(m, &opts, Limits::default())
                .expect("no panic");
            assert!(
                matches!(r,Exploration::Stopped(ref r) if matches!(r.verdict,Verdict::Unsupported { .. }) && r.stats.is_none() && r.workers.used==0)
            );
            assert!(
                m.evaluator.take_output().is_empty(),
                "admission evaluated hidden PrintT"
            );
        };
        refusal(&m);
        let p = m.invariants.remove(0).pred;
        m.actions = vec![m.evaluator.predicate_in(
            &p.scope,
            p.expr,
            dive_ir::actions::ActionName::Def(d),
            1,
        )];
        refusal(&m);
        m.actions.clear();
        m.invariants.push(dive_model::Named {
            name: "bad ID".into(),
            pred: dive_model::Pred::new(dive_ir::ExprId(u32::MAX), m.evaluator.scope(d)),
        });
        refusal(&m);
        let r = m.ir.root_def("R").expect("R");
        let sc = m
            .evaluator
            .scope_apply(
                &m.evaluator.scope(r),
                r,
                &[dive_ir::Arg::Expr {
                    expr: dive_ir::ExprId(u32::MAX),
                    pass: dive_ir::Pass::Lazy,
                }],
            )
            .expect("capture does not force");
        m.invariants[0].pred = dive_model::Pred::new(m.ir.def(r).body, sc);
        refusal(&m);
    }
}

#[test]
fn speculative_capture_refuses_runtime_name_creation_before_workers() {
    for width in [1, 4, 8, 65] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let m = model(
            "---- MODULE Names ----\nEXTENDS TLC\nVARIABLE x\nInit == x = FALSE\nNext == x' = ToString(x)\n====\n",
            "INIT Init\nNEXT Next\n",
            &opts,
        );
        let r = capture::explore_concrete_parallel_with_evidence(&m, &opts, Limits::default())
            .expect("typed stop");
        assert!(
            matches!(r,Exploration::Stopped(ref r) if matches!(r.verdict,Verdict::Unsupported {..}|Verdict::ResourceLimit {..}) && r.workers.used==0 && r.stats.is_none())
        );
        assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
    }
}

#[test]
fn caller_replaced_ir_or_names_refuses_before_walk_setup_or_workers() {
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let source = "---- MODULE Owned ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\n====\n";
    for replace_ir in [true, false] {
        let mut m = model(source, "INIT Init\nNEXT Next\n", &opts);
        if replace_ir {
            m.ir = Arc::new(m.ir.as_ref().clone());
        } else {
            m.names = Arc::new(dive_value::Interner::new());
        }
        let r = capture::explore_concrete_parallel_with_evidence(&m, &opts, Limits::default())
            .expect("typed stop");
        assert!(
            matches!(r,Exploration::Stopped(ref r) if matches!(&r.verdict,Verdict::Unsupported {feature} if feature.contains("foreign IR or name table")) && r.workers.used==0 && r.stats.is_none())
        );
        assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
    }
}
