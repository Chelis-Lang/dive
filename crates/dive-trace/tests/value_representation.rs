//! Stored trace transport preserves representation and refuses forged claims.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test controls require exact admitted values"
)]
use dive_trace::value_repr;
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, Interner, Value, codec};
use serde_json::json;

#[test]
fn preserves_nonliteral_functions_and_explicit_set_representation() {
    let names = Interner::new();
    names.intern("a");
    names.intern("b");
    for ctx in [Ctx::tlc(), Ctx::dive()] {
        let record = Value::record(
            vec![
                (names.intern("a"), Value::Int(1)),
                (names.intern("b"), Value::Int(1)),
            ],
            ctx,
        )
        .unwrap();
        let tuple = Value::tuple(vec![Value::Int(1)]);
        let raw = Value::set_raw(vec![record.clone(), tuple.clone()], ctx);
        let Value::Set(s) = &raw else { unreachable!() };
        s.elements(ctx).unwrap();
        let values = [
            raw,
            Value::set(vec![record, tuple], ctx).unwrap(),
            Value::tuple_fcn(vec![Value::Int(5)]),
            Value::fcn(vec![(Value::string(&names, "a"), Value::Int(1))], ctx).unwrap(),
        ];
        for original in &values {
            let doc = value_repr::to_json(original, ctx).unwrap();
            let back = value_repr::from_json(&doc, &names, ctx).unwrap();
            assert_eq!(value_repr::to_json(&back, ctx).unwrap(), doc);
            let mut a = Vec::new();
            let mut b = Vec::new();
            codec::encode(original, ctx, &mut a).unwrap();
            codec::encode(&back, ctx, &mut b).unwrap();
            assert_eq!(a, b);
            if let (Value::Fcn(a), Value::Fcn(b)) = (original, &back) {
                assert_eq!(a.is_tuple_literal(), b.is_tuple_literal());
                assert_eq!(a.is_record_literal(), b.is_record_literal());
            }
            assert_eq!(
                TlcFingerprint::new(0)
                    .unwrap()
                    .fingerprint(original, ctx)
                    .unwrap(),
                TlcFingerprint::new(0)
                    .unwrap()
                    .fingerprint(&back, ctx)
                    .unwrap()
            );
        }
    }
}
#[test]
fn refuses_forged_order_domain_flags_and_witnesses() {
    let names = Interner::new();
    names.intern("a");
    names.intern("b");
    for ctx in [Ctx::tlc(), Ctx::dive()] {
        let value = Value::set(vec![Value::Int(1), Value::Int(2)], ctx).unwrap();
        let j = value_repr::to_json(&value, ctx).unwrap();
        for bad in [json!([0, 0]), json!([0, 2]), json!([0]), json!([true, 1])] {
            let mut forged = j.clone();
            forged["witness"] = bad;
            assert!(value_repr::from_json(&forged, &names, ctx).is_err());
        }
        let mut reversed = j.clone();
        reversed["items"] = json!([2, 1]);
        assert!(value_repr::from_json(&reversed, &names, ctx).is_err());
        let mut order = j.clone();
        order["order"] = json!(if ctx.order == dive_value::ValueOrder::Tlc {
            "canonical"
        } else {
            "tlc"
        });
        assert!(value_repr::from_json(&order, &names, ctx).is_err());
        let mut literal = value_repr::to_json(&Value::tuple(vec![Value::Int(1)]), ctx).unwrap();
        literal["literal"] = json!(true);
        assert!(value_repr::from_json(&literal, &names, ctx).is_err());
        let mut duplicate = value_repr::to_json(
            &Value::record(vec![(names.intern("a"), Value::TRUE)], ctx).unwrap(),
            ctx,
        )
        .unwrap();
        duplicate["items"] = json!([["a", true], ["a", false]]);
        assert!(value_repr::from_json(&duplicate, &names, ctx).is_err());
        assert!(value_repr::from_json(&json!("unknown"), &names, ctx).is_err());
    }
}
#[test]
fn bounded_admission_and_witness_probes_do_not_change_operands() {
    let names = Interner::new();
    let ctx = Ctx::tlc();
    let nested = Value::set_raw(vec![Value::Int(2), Value::Int(1)], ctx);
    let value = Value::set_raw(vec![nested.clone()], ctx);
    let before = value_repr::to_json(&value, ctx).unwrap();
    assert_eq!(value_repr::to_json(&value, ctx).unwrap(), before);
    assert!(
        value_repr::from_json(
            &json!({"repr":"tuple","items":vec![0;value_repr::MAX_MEMBERS+1]}),
            &names,
            ctx
        )
        .is_err()
    );
    let mut deep = json!(1);
    for _ in 0..=value_repr::MAX_DEPTH {
        deep = json!({"repr":"tuple","items":[deep]});
    }
    assert!(value_repr::from_json(&deep, &names, ctx).is_err());
    assert!(value_repr::to_json(&Value::nat(ctx), ctx).is_err());
}

#[derive(Debug)]
struct Provider(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl dive_value::DeferredSet for Provider {
    fn kind(&self) -> dive_value::DeferredKind {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(vec![Value::TRUE])
    }
    fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(Ok(true))
    }
}
#[test]
fn providers_remain_refused_without_additional_calls() {
    for ctx in [Ctx::tlc(), Ctx::dive()] {
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let lazy = Value::deferred(std::sync::Arc::new(Provider(calls.clone())), ctx);
        for v in [lazy.clone(), Value::set_raw(vec![lazy.clone()], ctx)] {
            assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
            assert!(value_repr::to_json(&v, ctx).is_err());
            assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
        }
        // Compound constructors have their own existing child hashing behavior.
        // Transport must add exactly zero calls beyond that captured baseline.
        let tuple = Value::tuple(vec![lazy]);
        let before = calls.load(std::sync::atomic::Ordering::Relaxed);
        assert!(value_repr::to_json(&tuple, ctx).is_err());
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), before);
    }
}

#[test]
fn stored_raw_normalization_is_recomputed_not_trusted() {
    let names = Interner::new();
    names.intern("a");
    names.intern("b");
    let ctx = Ctx::tlc();
    let raw = Value::set_raw(
        vec![
            Value::tuple(vec![Value::Int(1)]),
            Value::record(
                vec![
                    (names.intern("a"), Value::Int(1)),
                    (names.intern("b"), Value::Int(1)),
                ],
                ctx,
            )
            .expect("record"),
        ],
        ctx,
    );
    let mut j = value_repr::to_json(&raw, ctx).expect("stored raw");
    assert!(value_repr::from_json(&j, &names, ctx).is_ok());
    j["normalized"] = json!(true);
    assert!(value_repr::from_json(&j, &names, ctx).is_err());
}

#[test]
fn nested_constructor_probe_keeps_existing_normalization_markers() {
    for ctx in [Ctx::tlc(), Ctx::dive()] {
        let raw = Value::set_raw(vec![Value::Int(2), Value::Int(1)], ctx);
        let set = Value::set(vec![raw.clone()], ctx).expect("one nested set");
        let Value::Set(s) = &raw else { unreachable!() };
        let normalized_before = s.is_tlc_normalized();
        let doc = value_repr::to_json(&set, ctx).expect("nested stored set");
        assert_eq!(s.is_tlc_normalized(), normalized_before);
        let names = Interner::new();
        let restored = value_repr::from_json(&doc, &names, ctx).expect("nested decode");
        assert_eq!(value_repr::to_json(&restored, ctx).expect("again"), doc);
    }
}
#[test]
fn literal_flags_are_representation_not_validation_authority() {
    let source = r#"{"format":"dive-trace","version":2,"variables":["n"],"states":[{"values":{"n":0},"action":null}],"ending":{"kind":"behavior"}}"#;
    let j: serde_json::Value = serde_json::from_str(source).expect("JSON");
    let mut forged = j.clone();
    forged["validated"] = json!(true);
    assert!(dive_trace::TraceDoc::from_json(&forged).is_err());
    let doc = dive_trace::TraceDoc::from_json(&j).expect("valid shape");
    assert_eq!(doc.to_json()["version"], 2);
    let mut malformed = doc.clone();
    malformed.labels.clear();
    assert!(malformed.admit_transport().is_err());
}

#[test]
fn canonical_large_integer_and_decimal_limits_are_explicit() {
    let ctx = Ctx::dive();
    let names = Interner::new();
    let big = Value::int(
        dive_value::Int::from_literal("18446744073709551616", ctx.int_mode).expect("large integer"),
    );
    let j = value_repr::to_json(&big, ctx).expect("large writer");
    assert_eq!(
        value_repr::to_json(
            &value_repr::from_json(&j, &names, ctx).expect("large reader"),
            ctx
        )
        .expect("again"),
        j
    );
    assert!(value_repr::from_json(&j, &names, Ctx::tlc()).is_err());
    assert!(value_repr::to_json(&Value::BigInt(std::sync::Arc::new(1.into())), ctx).is_err());
    assert!(value_repr::from_json(&json!({"int":"1"}), &names, ctx).is_err());
    assert!(
        value_repr::from_json(
            &json!({"int":"9".repeat(value_repr::MAX_INTEGER_DIGITS + 1)}),
            &names,
            ctx
        )
        .is_err()
    );
}

#[test]
fn normalization_policy_budget_is_shared_by_writer_and_reader() {
    let names = Interner::new();
    let ctx = Ctx::tlc();
    let value = Value::set_raw(
        vec![Value::tuple(vec![Value::TRUE; 4]); value_repr::MAX_MEMBERS],
        ctx,
    );
    assert!(value_repr::to_json(&value, ctx).is_err());
    let json = json!({"repr":"raw-set","order":"tlc","normalized":false,"items":vec![json!({"repr":"tuple","items":[true,true,true,true]});value_repr::MAX_MEMBERS]});
    assert!(value_repr::from_json(&json, &names, ctx).is_err());
}

#[test]
fn encoder_refuses_trusted_noncanonical_function_and_raw_flags() {
    let names = Interner::new();
    names.intern("a");
    for ctx in [Ctx::tlc(), Ctx::dive()] {
        let bit = u8::from(ctx.order == dive_value::ValueOrder::Tlc);
        for bytes in [
            vec![0x2a | bit, 1, 2, 2, 2, 2],
            vec![0x2a | bit, 1, 4, 0, 2, 2],
            vec![0x28 | bit, 0],
        ] {
            let value = codec::decode(&mut &bytes[..], &mut |i| names.resolve(i))
                .expect("trusted storage decoder admits raw tag");
            assert!(
                value_repr::to_json(&value, ctx).is_err(),
                "noncanonical trusted function"
            );
        }
        let record = Value::record(
            vec![
                (names.intern("a"), Value::Int(1)),
                (names.intern("b"), Value::Int(1)),
            ],
            ctx,
        )
        .expect("record");
        let raw = Value::set_raw(vec![Value::tuple(vec![Value::Int(1)]), record], ctx);
        let mut bytes = Vec::new();
        codec::encode(&raw, ctx, &mut bytes).expect("trusted raw encoding");
        bytes[1] = 1; // Forge old trusted-storage normalization, not a transport witness.
        let value =
            codec::decode(&mut &bytes[..], &mut |i| names.resolve(i)).expect("trusted flag");
        assert!(value_repr::to_json(&value, ctx).is_err());
        for value in [
            Value::tuple(vec![Value::TRUE]),
            Value::tuple_fcn(vec![Value::FALSE]),
            Value::record(vec![(names.intern("a"), Value::Int(1))], ctx).expect("record"),
            Value::fcn(vec![(Value::Int(2), Value::Int(3))], ctx).expect("general"),
            Value::fcn(vec![(Value::string(&names, "a"), Value::Int(1))], ctx)
                .expect("record function"),
        ] {
            let j = value_repr::to_json(&value, ctx).expect("ordinary constructor shape");
            let decoded = value_repr::from_json(&j, &names, ctx)
                .expect("same-table/profile structural admission");
            assert_eq!(value_repr::to_json(&decoded, ctx).expect("roundtrip"), j);
        }
    }
}
