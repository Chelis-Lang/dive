//! A simulation runtime owns the evaluator for one run: an overlapping or
//! reentrant run refuses before its backend starts, and the caller host is
//! restored afterwards. Core's simulation runs as such a backend.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded test setup and assertions"
)]

use std::sync::Arc;

use dive_sim::{Options, Outcome, Report, Runtime};
use dive_stdlib::random::RandomMode;

#[derive(Debug)]
struct Caller;
impl dive_eval::Host for Caller {}

fn model() -> dive_model::Model {
    let source = "---- MODULE Lease ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n";
    let analysis = dive_sema::analyze_source("Lease.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("configuration");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &config,
        &dive_model::BindOptions::tlc(),
    )
    .expect("model")
}

fn backend_report() -> Report {
    Report {
        stage: "sampling",
        outcome: Outcome::Sampled,
        generated: 1,
        initial_candidates: 1,
        pending_violation: None,
        traces_started: 1,
        traces: Vec::new(),
        action_flow: None,
        runtime_output: Vec::new(),
        sampled_sets: Vec::new(),
        traces_stalled: 0,
        temporal: None,
    }
}

fn refused(report: &Report) -> bool {
    matches!(&report.outcome, Outcome::Unsupported(reason) if reason.contains("concurrent or reentrant"))
        && report.stage == "preflight"
        && report.generated == 0
}

#[test]
fn overlapping_and_reentrant_runs_refuse_and_the_caller_host_is_restored() {
    let model = model();
    let caller: Arc<dyn dive_eval::Host> = Arc::new(Caller);
    model.evaluator.set_host(caller.clone());
    let options = Options::new(0, RandomMode::Tlc, 1, 1);

    // Another owner holds the evaluator: the backend never starts.
    let lease = model.evaluator.try_simulation_lease().expect("free lease");
    let overlap = Runtime::new(options.clone())
        .simulate_with_backend(&model, |_| panic!("an overlapping backend must not start"));
    assert!(refused(&overlap), "{overlap:?}");
    drop(lease);
    assert!(Arc::ptr_eq(
        &caller,
        &model.evaluator.host().expect("caller")
    ));

    // A run started from inside a running backend refuses; the outer run
    // keeps its own report and installed host.
    let outer = Runtime::new(options.clone());
    let report = outer.simulate_with_backend(&model, |_| {
        let active = model.evaluator.host().expect("runtime host");
        assert!(!Arc::ptr_eq(&caller, &active));
        let inner = Runtime::new(options.clone())
            .simulate_with_backend(&model, |_| panic!("a reentrant backend must not start"));
        assert!(refused(&inner), "{inner:?}");
        assert!(Arc::ptr_eq(
            &active,
            &model.evaluator.host().expect("unchanged host")
        ));
        backend_report()
    });
    assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
    assert!(Arc::ptr_eq(
        &caller,
        &model.evaluator.host().expect("restored caller")
    ));
    assert!(
        model.evaluator.try_simulation_lease().is_some(),
        "lease released"
    );
}
