//! Oracle-backed constant retry controls across native action demand and replay.
#![allow(
    clippy::expect_used,
    reason = "small pinned source regression controls"
)]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_engine};
use dive_core::compile::{BoundTest, compile_bound_test};
use dive_core::{Handle, Handoff, Machine, Opts, Role, Semantics, Store};
use dive_eval::{Evaluator, Model, Options, St};
use dive_value::{Ctx, Value};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/constant-retry-lazy/specifications")
        .join(name)
        .join(format!("{name}.tla"))
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one explicit source-bound handoff and demand protocol in both profiles"
)]
fn named_selection_is_true_but_whole_function_demand_still_errors() {
    let original = std::fs::read_to_string(fixture("NamedAction")).expect("pinned NamedAction");
    for tlc in [false, true] {
        for whole in [false, true] {
            let source = if whole {
                original.replace("F[0]=0", "F=F")
            } else {
                original.clone()
            };
            let analysis = dive_sema::analyze_source(
                "NamedAction.tla",
                &source,
                &dive_sema::Options::default(),
            );
            assert!(analysis.accepted(), "{analysis:?}");
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let options = if tlc { Options::tlc() } else { Options::dive() };
            let ev = Evaluator::new(Model::new(ir.clone()), options).expect("actual evaluator");
            let action = ir.root_def("A").expect("source action");
            let expr = ir.def(action).body;
            let scope = ev.scope(action).expect("actual root action scope");
            let constant = ir.root_def("F").expect("named function");
            assert_eq!(
                ev.precompute(constant)
                    .expect_err("genuine failed eager probe")
                    .ec()
                    .code(),
                2179
            );
            assert!(
                ev.guarded_action_stability_budgeted(&scope, expr, None)
                    .expect("closed optional source inspection")
            );
            let subscript = ir
                .nodes()
                .iter()
                .find_map(|node| match &node.expr {
                    dive_ir::Expr::Wf { sub, .. } => Some(*sub),
                    _ => None,
                })
                .expect("actual source fairness subscript");
            let ctx = if tlc { Ctx::tlc() } else { Ctx::dive() };
            let store = Arc::new(Store::with_semantics(
                true,
                if tlc {
                    Semantics::tlc()
                } else {
                    Semantics::default()
                },
            ));
            let handoff = Handoff {
                ir: &ir,
                ctx,
                action_composition: true,
                constants: vec![None; ir.consts().len()],
                redirects: HashMap::new(),
                std_redirects: HashMap::new(),
                precomputed: HashMap::new(),
                overrides: HashSet::new(),
                deferred: std::collections::BTreeSet::new(),
                startup_evaluator: None,
                assumptions: Vec::new(),
                init: Vec::new(),
                next: Vec::new(),
                invariants: Vec::new(),
                constraints: Vec::new(),
                action_constraints: Vec::new(),
                implied_inits: Vec::new(),
                implied_actions: vec![Role {
                    name: "A".into(),
                    expr,
                    def: action,
                    slots: Vec::new(),
                    substs: Vec::new(),
                }],
                check_deadlock: false,
                symmetry: Vec::new(),
                view: None,
            };
            let plan = compile_bound_test(
                &handoff,
                store.clone(),
                Opts::default(),
                BoundTest::Nonstuttering(subscript),
            )
            .expect("existing scoped compiler");
            let mut machine =
                Machine::new(store, 1, Opts::default(), dive_core::memo::Memo::default());
            for _ in 0..2 {
                machine.s0[0] = Handle::int(0).expect("before");
                machine.s1[0] = Handle::int(1).expect("after");
                machine.set_fuel(4096);
                let check = &plan.implied_actions[0];
                let frame = machine.enter(check.frame);
                let native = check.conjuncts.iter().try_fold(true, |holds, part| {
                    if holds {
                        machine.eval_bool(&plan.code, part.p)
                    } else {
                        Ok(false)
                    }
                });
                machine.leave(frame);
                machine.heap.reset();
                let reference = ev.eval_scoped(
                    &scope,
                    expr,
                    &St::action(
                        Arc::from([Some(Value::Int(0))]),
                        Arc::from([Some(Value::Int(1))]),
                    ),
                );
                if whole {
                    assert!(
                        native.is_err(),
                        "whole F=F demand must never supply native true"
                    );
                    assert_eq!(
                        reference
                            .expect_err("whole function really fails")
                            .ec()
                            .code(),
                        2179
                    );
                } else {
                    assert!(native.expect("only selected body demanded"));
                    assert_eq!(
                        reference.expect("fixed reference preserves selection"),
                        Value::TRUE
                    );
                }
            }
        }
    }
}

#[test]
fn three_oracle_cases_match_and_named_action_words_validate_freshly() {
    for tlc in [false, true] {
        for name in ["Named", "Inline", "NamedAction"] {
            let file = fixture(name);
            let request = CheckRequest {
                file: file.clone(),
                config: Some(file.with_extension("cfg")),
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                compat_tlc: tlc,
                ..CheckRequest::default()
            };
            let reference = check_file_engine(&request, Engine::Reference)
                .expect("reference")
                .to_json_with_exit_convention(ExitConvention::Tlc);
            let core = check_file_engine(&request, Engine::Core)
                .expect("native")
                .to_json_with_exit_convention(ExitConvention::Tlc);
            let expected = if name == "NamedAction" {
                "liveness-violation"
            } else {
                "success"
            };
            for report in [&reference, &core] {
                assert_eq!(report["verdict"], expected, "{name}/{tlc}: {report}");
                assert_eq!(report["statistics"]["distinct"], 2);
                assert_eq!(report["statistics"]["generated"], 3);
                assert_eq!(report["statistics"]["depth"], 2);
                assert_eq!(
                    report["exit_code"],
                    if name == "NamedAction" { 13 } else { 0 }
                );
            }
            if name == "NamedAction" {
                assert_eq!(core["property"], "P");
                assert!(
                    core["core"]["metrics"]["liveness"]["atoms"]["nonstuttering_evaluations"]
                        .as_u64()
                        .is_some_and(|n| n > 0),
                    "actual native wrapper consumed: {core}"
                );
                let validator = SpecModel::load(
                    &file,
                    &file.with_extension("cfg"),
                    &TraceOptions {
                        compat_tlc: tlc,
                        run: Some((0, 0)),
                        ..TraceOptions::default()
                    },
                )
                .expect("fresh independent validator");
                let doc = &core["counterexample"]["trace"];
                let word = TraceDoc::from_json(doc).expect("actual word");
                assert!(matches!(
                    validator.validate(&word).validation,
                    Validation::Valid
                ));
                let mut wrong = doc.clone();
                wrong["states"][0]["values"]["x"] = serde_json::json!(42);
                let wrong = TraceDoc::from_json(&wrong).expect("typed initial mutation");
                assert!(matches!(
                    validator.validate(&wrong).validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
}
