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
        assert_eq!(f.output, [] as [dive_model::Message; 0]);
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
        assert_eq!(f.output, [] as [dive_model::Message; 0]);
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
        assert_eq!(f.output, [] as [dive_model::Message; 0]);
    }
    let text = format!("Huge == \"{}\"", "x".repeat(50_001));
    let f = periodic_snapshot::bind_optional(lowered(&text), &plain())
        .err()
        .expect("unchanged text bound");
    assert_eq!(f.output, [] as [dive_model::Message; 0]);
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
    assert_eq!(f.output, [] as [dive_model::Message; 0]);
    let mut malformed = serde_json::to_value(lowered("").as_ref()).expect("IR");
    malformed["defs"][0]["body"] = serde_json::json!(u32::MAX);
    let f = periodic_snapshot::bind_optional(
        Arc::new(serde_json::from_value(malformed).expect("edited")),
        &plain(),
    )
    .err()
    .expect("invalid expression");
    assert_eq!(f.output, [] as [dive_model::Message; 0]);
    assert!(f.error.message.contains("C001"));
    let cfg = dive_config::parse("CONSTANT C <- Replacement\nINIT Init\nNEXT Next\n")
        .into_config()
        .expect("cycle cfg");
    let f = periodic_snapshot::bind_optional(lowered("Replacement == C"), &cfg)
        .err()
        .expect("cycle");
    assert_eq!(f.output, [] as [dive_model::Message; 0]);
}
#[test]
fn retain_all_twenty_six_original_sources_at_the_actual_native_binding_stage() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/optional-periodic-hook/primary");
    let selection: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("selection.json")).expect("selection"),
    )
    .expect("selection JSON");
    let mut rows = Vec::new();
    for (i, row) in selection.as_array().expect("rows").iter().enumerate() {
        let dir = root.join(format!("case-{i:03}"));
        let parsed =
            dive_config::parse(&std::fs::read_to_string(dir.join("Snapshot.cfg")).expect("config"));
        let actual = if let Some(cfg) = parsed.into_config() {
            let analysis =
                dive_sema::analyze_file(&dir.join("Snapshot.tla"), &dive_sema::Options::default())
                    .expect("source");
            let ir = Arc::new(dive_ir::lower(&analysis).expect("lowered actual source"));
            match periodic_snapshot::bind_optional(ir, &cfg) {
                Ok(snapshot) => {
                    let body=snapshot.predicate().map(|predicate| {
                        match snapshot.model().eval(predicate,&dive_eval::St::empty()) {
                            Ok(value)=>serde_json::json!({"value":dive_value::print::to_tlc_string_lenient(&value,snapshot.model().evaluator.options().ctx)}),
                            Err(error)=>serde_json::json!({"error":error.to_string(),"kind":format!("{error:?}")}),
                        }
                    });
                    serde_json::json!({"status":"selected","configured":snapshot.configured_name(),"definition":snapshot.definition().map(|d|snapshot.model().ir.def(d).name.clone()),"body":body,"output":snapshot.model().output.iter().map(|m|format!("{m:?}")).collect::<Vec<_>>()})
                }
                Err(failure) => {
                    serde_json::json!({"status":"refused","kind":format!("{:?}",failure.error.kind),"error":failure.to_string(),"output":failure.output.iter().map(|m|format!("{m:?}")).collect::<Vec<_>>() })
                }
            }
        } else {
            serde_json::json!({"status":"configuration-refused"})
        };
        rows.push(serde_json::json!({"id":row["id"],"stage":"native guarded binding and optional empty-state body evaluation only","actual":actual}));
    }
    assert_eq!(rows.len(), 26);
    if let Some(path) = std::env::var_os("DIVE_OPTIONAL_PERIODIC_BINDING_CAPTURE") {
        std::fs::write(path,serde_json::to_string_pretty(&serde_json::json!({"rows":rows,"model_proof":false,"ordinary_schedule_certified":false,"stage_matches_java_service":false})).expect("capture")).expect("write capture");
    }
}
