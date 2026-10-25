//! Actual lowered predicates and captures, not manually supplied truth masks.
#![allow(clippy::expect_used, reason = "bounded lowered context controls")]
use super::*;
use crate::{Host, HostError, Model, Options, St};
use ::std::sync::atomic::{AtomicUsize, Ordering};
use dive_ir::StdOp;
use dive_value::{Ctx, DeferredKind, Set};

fn set(value: &Value) -> &Set {
    match value {
        Value::Set(set) => Some(set),
        _ => None,
    }
    .expect("set fixture")
}
fn evaluator(extra: &str, expression: &str, options: Options) -> Evaluator {
    let source = format!(
        "---- MODULE MaskContext ----\nEXTENDS Integers,FiniteSets,TLC\nVARIABLE x\n{extra}\nF == {expression}\n====\n"
    );
    let analysis = dive_sema::analyze_source("MaskContext.tla", &source, &Default::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    Evaluator::new(
        Model::new(Arc::new(dive_ir::lower(&analysis).expect("IR"))),
        options,
    )
    .expect("evaluator")
}
fn fixture(
    extra: &str,
    expression: &str,
    options: Options,
) -> (Evaluator, Arc<dyn DeferredSet>, ConcreteSubsetStream) {
    let ev = evaluator(extra, expression, options);
    let def = ev.ir().root_def("F").expect("F");
    let value = ev
        .eval_def(def, &St::constant())
        .expect("original Filter construction");
    let owner = set(&value)
        .as_deferred()
        .expect("genuine private Filter")
        .clone();
    let filter = owner
        .as_any()
        .and_then(|f| f.downcast_ref::<Filter>())
        .expect("Filter");
    let stream = set(&filter.in_val)
        .concrete_subset_stream(options.ctx)
        .expect("component preparation")
        .expect("concrete subset");
    (ev, owner, stream)
}
fn filter(owner: &Arc<dyn DeferredSet>) -> &Filter {
    owner
        .as_any()
        .and_then(|f| f.downcast_ref())
        .expect("authentic Filter")
}
fn actual_singletons(owner: &Arc<dyn DeferredSet>, source: &mut ConcreteSubsetStream) -> Vec<bool> {
    let filter = filter(owner);
    let mut env = filter.env.clone();
    let empty = source.next().expect("original empty");
    assert!(
        filter
            .core
            .filter_keeps(filter, &mut env, &empty)
            .expect("vacuous predicate")
    );
    let mut values = Vec::new();
    for _ in 0..source.base_elements().len() {
        let value = source.next().expect("original singleton");
        assert_eq!(source.current_indices().expect("original index").len(), 1);
        values.push(
            filter
                .core
                .filter_keeps(filter, &mut env, &value)
                .expect("whole singleton predicate"),
        );
    }
    values
}
#[derive(Debug)]
struct NativeCount(Arc<AtomicUsize>);
impl Host for NativeCount {
    fn check_native_access(&self, _op: StdOp) -> std::result::Result<(), HostError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn closed_inline_resolved_and_tuple_predicates_inspect_without_evaluating_q() {
    for options in [Options::dive(), Options::tlc()] {
        for (extra, expression) in [
            (
                "",
                "{A \\in SUBSET (SUBSET (1..3)) : \\A e \\in A : Cardinality(e)=2}",
            ),
            (
                "Q(e) == Cardinality(e)=2",
                "{A \\in SUBSET (SUBSET (1..3)) : \\A e \\in A : Q(e)}",
            ),
            (
                "",
                "{A \\in SUBSET ((1..2) \\X (3..4)) : \\A <<a,b>> \\in A : a<b}",
            ),
        ] {
            let (ev, owner, mut source) = fixture(extra, expression, options);
            // Empty already ran at its original full-predicate site.
            let empty = source.next().expect("empty");
            let f = filter(&owner);
            assert!(
                ev.core
                    .filter_keeps(f, &mut f.env.clone(), &empty)
                    .expect("empty predicate")
            );
            let count = Arc::new(AtomicUsize::new(0));
            ev.set_host(Arc::new(NativeCount(count.clone())));
            let proof = ClosedForall::inspect(&ev.core, &owner, &source)
                .expect("read-only proof")
                .expect("closed context");
            assert!(proof.belongs_to(&owner, &source));
            assert_eq!(
                count.load(Ordering::Relaxed),
                0,
                "metadata does not evaluate Q"
            );
            let mut env = f.env.clone();
            for _ in 0..source.base_elements().len() {
                let value = source.next().expect("singleton");
                ev.core
                    .filter_keeps(f, &mut env, &value)
                    .expect("original whole predicate");
            }
            assert!(
                count.load(Ordering::Relaxed) > 0,
                "real native path is positive"
            );
            assert_eq!(ev.take_output(), Vec::<String>::new());
        }
    }
}

#[test]
fn diagonal_singleton_truth_cannot_prove_outer_dependence_or_two_dimensions() {
    for expression in [
        "{A \\in SUBSET {1,2} : \\A e \\in A : IF Cardinality(A)=1 THEN TRUE ELSE 1 \\div 0=0}",
        "{A \\in SUBSET {1,2} : \\A a,b \\in A : a=b}",
    ] {
        let (ev, owner, mut source) = fixture("", expression, Options::tlc());
        assert!(
            actual_singletons(&owner, &mut source)
                .into_iter()
                .all(|v| v)
        );
        assert!(
            ClosedForall::inspect(&ev.core, &owner, &source)
                .expect("source inspection")
                .is_none()
        );
        let pair = source.next().expect("first pair");
        let f = filter(&owner);
        let result = ev.core.filter_keeps(f, &mut f.env.clone(), &pair);
        if expression.contains("Cardinality") {
            assert_eq!(
                result.expect_err("original later error").ec(),
                crate::Ec::DivisionByZero
            );
        } else {
            assert!(!result.expect("original off-diagonal false"));
        }
    }
}

#[test]
fn resolved_operator_capture_of_outer_binder_still_declines_after_true_singletons() {
    let (ev, owner, mut source) = fixture(
        "Apply(P(_),v) == P(v)",
        "{A \\in SUBSET {1,2} : \\A e \\in A : Apply(LAMBDA z: Cardinality(A)=1, e)}",
        Options::tlc(),
    );
    assert!(
        actual_singletons(&owner, &mut source)
            .into_iter()
            .all(|v| v)
    );
    assert!(
        ClosedForall::inspect(&ev.core, &owner, &source)
            .expect("captured scope inspection")
            .is_none()
    );
    let pair = source.next().expect("pair");
    let f = filter(&owner);
    assert!(
        !ev.core
            .filter_keeps(f, &mut f.env.clone(), &pair)
            .expect("real captured outer value")
    );
}

#[test]
fn captured_mutable_thunks_state_and_effects_decline_without_forcing_them() {
    for expression in [
        "LET K == Print(9,2) IN {A \\in SUBSET (SUBSET {1,2}) : \\A e \\in A : Cardinality(e)=K}",
        "{A \\in SUBSET (SUBSET {1,2}) : \\A e \\in A : Cardinality(e)=x}",
        "{A \\in SUBSET (SUBSET {1,2}) : \\A e \\in A : PrintT(Cardinality(e)=1)}",
    ] {
        let (ev, owner, mut source) = fixture("", expression, Options::tlc());
        let empty = source.next().expect("empty");
        let f = filter(&owner);
        assert!(
            ev.core
                .filter_keeps(f, &mut f.env.clone(), &empty)
                .expect("original empty")
        );
        assert_eq!(ev.take_output(), Vec::<String>::new());
        assert!(
            ClosedForall::inspect(&ev.core, &owner, &source)
                .expect("conservative proof")
                .is_none()
        );
        assert_eq!(
            ev.take_output(),
            Vec::<String>::new(),
            "no speculative effects/force"
        );
    }
}

#[test]
fn closed_context_is_not_a_boolean_totality_or_fresh_source_certificate() {
    let (ev, owner, mut source) = fixture(
        "",
        "{A \\in SUBSET {1,2} : \\A e \\in A : 1}",
        Options::tlc(),
    );
    source.next().expect("empty");
    let proof = ClosedForall::inspect(&ev.core, &owner, &source)
        .expect("context proof")
        .expect("closed numeric body");
    let singleton = source.next().expect("singleton");
    let f = filter(&owner);
    assert_eq!(
        ev.core
            .filter_keeps(f, &mut f.env.clone(), &singleton)
            .expect_err("original Boolean check remains")
            .ec(),
        crate::Ec::ExpectedValue
    );
    let (_, other, other_source) = fixture(
        "",
        "{A \\in SUBSET {1,2} : \\A e \\in A : 1}",
        Options::tlc(),
    );
    assert!(!proof.belongs_to(&other, &other_source));
    assert!(
        ClosedForall::inspect(&ev.core, &other, &other_source)
            .expect("foreign decline")
            .is_none()
    );
}

#[derive(Debug)]
struct Provider {
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl Drop for Provider {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}
impl DeferredSet for Provider {
    fn kind(&self) -> DeferredKind {
        self.calls.fetch_add(1, Ordering::Relaxed);
        DeferredKind::Filter
    }
    fn elements(&self, _ctx: Ctx) -> dive_value::Result<Vec<Value>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(vec![1.into()])
    }
    fn member(&self, _v: &Value, _ctx: Ctx) -> Option<dive_value::Result<bool>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Vec::new()
    }
    fn as_any(&self) -> Option<&dyn ::std::any::Any> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Some(self)
    }
}
#[test]
fn unused_actual_provider_capture_is_not_queried_or_released_by_inspection() {
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    {
        let text = "---- MODULE CapturedProvider ----\nEXTENDS FiniteSets\nCONSTANT C\nMake(S,Unused) == {A \\in SUBSET S : \\A e \\in A : Cardinality(e)=1}\nF == Make(SUBSET {1,2},C)\n====\n";
        let analysis = dive_sema::analyze_source("CapturedProvider.tla", text, &Default::default());
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let def = ir.root_def("F").expect("F");
        let probe = Arc::new(Provider {
            calls: calls.clone(),
            drops: drops.clone(),
        });
        let mut model = Model::new(ir);
        model.constants[0] = Some(crate::ConstBinding::Value(Value::deferred(
            probe.clone(),
            Ctx::tlc(),
        )));
        let ev = Evaluator::new(model, Options::tlc()).expect("actual configured value");
        let value = ev
            .eval_def(def, &St::constant())
            .expect("original argument/capture construction");
        let owner = set(&value).as_deferred().expect("private Filter").clone();
        let mut source = set(&filter(&owner).in_val)
            .concrete_subset_stream(Ctx::tlc())
            .expect("component")
            .expect("positive cursor");
        source.next().expect("empty");
        let before = calls.load(Ordering::Relaxed);
        assert!(
            ClosedForall::inspect(&ev.core, &owner, &source)
                .expect("capture inspection")
                .is_none()
        );
        assert_eq!(calls.load(Ordering::Relaxed), before);
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        assert_eq!(
            probe.elements(Ctx::tlc()).expect("real positive provider"),
            vec![1.into()]
        );
        assert_eq!(calls.load(Ordering::Relaxed), before + 1);
    }
    assert_eq!(
        drops.load(Ordering::Relaxed),
        1,
        "actual final owner release is positive"
    );
}

#[derive(Debug)]
struct Stop(Arc<AtomicUsize>);
impl crate::HostControl for Stop {
    fn check(&self) -> std::result::Result<(), crate::Interrupted> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Err(crate::Interrupted::new(459))
    }
}
#[test]
fn metadata_cancellation_stays_typed_and_does_not_cache_a_failed_admission() {
    let (ev, owner, mut source) = fixture(
        "",
        "{A \\in SUBSET (SUBSET {1,2}) : \\A e \\in A : Cardinality(e)=1}",
        Options::tlc(),
    );
    let empty = source.next().expect("original empty");
    let f = filter(&owner);
    assert!(
        ev.core
            .filter_keeps(f, &mut f.env.clone(), &empty)
            .expect("empty predicate")
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let control = ev
        .host_control(Some(Arc::new(Stop(calls.clone()))))
        .expect("real control");
    let error = ClosedForall::inspect(&ev.core, &owner, &source)
        .err()
        .expect("original interruption");
    assert_eq!(error.interruption(), Some(crate::Interrupted::new(459)));
    assert!(calls.load(Ordering::Relaxed) > 0);
    assert_eq!(ev.take_output(), Vec::<String>::new());
    drop(control);
    assert!(
        ClosedForall::inspect(&ev.core, &owner, &source)
            .expect("same unconsumed context")
            .is_some()
    );
    assert_eq!(source.current_indices(), Some([].as_slice()));
}
