//! Actual pull/whole-predicate/sink sequencing and operative host replacement.
#![allow(
    clippy::expect_used,
    reason = "bounded source-lowered regression controls"
)]

use crate::tlc_enum::InElementStream;
use crate::{Evaluator, Host, HostControl, HostError, Interrupted, NativeHooks, Options, St};
use ::std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use dive_ir::StdOp;
use dive_value::Value;

fn evaluator(extra: &str, expression: &str, options: Options) -> Evaluator {
    let text = format!(
        "---- MODULE MaskRuntime ----\nEXTENDS Integers,FiniteSets,TLC\nVARIABLE x\n{extra}\nF == {expression}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("MaskRuntime.tla", &text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    Evaluator::new(
        crate::Model::new(Arc::new(dive_ir::lower(&analysis).expect("real IR"))),
        options,
    )
    .expect("evaluator")
}
fn set(value: &Value) -> &dive_value::Set {
    match value {
        Value::Set(set) => Some(set),
        _ => None,
    }
    .expect("set fixture")
}

#[derive(Debug)]
struct Ordinary;
impl Host for Ordinary {}

#[derive(Debug)]
struct Deny;
impl Host for Deny {
    fn check_native_access(&self, _op: StdOp) -> Result<(), HostError> {
        Err(HostError {
            code: 0,
            message: "actual callback refusal".into(),
        })
    }
}

fn cursor(ev: &Evaluator, st: &St) -> InElementStream {
    let value = ev
        .eval_def(ev.ir().root_def("F").expect("F"), st)
        .expect("real Filter");
    ev.core
        .in_element_stream(set(&value))
        .expect("prepare once")
        .expect("positive concrete cursor")
        .with_owner(value)
}

fn drain(cursor: &mut InElementStream) -> Vec<Value> {
    cursor
        .by_ref()
        .collect::<crate::Result<Vec<_>>>()
        .expect("original complete predicates")
}

fn ready(ev: &Evaluator) -> InElementStream {
    let mut cursor = cursor(ev, &St::constant());
    assert_eq!(
        cursor.next().expect("empty").expect("forall"),
        Value::empty_set(ev.options().ctx)
    );
    assert_eq!(
        cursor.mask_progress(),
        Some(("empty", 0)),
        "proof waits for the next pull"
    );
    for _ in 0..3 {
        assert!(cursor.next().expect("accepted singleton").is_ok());
    }
    assert_eq!(cursor.mask_progress(), Some(("ready", 0)));
    cursor
}

const THREE_OF_FOUR: &str = "{A \\in SUBSET (1..4) : \\A e \\in A : e<2 \\/ e>2}";

#[test]
fn actual_mask_matches_fresh_original_whole_filter_for_every_small_cardinality() {
    for options in [Options::dive(), Options::tlc()] {
        for cardinality in 0..=3 {
            let expression = format!(
                "{{A \\in SUBSET (SUBSET (1..3)) : \\A e \\in A : Cardinality(e)={cardinality}}}"
            );
            // Callback is the original unskipped path, executed first.
            let reference = evaluator("", &expression, options);
            reference.set_host(Arc::new(Ordinary));
            let expected = drain(&mut cursor(&reference, &St::constant()));
            let actual = evaluator("", &expression, options);
            actual.set_host(Arc::new(Ordinary));
            let _operative = actual
                .native_hook_scope(NativeHooks::Unrestricted)
                .expect("domain0");
            let mut stream = cursor(&actual, &St::constant());
            assert_eq!(drain(&mut stream), expected);
            let (phase, commits) = stream.mask_progress().expect("typed path");
            assert_eq!(phase, "ready");
            assert!(commits > 0, "actual current-binding commits occurred");
            assert_eq!(actual.take_output(), Vec::<String>::new());
        }
    }
}

#[test]
fn original_empty_sink_precedes_inspection_and_singleton_errors_are_not_learned() {
    for options in [Options::dive(), Options::tlc()] {
        for (q, code) in [
            ("IF e=2 THEN 1 \\div 0=0 ELSE TRUE", 2179),
            ("IF e=2 THEN 1 ELSE TRUE", 2215),
        ] {
            let expression = format!("{{A \\in SUBSET (1..2) : \\A e \\in A : {q}}}");
            let mut outcomes = Vec::new();
            for operative in [false, true] {
                let ev = evaluator("", &expression, options);
                ev.set_host(Arc::new(Ordinary));
                let _scope = operative.then(|| {
                    ev.native_hook_scope(NativeHooks::Unrestricted)
                        .expect("domain0")
                });
                let mut stream = cursor(&ev, &St::constant());
                let empty = stream.next().expect("first empty").expect("vacuous forall");
                assert_eq!(stream.mask_progress(), Some(("empty", 0)));
                // Stopping/dropping here has performed no proof or Q work.
                assert_eq!(empty, Value::empty_set(options.ctx));
                let singleton = stream.next().expect("first singleton").expect("TRUE");
                let error = stream
                    .next()
                    .expect("second singleton")
                    .expect_err("original error");
                assert_eq!(error.ec().code(), code);
                assert_eq!(stream.mask_progress().expect("typed source").1, 0);
                outcomes.push((empty, singleton, error.ec(), error.message().to_owned()));
            }
            assert_eq!(outcomes[1], outcomes[0]);
        }
    }
}

#[test]
fn callback_replacement_prevents_commit_and_preserves_the_exact_next_error() {
    for options in [Options::dive(), Options::tlc()] {
        let mut outcomes = Vec::new();
        for operative in [false, true] {
            let ev = evaluator("", THREE_OF_FOUR, options);
            ev.set_host(Arc::new(Ordinary));
            let _scope = operative.then(|| {
                ev.native_hook_scope(NativeHooks::Unrestricted)
                    .expect("domain0")
            });
            let mut stream = ready(&ev);
            ev.set_host(Arc::new(Deny)); // legacy replacement always Callback
            let error = stream
                .next()
                .expect("next original candidate")
                .expect_err("native hook");
            assert_eq!(stream.mask_progress(), Some(("ready", 0)));
            assert!(error.message().contains("actual callback refusal"));
            outcomes.push((error.class(), error.message().to_owned()));
        }
        assert_eq!(outcomes[1], outcomes[0]);
    }
}

#[derive(Debug)]
struct ReplaceOnPoll {
    ev: Evaluator,
    armed: Arc<AtomicBool>,
}
impl HostControl for ReplaceOnPoll {
    fn check(&self) -> Result<(), Interrupted> {
        if self.armed.swap(false, Ordering::Relaxed) {
            self.ev.set_host(Arc::new(Deny));
        }
        Ok(())
    }
}

#[test]
fn reentrant_control_replacement_is_outside_the_lock_and_checked_at_commit() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator("", THREE_OF_FOUR, options);
        ev.set_host(Arc::new(Ordinary));
        let _operative = ev
            .native_hook_scope(NativeHooks::Unrestricted)
            .expect("domain0");
        let mut stream = ready(&ev);
        let armed = Arc::new(AtomicBool::new(true));
        let _control = ev
            .host_control(Some(Arc::new(ReplaceOnPoll {
                ev: ev.clone(),
                armed: armed.clone(),
            })))
            .expect("selected control");
        let error = stream
            .next()
            .expect("original next")
            .expect_err("callback now active");
        assert!(!armed.load(Ordering::Relaxed), "actual reentrant hook ran");
        assert!(error.message().contains("actual callback refusal"));
        assert_eq!(stream.mask_progress(), Some(("ready", 0)));
    }
}

#[test]
fn callback_interval_can_enumerate_normally_then_resume_same_source_mask() {
    for options in [Options::dive(), Options::tlc()] {
        let reference = evaluator("", THREE_OF_FOUR, options);
        let expected = drain(&mut cursor(&reference, &St::constant()));
        let ev = evaluator("", THREE_OF_FOUR, options);
        ev.set_host(Arc::new(Ordinary));
        let mut stream = ready(&ev);
        let mut values = Vec::new();
        // The next original pair {1,2} is false, then {1,3} reaches the sink.
        values.push(
            stream
                .next()
                .expect("ordinary next accepted pair")
                .expect("predicate"),
        );
        assert_eq!(stream.mask_progress(), Some(("ready", 0)));
        {
            let _operative = ev
                .native_hook_scope(NativeHooks::Unrestricted)
                .expect("domain0");
            values.extend(drain(&mut stream));
        }
        assert_eq!(values, expected[4..]);
        assert!(stream.mask_progress().expect("typed path").1 > 0);
    }
}

#[derive(Debug)]
struct Stop;
impl HostControl for Stop {
    fn check(&self) -> Result<(), Interrupted> {
        Err(Interrupted::new(459))
    }
}

#[test]
fn typed_cancellation_and_changed_depth_do_not_consume_a_skipped_position() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator("", THREE_OF_FOUR, options);
        ev.set_host(Arc::new(Ordinary));
        let _operative = ev
            .native_hook_scope(NativeHooks::Unrestricted)
            .expect("domain0");
        let mut stream = ready(&ev);
        {
            let _control = ev.host_control(Some(Arc::new(Stop))).expect("control");
            let error = stream
                .next()
                .expect("original polling boundary")
                .expect_err("actual cancellation");
            assert_eq!(error.class(), crate::ErrorClass::Interrupted);
            assert_eq!(
                error.interruption().expect("actual typed receipt").token(),
                459
            );
        }
        // Cancellation in Iterator makes it terminal; a separate actual ready
        // cursor proves depth mismatch falls back rather than suppressing work.
        let mut stream = ready(&ev);
        let node = ev.ir().def(ev.ir().root_def("F").expect("F")).body;
        {
            let _depth = ev.core.enter(node).expect("one real active guard");
            assert!(stream.next().expect("ordinary pair").is_ok());
            assert_eq!(stream.mask_progress(), Some(("ready", 0)));
        }
        assert_ne!(drain(&mut stream), [] as [Value; 0]);
        assert!(stream.mask_progress().expect("typed path").1 > 0);
    }
}

#[test]
fn effects_outer_dependence_and_unknown_captures_keep_original_full_predicates() {
    for options in [Options::dive(), Options::tlc()] {
        for expression in [
            "{A \\in SUBSET (1..3) : \\A e \\in A : Print(e,e>1)}",
            "{A \\in SUBSET (1..3) : \\A e \\in A : Cardinality(A)=1}",
            "LET k == 1+0 IN {A \\in SUBSET (1..3) : \\A e \\in A : e>k}",
        ] {
            let mut outcomes = Vec::new();
            for operative in [false, true] {
                let ev = evaluator("", expression, options);
                ev.set_host(Arc::new(Ordinary));
                let _scope = operative.then(|| {
                    ev.native_hook_scope(NativeHooks::Unrestricted)
                        .expect("domain0")
                });
                let mut stream = cursor(&ev, &St::constant());
                let values = drain(&mut stream);
                assert_eq!(stream.mask_progress(), Some(("off", 0)));
                outcomes.push((values, ev.take_output()));
            }
            assert_eq!(outcomes[1], outcomes[0]);
        }
    }
}

#[test]
fn unused_partial_state_functions_are_fixed_data_but_actual_state_reads_decline() {
    for options in [Options::dive(), Options::tlc()] {
        for reads_state in [false, true] {
            let expression = if reads_state {
                "{A \\in SUBSET {10,20} : \\A e \\in A : x[e]=e}"
            } else {
                "{A \\in SUBSET {10,20} : \\A e \\in A : e>10}"
            };
            let function = Value::fcn(
                vec![(10.into(), 10.into()), (20.into(), 20.into())],
                options.ctx,
            )
            .expect("real general function");
            let f = match &function {
                Value::Fcn(f) => Some(f),
                _ => None,
            }
            .expect("actual function");
            assert_eq!(f.kind(), dive_value::FcnKind::General);
            let st = St::state(Arc::from([Some(function)]));
            let reference = evaluator("", expression, options);
            let expected = drain(&mut cursor(&reference, &st));
            let ev = evaluator("", expression, options);
            ev.set_host(Arc::new(Ordinary));
            let _operative = ev
                .native_hook_scope(NativeHooks::Unrestricted)
                .expect("domain0");
            let mut stream = cursor(&ev, &st);
            assert_eq!(drain(&mut stream), expected);
            let (phase, commits) = stream.mask_progress().expect("typed path");
            if reads_state {
                assert_eq!((phase, commits), ("off", 0));
            } else {
                assert_eq!(phase, "ready");
                assert!(commits > 0);
            }
        }
        // These are constructor errors before any Filter can retain the value;
        // the public immutable Fcn representation cannot forge these domains.
        assert!(
            Value::fcn(
                vec![(1.into(), 0.into()), (1.into(), 2.into())],
                options.ctx
            )
            .is_err()
        );
        assert!(
            Value::fcn(
                vec![(1.into(), 0.into()), (Value::TRUE, 2.into())],
                options.ctx
            )
            .is_err()
        );
        let nested = Value::fcn(
            vec![(
                10.into(),
                Value::set_raw(vec![1.into(), Value::TRUE], options.ctx),
            )],
            options.ctx,
        )
        .expect("stored raw child");
        let ev = evaluator("", "{A \\in SUBSET {10,20} : \\A e \\in A : e>10}", options);
        ev.set_host(Arc::new(Ordinary));
        let _operative = ev
            .native_hook_scope(NativeHooks::Unrestricted)
            .expect("domain0");
        let mut stream = cursor(&ev, &St::state(Arc::from([Some(nested)])));
        assert_eq!(drain(&mut stream).len(), 2);
        assert_eq!(
            stream.mask_progress(),
            Some(("off", 0)),
            "unknown normalization child never supplies fixed-data proof"
        );
    }
}

#[derive(Debug)]
struct StoredProvider {
    calls: Arc<::std::sync::atomic::AtomicUsize>,
    drops: Arc<::std::sync::atomic::AtomicUsize>,
}
impl Drop for StoredProvider {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
    }
}
impl dive_value::DeferredSet for StoredProvider {
    fn kind(&self) -> dive_value::DeferredKind {
        self.calls.fetch_add(1, Ordering::Relaxed);
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _ctx: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(vec![1.into()])
    }
    fn member(&self, _value: &Value, _ctx: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Vec::new()
    }
}

#[test]
fn provider_inside_unused_stored_function_declines_without_callbacks_or_early_release() {
    use dive_value::DeferredSet;
    for options in [Options::dive(), Options::tlc()] {
        let calls = Arc::new(::std::sync::atomic::AtomicUsize::new(0));
        let drops = Arc::new(::std::sync::atomic::AtomicUsize::new(0));
        {
            let provider = Arc::new(StoredProvider {
                calls: calls.clone(),
                drops: drops.clone(),
            });
            let value = Value::fcn(
                vec![(10.into(), Value::deferred(provider.clone(), options.ctx))],
                options.ctx,
            )
            .expect("real immutable function retaining provider");
            let ev = evaluator("", "{A \\in SUBSET (1..3) : \\A e \\in A : e>1}", options);
            ev.set_host(Arc::new(Ordinary));
            let _operative = ev
                .native_hook_scope(NativeHooks::Unrestricted)
                .expect("domain0");
            let mut stream = cursor(&ev, &St::state(Arc::from([Some(value)])));
            let before = calls.load(Ordering::Relaxed);
            assert_eq!(drain(&mut stream).len(), 4);
            assert_eq!(stream.mask_progress(), Some(("off", 0)));
            assert_eq!(calls.load(Ordering::Relaxed), before);
            assert_eq!(drops.load(Ordering::Relaxed), 0);
            assert_eq!(
                provider
                    .elements(options.ctx)
                    .expect("positive actual provider"),
                vec![1.into()]
            );
            assert_eq!(calls.load(Ordering::Relaxed), before + 1);
        }
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }
}
