//! Two actual primary nested ENABLED references and independent native words.
#![allow(clippy::expect_used, clippy::panic, reason = "tiny primary assertions")]
use dive_check::CheckOptions;
use dive_liveness::analyze::{Analysis, Limits, analyze_final};
use dive_trace::{Ending, TraceDoc, Validation};
use serde_json::{Value, json};
use std::sync::Arc;
const SOURCE: &str = include_str!("fixtures/nested-enabled/NestedEnabled.tla");
fn bound(config: &str, options: &CheckOptions) -> dive_model::Model {
    let a = dive_sema::analyze_source("NestedEnabled.tla", SOURCE, &dive_sema::Options::default());
    assert!(a.accepted(), "{:?}", a.diagnostics());
    dive_model::bind(
        Arc::new(dive_ir::lower(&a).expect("IR")),
        &dive_config::parse(config).into_config().expect("cfg"),
        &dive_model::BindOptions {
            eval: options.eval,
            ..dive_model::BindOptions::tlc()
        },
    )
    .expect("model")
}
#[test]
fn two_primary_counts_truth_and_native_words_match_in_both_profiles_and_stores() {
    let oracle: Value =
        serde_json::from_str(include_str!("fixtures/nested-enabled/oracle.json")).expect("oracle");
    assert_eq!(oracle["primary_before_implementation"], true);
    for tlc in [true, false] {
        for store in [
            dive_store::StoreMode::Fingerprint,
            dive_store::StoreMode::Exact,
        ] {
            let options = CheckOptions {
                store,
                ..if tlc {
                    CheckOptions::tlc(0, 0)
                } else {
                    CheckOptions::dive(0)
                }
            };
            for (cfg, fail) in [
                (include_str!("fixtures/nested-enabled/hold.cfg"), false),
                (include_str!("fixtures/nested-enabled/fail.cfg"), true),
            ] {
                let model = bound(cfg, &options);
                for _ in 0..2 {
                    let result = analyze_final(&model, &options, Limits::default())
                        .expect("nested finite analysis");
                    let stats = match &result {
                        Analysis::Holds(c) => c.stats,
                        Analysis::Candidate(c) => c.exploration.stats,
                    };
                    assert_eq!(json!(stats.generated), oracle["generated"]);
                    assert_eq!(json!(stats.distinct), oracle["distinct"]);
                    assert_eq!(json!(stats.depth), oracle["depth"]);
                    match result {
                        Analysis::Holds(_) => assert!(!fail),
                        Analysis::Candidate(c) => {
                            assert!(fail);
                            assert_eq!(c.target_property, "Fail");
                            let word_states=c.states.iter().map(|s|json!({"x":match &s[0]{Some(dive_value::Value::Int(v))=>*v,_=>panic!("finite x")}})).collect();
                            let doc = TraceDoc::behavior(word_states)
                                .expect("word")
                                .with_ending(Ending::Lasso { back_to: c.back_to })
                                .with_properties(c.properties);
                            let fresh = bound(cfg, &options);
                            assert_eq!(
                                dive_trace::validate_doc(&fresh, &doc, &[]).0.validation,
                                Validation::Valid
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn native_and_planner_walks_refuse_data_provider_operator_and_primed_query_placements() {
    use dive_eval::St;
    use dive_model::Pred;
    for tlc in [true, false] {
        let options = if tlc {
            CheckOptions::tlc(0, 0)
        } else {
            CheckOptions::dive(0)
        };
        for (extra, action) in [
            ("", "(ENABLED (x'=1))=TRUE /\\ x'=1"),
            ("", "LET q==ENABLED (x'=1) IN q' /\\ x'=1"),
            (
                "F(v)==ENABLED (x'=v)\nPass(H(_),v)==H(v)",
                "Pass(F,1) /\\ x'=1",
            ),
            (
                "",
                "SelectSeq(<<0>>,LAMBDA v: ENABLED (x'=v)) = << >> /\\ x'=1",
            ),
            ("", "ENABLED (LET held==x'=1 IN held) /\\ x'=1"),
            ("", "LET f[t \\in {0}]==ENABLED (x'=1) IN f[0] /\\ x'=1"),
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
            let after: dive_trace::State = Arc::from([Some(1.into())]);
            assert!(
                model
                    .evaluator
                    .check_finite_bindings(
                        &action_pred.scope,
                        action_pred.expr,
                        &St::action(before.clone(), after),
                        4096
                    )
                    .is_err(),
                "finite {action}"
            );
            assert!(
                dive_liveness::plan::lower(
                    &model,
                    &property,
                    dive_liveness::formula::Limits::default()
                )
                .is_err(),
                "planner {action}"
            );
            assert!(
                dive_trace::temporal::evaluate_lasso(
                    &model,
                    &property,
                    std::slice::from_ref(&before),
                    0
                )
                .is_err(),
                "word {action}"
            );
            assert!(
                dive_trace::exists::exists_successor_pure(
                    &model,
                    &action_pred,
                    &before,
                    None,
                    dive_trace::exists::Limits::default(),
                    4096
                )
                .is_err(),
                "independent {action}"
            );
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
#[derive(Debug)]
struct SubscriptProvider(Arc<std::sync::atomic::AtomicUsize>);
impl dive_value::DeferredSet for SubscriptProvider {
    fn kind(&self) -> dive_value::DeferredKind {
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<dive_value::Value>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(vec![0.into()])
    }
    fn member(
        &self,
        _: &dive_value::Value,
        _: dive_value::Ctx,
    ) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<dive_value::Value> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        vec![]
    }
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one prebound scope exercises refusal, callback isolation and original error ordering together"
)]
fn prebound_nonstuttering_subscripts_refuse_direct_and_hidden_lazy_queries_before_callbacks() {
    use dive_eval::St;
    use dive_liveness::formula::AtomKind;
    use dive_liveness::plan::{AtomTest, PredicateAtom};
    use dive_model::Pred;
    use std::sync::atomic::{AtomicUsize, Ordering};
    for options in [dive_eval::Options::tlc(), dive_eval::Options::dive()] {
        let source = "---- MODULE SubscriptQueries ----\nEXTENDS Integers\nVARIABLE x\nInit==x=0\nNext==x'=1\nQ==ENABLED (x' \\in x)\nWrap==LET held==Q IN TRUE /\\ held\nBody==1 \\div 0=0\nOrdinary==x\nBad==1 \\div 0\n====\n";
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
        let wrap = role("Wrap");
        let dive_ir::Expr::Let { body, .. } = model.ir.expr(wrap.expr) else {
            panic!("LET")
        };
        let scope = model
            .evaluator
            .scope_let(&wrap.scope, wrap.expr)
            .expect("lazy scope");
        let dive_ir::Expr::Junction { items, .. } = model.ir.expr(*body) else {
            panic!("conjunction")
        };
        let hidden = PredicateAtom {
            pred: Pred::new(items[0], scope),
            kind: AtomKind::Action,
            test: AtomTest::Nonstuttering { sub: items[1] },
            binding_limit: 4096,
        };
        let direct = PredicateAtom {
            pred: role("Body"),
            test: AtomTest::Nonstuttering {
                sub: role("Q").expr,
            },
            ..hidden.clone()
        };
        let hooks = Arc::new(SubscriptHooks::default());
        model.evaluator.set_host(hooks.clone());
        let count = Arc::new(AtomicUsize::new(0));
        let provider =
            dive_value::Value::deferred(Arc::new(SubscriptProvider(count.clone())), options.ctx);
        let state = |v| Arc::from([Some(v)]);
        for atom in [&direct, &hidden] {
            for (before, after) in [(0.into(), 1.into()), (provider.clone(), provider.clone())] {
                let error = atom
                    .evaluate(&model, &St::action(state(before), state(after)))
                    .expect_err("data-position query must be refused before evaluation");
                assert!(error.to_string().contains("ENABLED"), "{error}");
            }
        }
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        assert_eq!(count.load(Ordering::SeqCst), 0);
        let ordinary = PredicateAtom {
            test: AtomTest::Nonstuttering {
                sub: role("Ordinary").expr,
            },
            ..direct.clone()
        };
        assert!(
            !ordinary
                .evaluate(&model, &St::action(state(0.into()), state(0.into())))
                .expect("stutter skips body error")
        );
        assert_eq!(
            ordinary
                .evaluate(&model, &St::action(state(0.into()), state(1.into())))
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
            bad.evaluate(&model, &St::action(state(0.into()), state(0.into())))
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
                let before: dive_trace::State = Arc::from([Some(0.into())]);
                let after: dive_trace::State = Arc::from([Some(1.into())]);
                assert!(
                    m.evaluator
                        .check_finite_bindings(
                            &p.scope,
                            p.expr,
                            &dive_eval::St::action(before.clone(), after.clone()),
                            4096
                        )
                        .is_err(),
                    "finite {target}"
                );
                assert!(
                    dive_liveness::plan::lower(&m, &p, dive_liveness::formula::Limits::default())
                        .is_err(),
                    "planner {target}"
                );
                assert!(
                    dive_trace::temporal::evaluate_lasso(&m, &p, &[before.clone(), after], 0)
                        .is_err(),
                    "word {target}"
                );
                assert!(
                    dive_trace::exists::exists_successor_pure(
                        &m,
                        &p,
                        &before,
                        None,
                        dive_trace::exists::Limits::default(),
                        4096
                    )
                    .is_err(),
                    "independent {target}"
                );
            }
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        }
    }
}
