//! Reached query dispatch must preserve actual forcing and isolate caches.
#![allow(clippy::expect_used, reason = "owned semantic fixtures")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use dive_eval::{EnabledResolver, Evaluator, Model, Options, Scope, St};
use dive_ir::{Arg, ExprId, Pass};
use dive_value::Value;

#[derive(Debug)]
struct Answer {
    value: bool,
    calls: AtomicUsize,
}
impl Answer {
    fn new(value: bool) -> Arc<Self> {
        Arc::new(Self {
            value,
            calls: AtomicUsize::new(0),
        })
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
impl EnabledResolver for Answer {
    fn resolve(&self, _: &Scope, _: ExprId, _: &St) -> dive_eval::Result<bool> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.value)
    }
}

fn evaluator() -> Evaluator {
    let analysis = dive_sema::analyze_source(
        "Resolver.tla",
        "---- MODULE Resolver ----\nEXTENDS Naturals\nVARIABLE x\nQ==ENABLED (x'=1)\nGuard(p)==p /\\ p\nShort==FALSE /\\ Q\nData==<<Q, Q', UNCHANGED Q>>\n====\n",
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    Evaluator::new(
        Model::new(Arc::new(dive_ir::lower(&analysis).expect("IR"))),
        Options::tlc(),
    )
    .expect("evaluator")
}

#[test]
fn query_callbacks_are_reached_in_data_and_all_state_projections() {
    let ev = evaluator();
    let answer = Answer::new(true);
    let state = Arc::from([Some(0.into())]);
    let st = ev
        .with_enabled_resolver(
            &St::action(state, Arc::from([Some(1.into())])),
            answer.clone(),
        )
        .expect("invocation");
    assert_eq!(
        ev.eval_def(ev.ir().root_def("Short").expect("Short"), &st)
            .expect("unreached query"),
        Value::FALSE
    );
    assert_eq!(answer.calls(), 0);
    assert_eq!(
        ev.eval_def(ev.ir().root_def("Data").expect("Data"), &st)
            .expect("queries in original data evaluation"),
        Value::tuple(vec![Value::TRUE, Value::TRUE, Value::TRUE])
    );
    assert_eq!(answer.calls(), 4);
    assert_eq!(
        ev.eval_def(ev.ir().root_def("Q").expect("Q"), &st.at_constant())
            .expect("constant projection preserves callback"),
        Value::TRUE
    );
    assert_eq!(answer.calls(), 5);
}

#[test]
fn lazy_cache_requires_the_same_actual_resolver_and_does_not_retain_it() {
    let ev = evaluator();
    let guard = ev.ir().root_def("Guard").expect("Guard");
    let q = ev.ir().root_def("Q").expect("Q");
    let body = ev.ir().def(guard).body;
    assert!(
        !ev.ir()
            .node(body)
            .effects
            .contains(dive_ir::Effects::USES_ENABLED)
    );
    let scope = ev
        .scope_apply(
            &ev.scope(q).expect("caller"),
            guard,
            &[Arg::Expr {
                expr: ev.ir().def(q).body,
                pass: Pass::Lazy,
            }],
        )
        .expect("real lazy query argument");
    let state = St::state(Arc::from([Some(0.into())]));
    let yes = Answer::new(true);
    let yes_st = ev
        .with_enabled_resolver(&state, yes.clone())
        .expect("yes invocation");
    for _ in 0..2 {
        assert_eq!(
            ev.eval_scoped(&scope, body, &yes_st)
                .expect("cached within this resolver"),
            Value::TRUE
        );
    }
    assert_eq!(yes.calls(), 1, "ordinary thunk caching remains effective");
    let no = Answer::new(false);
    let no_st = ev
        .with_enabled_resolver(&state, no.clone())
        .expect("no invocation");
    assert_eq!(
        ev.eval_scoped(&scope, body, &no_st)
            .expect("different resolver must execute"),
        Value::FALSE
    );
    assert_eq!(no.calls(), 1);
    assert_eq!(
        ev.eval_scoped(&scope, body, &yes_st).expect("switch back"),
        Value::TRUE
    );
    assert_eq!(yes.calls(), 2);
    #[cfg(not(feature = "enumerate"))]
    assert!(
        ev.eval_scoped(&scope, body, &state).is_err(),
        "a cached client decision cannot supply core ENABLED"
    );
    drop(no_st);
    drop(yes_st);
    assert_eq!(
        Arc::strong_count(&yes),
        1,
        "cache retains only weak identity"
    );
    assert_eq!(Arc::strong_count(&no), 1);
}

#[test]
fn foreign_invocation_refuses_before_entering_the_callback() {
    let ev = evaluator();
    let foreign = evaluator();
    let answer = Answer::new(true);
    let st = ev
        .with_enabled_resolver(&St::state(Arc::from([Some(0.into())])), answer.clone())
        .expect("owned invocation");
    assert!(
        foreign
            .eval_def(foreign.ir().root_def("Q").expect("Q"), &st)
            .is_err()
    );
    assert!(foreign.with_enabled_resolver(&st, answer.clone()).is_err());
    assert_eq!(answer.calls(), 0);
}
