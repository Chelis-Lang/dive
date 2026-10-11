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

#[test]
fn nonenumerable_function_set_diagnostics_do_not_reenter_the_same_expansion() {
    small_stack(|| {
        for ctx in [Ctx::tlc(), Ctx::dive()] {
            let empty_domain = Value::fcn_set(Value::nat(ctx), Value::empty_set(ctx), ctx);
            let value = Value::fcn_set(
                empty_domain,
                Value::set(vec![Value::from(1)], ctx).expect("range"),
                ctx,
            );
            let expected = "{<<>>}";
            assert_eq!(
                value
                    .as_set()
                    .expect("set")
                    .elements(ctx)
                    .expect("empty domain"),
                std::sync::Arc::from([Value::tuple(Vec::new())])
            );
            assert_eq!(print::to_tlc_string_lenient(&value, ctx), expected);
            assert_eq!(
                print::to_tlc_string(&value, ctx).expect("strict print"),
                expected
            );
            let _ = format!("{value:?}");
            // A nonempty symbolic domain can trigger the same diagnostic cycle.
            let one = Value::set(vec![Value::from(1)], ctx).expect("range");
            let nonempty =
                Value::fcn_set(Value::fcn_set(Value::nat(ctx), one.clone(), ctx), one, ctx);
            assert_eq!(
                print::to_tlc_string_lenient(&nonempty, ctx),
                "[[Nat -> {1}] -> {1}]"
            );
            assert_eq!(
                nonempty
                    .as_set()
                    .expect("set")
                    .elements(ctx)
                    .expect_err("not enumerable")
                    .kind(),
                ErrorKind::NotEnumerable
            );
            assert_eq!(
                print::to_tlc_string_lenient(&Value::tuple(vec![Value::from(1)]), ctx),
                "<<1>>"
            );
        }
    });
}

#[test]
fn symbolic_function_emptiness_preserves_size_hash_equality_and_fingerprints() {
    for ctx in [Ctx::tlc(), Ctx::dive()] {
        let empty = Value::empty_set(ctx);
        let unit = Value::set(vec![Value::tuple(Vec::new())], ctx).expect("unit");
        let one = Value::set(vec![Value::from(1)], ctx).expect("one");
        let no_functions = Value::fcn_set(Value::nat(ctx), empty.clone(), ctx);
        let empty_domain = Value::fcn_set(no_functions.clone(), one.clone(), ctx);
        let infinite_range = Value::fcn_set(empty.clone(), Value::nat(ctx), ctx);
        for (value, expected, size) in [
            (&no_functions, &empty, 0),
            (&empty_domain, &unit, 1),
            (&infinite_range, &unit, 1),
        ] {
            assert_eq!(value.as_set().expect("set").size(ctx).expect("size"), size);
            assert!(!value.as_set().expect("set").is_infinite());
            assert_eq!(value, expected);
            assert_eq!(value.hash64(), expected.hash64());
            assert!(tlc::equals(value, expected, ctx).expect("equals"));
            let fp = TlcFingerprint::new(0).expect("poly");
            assert_eq!(
                fp.fingerprint(value, ctx).expect("fp"),
                fp.fingerprint(expected, ctx).expect("fp")
            );
        }
        // Cardinality one does not make an infinite-domain function enumerable.
        let symbolic = Value::fcn_set(Value::nat(ctx), one, ctx);
        assert_eq!(symbolic.as_set().expect("set").size(ctx).expect("size"), 1);
        assert!(!symbolic.as_set().expect("set").is_infinite());
        assert_eq!(
            symbolic
                .as_set()
                .expect("set")
                .elements(ctx)
                .expect_err("nonenumerable domain")
                .kind(),
            ErrorKind::NotEnumerable
        );
    }
}

#[derive(Debug)]
struct UnknownDomain;
impl dive_value::DeferredSet for UnknownDomain {
    fn kind(&self) -> dive_value::DeferredKind {
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _: Ctx) -> Result<Vec<Value>, dive_value::ValueError> {
        Err(dive_value::ValueError::new(
            ErrorKind::NotEnumerable,
            "unknown domain",
        ))
    }
    fn member(&self, _: &Value, _: Ctx) -> Option<Result<bool, dive_value::ValueError>> {
        None
    }
}

#[test]
fn unknown_domain_is_not_inferred_nonempty_from_failed_enumeration() {
    for ctx in [Ctx::tlc(), Ctx::dive()] {
        let opaque = Value::fcn_set(
            Value::deferred(std::sync::Arc::new(UnknownDomain), ctx),
            Value::empty_set(ctx),
            ctx,
        );
        assert_eq!(
            opaque
                .as_set()
                .expect("set")
                .size(ctx)
                .expect_err("unknown empty domain")
                .kind(),
            ErrorKind::NotEnumerable
        );
        assert!(opaque.as_set().expect("set").elements(ctx).is_err());
        assert!(print::to_tlc_string(&opaque, ctx).is_err());
        assert_ne!(opaque.hash64(), Value::empty_set(ctx).hash64());
        let invalid = Value::fcn_set(
            Value::TRUE,
            Value::set(vec![Value::from(1)], ctx).expect("range"),
            ctx,
        );
        assert!(invalid.as_set().expect("set").size(ctx).is_err());
        assert!(invalid.as_set().expect("set").elements(ctx).is_err());
    }
}
