//! An actual capture-thread callback retains the parent's armed budget.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "deterministic budget control"
)]

use super::*;
use dive_check::budget::{Phase, Resource};
use std::sync::Arc;

struct FreshCaptureThread<'a, 'b> {
    observer: &'a mut Observe<'b>,
    parent: std::thread::ThreadId,
    saw_callback: bool,
}

impl capture::Observer for FreshCaptureThread<'_, '_> {
    fn stop_after_level(&self) -> bool {
        self.observer.stop_after_level()
    }

    fn level_done(
        &mut self,
        level: u64,
        graph: &capture::BehaviorGraph,
        stats: Stats,
    ) -> Result<(), String> {
        assert_ne!(std::thread::current().id(), self.parent);
        assert!(
            !crate::budget::exhausted(),
            "fresh capture thread starts unlimited"
        );
        let result = self.observer.level_done(level, graph, stats);
        assert!(
            !crate::budget::exhausted(),
            "callback scope restores the prior budget"
        );
        self.saw_callback = true;
        result
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "actual capture-thread callback and parent serialization remain one end-to-end control"
)]
fn a_due_prefix_inherits_its_budget_and_parent_reports_a_typed_stop() {
    let source = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>(x = TRUE)\n====\n";
    let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config");
    let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    let limits = Limits::default();
    let planning = super::super::evaluator_control(&CheckOptions::tlc(0, 0)).expect("control");
    let plans = prepare(&model, limits, &planning).expect("plans before the budget expires");
    let watchdog = crate::budget::expired_for_test();
    let mut observer = Observe {
        budget: watchdog.budget(),
        control: dive_check::budget::EvaluationControl::new(
            watchdog.budget(),
            dive_check::budget::Phase::Temporal,
        )
        .expect("control"),
        model: &model,
        plans: &plans,
        limits,
        policy: AcceptancePolicy::Legacy,
        checks: Checks {
            schedule: Schedule {
                min_states: 1,
                growth_factor: 2,
            },
            ..Checks::default()
        },
        stage: Stage::Capture,
        action: None,
        decision: None,
    };
    // Keep capture itself unlimited so it deterministically reaches a due
    // callback with the already-expired temporal budget. The production path
    // supplies the same armed signal to both; no timing race is required here.
    let captured = {
        let mut wrapper = FreshCaptureThread {
            observer: &mut observer,
            parent: std::thread::current().id(),
            saw_callback: false,
        };
        let result = capture::explore_controlled_with_evidence(
            &model,
            &CheckOptions::tlc(0, 0),
            limits.capture,
            &mut wrapper,
        )
        .expect("capture thread");
        assert!(wrapper.saw_callback);
        result
    };
    let capture::ControlledExploration::Prefix(prefix) = captured else {
        panic!("budget exhaustion must stop the due callback before completion");
    };
    assert!(prefix.stats.left_on_queue > 0);
    let Err(error) = observer.decision.take().expect("callback decision") else {
        panic!("an exhausted callback cannot manufacture a candidate");
    };
    assert!(super::super::budget_limit(&error));
    let progress = prefix.stats;
    let failed = failure(
        error,
        observer.stage,
        Some(Box::new((&*prefix).into())),
        observer.action,
        Some(false),
        observer.checks,
    );
    let stopped = budget_failure(failed, watchdog.budget());
    assert_eq!(stopped.capture_completed, Some(false));
    assert_eq!(stopped.checks.prefixes, 1);
    assert_eq!(
        stopped
            .prefix
            .as_ref()
            .expect("actual prefix progress")
            .stats,
        progress
    );
    assert!(stopped.failure.failure.exploration.is_none());
    match &stopped.failure.failure.error {
        AnalysisError::Stopped(stop) => {
            assert_eq!(stop.resource, Resource::Time);
            assert_eq!(stop.phase, Phase::Temporal);
        }
        other => panic!("expected native typed stop, got {other:?}"),
    }
    // A genuine preexisting failure retains its class even when the same
    // watchdog is already tripped; only EXHAUSTED is converted.
    let genuine = failure(
        AnalysisError::Inconsistent("existing failure"),
        Stage::Product,
        None,
        None,
        Some(false),
        Checks::default(),
    );
    assert!(matches!(
        budget_failure(genuine, watchdog.budget())
            .failure
            .failure
            .error,
        AnalysisError::Inconsistent("existing failure")
    ));
}
