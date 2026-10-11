//! Pure user operator formals retain captured scopes without forcing callbacks.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded fixture assertions"
)]
use dive_liveness::analyze::Limits;
use dive_model::{BindOptions, Pred};
use std::sync::Arc;

const SOURCE: &str = r"---- MODULE Operators ----
VARIABLE x
Init == x=FALSE
Next == UNCHANGED x
P(v) == v
Temporal(v) == []<>v
Use(F(_)) == []<>F(x)
Direct(F(_)) == F(x)
Relay(G(_)) == Use(G)
Capture(saved) == LET Local(v) == v = saved IN Use(Local)
Nested(F(_)) == LET Pass(G(_)) == Use(G) IN Pass(F)
N == Nested(P)
NT == Nested(Temporal)
Live == Use(P)
T == Direct(Temporal)
R == Relay(P)
L == LET Local(v) == v IN Use(Local)
C == Capture(TRUE)
====
";
fn input(source: &str) -> Arc<dive_ir::Ir> {
    let a = dive_sema::analyze_source(
        &format!("{}.tla", source.split_whitespace().nth(2).expect("module")),
        source,
        &Default::default(),
    );
    assert!(a.accepted(), "{a:?}");
    Arc::new(dive_ir::lower(&a).expect("lower"))
}

#[test]
fn retained_operator_formals_match_independent_words_and_closing_edges() {
    let ir = input(SOURCE);
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        let model = dive_model::bind(ir.clone(), &config, &options).expect("model");
        for n in 1..=4 {
            for mask in 0_usize..(1 << n) {
                let values: Vec<bool> = (0..n).map(|i| mask & (1 << i) != 0).collect();
                let states: Vec<dive_trace::State> = values
                    .iter()
                    .map(|b| Arc::from([Some((*b).into())]))
                    .collect();
                for back in 0..n {
                    let expected = vec![values[back..].iter().any(|b| *b); n];
                    for name in ["Live", "T", "R", "L", "C", "N", "NT"] {
                        let d = ir.root_def(name).expect("def");
                        let pred =
                            Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
                        let actual =
                            dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                                .expect(name);
                        assert_eq!(actual, expected, "{name} {values:?} {back}");
                        let plan =
                            dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                                .expect(name);
                        let bins: Vec<Vec<bool>> = (0..n)
                            .map(|i| {
                                let st = dive_eval::St::action(
                                    states[i].clone(),
                                    states[if i + 1 == n { back } else { i + 1 }].clone(),
                                );
                                plan.atoms
                                    .iter()
                                    .map(|a| a.evaluate(&model, &st).expect("atom"))
                                    .collect()
                            })
                            .collect();
                        assert_eq!(
                            plan.formula
                                .evaluate_word(&bins, &bins, back, 1_000_000)
                                .expect("formula"),
                            actual
                        );
                    }
                }
            }
        }
    }
}

fn number(raw: &str, before: &str, after: &str) -> u64 {
    raw.split(before)
        .nth(1)
        .expect("before")
        .split(after)
        .next()
        .expect("after")
        .parse()
        .expect("number")
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "complete paired selection and fresh independent mutants"
)]
fn clean_paired_primary_models_have_final_outcomes_with_fresh_validation() {
    use dive_api::tlc_temporal::Outcome;
    use dive_trace::{Ending, Validation};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/temporal-operators");
    let main: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("selection.json")).expect("selection"))
            .expect("json");
    let instance: serde_json::Value = serde_json::from_slice(
        &std::fs::read(base.join("selection-instance.json")).expect("selection"),
    )
    .expect("json");
    let mut outcomes = 0;
    for case in main
        .as_array()
        .expect("cases")
        .iter()
        .chain(instance.as_array().expect("instance"))
    {
        let name = case["name"].as_str().expect("name");
        let path = base.join("models").join(name).join(format!("{name}.tla"));
        let a = dive_sema::analyze_file(&path, &Default::default()).expect("analysis");
        assert!(a.accepted(), "{name}");
        let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
            .into_config()
            .expect("cfg");
        let raw = std::fs::read_to_string(
            base.join(if name.contains("Instance") || name.contains("Exported") {
                "clean-instance"
            } else {
                "clean-primary"
            })
            .join(name)
            .join("stdout"),
        )
        .expect("raw pin");
        let stats = raw
            .split("@!@!@STARTMSG 2199:0 @!@!@\n")
            .nth(1)
            .expect("stats");
        for workers in [1, 3, 4, 8] {
            outcomes += 1;
            let options = dive_check::CheckOptions {
                workers,
                ..dive_check::CheckOptions::tlc(0, 0)
            };
            let result =
                dive_api::tlc_temporal::check_final(&ir, &cfg, &options, Default::default());
            if name.contains("Effect") {
                let refusal = result.expect_err("effect before host");
                assert_eq!(refusal.workers.as_ref().expect("workers").used, 0);
                assert!(refusal.evidence.is_none());
                continue;
            }
            let report = result.expect(name);
            let (capture, violation) = match report.outcome() {
                Outcome::Holds(e) => (&e.execution.capture, None),
                Outcome::Violation(v) => (&v.candidate().evidence.execution.capture, Some(v)),
                Outcome::Safety(e) => {
                    assert!(matches!(e.result().verdict, dive_check::Verdict::Success));
                    continue;
                }
            };
            assert_eq!(
                capture.stats.generated,
                number(stats, "", " states generated"),
                "{name}"
            );
            assert_eq!(
                capture.stats.distinct,
                number(stats, "states generated, ", " distinct"),
                "{name}"
            );
            assert_eq!(
                capture.stats.depth,
                number(
                    &raw,
                    "The depth of the complete state graph search is ",
                    "."
                ),
                "{name}"
            );
            assert_eq!(capture.workers.used, workers);
            if name.ends_with("Holds") || name.contains("Constant") {
                assert!(violation.is_none(), "{name}");
                continue;
            }
            let v = violation.expect("actual primary13 word");
            assert_eq!(v.validation().validation, Validation::Valid);
            assert_eq!(v.document().properties, vec!["Live"]);
            let fresh =
                dive_model::bind(ir.clone(), &cfg, &BindOptions::tlc()).expect("fresh authority");
            let check = |doc: &dive_trace::TraceDoc| {
                dive_trace::validate_doc(&fresh, doc, &[], None)
                    .0
                    .validation
            };
            assert_eq!(check(v.document()), Validation::Valid);
            let mut bad = v.document().clone();
            bad.properties.clear();
            assert_ne!(check(&bad), Validation::Valid);
            let mut bad = v.document().clone();
            bad.properties = vec!["Foreign".into()];
            assert_ne!(check(&bad), Validation::Valid);
            let mut bad = v.document().clone();
            bad.states[0] =
                serde_json::from_value(serde_json::json!({"x":99})).expect("initial mutant");
            assert_ne!(check(&bad), Validation::Valid);
            let mut bad = v.document().clone();
            bad.states
                .push(serde_json::from_value(serde_json::json!({"x":99})).expect("closing mutant"));
            assert_ne!(check(&bad), Validation::Valid);
            let mut bad = v.document().clone();
            bad.ending = Some(Ending::Lasso {
                back_to: usize::MAX,
            });
            assert_ne!(check(&bad), Validation::Valid);
        }
    }
    assert_eq!(outcomes, 72);
}

#[test]
fn unused_opaque_effect_recursive_and_native_operators_do_not_gain_purity() {
    let source = r"---- MODULE NegativeOperators ----
EXTENDS TLC
VARIABLE x
Init==x=FALSE
Next==UNCHANGED x
Good(v)==v
Evil(v)==PrintT(v)
RECURSIVE Loop(_)
Loop(v)==Loop(v)
Unused(F(_))==[]<>x
Required(F(_))==[]<>F(x)
Effect==Unused(Evil)
Recursive==Unused(Loop)
Native==Unused(PrintT)
====
";
    let ir = input(source);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        let model = dive_model::bind(ir.clone(), &cfg, &options).expect("model");
        let states: Vec<dive_trace::State> = vec![Arc::from([Some(false.into())])];
        for name in ["Effect", "Recursive", "Native"] {
            let d = ir.root_def(name).expect("role");
            let pred = Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
            assert!(
                dive_liveness::plan::lower(&model, &pred, Limits::default().formula).is_err(),
                "{name}"
            );
            assert!(
                dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0).is_err(),
                "{name}"
            );
            assert_eq!(
                model.evaluator.take_output(),
                Vec::<String>::new(),
                "{name}"
            );
        }
        let good = ir.root_def("Good").expect("good");
        let scope = model.evaluator.scope(good).expect("scope");
        let (_, analysis) = model
            .evaluator
            .scope_operator_analysis(&scope, &dive_ir::OpRef::Def(good))
            .expect("analysis")
            .expect("known");
        assert!(matches!(
            model.evaluator.scope_slot(
                &analysis,
                dive_ir::LocalRef {
                    up: 0,
                    slot: dive_ir::Slot(0)
                }
            ),
            dive_eval::SlotView::AnalysisBound
        ));
        assert!(
            model
                .evaluator
                .eval_scoped(&analysis, ir.def(good).body, &dive_eval::St::constant())
                .is_err()
        );
        for operator in [
            dive_ir::OpRef::Builtin(dive_ir::BuiltinOp::Not),
            dive_ir::OpRef::Def(ir.root_def("Unused").expect("higher-order")),
        ] {
            assert!(
                model
                    .evaluator
                    .scope_operator_analysis(&scope, &operator)
                    .expect("refusal")
                    .is_none()
            );
        }
        assert!(
            model
                .evaluator
                .scope_operator_analysis(&scope, &dive_ir::OpRef::Def(dive_ir::DefId(u32::MAX)))
                .is_err()
        );
        let foreign =
            dive_model::bind(Arc::new(ir.as_ref().clone()), &cfg, &options).expect("foreign");
        assert!(
            model
                .evaluator
                .scope_operator_analysis(
                    &foreign.evaluator.scope(good).expect("foreign scope"),
                    &dive_ir::OpRef::Def(good)
                )
                .is_err()
        );
        let d = ir.root_def("Required").expect("Required");
        let pred = Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("unbound scope"),
        );
        assert!(dive_liveness::plan::lower(&model, &pred, Limits::default().formula).is_err());
        assert!(dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0).is_err());
    }
}

#[test]
fn configured_target_origin_and_runtime_domains_keep_named_boundaries() {
    let source = r"---- MODULE ConfiguredOperators ----
EXTENDS TLC
CONSTANT F(_)
VARIABLE x
Init==x=FALSE
Next==UNCHANGED x
Good(v)==v
Evil(v)==PrintT(v)
Zero==TRUE
Use(G(_))==[]<>G(x)
Domain(v)==\A y \in v:y=y
Live==Use(Good)
====
";
    let ir = input(source);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nCONSTANT F <- Good\n")
        .into_config()
        .expect("cfg");
    let model = dive_model::bind(ir.clone(), &cfg, &BindOptions::tlc()).expect("configured model");
    let good = ir.root_def("Good").expect("Good");
    let use_def = ir.root_def("Use").expect("Use");
    let caller = model.evaluator.scope(use_def).expect("caller");
    assert!(
        model
            .evaluator
            .scope_operator_analysis(&caller, &dive_ir::OpRef::Def(good))
            .expect("explicit same target")
            .is_none()
    );
    let passed = model
        .evaluator
        .scope_apply(
            &caller,
            use_def,
            &[dive_ir::Arg::Op {
                op: dive_ir::OpRef::Const(dive_ir::ConstId(0)),
                level: dive_ir::Level::Constant,
            }],
        )
        .expect("configured operator captured");
    assert!(
        model
            .evaluator
            .scope_operator_analysis(
                &passed,
                &dive_ir::OpRef::Param(dive_ir::LocalRef {
                    up: 0,
                    slot: dive_ir::Slot(0)
                })
            )
            .expect("erased configured origin")
            .is_none()
    );
    for profile in [BindOptions::tlc(), BindOptions::dive()] {
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nCONSTANT F <- Good\n")
            .into_config()
            .expect("cfg");
        let mut model = dive_model::bind(ir.clone(), &cfg, &profile).expect("fresh");
        let mut admitted = dive_eval::Model::new(ir.clone());
        admitted.names = model.names.clone();
        admitted.constants[0] = Some(dive_eval::ConstBinding::Def(good));
        admitted
            .redirects
            .insert(good, ir.root_def("Evil").expect("Evil"));
        model.evaluator =
            dive_eval::Evaluator::new(admitted, profile.eval).expect("redirect input");
        let d = ir.root_def("Live").expect("Live");
        let pred = Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
        let word = vec![Arc::from([Some(false.into())])];
        assert!(dive_liveness::plan::lower(&model, &pred, Limits::default().formula).is_err());
        assert!(dive_trace::temporal::evaluate_lasso(&model, &pred, &word, 0).is_err());
        assert_eq!(model.evaluator.take_output(), Vec::<String>::new());
        let mut admitted = dive_eval::Model::new(ir.clone());
        admitted.names = model.names.clone();
        admitted.constants[0] = Some(dive_eval::ConstBinding::Def(good));
        let zero = ir.root_def("Zero").expect("Zero");
        admitted.def_values.insert(zero, true.into());
        let evaluator = dive_eval::Evaluator::new(admitted, profile.eval).expect("value override");
        let scope = evaluator.scope(zero).expect("zero");
        assert!(
            evaluator
                .scope_operator_analysis(&scope, &dive_ir::OpRef::Def(zero))
                .expect("override refusal")
                .is_none()
        );
    }
    // Static analysis does not force the operator's unknown domain. Actual
    // predicate execution still enforces the original finite binding ceiling.
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    let mut admitted = dive_eval::Model::new(ir.clone());
    admitted.constants[0] = Some(dive_eval::ConstBinding::Def(good));
    let evaluator =
        dive_eval::Evaluator::new(admitted, dive_eval::Options::tlc()).expect("evaluator");
    let d = ir.root_def("Domain").expect("Domain");
    let root = evaluator.scope(d).expect("scope");
    let values = dive_value::Value::set(
        (0..5).map(dive_value::Value::Int).collect(),
        dive_value::Ctx::tlc(),
    )
    .expect("set");
    let scope = evaluator
        .scope_call_values(&root, d, vec![values])
        .expect("actual finite domain");
    assert!(
        evaluator
            .check_finite_bindings(&scope, ir.def(d).body, &dive_eval::St::constant(), 4)
            .is_err()
    );
    evaluator
        .check_finite_bindings(&scope, ir.def(d).body, &dive_eval::St::constant(), 5)
        .expect("actual bounded domain");
    let _ = cfg;
}

#[test]
fn erased_effect_annotations_never_authorize_hidden_native_operator_callbacks() {
    let source = r"---- MODULE ErasedOperatorEffects ----
EXTENDS TLC
VARIABLE x
Init==x=FALSE
Next==UNCHANGED x
Evil(v)==PrintT(v)
Unused(F(_))==[]<>x
Live==Unused(Evil)
====
";
    let original = input(source);
    let mut encoded = serde_json::to_value(original.as_ref()).expect("arena");
    for node in encoded["exprs"].as_array_mut().expect("exprs") {
        node["effects"] = serde_json::json!(0);
    }
    for def in encoded["defs"].as_array_mut().expect("defs") {
        def["effects"] = serde_json::json!(0);
    }
    let ir = Arc::new(serde_json::from_value::<dive_ir::Ir>(encoded).expect("same shape"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        let model = dive_model::bind(ir.clone(), &cfg, &options).expect("model");
        let d = ir.root_def("Live").expect("Live");
        let pred = Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
        let states: Vec<dive_trace::State> = vec![Arc::from([Some(false.into())])];
        assert!(dive_liveness::plan::lower(&model, &pred, Limits::default().formula).is_err());
        assert!(dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0).is_err());
        assert_eq!(model.evaluator.take_output(), Vec::<String>::new());
    }
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
        .into_config()
        .expect("cfg");
    let refusal = dive_api::tlc_temporal::check_final(
        &ir,
        &cfg,
        &dive_check::CheckOptions {
            workers: 4,
            ..dive_check::CheckOptions::tlc(0, 0)
        },
        Default::default(),
    )
    .expect_err("native authority before binding");
    assert_eq!(refusal.workers.expect("workers").used, 0);
    assert!(refusal.evidence.is_none());
}

#[test]
fn actual_operator_planning_keeps_existing_work_and_formula_bounds() {
    let ir = input(SOURCE);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
        .into_config()
        .expect("cfg");
    let model = dive_model::bind(ir.clone(), &cfg, &BindOptions::tlc()).expect("model");
    let d = ir.root_def("Live").expect("Live");
    let pred = Pred::new(ir.def(d).body, model.evaluator.scope(d).expect("scope"));
    let limits = dive_liveness::formula::Limits {
        expansion_steps: 1,
        ..Default::default()
    };
    assert_eq!(
        dive_liveness::plan::lower(&model, &pred, limits)
            .expect_err("work bound")
            .code(),
        "DIVE-L0021"
    );
    let mut limits = dive_liveness::phase_graph::Limits::default();
    limits.phase_l.formula.expansion_steps = 1;
    let refusal = dive_api::tlc_temporal::check_final(
        &ir,
        &cfg,
        &dive_check::CheckOptions {
            workers: 4,
            ..dive_check::CheckOptions::tlc(0, 0)
        },
        limits,
    )
    .expect_err("before pool");
    assert_eq!(refusal.workers.expect("workers").used, 0);
}
