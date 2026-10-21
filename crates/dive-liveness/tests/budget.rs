//! An exhausted run budget stops temporal analysis as `Stopped`, never as a
//! refusal, a holding result or a violation (ADR 0107).
#![allow(clippy::expect_used, clippy::panic, reason = "test setup")]

use std::sync::Arc;
use std::time::{Duration, Instant};

use dive_check::CheckOptions;
use dive_check::budget::{Budget, Phase, Resource};
use dive_liveness::analyze::{Analysis, AnalysisError, Limits, analyze_final};

#[test]
fn an_exhausted_budget_stops_the_analysis_and_a_generous_one_changes_nothing() {
    let source =
        "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nP == []<>x\n====\n";
    let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config");
    let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");

    let watchdog = Budget::new(Some(Duration::from_millis(1)), None)
        .start()
        .expect("watchdog thread starts")
        .expect("watchdog");
    let started = Instant::now();
    while watchdog.budget().poll().expect("measurement").is_none()
        && started.elapsed() < Duration::from_secs(10)
    {
        std::thread::sleep(Duration::from_millis(1));
    }
    let exhausted = CheckOptions {
        budget: watchdog.budget().clone(),
        ..CheckOptions::tlc(0, 0)
    };
    match analyze_final(&model, &exhausted, Limits::default()) {
        Err(error) => match &error {
            AnalysisError::Stopped(stopped) => {
                assert_eq!(stopped.resource, Resource::Time);
                assert_eq!(stopped.phase, Phase::Temporal);
                assert_eq!(error.code(), "DIVE-L0038");
            }
            other => panic!("expected a stop, got {other:?}"),
        },
        Ok(_) => panic!("an exhausted budget cannot hold"),
    }

    let generous = CheckOptions {
        budget: Budget::new(Some(Duration::from_secs(3600)), Some(u64::MAX)),
        ..CheckOptions::tlc(0, 0)
    };
    // Without fairness the stuttering lasso at x = FALSE violates P; a
    // generous budget finds the same candidate as no budget.
    let candidate = |options: &CheckOptions| match analyze_final(&model, options, Limits::default())
    {
        Ok(Analysis::Candidate(c)) => (c.states, c.back_to),
        other => panic!("expected a candidate, got {other:?}"),
    };
    assert_eq!(candidate(&generous), candidate(&CheckOptions::tlc(0, 0)));
}
