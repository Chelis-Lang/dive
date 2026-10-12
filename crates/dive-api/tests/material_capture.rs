//! Stored captured values: material context and ordering refusals.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_check::CheckOptions;
use std::{path::PathBuf, sync::Arc};
fn base() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/material-capture")
}
fn input(name: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let b = base().join("models").join(name);
    let a = dive_sema::analyze_file(
        &b.join(format!("{name}.tla")),
        &dive_sema::Options::default(),
    )
    .expect("source");
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg =
        dive_config::parse(&std::fs::read_to_string(b.join(format!("{name}.cfg"))).expect("cfg"))
            .into_config()
            .expect("config");
    (ir, cfg)
}
#[test]
fn material_context_and_predicate_budget_refuse_before_pool_work() {
    let options = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let (ir, cfg) = input("MatNestedFails");
    for names in [false, true] {
        let mut model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())
            .expect("fresh model");
        if names {
            model.names = Arc::new(dive_value::Interner::new());
        } else {
            model.ir = Arc::new(ir.as_ref().clone());
        }
        let result = dive_check::capture::explore_concrete_parallel_with_evidence(
            &model,
            &options,
            dive_check::capture::Limits::default(),
        )
        .expect("typed outcome");
        assert!(
            matches!(result,dive_check::capture::Exploration::Stopped(ref r) if matches!(&r.verdict,dive_check::Verdict::Unsupported{feature}if feature.contains("DIVE-EVAL-C004")) && r.workers.used==0 && r.stats.is_none())
        );
        assert_eq!(model.evaluator.take_output(), Vec::<String>::new());
    }
    let a = dive_sema::analyze_source(
        "Bound.tla",
        "---- MODULE Bound ----\nVARIABLE x\nInit == x = TRUE\nR(v) == v = v /\\ x' = x\nNext == R(TRUE)\n====\n",
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    let mut model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    let def = model.ir.root_def("R").expect("R");
    let scope = model.evaluator.scope(def).expect("owned scope");
    let names = dive_value::Interner::new();
    let foreign = dive_value::Value::Str(names.intern("foreign"));
    assert!(
        model
            .evaluator
            .scope_bind(&scope, dive_ir::Slot(0), foreign)
            .is_err()
    );
    let value = dive_value::Value::tuple(vec![dive_value::Value::Int(0); 24_999]);
    let scope = model
        .evaluator
        .scope_bind(&scope, dive_ir::Slot(0), value)
        .expect("admitted stored value within separate evaluator context budget");
    model.next = Some(dive_model::Pred::new(model.ir.def(def).body, scope));
    let result = dive_check::capture::explore_concrete_parallel_with_evidence(
        &model,
        &options,
        dive_check::capture::Limits::default(),
    )
    .expect("bounded admission");
    assert!(
        matches!(result,dive_check::capture::Exploration::Stopped(ref r) if matches!(r.verdict,dive_check::Verdict::Unsupported{..}) && r.workers.used==0 && r.stats.is_none())
    );
    assert_eq!(model.evaluator.take_output(), Vec::<String>::new());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "paired public profile and stored representation admission"
)]
fn opposite_profile_stored_order_refuses_before_work_but_tuple_domains_remain_invariant() {
    let a = dive_sema::analyze_source(
        "Order.tla",
        "---- MODULE Order ----\nVARIABLE x\nUnused == \"zeta\"\nOther == \"alpha\"\nInit == x = TRUE\nR(v) == v = v /\\ x' = x\nNext == R(TRUE)\n====\n",
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("config");
    for tlc in [true, false] {
        for matched in [true, false] {
            let bind = if tlc {
                dive_model::BindOptions::tlc()
            } else {
                dive_model::BindOptions::default()
            };
            let options = CheckOptions {
                workers: 4,
                ..if tlc {
                    CheckOptions::tlc(0, 0)
                } else {
                    CheckOptions::dive(0)
                }
            };
            for kind in ["enum", "raw", "record", "general", "tuple"] {
                let mut model = dive_model::bind(ir.clone(), &cfg, &bind).expect("model");
                let run = model.evaluator.options().ctx;
                let ctx = dive_value::Ctx {
                    order: if matched {
                        run.order
                    } else {
                        match run.order {
                            dive_value::ValueOrder::Tlc => dive_value::ValueOrder::Canonical,
                            dive_value::ValueOrder::Canonical => dive_value::ValueOrder::Tlc,
                        }
                    },
                    ..run
                };
                let z = model.names.intern("zeta");
                let a = model.names.intern("alpha");
                let values = vec![
                    dive_value::Value::Str(z.clone()),
                    dive_value::Value::Str(a.clone()),
                ];
                let value = match kind {
                    "enum" => dive_value::Value::set(values, ctx).expect("set"),
                    "raw" => dive_value::Value::set_raw(values, ctx),
                    "record" => dive_value::Value::record(
                        vec![
                            (z, dive_value::Value::Int(1)),
                            (a, dive_value::Value::Int(2)),
                        ],
                        ctx,
                    )
                    .expect("record"),
                    "general" => dive_value::Value::fcn(
                        vec![
                            (dive_value::Value::Int(1), dive_value::Value::Str(z)),
                            (dive_value::Value::Int(3), dive_value::Value::Str(a)),
                        ],
                        ctx,
                    )
                    .expect("sparse function"),
                    "tuple" => dive_value::Value::tuple(values),
                    _ => panic!("selected kind"),
                };
                let def = model.ir.root_def("R").expect("R");
                let scope = model.evaluator.scope(def).expect("scope");
                let scope = model
                    .evaluator
                    .scope_bind(&scope, dive_ir::Slot(0), value)
                    .expect("same-owned material context");
                model.actions = vec![
                    model
                        .evaluator
                        .predicate_in(
                            &scope,
                            model.ir.def(def).body,
                            dive_ir::actions::ActionName::Def(def),
                            model.init.len(),
                        )
                        .expect("action"),
                ];
                model.next = Some(dive_model::Pred::new(model.ir.def(def).body, scope));
                let result = dive_check::capture::explore_concrete_parallel_with_evidence(
                    &model,
                    &options,
                    dive_check::capture::Limits::default(),
                )
                .expect("typed result");
                if matched || kind == "tuple" {
                    assert!(
                        matches!(result, dive_check::capture::Exploration::Complete(_)),
                        "{tlc}/{matched}/{kind}: {result:?}"
                    );
                } else {
                    assert!(
                        matches!(result,dive_check::capture::Exploration::Stopped(ref r) if matches!(r.verdict,dive_check::Verdict::Unsupported{..}) && r.workers.used==0 && r.stats.is_none()),
                        "{tlc}/{kind}: {result:?}"
                    );
                }
                assert_eq!(model.evaluator.take_output(), Vec::<String>::new());
            }
        }
    }
}
