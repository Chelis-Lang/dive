//! Public source-owned EOF/replay and noncertifying diagnostic controls.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "explicit paired API assertions"
)]
use dive_api::periodic_candidate::{self, Error, Origin};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};

fn inputs(property: &str, hook: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let source = format!(
        "---- MODULE Candidate ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == {property}\nHook == {hook}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Candidate.tla", &source, &dive_sema::Options::default());
    (
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        dive_config::parse(
            "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
        )
        .into_config()
        .expect("cfg"),
    )
}
fn options(width: usize) -> dive_check::CheckOptions {
    dive_check::CheckOptions {
        workers: width,
        chaos: Some(7),
        ..dive_check::CheckOptions::tlc(0, 0)
    }
}
fn requests(force: bool) -> dive_check::progress::Options {
    dive_check::progress::Options {
        interval: Duration::from_secs(3600),
        force,
    }
}
fn assert_invalid_word(
    model: &dive_model::Model,
    doc: &dive_trace::TraceDoc,
    cfg: &dive_config::ModelConfig,
) {
    let trace = dive_trace::Trace::decode(model, doc).expect("material mutant decode");
    assert!(matches!(
        dive_trace::validate(model, &trace, None).validation,
        dive_trace::Validation::Invalid(_)
    ));
    let bytes = serde_json::to_string(&doc.to_json()).expect("mutant JSON serialization");
    let parsed = dive_trace::TraceDoc::parse(&bytes).expect("mutant transport shape is parseable");
    assert!(matches!(
        dive_trace::validate_doc(model, &parsed, &cfg.model_values, None)
            .0
            .validation,
        dive_trace::Validation::Invalid(_)
    ));
}
#[test]
fn public_eof_candidates_are_freshly_rehomed_and_independently_replayed() {
    for width in [1, 3, 4, 8] {
        let (ir, cfg) = inputs("<>(x=2)", "TRUE");
        let report = periodic_candidate::check(
            ir.clone(),
            cfg.clone(),
            options(width),
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
            requests(false),
        )
        .expect("candidate admitted");
        let witness = report.witness().expect("actual EOF candidate");
        assert!(matches!(witness.origin(), Origin::Eof(_)));
        assert_eq!(
            witness.memory_validation().validation,
            dive_trace::Validation::Valid
        );
        assert_eq!(
            witness.json_validation().validation,
            dive_trace::Validation::Valid
        );
        assert_eq!(witness.document().properties, ["Live"]);
        assert!(!report.model_proof());
        let fresh = dive_liveness::periodic_snapshot::bind(ir, &cfg).expect("fresh");
        let mut wrong = witness.document().clone();
        wrong.states[0].insert("x".into(), serde_json::json!(99));
        assert_invalid_word(fresh.model(), &wrong, &cfg);
        let mut wrong = witness.document().clone();
        wrong.properties = vec!["Unknown".into()];
        assert_invalid_word(fresh.model(), &wrong, &cfg);
        let mut wrong = witness.document().clone();
        let mut unreachable = wrong.states[0].clone();
        unreachable.insert("x".into(), serde_json::json!(99));
        wrong.states.push(unreachable);
        wrong.labels.push(None);
        wrong.length = Some(wrong.states.len());
        wrong.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
        assert_invalid_word(fresh.model(), &wrong, &cfg);
        let mut wrong = witness.document().clone();
        wrong.ending = Some(dive_trace::Ending::Lasso {
            back_to: wrong.states.len(),
        });
        assert_invalid_word(fresh.model(), &wrong, &cfg);
        // The publicly inspected diagnostic model still cannot run ordinary
        // safety/temporal checking just because a concrete witness was validated.
        assert!(
            fresh
                .model()
                .unsupported
                .iter()
                .any(|u| u.feature == dive_model::PeriodicSnapshot::ORDINARY_REFUSAL)
        );
        let ordinary = dive_check::check(&fresh.model().ir, &cfg, &options(width));
        assert!(matches!(
            ordinary.verdict,
            dive_check::Verdict::Unsupported { .. }
        ));
    }
}

#[test]
fn actual_material_atoms_are_rehomed_into_fresh_interner() {
    for value in [
        "\"literal\"",
        "<<\"literal\",0>>",
        "[field |-> \"literal\"]",
        "{\"a\",\"b\"}",
    ] {
        let source = format!(
            "---- MODULE Material ----\nEXTENDS Integers\nVARIABLES x,s\nInit == x=0 /\\ s = {value}\nNext == x'=1 /\\ s'=s\nLive == <>(x=2)\nHook == TRUE\n====\n"
        );
        let analysis =
            dive_sema::analyze_source("Material.tla", &source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).unwrap_or_else(|e| {
            panic!(
                "material {value}: {e:?}; syntax={:?}; sema={:?}",
                analysis.syntax_diagnostics(),
                analysis.diagnostics()
            )
        }));
        let cfg = dive_config::parse(
            "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
        )
        .into_config()
        .expect("cfg");
        let report = periodic_candidate::check(
            ir,
            cfg,
            options(4),
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
            requests(false),
        )
        .expect("fresh material replay");
        let witness = report.witness().expect("word");
        assert_eq!(
            witness.memory_validation().validation,
            dive_trace::Validation::Valid
        );
        assert_eq!(
            witness.json_validation().validation,
            dive_trace::Validation::Valid
        );
    }
}
#[test]
fn absent_candidate_or_false_hook_is_never_holds() {
    for (property, hook, forced) in [("<>(x=0)", "TRUE", false), ("<>(x=2)", "FALSE", true)] {
        let (ir, cfg) = inputs(property, hook);
        let report = periodic_candidate::check(
            ir,
            cfg,
            options(4),
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
            requests(forced),
        )
        .expect("diagnostic");
        assert!(report.witness().is_none());
        assert!(!report.model_proof());
        if forced {
            assert!(report.run().lifecycle().safety_eof().is_none());
        }
    }
}
#[test]
fn changed_profile_refuses_without_execution() {
    let (ir, cfg) = inputs("<>(x=2)", "TRUE");
    let failure = periodic_candidate::check(
        ir,
        cfg,
        dive_check::CheckOptions::dive(0),
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        requests(true),
    )
    .err()
    .expect("unsupported profile");
    assert!(failure.run.is_none());
}

#[test]
fn effectful_startup_is_refused_before_any_run() {
    let source = "---- MODULE Effects ----\nEXTENDS Integers,TLC\nVARIABLE x\nUnused == PrintT(TRUE)\nInit == x=0\nNext == x'=1\nLive == <>(x=2)\nHook == TRUE\n====\n";
    let a = dive_sema::analyze_source("Effects.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("effect IR"));
    let (_, cfg) = inputs("<>(x=2)", "TRUE");
    let failure = periodic_candidate::check(
        ir,
        cfg,
        options(4),
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        requests(true),
    )
    .err()
    .expect("startup refusal");
    let Error::Binding(binding) = &failure.error else {
        panic!("source admission before run")
    };
    assert!(binding.output.is_empty());
    assert!(failure.run.is_none());
}
#[test]
fn capture_limits_retain_actual_incomplete_execution_without_witness() {
    let (ir, cfg) = inputs("<>(x=2)", "TRUE");
    let mut limits = phase_graph::Limits::default();
    limits.capture.states = 1;
    let report = periodic_candidate::check(
        ir,
        cfg,
        options(4),
        limits,
        periodic_lifecycle::Options::default(),
        requests(false),
    )
    .expect("actual bounded diagnostics");
    assert!(report.witness().is_none());
    assert!(!report.model_proof());
    assert!(report.run().lifecycle().backend().stop().is_some());
    assert!(report.run().lifecycle().safety_eof().is_none());
}

#[test]
fn original_configured_conjunction_and_model_atoms_survive_fresh_replay() {
    let source = "---- MODULE Origins ----\nEXTENDS Integers\nCONSTANT C\nVARIABLES x,s\nInit == x=0 /\\ s=C\nNext == x'=1 /\\ s'=s\nFull == <>(x=2) /\\ <>(x=3)\nHook == TRUE\n====\n";
    let a = dive_sema::analyze_source("Origins.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("model-atom IR"));
    let cfg = dive_config::parse("CONSTANT C = Atom\nINIT Init\nNEXT Next\nPROPERTY Full\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n").into_config().expect("cfg");
    let report = periodic_candidate::check(
        ir,
        cfg,
        options(4),
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        requests(false),
    )
    .expect("actual original claim");
    let witness = report.witness().expect("word");
    assert_eq!(witness.document().properties, ["Full"]);
    assert_eq!(
        witness.memory_validation().validation,
        dive_trace::Validation::Valid
    );
    assert_eq!(
        witness.json_validation().validation,
        dive_trace::Validation::Valid
    );
}
