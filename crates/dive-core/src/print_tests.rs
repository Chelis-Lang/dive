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
fn graph_conversion_does_not_infer_missing_set_history() {
    let mut heap = Heap::new(Arc::new(Store::default()), true, false);
    let names = Interner::new();
    let one = Handle::int(1).expect("integer");
    let two = Handle::int(2).expect("integer");
    let raw = heap.set_enum(&[two, one, two]);
    let symbolic = heap.interval(1, 2);
    for handle in [raw, symbolic] {
        assert!(
            to_value(&mut heap, &names, handle, Ctx::tlc(), Conversion::Graph)
                .expect_err("history is not certified")
                .contains("normalization history")
        );
        assert!(to_value(&mut heap, &names, handle, Ctx::tlc(), Conversion::Report).is_ok());
    }
}
