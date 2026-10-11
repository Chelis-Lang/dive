//! Actual initialization completion and producer-owned joined source waves.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "adversarial test assertions"
)]
use dive_check::{
    CheckOptions, Stats,
    capture::{self, CommittedSource, Exploration, SourceObserver},
};
use std::sync::Arc;
fn model(src: &str, opts: &CheckOptions) -> dive_model::Model {
    let a = dive_sema::analyze_source("Barrier.tla", src, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("config");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            no_deadlock: opts.no_deadlock,
            host: None,
        },
    )
    .expect("bind")
}
#[derive(Default)]
struct Observer {
    roots: usize,
    complete: Vec<Stats>,
    sources: usize,
    fail_complete: bool,
    width: usize,
}
impl SourceObserver for Observer {
    fn initial(
        &mut self,
        _id: u64,
        _state: &dive_eval::next::StateVals,
        _ordinal: u64,
        _stats: Stats,
    ) -> Result<(), String> {
        self.roots += 1;
        Ok(())
    }
    fn initial_complete(&mut self, stats: Stats) -> Result<(), String> {
        assert_eq!(self.sources, 0);
        self.complete.push(stats);
        if self.fail_complete {
            Err("stop-before-pool".into())
        } else {
            Ok(())
        }
    }
    fn source(&mut self, source: CommittedSource<'_>) -> Result<(), String> {
        assert_eq!(self.complete.len(), 1);
        assert_eq!(source.barrier.workers(), self.width);
        assert!(
            source.barrier.first_source() <= source.id && source.id < source.barrier.end_source()
        );
        assert!(source.barrier.end_source() - source.barrier.first_source() <= 8);
        self.sources += 1;
        Ok(())
    }
}
#[test]
fn complete_initialization_is_once_before_real_wave_and_never_after_failed_init() {
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            chaos: Some(7),
            ..CheckOptions::tlc(0, 0)
        };
        for inv in ["TRUE", "x=0"] {
            let src = format!(
                "---- MODULE Barrier ----\nVARIABLE x\nInit == x \\in {{0,1,2}}\nNext == UNCHANGED x\nInv == {inv}\n====\n"
            );
            let m = model(&src, &opts);
            let mut o = Observer {
                width,
                ..Default::default()
            };
            let r = capture::explore_observed_with_evidence(
                &m,
                &opts,
                capture::Limits::default(),
                &mut o,
            )
            .expect("capture");
            if inv == "TRUE" {
                assert!(matches!(r, Exploration::Complete(_)));
                assert_eq!(o.roots, 3);
                assert_eq!(o.complete.len(), 1);
                assert_eq!(o.complete[0].generated, 3);
                assert_eq!(o.complete[0].left_on_queue, 3);
                assert_eq!(o.sources, 3);
            } else {
                assert!(matches!(r, Exploration::Stopped(_)));
                assert_eq!(o.complete.len(), 0);
                assert_eq!(o.sources, 0);
            }
        }
        let src = "---- MODULE Barrier ----\nVARIABLE x\nInit == x \\in {0,1,2}\nNext == UNCHANGED x\nInv == TRUE\n====\n";
        let m = model(src, &opts);
        let mut o = Observer {
            width,
            fail_complete: true,
            ..Default::default()
        };
        let Exploration::Stopped(s) =
            capture::explore_observed_with_evidence(&m, &opts, capture::Limits::default(), &mut o)
                .expect("capture")
        else {
            panic!("stop")
        };
        assert_eq!(s.workers.used, 1);
        assert!(s.workers.sequential);
        assert_eq!(o.complete.len(), 1);
        assert_eq!(o.sources, 0);
        assert_eq!(s.stats.expect("stats").generated, 3);
    }
}

#[test]
fn generation_and_source_failures_keep_the_real_callback_prefix() {
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            chaos: Some(7),
            ..CheckOptions::tlc(0, 0)
        };
        for (init, next, roots, completions) in
            [("x=0", "x'=1", 1, 1), ("x=x", "UNCHANGED x", 0, 0)]
        {
            let src = format!(
                "---- MODULE Barrier ----\nVARIABLE x\nInit == {init}\nNext == {next}\nInv == x=0\n====\n"
            );
            let m = model(&src, &opts);
            let mut observer = Observer {
                width,
                ..Default::default()
            };
            let Exploration::Stopped(stop) = capture::explore_observed_with_evidence(
                &m,
                &opts,
                capture::Limits::default(),
                &mut observer,
            )
            .expect("capture") else {
                panic!("failure must stop capture")
            };
            assert_eq!(observer.roots, roots);
            assert_eq!(observer.complete.len(), completions);
            assert_eq!(observer.sources, 0);
            assert_eq!(stop.workers.used, if completions == 0 { 1 } else { width });
        }
    }
}
