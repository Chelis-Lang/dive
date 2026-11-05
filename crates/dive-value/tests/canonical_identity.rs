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
