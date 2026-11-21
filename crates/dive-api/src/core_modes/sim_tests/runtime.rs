//! An owned run context continues startup and restores its own installed host.
//! Core samples this model's reached `RandomElement` from each trace's own
//! stream (ADR 0162 §8); the constant's start-up draw is recorded once.
#![allow(clippy::expect_used, reason = "bounded fixture and assertions")]

use std::sync::Arc;

use super::simulate_in;
use dive_sim::{Options, Outcome, Runtime};
use dive_stdlib::random::RandomMode;

fn run(mode: RandomMode, seed: i64) {
    let mut options = Options::new(seed, mode, 3, 2);
    options.max_output_events = 0;
    let runtime = Runtime::new(options.clone());
    let source = "---- MODULE Owned ----\nEXTENDS Naturals,TLC\nCONSTANT C\nVARIABLES x,y\nBindingPrint==Print(\"binding\",1)\nPick==RandomElement({1,2})\nInit==x=C /\\ y=RandomElement({x,x+1})\nNext==x'=RandomElement({1,2}) /\\ y'=RandomElement({x',x'+1})\n====\n";
    let analysis = dive_sema::analyze_source("Owned.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let config = dive_config::parse("CONSTANT C <- Pick\nINIT Init\nNEXT Next\n")
        .into_config()
        .expect("configuration");
    let host = runtime.host();
    let model = dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &config,
        &dive_model::BindOptions {
            host: Some(host.clone()),
            ..if mode == RandomMode::Tlc {
                dive_model::BindOptions::tlc()
            } else {
                dive_model::BindOptions::dive()
            }
        },
    )
    .expect("bound with owned startup host");
    assert!(
        !model.output.is_empty(),
        "actual binding Print remains startup output"
    );
    let report = simulate_in(&model, &options, &runtime);
    assert_eq!(
        report.runtime_output,
        [] as [dive_eval::capture::Captured; 0]
    );
    assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
    assert_eq!(report.traces.len(), 2);
    assert_eq!(runtime.startup_draws().len(), 1, "C <- Pick drew once");
    for trace in &report.traces {
        // Init and every step draw twice each.
        assert_eq!(trace.draws.len(), trace.states.len());
        assert!(trace.draws.iter().all(|(_, values)| values.len() == 2));
    }
    assert!(Arc::ptr_eq(
        &host,
        &model.evaluator.host().expect("owned host restored")
    ));
    let repeated = simulate_in(&model, &options, &runtime);
    assert!(
        matches!(repeated.outcome, Outcome::Unsupported(ref reason) if reason.contains("already used")),
        "{repeated:?}"
    );
    assert_eq!(repeated.generated, 0);
}

#[test]
fn owned_runs_restore_their_host_and_refuse_reuse() {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || {
                for mode in [RandomMode::Dive, RandomMode::Tlc] {
                    for seed in [0, 7] {
                        run(mode, seed);
                    }
                }
            })
            .expect("stack")
            .join()
            .expect("runtime controls");
    });
}
