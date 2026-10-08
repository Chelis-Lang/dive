//! Representation guarantees from ADR 0011.

use dive_value::{Atom, Ctx, Fcn, Interner, Set, Value};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn values_are_compact_and_thread_safe() {
    assert_eq!(std::mem::size_of::<Value>(), 16);
    assert_eq!(std::mem::size_of::<Option<Value>>(), 16);
    assert_eq!(std::mem::size_of::<Atom>(), 8);
    assert_eq!(std::mem::size_of::<Set>(), 8);
    assert_eq!(std::mem::size_of::<Fcn>(), 8);
    assert_send_sync::<Value>();
    assert_send_sync::<Interner>();
}

#[test]
fn clones_share_structure() {
    let ctx = Ctx::tlc();
    let s = Value::set((0..100).map(Value::Int).collect(), ctx).unwrap_or(Value::FALSE);
    let t = s.clone();
    match (&s, &t) {
        (Value::Set(a), Value::Set(b)) => assert!(a.ptr_eq(b)),
        _ => unreachable!("built a set"),
    }
}
