//! Cooperative host control is typed, scoped and independent of evaluator caches.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite control assertions"
)]

use dive_eval::{ErrorClass, Evaluator, Host, HostControl, Interrupted, Model, Options, St};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
struct Control {
    calls: AtomicUsize,
    stop: bool,
}
impl Control {
    fn new(stop: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            stop,
        })
    }
}
impl HostControl for Control {
    fn check(&self) -> Result<(), Interrupted> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.stop {
            Err(Interrupted::new(0))
        } else {
            Ok(())
        }
    }
}
#[derive(Debug)]
struct Replacement;
impl Host for Replacement {}

fn evaluator(options: Options) -> Evaluator {
    let source = "---- MODULE Control ----\nEXTENDS Naturals, FiniteSets, TLC\nQ == \\A x \\in 1..10000: x >= 0\nNested == \\A x \\in 1..2000: \\A y \\in 1..2: y >= 0\nC == CHOOSE x \\in 1..10000: x > 10000\nF == Cardinality({f \\in [1..12 -> {0,1}] : f[1] = 0})\nE == \\A x \\in 1..10000: IF x=1 THEN (1 \\div 0)=0 ELSE TRUE\nMessage == Assert(FALSE, \"host interrupted evaluation\")\n====\n";
    let analysis = dive_sema::analyze_source("Control.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    Evaluator::new(Model::new(ir), options).expect("evaluator")
}
fn run(ev: &Evaluator, name: &str) -> dive_eval::Result<Value> {
    let def = ev.ir().root_def(name).expect("definition");
    ev.eval_in(def, ev.ir().def(def).body, &St::constant())
}
fn interrupted(ev: &Evaluator, name: &str) {
    assert_eq!(
        run(ev, name).expect_err("interruption").class(),
        ErrorClass::Interrupted
    );
}

#[test]
fn quantifier_nested_short_loops_choose_and_deferred_filter_interrupt_typed() {
    for options in [Options::tlc(), Options::dive()] {
        for name in ["Q", "Nested", "C", "F"] {
            let ev = evaluator(options);
            let control = Control::new(true);
            let scope = ev.host_control(Some(control.clone())).expect("owner");
            interrupted(&ev, name);
            assert!(control.calls.load(Ordering::Relaxed) > 0);
            drop(scope);
            // A partial deferred/constant value is never published as success.
            if name == "F" {
                let def = ev.ir().root_def(name).expect("F");
                assert_eq!(
                    ev.eval_def(def, &St::constant()).expect("retry"),
                    Value::Int(2048)
                );
            }
        }
    }
}

#[test]
fn nested_scopes_host_replacement_and_out_of_order_drop_preserve_owner() {
    let ev = evaluator(Options::tlc());
    let stopping = Control::new(true);
    let outer = ev.host_control(Some(stopping)).expect("outer");
    ev.set_host(Arc::new(Replacement));
    interrupted(&ev, "Q");
    {
        let _inner = ev.host_control(None).expect("nested suspension");
        assert_eq!(run(&ev, "Q").expect("suspended"), Value::Bool(true));
        std::thread::scope(|threads| {
            threads
                .spawn(|| interrupted(&ev, "Q"))
                .join()
                .expect("controlled worker");
        });
    }
    interrupted(&ev, "Q");
    let inner = ev.host_control(Some(Control::new(false))).expect("inner");
    drop(outer);
    assert_eq!(run(&ev, "Q").expect("inner still owns"), Value::Bool(true));
    drop(inner);
    assert_eq!(
        run(&ev, "Q").expect("old exhausted control not resurrected"),
        Value::Bool(true)
    );
    let _new = ev.host_control(Some(Control::new(true))).expect("new run");
    interrupted(&ev, "Q");
}

#[test]
fn another_driver_cannot_replace_control_but_workers_can_evaluate_under_it() {
    let ev = evaluator(Options::tlc());
    let _scope = ev.host_control(Some(Control::new(true))).expect("owner");
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                assert!(ev.host_control(None).is_err());
                interrupted(&ev, "Q");
            })
            .join()
            .expect("worker");
    });
    let independent = evaluator(Options::tlc());
    assert_eq!(
        run(&independent, "Q").expect("independent replay has no control"),
        Value::Bool(true)
    );
}

#[test]
fn genuine_error_before_interrupt_and_matching_user_text_keep_original_class() {
    // A fresh thread begins with a full amortization interval; the first
    // predicate raises its genuine error before any host checkpoint can trip.
    std::thread::spawn(|| {
        let ev = evaluator(Options::tlc());
        let control = Control::new(true);
        let _scope = ev.host_control(Some(control.clone())).expect("owner");
        assert_eq!(
            run(&ev, "E").expect_err("division error").class(),
            ErrorClass::Eval
        );
        assert_eq!(
            run(&ev, "Message").expect_err("assertion error").class(),
            ErrorClass::Eval
        );
        assert_eq!(control.calls.load(Ordering::Relaxed), 0);
    })
    .join()
    .expect("thread");
}

#[test]
fn diagnostic_pause_restores_control_on_unwind() {
    let ev = evaluator(Options::tlc());
    let _outer = ev.host_control(Some(Control::new(true))).expect("owner");
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _diagnostic = ev.host_control(None).expect("local pause");
        assert_eq!(run(&ev, "Q").expect("diagnostic"), Value::Bool(true));
        panic!("synthetic diagnostic failure");
    }));
    assert!(caught.is_err());
    interrupted(&ev, "Q");
}

#[test]
fn a_value_interruption_without_a_typed_provider_receipt_fails_closed() {
    std::thread::spawn(|| {
        let error = dive_eval::EvalError::from(dive_value::ValueError::new(
            dive_value::ErrorKind::Interrupted { receipt: 0 },
            "host interrupted evaluation",
        ));
        assert_eq!(error.class(), ErrorClass::Bug);
    })
    .join()
    .expect("thread");
}
