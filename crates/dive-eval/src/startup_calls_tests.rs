//! Start-up call results: kept only for pure calls on settled arguments, in
//! the default profile, and matched by identity.
#![allow(clippy::expect_used, reason = "finite test assertions")]

use std::sync::Arc;
use std::sync::atomic::Ordering;

use dive_value::{Int, Value};

use crate::{ConstBinding, Evaluator, Model, Options};

const MODULE: &str = r"---- MODULE Startup ----
EXTENDS Naturals, FiniteSets, TLC
CONSTANT N, A, B
Pairs(S) == {<<a, b>> : a \in S, b \in S}
Id(S) == S
Show(x) == PrintT(x)
Repeated == {Cardinality(Pairs(N)) : i \in 1..50}
Forms == <<Id(A), Id(B), Id(A)>>
Printed == Show(N) /\ Show(N)
====
";

fn evaluator(options: Options) -> Evaluator {
    let analysis = dive_sema::analyze_source("Startup.tla", MODULE, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    let mut model = Model::new(ir);
    let ctx = options.ctx;
    let n = Value::set(vec![1.into(), 2.into(), 3.into()], ctx).expect("N");
    let a = Value::interval(Int::from(1), Int::from(3), ctx);
    let b = Value::set(vec![1.into(), 2.into(), 3.into()], ctx).expect("B");
    model.constants[0] = Some(ConstBinding::Value(n));
    model.constants[1] = Some(ConstBinding::Value(a));
    model.constants[2] = Some(ConstBinding::Value(b));
    Evaluator::new(model, options).expect("evaluator")
}

fn hits(ev: &Evaluator) -> u64 {
    ev.core.startup_call_hits.load(Ordering::Relaxed)
}

fn precompute(ev: &Evaluator, name: &str) -> Value {
    let def = ev.ir().root_def(name).expect("definition");
    ev.precompute(def).expect("evaluates").expect("a value")
}

#[test]
fn repeated_pure_calls_are_evaluated_once_in_the_default_profile_only() {
    for (options, kept) in [(Options::dive(), true), (Options::tlc(), false)] {
        let ev = evaluator(options);
        let value = precompute(&ev, "Repeated");
        let ctx = options.ctx;
        let expected = Value::set(vec![9.into()], ctx).expect("expected");
        assert_eq!(value, expected);
        if kept {
            assert_eq!(hits(&ev), 49, "every call after the first is kept");
        } else {
            assert_eq!(hits(&ev), 0, "--compat=tlc evaluates every call");
        }
    }
}

#[test]
fn arguments_match_by_identity_not_equality() {
    let options = Options::dive();
    let ev = evaluator(options);
    let value = precompute(&ev, "Forms");
    // `A` (an interval) and `B` (an enumerated set) are equal but print
    // differently; only the second use of `A` is answered from the results.
    let plain = evaluator(options);
    let def = plain.ir().root_def("Forms").expect("Forms");
    let reference = plain
        .eval_in(def, plain.ir().def(def).body, &crate::St::constant())
        .expect("evaluates");
    let print = |v: &Value| dive_value::print::to_tlc_string(v, options.ctx).expect("prints");
    assert_eq!(print(&value), print(&reference));
    assert_eq!(print(&value), "<<1..3, {1, 2, 3}, 1..3>>");
    assert_eq!(hits(&ev), 1);
}

#[test]
fn impure_calls_are_evaluated_at_every_use() {
    let ev = evaluator(Options::dive());
    let value = precompute(&ev, "Printed");
    assert_eq!(value, Value::Bool(true));
    assert_eq!(ev.take_output().len(), 2);
    assert_eq!(hits(&ev), 0);
}
