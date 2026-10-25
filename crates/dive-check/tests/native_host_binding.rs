//! Real safety/capture entrypoints preserve state and installation boundaries.
#![allow(clippy::expect_used, reason = "bounded source and ownership controls")]

use dive_check::{CheckOptions, Verdict, capture};
use dive_eval::{Evaluator, Host, NativeHooks};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SOURCE: &str = "---- MODULE Binding ----\nEXTENDS Naturals,FiniteSets,TLC\nVARIABLE x\nASSUME PrintT(\"assume\")\nInit == x=Cardinality({1})\nNext == x'=x\n====\n";
const CONFIG: &str = "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n";
fn inputs(source: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let analysis = dive_sema::analyze_source("Binding.tla", source, &Default::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    (
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        dive_config::parse(CONFIG).into_config().expect("config"),
    )
}
#[derive(Debug)]
struct OldHost(Arc<AtomicUsize>);
impl Host for OldHost {}
impl Drop for OldHost {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}
fn model(options: &CheckOptions, drops: &Arc<AtomicUsize>) -> dive_model::Model {
    let (ir, config) = inputs(SOURCE);
    dive_model::bind(
        ir,
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: options.no_deadlock,
            host: Some(Arc::new(OldHost(drops.clone()))),
        },
    )
    .expect("ordinary fresh bound model")
}
struct Observe<'a> {
    evaluator: &'a Evaluator,
    drops: &'a AtomicUsize,
    observations: Vec<(NativeHooks, Option<u64>, usize)>,
}
impl capture::Observer for Observe<'_> {
    fn init(
        &mut self,
        _id: usize,
        _state: &dive_eval::next::StateVals,
        _stats: dive_check::Stats,
    ) -> Result<(), String> {
        let stamp = self.evaluator.host_binding_stamp();
        self.observations.push((
            stamp.native_hooks(),
            stamp.generation(),
            self.drops.load(Ordering::Relaxed),
        ));
        Ok(())
    }
}
#[test]
fn capture_drops_the_old_host_before_init_and_resets_only_the_fresh_host_policy()
-> Result<(), String> {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let reference_drops = Arc::new(AtomicUsize::new(0));
        let reference = model(&options, &reference_drops);
        let original = capture::explore_with_evidence(&reference, &options, Default::default())
            .expect("reference callback capture");
        let capture::Exploration::Complete(original) = original else {
            return Err("reference must complete".into());
        };
        assert_eq!(reference_drops.load(Ordering::Relaxed), 1);
        for hooks in [NativeHooks::Callback, NativeHooks::Unrestricted] {
            let drops = Arc::new(AtomicUsize::new(0));
            let current = model(&options, &drops);
            assert_eq!(drops.load(Ordering::Relaxed), 0);
            let mut observer = Observe {
                evaluator: &current.evaluator,
                drops: &drops,
                observations: Vec::new(),
            };
            let result = capture::explore_controlled_with_native_hooks(
                &current,
                &options,
                Default::default(),
                &mut observer,
                hooks,
            )
            .expect("operative capture");
            let capture::ControlledExploration::Complete(actual) = result else {
                return Err("actual must complete".into());
            };
            assert_eq!(actual.stats, original.stats);
            assert_eq!(actual.graph.states, original.graph.states);
            assert_eq!(actual.graph.edges, original.graph.edges);
            assert_eq!(actual.graph.roots, original.graph.roots);
            assert_eq!(actual.output, original.output);
            assert_eq!(actual.output.len(), 1);
            assert_eq!(observer.observations.len(), 1);
            let (observed, epoch, old_drops) = observer.observations[0];
            assert_eq!(observed, hooks);
            assert!(epoch.is_some());
            assert_eq!(old_drops, 1);
            let after = current.evaluator.host_binding_stamp();
            assert_eq!(after.native_hooks(), NativeHooks::Callback);
            if hooks == NativeHooks::Unrestricted {
                assert_ne!(after.generation(), epoch);
            }
        }
    }
    Ok(())
}

#[test]
fn ordinary_source_check_keeps_binding_output_and_registers_through_init() {
    let source = "---- MODULE Binding ----\nEXTENDS Naturals,TLC\nVARIABLE x\nC == Print(TLCSet(0,9),0)\nInit == C=0 /\\ x=TLCGet(0)\nNext == x'=x\n====\n";
    let (ir, config) = inputs(source);
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        let result = dive_check::check(&ir, &config, &options);
        assert_eq!(result.verdict, Verdict::Success);
        let stats = result.stats.expect("completed stats");
        assert_eq!((stats.distinct, stats.generated, stats.depth), (1, 2, 1));
        assert_eq!(
            result.output.len(),
            1,
            "binding output is neither discarded nor replayed"
        );
    }
}
