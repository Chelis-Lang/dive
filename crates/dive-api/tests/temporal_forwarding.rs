//! Temporal expression arguments retain their caller scopes on each periodic word.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::temporal::{self, Outcome};
use dive_check::CheckOptions;
use dive_liveness::analyze::Limits;
use dive_model::{BindOptions, Pred};
use dive_store::StoreMode;
use dive_trace::Validation;
use std::sync::Arc;

fn input(source: &str, cfg: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let a = dive_sema::analyze_source(
        &format!("{}.tla", source.split_whitespace().nth(2).expect("module")),
        source,
        &dive_sema::Options::default(),
    );
    assert!(a.accepted(), "{a:?}");
    (
        Arc::new(dive_ir::lower(&a).expect("lower")),
        dive_config::parse(cfg).into_config().expect("cfg"),
    )
}
const SOURCE: &str = r"---- MODULE Forward ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Identity(p) == p
Negate(p) == ~p
Combine(p,q) == p /\ q
Relay(p) == Identity(p)
Saved(p) == LET Local == p IN Relay(Local)
Local(p) == LET Pass(q) == Saved(q) IN Pass(p)
P == Identity([]<>x)
Q == Saved([]<>x)
R == Local([]<>x)
S == Negate(<>[](~x))
T == Combine([]<>x, []<>(~x))
U == Identity(\A v \in {TRUE,FALSE}: []<>(x=v))
V == IF Identity([]<>x) THEN Saved([]<>(~x)) ELSE FALSE
W == Identity([](ENABLED Next))
X == Negate([](ENABLED Next))
Y == Identity(WF_x(Next))
====
";

#[test]
fn forwarding_preserves_truth_at_all_positions_and_closing_edges() {
    let (ir, cfg) = input(SOURCE, "INIT Init\nNEXT Next\n");
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        let model = dive_model::bind(ir.clone(), &cfg, &options).expect("fresh model");
        for n in 1..=4 {
            for mask in 0_usize..(1 << n) {
                let vals: Vec<bool> = (0..n).map(|i| mask & (1 << i) != 0).collect();
                let states: Vec<dive_trace::State> = vals
                    .iter()
                    .map(|v| Arc::from([Some((*v).into())]))
                    .collect();
                for back in 0..n {
                    for name in ["P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y"] {
                        let truth = vals[back..].iter().any(|v| *v)
                            && (!matches!(name, "T" | "U" | "V" | "Y")
                                || vals[back..].iter().any(|v| !*v));
                        let truth = match name {
                            "W" => true,
                            "X" => false,
                            _ => truth,
                        };
                        let d = ir.root_def(name).expect(name);
                        let pred = Pred::new(
                            ir.def(d).body,
                            model.evaluator.scope(d).expect("valid scope"),
                        );
                        let direct =
                            dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                                .expect(name);
                        assert_eq!(direct, vec![truth; n], "{name} {vals:?} {back}");
                        let plan =
                            dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                                .expect(name);
                        let rows: Vec<Vec<bool>> = (0..n)
                            .map(|i| {
                                let next = if i + 1 == n { back } else { i + 1 };
                                let st =
                                    dive_eval::St::action(states[i].clone(), states[next].clone());
                                plan.atoms
                                    .iter()
                                    .map(|a| a.evaluate(&model, &st).expect(name))
                                    .collect()
                            })
                            .collect();
                        assert_eq!(
                            plan.formula
                                .evaluate_word(&rows, &rows, back, 1_000_000)
                                .expect(name),
                            direct
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn final_verdicts_and_fresh_witness_validation_keep_forwarded_properties() {
    for name in ["P", "Q", "R", "S", "T", "U", "V"] {
        let (ir, cfg) = input(SOURCE, &format!("INIT Init\nNEXT Next\nPROPERTY {name}\n"));
        for mut options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Exact, StoreMode::Fingerprint] {
                options.store = store;
                let report =
                    temporal::check_final(&ir, &cfg, &options, Limits::default()).expect(name);
                let Outcome::Violation(v) = report.outcome() else {
                    panic!("unfair stutter violates {name}: {report:?}");
                };
                assert_eq!(v.validation().validation, Validation::Valid);
            }
        }
    }
}

#[test]
fn forwarding_never_runs_impure_or_opaque_operator_arguments() {
    let src = r#"---- MODULE Forward ----
EXTENDS TLC
VARIABLE x
Init == x=FALSE
Next == x'=~x
Identity(p) == p
Hidden(p) == PrintT(p)
Higher(P(_)) == []<>P(x)
P == Identity([]<>Hidden(x))
Q == Identity([]<>(TLCGet("level")=1))
R == Identity(Higher(Hidden))
====
"#;
    let (ir, cfg) = input(src, "INIT Init\nNEXT Next\n");
    let model = dive_model::bind(ir.clone(), &cfg, &BindOptions::tlc()).expect("model");
    for name in ["P", "Q", "R"] {
        let d = ir.root_def(name).expect(name);
        let pred = Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
        assert!(
            dive_liveness::plan::lower(&model, &pred, Limits::default().formula).is_err(),
            "{name}"
        );
        assert!(
            dive_trace::temporal::evaluate_lasso(
                &model,
                &pred,
                &[Arc::from([Some(false.into())])],
                0
            )
            .is_err(),
            "{name}"
        );
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}

#[test]
fn forwarded_context_walk_is_bounded_without_forcing_temporal_values() {
    let (ir, cfg) = input(SOURCE, "INIT Init\nNEXT Next\n");
    let model = dive_model::bind(ir.clone(), &cfg, &BindOptions::tlc()).expect("model");
    let d = ir.root_def("R").expect("R");
    let p = Pred::new(
        ir.def(d).body,
        model.evaluator.scope(d).expect("valid scope"),
    );
    let mut limits = Limits::default().formula;
    limits.expansion_steps = 3;
    let error =
        dive_liveness::plan::lower(&model, &p, limits).expect_err("no proof under tiny budget");
    assert_eq!(error.code(), "DIVE-L0021");
    assert_eq!(model.take_output(), Vec::<String>::new());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "complete paired profile/store/worker census and independent negative controls"
)]
fn pinned_paired_forwarding_controls_keep_full_census_and_reference_refusals() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/temporal-forwarding");
    for kind in [
        "Identity",
        "Nested",
        "Boolean",
        "InstanceBody",
        "EnabledLive",
        "Instance",
    ] {
        for holds in [true, false] {
            let name = format!("Tf{kind}{}", if holds { "Holds" } else { "Fails" });
            let src =
                std::fs::read_to_string(root.join("specs").join(&name).join(format!("{name}.tla")))
                    .expect("source");
            let cfg =
                std::fs::read_to_string(root.join("specs").join(&name).join(format!("{name}.cfg")))
                    .expect("cfg");
            let stdout = std::fs::read_to_string(root.join("tlc").join(format!("{name}.txt")))
                .expect("raw oracle");
            let exit = std::fs::read_to_string(root.join("tlc").join(format!("{name}.exit")))
                .expect("oracle exit");
            assert_eq!(
                exit.trim(),
                if kind == "Instance" {
                    "255"
                } else if holds {
                    "0"
                } else {
                    "13"
                }
            );
            let (ir, cfg) = input(&src, &cfg);
            for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
                for store in [StoreMode::Exact, StoreMode::Fingerprint] {
                    for workers in [1, 4, 8] {
                        let options = CheckOptions {
                            store,
                            workers,
                            ..base.clone()
                        };
                        let report = temporal::check_final(&ir, &cfg, &options, Limits::default());
                        if kind == "Instance" {
                            assert!(stdout.contains("TLC cannot handle the temporal formula"));
                            assert!(
                                report
                                    .expect_err("reference refusal remains a refusal")
                                    .to_string()
                                    .contains("INSTANCE")
                            );
                            let bind = if base.profile == dive_eval::Profile::Tlc {
                                BindOptions::tlc()
                            } else {
                                BindOptions::dive()
                            };
                            let model = dive_model::bind(ir.clone(), &cfg, &bind)
                                .expect("fresh independent model");
                            let d = ir.root_def("P").expect("property");
                            let pred = Pred::new(
                                ir.def(d).body,
                                model.evaluator.scope(d).expect("valid scope"),
                            );
                            assert!(
                                dive_trace::temporal::evaluate_lasso(
                                    &model,
                                    &pred,
                                    &[Arc::from([Some(false.into())])],
                                    0
                                )
                                .expect_err("independent reference refusal")
                                .to_string()
                                .contains("INSTANCE")
                            );
                            continue;
                        }
                        assert!(stdout.contains("3 states generated, 2 distinct states found"));
                        let report = report.expect(&name);
                        let capture = match report.outcome() {
                            Outcome::Holds(c) => {
                                assert!(holds);
                                c.as_ref()
                            }
                            Outcome::Violation(v) => {
                                assert!(!holds);
                                assert_eq!(v.validation().validation, Validation::Valid);
                                let bind = if base.profile == dive_eval::Profile::Tlc {
                                    BindOptions::tlc()
                                } else {
                                    BindOptions::dive()
                                };
                                let model = dive_model::bind(ir.clone(), &cfg, &bind)
                                    .expect("fresh independent model");
                                let doc = dive_trace::TraceDoc::parse(
                                    &v.document().to_json().to_string(),
                                )
                                .expect("lossless");
                                assert_eq!(
                                    dive_trace::validate_doc(&model, &doc, &cfg.model_values)
                                        .0
                                        .validation,
                                    Validation::Valid
                                );
                                let mut bad_init = doc.clone();
                                bad_init.states[0]
                                    .insert("x".into(), serde_json::Value::Bool(true));
                                assert!(matches!(
                                    dive_trace::validate_doc(&model, &bad_init, &cfg.model_values)
                                        .0
                                        .validation,
                                    Validation::Invalid(_)
                                ));
                                let mut bad_claim = doc.clone();
                                bad_claim.properties = vec!["AbsentClaim".into()];
                                assert_ne!(
                                    dive_trace::validate_doc(&model, &bad_claim, &cfg.model_values)
                                        .0
                                        .validation,
                                    Validation::Valid
                                );
                                let mut bad_loop = doc.clone();
                                bad_loop.ending = Some(dive_trace::Ending::Lasso {
                                    back_to: bad_loop.states.len(),
                                });
                                assert!(matches!(
                                    dive_trace::validate_doc(&model, &bad_loop, &cfg.model_values)
                                        .0
                                        .validation,
                                    Validation::Invalid(_)
                                ));

                                // Fresh periodic truth is checked separately from whole-behavior validation.
                                let d = ir.root_def("P").expect("property");
                                let pred = Pred::new(
                                    ir.def(d).body,
                                    model.evaluator.scope(d).expect("valid scope"),
                                );
                                let alternating: Vec<dive_trace::State> = [false, true]
                                    .into_iter()
                                    .map(|v| Arc::from([Some(v.into())]))
                                    .collect();
                                assert!(
                                    !dive_trace::temporal::evaluate_lasso(
                                        &model,
                                        &pred,
                                        &alternating,
                                        0
                                    )
                                    .expect("actual violated property")[0]
                                );
                                &v.candidate().exploration
                            }
                            Outcome::PrefixViolation(prefix) => {
                                panic!("final API returned a prefix: {prefix:?}")
                            }
                            Outcome::Safety(s) => panic!("{name}: {:?}", s.verdict),
                        };
                        assert_eq!(
                            (
                                capture.stats.generated,
                                capture.stats.distinct,
                                capture.stats.depth
                            ),
                            (3, 2, 2)
                        );
                        assert_eq!(
                            (capture.workers.requested, capture.workers.used),
                            (workers, workers)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn opaque_rust_operator_capture_cannot_hide_under_a_temporal_argument() {
    let (ir, cfg) = input(
        r"---- MODULE Forward ----
EXTENDS TLC
VARIABLE x
Init == x=FALSE
Next == x'=~x
Identity(p) == p
Capture(Op(_)) == Identity([]<>Op(x))
P == []<>x
====
",
        "INIT Init\nNEXT Next\n",
    );
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        let model = dive_model::bind(ir.clone(), &cfg, &options).expect("model");
        let d = ir.root_def("Capture").expect("capturing operator");
        let caller = model
            .evaluator
            .scope(ir.root_def("P").expect("caller"))
            .expect("valid scope");
        let scope = model
            .evaluator
            .scope_apply(
                &caller,
                d,
                &[dive_ir::Arg::Op {
                    op: dive_ir::OpRef::Std(dive_ir::StdOp::TlcPrintT),
                    level: dive_ir::Level::Constant,
                }],
            )
            .expect("opaque caller-supplied operator");
        let pred = Pred::new(ir.def(d).body, scope);
        assert!(dive_liveness::plan::lower(&model, &pred, Limits::default().formula).is_err());
        assert!(
            dive_trace::temporal::evaluate_lasso(
                &model,
                &pred,
                &[Arc::from([Some(false.into())])],
                0
            )
            .is_err()
        );
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}

#[test]
fn state_only_forwarded_enabled_controls_preserve_safety_classification() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/temporal-forwarding");
    for holds in [true, false] {
        let name = format!("TfEnabled{}", if holds { "Holds" } else { "Fails" });
        let source =
            std::fs::read_to_string(root.join("specs").join(&name).join(format!("{name}.tla")))
                .expect("source");
        let cfg =
            std::fs::read_to_string(root.join("specs").join(&name).join(format!("{name}.cfg")))
                .expect("cfg");
        let exit =
            std::fs::read_to_string(root.join("tlc").join(format!("{name}.exit"))).expect("exit");
        assert_eq!(exit.trim(), if holds { "0" } else { "12" });
        let (ir, cfg) = input(&source, &cfg);
        for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            let result = dive_check::check(&ir, &cfg, &options);
            if holds {
                assert!(matches!(result.verdict, dive_check::Verdict::Success));
            } else {
                assert!(
                    matches!(
                        result.verdict,
                        dive_check::Verdict::InvariantViolated { initial: true, .. }
                    ),
                    "{result:?}"
                );
                assert!(!result.tlc_prints_stats);
            }
        }
    }
}

#[test]
fn unchecked_expression_and_definition_endpoints_refuse_without_panics() {
    let (ir, cfg) = input(SOURCE, "INIT Init\nNEXT Next\n");
    for options in [BindOptions::tlc(), BindOptions::dive()] {
        for capture in [false, true] {
            let model = dive_model::bind(ir.clone(), &cfg, &options).expect("model");
            let p = ir.root_def("P").expect("caller");
            let pred = Pred::new(
                dive_ir::ExprId(u32::MAX),
                model.evaluator.scope(p).expect("valid scope"),
            );
            if capture {
                let d = ir.root_def("Identity").expect("operator");
                let error = model
                    .evaluator
                    .scope_apply(
                        &pred.scope,
                        d,
                        &[dive_ir::Arg::Expr {
                            expr: dive_ir::ExprId(u32::MAX),
                            pass: dive_ir::Pass::Lazy,
                        }],
                    )
                    .expect_err("malformed lazy capture rejected before evaluation");
                assert!(error.message().contains("DIVE-EVAL-C001"));
                assert_eq!(model.take_output(), Vec::<String>::new());
                continue;
            }
            let plan = dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                .expect_err("unchecked ID cannot certify");
            assert_eq!(plan.code(), "DIVE-L0020");
            assert!(matches!(
                dive_trace::temporal::evaluate_lasso(
                    &model,
                    &pred,
                    &[Arc::from([Some(false.into())])],
                    0
                ),
                Err(dive_trace::temporal::TemporalError::Unsupported(_))
            ));
            assert_eq!(model.take_output(), Vec::<String>::new());
        }
        // Model.ir is public and IR is deserializable. Keep a valid evaluator
        // but supply a planner-facing definition reference outside that IR.
        let mut model = dive_model::bind(ir.clone(), &cfg, &options).expect("model");
        let p = ir.root_def("P").expect("property");
        let body = ir.def(p).body;
        let mut json = serde_json::to_value(ir.as_ref()).expect("IR serialization");
        json["exprs"][body.index()]["expr"] = serde_json::json!({"Def":u32::MAX});
        model.ir = Arc::new(serde_json::from_value(json).expect("unchecked IR"));
        let pred = Pred::new(body, model.evaluator.scope(p).expect("valid scope"));
        assert_eq!(
            dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                .expect_err("bad definition")
                .code(),
            "DIVE-L0020"
        );
        assert!(matches!(
            dive_trace::temporal::evaluate_lasso(
                &model,
                &pred,
                &[Arc::from([Some(false.into())])],
                0
            ),
            Err(dive_trace::temporal::TemporalError::Unsupported(_))
        ));
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}
