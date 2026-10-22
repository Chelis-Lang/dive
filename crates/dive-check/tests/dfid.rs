//! DFID against clean primary and separately identified seeded-worker references.
#![allow(clippy::expect_used, clippy::panic, reason = "oracle control tests")]

use std::path::Path;
use std::sync::Arc;

use dive_check::CheckOptions;
use dive_check::dfid::{Options, Outcome, Report};
use serde_json::Value;

fn run(name: &str, options: &Options) -> Report {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dfid");
    let src = std::fs::read_to_string(root.join(format!("{name}.tla"))).expect("spec");
    let cfg = std::fs::read_to_string(root.join(format!("{name}.cfg"))).expect("cfg");
    let analysis =
        dive_sema::analyze_source(&format!("{name}.tla"), &src, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    let config = dive_config::parse(&cfg).into_config().expect("config");
    let model = dive_model::bind(ir, &config, &dive_model::BindOptions::tlc()).expect("bind");
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || dive_check::dfid::check(&model, options))
            .expect("thread")
            .join()
            .expect("result")
    })
}

fn code(outcome: &Outcome) -> u64 {
    match outcome {
        Outcome::Violation { code, .. } | Outcome::Error { code, .. } => u64::from(*code),
        Outcome::Complete | Outcome::Incomplete => 0,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn seeded_reference_matrix() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dfid");
    let rows: Value = serde_json::from_str(
        &std::fs::read_to_string(base.join("secondary-controls.json")).expect("reference"),
    )
    .expect("json");
    for row in rows["observations"].as_array().expect("rows") {
        let name = row["model"].as_str().expect("model");
        let depth = u32::try_from(row["depth"].as_u64().expect("depth")).expect("bounded");
        let seed = row["worker_seed"].as_i64().expect("worker seed");
        let mut opts = Options::new(CheckOptions::tlc(0, 7), depth);
        opts.worker_seeds = Some(vec![seed; depth as usize]);
        for store in [
            dive_store::StoreMode::Fingerprint,
            dive_store::StoreMode::Exact,
        ] {
            opts.check.store = store;
            let report = run(name, &opts);
            assert_eq!(
                code(&report.outcome),
                row["code"].as_u64().expect("deciding code"),
                "{} {store:?} {:?}",
                row["control"],
                report.outcome
            );
            if let Some(generated) = row["generated"].as_u64() {
                assert_eq!(report.generated, generated, "{} {store:?}", row["control"]);
                assert_eq!(
                    report.distinct as u64,
                    row["distinct"].as_u64().expect("distinct"),
                    "{} {store:?}",
                    row["control"]
                );
            }
            if !["DfidRootsMixed", "DfidSeedNames"].contains(&name)
                && matches!(report.outcome, Outcome::Violation { .. })
            {
                let trace: Vec<_> = report
                    .trace
                    .iter()
                    .map(|s| {
                        dive_value::print::to_tlc_string(
                            s[0].as_ref().expect("x"),
                            dive_value::Ctx::tlc(),
                        )
                        .expect("printed")
                    })
                    .collect();
                let oracle: Vec<_> = row["trace_x"]
                    .as_array()
                    .expect("trace")
                    .iter()
                    .map(ToString::to_string)
                    .collect();
                assert_eq!(trace, oracle, "{} {store:?}", row["control"]);
            }
            assert_eq!(
                report.tlc_completion_message,
                row["completion_message"]
                    .as_bool()
                    .expect("complete message"),
                "{} {store:?}",
                row["control"]
            );
        }
    }
}

#[test]
fn limits_never_prove_model() {
    for depth in 0..=6 {
        let report = run("DfidBound", &Options::new(CheckOptions::dive(7), depth));
        assert_eq!(report.outcome, Outcome::Incomplete);
    }
    let mut opts = Options::new(CheckOptions::tlc(0, 7), 6);
    opts.max_states = 0;
    assert!(matches!(run("DfidCycle", &opts).outcome, Outcome::Limit(_)));
    opts.max_states = 100;
    opts.max_generated = 1;
    assert!(matches!(run("DfidCycle", &opts).outcome, Outcome::Limit(_)));
    opts.max_generated = 1000;
    opts.fingerprint_bits = 1;
    assert!(
        matches!(run("DfidCycle", &opts).outcome, Outcome::Limit(s) if s.contains("collision"))
    );
    opts.fingerprint_bits = 64;
    opts.max_trace = 0;
    assert!(matches!(
        run("DfidBranch", &opts).outcome,
        Outcome::Limit(_)
    ));
}

#[test]
fn default_replays_and_worker_constraints_refuse() {
    let opts = Options::new(CheckOptions::dive(7), 6);
    let a = run("DfidBranch", &opts);
    let b = run("DfidBranch", &opts);
    assert_eq!(a.worker_seeds, b.worker_seeds);
    assert_eq!(a.trace, b.trace);
    assert_eq!((a.generated, a.distinct), (b.generated, b.distinct));
    let mut opts = opts;
    opts.check.workers = 2;
    assert!(
        matches!(run("DfidCycle", &opts).outcome, Outcome::Unsupported(s) if s.contains("multiple workers"))
    );
    opts.check.workers = 1;
    assert!(matches!(
        run("DfidLiveGet", &opts).outcome,
        Outcome::Unsupported(_)
    ));
    assert!(matches!(
        run("DfidRedirectedGet", &opts).outcome,
        Outcome::Unsupported(_)
    ));
    assert!(matches!(
        run("DfidLive", &opts).outcome,
        Outcome::Unsupported(_)
    ));
    assert!(
        matches!(run("DfidActionConstrained", &opts).outcome, Outcome::Unsupported(s) if s.contains("validation contract"))
    );
}

#[test]
fn resource_sweep_keeps_partial_evidence_and_no_false_completion() {
    for name in ["DfidBound", "DfidCycle", "DfidBranch", "DfidRootsMixed"] {
        for depth in 0..=4 {
            for cap in [0, 1, 2, 3, 5] {
                for generated in [0, 1, 2, 4, 9] {
                    for trace_cap in [0, 1, 2, 4] {
                        let mut opts = Options::new(CheckOptions::tlc(0, 7), depth);
                        opts.worker_seeds = Some(vec![0; depth as usize]);
                        opts.max_states = cap;
                        opts.max_retained_candidates = cap;
                        opts.max_generated = generated;
                        opts.max_trace = trace_cap;
                        let report = run(name, &opts);
                        assert!(report.generated <= generated);
                        assert!(report.initial_candidates <= generated);
                        assert!(report.distinct <= cap);
                        assert!(report.trace.len() <= trace_cap);
                        if name == "DfidBound" {
                            assert_ne!(report.outcome, Outcome::Complete);
                        }
                        if matches!(report.outcome, Outcome::Limit(_)) {
                            assert_ne!(report.stage, "complete");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn captured_fixture_integrity() {
    for name in ["verify.py", "test-comparison.py"] {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/dfid")
            .join(name);
        assert!(
            std::process::Command::new("python3")
                .arg("-I")
                .arg("-B")
                .arg(script)
                .status()
                .expect("fixture verifier / comparison controls")
                .success()
        );
    }
}
#[test]
fn captured_effects_and_malformed_expressions_refuse_before_dfid() {
    let src = r"---- MODULE Graph ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Q(P(_)) == P(x)
R(y) == y
====
";
    let analysis = dive_sema::analyze_source("Graph.tla", src, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("cfg");
    let mut m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    let d = m.ir.root_def("Q").expect("Q");
    let scope = m
        .evaluator
        .scope_apply(
            &m.evaluator.scope(d).expect("valid scope"),
            d,
            &[dive_ir::Arg::Op {
                op: dive_ir::OpRef::Std(dive_ir::StdOp::TlcPrintT),
                level: dive_ir::Level::Constant,
            }],
        )
        .expect("captured operator scope");
    m.invariants.push(dive_model::Named {
        name: "Q(PrintT)".into(),
        pred: dive_model::Pred::new(m.ir.def(d).body, scope),
    });
    let report = dive_check::dfid::check(&m, &Options::new(CheckOptions::tlc(0, 7), 6));
    assert!(matches!(report.outcome, Outcome::Unsupported(_)));
    assert!(
        m.evaluator.take_output().is_empty(),
        "preflight ran hidden PrintT"
    );
    let predicate = m.invariants.remove(0).pred;
    m.actions = vec![
        m.evaluator
            .predicate_in(
                &predicate.scope,
                predicate.expr,
                dive_ir::actions::ActionName::Def(d),
                1,
            )
            .expect("valid owned action"),
    ];
    let report = dive_check::dfid::check(&m, &Options::new(CheckOptions::tlc(0, 7), 6));
    assert!(matches!(report.outcome, Outcome::Unsupported(_)));
    assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
    m.actions.clear();
    m.invariants.push(dive_model::Named {
        name: "invalid captured expression".into(),
        pred: dive_model::Pred::new(
            dive_ir::ExprId(u32::MAX),
            m.evaluator.scope(d).expect("valid scope"),
        ),
    });
    let report = dive_check::dfid::check(&m, &Options::new(CheckOptions::tlc(0, 7), 6));
    assert!(matches!(report.outcome, Outcome::Unsupported(_)));

    let r = m.ir.root_def("R").expect("R");
    let error = m
        .evaluator
        .scope_apply(
            &m.evaluator.scope(r).expect("valid scope"),
            r,
            &[dive_ir::Arg::Expr {
                expr: dive_ir::ExprId(u32::MAX),
                pass: dive_ir::Pass::Lazy,
            }],
        )
        .expect_err("malformed lazy capture rejected before DFID");
    assert!(error.message().contains("DIVE-EVAL-C001"));
    assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
}

#[test]
fn pure_operator_scopes_supplied_by_rust_callers_complete_dfid() {
    let source = "---- MODULE PureCaptured ----\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nQ(P(_)) == P(x)\nZero(y) == y = 0\n====\n";
    for check in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        let analysis =
            dive_sema::analyze_source("PureCaptured.tla", source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let config = dive_config::parse("INIT Init\nNEXT Next\n")
            .into_config()
            .expect("config");
        let mut m = dive_model::bind(
            ir,
            &config,
            &dive_model::BindOptions {
                eval: check.eval,
                symmetry_liveness: check.symmetry_liveness,
                no_deadlock: check.no_deadlock,
                host: None,
            },
        )
        .expect("model");
        let q = m.ir.root_def("Q").expect("Q");
        let zero = m.ir.root_def("Zero").expect("Zero");
        let scope = m
            .evaluator
            .scope_apply(
                &m.evaluator.scope(q).expect("owned scope"),
                q,
                &[dive_ir::Arg::Op {
                    op: dive_ir::OpRef::Def(zero),
                    level: dive_ir::Level::Constant,
                }],
            )
            .expect("pure captured operator scope");
        assert!(!m.evaluator.scope_has_effects(&scope));
        m.invariants.push(dive_model::Named {
            name: "Q(Zero)".into(),
            pred: dive_model::Pred::new(m.ir.def(q).body, scope),
        });
        let report = dive_check::dfid::check(&m, &Options::new(check, 6));
        assert!(
            matches!(report.outcome, Outcome::Complete),
            "{:?}",
            report.outcome
        );
        assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
    }
}

#[test]
fn bfs_discovery_budgets_are_never_silently_ignored_by_dfid() {
    for base in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        for (max_states, max_depth) in [(Some(0), None), (None, Some(0))] {
            let opts = Options::new(
                CheckOptions {
                    max_states,
                    max_depth,
                    ..base.clone()
                },
                6,
            );
            let result = run("DfidCycle", &opts);
            assert!(
                matches!(result.outcome, Outcome::Unsupported(ref s) if s.contains("BFS discovery budgets")),
                "{result:?}"
            );
            assert_eq!(result.generated, 0);
            assert_eq!(result.distinct, 0);
        }
    }
}
