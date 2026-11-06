//! Concrete union equivalence and guarded execution.

#![allow(clippy::unwrap_used, reason = "test construction and assertions")]

use super::*;

fn machine(tlc: bool, hashcons: bool) -> Machine {
    let semantics = if tlc {
        crate::Semantics::tlc()
    } else {
        crate::Semantics::default()
    };
    Machine::new(
        Arc::new(Store::with_semantics(hashcons, semantics)),
        1,
        Opts::default(),
        Memo::new(&[]),
    )
}

#[test]
fn canonical_union_matches_construction_across_all_compound_overlaps() {
    for tlc in [false, true] {
        for hashcons in [false, true] {
            let mut m = machine(tlc, hashcons);
            let mut elements = Vec::new();
            for n in 0..4 {
                let leaf = Handle::int(n).unwrap();
                let nested = m.heap.tuple_literal(&[leaf]);
                elements.push(m.heap.tuple_literal(&[nested, leaf]));
            }
            for left in 0u8..16 {
                for right in 0u8..16 {
                    let left_values = elements
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &e)| (left & (1 << i) != 0).then_some(e))
                        .collect::<Vec<_>>();
                    let right_values = elements
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &e)| (right & (1 << i) != 0).then_some(e))
                        .collect::<Vec<_>>();
                    let x = m.heap.set_sorted(&left_values);
                    let y = m.heap.set_sorted(&right_values);
                    let mut expected = left_values;
                    expected.extend(right_values);
                    let expected = m.heap.set_from(&mut expected);
                    let actual = m.set_bin(SetOp::Cup, x, y).unwrap();
                    assert_eq!(m.heap.view(actual).data, m.heap.view(expected).data);
                    assert_eq!(m.heap.hash(actual), m.heap.hash(expected));
                    assert!(m.heap.eq(actual, expected));
                    assert!(!m.heap.poisoned());
                }
            }
        }
    }
}

#[test]
fn canonical_union_retains_mixed_class_and_unknown_identity_guards() {
    for tlc in [false, true] {
        let mut m = machine(tlc, true);
        let zero = Handle::int(0).unwrap();
        let x = m.heap.set_sorted(&[zero]);
        let y = m.heap.set_sorted(&[Handle::TRUE]);
        let mixed = m.set_bin(SetOp::Cup, x, y);
        if tlc {
            assert_eq!(mixed.unwrap_err().kind(), crate::ErrorKind::Unsupported);
        } else {
            let mixed = mixed.unwrap();
            assert_eq!(m.member(zero, mixed), Ok(true));
            assert_eq!(m.member(Handle::TRUE, mixed), Ok(true));
        }
        let interval = m.heap.interval(0, 1);
        let unknown = m.heap.symbolic(Kind::SeqSet, 0, &[interval]);
        let unknown = m.heap.tuple_literal(&[unknown]);
        let y = m.heap.set_sorted(&[unknown]);
        let error = m.set_bin(SetOp::Cup, x, y).unwrap_err();
        assert_eq!(error.kind(), crate::ErrorKind::Unsupported);
        assert!(error.message().contains("mathematical identity"));
        assert!(!m.heap.poisoned());
    }
}

#[test]
fn canonical_union_polls_inside_a_merge() {
    #[derive(Debug)]
    struct Stop;
    impl WorkControl for Stop {
        fn proceed(&self) -> bool {
            false
        }
    }
    let mut m = machine(false, true);
    let values = (0..1024)
        .map(|n| Handle::int(n).unwrap())
        .collect::<Vec<_>>();
    let x = m.heap.set_sorted(&values);
    let y = m.heap.empty_set();
    m.set_work_control(Some(Arc::new(Stop)));
    let error = m.union_concrete(x, y).unwrap_err();
    assert_eq!(error.kind(), crate::ErrorKind::Stop);
    assert!(m.heap.work_interrupted());
}
