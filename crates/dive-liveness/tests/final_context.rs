//! Configured predicates preserve scope isolation and original native hooks.
#![allow(clippy::expect_used, clippy::panic, reason = "boundary assertions")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use dive_check::CheckOptions;
use dive_liveness::{
    analyze::{self, Analysis, AnalysisError, Limits, Stage},
    plan::PlanningError,
};

const CONFIGURED: &str = "---- MODULE ContextGuard ----\nEXTENDS Sequences\nCONSTANT Op(_)\nVARIABLE x\nInit==x=0\nNext==x'=x\nLength(v)==Len(v)\nP==<>(Op(<<1>>)=1)\n====\n";

fn bound(source: &str, configured: bool, options: &CheckOptions) -> dive_model::Model {
    bound_to(source, configured.then_some("Length"), options)
}

fn bound_to(source: &str, target: Option<&str>, options: &CheckOptions) -> dive_model::Model {
    let analysis =
        dive_sema::analyze_source("ContextGuard.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let cfg = format!(
        "INIT Init\nNEXT Next\nPROPERTY P\n{}",
        target.map_or_else(String::new, |name| format!("CONSTANT Op <- {name}\n"))
    );
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &dive_config::parse(&cfg).into_config().expect("config"),
        &dive_model::BindOptions {
            eval: options.eval,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}

#[derive(Debug)]
struct Hooks {
    calls: AtomicUsize,
    deny: bool,
}
impl dive_eval::Host for Hooks {
    fn check_native_access(&self, _: dive_ir::StdOp) -> Result<(), dive_eval::HostError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.deny {
            Err(dive_eval::HostError {
                code: 1000,
                message: "valid-model-native-hook-error".into(),
            })
        } else {
            Ok(())
        }
    }
}
fn install(model: &dive_model::Model, deny: bool) -> Arc<Hooks> {
    let host = Arc::new(Hooks {
        calls: AtomicUsize::new(0),
        deny,
    });
    model.evaluator.set_host(host.clone());
    host
}

#[test]
fn configured_user_wrapper_preserves_constant_native_hook_and_typed_error() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let model = bound(CONFIGURED, true, &options);
        let host = install(&model, true);
        let failure = analyze::analyze_final_with_evidence(&model, &options, Limits::default())
            .expect_err("valid configured wrapper invokes the original native hook");
        assert_eq!(failure.stage, Stage::Planning);
        assert!(failure.exploration.is_none());
        let AnalysisError::Planning(PlanningError::Evaluation { error, .. }) = failure.error else {
            panic!("original typed error: {failure:?}")
        };
        assert_eq!(error.class(), dive_eval::ErrorClass::Eval);
        assert_eq!(error.ec().code(), 1000);
        assert!(error.to_string().contains("valid-model-native-hook-error"));
        assert_eq!(host.calls.load(Ordering::SeqCst), 1);
        let model = bound(CONFIGURED, true, &options);
        let host = install(&model, false);
        assert!(matches!(
            analyze::analyze_final(&model, &options, Limits::default()),
            Ok(Analysis::Holds(_))
        ));
        assert!(host.calls.load(Ordering::SeqCst) > 0);
    }
}

#[test]
fn configured_native_target_still_refuses_before_native_hooks() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let model = bound_to(CONFIGURED, Some("Len"), &options);
        let host = install(&model, true);
        let failure = analyze::analyze_final_with_evidence(&model, &options, Limits::default())
            .expect_err("configured native target stays unsupported");
        assert!(failure.exploration.is_none());
        let AnalysisError::Planning(PlanningError::Unsupported(refusal)) = failure.error else {
            panic!("explicit configured native refusal: {failure:?}")
        };
        assert_eq!(refusal.detail, "configured predicate operator target");
        assert_eq!(host.calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn public_lowerers_reject_foreign_configured_context_before_hooks() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for mutation in ["names", "ir", "scope"] {
            let mut model = bound(CONFIGURED, true, &options);
            let host = install(&model, true);
            let mut property = model.property_claims()[0].pred.clone();
            match mutation {
                "names" => model.names = Arc::new(dive_value::Interner::new()),
                "ir" => model.ir = bound(CONFIGURED, true, &options).ir,
                "scope" => {
                    property.scope = bound(CONFIGURED, true, &options).property_claims()[0]
                        .pred
                        .scope
                        .clone();
                }
                _ => unreachable!(),
            }
            assert!(
                dive_liveness::plan::lower(
                    &model,
                    &property,
                    dive_liveness::formula::Limits::default()
                )
                .is_err()
            );
            assert!(
                dive_liveness::plan::lower_violation(
                    &model,
                    &property,
                    dive_liveness::formula::Limits::default()
                )
                .is_err()
            );
            assert_eq!(host.calls.load(Ordering::SeqCst), 0, "{mutation}");
        }
    }
}

#[test]
fn final_analysis_checks_all_retained_model_context_before_constant_hooks() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for mutation in [
            "property-location",
            "property-expression",
            "property-scope",
            "unused-invariant",
            "next",
            "initial",
            "assumption",
            "temporal-origins",
            "names",
            "ir",
            "foreign-location-owner",
        ] {
            let mut model = bound(CONFIGURED, true, &options);
            let foreign = bound(CONFIGURED, true, &options);
            let host = install(&model, true);
            match mutation {
                "property-location" => {
                    model.configured_properties[0].pred.location = dive_ir::ExprId(u32::MAX);
                }
                "property-expression" => {
                    model.configured_properties[0].pred.expr = dive_ir::ExprId(u32::MAX);
                }
                "property-scope" => {
                    model.configured_properties[0].pred.scope =
                        foreign.configured_properties[0].pred.scope.clone();
                }
                "unused-invariant" => model.invariants.push(dive_model::Named {
                    name: "UnusedForeignInvariant".into(),
                    pred: foreign.next.clone().expect("Next"),
                }),
                "next" => model.next = foreign.next.clone(),
                "initial" => model.init = foreign.init.clone(),
                "assumption" => model.assumptions.push(dive_ir::DefId(u32::MAX)),
                "temporal-origins" => model
                    .implied_temporals
                    .push(model.implied_temporals[0].clone()),
                "names" => model.names = Arc::new(dive_value::Interner::new()),
                "ir" => model.ir = foreign.ir.clone(),
                "foreign-location-owner" => {
                    let extended =
                        CONFIGURED.replace("====", "Padding == <<1,2,3,4,5,6,7,8,9,10>>\n====");
                    model.ir = bound(&extended, true, &options).ir;
                    let index = model.ir.nodes().len() - 1;
                    assert!(index >= model.evaluator.ir().nodes().len());
                    model.configured_properties[0].pred.location =
                        dive_ir::ExprId(u32::try_from(index).expect("small fixture"));
                }
                _ => unreachable!(),
            }
            let context_error = model
                .validate_context()
                .expect_err("the mutation invalidates retained model metadata")
                .to_string();
            let failure = analyze::analyze_final_with_evidence(&model, &options, Limits::default())
                .expect_err("invalid public metadata must refuse before constant evaluation");
            assert_eq!(host.calls.load(Ordering::SeqCst), 0, "{mutation}");
            assert_eq!(failure.stage, Stage::Planning, "{mutation}");
            assert!(failure.exploration.is_none(), "{mutation}");
            let AnalysisError::Planning(PlanningError::Unsupported(refusal)) = failure.error else {
                panic!("model context must precede native hooks: {mutation}: {failure:?}")
            };
            assert_eq!(refusal.detail, context_error, "{mutation}");
        }
    }
}
