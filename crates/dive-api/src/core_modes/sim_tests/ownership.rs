//! Terminal error, limit and refusal cleanup restore the caller host and
//! release the evaluator's simulation lease, on Core. The overlap and reentry
//! controls synchronised on caller-injected deferred-set callbacks that only
//! the retired reference simulator evaluated; `dive-sim`'s `lease` test keeps
//! the lease refusal itself (#791, #910).
#![allow(clippy::expect_used, reason = "bounded test setup and assertions")]
use super::simulate;
use dive_eval::{Host, HostError};
use dive_model::Model;
use dive_sim::{Options, Outcome};
use dive_stdlib::random::RandomMode;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Default)]
struct Caller(AtomicUsize);
impl Host for Caller {
    fn print_output(&self, _: &str) -> Result<bool, HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }
}
fn on_stack(work: impl FnOnce() + Send) {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(s, work)
            .expect("stack thread")
            .join()
            .expect("test");
    });
}
fn model(mode: RandomMode, next: &str) -> Model {
    let source = format!(
        "---- MODULE SharedPrint ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=0\nNext == {next}\nGate(S) == 0 \\in S\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("SharedPrint.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("configuration");
    let bind = if mode == RandomMode::Tlc {
        dive_model::BindOptions::tlc()
    } else {
        dive_model::BindOptions::dive()
    };
    dive_model::bind(ir, &cfg, &bind).expect("bound model")
}
fn invariant_error_model() -> Model {
    let source = include_str!("../../../../dive-sim/tests/fixtures/SimInvariantError.tla");
    let analysis = dive_sema::analyze_source(
        "SimInvariantError.tla",
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(include_str!(
        "../../../../dive-sim/tests/fixtures/SimInvariantError.cfg"
    ))
    .into_config()
    .expect("config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("pinned error model")
}

fn options(mode: RandomMode, stats: bool, events: usize) -> Options {
    let mut opts = Options::new(0, mode, 1, 1);
    opts.action_stats = stats;
    opts.max_output_events = events;
    opts
}
#[test]
fn error_limit_and_unsupported_cleanup_release_ownership_for_the_next_call() {
    on_stack(|| {
        for (next, expected) in [
            (r"x'=1 \div x", "error"),
            (r"x'=RandomElement({1})", "unsupported"),
            (r"PrintT(x) /\ x'=x+1", "limit"),
        ] {
            let base = if expected == "error" {
                // The pinned invariant-error context has an implemented TLC wrapper.
                // Direct successor division errors deliberately remain unverified.
                invariant_error_model()
            } else {
                model(RandomMode::Tlc, next)
            };
            let caller: Arc<dyn Host> = Arc::new(Caller::default());
            base.evaluator.set_host(caller.clone());
            for _ in 0..2 {
                let report = simulate(&base, &options(RandomMode::Tlc, true, 0));
                match expected {
                    "error" => assert!(matches!(report.outcome, Outcome::Error { .. })),
                    "unsupported" => assert!(
                        matches!(report.outcome, Outcome::Unsupported(ref s) if s.contains("RandomElement"))
                    ),
                    _ => assert!(matches!(report.outcome, Outcome::Limit(_))),
                }
                assert!(Arc::ptr_eq(
                    &caller,
                    &base.evaluator.host().expect("restored")
                ));
                let lease = base.evaluator.try_simulation_lease();
                assert!(
                    lease.is_some(),
                    "terminal cleanup releases actual shared-core ownership"
                );
                drop(lease);
            }
        }
    });
}
