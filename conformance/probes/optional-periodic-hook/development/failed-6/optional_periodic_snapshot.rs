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
            exploration,
            dive_check::capture::Exploration::Stopped(stop)
                if matches!(stop.verdict, dive_check::Verdict::Unsupported { .. })
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
fn lowered(extra: &str) -> Arc<dive_ir::Ir> {
    let source = format!(
        "---- MODULE Guard ----\nEXTENDS Integers,TLC\nCONSTANT C\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\n{extra}\n====\n"
    );
    let a = dive_sema::analyze_source("Guard.tla", &source, &dive_sema::Options::default());
    Arc::new(dive_ir::lower(&a).expect("guard IR"))
}
fn plain() -> dive_config::ModelConfig {
    dive_config::parse("CONSTANT C=0\nINIT Init\nNEXT Next\n")
        .into_config()
        .expect("no role config")
}
#[test]
fn genuine_absence_preserves_prestartup_alias_cycle_shape_and_reservation_guards() {
    for extra in [
        "Huge == 1..1000000000",
        "Huge == \\A i \\in 1..1000000000 : i=i",
        "RECURSIVE Loop\nLoop == Loop",
    ] {
        let f = periodic_snapshot::bind_optional(lowered(extra), &plain())
            .err()
            .expect("unsupported startup");
        assert!(f.output.is_empty());
    }
    let text = format!("Huge == \"{}\"", "x".repeat(50_001));
    let f = periodic_snapshot::bind_optional(lowered(&text), &plain())
        .err()
        .expect("unchanged text bound");
    assert!(f.output.is_empty());
    let original = lowered("Container(v) == LET Hidden == PrintT(TRUE) IN v");
    let hidden = original
        .defs()
        .iter()
        .position(|d| d.name == "Hidden")
        .expect("Hidden");
    let mut edited = serde_json::to_value(original.as_ref()).expect("IR");
    let root = original.root().index();
    edited["modules"][root]["standard"] = serde_json::json!(true);
    edited["modules"][root]["context"]
        .as_array_mut()
        .expect("context")
        .push(serde_json::json!(["Alias",{"Def":hidden}]));
    let ir = Arc::new(serde_json::from_value(edited).expect("valid alias IR"));
    dive_eval::validate_ir(&ir).expect("structural valid alias");
    let cfg = dive_config::parse("CONSTANT C <- Alias\nINIT Init\nNEXT Next\n")
        .into_config()
        .expect("cfg");
    let f = periodic_snapshot::bind_optional(ir, &cfg)
        .err()
        .expect("hidden alias effect before startup");
    assert!(f.output.is_empty());
    let mut malformed = serde_json::to_value(lowered("").as_ref()).expect("IR");
    malformed["defs"][0]["body"] = serde_json::json!(u32::MAX);
    let f = periodic_snapshot::bind_optional(
        Arc::new(serde_json::from_value(malformed).expect("edited")),
        &plain(),
    )
    .err()
    .expect("invalid expression");
    assert!(f.output.is_empty());
    assert!(f.error.message.contains("C001"));
    let cfg = dive_config::parse("CONSTANT C <- Replacement\nINIT Init\nNEXT Next\n")
        .into_config()
        .expect("cycle cfg");
    let f = periodic_snapshot::bind_optional(lowered("Replacement == C"), &cfg)
        .err()
        .expect("cycle");
    assert!(f.output.is_empty());
}
