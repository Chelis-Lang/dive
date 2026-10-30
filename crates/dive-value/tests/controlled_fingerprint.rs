//! Replay's materialized fingerprint demand retains exact bits and stops.
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{ConstructionError, Ctx, Interner, Value};
type TestResult = Result<(), Box<dyn std::error::Error>>;
#[test]
fn controlled_materialized_fingerprints_match_and_never_force_unproved_sets() -> TestResult {
    let fingerprint = TlcFingerprint::new(0).ok_or("polynomial")?;
    for ctx in [Ctx::dive(), Ctx::tlc()] {
        let names = Interner::new();
        let cases = [
            Value::Int(1),
            Value::Bool(true),
            Value::tuple(vec![Value::Int(2), Value::Int(3)]),
            Value::set(vec![Value::Int(2), Value::Int(1)], ctx)?,
            Value::record(vec![(names.intern("a"), Value::Int(3))], ctx)?,
            Value::fcn(vec![(Value::Int(4), Value::Int(5))], ctx)?,
        ];
        for value in cases {
            assert_eq!(
                fingerprint.fingerprint_materialized_controlled(&value, ctx, &mut || true)?,
                fingerprint.fingerprint(&value, ctx)?
            );
        }
        for value in [
            Value::set_raw(vec![Value::Int(2), Value::Int(1)], ctx),
            Value::interval(1.into(), 1_000_000_000.into(), ctx),
        ] {
            let set = value.as_set().ok_or("set")?;
            let cached = set.has_cached_elements();
            let normalized = set.is_tlc_normalized();
            assert!(matches!(
                fingerprint.fingerprint_materialized_controlled(&value, ctx, &mut || true),
                Err(ConstructionError::Value(_))
            ));
            assert_eq!(set.has_cached_elements(), cached);
            assert_eq!(set.is_tlc_normalized(), normalized);
        }
        let large = Value::tuple(vec![Value::Int(0); 8192]);
        let mut calls = 0;
        assert!(matches!(
            fingerprint.fingerprint_materialized_controlled(&large, ctx, &mut || {
                calls += 1;
                calls < 100
            }),
            Err(ConstructionError::Interrupted)
        ));
        assert_eq!(
            fingerprint.fingerprint_materialized_controlled(&large, ctx, &mut || true)?,
            fingerprint.fingerprint(&large, ctx)?
        );
    }
    Ok(())
}
