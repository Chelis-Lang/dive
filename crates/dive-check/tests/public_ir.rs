//! Malformed public IR and bound-model metadata refuse before effects.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_check::{
    CheckOptions, CheckResult, Verdict,
    capture::{self, Exploration, Limits},
    checkpoint,
};
use dive_ir::Ir;
use std::sync::Arc;

const SOURCE: &str = "---- MODULE PublicArena ----\nEXTENDS Naturals, TLC\nVARIABLE x\nASSUME Print(\"should not run\", TRUE)\nInit == x = 0\nNext == x' = x\nInv == x = x\n====\n";
fn ir(source: &str) -> Arc<Ir> {
    let a = dive_sema::analyze_source("PublicArena.tla", source, &dive_sema::Options::default());
    Arc::new(dive_ir::lower(&a).expect("IR"))
}
fn config() -> dive_config::ModelConfig {
    dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\n")
        .into_config()
        .expect("config")
}
fn no_work(r: &CheckResult, code: &str) {
    assert!(
        matches!(&r.verdict, Verdict::Unsupported{feature} if feature.contains(code)),
        "{r:?}"
    );
    assert_eq!(r.workers.used, 0);
    assert!(r.stats.is_none() && r.counterexample.is_none());
    assert!(r.output.is_empty() && r.warnings.is_empty());
}
fn mutate(base: &Arc<Ir>, n: usize) -> Arc<Ir> {
    let mut j = serde_json::to_value(base.as_ref()).expect("serialize");
    match n {
        0 => j["vars"] = serde_json::json!([u32::MAX]),
        1 => j["consts"] = serde_json::json!([u32::MAX]),
        2 => j["root"] = serde_json::json!(u32::MAX),
        3 => j["defs"][0]["body"] = serde_json::json!(u32::MAX),
        4 => j["tlc_vars"] = serde_json::json!([u32::MAX]),
        5 => j["modules"][0]["assumptions"] = serde_json::json!([u32::MAX]),
        6 => j["modules"][0]["context"][0][1] = serde_json::json!({"Decl": u32::MAX}),
        _ => panic!("bad control"),
    }
    Arc::new(serde_json::from_value(j).expect("deserialize"))
}
#[test]
fn malformed_arena_check_checkpoint_and_binding_admit_before_effects() {
    let base = ir(SOURCE);
    let cfg = config();
    for n in 0..7 {
        let bad = mutate(&base, n);
        let error = dive_eval::validate_ir(&bad).expect_err("typed refusal");
        for requested in [0, 1, 4, 65] {
            let opts = CheckOptions {
                workers: requested,
                ..CheckOptions::tlc(0, 7)
            };
            let ordinary = dive_check::check(&bad, &cfg, &opts);
            no_work(&ordinary, error.code());
            assert_eq!(ordinary.workers.requested, requested.max(1));
            let request = checkpoint::Request {
                path: "".into(),
                action: checkpoint::Action::Recover,
                counters: checkpoint::Counters::Tlc,
                config_source_sha256: None,
            };
            let native = checkpoint::check(&bad, &cfg, &opts, &request);
            no_work(&native.check, error.code());
            assert!(native.receipt.is_none());
        }
        let failure =
            dive_model::bind(bad, &cfg, &dive_model::BindOptions::tlc()).expect_err("bind refusal");
        assert!(failure.error.message.contains(error.code()));
        assert_eq!(failure.output, Vec::<dive_model::Message>::new());
    }
}
#[test]
fn foreign_capture_metadata_and_actions_refuse_before_assumptions() {
    let opts = CheckOptions::tlc(0, 7);
    let valid =
        dive_model::bind(ir(SOURCE), &config(), &dive_model::BindOptions::tlc()).expect("bind");
    let foreign =
        dive_model::bind(ir(SOURCE), &config(), &dive_model::BindOptions::tlc()).expect("foreign");
    for n in 0..6 {
        let mut model = valid.clone();
        let code = match n {
            0 => {
                model.ir = mutate(&valid.ir, 0);
                "DIVE-EVAL-C004"
            }
            1 => {
                model.ir = foreign.ir.clone();
                "DIVE-EVAL-C004"
            }
            2 => {
                model.names = foreign.names.clone();
                "DIVE-EVAL-C004"
            }
            3 => {
                model.init_actions = foreign.init_actions.clone();
                "DIVE-EVAL-C004"
            }
            4 => {
                model.invariants[0].pred = foreign.invariants[0].pred.clone();
                "DIVE-EVAL-C004"
            }
            5 => {
                model.invariants[0].pred.location = dive_ir::ExprId(u32::MAX);
                "DIVE-EVAL-C001"
            }
            _ => panic!("bad control"),
        };
        for requested in [0, 1, 4, 65] {
            let options = CheckOptions {
                workers: requested,
                ..opts.clone()
            };
            let Exploration::Stopped(r) = capture::explore(&model, &options, Limits::default())
            else {
                panic!("malformed capture succeeded")
            };
            no_work(&r, code);
            assert_eq!(valid.evaluator.take_output(), Vec::<String>::new());
            assert_eq!(foreign.evaluator.take_output(), Vec::<String>::new());
        }
    }
}
#[test]
fn valid_controls_preserve_metadata_and_capture_evidence() {
    let cfg = config();
    let base = ir(SOURCE);
    dive_eval::validate_ir(&base).expect("valid");
    let opts = CheckOptions::tlc(0, 7);
    let r = dive_check::check(&base, &cfg, &opts);
    assert_eq!(r.verdict, Verdict::Success);
    assert_eq!(r.var_names, ["x"]);
    assert_eq!(r.tlc_var_order, [0]);
    assert_eq!(r.workers.used, 1);
    assert_eq!(r.output, ["\"should not run\"  TRUE"]);
    let model = dive_model::bind(base, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    model.validate_context().expect("valid metadata");
    let pure = ir(
        "---- MODULE PublicArena ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nInv == x = x\n====\n",
    );
    let model = dive_model::bind(pure, &cfg, &dive_model::BindOptions::tlc()).expect("puremodel");
    let Exploration::Complete(c) = capture::explore(&model, &opts, Limits::default()) else {
        panic!("valid capture refused")
    };
    assert_eq!(c.stats.distinct, 1);
    assert_eq!(c.graph.states.len(), 1);
}
