//! Stored captured values have pinned final outcomes and independent witness controls.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::tlc_temporal::{self, Outcome};
use dive_check::CheckOptions;
use dive_liveness::phase_graph::Limits;
use dive_trace::{Ending, Validation};
use std::{path::PathBuf, sync::Arc};
fn base() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/probes/material-capture")
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
fn raw_field(raw: &str, prefix: &str, suffix: &str) -> u64 {
    raw.split(prefix)
        .nth(1)
        .expect("field prefix")
        .split(suffix)
        .next()
        .expect("field suffix")
        .parse()
        .expect("integer")
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "all selected paired outcomes and independent witness mutants"
)]
fn all_material_pairs_match_clean_pin_with_fresh_memory_and_json_replay() {
    let selection: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(base().join("selection.json")).expect("selection"),
    )
    .expect("JSON");
    let general: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(base().join("selection-general.json")).expect("sparse selection"),
    )
    .expect("JSON");
    let mut count = 0;
    for row in selection
        .as_array()
        .expect("rows")
        .iter()
        .chain(general.as_array().expect("sparse rows"))
    {
        let name = row["name"].as_str().expect("name");
        let (ir, cfg) = input(name);
        let raw = std::fs::read_to_string(
            base()
                .join(if name.starts_with("MatGeneralFunction") {
                    "clean-general"
                } else {
                    "clean-tlc"
                })
                .join(name)
                .join("stdout"),
        )
        .expect("raw TLC");
        let summary = raw
            .split("@!@!@STARTMSG 2199:0 @!@!@\n")
            .nth(1)
            .expect("stats");
        let generated = raw_field(summary, "", " states generated");
        let distinct = raw_field(summary, "states generated, ", " distinct");
        let depth = raw_field(
            &raw,
            "The depth of the complete state graph search is ",
            ".",
        );
        for workers in [1, 3, 4, 8] {
            count += 1;
            let opts = CheckOptions {
                workers,
                ..CheckOptions::tlc(0, 0)
            };
            let report = tlc_temporal::check_final(&ir, &cfg, &opts, Limits::default())
                .expect("stored captured values");
            let (e, violation) = match report.outcome() {
                Outcome::Holds(e) => (&e.execution, None),
                Outcome::Violation(v) => (&v.candidate().evidence.execution, Some(v.as_ref())),
                Outcome::Safety(_) => panic!("Live is temporal"),
            };
            assert_eq!(e.capture.stats.generated, generated, "{name}");
            assert_eq!(e.capture.stats.distinct, distinct, "{name}");
            assert_eq!(e.capture.stats.depth, depth, "{name}");
            assert_eq!(e.capture.workers.used, workers);
            assert_eq!(
                e.configured_properties
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["Live"]
            );
            assert_eq!(e.expanded_property_names, vec!["Live"]);
            assert_eq!(e.role_names, vec!["Live"]);
            assert_eq!(report.scheduling(), "final-only");
            if row["expected_exit"] == 0 {
                assert!(violation.is_none(), "{name}");
                continue;
            }
            let v = violation.expect("private validated violation");
            assert_eq!(v.validation().validation, Validation::Valid);
            assert_eq!(v.document().properties, vec!["Live"]);
            let pinned: Vec<bool> = raw
                .split("@!@!@STARTMSG 2217:4 @!@!@")
                .skip(1)
                .map(|s| s.lines().find_map(|l| l.strip_prefix("x = ")).expect("x") == "TRUE")
                .collect();
            let actual: Vec<bool> = v
                .candidate()
                .states
                .iter()
                .map(|s| s[0].as_ref().expect("x").as_bool().expect("Boolean"))
                .collect();
            assert_eq!(actual, pinned, "{name} pinned selected word");
            assert!(matches!(v.document().ending, Some(Ending::Stuttering)));
            let fresh = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())
                .expect("independent bind");
            let check = |doc: &dive_trace::TraceDoc| {
                dive_trace::validate_doc(&fresh, doc, &[], None)
                    .0
                    .validation
            };
            assert_eq!(check(v.document()), Validation::Valid);
            let encoded = serde_json::to_string(&v.document().to_json()).expect("JSON");
            let decoded = dive_trace::TraceDoc::parse(&encoded).expect("fresh JSON");
            assert_eq!(check(&decoded), Validation::Valid);
            let mut wrong = decoded.clone();
            wrong.states[0] = serde_json::from_value(serde_json::json!({"x":99})).expect("mutant");
            assert_ne!(check(&wrong), Validation::Valid, "{name} Init mutant");
            let mut wrong = decoded.clone();
            wrong
                .states
                .push(serde_json::from_value(serde_json::json!({"x":99})).expect("closing mutant"));
            assert_ne!(
                check(&wrong),
                Validation::Valid,
                "{name} Next/closing mutant"
            );
            let mut wrong = decoded.clone();
            wrong.properties.clear();
            assert_ne!(check(&wrong), Validation::Valid, "{name} empty claim");
            let mut wrong = decoded.clone();
            wrong.properties = vec!["NoSuchProperty".into()];
            assert_ne!(check(&wrong), Validation::Valid, "{name} foreign claim");
            let mut wrong = decoded;
            wrong.ending = Some(Ending::Lasso {
                back_to: usize::MAX,
            });
            assert_ne!(check(&wrong), Validation::Valid, "{name} ending mutant");
        }
    }
    assert_eq!(count, 72);
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
