//! Operative hooks, ownership and restoration through the original evaluator.
#![allow(clippy::expect_used, reason = "bounded real evaluator controls")]

use super::*;
use crate::{Evaluator, Model, Options};
use std::sync::Weak;
use std::sync::atomic::{AtomicBool, AtomicUsize};

type Trace = Arc<Mutex<Vec<&'static str>>>;
fn event(trace: &Trace, value: &'static str) {
    trace
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(value);
}
fn take(trace: &Trace) -> Vec<&'static str> {
    std::mem::take(&mut *trace.lock().unwrap_or_else(PoisonError::into_inner))
}
fn evaluator(options: Options) -> Evaluator {
    let source = "---- MODULE Binding ----\nEXTENDS Naturals, FiniteSets, TLC\nCalc == Cardinality(Print({1,2}, {1,2}))\nOutput == PrintT(7)\nLevel == TLCGet(\"level\")\nLoop == \\A x \\in 1..2048: TRUE\nPure == Cardinality({1,2})\n====\n";
    let analysis = dive_sema::analyze_source("Binding.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    Evaluator::new(
        Model::new(Arc::new(dive_ir::lower(&analysis).expect("IR"))),
        options,
    )
    .expect("evaluator")
}
fn run(ev: &Evaluator, name: &str) -> Result<Value> {
    let def = ev.ir().root_def(name).expect("definition");
    ev.eval_in(def, ev.ir().def(def).body, &St::constant())
}
#[derive(Debug)]
struct Probe {
    trace: Trace,
    drops: Arc<AtomicUsize>,
    refuse_output: AtomicBool,
    domain: u64,
}
impl Host for Probe {
    fn check_native_access(&self, _op: StdOp) -> std::result::Result<(), HostError> {
        event(&self.trace, "native-denied");
        Err(HostError {
            code: 0,
            message: "native denied".into(),
        })
    }
    fn print_output(&self, _text: &str) -> std::result::Result<bool, HostError> {
        event(&self.trace, "print");
        if self.refuse_output.load(Ordering::Relaxed) {
            Err(HostError {
                code: 0,
                message: "output denied".into(),
            })
        } else {
            Ok(true)
        }
    }
    fn level(&self) -> Option<i64> {
        event(&self.trace, "level-panic");
        std::panic::resume_unwind(Box::new("actual host level panic"))
    }
    fn expression_cache_domain(&self) -> u64 {
        event(&self.trace, "domain");
        self.domain
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}
fn probe(trace: &Trace, drops: &Arc<AtomicUsize>) -> Arc<Probe> {
    Arc::new(Probe {
        trace: trace.clone(),
        drops: drops.clone(),
        refuse_output: AtomicBool::new(false),
        domain: 0,
    })
}

#[test]
fn callback_denial_precedes_arguments_and_unrestricted_keeps_other_access_checks() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator(options);
        let trace = Trace::default();
        let drops = Arc::new(AtomicUsize::new(0));
        let host = probe(&trace, &drops);
        ev.set_host(host.clone());
        assert_eq!(take(&trace), ["domain"]);
        assert_eq!(
            run(&ev, "Calc").expect_err("native denial").message(),
            "native denied"
        );
        assert_eq!(take(&trace), ["native-denied"]);
        assert_eq!(ev.take_output(), Vec::<String>::new());
        {
            let _scope = ev.native_hook_scope(NativeHooks::Unrestricted);
            assert_eq!(run(&ev, "Calc").expect("original kernel"), Value::Int(2));
            assert_eq!(take(&trace), ["print"]);
            assert_eq!(ev.take_output().len(), 1);
            host.refuse_output.store(true, Ordering::Relaxed);
            assert_eq!(
                run(&ev, "Output")
                    .expect_err("output hook retained")
                    .message(),
                "output denied"
            );
            assert_eq!(take(&trace), ["print"]);
            assert_eq!(ev.take_output(), Vec::<String>::new());
        }
        // Neither policy selection nor restoration re-queries a cache domain.
        assert_eq!(take(&trace), [] as [&str; 0]);
        assert_eq!(
            run(&ev, "Calc")
                .expect_err("restored native denial")
                .message(),
            "native denied"
        );
        drop(host);
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        drop(ev);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn complete_binding_restores_mode_cache_and_owner_but_legacy_setters_clear_policy() {
    let ev = evaluator(Options::tlc());
    let trace = Trace::default();
    let drops = Arc::new(AtomicUsize::new(0));
    let host = probe(&trace, &drops);
    ev.set_host(host.clone());
    take(&trace);
    let before = ev.host_binding_stamp();
    let scope = ev
        .native_hook_scope(NativeHooks::Unrestricted)
        .expect("ordinary domain");
    let unrestricted = ev.host_binding_stamp();
    assert_ne!(before.generation(), unrestricted.generation());
    assert_eq!(run(&ev, "Calc").expect("unrestricted"), Value::Int(2));
    let saved = ev.replace_host_binding(HostBinding::callback(Some(host.clone())));
    assert_eq!(saved.native_hooks(), NativeHooks::Unrestricted);
    assert!(run(&ev, "Calc").is_err());
    drop(ev.replace_host_binding(saved));
    assert_eq!(
        ev.host_binding_stamp().native_hooks(),
        NativeHooks::Unrestricted
    );
    let old = ev
        .replace_host(Some(host.clone()))
        .expect("displaced owner");
    assert!(Arc::ptr_eq(&old, &(host.clone() as Arc<dyn Host>)));
    assert_eq!(
        ev.host_binding_stamp().native_hooks(),
        NativeHooks::Callback
    );
    assert!(run(&ev, "Calc").is_err());
    drop(old);
    drop(scope);
    assert_eq!(ev.core.expression_cache(), 0);
    assert_eq!(
        ev.host_binding_stamp().native_hooks(),
        NativeHooks::Callback
    );
    let final_stamp = ev.host_binding_stamp();
    assert_ne!(final_stamp.generation(), before.generation());
    assert_ne!(final_stamp.generation(), unrestricted.generation());
    drop(host);
    assert_eq!(drops.load(Ordering::Relaxed), 0);
    drop(ev);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

#[test]
fn actual_host_panic_restores_complete_binding_without_converting_the_unwind() {
    let ev = evaluator(Options::tlc());
    let trace = Trace::default();
    let drops = Arc::new(AtomicUsize::new(0));
    ev.set_host(probe(&trace, &drops));
    let before = ev.host_binding_stamp();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _scope = ev.native_hook_scope(NativeHooks::Unrestricted);
        // TLCGet("level") is literal zero in CONST/INIT; state evaluation
        // reaches the real host callback whose unwind this control exercises.
        let def = ev.ir().root_def("Level").expect("level definition");
        let _ = ev.eval_in(def, ev.ir().def(def).body, &St::state(Arc::from([])));
    }))
    .expect_err("real unchanged host panic");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"actual host level panic")
    );
    assert_eq!(
        ev.host_binding_stamp().native_hooks(),
        NativeHooks::Callback
    );
    assert_ne!(ev.host_binding_stamp().generation(), before.generation());
    assert_eq!(take(&trace), ["domain", "level-panic"]);
    assert!(run(&ev, "Calc").is_err());
    assert_eq!(take(&trace), ["native-denied"]);
    assert_eq!(drops.load(Ordering::Relaxed), 0);
    drop(ev);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

#[derive(Debug)]
struct Reentrant {
    core: Weak<Core>,
    trace: Trace,
}
impl Host for Reentrant {
    fn check_native_access(&self, _op: StdOp) -> std::result::Result<(), HostError> {
        event(&self.trace, "enter");
        let core = self.core.upgrade().expect("live original Core");
        drop(core.host.replace(None));
        event(&self.trace, "exit");
        Ok(())
    }
}
impl Drop for Reentrant {
    fn drop(&mut self) {
        event(&self.trace, "drop");
    }
}
#[test]
fn native_callback_can_replace_itself_before_arguments_and_drops_after_return() {
    let ev = evaluator(Options::tlc());
    let trace = Trace::default();
    ev.set_host(Arc::new(Reentrant {
        core: Arc::downgrade(&ev.core),
        trace: trace.clone(),
    }));
    assert_eq!(run(&ev, "Calc").expect("hook outside mutex"), Value::Int(2));
    assert_eq!(take(&trace), ["enter", "exit", "drop"]);
    assert_eq!(ev.take_output().len(), 1);
    assert_eq!(
        ev.host_binding_stamp().native_hooks(),
        NativeHooks::Callback
    );
}

#[test]
fn exhausted_generation_never_reappears_and_original_permission_checks_continue() {
    let ev = evaluator(Options::tlc());
    let trace = Trace::default();
    let drops = Arc::new(AtomicUsize::new(0));
    ev.core
        .host
        .host
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .generation = u64::MAX;
    ev.set_host(probe(&trace, &drops));
    assert_eq!(ev.host_binding_stamp().generation(), None);
    assert!(ev.native_hook_scope(NativeHooks::Unrestricted).is_none());
    assert!(
        run(&ev, "Calc").is_err(),
        "original callback remains active"
    );
    assert_eq!(ev.host_binding_stamp().generation(), None);
    assert!(run(&ev, "Calc").is_err());
    assert!(take(&trace).contains(&"native-denied"));
}

#[derive(Debug)]
struct Stop;
impl HostControl for Stop {
    fn check(&self) -> std::result::Result<(), Interrupted> {
        Err(Interrupted::new(731))
    }
}
#[test]
fn absent_host_cannot_claim_unrestricted_and_real_control_is_independent() {
    let ev = evaluator(Options::tlc());
    assert!(ev.native_hook_scope(NativeHooks::Unrestricted).is_none());
    assert_eq!(
        ev.host_binding_stamp().native_hooks(),
        NativeHooks::Callback
    );
    let trace = Trace::default();
    let drops = Arc::new(AtomicUsize::new(0));
    ev.set_host(probe(&trace, &drops));
    let _hooks = ev.native_hook_scope(NativeHooks::Unrestricted);
    {
        let _control = ev
            .host_control(Some(Arc::new(Stop)))
            .expect("control owner");
        let error = run(&ev, "Loop").expect_err("typed interruption");
        assert_eq!(error.interruption(), Some(Interrupted::new(731)));
    }
    assert_eq!(run(&ev, "Loop").expect("control restored"), Value::TRUE);
}

#[test]
fn independent_guard_domain_declines_and_cannot_reuse_a_warmed_ordinary_thunk() {
    use crate::env::{Sv, Thunk};
    let ev = evaluator(Options::tlc());
    let trace = Trace::default();
    let drops = Arc::new(AtomicUsize::new(0));
    let guard = Arc::new(Probe {
        trace: trace.clone(),
        drops: drops.clone(),
        refuse_output: AtomicBool::new(false),
        domain: 7,
    });
    let def = ev.ir().root_def("Pure").expect("pure native body");
    let mut scope = ev.scope(def).expect("original frame");
    let expr = ev.ir().def(def).body;
    let thunk = Thunk::new(expr, scope.env.clone(), true);
    let slot = Sv::Thunk(thunk.clone());
    let state = St::state(Arc::from([]));
    ev.set_host(guard.clone());
    let before = ev.host_binding_stamp();
    assert!(ev.native_hook_scope(NativeHooks::Unrestricted).is_none());
    assert_eq!(
        ev.host_binding_stamp(),
        before,
        "decline changes no installation"
    );
    assert_eq!(
        ev.core
            .force(&mut scope.env, &slot, &state, expr)
            .expect_err("original guard executes")
            .message(),
        "native denied"
    );
    assert!(thunk.cached(&state, 7).is_none());
    // Positive real warm under the new admitted ordinary-domain operation.
    ev.set_host(probe(&trace, &drops));
    {
        let _ordinary = ev
            .native_hook_scope(NativeHooks::Unrestricted)
            .expect("ordinary authority only");
        let value = ev
            .core
            .force(&mut scope.env, &slot, &state, expr)
            .expect("actual ordinary force");
        assert_eq!(ev.core.force_fcn(value).expect("scalar"), Value::Int(2));
    }
    assert!(thunk.cached(&state, 0).is_some());
    // Callback restoration is not revocation of the shared ordinary cache.
    let value = ev
        .core
        .force(&mut scope.env, &slot, &state, expr)
        .expect("same ordinary authority remains cached");
    assert_eq!(ev.core.force_fcn(value).expect("scalar"), Value::Int(2));
    ev.set_host(guard.clone());
    assert!(ev.native_hook_scope(NativeHooks::Unrestricted).is_none());
    assert_eq!(
        ev.core
            .force(&mut scope.env, &slot, &state, expr)
            .expect_err("warm ordinary value is not guard authority")
            .message(),
        "native denied"
    );
    assert!(thunk.cached(&state, 7).is_none());
    assert_eq!(
        take(&trace),
        [
            "domain",
            "native-denied",
            "domain",
            "domain",
            "native-denied"
        ]
    );
}
