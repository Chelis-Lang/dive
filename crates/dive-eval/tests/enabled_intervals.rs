//! ENABLED interval search keeps candidate order and cooperative interruption.
#![allow(clippy::expect_used, reason = "finite evaluator control assertions")]

use dive_eval::{ErrorClass, Evaluator, HostControl, Interrupted, Model, Options, St};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
struct Stop {
    calls: AtomicUsize,
}
impl HostControl for Stop {
    fn check(&self) -> Result<(), Interrupted> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(Interrupted::new(0))
    }
}

fn evaluator(options: Options) -> Evaluator {
    let source = r"---- MODULE EnabledIntervals ----
EXTENDS Naturals
VARIABLE x
First == ENABLED (x' \in 1..1000000 /\ IF x'=1 THEN TRUE ELSE 1 \div 0=0)
Last == ENABLED (x' \in 1..4 /\ x'=4)
Empty == ENABLED (x' \in 2..1 /\ 1 \div 0=0)
Assigned == ENABLED (x'=3 /\ x' \in 1..2)
WideReject == ENABLED (x' \in 1..1000000 /\ FALSE)
FiniteReject == ENABLED (x' \in 1..4096 /\ FALSE)
FirstError == ENABLED (x' \in 1..1000000 /\ 1 \div 0=0)
BigLast == ENABLED (x' \in 9223372036854775806..9223372036854775808 /\ x'=9223372036854775808)
BigEmpty == ENABLED (x' \in 9223372036854775808..9223372036854775806 /\ 1 \div 0=0)
====
";
    let analysis = dive_sema::analyze_source(
        "EnabledIntervals.tla",
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    Evaluator::new(Model::new(ir), options).expect("evaluator")
}

fn run(ev: &Evaluator, name: &str) -> dive_eval::Result<Value> {
    let def = ev.ir().root_def(name).expect("definition");
    ev.eval_in(def, ev.ir().def(def).body, &St::constant())
}

#[test]
fn interval_witness_order_empty_and_assigned_membership_keep_their_answers() {
    for options in [Options::tlc(), Options::dive()] {
        let ev = evaluator(options);
        for (name, expected) in [
            ("First", true),
            ("Last", true),
            ("Empty", false),
            ("Assigned", false),
        ] {
            assert_eq!(run(&ev, name).expect("answer"), Value::Bool(expected));
        }
    }
}

#[test]
fn rejected_enabled_candidates_interrupt_and_do_not_cache_an_incomplete_answer() {
    for options in [Options::tlc(), Options::dive()] {
        let ev = evaluator(options);
        let control = Arc::new(Stop {
            calls: AtomicUsize::new(0),
        });
        let scope = ev.host_control(Some(control.clone())).expect("control");
        assert_eq!(
            run(&ev, "WideReject").expect_err("interruption").class(),
            ErrorClass::Interrupted
        );
        assert!(control.calls.load(Ordering::Relaxed) > 0);
        drop(scope);
        assert_eq!(
            run(&ev, "FiniteReject").expect("retry after scope"),
            Value::Bool(false)
        );
        assert_eq!(run(&ev, "First").expect("new witness"), Value::Bool(true));
    }
}

#[test]
fn first_candidate_error_precedes_the_next_amortized_host_check() {
    std::thread::spawn(|| {
        for options in [Options::tlc(), Options::dive()] {
            let ev = evaluator(options);
            let control = Arc::new(Stop {
                calls: AtomicUsize::new(0),
            });
            let _scope = ev.host_control(Some(control.clone())).expect("control");
            assert_eq!(
                run(&ev, "FirstError").expect_err("division error").class(),
                ErrorClass::Eval
            );
            assert_eq!(control.calls.load(Ordering::Relaxed), 0);
        }
    })
    .join()
    .expect("fresh host counter thread");
}

#[test]
fn big_integer_interval_crosses_the_machine_boundary_without_overflow() {
    let ev = evaluator(Options::dive());
    assert_eq!(
        run(&ev, "BigLast").expect("large witness"),
        Value::Bool(true)
    );
    assert_eq!(
        run(&ev, "BigEmpty").expect("empty large interval"),
        Value::Bool(false)
    );
}
