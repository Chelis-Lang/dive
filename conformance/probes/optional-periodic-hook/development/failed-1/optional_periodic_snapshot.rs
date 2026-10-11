//! Original optional role selection, including genuine absence and early refusal.
#![allow(clippy::expect_used, reason = "explicit bounded admission controls")]
use dive_liveness::periodic_snapshot;
use std::sync::Arc;
fn case(i: usize) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/optional-periodic-hook/primary")
        .join(format!("case-{i:03}"));
    let a = dive_sema::analyze_file(&root.join("Snapshot.tla"), &dive_sema::Options::default())
        .expect("source");
    (
        Arc::new(dive_ir::lower(&a).expect("IR")),
        dive_config::parse(&std::fs::read_to_string(root.join("Snapshot.cfg")).expect("cfg"))
            .into_config()
            .expect("valid config"),
    )
}
#[test]
fn absent_unused_definitions_never_create_a_role_and_mandatory_api_stays_required() {
    for i in [0, 1, 2, 17, 18, 19] {
        let (ir, cfg) = case(i);
        let s = periodic_snapshot::bind_optional(ir.clone(), &cfg).expect("actual absence");
        assert_eq!(s.configured_name(), None);
        assert_eq!(s.definition(), None);
        assert!(s.predicate().is_none());
        assert!(
            s.model()
                .unsupported
                .iter()
                .any(|u| u.feature == dive_model::PeriodicSnapshot::ORDINARY_REFUSAL)
        );
        assert!(periodic_snapshot::bind(ir, &cfg).is_err());
        let exploration = dive_check::capture::explore(
            s.model(),
            &dive_check::CheckOptions::tlc(0, 0),
            dive_check::capture::Limits::default(),
        );
        assert!(matches!(
            exploration.result.verdict,
            dive_check::Verdict::Unsupported { .. }
        ));
    }
}
#[test]
fn original_present_name_and_actual_replacement_definition_are_preserved() {
    for (i, def) in [
        (3, "Hook"),
        (4, "Hook"),
        (5, "Hook"),
        (9, "Hook"),
        (10, "Hook"),
        (15, "Replacement"),
        (20, "Hook"),
        (21, "Hook"),
        (22, "Hook"),
        (23, "Hook"),
        (25, "Hook"),
    ] {
        let (ir, cfg) = case(i);
        let s = periodic_snapshot::bind_optional(ir, &cfg).expect("actual role");
        assert_eq!(s.configured_name(), Some("Hook"));
        assert_eq!(s.model().ir.def(s.definition().expect("def")).name, def);
        assert_eq!(
            s.predicate().expect("body").expr,
            s.model().ir.def(s.definition().expect("def")).body
        );
    }
}
#[test]
fn errors_and_unproved_effects_keep_original_prebinding_dispositions() {
    for (i, code) in [(11, 2229), (12, 2228)] {
        let (ir, cfg) = case(i);
        let f = periodic_snapshot::bind_optional(ir, &cfg)
            .err()
            .expect("original binding error");
        assert!(matches!(f.error.kind,dive_model::ErrorKind::Tlc(ec) if ec.code()==code));
        assert!(f.output.is_empty());
    }
    for i in [6, 7, 8, 13, 16, 24] {
        let (ir, cfg) = case(i);
        let f = periodic_snapshot::bind_optional(ir, &cfg)
            .err()
            .expect("named unsupported");
        assert!(matches!(
            f.error.kind,
            dive_model::ErrorKind::Unsupported { .. }
        ));
        assert!(f.output.is_empty());
    }
    // Qualified configuration names remain rejected by configuration parsing,
    // not invented as runtime role errors.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/optional-periodic-hook/primary/case-014/Snapshot.cfg");
    assert!(
        dive_config::parse(&std::fs::read_to_string(root).expect("qualified cfg"))
            .into_config()
            .is_none()
    );
}
