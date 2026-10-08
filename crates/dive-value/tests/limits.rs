//! Resource limits: deep nesting never aborts the process, and huge lazy sets are
//! compared, hashed and fingerprinted without being materialized.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed expectation is a test failure"
)]

use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, ErrorKind, Int, MAX_DEPTH, Value, json, print, tlc};

fn nested(depth: usize) -> Value {
    let mut v = Value::tuple(Vec::new());
    for _ in 0..depth {
        v = Value::tuple(vec![v]);
    }
    v
}

/// Runs `f` on a 2 MiB stack, as a worker thread might have.
fn small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("no panic")
}

#[test]
fn recursive_operations_refuse_deep_values_with_an_error() {
    small_stack(|| {
        let ctx = Ctx::tlc();
        let a = nested(200_000);
        let b = nested(200_000);
        let limit = |r: Result<String, dive_value::ValueError>| {
            r.expect_err("too deep").kind() == ErrorKind::DiveLimit
        };
        assert!(limit(print::to_tlc_string(&a, ctx)));
        assert!(limit(json::to_tlc_json(&a, ctx)));
        assert!(limit(tlc::equals(&a, &b, ctx).map(|x| x.to_string())));
        assert!(limit(tlc::compare(&a, &b, ctx).map(|x| format!("{x:?}"))));
        // The iterative operations still work.
        assert_eq!(a, b);
        assert!(
            TlcFingerprint::new(0)
                .expect("poly")
                .fingerprint(&a, ctx)
                .is_ok()
        );
        let _ = format!("{a:?}");
    });
}

#[test]
fn recursive_operations_handle_the_documented_depth() {
    small_stack(|| {
        let ctx = Ctx::tlc();
        let depth = usize::try_from(MAX_DEPTH).expect("u32") - 10;
        let a = nested(depth);
        let b = nested(depth);
        assert!(print::to_tlc_string(&a, ctx).is_ok());
        assert!(json::to_tlc_json(&a, ctx).is_ok());
        assert!(tlc::equals(&a, &b, ctx).expect("equal"));
        assert!(tlc::compare(&a, &b, ctx).is_ok());
        assert!(print::print_value(&a, ctx).is_ok());
    });
}

#[test]
fn huge_intervals_are_compared_without_materializing() {
    let ctx = Ctx::tlc();
    let a = Value::interval(Int::ZERO, Int::from(2_147_483_646i64), ctx);
    let b = Value::interval(Int::ZERO, Int::from(2_147_483_646i64), ctx);
    let t = std::time::Instant::now();
    assert_eq!(a, b);
    assert!(tlc::equals(&a, &b, ctx).expect("equals"));
    assert_eq!(a.cmp_with(&b, ctx.order), std::cmp::Ordering::Equal);
    let small = Value::interval(Int::from(5i64), Int::from(6i64), ctx);
    let s = Value::set(vec![a.clone(), small], ctx).expect("set");
    assert_eq!(
        print::to_tlc_string(&s, ctx).expect("print"),
        "{5..6, 0..2147483646}"
    );
    assert!(t.elapsed().as_millis() < 500, "took {:?}", t.elapsed());
}

#[test]
fn large_lazy_sets_are_streamed_for_hash_and_fingerprint() {
    let ctx = Ctx::tlc();
    // 2^20 subsets: streamed, never collected into one Vec.
    let base = Value::interval(Int::ONE, Int::from(20i64), ctx);
    let s = Value::subset(base.clone(), ctx);
    let t = Value::subset(base, ctx);
    assert_eq!(s.hash64(), t.hash64());
    let fp = TlcFingerprint::new(0).expect("poly");
    assert_eq!(
        fp.fingerprint(&s, ctx).expect("fp"),
        fp.fingerprint(&t, ctx).expect("fp")
    );
    // SUBSET of 40 elements is far too large to stream: compared by definition.
    let big = |n: i64| Value::subset(Value::interval(Int::ONE, Int::from(n), ctx), ctx);
    assert_eq!(big(40), big(40));
    assert_ne!(big(40), big(41));
    assert_ne!(
        big(40).cmp_with(&big(41), ctx.order),
        std::cmp::Ordering::Equal
    );
}
