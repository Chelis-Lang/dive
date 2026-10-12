//! Producer-owned exhaustion before joins; callback failures cannot manufacture EOF.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "actual finite observer lifecycle assertions"
)]
use dive_check::{
    CheckOptions, Stats,
    capture::{self, Exploration, SourceObserver},
};
use std::{sync::Arc, time::Duration};
fn model(init: &str, next: &str, inv: &str) -> dive_model::Model {
    let source = format!(
        "---- MODULE Terminal ----\nEXTENDS Integers\nVARIABLE x\nInit == {init}\nNext == {next}\nInv == {inv}\n====\n"
    );
    let a = dive_sema::analyze_source("Terminal.tla", &source, &dive_sema::Options::default());
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("cfg");
    dive_model::bind(
        Arc::new(dive_ir::lower(&a).expect("IR")),
        &cfg,
        &dive_model::BindOptions {
            eval: dive_eval::Options::tlc(),
            no_deadlock: false,
            host: None,
        },
    )
    .expect("bind")
}
#[derive(Default)]
struct Observer {
    terminal: Vec<Stats>,
    final_count: usize,
    fail: bool,
    fail_source: bool,
    invalid_lane: bool,
}
impl SourceObserver for Observer {
    fn progress_requests(&self) -> Option<dive_check::progress::Options> {
        Some(dive_check::progress::Options {
            interval: if self.invalid_lane {
                Duration::ZERO
            } else {
                Duration::from_secs(3600)
            },
            force: false,
        })
    }
    fn requires_terminal_service(&self) -> bool {
        true
    }
    fn terminal_queue_finished(&mut self, end: &capture::ExhaustedQueue<'_>) -> Result<(), String> {
        assert_eq!(self.final_count, 0);
        assert_eq!(end.stats().left_on_queue, 0);
        assert!(end.pending_request().is_none());
        self.terminal.push(end.stats());
        if self.fail {
            Err("controlled terminal metadata error".into())
        } else {
            Ok(())
        }
    }
    fn exploration_complete(&mut self, end: &capture::ExplorationEnd) -> Result<(), String> {
        assert_eq!(self.terminal.as_slice(), &[end.stats()]);
        self.final_count += 1;
        Ok(())
    }
    fn source(&mut self, _: capture::CommittedSource<'_>) -> Result<(), String> {
        if self.fail_source {
            Err("controlled source stop".into())
        } else {
            Ok(())
        }
    }
}
#[test]
fn actual_terminal_failure_preserves_completed_counters_and_never_calls_final() {
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            chaos: Some(7),
            ..CheckOptions::tlc(0, 0)
        };
        for fail in [false, true] {
            let m = model("x=0", "x'=1", "TRUE");
            let mut observer = Observer {
                fail,
                ..Observer::default()
            };
            let r = capture::explore_observed_with_evidence(
                &m,
                &opts,
                capture::Limits::default(),
                &mut observer,
            )
            .expect("capture");
            assert_eq!(observer.terminal.len(), 1);
            let stats = observer.terminal[0];
            assert_eq!(
                (stats.generated, stats.distinct, stats.left_on_queue),
                (3, 2, 0)
            );
            if fail {
                let Exploration::Stopped(stop) = r else {
                    panic!("terminal failure must stop")
                };
                assert_eq!(stop.stats, Some(stats));
                assert_eq!(stop.workers.used, width);
                assert_eq!(observer.final_count, 0);
            } else {
                assert!(matches!(r, Exploration::Complete(_)));
                assert_eq!(observer.final_count, 1);
            }
        }
    }
}
#[test]
fn init_source_and_capture_limit_stops_never_authorize_terminal_or_final() {
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            chaos: Some(7),
            ..CheckOptions::tlc(0, 0)
        };
        for (init, next, inv, source, bound) in [
            ("x=0", "x'=1", "x=9", false, false),
            ("x=0", "x'=(1 \\div 0)", "TRUE", false, false),
            ("x=0", "x'=1", "TRUE", true, false),
            ("x=0", "x'=x+1", "TRUE", false, true),
        ] {
            let m = model(init, next, inv);
            let mut observer = Observer {
                fail_source: source,
                ..Observer::default()
            };
            let mut limits = capture::Limits::default();
            if bound {
                limits.states = 3;
            }
            let r = capture::explore_observed_with_evidence(&m, &opts, limits, &mut observer)
                .expect("capture");
            assert!(matches!(r, Exploration::Stopped(_)));
            assert_eq!(observer.terminal, [] as [Stats; 0]);
            assert_eq!(observer.final_count, 0);
        }
    }
}

#[test]
fn invalid_interval_keeps_actual_initial_set_queue_counts_before_store_swap() {
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let m = model("x \\in {0,1,2}", "UNCHANGED x", "TRUE");
        let mut o = Observer {
            invalid_lane: true,
            ..Observer::default()
        };
        let r =
            capture::explore_observed_with_evidence(&m, &opts, capture::Limits::default(), &mut o)
                .expect("actual setup failure");
        let Exploration::Stopped(stop) = r else {
            panic!("invalid interval must stop");
        };
        let stats = stop.stats.expect("actual Init stats");
        assert_eq!(
            (
                stats.initial_generated,
                stats.generated,
                stats.distinct,
                stats.left_on_queue
            ),
            (3, 3, 3, 3)
        );
        assert_eq!(stop.workers.used, width);
        assert_eq!(o.terminal, [] as [Stats; 0]);
        assert_eq!(o.final_count, 0);
    }
}
