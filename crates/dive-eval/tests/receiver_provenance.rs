//! Source hooks belong to actual receivers and the evaluator that built them.
#![allow(clippy::expect_used, reason = "actual evaluator regression controls")]
use dive_eval::{
    ConstBinding, ErrorClass, Evaluator, HostControl, Interrupted, Model, Options, St,
};
use dive_value::Value;
use std::sync::Arc;
fn evaluator(source: &str) -> Evaluator {
    evaluator_with_options(source, Options::tlc())
}
fn evaluator_with_options(source: &str, options: Options) -> Evaluator {
    let text = format!(
        "---- MODULE Receiver ----\nEXTENDS Integers,FiniteSets\nVARIABLE x\n{source}\n====\n"
    );
    let analysis = dive_sema::analyze_source("Receiver.tla", &text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    Evaluator::new(
        Model::new(Arc::new(dive_ir::lower(&analysis).expect("IR"))),
        options,
    )
    .expect("evaluator")
}
fn state() -> St {
    St::state(Arc::from([Some(Value::Int(7))]))
}
#[test]
fn foreign_receiver_source_cannot_be_recovered_by_another_evaluator() {
    let a = evaluator("CONSTANT C\nMake==<<x>>\nRead==C#[a|->x]");
    let value = a
        .eval_def(
            a.ir().root_def("Make").expect("Make"),
            &state().diagnostic(),
        )
        .expect("actual receiver");
    let mut model = Model::new(a.ir().clone());
    model.constants[0] = Some(ConstBinding::Value(value));
    let b = Evaluator::new(model, Options::tlc()).expect("second evaluator");
    let error = b
        .eval_def(
            b.ir().root_def("Read").expect("Read"),
            &state().diagnostic(),
        )
        .expect_err("actual comparison failure");
    assert_eq!(error.class(), ErrorClass::Eval);
    assert!(error.value_method().is_none());
}
#[derive(Debug)]
struct Stop;
impl HostControl for Stop {
    fn check(&self) -> Result<(), Interrupted> {
        Err(Interrupted::new(620))
    }
}
#[test]
fn diagnostic_constructor_evaluation_keeps_the_actual_interrupt_owner() {
    let ev = evaluator("Read==<<IF (\\A j \\in 1..20000 : j>=0) THEN x ELSE 0>>#[a|->x]");
    let guard = ev.host_control(Some(Arc::new(Stop))).expect("owner");
    let error = ev
        .eval_def(
            ev.ir().root_def("Read").expect("Read"),
            &state().diagnostic(),
        )
        .expect_err("owned stop");
    assert_eq!(error.class(), ErrorClass::Interrupted);
    assert_eq!(error.interruption().map(Interrupted::token), Some(620));
    assert!(error.value_method().is_none());
    drop(guard);
    let error = ev
        .eval_def(
            ev.ir().root_def("Read").expect("Read"),
            &state().diagnostic(),
        )
        .expect_err("now actual comparison");
    assert_eq!(error.class(), ErrorClass::Eval);
    assert!(
        error
            .value_method()
            .is_some_and(|failure| failure.source().is_some())
    );
}

#[test]
fn fresh_literal_catches_preserve_scopes_unused_arguments_and_warm_cache_lifetimes() {
    for options in [Options::tlc(), Options::dive()] {
        for body in [
            "T==<<x>>\nRead==T#FALSE",
            "Read==[a|->x]#FALSE",
            "Read==LET T==<<x>> IN T#FALSE",
            "Ignore(a)==<<x>>#FALSE\nRead==Ignore(1 \\div 0)",
            "---- MODULE Inner ----\nVARIABLE y\nRead==<<y>>#FALSE\n====\nI==INSTANCE Inner WITH y<-x\nRead==I!Read",
        ] {
            let ev = evaluator_with_options(body, options);
            let def = ev.ir().root_def("Read").expect("Read");
            for _ in 0..3 {
                let original = ev
                    .eval_def(def, &state())
                    .expect_err("actual ordinary receiver error");
                assert_eq!(
                    original.ec().code(),
                    1000,
                    "unused division must stay unforced"
                );
                let receiver = original
                    .value_method()
                    .expect("actual literal receiver catch");
                assert!(receiver.source().is_none());
                let diagnostic = ev
                    .eval_def(def, &state().diagnostic())
                    .expect_err("fresh diagnostic construction");
                let actual = diagnostic
                    .value_method()
                    .expect("actual diagnostic receiver catch");
                assert_eq!(actual.method(), receiver.method());
                assert_eq!(actual.constructor(), receiver.constructor());
                assert_eq!(actual.source(), Some(actual.constructor()));
            }
        }
    }
}
