//! Nested queries are independent of the optional checker enumerator.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded semantic assertions"
)]
use dive_eval::Options;
use dive_model::{Model, Pred};
use dive_trace::{Ending, State, TraceDoc, Validation, temporal::evaluate_lasso};
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
fn query(m: &Model, name: &str, x: i64) -> Result<bool, dive_trace::enabled::EnabledError> {
    dive_trace::enabled::enabled(m, &pred(m, name), None, &state(x))
}

#[test]
fn nested_queries_keep_inner_namespaces_and_ignore_constraints() {
    for options in [Options::tlc(), Options::dive()] {
        let m = small(
            "Inner(v)==x'=v /\\ x=0\nA==ENABLED Inner(99) /\\ x'=1\nB==~ENABLED (x'=1 /\\ x=1) /\\ x'=1\nC==LET guard==ENABLED Inner(99) IN guard /\\ x'=1\nD==\\E v \\in {99}: ENABLED Inner(v) /\\ x'=1\n",
            options,
        );
        for _ in 0..2 {
            for name in ["A", "B", "C", "D"] {
                assert_eq!(query(&m, name, 0), Ok(true), "{name}");
            }
        }
        let source = "---- MODULE NestedEnabled ----\nEXTENDS Naturals\nVARIABLE x\nInit==x=0\nNext==x'=0\nLimit==x=0\nA==ENABLED (x'=99) /\\ x'=1\n====\n";
        let constrained = bind(source, "INIT Init\nNEXT Next\nCONSTRAINT Limit\n", options);
        assert_eq!(
            query(&constrained, "A", 0),
            Ok(true),
            "not graph-restricted"
        );
    }
}

#[test]
fn nested_boolean_source_order_preserves_reached_and_unreached_errors() {
    let source = "A==FALSE /\\ (1 \\div 0=0) /\\ ENABLED (x'=1) /\\ x'=1\nB==ENABLED (x'=1) /\\ FALSE /\\ (1 \\div 0=0) /\\ x'=1\nC==ENABLED (x'=1) /\\ (1 \\div 0=0) /\\ FALSE /\\ x'=1\nD==ENABLED (x'=1 /\\ 1 \\div 0=0) /\\ FALSE /\\ x'=1\nE==~ENABLED (FALSE /\\ 1 \\div 0=0 /\\ x'=1) /\\ x'=1\nP==<>[][B]_x\n";
    for options in [Options::tlc(), Options::dive()] {
        let m = small(source, options);
        for name in ["A", "B"] {
            assert_eq!(query(&m, name, 0), Ok(false), "{name}: unreached division");
        }
        assert_eq!(query(&m, "E", 0), Ok(true), "inner unreached division");
        for name in ["C", "D"] {
            assert!(
                query(&m, name, 0).is_err(),
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
fn effects_foreign_scopes_and_incomplete_states_refuse() {
    for options in [Options::tlc(), Options::dive()] {
        for body in [
            "A==ENABLED (PrintT(0) /\\ x'=1) /\\ x'=1",
            "A==ENABLED (x' \\in {PrintT(0)}) /\\ x'=1",
        ] {
            let m = small(body, options);
            let h = Arc::new(Hooks::default());
            m.evaluator.set_host(h.clone());
            assert!(query(&m, "A", 0).is_err(), "{body}");
            assert_eq!(h.0.load(Ordering::SeqCst), 0);
        }
        let m = small("A==ENABLED (x'=1) /\\ x'=1", options);
        let other = small("A==ENABLED (x'=1) /\\ x'=1", options);
        let mut p = pred(&m, "A");
        p.scope = pred(&other, "A").scope;
        assert!(dive_trace::enabled::enabled(&m, &p, None, &state(0)).is_err());
        assert!(
            dive_trace::enabled::enabled(&m, &pred(&m, "A"), None, &Arc::from([None])).is_err()
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
fn nested_queries_refuse_rust_providers_without_callbacks() {
    for options in [Options::tlc(), Options::dive()] {
        let m = small("A==ENABLED (x' \\in x) /\\ x'=0", options);
        let count = Arc::new(AtomicUsize::new(0));
        let set = dive_value::Value::deferred(Arc::new(Provider(count.clone())), options.ctx);
        let before: State = Arc::from([Some(set)]);
        for _ in 0..2 {
            assert!(dive_trace::enabled::enabled(&m, &pred(&m, "A"), None, &before).is_err());
        }
        assert_eq!(count.load(Ordering::SeqCst), 0);
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
fn periodic_query_labels_are_checked_and_cannot_hide_a_forged_next_edge() {
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
fn nested_queries_over_actual_recursive_domains() {
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
                let nested = pred(&m, "Nested");
                let vars = pred(&m, "vars");
                let result = dive_trace::enabled::enabled(&m, &nested, Some(vars.expr), &before)
                    .expect("decision");
                assert_eq!(result, !(values.is_empty() && done));
            }
        }
    }
}

#[test]
fn boolean_quantified_queries_decide_lasso_words() {
    let source = "---- MODULE NestedEnabled ----\nEXTENDS Naturals,Sequences\nVARIABLE x\nInit==x=0\nNext==x'=1\nQ==ENABLED (x'=7)\nFilterDomain(z)==\\E v \\in {i \\in {0}:z}:TRUE\nForallDomain(z)==\\A v \\in {i \\in {0}:z}:TRUE\nFunctionDomain(z)==[v \\in (IF z THEN {0} ELSE {})|->TRUE]\nMapDomain(z)=={TRUE:v \\in (IF z THEN {0} ELSE {})}\nFilterBase(z)=={v \\in (IF z THEN {0} ELSE {}):TRUE}\nFunctionData(z)==[v \\in {0}|->z]\nMapData(z)=={z:v \\in {0}}\nFilterData(z)=={v \\in {0}:z}\nChooseData(z)==CHOOSE v \\in {0}:z\nUnchangedData(z)==UNCHANGED z\nPrimeData(z)==z'\nKeep(v)==TRUE\nSequenceData(z)==SelectSeq(z,Keep)\nNativeData(z)==Len(z)\nParameterData(H(_),z)==H(z)\nParameterSource==ParameterData(Len,Q)\nExistsBody(z)==\\E v \\in {0}:z\nForallBody(z)==\\A v \\in {0}:z\nExistsDirect==\\E v \\in {7}:ENABLED (x'=v)\nForallDirect==\\A v \\in {7}:ENABLED (x'=v)\n====\n";
    for options in [Options::tlc(), Options::dive()] {
        let m = bind(source, "INIT Init\nNEXT Next\n", options);
        assert_eq!(dive_ir::validate::check(&m.ir), [] as [String; 0]);
        let hooks = Arc::new(Hooks::default());
        m.evaluator.set_host(hooks.clone());
        for name in ["ExistsDirect", "ForallDirect"] {
            let p = pred(&m, name);
            for _ in 0..2 {
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
fn temporal_expression_forwarding_keeps_queries_and_data_captures() {
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
                ("Hidden", true),
            ] {
                assert_eq!(
                    dive_trace::temporal::evaluate_lasso(&model, &pred(&model, name), &states, 0)
                        .expect(name),
                    vec![expected; states.len()],
                    "{name}"
                );
            }
            assert!(matches!(
                dive_trace::temporal::evaluate_lasso(
                    &model,
                    &pred(&model, "OperatorCapture"),
                    &states,
                    0
                ),
                Err(dive_trace::temporal::TemporalError::Unsupported(_))
            ));
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
            assert_eq!(model.evaluator.take_output(), [] as [String; 0]);
        }
    }
}
