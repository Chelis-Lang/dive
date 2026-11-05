//! Canonical construction must not treat symbolic definitions as inequality.
use dive_value::{Ctx, ErrorKind, Interner, Value};

#[test]
fn singleton_and_nested_symbolic_elements_refuse_before_identity_use()
-> Result<(), Box<dyn std::error::Error>> {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let names = Interner::new();
        let empty = Value::set_canonical(vec![], ctx)?;
        let sequence = Value::seq_set(Value::interval(1.into(), 1.into(), ctx), ctx);
        let product = Value::product(vec![empty.clone(), Value::nat(ctx)], ctx);
        for symbolic in [sequence, product] {
            for value in [
                symbolic.clone(),
                Value::tuple(vec![symbolic.clone()]),
                Value::record(vec![(names.intern("field"), symbolic.clone())], ctx)?,
                Value::fcn(vec![(symbolic.clone(), Value::Int(1))], ctx)?,
                Value::fcn(vec![(Value::Int(0), symbolic)], ctx)?,
            ] {
                for elements in [vec![value.clone()], vec![value, empty.clone()]] {
                    assert!(
                        matches!(Value::set_canonical(elements, ctx), Err(error) if error.kind() == ErrorKind::Unsupported)
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn bounded_concrete_domains_materialize_before_canonical_identity()
-> Result<(), Box<dyn std::error::Error>> {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let names = Interner::new();
        let bit = Value::set_canonical(vec![Value::Int(0), Value::Int(1)], ctx)?;
        let one = Value::set_canonical(vec![Value::Int(1)], ctx)?;
        let set = |values| Value::set_canonical(values, ctx);
        let powerset = set(vec![
            set(vec![])?,
            set(vec![Value::Int(1)])?,
            set(vec![Value::Int(2)])?,
            set(vec![Value::Int(1), Value::Int(2)])?,
        ])?;
        let functions = set(vec![
            Value::tuple(vec![Value::Int(0), Value::Int(0)]),
            Value::tuple(vec![Value::Int(0), Value::Int(1)]),
            Value::tuple(vec![Value::Int(1), Value::Int(0)]),
            Value::tuple(vec![Value::Int(1), Value::Int(1)]),
        ])?;
        let product = set(vec![
            Value::tuple(vec![Value::Int(0), Value::Int(1)]),
            Value::tuple(vec![Value::Int(1), Value::Int(1)]),
        ])?;
        let records = set(vec![
            Value::record(vec![(names.intern("a"), Value::Int(0))], ctx)?,
            Value::record(vec![(names.intern("a"), Value::Int(1))], ctx)?,
        ])?;
        for (symbolic, explicit) in [
            (
                Value::subset(Value::interval(1.into(), 2.into(), ctx), ctx),
                powerset,
            ),
            (
                Value::fcn_set(Value::interval(1.into(), 2.into(), ctx), bit.clone(), ctx),
                functions,
            ),
            (Value::product(vec![bit.clone(), one.clone()], ctx), product),
            (
                Value::record_set(vec![(names.intern("a"), bit)], ctx)?,
                records,
            ),
        ] {
            let Value::Set(outer) = set(vec![symbolic, explicit])? else {
                return Err("canonical set expected".into());
            };
            assert_eq!(outer.size(ctx)?, 1, "equivalent finite domains deduplicate");
        }
        for huge in [
            Value::subset(Value::interval(1.into(), 100.into(), ctx), ctx),
            Value::fcn_set(
                Value::interval(1.into(), 1_000_000_000.into(), ctx),
                one,
                ctx,
            ),
        ] {
            assert!(
                matches!(set(vec![huge]), Err(error) if error.kind() == ErrorKind::Unsupported)
            );
        }
    }
    Ok(())
}
