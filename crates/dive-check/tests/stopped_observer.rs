//! Source-error authority is separate from EOF; diagnostics retain the winner.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded independent observer negative controls"
)]
use dive_check::{
    CheckOptions, Stats, Verdict,
    capture::{self, Exploration, SourceObserver, StoppedDiagnosticFault, WorkerStop},
};
use std::{sync::Arc, time::Duration};
fn model(init: &str, next: &str, role: &str) -> dive_model::Model {
    let src = format!(
        "---- MODULE Stopped ----\nEXTENDS Integers\nVARIABLES x,y\nInit == {init}\nNext == {next}\nInv == x=0\nAction == [][x'=x]_<<x,y>>\n====\n"
    );
    let a = dive_sema::analyze_source("Stopped.tla", &src, &dive_sema::Options::default());
    let cfg = dive_config::parse(&format!("INIT Init\nNEXT Next\n{role}\n"))
        .into_config()
        .expect("cfg");
    dive_model::bind(
        Arc::new(dive_ir::lower(&a).expect("IR")),
        &cfg,
        &dive_model::BindOptions {
            eval: dive_eval::Options::tlc(),
            host: None,
            no_deadlock: false,
        },
    )
    .expect("bind")
}
#[derive(Default)]
struct Observer {
    pre: Vec<(Stats, Verdict, WorkerStop, u64)>,
    post: usize,
    faults: Vec<StoppedDiagnosticFault>,
    mode: u8,
    source_failure: bool,
    invalid_interval: bool,
}
impl SourceObserver for Observer {
    fn requires_stopped_service(&self) -> bool {
        true
    }
    fn progress_requests(&self) -> Option<dive_check::progress::Options> {
        Some(dive_check::progress::Options {
            interval: if self.invalid_interval {
                Duration::ZERO
            } else {
                Duration::from_secs(3600)
            },
            force: false,
        })
    }
    fn stopped_queue_finished(&mut self, end: &capture::StoppedQueue<'_>) -> Result<(), String> {
        assert_eq!(self.post, 0);
        assert!(end.pool().0 > 0);
        assert!(end.pending_request().is_none());
        self.pre
            .push((end.stats(), end.verdict().clone(), end.kind(), end.source()));
        match self.mode {
            1 => Err("controlled stopped metadata rejection".into()),
            2 => panic!("controlled stopped callback unwind"),
            _ => Ok(()),
        }
    }
    fn stopped_diagnostic_failed(&mut self, fault: StoppedDiagnosticFault) {
        self.faults.push(fault);
    }
    fn exploration_stopped(&mut self, end: &capture::ExplorationStop<'_>) {
        assert_eq!(end.result().stats, Some(self.pre[0].0));
        assert_eq!(end.result().verdict, self.pre[0].1);
        self.post += 1;
    }
    fn exploration_complete(&mut self, _: &capture::ExplorationEnd) -> Result<(), String> {
        panic!("source failure is not EOF")
    }
    fn source(&mut self, _: capture::CommittedSource<'_>) -> Result<(), String> {
        if self.source_failure {
            Err("controlled unsupported observer".into())
        } else {
            Ok(())
        }
    }
}
fn stopped(
    m: &dive_model::Model,
    o: &CheckOptions,
    limits: capture::Limits,
    observer: &mut Observer,
) -> Box<dive_check::CheckResult> {
    match capture::explore_observed_with_evidence(m, o, limits, observer).expect("capture") {
        Exploration::Stopped(r) => r,
        Exploration::Complete(_) => panic!("must stop"),
    }
}
fn assert_trace(a: Option<&dive_check::Counterexample>, b: Option<&dive_check::Counterexample>) {
    assert_eq!(a.is_some(), b.is_some());
    if let (Some(a), Some(b)) = (a, b) {
        assert_eq!(a.states.len(), b.states.len());
        for (a, b) in a.states.iter().zip(&b.states) {
            assert_eq!(a.label, b.label);
            assert_eq!(a.action, b.action);
            assert_eq!(a.values, b.values);
            assert_eq!(a.text, b.text);
        }
    }
}
#[test]
fn every_source_family_retains_verdict_counts_trace_and_workers_on_rejection_or_unwind() {
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            chaos: Some(7),
            ..CheckOptions::tlc(0, 0)
        };
        for (next, role, kind) in [
            ("x'=1 \\div 0 /\\ y'=y", "", WorkerStop::Evaluation),
            ("x'=1 /\\ y'=y", "INVARIANT Inv", WorkerStop::Invariant),
            (
                "x'=1 /\\ y'=y",
                "PROPERTY Action",
                WorkerStop::ActionProperty,
            ),
            ("x'=1", "", WorkerStop::Incomplete),
            ("x<0 /\\ x'=x /\\ y'=y", "", WorkerStop::Deadlock),
        ] {
            let baseline = capture::explore(
                &model("x=0 /\\ y=0", next, role),
                &opts,
                capture::Limits::default(),
            );
            let Exploration::Stopped(baseline) = baseline else {
                panic!("baseline stops")
            };
            for mode in [0, 1, 2] {
                let mut observer = Observer {
                    mode,
                    ..Observer::default()
                };
                let result = stopped(
                    &model("x=0 /\\ y=0", next, role),
                    &opts,
                    capture::Limits::default(),
                    &mut observer,
                );
                assert_eq!(result.verdict, baseline.verdict);
                assert_eq!(result.stats, baseline.stats);
                assert_trace(
                    result.counterexample.as_ref(),
                    baseline.counterexample.as_ref(),
                );
                assert_eq!(result.output, baseline.output);
                assert_eq!(result.workers.used, width);
                assert_eq!(observer.pre.len(), 1);
                assert_eq!(observer.pre[0].2, kind);
                assert_eq!(observer.pre[0].3, 0);
                assert_eq!(observer.post, 1);
                assert_eq!(
                    observer.faults.as_slice(),
                    match mode {
                        1 => &[StoppedDiagnosticFault::Callback][..],
                        2 => &[StoppedDiagnosticFault::Unwind][..],
                        _ => &[],
                    }
                );
                if mode != 0 {
                    assert!(
                        result
                            .notes
                            .iter()
                            .any(|n| n.contains("original ordered source winner retained"))
                    );
                }
            }
        }
    }
}
#[test]
fn init_resource_unsupported_and_lane_setup_stops_never_mint_source_authority() {
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        for (init, role, source_failure, resource, invalid_interval) in [
            ("x=1 /\\ y=0", "INVARIANT Inv", false, false, false),
            ("x=1 \\div 0 /\\ y=0", "", false, false, false),
            ("x=0 /\\ y=0", "", true, false, false),
            ("x=0 /\\ y=0", "", false, true, false),
            ("x=0 /\\ y=0", "", false, false, true),
        ] {
            let mut observer = Observer {
                source_failure,
                invalid_interval,
                ..Observer::default()
            };
            let mut limits = capture::Limits::default();
            if resource {
                limits.states = 1;
            }
            let result = stopped(
                &model(init, "x'=x+1 /\\ y'=y", role),
                &opts,
                limits,
                &mut observer,
            );
            assert!(matches!(
                result.verdict,
                Verdict::InvariantViolated { initial: true, .. }
                    | Verdict::EvaluationError {
                        site: dive_check::ErrorSite::Init,
                        ..
                    }
                    | Verdict::Unsupported { .. }
                    | Verdict::ResourceLimit { .. }
            ));
            assert_eq!(observer.pre.len(), 0);
            assert_eq!(observer.post, 0);
            assert_eq!(observer.faults.len(), 0);
        }
    }
}

#[test]
fn stateful_admission_is_read_once_and_never_mints_postjoin_authority() {
    use std::cell::Cell;
    struct Stateful {
        calls: Cell<usize>,
        panic: bool,
        pre: usize,
        post: usize,
        faults: usize,
    }
    impl SourceObserver for Stateful {
        fn requires_stopped_service(&self) -> bool {
            self.calls.set(self.calls.get() + 1);
            assert!(!self.panic, "controlled admission unwind");
            self.calls.get() > 1
        }
        fn stopped_queue_finished(&mut self, _: &capture::StoppedQueue<'_>) -> Result<(), String> {
            self.pre += 1;
            Ok(())
        }
        fn exploration_stopped(&mut self, _: &capture::ExplorationStop<'_>) {
            self.post += 1;
        }
        fn stopped_diagnostic_failed(&mut self, _: StoppedDiagnosticFault) {
            self.faults += 1;
        }
        fn source(&mut self, _: capture::CommittedSource<'_>) -> Result<(), String> {
            Ok(())
        }
    }
    for width in [1, 3, 4, 8] {
        for panic in [false, true] {
            let opts = CheckOptions {
                workers: width,
                ..CheckOptions::tlc(0, 0)
            };
            let m = model("x=0 /\\ y=0", "x'=1 /\\ y'=y", "INVARIANT Inv");
            let Exploration::Stopped(baseline) =
                capture::explore(&m, &opts, capture::Limits::default())
            else {
                panic!("baseline")
            };
            let mut observer = Stateful {
                calls: Cell::new(0),
                panic,
                pre: 0,
                post: 0,
                faults: 0,
            };
            let Exploration::Stopped(result) = capture::explore_observed_with_evidence(
                &m,
                &opts,
                capture::Limits::default(),
                &mut observer,
            )
            .expect("capture") else {
                panic!("stop")
            };
            assert_eq!(result.verdict, baseline.verdict);
            assert_eq!(result.stats, baseline.stats);
            assert_eq!(result.output, baseline.output);
            assert_trace(
                result.counterexample.as_ref(),
                baseline.counterexample.as_ref(),
            );
            assert_eq!(observer.calls.get(), 1);
            assert_eq!((observer.pre, observer.post), (0, 0));
            assert_eq!(observer.faults, usize::from(panic));
        }
    }
}
