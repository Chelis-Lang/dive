#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "actual runtime shape assertions"
)]
use crate::env::{Ev, Sv};
use crate::{Evaluator, Model, Options, St, StateRef};
use dive_value::{Ctx, Value};
use std::sync::Arc;

#[test]
fn lazy_function_keeps_actual_self_domain_lexical_and_state_captures() {
    let source = "---- MODULE SelfShape ----\nEXTENDS Naturals\nVARIABLE v\nF(S)== LET f[x \\in S] == IF x=0 THEN v ELSE f[x-1] IN f\n====\n";
    let analysis =
        dive_sema::analyze_source("SelfShape.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let def = ir.root_def("F").expect("F");
    let ev = Evaluator::new(Model::new(ir), Options::tlc()).expect("owned core");
    let domain = Value::set(vec![0.into(), 1.into()], Ctx::tlc()).expect("domain");
    let scope = ev
        .scope_call_values(&ev.scope(def).expect("scope"), def, vec![domain.clone()])
        .expect("call");
    let states: Arc<[Option<Value>]> = Arc::from([Some(7.into())]);
    let st = St::state(states.clone());
    let Ev::F(function) = ev
        .core
        .eval(&mut scope.env.clone(), &st, ev.ir().def(def).body)
        .expect("lazy eval")
    else {
        panic!("actual lazy function");
    };
    assert!(Arc::ptr_eq(&function.core, &ev.core));
    assert_eq!(function.bounds.len(), 1);
    assert_eq!(function.bounds[0].domain, domain);
    assert!(matches!(&function.st.s0, StateRef::Vals(actual) if Arc::ptr_eq(actual, &states)));
    assert!(matches!(&function.env.frame.slots[0], Sv::Val(value) if *value == domain));
    let body = function.body_env();
    assert!(
        matches!(&body.frame.slots[function.rec_slot.expect("self slot")], Sv::Fcn(actual) if Arc::ptr_eq(actual, &function))
    );
    assert!(matches!(&body.frame.slots[0], Sv::Val(value) if *value == domain));
    let result = ev
        .core
        .apply_lazy(&function, &Value::Int(1), function.st.control)
        .expect("existing recursive application");
    assert_eq!(ev.core.force_fcn(result).expect("value"), Value::Int(7));
}
