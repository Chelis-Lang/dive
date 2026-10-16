//! Nested queries are independent of the optional checker enumerator.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded semantic assertions"
)]
use dive_eval::Options;
use dive_model::{Model, Pred};
use dive_trace::{
    Ending, State, TraceDoc, Validation,
    exists::{Decision, Limits, exists_successor_pure},
    temporal::evaluate_lasso,
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn bind(source: &str, config: &str, options: Options) -> Model {
    let a = dive_sema::analyze_source("NestedEnabled.tla", source, &dive_sema::Options::default());
    assert!(a.accepted(), "{:?}", a.diagnostics());
    dive_model::bind(
        Arc::new(dive_ir::lower(&a).expect("IR")),
        &dive_config::parse(config).into_config().expect("cfg"),
        &dive_model::BindOptions {
            eval: options,
            ..dive_model::BindOptions::tlc()
        },
    )
    .expect("model")
}
fn small(body: &str, options: Options) -> Model {
    bind(
        &format!(
            "---- MODULE NestedEnabled ----\nEXTENDS Integers,TLC\nVARIABLE x\nInit==x=0\nNext==x'=1\n{body}\n====\n"
        ),
        "INIT Init\nNEXT Next\n",
        options,
    )
}
fn pred(m: &Model, name: &str) -> Pred {
    let d = m.ir.root_def(name).expect(name);
    Pred::new(m.ir.def(d).body, m.evaluator.scope(d).expect("scope"))
}
fn state(x: i64) -> State {
    Arc::from([Some(x.into())])
}
fn query(
    m: &Model,
    name: &str,
    x: i64,
    limits: Limits,
) -> Result<Decision, dive_trace::exists::ExistsError> {
    exists_successor_pure(m, &pred(m, name), &state(x), None, limits, 4096)
}

#[test]
fn nested_queries_keep_inner_witness_namespaces_and_independent_constraints() {
    for options in [Options::tlc(), Options::dive()] {
        let m = small(
            "Inner(v)==x'=v /\\ x=0\nA==ENABLED Inner(99) /\\ x'=1\nB==~ENABLED (x'=1 /\\ x=1) /\\ x'=1\nC==LET guard==ENABLED Inner(99) IN guard /\\ x'=1\nD==\\E v \\in {99}: ENABLED Inner(v) /\\ x'=1\n",
            options,
        );
        for _ in 0..2 {
            for name in ["A", "B", "C", "D"] {
                let Decision::Enabled { witness, .. } =
                    query(&m, name, 0, Limits::default()).expect(name)
                else {
                    panic!("{name}: actual inner99/outer1 witness")
                };
                assert_eq!(witness, state(1));
            }
        }
        // Inner primes supply no cover for the outer x'.
        let missing = small("A==ENABLED (x'=99) /\\ x'>0", options);
        assert!(query(&missing, "A", 0, Limits::default()).is_err());
        let source = "---- MODULE NestedEnabled ----\nEXTENDS Naturals\nVARIABLE x\nInit==x=0\nNext==x'=0\nLimit==x=0\nA==ENABLED (x'=99) /\\ x'=1\n====\n";
        let constrained = bind(source, "INIT Init\nNEXT Next\nCONSTRAINT Limit\n", options);
        assert!(matches!(
            query(&constrained, "A", 0, Limits::default()).expect("not graph-restricted"),
            Decision::Enabled { .. }
        ));
    }
}

#[test]
fn nested_candidate_assignment_and_expression_budgets_never_reset() {
    for options in [Options::tlc(), Options::dive()] {
        let m = small("A==x' \\in {0,1} /\\ ENABLED (x'=7) /\\ x'=1\n", options);
        assert!(matches!(
            query(
                &m,
                "A",
                0,
                Limits {
                    assignments: 4,
                    ..Limits::default()
                }
            )
            .expect("two outer plus two inner visits"),
            Decision::Enabled { .. }
        ));
        for limits in [
            Limits {
                assignments: 3,
                ..Limits::default()
            },
            Limits {
                assignments: 0,
                ..Limits::default()
            },
            Limits {
                expressions: 0,
                ..Limits::default()
            },
            Limits {
                domain_values: 0,
                ..Limits::default()
            },
        ] {
            assert!(
                query(&m, "A", 0, limits).is_err(),
                "{limits:?}: never Disabled"
            );
        }
        let late = small(
            "A==x' \\in {0,1,2} /\\ ((x'=0 /\\ ENABLED (x'=7) /\\ FALSE) \\/ x'=99)",
            options,
        );
        assert!(
            query(
                &late,
                "A",
                0,
                Limits {
                    assignments: 3,
                    ..Limits::default()
                }
            )
            .is_err(),
            "inner consumption must be checked before the late outer shortcircuit"
        );
        let bounded = small("A==ENABLED (\\E i \\in 1..100: x'=i) /\\ x'=1", options);
        assert!(
            exists_successor_pure(
                &bounded,
                &pred(&bounded, "A"),
                &state(0),
                None,
                Limits::default(),
                10
            )
            .is_err(),
            "inner binders retain the original invocation binding limit"
        );
        for inner in ["x'>0", "x' \\in Nat", "x' \\in {} /\\ ENABLED (x'=0)"] {
            let m = small(&format!("A==ENABLED ({inner}) /\\ x'=1"), options);
            assert!(query(&m, "A", 0, Limits::default()).is_err(), "{inner}");
        }
    }
}

#[test]
fn nested_boolean_source_order_preserves_reached_and_unreached_errors() {
    let source = "A==FALSE /\\ (1 \\div 0=0) /\\ ENABLED (x'=1) /\\ x'=1\nB==ENABLED (x'=1) /\\ FALSE /\\ (1 \\div 0=0) /\\ x'=1\nC==ENABLED (x'=1) /\\ (1 \\div 0=0) /\\ FALSE /\\ x'=1\nD==ENABLED (x'=1 /\\ 1 \\div 0=0) /\\ FALSE /\\ x'=1\nE==~ENABLED (FALSE /\\ 1 \\div 0=0 /\\ x'=1) /\\ x'=1\nP==<>[][B]_x\n";
    for options in [Options::tlc(), Options::dive()] {
        let m = small(source, options);
        for name in ["A", "B"] {
            assert!(matches!(
                query(&m, name, 0, Limits::default()).expect("unreached division"),
                Decision::Disabled
            ));
        }
        assert!(matches!(
            query(&m, "E", 0, Limits::default()).expect("inner unreached division"),
            Decision::Enabled { .. }
        ));
        for name in ["C", "D"] {
            assert!(
                query(&m, name, 0, Limits::default()).is_err(),
                "{name}: reached division remains error"
            );
        }
        assert_eq!(
            evaluate_lasso(&m, &pred(&m, "P"), &[state(0), state(1)], 0)
                .expect("closing action evaluation keeps shortcircuit"),
            vec![false, false]
        );
    }
}

#[derive(Debug, Default)]
struct Hooks(AtomicUsize);
impl dive_eval::Host for Hooks {
    fn print_output(&self, _: &str) -> Result<bool, dive_eval::HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }
    fn check_native_access(&self, _: dive_ir::StdOp) -> Result<(), dive_eval::HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[test]
fn unsupported_query_placements_effects_foreign_scopes_and_callbacks_refuse() {
    for options in [Options::tlc(), Options::dive()] {
        for body in [
            "A==(ENABLED (x'=1))' /\\ x'=1",
            "A==LET q==ENABLED (x'=1) IN q' /\\ x'=1",
            "A==(ENABLED (x'=1))=TRUE /\\ x'=1",
            "A==ENABLED (PrintT(0) /\\ x'=1) /\\ x'=1",
            "A==ENABLED (TLCGet(\"level\")=0 /\\ x'=1) /\\ x'=1",
            "A==ENABLED (x' \\in {PrintT(0)}) /\\ x'=1",
        ] {
            let m = small(body, options);
            let h = Arc::new(Hooks::default());
            m.evaluator.set_host(h.clone());
            assert!(query(&m, "A", 0, Limits::default()).is_err(), "{body}");
            assert_eq!(h.0.load(Ordering::SeqCst), 0);
        }
        let m = small("A==ENABLED (x'=1) /\\ x'=1", options);
        let other = small("A==ENABLED (x'=1) /\\ x'=1", options);
        let mut p = pred(&m, "A");
        p.scope = pred(&other, "A").scope;
        assert!(exists_successor_pure(&m, &p, &state(0), None, Limits::default(), 4096).is_err());
        assert!(
            exists_successor_pure(
                &m,
                &pred(&m, "A"),
                &Arc::from([None]),
                None,
                Limits::default(),
                4096
            )
            .is_err()
        );
    }
}

#[test]
fn actual_primary_word_and_init_edge_closing_property_mutants_validate_independently() {
    let source =
        include_str!("../../dive-liveness/tests/fixtures/nested-enabled/NestedEnabled.tla");
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-liveness/tests/fixtures/nested-enabled/oracle.json"
    ))
    .expect("primary observations");
    let original = oracle["fail"]["states"]
        .as_array()
        .expect("actual word")
        .clone();
    let back = usize::try_from(oracle["fail"]["back_to"].as_u64().expect("actual loop"))
        .expect("tiny loop");
    for options in [Options::tlc(), Options::dive()] {
        for strong in [false, true] {
            let source = if strong {
                source.replace("WF_", "SF_")
            } else {
                source.into()
            };
            let m = bind(&source, "SPECIFICATION Spec\nPROPERTY Fail\n", options);
            let doc = TraceDoc::behavior(original.clone())
                .expect("actual primary word")
                .with_ending(Ending::Lasso { back_to: back })
                .with_properties(vec!["Fail".into()]);
            for _ in 0..2 {
                assert_eq!(
                    dive_trace::validate_doc(&m, &doc, &[]).0.validation,
                    Validation::Valid
                );
            }
            let mut bad = doc.clone();
            bad.states[0]["x"] = json!(1);
            assert_ne!(
                dive_trace::validate_doc(&m, &bad, &[]).0.validation,
                Validation::Valid
            );
            let mut bad = doc.clone();
            bad.states[1]["x"] = json!(2);
            assert_ne!(
                dive_trace::validate_doc(&m, &bad, &[]).0.validation,
                Validation::Valid
            );
            let bad = doc.clone().with_ending(Ending::Lasso { back_to: 1 });
            assert_ne!(
                dive_trace::validate_doc(&m, &bad, &[]).0.validation,
                Validation::Valid
            );
            let good = bind(&source, "SPECIFICATION Spec\nPROPERTY Hold\n", options);
            let bad = doc.with_properties(vec!["Hold".into()]);
            assert_ne!(
                dive_trace::validate_doc(&good, &bad, &[]).0.validation,
                Validation::Valid
            );
            assert_eq!(
                evaluate_lasso(&m, &pred(&m, "B"), &[state(0), state(1)], 0)
                    .expect("inner x'=1 while closing target0"),
                vec![false, true]
            );
        }
    }
}

#[derive(Debug)]
struct Provider(Arc<AtomicUsize>);
impl dive_value::DeferredSet for Provider {
    fn kind(&self) -> dive_value::DeferredKind {
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<dive_value::Value>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![0.into()])
    }
    fn member(
        &self,
        _: &dive_value::Value,
        _: dive_value::Ctx,
    ) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<dive_value::Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        vec![]
    }
}
#[test]
fn nested_queries_refuse_rust_providers_and_existing_depth_exhaustion_without_callbacks() {
    for options in [Options::tlc(), Options::dive()] {
        let m = small("A==ENABLED (x' \\in x) /\\ x'=0", options);
        let count = Arc::new(AtomicUsize::new(0));
        let set = dive_value::Value::deferred(Arc::new(Provider(count.clone())), options.ctx);
        let before: State = Arc::from([Some(set)]);
        for _ in 0..2 {
            assert!(
                exists_successor_pure(&m, &pred(&m, "A"), &before, None, Limits::default(), 4096)
                    .is_err()
            );
        }
        assert_eq!(count.load(Ordering::SeqCst), 0);
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn(move || {
                let mut inner = "x'=1".to_owned();
                for _ in 0..100 {
                    inner = format!("x'=1 /\\ ENABLED ({inner})");
                }
                let m = small(&format!("A=={inner}"), options);
                let error = query(&m, "A", 0, Limits::default())
                    .expect_err("existing absolute source-depth guard");
                assert!(
                    matches!(error, dive_trace::exists::ExistsError::Limit("expressions")),
                    "{error}"
                );
            })
            .expect("documented stack worker")
            .join()
            .expect("deep control completed without a panic");
    }
}

#[test]
fn direct_state_query_wrappers_preserve_plain_and_instance_words() {
    let instance =
        include_str!("../../dive-liveness/tests/fixtures/nested-enabled/InstanceEnabled.tla");
    let plain = instance
        .replace("D==I!Direct", "D==ENABLED (x'=1)")
        .replace("F==I!F", "F==D /\\ TRUE")
        .replace("L==I!L", "L==LET guard==D IN guard /\\ TRUE")
        .replace("Alias==I!Alias", "Alias==F")
        .replace("Neg==I!Neg", "Neg==~(D /\\ FALSE)")
        .replace("Or==I!Or", "Or==FALSE \\/ D");
    let disabled = instance.replace("A==x'=C", "A==x'=C /\\ FALSE");
    for options in [Options::tlc(), Options::dive()] {
        for (source, enabled) in [
            (instance, true),
            (plain.as_str(), true),
            (disabled.as_str(), false),
        ] {
            let a = dive_sema::analyze_source(
                "InstanceEnabled.tla",
                source,
                &dive_sema::Options::default(),
            );
            assert!(a.accepted(), "{:?}", a.diagnostics());
            let m = dive_model::bind(
                Arc::new(dive_ir::lower(&a).expect("IR")),
                &dive_config::parse("INIT Init\nNEXT Next\n")
                    .into_config()
                    .expect("config"),
                &dive_model::BindOptions {
                    eval: options,
                    ..dive_model::BindOptions::tlc()
                },
            )
            .expect("model");
            for name in ["D", "F", "L", "Alias", "Neg", "Or"] {
                let result = evaluate_lasso(&m, &pred(&m, name), &[state(0), state(1)], 0);
                assert_eq!(
                    result.expect("actual root or INSTANCE query word"),
                    vec![enabled || name == "Neg"; 2],
                    "{name}: enabled={enabled}"
                );
            }
        }
    }
}

#[test]
fn actual_lazy_action_formals_use_independent_nested_queries_in_source_order() {
    let source =
        include_str!("../../dive-liveness/tests/fixtures/nested-enabled/LazyActionEnabled.tla");
    for options in [Options::tlc(), Options::dive()] {
        let a = dive_sema::analyze_source(
            "LazyActionEnabled.tla",
            source,
            &dive_sema::Options::default(),
        );
        assert!(a.accepted(), "{:?}", a.diagnostics());
        let m = dive_model::bind(
            Arc::new(dive_ir::lower(&a).expect("IR")),
            &dive_config::parse("INIT Init\nNEXT Next\n")
                .into_config()
                .expect("config"),
            &dive_model::BindOptions {
                eval: options,
                ..dive_model::BindOptions::tlc()
            },
        )
        .expect("model");
        assert_eq!(
            evaluate_lasso(&m, &pred(&m, "A"), &[state(0), state(1)], 0)
                .expect("actual action formal plus fresh inner7/8"),
            vec![true, false]
        );
        assert_eq!(
            evaluate_lasso(&m, &pred(&m, "B"), &[state(0), state(1)], 0)
                .expect("unreached division"),
            vec![false, false]
        );
        assert!(
            evaluate_lasso(&m, &pred(&m, "C"), &[state(0), state(1)], 0).is_err(),
            "reached inner error precedes false action actual"
        );
    }
}

#[test]
fn hidden_state_query_actuals_use_independent_exists_and_keep_word_boundary() {
    use dive_ir::{Arg, Effects, Pass};
    let source =
        include_str!("../../dive-liveness/tests/fixtures/nested-enabled/StateFormalEnabled.tla");
    for options in [Options::tlc(), Options::dive()] {
        let mut m = bind(source, "INIT Init\nNEXT Next\n", options);
        let hooks = Arc::new(Hooks::default());
        m.evaluator.set_host(hooks.clone());
        for name in ["A", "B", "C", "D"] {
            assert!(
                matches!(
                    evaluate_lasso(&m, &pred(&m, name), &[state(0), state(1)], 0),
                    Err(dive_trace::temporal::TemporalError::Unsupported(_))
                ),
                "{name}"
            );
        }
        let q = pred(&m, "Q");
        let guard = m.ir.root_def("Guard").expect("helper");
        let body = m.ir.def(guard).body;
        assert!(
            !m.ir.node(body).effects.contains(Effects::USES_ENABLED),
            "actual raw-low-effects body"
        );
        let called = m
            .evaluator
            .scope_apply(
                &q.scope,
                guard,
                &[Arg::Expr {
                    expr: q.expr,
                    pass: Pass::Lazy,
                }],
            )
            .expect("actual lazy query slot");
        let thunk = Pred::new(body, called);
        assert!(matches!(
            evaluate_lasso(&m, &thunk, &[state(0), state(1)], 0),
            Err(dive_trace::temporal::TemporalError::Unsupported(_))
        ));
        let Decision::Enabled {
            witness,
            assignment,
        } = exists_successor_pure(&m, &thunk, &state(0), None, Limits::default(), 4096)
            .expect("actual lazy state query uses independent inner cover")
        else {
            panic!("the reached query has a successor")
        };
        assert_eq!(witness, state(0));
        assert!(
            assignment.is_empty(),
            "inner primes cannot populate an outer candidate"
        );
        let limited = Limits {
            assignments: 1,
            ..Limits::default()
        };
        assert!(
            matches!(
                exists_successor_pure(&m, &thunk, &state(0), None, limited, 4096),
                Err(dive_trace::exists::ExistsError::Limit("assignments"))
            ),
            "the outer candidate and reached inner query share work"
        );
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        m.next = Some(thunk);
        let doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])
            .expect("word")
            .with_ending(Ending::Lasso { back_to: 0 });
        assert!(
            matches!(
                dive_trace::validate_doc(&m, &doc, &[]).0.validation,
                Validation::Unsupported(_)
            ),
            "original periodic predicate keeps the actual lazy scope"
        );
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn periodic_query_labels_are_optional_notes_and_cannot_hide_a_forged_next_edge() {
    for options in [Options::tlc(), Options::dive()] {
        let source =
            include_str!("../../dive-liveness/tests/fixtures/nested-enabled/NestedEnabled.tla")
                .replace(
                    "Spec ==",
                    "Guard(p)==p /\\ TRUE\nQueryLabel==Guard(ENABLED (x'=7))\nSpec ==",
                );
        let m = bind(&source, "SPECIFICATION Spec\nPROPERTY Fail\n", options);
        let hooks = Arc::new(Hooks::default());
        m.evaluator.set_host(hooks.clone());
        let mut doc = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])
            .expect("word")
            .with_ending(Ending::Lasso { back_to: 0 })
            .with_properties(vec!["Fail".into()]);
        doc.labels[1] = Some(dive_trace::Label {
            name: "QueryLabel".into(),
            location: None,
        });
        let report = dive_trace::validate_doc(&m, &doc, &[]).0;
        assert_eq!(report.validation, Validation::Valid);
        assert!(
            report
                .notes
                .iter()
                .any(|n| n == "label `QueryLabel` could not be evaluated on its own; not checked")
        );
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        doc.states[1]["x"] = json!(2);
        let Validation::Invalid(failure) = dive_trace::validate_doc(&m, &doc, &[]).0.validation
        else {
            panic!("forged original Next edge must fail")
        };
        assert_eq!(failure.check, dive_trace::Check::Next);
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn nested_queries_retain_actual_recursive_domains_and_original_schedule_witnesses() {
    let source =
        include_str!("../../dive-liveness/tests/fixtures/recursive-function/PermSeqsScope.tla")
            .replace("MODULE PermSeqsScope", "MODULE NestedEnabled")
            .replace("====", "Nested==ENABLED Schedule /\\ Schedule\n====");
    for options in [Options::tlc(), Options::dive()] {
        let m = bind(&source, "CONSTANT N=2\nINIT Init\nNEXT Next\n", options);
        for values in [vec![], vec![1], vec![1, 2]] {
            for done in [false, true] {
                let before: State = Arc::from([
                    Some(dive_value::Value::set_raw(
                        values.iter().copied().map(dive_value::Value::Int).collect(),
                        options.ctx,
                    )),
                    Some(done.into()),
                ]);
                let result = exists_successor_pure(
                    &m,
                    &pred(&m, "Nested"),
                    &before,
                    Some(&pred(&m, "vars")),
                    Limits::default(),
                    4096,
                )
                .expect("bounded recursive inner and outer covers");
                assert_eq!(
                    matches!(result, Decision::Enabled { .. }),
                    !(values.is_empty() && done)
                );
                if let Decision::Enabled { witness, .. } = result {
                    assert_eq!(
                        m.eval(
                            &pred(&m, "Schedule"),
                            &dive_eval::St::action(before, witness)
                        )
                        .expect("original Schedule correlation"),
                        dive_value::Value::Bool(true)
                    );
                }
            }
        }
    }
}

#[test]
fn independent_configured_query_actuals_refuse_even_when_raw_effects_are_clear() {
    let source = include_str!(
        "../../dive-liveness/tests/fixtures/nested-enabled/ConfiguredQueryEnabled.tla"
    );
    for options in [Options::tlc(), Options::dive()] {
        for target in ["Use", "Ignore", "QueryTarget"] {
            let m = bind(
                source,
                &format!("INIT Init\nNEXT Next\nCONSTANT K <- {target}\n"),
                options,
            );
            let hooks = Arc::new(Hooks::default());
            m.evaluator.set_host(hooks.clone());
            let q = pred(&m, "Q");
            let d = m.ir.root_def("Raw").expect("helper");
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
                .expect("actual configured query thunk");
            for p in [
                pred(&m, "Known"),
                pred(&m, "Lazy"),
                pred(&m, "Unused"),
                Pred::new(body, scope),
            ] {
                assert!(matches!(
                    evaluate_lasso(&m, &p, &[state(0), state(1)], 0),
                    Err(dive_trace::temporal::TemporalError::Unsupported(_))
                ));
                assert!(matches!(
                    exists_successor_pure(&m, &p, &state(0), None, Limits::default(), 4096),
                    Err(dive_trace::exists::ExistsError::Unsupported(_))
                ));
            }
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        }
    }
}

fn actual_query_thunk_pred(m: &Model, name: &str, q: &Pred) -> Pred {
    let d = m.ir.root_def(name).expect(name);
    let body = m.ir.def(d).body;
    assert!(
        !m.ir
            .node(body)
            .effects
            .contains(dive_ir::Effects::USES_ENABLED)
    );
    let mut args = vec![dive_ir::Arg::Expr {
        expr: q.expr,
        pass: dive_ir::Pass::Lazy,
    }];
    if name == "ParameterData" {
        let call = m.ir.root_def("ParameterSource").expect("source call");
        let dive_ir::Expr::Apply { args: actual, .. } = m.ir.expr(m.ir.def(call).body) else {
            panic!("actual operator call")
        };
        args.insert(0, actual[0].clone());
    }
    let scope = m
        .evaluator
        .scope_apply(&q.scope, d, &args)
        .expect("actual Lazy(query) argument");
    let mut todo = vec![body];
    let mut actual_query = false;
    while let Some(e) = todo.pop() {
        if let dive_ir::Expr::Local(local) = m.ir.expr(e)
            && let dive_eval::SlotView::Lazy(actual, _) = m.evaluator.scope_slot(&scope, *local)
        {
            assert_eq!(actual, q.expr);
            actual_query = true;
        }
        todo.extend(m.ir.expr(e).children());
    }
    assert!(actual_query, "{name}: original owned query source");
    Pred::new(body, scope)
}

#[test]
fn finite_data_positions_refuse_actual_query_thunks_before_domain_evaluation() {
    let source = "---- MODULE NestedEnabled ----\nEXTENDS Naturals,Sequences\nVARIABLE x\nInit==x=0\nNext==x'=1\nQ==ENABLED (x'=7)\nFilterDomain(z)==\\E v \\in {i \\in {0}:z}:TRUE\nForallDomain(z)==\\A v \\in {i \\in {0}:z}:TRUE\nFunctionDomain(z)==[v \\in (IF z THEN {0} ELSE {})|->TRUE]\nMapDomain(z)=={TRUE:v \\in (IF z THEN {0} ELSE {})}\nFilterBase(z)=={v \\in (IF z THEN {0} ELSE {}):TRUE}\nFunctionData(z)==[v \\in {0}|->z]\nMapData(z)=={z:v \\in {0}}\nFilterData(z)=={v \\in {0}:z}\nChooseData(z)==CHOOSE v \\in {0}:z\nUnchangedData(z)==UNCHANGED z\nPrimeData(z)==z'\nKeep(v)==TRUE\nSequenceData(z)==SelectSeq(z,Keep)\nNativeData(z)==Len(z)\nParameterData(H(_),z)==H(z)\nParameterSource==ParameterData(Len,Q)\nExistsBody(z)==\\E v \\in {0}:z\nForallBody(z)==\\A v \\in {0}:z\nExistsDirect==\\E v \\in {7}:ENABLED (x'=v)\nForallDirect==\\A v \\in {7}:ENABLED (x'=v)\n====\n";
    for options in [Options::tlc(), Options::dive()] {
        let m = bind(source, "INIT Init\nNEXT Next\n", options);
        assert_eq!(dive_ir::validate::check(&m.ir), [] as [String; 0]);
        let hooks = Arc::new(Hooks::default());
        m.evaluator.set_host(hooks.clone());
        let q = pred(&m, "Q");
        for name in [
            "FilterDomain",
            "ForallDomain",
            "FunctionDomain",
            "MapDomain",
            "FilterBase",
            "FunctionData",
            "MapData",
            "FilterData",
            "ChooseData",
            "UnchangedData",
            "PrimeData",
            "SequenceData",
            "NativeData",
            "ParameterData",
            "ExistsBody",
            "ForallBody",
        ] {
            let p = actual_query_thunk_pred(&m, name, &q);
            let body = p.expr;
            let scope = p.scope;
            for _ in 0..2 {
                let result = m.evaluator.check_finite_bindings(
                    &scope,
                    body,
                    &dive_eval::St::state(state(0)),
                    4096,
                );
                if matches!(name, "ExistsBody" | "ForallBody") {
                    result.expect("Boolean quantifier bodies retain query support");
                } else {
                    let error =
                        result.expect_err("data query must refuse before evaluating a domain");
                    assert_eq!(error.class(), dive_eval::ErrorClass::Unsupported, "{name}");
                    // Core ENABLED evaluation attaches a source origin, even
                    // when its enumerator is absent. This is source refusal.
                    assert_eq!(error.origin(), None, "{name}: {error}");
                }
                assert!(
                    m.evaluator
                        .check_finite_data_bindings(
                            &scope,
                            body,
                            &dive_eval::St::state(state(0)),
                            4096
                        )
                        .is_err()
                );
            }
        }
        for name in ["ExistsDirect", "ForallDirect"] {
            let p = pred(&m, name);
            for _ in 0..2 {
                m.evaluator
                    .check_finite_bindings(&p.scope, p.expr, &dive_eval::St::state(state(0)), 4096)
                    .expect("Boolean query source");
                assert_eq!(
                    evaluate_lasso(&m, &p, &[state(0)], 0).expect("independent Boolean query"),
                    [true]
                );
            }
        }
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn temporal_expression_forwarding_keeps_queries_but_not_operator_or_data_captures() {
    let source = r"---- MODULE NestedEnabled ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Identity(p) == p
Relay(p) == LET Saved == p IN Identity(Saved)
Guard(p) == p /\ TRUE
Query == ENABLED Next
AlwaysQuery == Identity([]Query)
EventuallyQuery == Relay([]<>Query)
NegatedQuery == Identity(<>(~Query))
Fair == Relay(WF_x(Next))
Hidden == Identity([]Guard(Query))
Higher(Op(_)) == []Op(x)
Captured(v) == ENABLED Next
OperatorCapture == Identity(Higher(Captured))
Data == Identity([](<<Query>> = <<TRUE>>))
====
";
    for options in [Options::tlc(), Options::dive()] {
        let model = bind(source, "INIT Init\nNEXT Next\n", options);
        let hooks = Arc::new(Hooks::default());
        model.evaluator.set_host(hooks.clone());
        let states: Vec<State> = vec![
            Arc::from([Some(false.into())]),
            Arc::from([Some(true.into())]),
        ];
        for _ in 0..2 {
            for (name, expected) in [
                ("AlwaysQuery", true),
                ("EventuallyQuery", true),
                ("NegatedQuery", false),
                ("Fair", true),
            ] {
                assert_eq!(
                    dive_trace::temporal::evaluate_lasso(&model, &pred(&model, name), &states, 0)
                        .expect(name),
                    vec![expected; states.len()],
                    "{name}"
                );
            }
            for name in ["Hidden", "OperatorCapture", "Data"] {
                assert!(matches!(
                    dive_trace::temporal::evaluate_lasso(&model, &pred(&model, name), &states, 0),
                    Err(dive_trace::temporal::TemporalError::Unsupported(_))
                ));
            }
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
            assert_eq!(model.evaluator.take_output(), [] as [String; 0]);
        }
    }
}
