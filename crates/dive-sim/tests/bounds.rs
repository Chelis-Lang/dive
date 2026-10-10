//! Bounds and unsupported features never become sampled successes.
#![allow(clippy::unwrap_used, reason = "test setup failures must fail the test")]
use dive_sim::{Options, Outcome, simulate};
use dive_stdlib::random::RandomMode;
use std::sync::Arc;

fn run(source: &str, config: &str, options: &Options) -> dive_sim::Report {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || {
                let analysis =
                    dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
                assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
                let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
                let config = dive_config::parse(config).into_config().unwrap();
                let model = dive_model::bind(ir, &config, &dive_model::BindOptions::tlc()).unwrap();
                simulate(&model, options)
            })
            .unwrap()
            .join()
            .unwrap()
    })
}

#[test]
fn exhausted_budgets_keep_partial_statistics_and_refuse() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n";
    let mut options = Options::new(0, RandomMode::Tlc, 10, 1);
    options.max_generated = 2;
    let report = run(source, "INIT Init\nNEXT Next\n", &options);
    assert!(
        matches!(report.outcome, Outcome::Limit(_)),
        "{:?}",
        report.outcome
    );
    assert_eq!(report.generated, 2);
    assert_eq!(report.stage, "sampling");
    assert_eq!(report.traces_started, 1);
    options.max_generated = 100;
    options.max_retained = 2;
    assert!(matches!(
        run(source, "INIT Init\nNEXT Next\n", &options).outcome,
        Outcome::Limit(_)
    ));
    options.max_candidates = 0;
    let report = run(source, "INIT Init\nNEXT Next\n", &options);
    assert_eq!(report.stage, "initialization");
    assert!(matches!(report.outcome, Outcome::Limit(_)));
}

#[test]
fn temporal_effects_and_compound_normalization_are_named_refusals() {
    for (source, config) in [
        (
            "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nLive == []<>(x = 0)\n====\n",
            "INIT Init\nNEXT Next\nPROPERTY Live\n",
        ),
        (
            "---- MODULE M ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x' = RandomElement({1, 2})\n====\n",
            "INIT Init\nNEXT Next\n",
        ),
        (
            "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = <<0>>\nNext == x' = x\n====\n",
            "INIT Init\nNEXT Next\n",
        ),
    ] {
        let report = run(source, config, &Options::new(0, RandomMode::Tlc, 3, 1));
        assert!(
            matches!(report.outcome, Outcome::Unsupported(_)),
            "{:?}",
            report.outcome
        );
        assert_eq!(report.traces_started, 0);
    }
}

#[test]
fn initial_enumeration_finishes_before_checking_completeness() {
    let mut source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLES x, y\nInit == (x = 1) \\/ (x = 0 /\\ y = 1 \\div x)\nNext == x' = x /\\ y' = y\n====\n".to_owned();
    let options = Options::new(0, RandomMode::Tlc, 4, 1);
    let report = run(&source, "INIT Init\nNEXT Next\n", &options);
    assert!(
        matches!(report.outcome, Outcome::Error { code: 1000, .. }),
        "{report:?}"
    );
    assert_eq!(report.generated, 0);
    assert_eq!(report.initial_candidates, 1);
    source = source.replace("y = 1 \\div x", "y = 0");
    let report = run(&source, "INIT Init\nNEXT Next\n", &options);
    assert!(
        matches!(report.outcome, Outcome::Error { code: 2106, .. }),
        "{report:?}"
    );
    assert_eq!(report.generated, 2);
    assert_eq!(report.initial_candidates, 2);
}

#[test]
fn failure_traces_respect_zero_and_tiny_retained_budgets() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = 1\nInv == x = 0\nBadInit == x = 1\n====\n";
    let mut options = Options::new(0, RandomMode::Tlc, 3, 2);
    options.max_retained = 0;
    let report = run(source, "INIT BadInit\nNEXT Next\nINVARIANT Inv\n", &options);
    assert!(matches!(report.outcome, Outcome::Limit(_)), "{report:?}");
    assert!(report.traces.is_empty());
    assert_eq!(report.pending_violation.unwrap().code, 2107);
    options.max_retained = 1;
    let report = run(source, "INIT Init\nNEXT Next\nINVARIANT Inv\n", &options);
    assert!(matches!(report.outcome, Outcome::Limit(_)), "{report:?}");
    assert_eq!(
        report.traces.iter().map(|t| t.states.len()).sum::<usize>(),
        1
    );
    assert_eq!(report.generated, 2);
    assert_eq!(report.pending_violation.unwrap().code, 2110);
    let report = run(source, "INIT BadInit\nNEXT Next\nINVARIANT Inv\n", &options);
    assert!(matches!(
        report.outcome,
        Outcome::Violation { code: 2107, .. }
    ));
    assert_eq!(report.traces[0].states.len(), 1);
    options.max_retained = 2;
    let report = run(source, "INIT Init\nNEXT Next\nINVARIANT Inv\n", &options);
    assert!(matches!(
        report.outcome,
        Outcome::Violation { code: 2110, .. }
    ));
    assert_eq!(report.traces[0].states.len(), 2);
    assert!(report.pending_violation.is_none());
}
