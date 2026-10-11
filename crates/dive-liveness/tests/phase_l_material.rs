//! Stored successor admission is callback-free and preserves existing session bounds.
#![allow(clippy::expect_used, clippy::panic, reason = "adversarial controls")]
use dive_liveness::phase_l::{Error, Limits, Session, State};
use dive_model::{BindOptions, Model};
use dive_value::{Ctx, Interner, Value};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
fn model(options: &BindOptions) -> Model {
    let analysis = dive_sema::analyze_source(
        "Stored.tla",
        "---- MODULE Stored ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=x)\n====\n",
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n")
        .into_config()
        .expect("config");
    dive_model::bind(ir, &cfg, options).expect("owned model")
}
fn state(value: Value) -> State {
    Arc::from([Some(value)])
}
#[test]
fn stored_enum_normalized_raw_and_nested_functions_admit_without_new_global_cap() {
    for (options, ctx) in [(BindOptions::tlc(), Ctx::tlc())] {
        let m = model(&options);
        let raw = Value::set_raw(vec![2.into(), 1.into()], ctx);
        let normalized = Value::set_raw(vec![2.into(), 1.into()], ctx);
        let Value::Set(set) = &normalized else {
            panic!("set")
        };
        set.elements(ctx).expect("explicit normalization");
        assert!(set.raw_elements().is_none());
        assert!(set.stored_elements().is_some());
        let enumerated = Value::set(vec![2.into(), 1.into()], ctx).expect("Enum");
        let values = vec![
            raw,
            normalized,
            enumerated.clone(),
            Value::tuple(vec![enumerated.clone()]),
            Value::record(vec![(m.names.intern("bag"), enumerated.clone())], ctx).expect("record"),
            Value::fcn(vec![(Value::string(&m.names, "alpha"), enumerated)], ctx)
                .expect("function"),
        ];
        for value in values {
            let mut session = Session::new(&m, 0, 1, Limits::default()).expect("session");
            session
                .initial(&state(value))
                .expect("stored material admits");
            assert!(session.progress().values > 0);
        }
        // Existing unnormalized Raw input is not silently narrowed to the
        // independent context/native admission's unrelated 50k bound.
        let large = Value::set_raw((0..50_001).map(Value::Int).collect(), ctx);
        let mut session = Session::new(
            &m,
            0,
            1,
            Limits {
                values: 50_002,
                ..Limits::default()
            },
        )
        .expect("session");
        let input = state(large);
        session.initial(&input).expect("original Raw bound remains");
        assert_eq!(session.progress().values, 50_002);
        assert!(matches!(
            session.initial(&input).expect_err("cumulative bound").error,
            Error::Limit("material value visits")
        ));
    }
}
#[derive(Debug)]
struct Provider {
    armed: Arc<AtomicBool>,
    calls: Arc<AtomicUsize>,
}
impl dive_value::DeferredSet for Provider {
    fn kind(&self) -> dive_value::DeferredKind {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(!self.armed.load(Ordering::SeqCst));
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(!self.armed.load(Ordering::SeqCst));
        Ok(vec![1.into()])
    }
    fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(!self.armed.load(Ordering::SeqCst));
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(!self.armed.load(Ordering::SeqCst));
        vec![]
    }
}
#[test]
fn cached_providers_foreign_atoms_and_wrong_storage_order_refuse_before_predicates() {
    for (options, ctx, wrong) in [(BindOptions::tlc(), Ctx::tlc(), Ctx::dive())] {
        let m = model(&options);
        let armed = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(AtomicUsize::new(0));
        let deferred = Value::deferred(
            Arc::new(Provider {
                armed: armed.clone(),
                calls: calls.clone(),
            }),
            ctx,
        );
        let Value::Set(set) = &deferred else {
            panic!("set")
        };
        set.elements(ctx).expect("materialized cache");
        assert!(set.stored_elements().is_none());
        let nested = Value::tuple(vec![deferred.clone()]);
        let baseline = calls.load(Ordering::SeqCst);
        armed.store(true, Ordering::SeqCst);
        for value in [deferred, nested] {
            let mut session = Session::new(&m, 0, 1, Limits::default()).expect("session");
            let failure = session
                .initial(&state(value))
                .expect_err("provider stays refused");
            assert!(matches!(failure.error, Error::Unsupported(_)));
            assert_eq!(failure.progress.predicates, 0);
            assert_eq!(calls.load(Ordering::SeqCst), baseline);
        }
        let foreign = Interner::new();
        let atom = foreign.intern("not-owned");
        for value in [
            Value::Str(atom.clone()),
            Value::Model(atom.clone()),
            Value::set_raw(vec![Value::Model(atom)], ctx),
            Value::set_raw(vec![1.into()], wrong),
            Value::record(vec![(m.names.intern("field"), 1.into())], wrong)
                .expect("wrong-order record"),
            Value::fcn(vec![(Value::string(&m.names, "domain"), 1.into())], wrong)
                .expect("wrong-order function"),
        ] {
            let mut session = Session::new(&m, 0, 1, Limits::default()).expect("session");
            let failure = session
                .initial(&state(value))
                .expect_err("namespace/order refused");
            assert!(matches!(failure.error, Error::Input(_)));
            assert_eq!(failure.progress.predicates, 0);
            assert_eq!(failure.progress.comparisons, 0);
        }
        let mut session = Session::new(&m, 0, 1, Limits::default()).expect("session");
        session
            .initial(&state(Value::tuple(vec![1.into()])))
            .expect("tuple order exemption");
    }
}
#[test]
fn original_child_visit_depth_and_error_precedence_are_retained() {
    let m = model(&BindOptions::tlc());
    let foreign = Interner::new();
    let value = Value::set_raw(vec![Value::string(&foreign, "not-owned")], Ctx::tlc());
    for (bound, limit_first) in [(1, true), (2, false)] {
        let mut session = Session::new(
            &m,
            0,
            1,
            Limits {
                values: bound,
                ..Limits::default()
            },
        )
        .expect("session");
        let failure = session
            .initial(&state(value.clone()))
            .expect_err("bounded/invalid");
        assert_eq!(failure.progress.values, bound);
        assert_eq!(failure.progress.predicates, 0);
        if limit_first {
            assert!(matches!(
                failure.error,
                Error::Limit("material value visits")
            ));
        } else {
            assert!(matches!(failure.error, Error::Input(_)));
        }
    }
    let mut value = Value::Int(0);
    for _ in 0..257 {
        value = Value::tuple(vec![value]);
    }
    let mut session = Session::new(&m, 0, 1, Limits::default()).expect("session");
    let failure = session
        .initial(&state(value))
        .expect_err("existing depth ceiling");
    assert!(matches!(
        failure.error,
        Error::Limit("material value depth")
    ));
    // Each admitted tuple also charges its stored integer domain key.
    assert_eq!(failure.progress.values, 513);
    assert_eq!(failure.progress.predicates, 0);
}

#[test]
fn default_profile_still_refuses_phase_l_before_any_material_inspection() {
    let m = model(&BindOptions::dive());
    let Err(failure) = Session::new(&m, 0, 1, Limits::default()) else {
        panic!("strict receipt profile");
    };
    assert!(matches!(failure.error, Error::Unsupported(_)));
    assert_eq!(failure.progress.values, 0);
    assert_eq!(failure.progress.predicates, 0);
}
