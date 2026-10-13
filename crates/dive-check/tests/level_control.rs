//! Deliberate completed-level stops are distinct from completion and failure.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "controlled test assertions and callback panic"
)]
use dive_check::{
    CheckOptions, Stats, Verdict,
    capture::{self, BehaviorGraph, Exploration, LevelControl, Observer},
};
use dive_store::StoreMode;
use std::sync::Arc;
fn model(opts: &CheckOptions) -> dive_model::Model {
    let source = "---- MODULE Boundary ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit == x \\in 0..11\nNext == x'=(x+12)%36\nPost == FALSE\n====\n";
    let a = dive_sema::analyze_source("Boundary.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPOSTCONDITION Post\n")
        .into_config()
        .expect("cfg");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}
struct Stop {
    stats: Option<Stats>,
    fail: u8,
}
impl Observer for Stop {
    fn level_control(
        &mut self,
        level: u64,
        graph: &BehaviorGraph,
        stats: Stats,
    ) -> Result<LevelControl, String> {
        assert_eq!(level, 1);
        assert_eq!(graph.states.len(), 24);
        assert!(graph.edges[12..].iter().all(Vec::is_empty));
        self.stats = Some(stats);
        match self.fail {
            1 => Err("consumer failed".into()),
            2 => panic!("controlled callback panic"),
            _ => Ok(LevelControl::Stop),
        }
    }
}
#[test]
fn prefix_keeps_real_width_counts_graph_and_skips_postconditions() {
    for base in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        for store in [StoreMode::Exact, StoreMode::Fingerprint] {
            for width in [1, 4] {
                for chaos in [None, Some(91)] {
                    let opts = CheckOptions {
                        workers: width,
                        store,
                        chaos,
                        ..base.clone()
                    };
                    let mut observer = Stop {
                        stats: None,
                        fail: 0,
                    };
                    let result = capture::explore_observed_with_evidence(
                        &model(&opts),
                        &opts,
                        capture::Limits::default(),
                        &mut observer,
                    )
                    .expect("capture");
                    let Exploration::Prefix(prefix) = result else {
                        panic!("expected prefix: {result:?}")
                    };
                    assert_eq!(prefix.stats, observer.stats.expect("boundary"));
                    assert_eq!(
                        (
                            prefix.stats.generated,
                            prefix.stats.distinct,
                            prefix.stats.depth,
                            prefix.stats.left_on_queue
                        ),
                        (24, 24, 2, 12)
                    );
                    assert_eq!(prefix.workers.used, width);
                    assert!(!prefix.workers.sequential);
                    assert_eq!(prefix.output, [] as [String; 0], "postcondition executed");
                    assert_eq!(prefix.collision.is_some(), store == StoreMode::Fingerprint);
                    assert_eq!(
                        prefix.graph.concrete[..12]
                            .iter()
                            .map(Vec::len)
                            .sum::<usize>(),
                        12
                    );
                }
            }
        }
    }
}
#[test]
fn callback_error_and_panic_are_not_deliberate_prefix_stops() {
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 7)
    };
    for fail in [1, 2] {
        let mut observer = Stop { stats: None, fail };
        let result = capture::explore_observed_with_evidence(
            &model(&opts),
            &opts,
            capture::Limits::default(),
            &mut observer,
        )
        .expect("capture");
        let Exploration::Stopped(result) = result else {
            panic!("failure returned graph: {result:?}")
        };
        assert_eq!(result.stats, observer.stats);
        assert_eq!(result.workers.used, 4);
        assert_eq!(result.output, [] as [String; 0]);
        if fail == 1 {
            assert!(matches!(result.verdict, Verdict::Unsupported { .. }));
        } else {
            assert!(matches!(result.verdict, Verdict::InternalError { .. }));
        }
    }
}
