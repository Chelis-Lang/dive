//! Controlled supplied-trace replay retains typed interruption and real progress.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite validation controls"
)]

use dive_eval::{HostControl, Interrupted};
use dive_trace::{Check, Ending, Label, Trace, Validation, ValidationRunError};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
struct Control {
    calls: AtomicUsize,
    stop_at: AtomicUsize,
}
impl Control {
    fn new(stop_at: usize) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            stop_at: AtomicUsize::new(stop_at),
        })
    }
}
impl HostControl for Control {
    fn check(&self) -> Result<(), Interrupted> {
        if self.calls.fetch_add(1, Ordering::Relaxed) + 1 >= self.stop_at.load(Ordering::Relaxed) {
            Err(Interrupted::new(42))
        } else {
            Ok(())
        }
    }
}

fn model(body: &str, cfg: &str) -> dive_model::Model {
    let source =
        format!("---- MODULE Controlled ----\nEXTENDS Naturals\nVARIABLE x\n{body}\n====\n");
    let analysis =
        dive_sema::analyze_source("Controlled.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(cfg)
        .into_config()
        .expect("configuration");
    dive_model::bind(ir, &config, &dive_model::BindOptions::tlc()).expect("model")
}

#[test]
fn finite_session_words_keep_fresh_replay_and_typed_control() {
    let source = "---- MODULE FiniteControl ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x+1 /\\ (\\A k \\in 1..65536: k+x>=0)\n====\n";
    let analysis =
        dive_sema::analyze_source("FiniteControl.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    let session = dive_trace::Session::new(
        dive_model::Recipe::new(ir, config, dive_eval::Options::tlc(), false),
        None,
    );
    let valid = dive_trace::TraceDoc::behavior(vec![
        serde_json::json!({"x": 0}),
        serde_json::json!({"x": 1}),
    ])
    .expect("word");
    assert_eq!(
        session
            .read_controlled(&valid, Control::new(usize::MAX))
            .expect("completed replay")
            .0
            .validation,
        Validation::Valid
    );
    let forged = dive_trace::TraceDoc::behavior(vec![
        serde_json::json!({"x": 0}),
        serde_json::json!({"x": 2}),
    ])
    .expect("word");
    assert!(matches!(
        session
            .read_controlled(&forged, Control::new(usize::MAX))
            .expect("completed invalid replay")
            .0
            .validation,
        Validation::Invalid(_)
    ));
    let interrupted = session.read_controlled(&valid, Control::new(16));
    assert!(
        matches!(interrupted, Err(ValidationRunError::Interrupted { .. })),
        "{interrupted:?}"
    );
}
fn trace(states: &[i64]) -> Trace {
    Trace {
        states: states
            .iter()
            .map(|x| Arc::from([Some(Value::from(*x))]))
            .collect(),
        labels: vec![None; states.len()],
        ending: Ending::Behavior,
        length: None,
        properties: Vec::new(),
        concrete_closure: false,
        witnesses: Vec::new(),
        draws: Vec::new(),
    }
}
fn interruption(error: ValidationRunError, check: Check, state: Option<usize>) {
    let ValidationRunError::Interrupted { error, progress } = error else {
        panic!("typed interruption")
    };
    assert_eq!(error.class(), dive_eval::ErrorClass::Interrupted);
    assert_eq!(error.interruption().expect("identity").token(), 42);
    assert_eq!(progress.check, Some(check));
    assert_eq!(progress.state, state);
    assert!(progress.checks_passed > 0);
}

fn init_and_next_predicate_loops_interrupt_with_the_actual_check_position() {
    let m = model(
        "Init == x=0 /\\ \\A k \\in 1..4096: k+x=k\nNext == x'=x",
        "INIT Init\nNEXT Next\n",
    );
    let word = trace(&[0]);
    interruption(
        dive_trace::validate_controlled(&m, &word, Control::new(4)).expect_err("Init loop"),
        Check::Init,
        Some(0),
    );
    assert_eq!(
        dive_trace::validate(&m, &word).validation,
        Validation::Valid
    );
    let m = model(
        "Init == x=0\nNext == x'=x+1 /\\ \\A k \\in 1..4096: k+x>=0",
        "INIT Init\nNEXT Next\n",
    );
    let word = trace(&[0, 1]);
    interruption(
        dive_trace::validate_controlled(&m, &word, Control::new(4)).expect_err("Next loop"),
        Check::Next,
        Some(1),
    );
    assert_eq!(
        dive_trace::validate(&m, &word).validation,
        Validation::Valid
    );
}

fn optional_label_errors_never_swallow_host_interruption() {
    let m = model(
        "Init == x=0\nNext == x'=x+1\nLongLabel == x'=x+1 /\\ \\A k \\in 1..4096: k+x>=0",
        "INIT Init\nNEXT Next\n",
    );
    let mut word = trace(&[0, 1]);
    word.labels[1] = Some(Label {
        name: "LongLabel".into(),
        location: None,
    });
    interruption(
        dive_trace::validate_controlled(&m, &word, Control::new(4)).expect_err("label loop"),
        Check::Label,
        Some(1),
    );
    assert_eq!(
        dive_trace::validate(&m, &word).validation,
        Validation::Valid
    );
}

fn deadlock_enabled_and_temporal_atoms_preserve_the_typed_transport() {
    let m = model(
        "Init == x=0\nNext == x'=x /\\ \\E k \\in 1..4096: k+x<0",
        "INIT Init\nNEXT Next\n",
    );
    let mut word = trace(&[0]);
    word.ending = Ending::Deadlock;
    interruption(
        dive_trace::validate_controlled(&m, &word, Control::new(4)).expect_err("ENABLED loop"),
        Check::Ending,
        Some(0),
    );
    assert_eq!(
        dive_trace::validate(&m, &word).validation,
        Validation::Valid
    );
    let m = model(
        "Init == x=0\nNext == x'=x\nP == []<>((\\A k \\in 1..4096: k+x>=0) /\\ x=1)",
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    );
    let mut word = trace(&[0]);
    word.ending = Ending::Lasso { back_to: 0 };
    word.properties = vec!["P".into()];
    interruption(
        dive_trace::validate_controlled(&m, &word, Control::new(4))
            .expect_err("temporal atom loop"),
        Check::Temporal,
        None,
    );
    assert_eq!(
        dive_trace::validate(&m, &word).validation,
        Validation::Valid
    );
}

fn fairness_enabled_interruption_is_not_a_refusal_or_truth_value() {
    let m = model(
        "Init == x=0\nNext == x'=x\nA == x'=x+1 /\\ \\A k \\in 1..4096: k+x>=0\nSpec == Init /\\ [][Next]_x /\\ WF_x(A)\nP == <>(x=1)",
        "SPECIFICATION Spec\nPROPERTY P\n",
    );
    let mut word = trace(&[0]);
    word.ending = Ending::Lasso { back_to: 0 };
    word.properties = vec!["P".into()];
    interruption(
        dive_trace::validate_controlled(&m, &word, Control::new(4))
            .expect_err("fairness enabled loop"),
        Check::Temporal,
        None,
    );
}

fn earlier_invalid_checks_keep_their_class_and_scopes_restore_outer_control() {
    let m = model(
        "Init == x=0\nNext == x'=x\nQ == \\A k \\in 1..4096: k>=0",
        "INIT Init\nNEXT Next\n",
    );
    let control = Control::new(usize::MAX);
    let outer = m
        .evaluator
        .host_control(Some(Control::new(1)))
        .expect("outer owner");
    let report = dive_trace::validate_controlled(&m, &trace(&[1]), control.clone())
        .expect("ordinary invalid report");
    assert!(matches!(report.validation, Validation::Invalid(ref e) if e.check==Check::Init));
    assert_eq!(control.calls.load(Ordering::Relaxed), 3);
    // The controlled frame and private TraceHost are gone; the exact prior
    // owner remains installed, without borrowing its values or cache domain.
    assert!(m.evaluator.host().is_none());
    let q = m.ir.root_def("Q").expect("Q");
    let error = m
        .evaluator
        .eval_def(q, &dive_eval::St::constant())
        .expect_err("outer control restored");
    assert_eq!(error.class(), dive_eval::ErrorClass::Interrupted);
    drop(outer);
    assert_eq!(
        dive_trace::validate(&m, &trace(&[0])).validation,
        Validation::Valid
    );
}

fn on_stack(run: impl FnOnce() + Send) {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, run)
            .expect("recommended stack")
            .join()
            .expect("validation thread");
    });
}

#[test]
fn a_valid_first_pass_does_not_hide_interruption_of_the_decoded_native_pass() {
    on_stack(|| {
        let m = model("Init == x=0\nNext == x'=x", "INIT Init\nNEXT Next\n");
        let word = trace(&[0]);
        let control = Control::new(usize::MAX);
        assert_eq!(
            dive_trace::validate_controlled(&m, &word, control.clone())
                .expect("typed pass")
                .validation,
            Validation::Valid
        );
        let document = word.to_doc(&m).expect("native document");
        let decoded = Trace::decode(&m, &document).expect("native decoded states");
        control
            .stop_at
            .store(control.calls.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
        let ValidationRunError::Interrupted { error, progress } =
            dive_trace::validate_controlled(&m, &decoded, control)
                .expect_err("no second certificate")
        else {
            panic!("typed interruption")
        };
        assert_eq!(error.interruption().expect("same caller").token(), 42);
        assert_eq!(progress.checks_passed, 0);
        assert_eq!(progress.state, None);
        assert_eq!(
            dive_trace::validate(&m, &word).validation,
            Validation::Valid
        );
    });
}

#[test]
fn assumption_scope_defers_control_then_resumes_before_initial_predicates() {
    on_stack(|| {
        let m = model(
            "ASSUME \\A k \\in 1..4096: k>=0\nInit == x=0\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
        );
        let control = Control::new(3);
        let ValidationRunError::Interrupted { progress, .. } =
            dive_trace::validate_controlled(&m, &trace(&[0]), control.clone())
                .expect_err("after ASSUME")
        else {
            panic!("typed interruption")
        };
        assert_eq!(
            progress.checks_passed, 1,
            "actual assumption check completed"
        );
        assert_eq!(progress.state, Some(0));
        assert_eq!(progress.check, None, "Init was not evaluated");
        assert_eq!(control.calls.load(Ordering::Relaxed), 3);
        assert!(m.evaluator.host().is_none());
    });
}

#[derive(Debug)]
struct Panicking;
impl HostControl for Panicking {
    fn check(&self) -> Result<(), Interrupted> {
        panic!("caller control unwound")
    }
}

#[test]
fn unwind_and_cross_thread_conflict_restore_the_original_runtime() {
    on_stack(|| {
        let m = model("Init == x=0\nNext == x'=x", "INIT Init\nNEXT Next\n");
        let word = trace(&[0]);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dive_trace::validate_controlled(&m, &word, Arc::new(Panicking))
        }));
        assert!(outcome.is_err());
        assert!(m.evaluator.host().is_none());
        assert_eq!(
            dive_trace::validate(&m, &word).validation,
            Validation::Valid
        );
        let _outer = m
            .evaluator
            .host_control(Some(Control::new(usize::MAX)))
            .expect("outer");
        std::thread::scope(|scope| {
            let error = scope
                .spawn(|| dive_trace::validate_controlled(&m, &word, Control::new(usize::MAX)))
                .join()
                .expect("conflicting caller")
                .expect_err("explicit ownership failure");
            assert!(matches!(error, ValidationRunError::ControlConflict(_)));
        });
        assert!(m.evaluator.host().is_none());
    });
}

#[test]
fn init_and_next_predicate_loops_interrupt_with_the_actual_check_position_on_recommended_stack() {
    on_stack(init_and_next_predicate_loops_interrupt_with_the_actual_check_position);
}

#[test]
fn optional_label_errors_never_swallow_host_interruption_on_recommended_stack() {
    on_stack(optional_label_errors_never_swallow_host_interruption);
}

#[test]
fn deadlock_enabled_and_temporal_atoms_preserve_the_typed_transport_on_recommended_stack() {
    on_stack(deadlock_enabled_and_temporal_atoms_preserve_the_typed_transport);
}

#[test]
fn fairness_enabled_interruption_is_not_a_refusal_or_truth_value_on_recommended_stack() {
    on_stack(fairness_enabled_interruption_is_not_a_refusal_or_truth_value);
}

#[test]
fn earlier_invalid_checks_keep_their_class_and_scopes_restore_outer_control_on_recommended_stack() {
    on_stack(earlier_invalid_checks_keep_their_class_and_scopes_restore_outer_control);
}

#[test]
fn runtime_name_refusal_cannot_mask_an_already_interrupted_check() {
    #[derive(Debug)]
    struct NameAtStop {
        inner: Arc<Control>,
        names: Arc<dive_value::Interner>,
    }
    impl HostControl for NameAtStop {
        fn check(&self) -> Result<(), Interrupted> {
            self.inner.check().inspect_err(|_| {
                self.names.intern("created-at-control-stop");
            })
        }
    }
    on_stack(|| {
        for (body, ending, properties, check, state) in [
            (
                "Init == x=0 /\\ \\A k \\in 1..4096: k+x=k\nNext == x'=x",
                Ending::Behavior,
                Vec::new(),
                Check::Init,
                Some(0),
            ),
            (
                "Init == x=0\nNext == x'=x /\\ \\E k \\in 1..4096: k+x<0",
                Ending::Deadlock,
                Vec::new(),
                Check::Ending,
                Some(0),
            ),
            (
                "Init == x=0\nNext == x'=x\nP == []<>((\\A k \\in 1..4096: k+x>=0) /\\ x=1)",
                Ending::Lasso { back_to: 0 },
                vec!["P".into()],
                Check::Temporal,
                None,
            ),
        ] {
            let config = if properties.is_empty() {
                "INIT Init\nNEXT Next\n"
            } else {
                "INIT Init\nNEXT Next\nPROPERTY P\n"
            };
            let m = model(body, config);
            let mut word = trace(&[0]);
            word.ending = ending;
            word.properties = properties;
            let control = Arc::new(NameAtStop {
                inner: Control::new(4),
                names: m.names.clone(),
            });
            interruption(
                dive_trace::validate_controlled(&m, &word, control)
                    .expect_err("typed interruption wins over final name guard"),
                check,
                state,
            );
            assert!(m.has_runtime_names());
        }
    });
}

#[test]
fn final_invariant_claim_cannot_skip_an_interrupted_constraint() {
    on_stack(|| {
        let m = model(
            "Init == x=0\nNext == x'=x+1\nLimit == IF x=0 THEN TRUE ELSE \\A k \\in 1..4096: k+x>=0\nInv == x<1",
            "INIT Init\nNEXT Next\nCONSTRAINT Limit\nINVARIANT Inv\n",
        );
        let mut word = trace(&[0, 1]);
        word.ending = Ending::Invariant("Inv".into());
        interruption(
            dive_trace::validate_controlled(&m, &word, Control::new(4))
                .expect_err("constraint loop must remain demanded"),
            Check::Constraint,
            Some(1),
        );
        assert_eq!(
            dive_trace::validate(&m, &word).validation,
            Validation::Valid
        );
    });
}
