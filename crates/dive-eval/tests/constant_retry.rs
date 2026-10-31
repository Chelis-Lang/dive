//! Failed startup evaluation must not erase the actual lazy call-site demand.
#![allow(clippy::expect_used, reason = "small source-bound regression fixtures")]

use dive_eval::{
    ConstBinding, ErrorClass, Evaluator, HostControl, Interrupted, Model, Options, St,
};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn evaluator(body: &str, options: Options) -> Evaluator {
    let source = format!(
        "---- MODULE Retry ----\nEXTENDS Integers, Sequences, TLC\nVARIABLE x\n{body}\n====\n"
    );
    let analysis = dive_sema::analyze_source("Retry.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    Evaluator::new(Model::new(ir), options).expect("evaluator")
}
fn run(ev: &Evaluator, name: &str) -> dive_eval::Result<Value> {
    ev.eval_def(
        ev.ir().root_def(name).expect("definition"),
        &St::state(Arc::from([Some(Value::Int(7))])),
    )
}
const FUNCTION: &str = "[i \\in 0..2 |-> IF i=0 THEN 0 ELSE 1 \\div 0]";

#[test]
fn failed_constant_preevaluation_keeps_caller_function_selection_demand() {
    for options in [Options::tlc(), Options::dive()] {
        for body in [
            format!("F == {FUNCTION}\nRead == F[0]"),
            format!("Read == {FUNCTION}[0]"),
            format!("Read == LET F == {FUNCTION} IN F[0]"),
            format!(
                "---- MODULE Inner ----\nF == {FUNCTION}\nRead == F[0]\n====\nI == INSTANCE Inner\nRead == I!Read"
            ),
        ] {
            for warm in [false, true] {
                let ev = evaluator(&body, options);
                if warm && let Some(function) = ev.ir().root_def("F") {
                    assert!(
                        ev.precompute(function).is_err(),
                        "genuine failed startup work"
                    );
                    assert!(ev.precomputed(function).is_none());
                }
                for _ in 0..3 {
                    assert_eq!(
                        run(&ev, "Read").expect("only entry0 demanded"),
                        Value::Int(0),
                        "{body}"
                    );
                }
            }
        }
    }
}

#[test]
fn demanded_errors_and_nonlazy_consumers_are_not_hidden_or_cached() {
    for options in [Options::tlc(), Options::dive()] {
        let ev = evaluator(
            &format!(
                "F == {FUNCTION}\nRead == F[0]\nBad == F[1]\nWhole == F=F\nOutside == F[9]\nOutsideInline == {FUNCTION}[9]\nDirect == 1 \\div 0\nUnused(a) == F[0]\nWithUnused == Unused(1 \\div 0)"
            ),
            options,
        );
        for _ in 0..2 {
            assert_eq!(run(&ev, "Read").expect("good key"), Value::Int(0));
            assert_eq!(
                run(&ev, "WithUnused").expect("unused failing argument remains lazy"),
                Value::Int(0)
            );
            for name in ["Bad", "Whole", "Direct"] {
                let error = run(&ev, name).expect_err("genuine demanded error");
                assert_eq!(error.class(), ErrorClass::Eval);
                assert_eq!(error.ec().code(), 2179, "{name}");
            }
            let outside = run(&ev, "Outside").expect_err("actual absent key");
            assert_eq!(outside.class(), ErrorClass::Eval);
            assert_eq!(
                outside.ec(),
                run(&ev, "OutsideInline")
                    .expect_err("inline absent key")
                    .ec()
            );
            assert_ne!(
                outside.ec().code(),
                2179,
                "an absent key does not demand the failing body entry"
            );
        }
    }
}

#[test]
fn diagnostic_source_belongs_to_the_actual_materializing_receiver() {
    for options in [Options::tlc(), Options::dive()] {
        let ev = evaluator(
            &format!(
                "F == {FUNCTION}\nWhole == F=F\nBad == F[1]\nRead == F[0]\nDirect == 1 \\div 0"
            ),
            options,
        );
        let original = run(&ev, "Whole").expect_err("actual whole-function failure");
        let receiver = original.value_method().expect("materialization catch");
        assert_eq!(
            receiver.method(),
            dive_eval::ValueMethod::FunctionMaterialize
        );
        assert!(
            receiver.source().is_none(),
            "ordinary tool has no source hook"
        );
        let context = St::state(Arc::from([Some(Value::Int(7))])).diagnostic();
        let whole = ev.ir().root_def("Whole").expect("Whole");
        let replay = ev
            .eval_def(whole, &context)
            .expect_err("actual diagnostic replay");
        let actual = replay
            .value_method()
            .expect("new receiver caught the body error");
        assert_eq!(actual.constructor(), receiver.constructor());
        assert_eq!(actual.source(), Some(actual.constructor()));
        assert_eq!(
            replay.ec(),
            original.ec(),
            "underlying division is preserved"
        );
        for name in ["Bad", "Direct"] {
            let definition = ev.ir().root_def(name).expect("definition");
            let error = ev
                .eval_def(definition, &context)
                .expect_err("demanded division");
            assert_eq!(error.ec().code(), 2179);
            assert!(
                error.value_method().is_none(),
                "{name}: no materialization catch"
            );
        }
        let read = ev.ir().root_def("Read").expect("Read");
        assert_eq!(
            ev.eval_def(read, &context)
                .expect("only good entry demanded"),
            Value::Int(0)
        );
        assert!(
            dive_eval::EvalError::coded(original.ec(), original.message())
                .value_method()
                .is_none()
        );
    }
}

#[test]
fn successful_startup_values_and_stateful_overrides_keep_their_cache_boundaries() {
    for options in [Options::tlc(), Options::dive()] {
        let ev = evaluator("F == [i \\in 0..2 |-> i+1]\nRead == F[0]", options);
        let f = ev.ir().root_def("F").expect("F");
        let cached = ev
            .precompute(f)
            .expect("successful precomputation")
            .expect("stored function");
        for _ in 0..3 {
            assert_eq!(run(&ev, "Read").expect("cached selection"), Value::Int(1));
            assert_eq!(ev.precomputed(f), Some(cached.clone()));
        }
        let ev = evaluator("CONSTANT C\nState == x\nRead == C+1", options);
        let mut model = Model::new(ev.ir().clone());
        model.constants[0] = Some(ConstBinding::Def(
            ev.ir().root_def("State").expect("configured target"),
        ));
        let bound = Evaluator::new(model, options).expect("actual binding");
        let read = bound.ir().root_def("Read").expect("Read");
        for value in [3, 9, 3] {
            assert_eq!(
                bound
                    .eval_def(read, &St::state(Arc::from([Some(Value::Int(value))])))
                    .expect("stateful constant override is not a constant cache"),
                Value::Int(value + 1)
            );
        }
        let effectful = evaluator("F == PrintT(TRUE)\nRead == F /\\ F", options);
        for _ in 0..2 {
            assert_eq!(
                run(&effectful, "Read").expect("effects retain actual demands"),
                Value::TRUE
            );
            assert_eq!(effectful.take_output(), ["TRUE", "TRUE"]);
        }
        // A LET's actual scope is retained; the fallback must not use a fresh
        // empty frame or cache a value computed with caller-specific bindings.
        let ev = evaluator(
            &format!("F == {FUNCTION}\nRead == LET a == x IN F[0]+a"),
            options,
        );
        assert_eq!(
            run(&ev, "Read").expect("actual state in caller"),
            Value::Int(7)
        );
    }
}

#[derive(Debug)]
struct OneShot {
    calls: AtomicUsize,
    at: usize,
}
impl HostControl for OneShot {
    fn check(&self) -> Result<(), Interrupted> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == self.at {
            Err(Interrupted::new(281))
        } else {
            Ok(())
        }
    }
}
#[test]
fn failed_constant_retry_never_swallows_owned_interruption_or_resource_limit() {
    for options in [Options::tlc(), Options::dive()] {
        for at in [0, 3, 8] {
            // The real quantified body crosses the evaluator's 1024-row poll
            // cadence; each chosen stop occurs inside the constant probe.
            let ev = evaluator(
                "F == [i \\in 0..1 |-> IF i=0 THEN 0 ELSE IF (\\A j \\in 1..20000: j>=0) THEN 1 \\div 0 ELSE 0]\nRead == F[0]",
                options,
            );
            let control = Arc::new(OneShot {
                calls: AtomicUsize::new(0),
                at,
            });
            let guard = ev
                .host_control(Some(control.clone()))
                .expect("owned control");
            let error = run(&ev, "Read").expect_err("even a one-shot owned receipt must propagate");
            assert_eq!(error.class(), ErrorClass::Interrupted);
            assert_eq!(error.interruption(), Some(Interrupted::new(281)));
            assert_eq!(
                control.calls.load(Ordering::SeqCst),
                at + 1,
                "no retry after observed stop"
            );
            drop(guard);
            assert_eq!(
                run(&ev, "Read").expect("fresh owner can select"),
                Value::Int(0)
            );
        }
        let mut chain = "D0 == 1\n".to_owned();
        for index in 1..40 {
            use std::fmt::Write as _;
            writeln!(chain, "D{index} == D{}+1", index - 1).expect("source");
        }
        let ev = evaluator(
            &format!("{chain}F == [i \\in 0..1 |-> IF i=0 THEN 0 ELSE D39]\nRead == F[0]"),
            Options {
                max_depth: 12,
                ..options
            },
        );
        let function = ev.ir().root_def("F").expect("F");
        assert_eq!(
            ev.precompute(function)
                .expect_err("actual constant probe exceeds depth")
                .class(),
            ErrorClass::ResourceLimit
        );
        assert_eq!(
            run(&ev, "Read")
                .expect_err(
                    "constant probe's resource limit cannot become successful lazy fallback"
                )
                .class(),
            ErrorClass::ResourceLimit
        );
    }
}
