//! Cooperative construction preserves value/error and receiver semantics.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_value::{ConstructionError, Ctx, Interner, Value};

fn signature(value: &Value) -> String {
    match value {
        Value::Fcn(f) => format!(
            "{:?}/{}/{}",
            f.kind(),
            f.is_tuple_literal(),
            f.is_record_literal()
        ),
        Value::Set(s) => format!("{:?}", s.kind()),
        other => format!("{other:?}"),
    }
}

fn same(expected: dive_value::Result<Value>, actual: Result<Value, ConstructionError>) {
    match (expected, actual) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a, b);
            assert_eq!(signature(&a), signature(&b));
            assert_eq!(a.hash64(), b.hash64());
        }
        (Err(a), Err(ConstructionError::Value(b))) => assert_eq!(a, b),
        (a, b) => panic!("constructor disagreement: {a:?} / {b:?}"),
    }
}

fn values(ctx: Ctx, names: &Interner) -> Vec<Value> {
    let a = names.intern("z");
    let b = names.intern("a");
    vec![
        Value::Int(1),
        Value::Int(2),
        Value::Bool(false),
        Value::Str(a.clone()),
        Value::Model(names.intern("m")),
        Value::Model(names.intern("T_x")),
        Value::tuple(vec![]),
        Value::tuple(vec![Value::Int(1)]),
        Value::tuple_fcn(vec![Value::Int(1)]),
        Value::record(
            vec![(a.clone(), Value::Int(1)), (b.clone(), Value::Int(2))],
            ctx,
        )
        .expect("record"),
        Value::fcn(
            vec![
                (Value::Str(a), Value::Int(1)),
                (Value::Str(b), Value::Int(2)),
            ],
            ctx,
        )
        .expect("function"),
        Value::set(vec![Value::Int(2), Value::Int(1)], ctx).expect("set"),
        Value::set(vec![Value::tuple(vec![Value::Int(1)])], ctx).expect("nested set"),
        Value::fcn(vec![(Value::Int(0), Value::Int(1))], ctx).expect("general function"),
    ]
}

#[test]
fn sets_and_functions_keep_values_classes_survivors_and_exact_errors() {
    for ctx in [Ctx::tlc(), Ctx::default()] {
        let names = Interner::new();
        let values = values(ctx, &names);
        for a in &values {
            for b in &values {
                for c in &values {
                    let elements = vec![a.clone(), b.clone(), c.clone()];
                    same(
                        Value::set(elements.clone(), ctx),
                        Value::set_controlled(elements, ctx, &mut || true),
                    );
                    let pairs = vec![
                        (a.clone(), Value::Int(7)),
                        (b.clone(), Value::Int(8)),
                        (c.clone(), Value::Int(9)),
                    ];
                    same(
                        Value::fcn(pairs.clone(), ctx),
                        Value::fcn_controlled(pairs, ctx, &mut || true),
                    );
                }
            }
        }
    }
}

#[test]
fn records_keep_name_order_and_duplicate_messages() {
    let names = Interner::new();
    let fields = [names.intern("z"), names.intern("a"), names.intern("m")];
    for ctx in [Ctx::tlc(), Ctx::default()] {
        for order in [[0, 1, 2], [2, 1, 0], [0, 1, 0], [1, 1, 0]] {
            let fields = order
                .into_iter()
                .map(|at| (fields[at].clone(), Value::Int(1)))
                .collect::<Vec<_>>();
            same(
                Value::record(fields.clone(), ctx),
                Value::record_controlled(fields, ctx, &mut || true),
            );
        }
    }
}

#[test]
fn every_normalization_checkpoint_can_interrupt_without_returning_a_value() {
    let ctx = Ctx::default();
    let elements = (0..32)
        .rev()
        .map(|at| Value::tuple(vec![Value::Int(at), Value::Int(0)]))
        .collect::<Vec<_>>();
    let mut count = 0;
    Value::set_controlled(elements.clone(), ctx, &mut || {
        count += 1;
        true
    })
    .expect("construction");
    assert!(
        count > 100,
        "poll inside deep normalization, not just the outer value"
    );
    for stop in 0..count {
        let mut visited = 0;
        let result = Value::set_controlled(elements.clone(), ctx, &mut || {
            let proceed = visited != stop;
            visited += 1;
            proceed
        });
        assert!(
            matches!(result, Err(ConstructionError::Interrupted)),
            "checkpoint {stop}: {result:?}"
        );
        assert_eq!(visited, stop + 1);
    }
}
