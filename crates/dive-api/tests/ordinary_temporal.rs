//! Ordinary temporal consumers retain exact boundary facts and fresh validation.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "meaningful outcome and error controls"
)]
use dive_api::temporal::{self, Outcome};
use dive_check::{CheckOptions, Verdict};
use dive_liveness::{analyze::Limits, periodic_backend::Schedule};
use dive_trace::Validation;
use std::sync::Arc;
fn input(source: &str, cfg: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let a = dive_sema::analyze_source("Ordinary.tla", source, &dive_sema::Options::default());
    assert!(
        a.accepted(),
        "{:?} {:?}",
        a.diagnostics(),
        a.syntax_diagnostics()
    );
    (
        Arc::new(dive_ir::lower(&a).expect("IR")),
        dive_config::parse(cfg).into_config().expect("config"),
    )
}
const COUNTER: &str = "---- MODULE Ordinary ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=(x+1)%5\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == []<>(x=4)\nQ == <>(x=4)\nInv == x<2\n====\n";
#[test]
fn validated_prefix_keeps_real_queue_width_counts_and_all_claims() {
    let (ir, cfg) = input(COUNTER, "INIT Init\nNEXT Next\nPROPERTY P Q\n");
    for base in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        for width in [1, 4] {
            for chaos in [None, Some(19)] {
                let opts = CheckOptions {
                    workers: width,
                    chaos,
                    ..base.clone()
                };
                let r = temporal::check_with_schedule_with_evidence(
                    &ir,
                    &cfg,
                    &opts,
                    Schedule::Default,
                    Limits::default(),
                )
                .expect("ordinary");
                let Outcome::PrefixViolation(v) = r.outcome() else {
                    panic!("expected prefix: {r:?}")
                };
                assert_eq!(v.word().target_property, "P");
                assert_eq!(v.document().properties, ["P", "Q"]);
                assert_eq!(v.validation().validation, Validation::Valid);
                let c = v.exploration();
                assert_eq!(c.level, 1);
                assert_eq!(
                    (
                        c.stats.generated,
                        c.stats.distinct,
                        c.stats.depth,
                        c.stats.left_on_queue
                    ),
                    (2, 2, 2, 1)
                );
                assert_eq!(c.workers.used, width);
                assert!(!c.workers.sequential);
                let fresh = dive_model::bind(
                    ir.clone(),
                    &cfg,
                    &dive_model::BindOptions {
                        eval: opts.eval,
                        no_deadlock: false,
                        host: None,
                    },
                )
                .expect("fresh model");
                assert_eq!(
                    dive_trace::validate_doc(&fresh, v.document(), &cfg.model_values)
                        .0
                        .validation,
                    Validation::Valid
                );
                let mut forged = v.document().clone();
                forged.properties.push("Inv".into());
                assert!(matches!(
                    dive_trace::validate_doc(&fresh, &forged, &cfg.model_values)
                        .0
                        .validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
}
#[test]
fn exhaustive_fair_holds_and_final_violation_are_separate_from_prefix() {
    for base in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        for width in [1, 4] {
            let opts = CheckOptions {
                workers: width,
                ..base.clone()
            };
            let (ir, cfg) = input(COUNTER, "SPECIFICATION Spec\nPROPERTY P\n");
            let r = temporal::check_with_schedule_with_evidence(
                &ir,
                &cfg,
                &opts,
                Schedule::Default,
                Limits::default(),
            )
            .expect("fair holds");
            let Outcome::Holds(c) = r.outcome() else {
                panic!("expected exhaustive Holds: {r:?}")
            };
            assert_eq!(
                (
                    c.stats.generated,
                    c.stats.distinct,
                    c.stats.depth,
                    c.stats.left_on_queue
                ),
                (6, 5, 5, 0)
            );
            assert_eq!(c.workers.used, width);
            let (ir, cfg) = input(COUNTER, "INIT Init\nNEXT Next\nPROPERTY P\n");
            let r = temporal::check_with_schedule_with_evidence(
                &ir,
                &cfg,
                &opts,
                Schedule::Final,
                Limits::default(),
            )
            .expect("final");
            let Outcome::Violation(v) = r.outcome() else {
                panic!("final must not return prefix: {r:?}")
            };
            assert_eq!(v.candidate().exploration.stats.distinct, 5);
            assert_eq!(v.candidate().exploration.stats.generated, 6);
            assert_eq!(v.validation().validation, Validation::Valid);
        }
    }
}
#[test]
fn incomplete_level_safety_wins_and_prior_validated_boundary_can_stop_later_safety() {
    let (ir, cfg) = input(COUNTER, "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY P\n");
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::dive(7)
    };
    let early = temporal::check_with_schedule_with_evidence(
        &ir,
        &cfg,
        &opts,
        Schedule::Default,
        Limits::default(),
    )
    .expect("early");
    assert!(matches!(early.outcome(), Outcome::PrefixViolation(_)));
    let final_run = temporal::check_with_schedule_with_evidence(
        &ir,
        &cfg,
        &opts,
        Schedule::Final,
        Limits::default(),
    )
    .expect("safety");
    assert!(
        matches!(final_run.outcome(),Outcome::Safety(s) if matches!(s.verdict,Verdict::InvariantViolated {..}))
    );
    let (ir, cfg) = input(
        &COUNTER.replace("Inv == x<2", "Inv == x<1"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY P\n",
    );
    let result = temporal::check_with_schedule_with_evidence(
        &ir,
        &cfg,
        &opts,
        Schedule::Default,
        Limits::default(),
    )
    .expect("same level safety");
    let Outcome::Safety(s) = result.outcome() else {
        panic!("failed level emitted temporal result")
    };
    assert!(matches!(s.verdict, Verdict::InvariantViolated { .. }));
    assert_eq!(s.stats.expect("actual stats").generated, 2);
    assert_eq!(s.workers.used, 4);
}
#[test]
fn no_temporal_claim_preserves_single_startup_binding_registers_and_output() {
    let source = "---- MODULE Ordinary ----\nEXTENDS Naturals,TLC\nCONSTANT C\nVARIABLE x\nBound == PrintT(\"once\") /\\ TLCSet(0,7)\nInit == x=TLCGet(0)\nNext == x'=x\nP == x=7\n====\n";
    let (ir, cfg) = input(
        source,
        "CONSTANT C <- Bound\nINIT Init\nNEXT Next\nPROPERTY P\n",
    );
    for base in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        let opts = CheckOptions { workers: 4, ..base };
        let plain = dive_check::check(&ir, &cfg, &opts);
        let run = temporal::check_with_schedule_with_evidence(
            &ir,
            &cfg,
            &opts,
            Schedule::Default,
            Limits::default(),
        )
        .expect("safety dispatch");
        let Outcome::Safety(actual) = run.outcome() else {
            panic!("state PROPERTY must use safety")
        };
        assert_eq!(actual.verdict, Verdict::Success);
        assert_eq!(actual.output, plain.output);
        assert_eq!(actual.output.len(), 1);
        assert_eq!(actual.stats, plain.stats);
        assert_eq!(actual.workers, plain.workers);
        assert_eq!(actual.workers.used, 1);
    }
}
#[test]
fn exact_atom_error_and_resource_bound_never_report_a_temporal_verdict() {
    let source = COUNTER.replace("P == []<>(x=4)", "P == []<>(1 \\div x = 1)");
    let (ir, cfg) = input(&source, "INIT Init\nNEXT Next\nPROPERTY P\n");
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 7)
    };
    let failure = temporal::check_with_schedule_with_evidence(
        &ir,
        &cfg,
        &opts,
        Schedule::Default,
        Limits::default(),
    )
    .expect_err("division by zero");
    assert!(
        matches!(failure.error(),temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Atom {error,..}) if error.ec()==dive_eval::Ec::DivisionByZero)
    );
    assert_eq!(failure.capture_completed(), Some(false));
    assert_eq!(failure.workers(4).expect("actual workers").used, 4);
    assert_eq!(
        failure
            .exploration()
            .expect("boundary facts")
            .stats
            .left_on_queue,
        1
    );
    let (ir, cfg) = input(COUNTER, "INIT Init\nNEXT Next\nPROPERTY P\n");
    let mut limits = Limits::default();
    limits.capture.states = 1;
    let r =
        temporal::check_with_schedule_with_evidence(&ir, &cfg, &opts, Schedule::Default, limits)
            .expect("resource safety stop");
    assert!(
        matches!(r.outcome(),Outcome::Safety(s) if matches!(s.verdict,Verdict::ResourceLimit {..}))
    );
}

#[test]
fn ordinary_request_selects_profile_schedule_and_reports_prefix_exhaustion_honestly() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ordinary-liveness/Ordinary.tla");
    for compat in [false, true] {
        let request = dive_api::CheckRequest {
            file: path.clone(),
            compat_tlc: compat,
            workers: Some(4),
            seed: Some(7),
            fp: Some(0),
            ..Default::default()
        };
        let report = dive_api::check_file(&request).expect("ordinary request");
        let json = report.to_json();
        assert_eq!(json["verdict"], "liveness-violation");
        assert_eq!(json["property"], "P");
        assert_eq!(json["options"]["workers"]["used"], 4);
        assert_eq!(
            json["options"]["profile"],
            if compat { "tlc" } else { "dive" }
        );
        assert_eq!(
            json["options"]["lncheck"],
            if compat { "final" } else { "deterministic" }
        );
        assert_eq!(json["execution"]["exhausted"], compat);
        assert_eq!(json["execution"]["capture_completed"], compat);
        assert_eq!(json["statistics"]["distinct"], if compat { 5 } else { 2 });
        assert_eq!(json["statistics"]["generated"], if compat { 6 } else { 2 });
        assert_eq!(json["statistics"]["left_on_queue"], u8::from(!compat));
        assert_eq!(json["counterexample"]["validation"], "valid");
        assert_eq!(json["schema_version"], 2);
        assert_eq!(report.exit_code(), if compat { 13 } else { 10 });
    }
}

#[test]
fn transformed_rng_and_invalid_ir_refuse_before_binding_or_callback() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let source = "---- MODULE Ordinary ----\nEXTENDS Naturals,TLC\nCONSTANT C\nVARIABLE x\nPick == RandomElement({0,1})\nASSUME PrintT(\"must not bind\")\nInit == x=C\nNext == x'=x\nView == x\nP == []<>(x=0)\n====\n";
    let (ir, cfg) = input(
        source,
        "CONSTANT C <- Pick\nINIT Init\nNEXT Next\nVIEW View\nPROPERTY P\n",
    );
    let opts = CheckOptions::tlc(0, 7);
    let calls = AtomicUsize::new(0);
    let dispatch = dive_check::check_with_temporal(&ir, &cfg, &opts, |_| {
        calls.fetch_add(1, Ordering::SeqCst);
    });
    let dive_check::CheckDispatch::Safety(result) = dispatch else {
        panic!("prebinding refusal invoked consumer")
    };
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(result.output, [] as [String; 0]);
    assert!(result.stats.is_none());
    assert_eq!(result.workers.used, 0);
    assert!(
        matches!(result.verdict,Verdict::Unsupported {feature} if feature=="randomness under VIEW is not supported")
    );
    let (ir, cfg) = input(COUNTER, "INIT Init\nNEXT Next\nPROPERTY P\n");
    let mut value = serde_json::to_value(ir.as_ref()).expect("IR JSON");
    value["vars"] = serde_json::json!([u32::MAX]);
    let invalid =
        Arc::new(serde_json::from_value::<dive_ir::Ir>(value).expect("malformed public IR"));
    let dispatch = dive_check::check_with_temporal(&invalid, &cfg, &opts, |_| {
        calls.fetch_add(1, Ordering::SeqCst);
    });
    let dive_check::CheckDispatch::Safety(result) = dispatch else {
        panic!("invalid IR invoked consumer")
    };
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(result.output.is_empty() && result.stats.is_none());
    assert_eq!(result.workers.used, 0);
}
#[test]
fn classifier_runs_before_tableau_and_keeps_tlc_refusal_codes() {
    let source = COUNTER.replace("P == []<>(x=4)", "P == [](x=0 => <><<Next>>_x)");
    let (ir, cfg) = input(&source, "INIT Init\nNEXT Next\nPROPERTY P\n");
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 7)
    };
    let failure = temporal::check_with_schedule_with_evidence(
        &ir,
        &cfg,
        &opts,
        Schedule::Final,
        Limits::default(),
    )
    .expect_err("2214");
    assert!(
        matches!(failure.error(),temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Classification(error)) if error.tlc_code()==Some(2214)),
        "{failure:?}"
    );
    assert_eq!(failure.capture_completed(), Some(false));
    assert_eq!(failure.workers(4).expect("pre-search").used, 0);
    let source = COUNTER.replace(
        "P == []<>(x=4)",
        "P == ~(\\A i \\in 1..32: (<>[](x=i)) \\/ (<>[](x=i+1)))",
    );
    let (ir, cfg) = input(&source, "INIT Init\nNEXT Next\nPROPERTY P\n");
    let failure = temporal::check_with_schedule_with_evidence(
        &ir,
        &cfg,
        &opts,
        Schedule::Final,
        Limits::default(),
    )
    .expect_err("2213");
    assert!(
        matches!(failure.error(),temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Classification(error)) if error.tlc_code()==Some(2213)),
        "{failure:?}"
    );
}

#[test]
fn independent_prefix_refusal_is_not_a_violation_and_late_wave_safety_wins() {
    let source = "---- MODULE Ordinary ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x=0 /\\ y=[n \\in {0}|->0]\nNext == x'=(x+1)%5 /\\ y'=y\nA == y' \\in [{0}->{0}] /\\ y' # y' /\\ UNCHANGED x\nSpec == Init /\\ [][Next]_<<x,y>> /\\ WF_<<x,y>>(A)\nP == <>FALSE\n====\n";
    let (ir, cfg) = input(
        source,
        "SPECIFICATION Spec\nPROPERTY P\nCHECK_DEADLOCK FALSE\n",
    );
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::dive(7)
    };
    let failure = temporal::check_with_schedule_with_evidence(
        &ir,
        &cfg,
        &opts,
        Schedule::Default,
        Limits::default(),
    )
    .expect_err("independent lazy cover refusal");
    assert!(
        matches!(failure.error(),temporal::Error::PrefixValidation {report,..} if matches!(report.validation,Validation::Unsupported(_))),
        "{failure:?}"
    );
    assert_eq!(failure.capture_completed(), Some(false));
    assert_eq!(failure.workers(4).expect("actual pool").used, 4);
    assert_eq!(
        failure
            .exploration()
            .expect("boundary stats")
            .stats
            .left_on_queue,
        1
    );
    let source = "---- MODULE Ordinary ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..11\nNext == x'=(x+12)%36\nInv == x<23\nP == <>FALSE\n====\n";
    let (ir, cfg) = input(source, "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY P\n");
    for width in [1, 4] {
        let options = CheckOptions {
            workers: width,
            ..opts.clone()
        };
        let result = temporal::check_with_schedule_with_evidence(
            &ir,
            &cfg,
            &options,
            Schedule::Default,
            Limits::default(),
        )
        .expect("last-wave safety");
        let Outcome::Safety(s) = result.outcome() else {
            panic!("incomplete wave exposed temporal candidate")
        };
        assert!(matches!(s.verdict, Verdict::InvariantViolated { .. }));
        assert_eq!(s.stats.expect("actual counters").generated, 24);
        assert_eq!(s.workers.used, width);
    }
}

#[test]
fn ordinary_error_reports_preserve_class_code_exit_and_actual_stage_work() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ordinary-liveness");
    for compat in [false, true] {
        for (name, code, verdict, default_exit) in [
            ("OrdinaryError", 2179, "evaluation-error", 13),
            ("OrdinaryClass", 2214, "model-error", 1),
        ] {
            let request = dive_api::CheckRequest {
                file: root.join(format!("{name}.tla")),
                compat_tlc: compat,
                workers: Some(4),
                seed: Some(7),
                fp: Some(0),
                ..Default::default()
            };
            let report = dive_api::check_file(&request).expect("ordinary error request");
            let json = report.to_json();
            assert_eq!(json["verdict"], verdict);
            assert_eq!(json["tlc"]["code"], code);
            assert_eq!(report.exit_code(), if compat { 255 } else { default_exit });
            assert!(json["counterexample"].is_null());
            if code == 2214 {
                assert_eq!(json["options"]["workers"]["used"], 0);
                assert!(json["statistics"].is_null());
            } else {
                assert_eq!(json["options"]["workers"]["used"], 4);
                assert_eq!(json["execution"]["capture_completed"], compat);
            }
        }
    }
}

#[test]
fn classifier_separates_safety_roles_but_attribution_keeps_original_claims() {
    let source = COUNTER.replace("P == []<>(x=4)", "P == [][x'=(x+1)%5]_x");
    let (ir, cfg) = input(&source, "INIT Init\nNEXT Next\nPROPERTY P Q\n");
    for base in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
        for width in [1, 4] {
            let opts = CheckOptions {
                workers: width,
                ..base.clone()
            };
            let run = temporal::check_with_schedule_with_evidence(
                &ir,
                &cfg,
                &opts,
                Schedule::Default,
                Limits::default(),
            )
            .expect("mixed safety/temporal properties");
            let Outcome::PrefixViolation(v) = run.outcome() else {
                panic!("expected Q prefix violation: {run:?}")
            };
            assert_eq!(v.word().target_property, "Q");
            assert_eq!(v.document().properties, ["Q"]);
            assert_eq!(v.validation().validation, Validation::Valid);
            assert_eq!(v.exploration().workers.used, width);
        }
    }
}
#[test]
fn no_temporal_state_and_action_property_violations_keep_exact_raw_results() {
    for claim in ["P == x=99", "P == [](x<1)", "P == [][x'<x]_x"] {
        let source = COUNTER.replace("P == []<>(x=4)", claim);
        let (ir, cfg) = input(&source, "INIT Init\nNEXT Next\nPROPERTY P\n");
        for base in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
            for width in [1, 4] {
                let opts = CheckOptions {
                    workers: width,
                    ..base.clone()
                };
                let expected = dive_check::check(&ir, &cfg, &opts);
                assert_ne!(expected.verdict, Verdict::Success);
                assert_eq!(expected.verdict.property(), Some("P"));
                let run = temporal::check_with_schedule_with_evidence(
                    &ir,
                    &cfg,
                    &opts,
                    Schedule::Default,
                    Limits::default(),
                )
                .expect("safety property dispatch");
                let Outcome::Safety(actual) = run.outcome() else {
                    panic!("safety PROPERTY was captured as liveness")
                };
                assert_eq!(actual.verdict, expected.verdict);
                assert_eq!(actual.verdict.tlc(), expected.verdict.tlc());
                assert_eq!(actual.stats, expected.stats);
                assert_eq!(actual.output, expected.output);
                assert_eq!(actual.workers, expected.workers);
                assert_eq!(
                    actual.counterexample.as_ref().map(|x| x.states.len()),
                    expected.counterexample.as_ref().map(|x| x.states.len())
                );
            }
        }
    }
}

#[test]
fn exhaustive_success_json_has_verified_tlc_success_and_real_pool_in_both_profiles() {
    let file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ordinary-liveness/OrdinaryFair.tla");
    for compat in [false, true] {
        for workers in [1, 4] {
            let request = dive_api::CheckRequest {
                file: file.clone(),
                compat_tlc: compat,
                workers: Some(workers),
                seed: Some(7),
                fp: Some(0),
                ..Default::default()
            };
            let report = dive_api::check_file(&request).expect("exhaustive ordinary success");
            let json = report.to_json();
            assert_eq!(json["verdict"], "success");
            assert_eq!(json["tlc"]["code"], 2193);
            assert_eq!(json["tlc"]["name"], "TLC_SUCCESS");
            assert_eq!(json["tlc"]["reported"], 2193);
            assert_eq!(json["tlc"]["exit_status"], 0);
            assert_eq!(json["tlc"]["verified"], true);
            assert_eq!(report.exit_code(), 0);
            assert_eq!(json["execution"]["exhausted"], true);
            assert_eq!(json["execution"]["capture_completed"], true);
            assert_eq!(json["statistics"]["left_on_queue"], 0);
            assert_eq!(json["statistics"]["generated"], 6);
            assert_eq!(json["statistics"]["distinct"], 5);
            assert_eq!(json["statistics"]["depth"], 5);
            assert_eq!(json["options"]["workers"]["used"], workers);
            assert_eq!(json["options"]["workers"]["sequential"], false);
            assert!(json["counterexample"].is_null());
        }
    }
}
