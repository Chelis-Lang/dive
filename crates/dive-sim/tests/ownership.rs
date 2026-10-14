//! Actual public captured-value callbacks synchronize operational ownership tests.
//! Opaque providers grant no independent word-validation authority.
#![allow(clippy::expect_used, reason = "bounded test setup and assertions")]
use dive_eval::{Host, HostError};
use dive_model::{Model, Named, Pred};
use dive_sim::{Options, Outcome, Report, simulate};
use dive_stdlib::random::RandomMode;
use dive_value::{Ctx, DeferredKind, DeferredSet, Value};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

#[derive(Debug, Default)]
struct Caller(AtomicUsize);
impl Host for Caller {
    fn print_output(&self, _: &str) -> Result<bool, HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }
}
#[derive(Debug, Default)]
struct Gate {
    entered: Option<mpsc::Sender<()>>,
    release: Option<Mutex<mpsc::Receiver<()>>>,
    reenter: Option<(Arc<Model>, Options)>,
    reentered: Mutex<Option<Report>>,
    visits: AtomicUsize,
}
impl DeferredSet for Gate {
    fn kind(&self) -> DeferredKind {
        DeferredKind::Map
    }
    fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
        // An unexpected enumeration cannot provide a successful gate.
        Ok(Vec::new())
    }
    fn member(&self, elem: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
        assert_eq!(elem, &Value::Int(0));
        if self.visits.fetch_add(1, Ordering::SeqCst) == 0 {
            if let Some((model, options)) = &self.reenter {
                *self.reentered.lock().expect("reentry result") = Some(simulate(model, options));
            }
            if let Some(entered) = &self.entered {
                entered.send(()).expect("entered");
                self.release
                    .as_ref()
                    .expect("release")
                    .lock()
                    .expect("release lock")
                    .recv_timeout(Duration::from_secs(10))
                    .expect("bounded callback release");
            }
        }
        Some(Ok(true))
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
    let source = include_str!("fixtures/SimInvariantError.tla");
    let analysis = dive_sema::analyze_source(
        "SimInvariantError.tla",
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(include_str!("fixtures/SimInvariantError.cfg"))
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
fn gated(base: &Model, gate: Arc<Gate>) -> Model {
    let mut m = base.clone();
    let def = m.ir.root_def("Gate").expect("Gate");
    let empty = m.evaluator.scope(def).expect("owned scope");
    let scope = m
        .evaluator
        .scope_call_values(
            &empty,
            def,
            vec![Value::deferred(gate, m.evaluator.options().ctx)],
        )
        .expect("public value capture");
    let pred = Pred::new(m.ir.def(def).body, scope);
    m.evaluator
        .validate_predicate(&pred.scope, pred.expr)
        .expect("public admission");
    m.invariants.push(Named {
        name: "Gate".to_owned(),
        pred,
    });
    m
}
fn channel_gate() -> (Arc<Gate>, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered, recv) = mpsc::channel();
    let (send, release) = mpsc::channel();
    (
        Arc::new(Gate {
            entered: Some(entered),
            release: Some(Mutex::new(release)),
            ..Gate::default()
        }),
        recv,
        send,
    )
}
fn refused(report: &Report) {
    assert!(
        matches!(&report.outcome, Outcome::Unsupported(s) if s.contains("concurrent or reentrant"))
    );
    assert_eq!(report.stage, "preflight");
    assert_eq!(
        (
            report.generated,
            report.initial_candidates,
            report.traces_started
        ),
        (0, 0, 0)
    );
    assert_eq!(report.traces.len(), 0);
    assert_eq!(report.runtime_output.len(), 0);
    assert!(report.action_flow.is_none());
}
fn assert_flow(report: &Report, stats: bool, count: u64) {
    if stats {
        assert_eq!(
            report
                .action_flow
                .as_ref()
                .expect("flow")
                .transitions
                .values()
                .sum::<u64>(),
            count
        );
    } else {
        assert!(report.action_flow.is_none());
    }
}
#[test]
fn overlap_refuses_without_callback_host_or_output_and_preserves_preflight_priority() {
    on_stack(|| {
        for mode in [RandomMode::Tlc, RandomMode::Dive] {
            for stats in [false, true] {
                let base = model(mode, r"PrintT(x) /\ x'=x+1");
                let caller = Arc::new(Caller::default());
                let original: Arc<dyn Host> = caller.clone();
                base.evaluator.set_host(original.clone());
                let (a_gate, entered, release) = channel_gate();
                let a = gated(&base, a_gate.clone());
                let b_gate = Arc::new(Gate::default());
                let b = gated(&base, b_gate.clone());
                std::thread::scope(|workers| {
                    let run = std::thread::Builder::new()
                        .stack_size(dive_eval::RECOMMENDED_STACK)
                        .spawn_scoped(workers, || simulate(&a, &options(mode, stats, 0)))
                        .expect("A");
                    entered
                        .recv_timeout(Duration::from_secs(10))
                        .expect("actual A callback entered");
                    assert!(a_gate.visits.load(Ordering::SeqCst) > 0);
                    let active = base.evaluator.host().expect("A host");
                    refused(&simulate(&b, &options(mode, !stats, 1)));
                    assert_eq!(b_gate.visits.load(Ordering::SeqCst), 0);
                    assert!(Arc::ptr_eq(
                        &active,
                        &base.evaluator.host().expect("unchanged active host")
                    ));
                    let mut invalid = options(mode, stats, 1);
                    invalid.traces = 0;
                    let rejected = simulate(&b, &invalid);
                    assert!(
                        matches!(rejected.outcome, Outcome::Unsupported(ref s) if s.contains("positive finite trace count"))
                    );
                    assert_eq!(rejected.stage, "preflight");
                    assert_eq!(rejected.generated, 0);
                    assert_eq!(b_gate.visits.load(Ordering::SeqCst), 0);
                    release.send(()).expect("release A");
                    let result = run.join().expect("A terminal");
                    assert!(matches!(result.outcome, Outcome::Limit(_)));
                    assert_eq!(result.generated, 1);
                    assert_eq!(result.runtime_output.len(), 0);
                    assert_flow(&result, stats, 0);
                });
                assert!(Arc::ptr_eq(
                    &original,
                    &base.evaluator.host().expect("restored caller")
                ));
                let serial = simulate(&b, &options(mode, stats, 1));
                assert!(b_gate.visits.load(Ordering::SeqCst) > 0);
                assert!(matches!(serial.outcome, Outcome::Sampled));
                assert_eq!(serial.generated, 2);
                assert_eq!(
                    serial
                        .runtime_output
                        .iter()
                        .map(|e| e.text.as_str())
                        .collect::<Vec<_>>(),
                    ["0"]
                );
                assert_flow(&serial, stats, 1);
                assert!(Arc::ptr_eq(
                    &original,
                    &base.evaluator.host().expect("serial caller")
                ));
                assert_eq!(caller.0.load(Ordering::SeqCst), 0);
            }
        }
    });
}
#[test]
fn reentry_refuses_without_waiting_or_output_and_parent_keeps_its_host() {
    on_stack(|| {
        let base = model(RandomMode::Tlc, r"PrintT(x) /\ x'=x+1");
        let caller: Arc<dyn Host> = Arc::new(Caller::default());
        base.evaluator.set_host(caller.clone());
        let gate = Arc::new(Gate {
            reenter: Some((Arc::new(base.clone()), options(RandomMode::Tlc, true, 1))),
            ..Gate::default()
        });
        let outer = gated(&base, gate.clone());
        let result = simulate(&outer, &options(RandomMode::Tlc, true, 1));
        assert!(gate.visits.load(Ordering::SeqCst) > 0);
        refused(
            gate.reentered
                .lock()
                .expect("result")
                .as_ref()
                .expect("actual reentry"),
        );
        assert!(matches!(result.outcome, Outcome::Sampled));
        assert_eq!(result.runtime_output.len(), 1);
        assert_flow(&result, true, 1);
        assert!(Arc::ptr_eq(
            &caller,
            &base.evaluator.host().expect("caller")
        ));
    });
}
#[test]
fn separate_bound_cores_can_both_enter_callbacks_before_either_finishes() {
    on_stack(|| {
        let a_base = model(RandomMode::Tlc, r"PrintT(x) /\ x'=x+1");
        let b_base = model(RandomMode::Dive, r"PrintT(x) /\ x'=x+1");
        let a_caller: Arc<dyn Host> = Arc::new(Caller::default());
        let b_caller: Arc<dyn Host> = Arc::new(Caller::default());
        a_base.evaluator.set_host(a_caller.clone());
        b_base.evaluator.set_host(b_caller.clone());
        let (a_gate, a_enter, a_release) = channel_gate();
        let (b_gate, b_enter, b_release) = channel_gate();
        let a = gated(&a_base, a_gate.clone());
        let b = gated(&b_base, b_gate.clone());
        std::thread::scope(|workers| {
            let first = std::thread::Builder::new()
                .stack_size(dive_eval::RECOMMENDED_STACK)
                .spawn_scoped(workers, || {
                    simulate(&a, &options(RandomMode::Tlc, false, 1))
                })
                .expect("A");
            a_enter
                .recv_timeout(Duration::from_secs(10))
                .expect("A entered");
            let second = std::thread::Builder::new()
                .stack_size(dive_eval::RECOMMENDED_STACK)
                .spawn_scoped(workers, || {
                    simulate(&b, &options(RandomMode::Dive, true, 1))
                })
                .expect("B");
            b_enter
                .recv_timeout(Duration::from_secs(10))
                .expect("B entered before A finishes");
            assert!(
                a_gate.visits.load(Ordering::SeqCst) > 0
                    && b_gate.visits.load(Ordering::SeqCst) > 0
            );
            a_release.send(()).expect("release A");
            b_release.send(()).expect("release B");
            for report in [first.join().expect("A"), second.join().expect("B")] {
                assert!(matches!(report.outcome, Outcome::Sampled));
                assert_eq!(report.runtime_output.len(), 1);
                assert_eq!(report.generated, 2);
            }
        });
        assert!(Arc::ptr_eq(
            &a_caller,
            &a_base.evaluator.host().expect("A restored")
        ));
        assert!(Arc::ptr_eq(
            &b_caller,
            &b_base.evaluator.host().expect("B restored")
        ));
    });
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
