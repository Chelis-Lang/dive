//! Graph values preserve evaluation classes; report formatting stays separate.
#![allow(clippy::expect_used, reason = "bounded conversion fixtures")]

use std::sync::Arc;

use dive_value::{Ctx, Interner, Value, tlc};

use super::{Conversion, to_value};
use crate::{Handle, Heap, Store};

#[test]
fn graph_conversion_preserves_function_classes_recursively() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let mut heap = Heap::new(Arc::new(Store::default()), true, false);
        let names = Interner::new();
        let key = heap.string("a").expect("string");
        let zero = Handle::int(0).expect("integer");
        let general = heap.fcn_sorted(&[key], &[zero]);
        let record = heap.record(&[key], &[zero]);
        let tuple_fcn = heap.tuple(&[zero]);
        let tuple = heap.tuple_literal(&[zero]);
        let rhs = Value::tuple(vec![Value::Int(0), Value::Int(0)]);
        for (handle, literal) in [(general, false), (record, true)] {
            let converted = to_value(&mut heap, &names, handle, ctx, Conversion::Graph)
                .expect("graph conversion");
            assert_eq!(
                converted.as_fcn().expect("function").is_record_literal(),
                literal
            );
            let equal = tlc::equals(&converted, &rhs, ctx);
            if literal {
                assert!(
                    equal.is_err(),
                    "a literal record is incomparable with a tuple"
                );
            } else {
                assert!(!equal.expect("general function comparison"));
            }
            // The same distinction must survive nested conversion.
            let nested = heap.tuple_literal(&[handle]);
            let nested = to_value(&mut heap, &names, nested, ctx, Conversion::Graph)
                .expect("nested graph conversion");
            let equal = tlc::equals(&nested, &Value::tuple(vec![rhs.clone()]), ctx);
            assert_eq!(equal.is_err(), literal);
            if !literal {
                assert!(!equal.expect("nested general function comparison"));
            }
        }
        for (handle, literal) in [(tuple_fcn, false), (tuple, true)] {
            let nested = heap.record(&[key], &[handle]);
            let converted = to_value(&mut heap, &names, nested, ctx, Conversion::Graph)
                .expect("nested tuple conversion");
            let child = converted
                .as_fcn()
                .expect("record")
                .iter()
                .next()
                .expect("field")
                .1;
            assert_eq!(child.as_fcn().expect("tuple").is_tuple_literal(), literal);
        }
        let report = to_value(&mut heap, &names, general, ctx, Conversion::Report)
            .expect("existing report conversion");
        assert!(report.as_fcn().expect("record").is_record_literal());
        assert!(
            tlc::equals(&report, &rhs, ctx).is_err(),
            "report syntax is not an evaluation conversion"
        );
    }
}

#[test]
fn graph_conversion_refuses_unmaterialized_symbolic_identity() {
    let mut heap = Heap::new(Arc::new(Store::default()), true, false);
    let names = Interner::new();
    let one = Handle::int(1).expect("integer");
    let two = Handle::int(2).expect("integer");
    let raw = heap.set_enum(&[two, one, two]);
    let symbolic = heap.interval(1, 2);
    for handle in [raw, symbolic] {
        assert!(
            to_value(&mut heap, &names, handle, Ctx::tlc(), Conversion::Graph)
                .expect_err("symbolic identity is not materialized")
                .contains("unmaterialized symbolic")
        );
        assert!(to_value(&mut heap, &names, handle, Ctx::tlc(), Conversion::Report).is_ok());
    }
}

#[test]
fn imported_functions_keep_available_literal_classes_recursively() {
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        for hashcons in [false, true] {
            let mut heap = Heap::new(Arc::new(Store::new(hashcons)), true, false);
            let names = Interner::new();
            for literal in [false, true] {
                let field = names.intern("a");
                let mut source = if literal {
                    Value::record(vec![(field, Value::Int(0))], ctx).expect("record")
                } else {
                    Value::fcn(vec![(Value::Str(field), Value::Int(0))], ctx)
                        .expect("general function")
                };
                let mut rhs = Value::tuple(vec![Value::Int(0), Value::Int(0)]);
                for _ in 0..3 {
                    let handle =
                        crate::convert::from_value(&mut heap, &source, ctx).expect("import");
                    heap.reset();
                    let converted = to_value(&mut heap, &names, handle, ctx, Conversion::Graph)
                        .expect("graph value");
                    let compared = tlc::equals(&converted, &rhs, ctx);
                    assert_eq!(compared.is_err(), literal);
                    if !literal {
                        assert!(!compared.expect("general function differs"));
                    }
                    source = Value::tuple_fcn(vec![source]);
                    rhs = Value::tuple(vec![rhs]);
                }
            }
            for source in [
                Value::tuple(vec![Value::Int(0)]),
                Value::tuple_fcn(vec![Value::Int(0)]),
            ] {
                let literal = source.as_fcn().expect("function").is_tuple_literal();
                let handle = crate::convert::from_value(&mut heap, &source, ctx)
                    .expect("import tuple class");
                heap.reset();
                let converted = to_value(&mut heap, &names, handle, ctx, Conversion::Graph)
                    .expect("converted tuple class");
                assert_eq!(
                    converted.as_fcn().expect("function").is_tuple_literal(),
                    literal
                );
            }
        }
    }
}

/// A small deterministic generator (xorshift64*) for the order property.
struct Gen(u64);

impl Gen {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) % n
    }
}

const NAMES: [&str; 6] = ["zeta", "a", "\u{e9}t\u{e9}", "Ab", "b", "mm"];

fn generated(heap: &mut Heap, g: &mut Gen, depth: u8) -> Handle {
    let leaf = |heap: &mut Heap, g: &mut Gen| -> Handle {
        #[allow(clippy::cast_possible_truncation, reason = "below the name count")]
        let name = NAMES[g.below(NAMES.len() as u64) as usize];
        match g.below(4) {
            0 => Handle::bool(g.below(2) == 1),
            #[allow(clippy::cast_possible_wrap, reason = "small range")]
            1 => Handle::int(g.below(7) as i64 - 3).expect("integer"),
            2 => heap.string(name).expect("string"),
            _ => Handle::model(heap.store().strings().intern(name).expect("name")),
        }
    };
    if depth == 0 || g.below(3) == 0 {
        return leaf(heap, g);
    }
    #[allow(clippy::cast_possible_truncation, reason = "below 4")]
    let len = g.below(4) as usize;
    let mut items: Vec<Handle> = (0..len).map(|_| generated(heap, g, depth - 1)).collect();
    match g.below(5) {
        0 => heap.set_from(&mut items),
        1 => heap.tuple_literal(&items),
        2 => {
            let mut keys = Vec::new();
            for field in NAMES.iter().take(len) {
                keys.push(heap.string(field).expect("field"));
            }
            keys.sort_by(|&a, &b| heap.cmp(a, b));
            heap.record(&keys, &items)
        }
        3 => {
            #[allow(clippy::cast_possible_wrap, reason = "small range")]
            let lo = g.below(3) as i64;
            heap.int_fcn(lo, &items)
        }
        _ => {
            let mut keys: Vec<Handle> = (0..len).map(|_| generated(heap, g, depth - 1)).collect();
            keys.sort_by(|&a, &b| heap.cmp(a, b));
            keys.dedup_by(|a, b| heap.eq(*a, *b));
            let values = items[..keys.len()].to_vec();
            heap.fcn_sorted(&keys, &values)
        }
    }
}

/// Native liveness atoms evaluate Core values in `Heap::cmp` order; the
/// reference evaluator sees their graph conversions in `Value::cmp_with`
/// order. Set iteration, CHOOSE and enumeration agree only if both orders do.
#[test]
fn heap_order_agrees_with_reference_order_after_graph_conversion() {
    use crate::strings::StringOrder;
    use std::cmp::Ordering;
    for (order, ctx) in [
        (StringOrder::Text, Ctx::dive()),
        (StringOrder::Token, Ctx::tlc()),
    ] {
        let store = Arc::new(Store::with_string_order(true, order));
        let names = Interner::new();
        // TLC's token order is the shared encounter order of both tables.
        for name in NAMES {
            store.strings().intern(name).expect("name");
            names.intern(name);
        }
        let mut heap = Heap::new(store, true, false);
        let mut g = Gen(0x9e37_79b9_7f4a_7c15 ^ u64::from(order == StringOrder::Token));
        let mut values = Vec::new();
        for _ in 0..1200 {
            let handle = generated(&mut heap, &mut g, 3);
            let handle = heap.intern_exact(handle).expect("persistent");
            // Liveness admission converts every retained row first and
            // refuses the run when the reference cannot represent it (for
            // example a set TLC cannot normalize), so only converted values
            // can reach a native atom.
            if let Ok(value) = to_value(&mut heap, &names, handle, ctx, Conversion::Graph) {
                values.push((handle, value));
            }
        }
        let mut sets = 0;
        let mut models = 0;
        let mut functions = 0;
        for (a, va) in &values {
            functions += usize::from(heap.kind(*a).is_some_and(crate::node::Kind::is_fcn));
            sets += usize::from(heap.kind(*a) == Some(crate::node::Kind::Set));
            models += usize::from(a.as_model_id().is_some());
            for (b, vb) in &values {
                let core = heap.cmp(*a, *b);
                let reference = va.cmp_with(vb, ctx.order);
                assert_eq!(
                    core, reference,
                    "{order:?}: core {a:?} vs {b:?} orders {core:?}; reference {va:?} vs {vb:?} orders {reference:?}"
                );
                assert_eq!(core == Ordering::Equal, va == vb);
            }
        }
        assert!(
            values.len() > 300 && sets > 30 && models > 30 && functions > 100,
            "the generator covers sets, functions and model values: {} values, {sets} sets, {functions} functions, {models} models",
            values.len()
        );
        assert!(!heap.poisoned());
    }
}
