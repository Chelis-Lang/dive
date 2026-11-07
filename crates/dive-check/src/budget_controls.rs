//! Budget controls that need the reference engine: they moved here with the
//! budget module's split into `dive-runtime` (ADR 0155).
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "deterministic budget control evidence"
)]

use std::sync::Arc;
use std::time::Duration;

use crate::budget::{Budget, MeasurementError, Phase, SamplePoint, finish_result};

const LOOP: &str = "PrintT(\"before-loop\") /\\ (\\A i \\in 1..4096: i = i)";

fn model(assumption: &str, options: &crate::CheckOptions) -> dive_model::Model {
    let source = format!(
        "---- MODULE ControlledAssume ----\nEXTENDS Naturals, TLC\nVARIABLE x\n\
         ASSUME {assumption}\nInit == x = 0\nNext == x' = x\n====\n"
    );
    let analysis = dive_sema::analyze_source(
        "ControlledAssume.tla",
        &source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    dive_model::bind(
        ir,
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("bind")
}

#[test]
fn terminal_memory_failures_preserve_real_earlier_errors_but_replace_success() {
    let source = "---- MODULE Earlier ----\nVARIABLE x\nInit == x = TRUE\nNext == UNCHANGED x\nInv == ~x\n====\n";
    let analysis = dive_sema::analyze_source("Earlier.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let opts = crate::CheckOptions::tlc(0, 0);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\n")
        .into_config()
        .expect("cfg");
    let mut violation = crate::check(&ir, &cfg, &opts);
    assert!(
        matches!(violation.verdict, crate::Verdict::InvariantViolated { .. }),
        "{:?}",
        violation.verdict
    );
    let witness = format!("{:?}", violation.counterexample);
    let failed = Err(MeasurementError::unavailable(SamplePoint::Final));
    finish_result(&mut violation, failed, Phase::Finalization);
    assert!(
        matches!(violation.verdict, crate::Verdict::InvariantViolated { .. }),
        "{:?}",
        violation.verdict
    );
    assert_eq!(format!("{:?}", violation.counterexample), witness);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    let mut success = crate::check(&ir, &cfg, &opts);
    assert_eq!(success.verdict, crate::Verdict::Success);
    finish_result(&mut success, failed, Phase::Finalization);
    assert!(matches!(
        success.verdict,
        crate::Verdict::BudgetMeasurement { .. }
    ));
    assert!(success.counterexample.is_none());
    assert!(success.collision.is_none());
    assert!(success.verdict.tlc().is_none());
}

#[test]
fn capture_assumption_stop_has_no_graph_search_statistics_or_witness() {
    for mut options in [crate::CheckOptions::tlc(0, 0), crate::CheckOptions::dive(0)] {
        let model = model(LOOP, &options);
        let watchdog = Budget::new(Some(Duration::from_secs(3600)), None)
            .start()
            .expect("watchdog")
            .expect("finite budget");
        watchdog.budget().trip_for_test();
        options.budget = watchdog.budget().clone();
        let outcome = crate::capture::explore(&model, &options, crate::capture::Limits::default());
        let crate::capture::Exploration::Stopped(result) = outcome else {
            panic!("an interrupted assumption cannot produce a complete graph");
        };
        assert!(
            matches!(result.verdict, crate::Verdict::Stopped(ref stop) if stop.phase == Phase::Startup)
        );
        assert!(result.stats.is_none());
        assert!(result.counterexample.is_none());
        assert!(result.verdict.tlc().is_none());
        assert!(
            result
                .output
                .iter()
                .any(|line| line.contains("before-loop"))
        );
    }
}
