//! Nested ENABLED query placements keep their runtime and planner boundaries.
//! The primary verdicts, counts and words run on Core
//! (`dive-api/tests/liveness_nested_enabled.rs`).
#![allow(clippy::expect_used, clippy::panic, reason = "tiny primary assertions")]
use dive_runtime::CheckOptions;
use dive_trace::{Ending, TraceDoc, Validation};
use serde_json::Value;
use std::sync::Arc;
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "keep all runtime and planner boundaries in one parallel profile matrix"
)]
fn query_placements_preserve_each_runtime_and_planner_boundary() {
    use dive_model::Pred;
    for tlc in [true, false] {
        let options = if tlc {
            CheckOptions::tlc(0, 0)
        } else {
            CheckOptions::dive(0)
        };
        for (extra, action, runtime_supported) in [
            ("", "(ENABLED (x'=1))=TRUE /\\ x'=1", false),
            ("", "LET q==ENABLED (x'=1) IN q' /\\ x'=1", false),
            (
                "F(v)==ENABLED (x'=v)\nPass(H(_),v)==H(v)",
                "Pass(F,1) /\\ x'=1",
                false,
            ),
            (
                "",
                "SelectSeq(<<0>>,LAMBDA v: ENABLED (x'=v)) = << >> /\\ x'=1",
                false,
            ),
            ("", "ENABLED (LET held==x'=1 IN held) /\\ x'=1", true),
            (
                "",
                "LET f[t \\in {0}]==ENABLED (x'=1) IN f[0] /\\ x'=1",
                false,
            ),
        ] {
            let source = format!(
                "---- MODULE NestedEnabled ----\nEXTENDS Integers,Sequences\nVARIABLE x\nInit==x=0\nNext==x'=1\n{extra}\nA=={action}\nP==<>[][A]_x\n====\n"
            );
            let a = dive_sema::analyze_source(
                "NestedEnabled.tla",
                &source,
                &dive_sema::Options::default(),
            );
            assert!(a.accepted(), "{:?}", a.diagnostics());
            let model = dive_model::bind(
                Arc::new(dive_ir::lower(&a).expect("IR")),
                &dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
                    .into_config()
                    .expect("cfg"),
                &dive_model::BindOptions {
                    eval: options.eval,
                    ..dive_model::BindOptions::tlc()
                },
            )
            .expect("model");
            let role = |name| {
                let d = model.ir.root_def(name).expect("role");
                Pred::new(
                    model.ir.def(d).body,
                    model.evaluator.scope(d).expect("scope"),
                )
            };
            let action_pred = role("A");
            let property = role("P");
            let before: dive_trace::State = Arc::from([Some(0.into())]);
            assert!(
                dive_liveness::plan::lower(
                    &model,
                    &property,
                    dive_liveness::formula::Limits::default()
                )
                .is_err(),
                "planner {action}"
            );
            let word = dive_trace::temporal::evaluate_lasso(
                &model,
                &property,
                std::slice::from_ref(&before),
                0,
            );
            if runtime_supported {
                assert_eq!(word.expect("bounded runtime word"), vec![true]);
                assert_eq!(
                    dive_trace::enabled::enabled(&model, &action_pred, None, &before),
                    Ok(true),
                    "{action}"
                );
            } else {
                // The planner refuses these placements, so no checker word
                // reaches replay; replay decides them with the core evaluator.
                let _ = word;
            }
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "keep pinned truth, genuine word and both fabricated claims in one transport matrix"
)]
fn held_action_query_matches_primary_and_validates_its_own_words() {
    use dive_model::BindOptions;
    use dive_trace::{Check, Trace};
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/held-enabled");
    let primary: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture.join("primary.json")).expect("pinned primary facts"),
    )
    .expect("primary JSON");
    assert_eq!(primary["sany_exit"], 0);
    assert_eq!(primary["hold"]["code"], 2193);
    assert_eq!(primary["hold"]["exit"], 0);
    assert_eq!(primary["hold"]["generated"], 3);
    assert_eq!(primary["hold"]["distinct"], 2);
    assert_eq!(primary["hold"]["depth"], 2);
    assert_eq!(primary["fail"]["code"], 2107);
    assert_eq!(primary["fail"]["exit"], 12);
    assert_eq!(primary["fail"]["property"], "Bad");
    let initial = primary["fail"]["initial_x"]
        .as_i64()
        .expect("primary initial x");
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        let analysis = dive_sema::analyze_file(
            &fixture.join("HeldEnabled.tla"),
            &dive_sema::Options::default(),
        )
        .expect("primary source");
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let config = dive_config::parse(
            &std::fs::read_to_string(fixture.join("fail.cfg")).expect("original config"),
        )
        .into_config()
        .expect("configuration");
        let model = dive_model::bind(
            Arc::new(dive_ir::lower(&analysis).expect("IR")),
            &config,
            &options,
        )
        .expect("model");
        let genuine = Trace {
            states: vec![Arc::from([Some(initial.into())])],
            labels: vec![None],
            ending: Ending::Invariant("Bad".into()),
            length: Some(1),
            properties: Vec::new(),
        };
        let mut bad_init = genuine.clone();
        bad_init.states[0] = Arc::from([Some(99.into())]);
        let mut bad_ending = genuine.clone();
        bad_ending.ending = Ending::Invariant("Good".into());
        for (name, word) in [
            ("genuine", genuine),
            ("forged Init", bad_init),
            ("forged ending", bad_ending),
        ] {
            let typed = dive_trace::validate(&model, &word).validation;
            let document = word.to_doc(&model).expect("document");
            let parsed = TraceDoc::parse(&document.to_json().to_string()).expect("transport");
            let serialized = dive_trace::validate_doc(&model, &parsed, &config.model_values)
                .0
                .validation;
            for result in [typed, serialized] {
                match name {
                    "genuine" => assert_eq!(result, Validation::Valid),
                    "forged Init" => assert!(
                        matches!(result, Validation::Invalid(f) if f.state == 0 && f.check == Check::Init)
                    ),
                    _ => assert!(
                        matches!(result, Validation::Invalid(f) if f.state == 0 && f.check == Check::Ending)
                    ),
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct SubscriptHooks(std::sync::atomic::AtomicUsize);
impl dive_eval::Host for SubscriptHooks {
    fn check_native_access(&self, _: dive_ir::StdOp) -> Result<(), dive_eval::HostError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn prebound_nonstuttering_subscripts_keep_original_error_ordering() {
    use dive_eval::St;
    use dive_liveness::formula::AtomKind;
    use dive_liveness::plan::{AtomTest, PredicateAtom};
    use dive_model::Pred;
    for options in [dive_eval::Options::tlc(), dive_eval::Options::dive()] {
        let source = "---- MODULE SubscriptQueries ----\nEXTENDS Integers\nVARIABLE x\nInit==x=0\nNext==x'=1\nBody==1 \\div 0=0\nOrdinary==x\nBad==1 \\div 0\n====\n";
        let a = dive_sema::analyze_source(
            "SubscriptQueries.tla",
            source,
            &dive_sema::Options::default(),
        );
        assert!(a.accepted(), "{:?}", a.diagnostics());
        let model = dive_model::bind(
            Arc::new(dive_ir::lower(&a).expect("IR")),
            &dive_config::parse("INIT Init\nNEXT Next\n")
                .into_config()
                .expect("cfg"),
            &dive_model::BindOptions {
                eval: options,
                ..dive_model::BindOptions::tlc()
            },
        )
        .expect("model");
        let role = |name| {
            let d = model.ir.root_def(name).expect("role");
            Pred::new(
                model.ir.def(d).body,
                model.evaluator.scope(d).expect("scope"),
            )
        };
        let direct = PredicateAtom {
            pred: role("Body"),
            kind: AtomKind::Action,
            test: AtomTest::Nonstuttering {
                sub: role("Ordinary").expr,
            },
        };
        let state = |v: i64| -> dive_trace::State { Arc::from([Some(v.into())]) };
        assert!(
            !direct
                .evaluate(&model, &St::action(state(0), state(0)))
                .expect("stutter skips body error")
        );
        assert_eq!(
            direct
                .evaluate(&model, &St::action(state(0), state(1)))
                .expect_err("changed subscript reaches body")
                .ec()
                .code(),
            2179
        );
        let bad = PredicateAtom {
            test: AtomTest::Nonstuttering {
                sub: role("Bad").expr,
            },
            ..direct
        };
        assert_eq!(
            bad.evaluate(&model, &St::action(state(0), state(0)))
                .expect_err("ordinary subscript error precedes stutter")
                .ec()
                .code(),
            2179
        );
    }
}

#[test]
fn direct_query_instance_wrappers_keep_original_planning_and_native_atoms() {
    let source = include_str!("fixtures/nested-enabled/InstanceEnabled.tla");
    for eval in [dive_eval::Options::tlc(), dive_eval::Options::dive()] {
        let a = dive_sema::analyze_source(
            "InstanceEnabled.tla",
            source,
            &dive_sema::Options::default(),
        );
        assert!(a.accepted(), "{:?}", a.diagnostics());
        let model = dive_model::bind(
            Arc::new(dive_ir::lower(&a).expect("IR")),
            &dive_config::parse("INIT Init\nNEXT Next\n")
                .into_config()
                .expect("config"),
            &dive_model::BindOptions {
                eval,
                ..dive_model::BindOptions::tlc()
            },
        )
        .expect("model");
        for name in ["D", "F", "L", "Alias", "Neg", "Or"] {
            let d = model.ir.root_def(name).expect("role");
            let pred = dive_model::Pred::new(
                model.ir.def(d).body,
                model.evaluator.scope(d).expect("scope"),
            );
            let plan = dive_liveness::plan::lower(
                &model,
                &pred,
                dive_liveness::formula::Limits::default(),
            )
            .expect("established state-level query lowering");
            assert_eq!(plan.atoms.len(), 1, "{name}");
            for x in [0, 1] {
                let state: dive_trace::State = Arc::from([Some(x.into())]);
                for atom in &plan.atoms {
                    assert!(
                        atom.evaluate(&model, &dive_eval::St::state(state.clone()))
                            .expect("original native query atom"),
                        "{name}"
                    );
                }
            }
        }
    }
}

#[test]
fn actual_lazy_action_formals_keep_nested_query_ordering() {
    let source = include_str!("fixtures/nested-enabled/LazyActionEnabled.tla");
    for eval in [dive_eval::Options::tlc(), dive_eval::Options::dive()] {
        let a = dive_sema::analyze_source(
            "LazyActionEnabled.tla",
            source,
            &dive_sema::Options::default(),
        );
        assert!(a.accepted(), "{:?}", a.diagnostics());
        let model = dive_model::bind(
            Arc::new(dive_ir::lower(&a).expect("IR")),
            &dive_config::parse("INIT Init\nNEXT Next\n")
                .into_config()
                .expect("config"),
            &dive_model::BindOptions {
                eval,
                ..dive_model::BindOptions::tlc()
            },
        )
        .expect("model");
        for name in ["Guarded", "Ordered"] {
            let d = model.ir.root_def(name).expect("helper");
            assert!(model.ir.nodes()[model.ir.def(d).body.index()].level <= dive_ir::Level::State);
        }
        for (name, expected) in [("A", Some(true)), ("B", Some(false)), ("C", None)] {
            let d = model.ir.root_def(name).expect("role");
            let pred = dive_model::Pred::new(
                model.ir.def(d).body,
                model.evaluator.scope(d).expect("scope"),
            );
            let plan = dive_liveness::plan::lower(
                &model,
                &pred,
                dive_liveness::formula::Limits::default(),
            )
            .expect("actual action argument routing");
            assert_eq!(plan.atoms.len(), 1, "{name}");
            assert_eq!(
                plan.atoms[0].kind,
                dive_liveness::formula::AtomKind::Action,
                "{name}"
            );
            let result = plan.atoms[0].evaluate(
                &model,
                &dive_eval::St::action(Arc::from([Some(0.into())]), Arc::from([Some(1.into())])),
            );
            if let Some(expected) = expected {
                assert_eq!(result.expect("ordered Boolean action"), expected, "{name}");
            } else {
                assert!(
                    result.is_err(),
                    "reached inner division precedes false actual"
                );
            }
        }
    }
}

#[test]
fn configured_known_hidden_and_unused_query_actuals_refuse_in_all_four_routes() {
    use dive_model::Pred;
    use std::sync::atomic::Ordering;
    let source = include_str!("fixtures/nested-enabled/ConfiguredQueryEnabled.tla");
    for eval in [dive_eval::Options::tlc(), dive_eval::Options::dive()] {
        for target in ["Use", "Ignore", "QueryTarget"] {
            let a = dive_sema::analyze_source(
                "NestedEnabled.tla",
                source,
                &dive_sema::Options::default(),
            );
            assert!(a.accepted(), "{:?}", a.diagnostics());
            let cfg =
                dive_config::parse(&format!("INIT Init\nNEXT Next\nCONSTANT K <- {target}\n"))
                    .into_config()
                    .expect("config");
            let m = dive_model::bind(
                Arc::new(dive_ir::lower(&a).expect("IR")),
                &cfg,
                &dive_model::BindOptions {
                    eval,
                    ..dive_model::BindOptions::tlc()
                },
            )
            .expect("model");
            let hooks = Arc::new(SubscriptHooks::default());
            m.evaluator.set_host(hooks.clone());
            let role = |name| {
                let d = m.ir.root_def(name).expect("role");
                Pred::new(m.ir.def(d).body, m.evaluator.scope(d).expect("scope"))
            };
            let q = role("Q");
            let d = m.ir.root_def("Raw").expect("raw helper");
            let body = m.ir.def(d).body;
            assert!(
                !m.ir
                    .node(body)
                    .effects
                    .contains(dive_ir::Effects::USES_ENABLED)
            );
            let scope = m
                .evaluator
                .scope_apply(
                    &q.scope,
                    d,
                    &[dive_ir::Arg::Expr {
                        expr: q.expr,
                        pass: dive_ir::Pass::Lazy,
                    }],
                )
                .expect("actual raw-low-effects scope");
            for p in [
                role("Known"),
                role("Lazy"),
                role("Unused"),
                Pred::new(body, scope),
            ] {
                assert!(
                    dive_liveness::plan::lower(&m, &p, dive_liveness::formula::Limits::default())
                        .is_err(),
                    "planner {target}"
                );
            }
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        }
    }
}
