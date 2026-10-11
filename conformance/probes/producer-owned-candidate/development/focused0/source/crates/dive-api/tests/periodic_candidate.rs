//! Public source-owned EOF/replay and noncertifying diagnostic controls.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "explicit paired API assertions"
)]
use dive_api::periodic_candidate::{self, Origin};
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
        assert!(matches!(
            dive_trace::validate_doc(fresh.model(), &wrong, &cfg.model_values, None)
                .0
                .validation,
            dive_trace::Validation::Invalid(_)
        ));
        let mut wrong = witness.document().clone();
        wrong.properties = vec!["Unknown".into()];
        assert!(matches!(
            dive_trace::validate_doc(fresh.model(), &wrong, &cfg.model_values, None)
                .0
                .validation,
            dive_trace::Validation::Invalid(_)
        ));
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
            "---- MODULE Material ----\nEXTENDS Integers\nVARIABLES x,s\nInit == x=0 /\\ s={value}\nNext == x'=1 /\\ s'=s\nLive == <>(x=2)\nHook == TRUE\n====\n"
        );
        let analysis =
            dive_sema::analyze_source("Material.tla", &source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("material IR"));
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
fn opaque_startup_and_changed_profile_refuse_without_execution() {
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
