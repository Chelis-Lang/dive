//! Controlled binding preserves actual startup effects and typed interruption.
#![allow(clippy::expect_used, reason = "bounded deterministic binding fixtures")]

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use dive_eval::{Host, HostControl, HostError, Interrupted};
use dive_model::{BindOptions, ControlledBindError, Message};

#[derive(Debug, Default)]
struct Control {
    armed: AtomicBool,
    checks: AtomicUsize,
}

impl Host for Control {
    fn print_output(&self, text: &str) -> Result<bool, HostError> {
        if text.contains("arm") {
            self.armed.store(true, Ordering::Relaxed);
        }
        // Keep ordinary output collection, rather than swallowing it in the host.
        Ok(true)
    }
}
impl HostControl for Control {
    fn check(&self) -> Result<(), Interrupted> {
        self.checks.fetch_add(1, Ordering::Relaxed);
        if self.armed.load(Ordering::Relaxed) {
            Err(Interrupted::new(73))
        } else {
            Ok(())
        }
    }
}

fn inputs(body: &str, config: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let source =
        format!("---- MODULE Binding ----\nEXTENDS Naturals, TLC\nVARIABLE x\n{body}\n====\n");
    let sema = dive_sema::analyze_source("Binding.tla", &source, &dive_sema::Options::default());
    assert!(sema.accepted(), "{sema:?}");
    let ir = Arc::new(dive_ir::lower(&sema).expect("IR"));
    let cfg = dive_config::parse(config).into_config().expect("config");
    (ir, cfg)
}

fn interrupted(failure: ControlledBindError) {
    let ControlledBindError::Interrupted { error, output } = failure else {
        unreachable_failure();
        return;
    };
    assert_eq!(error.class(), dive_eval::ErrorClass::Interrupted);
    assert_eq!(
        error.interruption().expect("typed interruption").token(),
        73
    );
    assert!(
        error.origin().is_some(),
        "actual failing loop expression retained"
    );
    assert_eq!(output.len(), 1);
    assert!(
        matches!(&output[0], Message::Print(text) if text.contains("arm") && !text.contains("after"))
    );
}

#[allow(clippy::panic, reason = "a different error is a semantic regression")]
fn unreachable_failure() {
    panic!("expected original typed binding interruption");
}

const STARTUP: &str = r#"
D == IF PrintT("arm") THEN ((\A i \in 1..4096 : i >= 0) /\ PrintT("after")) ELSE FALSE
Init == x = 0
Next == x' = x
"#;

#[test]
fn interrupted_constant_and_configured_constant_preserve_output_without_refusal() {
    for base in [BindOptions::dive(), BindOptions::tlc()] {
        for configured in [false, true] {
            let body = if configured {
                format!("CONSTANT C\n{STARTUP}")
            } else {
                STARTUP.into()
            };
            let config = if configured {
                "CONSTANT C <- D\nINIT Init\nNEXT Next\n"
            } else {
                "INIT Init\nNEXT Next\n"
            };
            let (ir, cfg) = inputs(&body, config);
            let control = Arc::new(Control::default());
            let options = BindOptions {
                host: Some(control.clone()),
                ..base.clone()
            };
            interrupted(
                dive_model::bind_controlled(ir.clone(), &cfg, &options, control.clone())
                    .expect_err("controlled startup"),
            );
            assert!(control.checks.load(Ordering::Relaxed) >= 2);
            let checks = control.checks.load(Ordering::Relaxed);
            let retry =
                dive_model::bind(ir, &cfg, &options).expect("new uncontrolled runtime after stop");
            assert_eq!(control.checks.load(Ordering::Relaxed), checks);
            assert!(
                retry.output.iter().any(
                    |message| matches!(message,Message::Print(text) if text.contains("after"))
                )
            );
        }
    }
}

#[test]
fn property_and_threaded_action_expansion_do_not_relabel_interruption() {
    for base in [BindOptions::dive(), BindOptions::tlc()] {
        for property in [false, true] {
            let body = if property {
                r#"Dom(a) == IF PrintT("arm") /\ (\A i \in 1..4096 : i >= a) THEN {0} ELSE {}
Init == x=0
Next == x'=x
P == \A a \in Dom(0) : [](x=a)"#
            } else {
                r#"Dom(a) == IF PrintT("arm") /\ (\A i \in 1..4096 : i >= a) THEN {0} ELSE {}
Init == x=0
Next == \E a \in Dom(0) : x'=a"#
            };
            let config = if property {
                "INIT Init\nNEXT Next\nPROPERTY P\n"
            } else {
                "INIT Init\nNEXT Next\n"
            };
            let (ir, cfg) = inputs(body, config);
            let control = Arc::new(Control::default());
            let options = BindOptions {
                host: Some(control.clone()),
                ..base.clone()
            };
            interrupted(
                dive_model::bind_controlled(ir, &cfg, &options, control)
                    .expect_err("role or action interruption"),
            );
        }
    }
}

#[test]
fn ordinary_binding_and_genuine_swallowed_errors_are_unchanged() {
    for base in [BindOptions::dive(), BindOptions::tlc()] {
        let (ir, cfg) = inputs(
            "Unused == 1 \\div 0\nD == PrintT(\"before\")\nInit==x=0\nNext==x'=x",
            "INIT Init\nNEXT Next\n",
        );
        let plain = dive_model::bind(ir.clone(), &cfg, &base)
            .expect("TLC swallows unused evaluation error");
        let control = Arc::new(Control::default());
        let controlled = dive_model::bind_controlled(ir, &cfg, &base, control.clone())
            .expect("same ordinary startup");
        assert_eq!(plain.output, controlled.output);
        assert_eq!(plain.unsupported, controlled.unsupported);
        assert_eq!(plain.actions.len(), controlled.actions.len());
        // A returned model does not retain the old binding registration.
        let evaluator = controlled.evaluator.clone();
        std::thread::scope(|scope| {
            scope
                .spawn(move || {
                    let _guard = evaluator
                        .host_control(Some(control))
                        .expect("binding owner removed before returning model");
                })
                .join()
                .expect("registration thread");
        });
    }
}

#[test]
fn uncontrolled_binding_does_not_install_the_hosts_control_capability() {
    let (ir, cfg) = inputs(STARTUP, "INIT Init\nNEXT Next\n");
    let control = Arc::new(Control::default());
    let options = BindOptions {
        host: Some(control.clone()),
        ..BindOptions::tlc()
    };
    let model = dive_model::bind(ir, &cfg, &options).expect("ordinary complete startup");
    assert_eq!(control.checks.load(Ordering::Relaxed), 0);
    assert_eq!(model.output.len(), 2);
    assert!(matches!(&model.output[1],Message::Print(text) if text.contains("after")));
}

#[test]
fn earlier_configuration_failure_precedes_control_registration() {
    let (ir, cfg) = inputs(
        "CONSTANT K\nInit==x=0\nNext==x'=x",
        "INIT Init\nNEXT Next\n",
    );
    let ordinary =
        dive_model::bind(ir.clone(), &cfg, &BindOptions::tlc()).expect_err("configuration error");
    let control = Arc::new(Control::default());
    control.armed.store(true, Ordering::Relaxed);
    let controlled = dive_model::bind_controlled(ir, &cfg, &BindOptions::tlc(), control.clone())
        .expect_err("same configuration error");
    assert!(matches!(controlled,ControlledBindError::Binding(failure) if failure==ordinary));
    assert_eq!(control.checks.load(Ordering::Relaxed), 0);
}

#[test]
fn fresh_recipe_binding_keeps_typed_startup_and_predicate_only_scope() {
    let (ir, cfg) = inputs(STARTUP, "INIT Init\nNEXT Next\n");
    let recipe = dive_model::Recipe::new(ir, cfg, dive_eval::Options::tlc(), false);
    let control = Arc::new(Control::default());
    interrupted(
        recipe
            .bind_controlled(control.clone(), control)
            .expect_err("fresh recipe stop"),
    );
}

#[test]
fn fallible_recipe_retains_real_startup_depth_exhaustion_and_legacy_stays_unchanged() {
    let source = "---- MODULE ResourceBinding ----\nEXTENDS Naturals\nVARIABLE x\nRECURSIVE F(_)\nF(n) == IF n=0 THEN 1 ELSE F(n-1)\nD == F(32)\nInit == x=0\nNext == x'=x\n====\n";
    let analysis = dive_sema::analyze_source(
        "ResourceBinding.tla",
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("source IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    let mut options = dive_eval::Options::tlc();
    options.max_depth = 4;
    let direct = dive_eval::Evaluator::new(dive_eval::Model::new(ir.clone()), options)
        .expect("actual evaluator");
    let error = direct
        .precompute(ir.root_def("D").expect("source D"))
        .expect_err("actual intrinsic depth exhaustion");
    assert_eq!(error.class(), dive_eval::ErrorClass::ResourceLimit);
    let recipe = dive_model::Recipe::new(ir, config, options, false);
    let host = Arc::new(Control::default());
    assert!(
        recipe.bind(host.clone()).is_ok(),
        "legacy unused startup failure remains swallowed"
    );
    assert!(
        recipe.bind_controlled(host.clone(), host.clone()).is_ok(),
        "existing controlled legacy mapping unchanged"
    );
    let failure = recipe
        .bind_fallible(host, None)
        .expect_err("new fallible startup exception");
    assert!(
        matches!(failure,ControlledBindError::EvaluationFailure{error:actual,..} if actual.class()==error.class()&&actual.origin()==error.origin())
    );
}

#[test]
fn finite_replay_elides_only_unused_permutations_of_a_configured_set() {
    let (ir, cfg) = inputs(
        "CONSTANT Peers\nUnused == Permutations(Peers)\nKept == Permutations({1,2})\nInit == x=0\nNext == x'=x",
        "CONSTANT Peers={a,b,c}\nINIT Init\nNEXT Next\n",
    );
    let unused = ir.root_def("Unused").expect("unused");
    let kept = ir.root_def("Kept").expect("ordinary eager startup");
    let recipe = dive_model::Recipe::new(ir, cfg, dive_eval::Options::tlc(), false);
    let host = Arc::new(Control::default());
    let ordinary = recipe.bind(host.clone()).expect("ordinary eager recipe");
    assert!(ordinary.evaluator.precomputed(unused).is_some());
    let m = recipe
        .bind_fallible(host.clone(), None)
        .expect("finite private replay");
    assert!(m.evaluator.precomputed(unused).is_none());
    assert!(m.evaluator.precomputed(kept).is_some());
    let controlled = recipe
        .bind_fallible(host.clone(), Some(host))
        .expect("controlled private replay");
    assert!(controlled.evaluator.precomputed(unused).is_some());
}

#[test]
fn replay_retains_other_permutation_domains_and_operator_replacements() {
    for (assignment, replacement) in [
        ("{p_a,p_b}", ""),
        ("{p_a,q_b}", ""),
        ("{a,1}", ""),
        ("{{a},{b}}", ""),
        ("{a,b}", "CONSTANT Permutations <- Other"),
    ] {
        let (ir, cfg) = inputs(
            "CONSTANT Peers\nOther(p) == {}\nUnused == Permutations(Peers)\nInit == x=0\nNext == x'=x",
            &format!("CONSTANT Peers = {assignment}\n{replacement}\nINIT Init\nNEXT Next"),
        );
        let id = ir.root_def("Unused").expect("definition");
        let opts = BindOptions::tlc();
        let ordinary = dive_model::bind(ir.clone(), &cfg, &opts);
        let replay = dive_model::bind_for_trace(ir, &cfg, &opts);
        match (ordinary, replay) {
            (Ok(eager), Ok(current)) => {
                assert_eq!(
                    eager.evaluator.precomputed(id),
                    current.evaluator.precomputed(id),
                    "{assignment} {replacement}"
                );
                assert_eq!(eager.output, current.output);
            }
            (Err(eager), Err(current)) => assert_eq!(eager.error, current.error),
            _ => unreachable_failure(),
        }
    }
}
