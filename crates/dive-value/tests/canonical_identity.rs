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

#[test]
fn nested_raw_storage_cannot_normalize_an_oversized_grandchild()
-> Result<(), Box<dyn std::error::Error>> {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let one = Value::set_canonical(vec![Value::Int(1)], ctx)?;
        let huge = Value::fcn_set(Value::interval(1.into(), 20_000_000.into(), ctx), one, ctx);
        // Sizing this raw duplicate set used to normalize the function before
        // preflight reached its oversized domain. The outer powerset is tiny.
        let nested = Value::subset(Value::set_raw(vec![huge.clone(), huge], ctx), ctx);
        assert!(
            matches!(Value::set_canonical(vec![nested], ctx), Err(error) if error.kind() == ErrorKind::Unsupported)
        );
    }
    Ok(())
}

#[test]
fn finite_raw_cross_order_children_rebuild_keys_and_record_fields_exactly()
-> Result<(), Box<dyn std::error::Error>> {
    for (source, target) in [(Ctx::tlc(), Ctx::dive()), (Ctx::dive(), Ctx::tlc())] {
        let names = Interner::new();
        let z = names.intern("z");
        let a = names.intern("a");
        let raw = Value::set_raw(
            vec![
                Value::Str(z.clone()),
                Value::Str(a.clone()),
                Value::Str(z.clone()),
            ],
            source,
        );
        let record = Value::record(
            vec![(z.clone(), raw.clone()), (a.clone(), Value::Int(9))],
            source,
        )?;
        let function = Value::fcn(vec![(raw.clone(), record.clone())], source)?;
        let original = Value::tuple(vec![raw.clone(), record, function]);
        original.require_materialized(target)?;
        assert!(!raw.as_set().ok_or("raw set")?.is_tlc_normalized());

        let canonical =
            Value::set_canonical(vec![Value::Str(a.clone()), Value::Str(z.clone())], target)?;
        let expected_record =
            Value::record(vec![(a, Value::Int(9)), (z, canonical.clone())], target)?;
        let expected_function =
            Value::fcn(vec![(canonical.clone(), expected_record.clone())], target)?;
        let expected = Value::tuple(vec![canonical, expected_record, expected_function]);
        let rebuilt = original.materialized_identity(target)?;
        assert_eq!(rebuilt, expected);
        assert_eq!(rebuilt.hash64(), expected.hash64());
        let outer = Value::set_canonical(vec![original, expected], target)?;
        assert_eq!(outer.as_set().ok_or("outer set")?.size(target)?, 1);
        assert!(!raw.as_set().ok_or("raw set")?.is_tlc_normalized());
    }
    Ok(())
}

#[derive(Debug)]
struct UnprovedProvider(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl dive_value::DeferredSet for UnprovedProvider {
    fn kind(&self) -> dive_value::DeferredKind {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _ctx: Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(vec![Value::Int(1)])
    }
    fn member(&self, _value: &Value, _ctx: Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Vec::new()
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(self)
    }
}

#[test]
fn finite_storage_inspection_refuses_hidden_symbolic_children_without_callbacks() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let provider = Value::deferred(std::sync::Arc::new(UnprovedProvider(calls.clone())), ctx);
        for symbolic in [
            provider,
            Value::seq_set(Value::nat(ctx), ctx),
            Value::nat(ctx),
        ] {
            let value = Value::tuple(vec![Value::set_raw(vec![symbolic], ctx)]);
            let before = calls.load(std::sync::atomic::Ordering::Relaxed);
            assert!(
                matches!(value.require_materialized(ctx), Err(error) if error.kind()==ErrorKind::Unsupported)
            );
            assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), before);
        }
    }
}

#[test]
fn sets_of_tuples_are_sorted_and_distinct_in_any_input_order()
-> Result<(), Box<dyn std::error::Error>> {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let names = Interner::new();
        let (a, b) = (
            Value::Model(names.intern("a")),
            Value::Model(names.intern("b")),
        );
        let t = |xs: &[&Value]| Value::tuple(xs.iter().map(|x| (*x).clone()).collect());
        let ascending = vec![
            t(&[]),
            t(&[&a]),
            t(&[&b]),
            t(&[&a, &a]),
            t(&[&a, &b]),
            t(&[&b, &a]),
            t(&[&b, &b]),
        ];
        let expected = Value::set_canonical(ascending.clone(), ctx)?;
        let mut descending = ascending.clone();
        descending.reverse();
        let mut repeated = ascending.clone();
        repeated.extend(ascending.iter().cloned());
        for elements in [descending, repeated] {
            let built = Value::set_canonical(elements, ctx)?;
            assert_eq!(built, expected);
            assert_eq!(format!("{built:?}"), format!("{expected:?}"));
        }
        let Value::Set(set) = &expected else {
            return Err("not a set".into());
        };
        assert_eq!(set.elements(ctx)?.len(), 7);
        // A set of function sets materializes to the same elements.
        let s = Value::set_canonical(vec![a.clone(), b.clone()], ctx)?;
        let sets = Value::set_canonical(
            vec![
                Value::fcn_set(Value::interval(1.into(), 2.into(), ctx), s.clone(), ctx),
                Value::fcn_set(Value::interval(1.into(), 1.into(), ctx), s, ctx),
            ],
            ctx,
        )?;
        let explicit = Value::set_canonical(
            vec![
                Value::set_canonical(vec![t(&[&a]), t(&[&b])], ctx)?,
                Value::set_canonical(ascending[3..].to_vec(), ctx)?,
            ],
            ctx,
        )?;
        assert_eq!(sets, explicit);
    }
    Ok(())
}
