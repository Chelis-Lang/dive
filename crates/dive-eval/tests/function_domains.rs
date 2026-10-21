//! Persistent function metadata preserves TLC checked selection and EXCEPT order.
#![allow(clippy::expect_used, reason = "fixture and exact error assertions")]
use dive_eval::{ErrorClass, Evaluator, Model, Options, St};
use dive_value::{Ctx, Interner, Value};
use std::sync::Arc;

fn run(
    options: Options,
    values: impl FnOnce(Ctx, &Interner) -> (Value, Value),
    operation: &str,
) -> dive_eval::Result<Value> {
    let source = "---- MODULE FunctionDomains ----\nEXTENDS Integers, TLC\nF == TRUE\nA == TRUE\nApply == F[A]\nUpdate == [F EXCEPT ![A] = 99]\nNested == [F EXCEPT ![A][0] = 99]\nBadKind == FALSE[Print(42, 0)]\n====\n";
    let analysis = dive_sema::analyze_source(
        "FunctionDomains.tla",
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let mut model = Model::new(ir.clone());
    let (f, argument) = values(options.ctx, &model.names);
    model.def_values.insert(ir.root_def("F").expect("F"), f);
    model
        .def_values
        .insert(ir.root_def("A").expect("A"), argument);
    let evaluator = Evaluator::new(model, options).expect("context");
    let def = ir.root_def(operation).expect("operation");
    evaluator.eval_in(def, ir.def(def).body, &St::constant())
}
fn grid(ctx: Ctx, argument: Value) -> (Value, Value) {
    let mut pairs = Vec::new();
    for x in 0..2 {
        for y in 0..2 {
            pairs.push((
                Value::tuple(vec![Value::Int(x), Value::Int(y)]),
                Value::Int(x * 10 + y),
            ));
        }
    }
    (Value::fcn(pairs, ctx).expect("grid"), argument)
}

#[test]
fn integer_product_hits_misses_and_late_incompatible_components_preserve_errors() {
    for options in [Options::dive(), Options::tlc()] {
        assert_eq!(
            run(
                options,
                |ctx, _| grid(ctx, Value::tuple(vec![Value::Int(1), Value::Int(0)])),
                "Apply"
            )
            .expect("hit"),
            Value::Int(10)
        );
        let miss = run(
            options,
            |ctx, _| grid(ctx, Value::tuple(vec![Value::Int(99), Value::TRUE])),
            "Apply",
        )
        .expect_err("all first coordinates miss before boolean");
        assert_eq!(miss.class(), ErrorClass::Eval);
        assert!(miss.message().contains("not in the domain"), "{miss}");
        let incompatible = run(
            options,
            |ctx, _| grid(ctx, Value::tuple(vec![Value::Int(0), Value::TRUE])),
            "Apply",
        )
        .expect_err("first coordinate matches, second errors");
        assert_eq!(incompatible.class(), ErrorClass::Eval);
        assert!(
            incompatible
                .message()
                .contains("equality of integer 0 with non-integer"),
            "{incompatible}"
        );
        for n in [31, 32] {
            let error = run(
                options,
                |ctx, _| {
                    (
                        Value::fcn(
                            (0..n).map(|i| (Value::Int(i), Value::Int(i))).collect(),
                            ctx,
                        )
                        .expect("integer domain"),
                        Value::TRUE,
                    )
                },
                "Apply",
            )
            .expect_err("incompatible argument");
            assert_eq!(
                error.class(),
                if n == 32 {
                    ErrorClass::Unsupported
                } else {
                    ErrorClass::Eval
                }
            );
        }
    }
}

#[test]
fn general_updates_and_missing_keys_keep_original_value_and_error_order() {
    for options in [Options::dive(), Options::tlc()] {
        let updated = run(
            options,
            |ctx, _| grid(ctx, Value::tuple(vec![Value::Int(1), Value::Int(0)])),
            "Update",
        )
        .expect("update");
        assert_eq!(
            updated
                .as_fcn()
                .expect("function")
                .apply(&Value::tuple(vec![Value::Int(1), Value::Int(0)])),
            Some(&Value::Int(99))
        );
        let unchanged = run(
            options,
            |ctx, _| grid(ctx, Value::tuple(vec![Value::Int(9), Value::Int(9)])),
            "Nested",
        )
        .expect("missing key skips nested malformed update");
        assert_eq!(
            unchanged
                .as_fcn()
                .expect("function")
                .apply(&Value::tuple(vec![Value::Int(1), Value::Int(0)])),
            Some(&Value::Int(10))
        );
        let error = run(
            options,
            |ctx, _| grid(ctx, Value::tuple(vec![Value::Int(0), Value::TRUE])),
            "Nested",
        )
        .expect_err("key error before nested nonfunction");
        assert!(
            error
                .message()
                .contains("equality of integer 0 with non-integer"),
            "{error}"
        );
    }
}
