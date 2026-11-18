//! Native candidate binding retains run authority and real capture evidence.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded deterministic binding controls"
)]
use super::*;
use dive_eval::HostControl;
use dive_liveness::analyze::periodic::external::{NativeAnalysis, NativeFinal};

/// A validation-binding failure carrying a complete capture's evidence.
fn final_binding_validation_failure(
    failure: Failure,
    capture: &capture::CompleteEvidence,
) -> Failure {
    let mut failure = validation_binding_failure(failure);
    failure.exploration = Some(Box::new(native::facts(
        capture.safety_result(),
        capture.stats(),
    )));
    failure
}

/// A validation-binding failure carrying an incomplete prefix's evidence.
fn prefix_binding_validation_failure(
    failure: Failure,
    capture: &capture::PrefixEvidence,
) -> Failure {
    let mut failure = validation_binding_failure(failure);
    failure.exploration = Some(Box::new(native::facts(
        capture.safety_result(),
        capture.stats(),
    )));
    failure.incomplete = true;
    failure
}

/// Core's complete evidence for a final candidate of `ir`.
fn core_candidate(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    options: &CheckOptions,
    tlc: bool,
) -> capture::CompleteEvidence {
    let NativeAnalysis::Final(NativeFinal::Candidate(_, evidence)) =
        crate::temporal::core_analysis(ir, cfg, options, tlc, false)
    else {
        panic!("Core final candidate");
    };
    evidence
}

/// Core's evidence for a due-prefix candidate of `ir`.
fn core_prefix(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    options: &CheckOptions,
    tlc: bool,
) -> capture::PrefixEvidence {
    let NativeAnalysis::Prefix(prefix) =
        crate::temporal::core_analysis(ir, cfg, options, tlc, true)
    else {
        panic!("Core prefix candidate");
    };
    prefix.exploration
}

fn input() -> (Arc<Ir>, ModelConfig) {
    let source = "---- MODULE Binding ----\nEXTENDS Naturals\nVARIABLE x\nInit==x=0\nNext==x'=IF x<2 THEN x+1 ELSE x\nP==<>(x=3)\n====\n";
    let sema = dive_sema::analyze_source("Binding.tla", source, &dive_sema::Options::default());
    assert!(sema.accepted());
    (
        Arc::new(dive_ir::lower(&sema).expect("IR")),
        dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("config"),
    )
}

#[test]
fn initial_binding_stop_has_actual_no_search_evidence() {
    let (ir, cfg) = input();
    for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let watchdog = dive_runtime::budget::Budget::new(Some(Duration::ZERO), None)
            .start()
            .expect("thread")
            .expect("watchdog");
        options.budget = watchdog.budget().clone();
        let control = dive_runtime::budget::EvaluationControl::new(
            &options.budget,
            dive_runtime::budget::Phase::Binding,
        )
        .expect("owner");
        let failure =
            bind_controlled(&ir, &cfg, &options, &control).expect_err("binding entry stop");
        assert_eq!(failure.stage(), Stage::Binding);
        assert_eq!(failure.capture_completed(), Some(false));
        assert!(failure.exploration().is_none());
        assert_eq!(failure.workers(4).expect("actual no search").used, 0);
        assert!(failure.validation_progress().is_none());
        assert!(
            matches!(failure.error(),Error::Analysis(AnalysisError::Stopped(stop)) if stop.phase==dive_runtime::budget::Phase::Binding)
        );
    }
}

#[test]
fn fresh_candidate_binding_stop_retains_complete_capture_and_cannot_certify_a_word() {
    let (ir, cfg) = input();
    for (tlc, mut options) in [
        (false, CheckOptions::dive(0)),
        (true, CheckOptions::tlc(0, 0)),
    ] {
        let candidate = core_candidate(&ir, &cfg, &options, tlc);
        let actual = candidate.stats();
        let watchdog = dive_runtime::budget::Budget::new(Some(Duration::ZERO), None)
            .start()
            .expect("thread")
            .expect("watchdog");
        options.budget = watchdog.budget().clone();
        let control = validation_control(&options).expect("validation owner");
        // Enter the real fresh binder directly with an expired validation
        // owner, isolating the new binding boundary from admission checkpoints.
        let failure =
            bind_controlled(&ir, &cfg, &options, &control).expect_err("fresh binding interrupted");
        let failure = final_binding_validation_failure(failure, &candidate);
        assert_eq!(failure.stage(), Stage::Validation);
        assert_eq!(failure.capture_completed(), Some(true));
        assert_eq!(failure.exploration().expect("real capture").stats, actual);
        assert!(
            matches!(failure.error(),Error::Analysis(AnalysisError::Stopped(stop)) if stop.phase==dive_runtime::budget::Phase::Validation)
        );
        let progress = failure
            .validation_progress()
            .expect("binding admits no validator check");
        assert_eq!(progress.checks_passed, 0);
        assert_eq!(progress.state, None);
        assert_eq!(progress.check, None);
    }
}

#[test]
fn binding_error_transport_preserves_output_and_rejects_foreign_or_unarmed_authority() {
    let watchdog = dive_runtime::budget::Budget::new(Some(Duration::ZERO), None)
        .start()
        .expect("thread")
        .expect("watchdog");
    let control = dive_runtime::budget::EvaluationControl::new(
        watchdog.budget(),
        dive_runtime::budget::Phase::Binding,
    )
    .expect("owner");
    let error = dive_eval::EvalError::interrupted(control.check().expect_err("expired"));
    let output = vec![dive_model::Message::Print("before".into())];
    let make_failure = || dive_model::ControlledBindError::Interrupted {
        error: error.clone(),
        output: output.clone(),
    };
    let failure = controlled_binding_failure(make_failure(), &control);
    assert_eq!(failure.binding_output(), output);
    assert!(matches!(
        failure.error(),
        Error::Analysis(AnalysisError::Stopped(_))
    ));
    let other = dive_runtime::budget::EvaluationControl::new(
        &dive_runtime::budget::Budget::unlimited(),
        dive_runtime::budget::Phase::Binding,
    )
    .expect("unarmed owner");
    let foreign = controlled_binding_failure(make_failure(), &other);
    assert!(matches!(
        foreign.error(),
        Error::Analysis(AnalysisError::Inconsistent(_))
    ));
    assert_eq!(foreign.binding_output(), output);
}

#[test]
fn fresh_prefix_binding_stop_retains_incomplete_capture() {
    let (ir, cfg) = input();
    for (tlc, mut options) in [
        (false, CheckOptions::dive(0)),
        (true, CheckOptions::tlc(0, 0)),
    ] {
        let candidate = core_prefix(&ir, &cfg, &options, tlc);
        let actual = candidate.stats();
        let watchdog = dive_runtime::budget::Budget::new(Some(Duration::ZERO), None)
            .start()
            .expect("thread")
            .expect("watchdog");
        options.budget = watchdog.budget().clone();
        let control = validation_control(&options).expect("validation owner");
        let failure =
            bind_controlled(&ir, &cfg, &options, &control).expect_err("fresh binding interrupted");
        let failure = prefix_binding_validation_failure(failure, &candidate);
        assert_eq!(failure.stage(), Stage::Validation);
        assert_eq!(failure.capture_completed(), Some(false));
        assert_eq!(
            failure
                .exploration()
                .expect("actual incomplete capture")
                .stats,
            actual
        );
        assert_eq!(
            failure
                .validation_progress()
                .expect("no validation checks")
                .checks_passed,
            0
        );
        assert!(
            matches!(failure.error(),Error::Analysis(AnalysisError::Stopped(stop)) if stop.phase==dive_runtime::budget::Phase::Validation)
        );
    }
}

enum WriterExploration {
    Complete(Box<capture::CompleteEvidence>),
    Prefix(Box<capture::PrefixEvidence>),
}

/// Produces a real interrupted binder result for the report projection controls.
/// `capture` selects no exploration, a complete candidate, or a real prefix.
#[allow(
    clippy::too_many_lines,
    reason = "keep real capture and separately interrupted binding provenance together in one fixture"
)]
pub(crate) fn writer_failure(tlc: bool, capture: Option<bool>) -> (CheckOptions, Failure) {
    use std::sync::atomic::{AtomicBool, Ordering};
    #[derive(Debug)]
    struct ArmOnPrint {
        printed: AtomicBool,
        owner: Arc<dive_runtime::budget::EvaluationControl>,
    }
    impl dive_eval::Host for ArmOnPrint {
        fn print_output(&self, text: &str) -> Result<bool, dive_eval::HostError> {
            if text.contains("rebind-arm") {
                self.printed.store(true, Ordering::Relaxed);
            }
            Ok(true)
        }
    }
    impl HostControl for ArmOnPrint {
        fn check(&self) -> Result<(), dive_eval::Interrupted> {
            if self.printed.load(Ordering::Relaxed) {
                self.owner.check()
            } else {
                Ok(())
            }
        }
    }
    let source = r#"---- MODULE BindingWriter ----
EXTENDS Naturals, TLC
VARIABLE x
Boot == PrintT("startup-output")
Dom(a) == IF PrintT("rebind-arm") /\ (\A i \in 1..4096 : i >= a) THEN {0} ELSE {}
Init == x=0
Next == \E a \in Dom(0) : x'=IF x<2 THEN x+1 ELSE x
StateP == x>=0
P == <>(x=3)
====
"#;
    let sema =
        dive_sema::analyze_source("BindingWriter.tla", source, &dive_sema::Options::default());
    assert!(sema.accepted());
    let ir = Arc::new(dive_ir::lower(&sema).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY StateP P\n")
        .into_config()
        .expect("config");
    let mut options = if tlc {
        CheckOptions::tlc(0, 0)
    } else {
        CheckOptions::dive(0)
    };
    let exploration = capture.map(|complete| {
        // These report-projection controls combine independently real capture
        // and binding-failure evidence. The four controls above separately
        // prove same-model fresh-binding authority. Keep this capture's Next
        // pure: an effectful action need not yield an admitted temporal prefix.
        let source = "---- MODULE BindingCapture ----\nEXTENDS Naturals, TLC\nVARIABLE x\nBoot == PrintT(\"capture-output\")\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nP == <>(x=3)\n====\n";
        let sema = dive_sema::analyze_source("BindingCapture.tla", source, &dive_sema::Options::default());
        assert!(sema.accepted());
        let ir = Arc::new(dive_ir::lower(&sema).expect("capture IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config().expect("capture configuration");
        if complete {
            WriterExploration::Complete(Box::new(core_candidate(&ir, &cfg, &options, tlc)))
        } else {
            WriterExploration::Prefix(Box::new(core_prefix(&ir, &cfg, &options, tlc)))
        }
    });
    let watchdog = dive_runtime::budget::Budget::new(Some(Duration::ZERO), None)
        .start()
        .expect("thread")
        .expect("watchdog");
    options.budget = watchdog.budget().clone();
    let phase = if capture.is_some() {
        dive_runtime::budget::Phase::Validation
    } else {
        dive_runtime::budget::Phase::Binding
    };
    let owner =
        dive_runtime::budget::EvaluationControl::new(&options.budget, phase).expect("actual owner");
    let adapter = Arc::new(ArmOnPrint {
        printed: AtomicBool::new(false),
        owner: owner.clone(),
    });
    let opts = BindOptions {
        eval: options.eval,
        symmetry_liveness: options.symmetry_liveness,
        no_deadlock: options.no_deadlock,
        host: Some(adapter.clone()),
    };
    let failure = dive_model::bind_controlled(ir, &cfg, &opts, adapter)
        .expect_err("real binding stops only after actual Print");
    assert!(
        failure
            .output()
            .iter()
            .any(|message| matches!(message, dive_model::Message::Warning { .. }))
    );
    assert_eq!(failure.output().iter().filter(|message| matches!(message, dive_model::Message::Print(text) if text.contains("rebind-arm"))).count(), 1);
    let failure = controlled_binding_failure(failure, &owner);
    let failure = match exploration {
        Some(WriterExploration::Complete(complete)) => {
            final_binding_validation_failure(failure, &complete)
        }
        Some(WriterExploration::Prefix(prefix)) => {
            prefix_binding_validation_failure(failure, &prefix)
        }
        None => failure,
    };
    (options, failure)
}
