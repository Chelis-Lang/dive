//! Ordered real engine commits; successful consumers are not temporal proofs.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_check::{
    CheckOptions, Stats, Verdict,
    capture::{self, CommittedSource, Exploration, SourceObserver},
};
use dive_eval::{EvalError, St};
use dive_store::StoreMode;
use std::sync::Arc;

fn model(source: &str, config: &str, opts: &CheckOptions) -> dive_model::Model {
    let name = source.split_whitespace().nth(2).expect("module");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(config).into_config().expect("config");
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
struct Observer<'a> {
    model: &'a dive_model::Model,
    predicate: dive_model::Pred,
    init: bool,
    roots: Vec<(u64, Stats)>,
    sources: Vec<(u64, Stats, Vec<capture::ConcreteSuccessor>)>,
    deciding: Option<EvalError>,
}
impl<'a> Observer<'a> {
    fn new(m: &'a dive_model::Model, init: bool) -> Self {
        let def = m.ir.root_def("Bad").expect("Bad");
        Self {
            model: m,
            predicate: dive_model::Pred::new(m.ir.def(def).body, m.evaluator.scope(def)),
            init,
            roots: vec![],
            sources: vec![],
            deciding: None,
        }
    }
    fn evaluate(&mut self, state: &dive_eval::next::StateVals) -> Result<(), String> {
        match self.model.eval(&self.predicate, &St::state(state.clone())) {
            Ok(_) => Ok(()),
            Err(e) => {
                let message = e.to_string();
                self.deciding = Some(e);
                Err(message)
            }
        }
    }
}
impl SourceObserver for Observer<'_> {
    fn initial(
        &mut self,
        id: u64,
        state: &dive_eval::next::StateVals,
        _ordinal: u64,
        counts: Stats,
    ) -> Result<(), String> {
        self.roots.push((id, counts));
        if self.init {
            self.evaluate(state)
        } else {
            Ok(())
        }
    }
    fn source(&mut self, source: CommittedSource<'_>) -> Result<(), String> {
        self.sources
            .push((source.id, source.stats, source.successors.to_vec()));
        self.evaluate(source.state)
    }
}
const CONFIG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";

#[test]
fn temporal_consumer_error_outranks_later_safety_and_enum_errors_at_every_width() {
    for next in [
        "x < 2 /\\ x' = x + 1",
        "x' = IF x = 0 THEN 0 ELSE 1 \\div 0",
    ] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            for width in [1, 3, 4, 8] {
                for chaos in [None, Some(7), Some(19)] {
                    let opts = CheckOptions {
                        workers: width,
                        store,
                        chaos,
                        ..CheckOptions::tlc(0, 0)
                    };
                    let source = format!(
                        "---- MODULE Before ----\nEXTENDS Integers\nVARIABLE x\nInit == x \\in {{0,1}}\nNext == {next}\nInv == x < 2\nBad == 1 \\div x = 0\n====\n"
                    );
                    let m = model(&source, CONFIG, &opts);
                    let mut observer = Observer::new(&m, false);
                    let r = capture::explore_observed_with_evidence(
                        &m,
                        &opts,
                        capture::Limits::default(),
                        &mut observer,
                    )
                    .expect("evidence");
                    let Exploration::Stopped(stop) = r else {
                        panic!("consumer error exposes no complete graph")
                    };
                    assert!(
                        matches!(&stop.verdict,Verdict::Unsupported {feature} if feature.contains("ordered source observer"))
                    );
                    let stats = stop.stats.expect("logical stop counts");
                    assert_eq!(
                        (
                            stats.generated,
                            stats.distinct,
                            stats.left_on_queue,
                            stats.depth
                        ),
                        (3, 2, 1, 1)
                    );
                    assert_eq!(stop.workers.used, width);
                    assert_eq!(observer.sources.len(), 1);
                    assert_eq!(observer.sources[0].0, 0);
                    assert_eq!(observer.sources[0].1, stats);
                    assert_eq!(observer.sources[0].2.len(), 1);
                    assert_eq!(
                        observer.deciding.as_ref().expect("typed error").ec().code(),
                        2179
                    );
                    assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
                }
            }
        }
    }
}

#[test]
fn earlier_safety_stop_prevents_source_callback_but_retains_initial_callback() {
    for width in [1, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let source = "---- MODULE Earlier ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' = 1\nInv == x = 0\nBad == 1 \\div x = 0\n====\n";
        let m = model(source, CONFIG, &opts);
        let mut observer = Observer::new(&m, false);
        let Exploration::Stopped(stop) = capture::explore_observed_with_evidence(
            &m,
            &opts,
            capture::Limits::default(),
            &mut observer,
        )
        .expect("evidence") else {
            panic!("safety stops")
        };
        assert!(matches!(stop.verdict, Verdict::InvariantViolated { .. }));
        let stats = stop.stats.expect("counts");
        assert_eq!(
            (stats.generated, stats.distinct, stats.left_on_queue),
            (2, 2, 0)
        );
        assert_eq!(observer.roots.len(), 1);
        assert!(observer.sources.is_empty());
        assert!(observer.deciding.is_none());
    }
}

#[test]
fn initial_consumer_precedes_initial_invariant_without_fabricating_replay_counts() {
    for width in [1, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let source = "---- MODULE Initial ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' = x\nInv == x < 0\nBad == 1 \\div x = 0\n====\n";
        let m = model(source, CONFIG, &opts);
        let mut observer = Observer::new(&m, true);
        let Exploration::Stopped(stop) = capture::explore_observed_with_evidence(
            &m,
            &opts,
            capture::Limits::default(),
            &mut observer,
        )
        .expect("evidence") else {
            panic!("initial consumer stops")
        };
        assert!(
            matches!(&stop.verdict,Verdict::Unsupported {feature} if feature.contains("ordered source observer"))
        );
        assert_eq!(stop.workers.used, 1);
        assert_eq!(stop.workers.requested, width);
        assert_eq!(observer.roots.len(), 1);
        assert!(observer.sources.is_empty());
        let stats = observer.roots[0].1;
        assert_eq!(
            (
                stats.generated,
                stats.distinct,
                stats.left_on_queue,
                stats.initial_generated
            ),
            (1, 1, 1, 1)
        );
        assert_eq!(observer.deciding.as_ref().expect("error").ec().code(), 2179);
    }
}

#[test]
fn parameter_and_foreign_ownership_admission_runs_no_callback_or_provider() {
    for width in [0, 1, 4, 8, 65] {
        for foreign in [0, 1, 2] {
            let opts = CheckOptions {
                workers: width,
                ..CheckOptions::tlc(0, 0)
            };
            let source = "---- MODULE Admission ----\nVARIABLE x\nInit == x = 0\nNext == x' = x\nInv == TRUE\nBad == TRUE\n====\n";
            let mut m = model(source, CONFIG, &opts);
            if foreign == 1 {
                m.ir = Arc::new(m.ir.as_ref().clone());
            }
            if foreign == 2 {
                m.names = Arc::new(dive_value::Interner::new());
            }
            if foreign == 0 && (1..=64).contains(&width) {
                continue;
            }
            let mut observer = Observer::new(&m, false);
            let Exploration::Stopped(stop) = capture::explore_observed_with_evidence(
                &m,
                &opts,
                capture::Limits::default(),
                &mut observer,
            )
            .expect("typed stop") else {
                panic!("admission")
            };
            assert!(observer.roots.is_empty() && observer.sources.is_empty());
            assert!(observer.deciding.is_none());
            assert!(stop.stats.is_none());
            assert_eq!(stop.workers.used, 0);
            assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
        }
    }
}
