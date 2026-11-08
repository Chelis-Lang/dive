//! Temporal atoms may call state-level `RECURSIVE` operators
//! (`Evaluator::recursive_leaf`), which TLC leaves to evaluation instead of
//! expanding (`Liveness.astToLive`). Counterexamples through them pass the
//! independent validator; recursive operators outside the rule keep the named
//! planning refusal. Expected results are the pinned TLC's.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
#[path = "support/refused.rs"]
mod refused;
use dive_api::CheckRequest;

const SPEC: &str = include_str!("fixtures/recursive-temporal/Rec.tla");

fn check(directory: &std::path::Path, property: &str, compat_tlc: bool) -> serde_json::Value {
    std::fs::write(
        directory.join("Rec.cfg"),
        if property == "Done" {
            include_str!("fixtures/recursive-temporal/Rec.cfg").to_owned()
        } else {
            format!("SPECIFICATION Spec\nPROPERTY {property}\n")
        },
    )
    .expect("cfg");
    dive_api::check_file_engine(
        &CheckRequest {
            file: directory.join("Rec.tla"),
            compat_tlc,
            seed: Some(0),
            fp: Some(0),
            workers: Some(1),
            ..CheckRequest::default()
        },
        dive_api::Engine::Core,
    )
    .expect("checked")
    .to_json()
}

#[test]
fn state_level_recursive_calls_are_evaluation_leaves() {
    let directory =
        std::env::temp_dir().join(format!("dive-recursive-temporal-{}", std::process::id()));
    std::fs::create_dir(&directory).expect("private directory");
    std::fs::write(directory.join("Rec.tla"), SPEC).expect("source");
    for compat_tlc in [false, true] {
        // The recursive call occurs in a binder domain of the fair action and
        // in the property; a binder, LET and bounded CHOOSE inside the
        // recursive body are ordinary values. TLC: success, 3 distinct and 7
        // generated states for each.
        for property in [
            "Done",
            "Enumerates",
            "Direct",
            "ZeroProperty",
            "Shapes",
            "ActionActual",
        ] {
            let doc = check(&directory, property, compat_tlc);
            assert_eq!(doc["verdict"], "success", "{property}: {doc}");
            assert_eq!(doc["statistics"]["distinct"], 3, "{property}: {doc}");
            assert_eq!(doc["statistics"]["generated"], 7, "{property}: {doc}");
            assert_eq!(doc["statistics"]["depth"], 3, "{property}: {doc}");
        }
        // A false property through a recursive call is a violation whose
        // counterexample passes the independent validator, as TLC reports
        // (2116, exit 13).
        for property in ["Never", "DirectFalse", "ActionActualFalse"] {
            let doc = check(&directory, property, compat_tlc);
            assert_eq!(doc["verdict"], "liveness-violation", "{property}: {doc}");
            assert_eq!(doc["property"], property, "{doc}");
            assert_eq!(doc["counterexample"]["validation"], "valid", "{doc}");
            assert_eq!(doc["tlc"]["code"], 2116, "{doc}");
            assert_eq!(doc["tlc"]["exit_status"], 13, "{doc}");
            assert_eq!(doc["statistics"]["distinct"], 3, "{property}: {doc}");
            assert_eq!(doc["statistics"]["generated"], 7, "{property}: {doc}");
            assert_eq!(doc["statistics"]["depth"], 3, "{property}: {doc}");
        }
        // An unbounded CHOOSE reaches the evaluator unchanged. The existing
        // liveness-atom error boundary remains an explicit refusal, never a
        // successful verdict (TLC reports an evaluation error there).
        // Core reaches the evaluation error and refuses to build its report
        // (#910); the retired reference engine named the unbounded CHOOSE.
        let doc = check(&directory, "Opaque", compat_tlc);
        assert_eq!(doc["verdict"], "unsupported", "{doc}");
        assert!(doc["counterexample"].is_null(), "{doc}");
        assert!(
            doc["message"]
                .as_str()
                .is_some_and(|m| m.contains(refused::TEMPORAL_FAILURE_REPORT)),
            "{doc}"
        );
    }
    std::fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn recursive_arguments_keep_the_callers_analysis_and_validation_boundaries() {
    use std::sync::Arc;

    let source = r"---- MODULE RecursiveActuals ----
EXTENDS Naturals, TLC
VARIABLE v
Init == v = FALSE
Next == UNCHANGED v
RECURSIVE R(_,_)
R(p,n) == IF n = 0 THEN p ELSE R(p,n-1)
HiddenEnabled == <>R(ENABLED (v' = v),0)
HiddenEffect == <>R(PrintT(v = v),0)
RECURSIVE S(_,_)
S(p,n) == IF n = 0 THEN p ELSE S(p,n-1)
Redirected == <>R(v,0)
====
";
    let analysis = dive_sema::analyze_source(
        "RecursiveActuals.tla",
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    for options in [
        dive_model::BindOptions::tlc(),
        dive_model::BindOptions::dive(),
    ] {
        let model = dive_model::bind(ir.clone(), &cfg, &options).expect("model");
        let states = vec![Arc::from([Some(false.into())])];
        for name in ["HiddenEnabled", "HiddenEffect"] {
            let def = ir.root_def(name).expect("property");
            let pred =
                dive_model::Pred::new(ir.def(def).body, model.evaluator.scope(def).expect("scope"));
            assert!(
                dive_liveness::plan::lower(
                    &model,
                    &pred,
                    dive_liveness::analyze::Limits::default().formula,
                )
                .is_err(),
                "planner: {name}"
            );
            assert!(
                dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0).is_err(),
                "validator: {name}"
            );
            assert_eq!(
                model.evaluator.take_output(),
                Vec::<String>::new(),
                "{name}"
            );
        }
    }
    // A definition redirect must not erase the configured call's origin
    // before the recursive-leaf proof. Its pure replacement does not certify
    // the original configured call as an ordinary recursive leaf.
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nCONSTANT R <- S\n")
        .into_config()
        .expect("redirect cfg");
    for options in [
        dive_model::BindOptions::tlc(),
        dive_model::BindOptions::dive(),
    ] {
        let model = dive_model::bind(ir.clone(), &cfg, &options).expect("redirect model");
        let def = ir.root_def("Redirected").expect("property");
        let pred =
            dive_model::Pred::new(ir.def(def).body, model.evaluator.scope(def).expect("scope"));
        let states = vec![Arc::from([Some(false.into())])];
        assert!(
            dive_liveness::plan::lower(
                &model,
                &pred,
                dive_liveness::analyze::Limits::default().formula
            )
            .is_err()
        );
        assert!(dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0).is_err());
    }
}
