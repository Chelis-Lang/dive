//! Exact-run deferral admission and genuine completion negatives.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "adversarial endpoint assertions"
)]
use dive_check::{
    CheckOptions, CheckResult,
    capture::{self, Exploration, SourceObserver},
};
use std::sync::Arc;
fn model(post: &str) -> dive_model::Model {
    let source = format!(
        "---- MODULE Deferred ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x\nPost == {post}\n====\n"
    );
    let a = dive_sema::analyze_source("Deferred.tla", &source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    bind(ir)
}
fn bind(ir: Arc<dive_ir::Ir>) -> dive_model::Model {
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPOSTCONDITION Post\n")
        .into_config()
        .expect("cfg");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model")
}
struct End {
    token: capture::DeferredPostconditions,
    result: Option<Box<CheckResult>>,
    entries: Option<(usize, usize, usize, usize)>,
}
impl SourceObserver for End {
    fn source(&mut self, _: capture::CommittedSource<'_>) -> Result<(), String> {
        Ok(())
    }
    fn exploration_complete(&mut self, end: &capture::ExplorationEnd) -> Result<(), String> {
        self.entries = Some(end.completion().entries());
        match self.token.evaluate(end) {
            Ok(()) => Ok(()),
            Err(failure) => {
                self.result = Some(failure);
                Err("deciding deferred post".into())
            }
        }
    }
}
#[test]
fn same_core_clone_positive_and_separately_bound_same_ir_completion_negative() {
    let original = model("x=x");
    assert!(
        original
            .evaluator
            .shares_run_state(&original.evaluator.clone())
    );
    let fresh = bind(original.ir.clone());
    assert!(!original.evaluator.shares_run_state(&fresh.evaluator));
    assert!(
        fresh
            .evaluator
            .validate_scope(
                &original.postconditions.as_ref().expect("post")[0]
                    .pred
                    .scope
            )
            .is_ok()
    );
    for same_run in [true, false] {
        let execution =
            capture::DeferredPostconditions::admit(&original).expect("admitted direct grammar");
        let mut end = End {
            token: if same_run {
                execution.clone()
            } else {
                capture::DeferredPostconditions::admit(&fresh).expect("token")
            },
            result: None,
            entries: None,
        };
        let result = capture::explore_observed_deferred_with_evidence(
            &execution,
            &CheckOptions::tlc(0, 0),
            capture::Limits::default(),
            &mut end,
        )
        .expect("capture");
        assert!(matches!(result, Exploration::Stopped(_)));
        assert_eq!(end.entries, Some((1, 2, 1, 1)));
        let deciding = &end.result.expect("actual failure").verdict;
        if same_run {
            assert_eq!(deciding.tlc().expect("original class").code, 2405);
        } else {
            assert!(matches!(
                deciding,
                dive_check::Verdict::InternalError { .. }
            ));
        }
    }
}
#[test]
fn unsupported_post_work_and_metadata_refuse_before_evaluation() {
    for body in ["CHOOSE n \\in Nat: TRUE", "SUBSET (1..1000000000) = {}"] {
        let input = model(body);
        assert!(capture::DeferredPostconditions::admit(&input).is_err());
        assert_eq!(input.take_output(), Vec::<String>::new());
    }
    let mut input = model("x=x");
    input.postconditions.as_mut().expect("list")[0].name = "p".repeat(50_001);
    assert!(capture::DeferredPostconditions::admit(&input).is_err());
    assert_eq!(input.take_output(), Vec::<String>::new());
    let mut input = model("x=x");
    input.postconditions.as_mut().expect("list")[0].pred.expr = dive_ir::ExprId(u32::MAX);
    assert!(capture::DeferredPostconditions::admit(&input).is_err());
    assert_eq!(input.take_output(), Vec::<String>::new());
}

#[test]
fn separately_admitted_same_core_models_with_changed_posts_or_next_cannot_use_foreign_end() {
    let original = model("x=x");
    let execution = capture::DeferredPostconditions::admit(&original).expect("execution token");
    for change in [0, 1, 2] {
        let mut edited = original.clone();
        if change == 1 {
            edited.postconditions = Ok(vec![]);
        }
        if change == 2 {
            edited.actions.clear();
        }
        assert!(edited.evaluator.shares_run_state(&original.evaluator));
        let mut end = End {
            token: capture::DeferredPostconditions::admit(&edited).expect("separate admission"),
            result: None,
            entries: None,
        };
        let result = capture::explore_observed_deferred_with_evidence(
            &execution,
            &CheckOptions::tlc(0, 0),
            capture::Limits::default(),
            &mut end,
        )
        .expect("actual capture");
        assert!(matches!(result, Exploration::Stopped(_)));
        assert!(matches!(
            end.result.expect("token mismatch").verdict,
            dive_check::Verdict::InternalError { .. }
        ));
        assert_eq!(end.entries, Some((1, 2, 1, 1)));
    }
}
