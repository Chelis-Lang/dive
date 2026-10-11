//! Actual configuration snapshot identities, callback-free prebind and paused use.
#![allow(clippy::expect_used, reason = "paired bounded snapshot controls")]
use std::sync::Arc;

use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use dive_model::{BindOptions, ErrorKind};

fn case(index: usize) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/periodic-snapshot/models")
        .join(format!("case-{index:03}"));
    let analysis =
        dive_sema::analyze_file(&root.join("Snapshot.tla"), &dive_sema::Options::default())
            .expect("load primary source");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("primary lowered source"));
    let cfg = dive_config::parse(&std::fs::read_to_string(root.join("Snapshot.cfg")).expect("cfg"))
        .into_config()
        .expect("primary config");
    (ir, cfg)
}
#[test]
fn actual_preconstant_snapshot_retains_body_name_and_substitution_identity() {
    for (index, selected, value) in [
        (1, "Hook", "TRUE"),
        (2, "Hook", "FALSE"),
        (3, "Hook", "TRUE"),
        (4, "Hook", "TRUE"),
        (8, "Hook", "1"),
        (10, "Hook", "TRUE"),
        (11, "Replacement", "FALSE"),
        (22, "Hook", "TRUE"),
    ] {
        let (ir, cfg) = case(index);
        let snapshot = periodic_snapshot::bind(ir.clone(), &cfg).expect("pure selection");
        assert_eq!(snapshot.configured_name(), "Hook");
        assert_eq!(
            snapshot.model().ir.def(snapshot.definition()).name,
            selected
        );
        assert_eq!(
            snapshot.predicate().expr,
            ir.def(snapshot.definition()).body
        );
        assert_eq!(snapshot.predicate().location, snapshot.predicate().expr);
        let actual = snapshot
            .model()
            .eval(snapshot.predicate(), &dive_eval::St::empty())
            .expect("original selected body");
        assert_eq!(
            dive_value::print::to_tlc_string_lenient(
                &actual,
                snapshot.model().evaluator.options().ctx
            ),
            value
        );
        let ordinary =
            dive_model::bind(ir, &cfg, &BindOptions::tlc()).expect_err("ordinary still refused");
        assert!(ordinary.error.message.contains("_PERIODIC"));
    }
}
#[test]
fn missing_arity_and_actual_exception_boundaries_are_separate() {
    for (index, expected) in [(14, 2229), (15, 2228)] {
        let (ir, cfg) = case(index);
        let failure = periodic_snapshot::bind(ir, &cfg)
            .err()
            .expect("must refuse");
        assert!(matches!(failure.error.kind, ErrorKind::Tlc(code) if code.code()==expected));
        assert!(failure.output.is_empty());
    }
    for index in [0, 12, 16] {
        let (ir, cfg) = case(index);
        let failure = periodic_snapshot::bind(ir, &cfg)
            .err()
            .expect("must refuse");
        assert!(matches!(failure.error.kind, ErrorKind::Unsupported { .. }));
        assert!(failure.output.is_empty());
    }
}
#[test]
fn effects_and_unproved_state_or_instance_inputs_refuse_without_output() {
    for index in [5, 6, 7, 13, 17, 18, 19, 20, 21, 24, 25] {
        let (ir, cfg) = case(index);
        let failure = periodic_snapshot::bind(ir, &cfg)
            .err()
            .expect("must refuse");
        assert!(
            matches!(failure.error.kind, ErrorKind::Unsupported { .. }),
            "{index}: {}",
            failure.error
        );
        assert!(failure.output.is_empty(), "{index}: {:?}", failure.output);
    }
}
#[test]
fn configured_false_is_the_only_paused_expression_and_never_a_model_proof() {
    let source = "---- MODULE Configured ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=0)\nHook == FALSE\nOther == TRUE\n====\n";
    let analyzed =
        dive_sema::analyze_source("Configured.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analyzed).expect("IR"));
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    let snapshot = periodic_snapshot::bind(ir, &cfg).expect("snapshot");
    for workers in [1, 3, 4, 8] {
        let options = dive_check::CheckOptions {
            workers,
            ..dive_check::CheckOptions::tlc(0, 0)
        };
        let run = snapshot
            .execute(
                &options,
                phase_graph::Limits::default(),
                periodic_lifecycle::Options {
                    progress_interval: 0,
                    ratio_limit: 0.0,
                    ..periodic_lifecycle::Options::default()
                },
            )
            .expect("admitted");
        assert!(!run.model_proof());
        assert!(run.safety_eof().is_none());
        assert_eq!(run.hook_evaluations().len(), 1);
        assert_eq!(
            run.hook_evaluations()[0].expression(),
            snapshot.predicate().expr
        );
        assert_eq!(
            run.hook_evaluations()[0].result().as_ref().expect("value"),
            &true
        );
        assert!(!run.invocations()[0].resume_requested());
        assert_eq!(run.invocations()[0].workers(), workers);
    }
}

fn lowered(source: &str) -> Arc<dive_ir::Ir> {
    let a = dive_sema::analyze_source("Bounded.tla", source, &dive_sema::Options::default());
    Arc::new(dive_ir::lower(&a).expect("control IR"))
}
fn configured() -> dive_config::ModelConfig {
    dive_config::parse("INIT Init\nNEXT Next\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("config")
}
fn source_with(unused: &str) -> String {
    format!(
        "---- MODULE Bounded ----\nEXTENDS Integers,TLC\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nHook == TRUE\n{unused}\n====\n"
    )
}
#[test]
fn unsupported_pure_startup_work_refuses_before_binding_and_output() {
    for unused in [
        "Huge == SUBSET (1..1000000000)",
        "Huge == [1..1000000000 -> {0,1}]",
        "Huge == \\A i \\in 1..1000000000 : i=i",
        "Huge == (SUBSET (1..1000000000)) = (SUBSET (1..1000000000))",
        "Huge == [i \\in 1..1000000000 |-> i]",
        "Huge == LET Unused == SUBSET (1..1000000000) IN TRUE",
        "RECURSIVE Huge(_)\nHuge(v) == Huge(v)\nUnused == Huge(0)",
    ] {
        let ir = lowered(&source_with(unused));
        let f = periodic_snapshot::bind(ir, &configured())
            .err()
            .expect("before startup refusal");
        assert!(
            f.error.message.contains("DIVE-L0058"),
            "{unused}: {}",
            f.error
        );
        assert!(f.output.is_empty());
    }
}
#[test]
fn forged_constant_annotations_do_not_cause_structural_nonconstant_precomputation() {
    for unused in [
        "Danger == IF x=0 THEN SUBSET (1..1000000000) ELSE {}",
        "Danger == IF x'=0 THEN [1..1000000000 -> {0,1}] ELSE {}",
        "Danger == <>((SUBSET (1..1000000000)) = {})",
    ] {
        let original = lowered(&source_with(unused));
        let id = original
            .defs()
            .iter()
            .position(|d| d.name == "Danger")
            .expect("Danger");
        let mut edited = serde_json::to_value(original.as_ref()).expect("serialized");
        edited["defs"][id]["level"] = serde_json::json!("Constant");
        for node in edited["nodes"].as_array_mut().expect("nodes") {
            node["level"] = serde_json::json!("Constant");
        }
        let ir = Arc::new(serde_json::from_value(edited).expect("edited IR"));
        let snapshot = periodic_snapshot::bind(ir, &configured())
            .expect("structural nonconstant, never cached");
        assert!(
            !snapshot
                .model()
                .evaluator
                .is_value_override(dive_ir::DefId(u32::try_from(id).expect("ID")))
        );
        assert!(snapshot.model().output.is_empty());
    }
}
#[test]
fn replacement_erasing_a_nonconstant_witness_requires_startup_grammar() {
    let source = source_with(
        "StateHelper == x=0\nDanger == IF StateHelper THEN SUBSET (1..1000000000) ELSE {}\nReplacement == TRUE",
    );
    for replacement in [
        "CONSTANT StateHelper = TRUE\n",
        "CONSTANT StateHelper <- Replacement\n",
    ] {
        let cfg = dive_config::parse(&format!(
            "INIT Init\nNEXT Next\n_PERIODIC Hook\n{replacement}"
        ))
        .into_config()
        .expect("config");
        let f = periodic_snapshot::bind(lowered(&source), &cfg)
            .err()
            .expect("erased witness refused");
        assert!(f.error.message.contains("DIVE-L0058"));
        assert!(f.output.is_empty());
    }
}
#[test]
fn local_alias_hidden_effect_is_rooted_before_substitution_precompute() {
    let source = "---- MODULE Bounded ----\nEXTENDS Integers,TLC\nCONSTANT Gate\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nHook == TRUE\nContainer(v) == LET Hidden == PrintT(\"HIDDEN_MARKER\") IN v\n====\n";
    let original = lowered(source);
    let hidden = original
        .defs()
        .iter()
        .position(|d| d.name == "Hidden")
        .expect("Hidden");
    let mut edit = serde_json::to_value(original.as_ref()).expect("IR");
    let root = original.root().index();
    edit["modules"][root]["standard"] = serde_json::json!(true);
    edit["modules"][root]["context"]
        .as_array_mut()
        .expect("context")
        .push(serde_json::json!(["Alias",{"Def":hidden}]));
    let ir = Arc::new(serde_json::from_value(edit).expect("valid alias IR"));
    dive_eval::validate_ir(&ir).expect("structurally valid actual alias");
    let cfg = dive_config::parse("CONSTANT Gate <- Alias\nINIT Init\nNEXT Next\n_PERIODIC Hook\n")
        .into_config()
        .expect("cfg");
    let failure = periodic_snapshot::bind(ir, &cfg)
        .err()
        .expect("before alias precompute");
    assert!(failure.output.is_empty());
    assert!(failure.error.message.contains("DIVE-L0058"));
}
#[test]
fn malformed_and_exhausted_public_inputs_refuse_without_startup() {
    let original = lowered(&source_with(""));
    let mut malformed = serde_json::to_value(original.as_ref()).expect("IR");
    malformed["defs"][0]["body"] = serde_json::json!(u32::MAX);
    let failure = periodic_snapshot::bind(
        Arc::new(serde_json::from_value(malformed).expect("edited")),
        &configured(),
    )
    .err()
    .expect("invalid expression");
    assert!(failure.output.is_empty());
    assert!(failure.error.message.contains("C001"));
    let large = format!("Huge == \"{}\"", "x".repeat(50_001));
    let f = periodic_snapshot::bind(lowered(&source_with(&large)), &configured())
        .err()
        .expect("text bound");
    assert!(f.output.is_empty());
    assert!(f.error.message.contains("DIVE-L0058"));
    let mut cfg = configured();
    cfg.periodic.as_mut().expect("periodic").text = "x".repeat(50_001);
    assert!(
        periodic_snapshot::bind(original, &cfg)
            .err()
            .expect("name bound")
            .output
            .is_empty()
    );
    for size in [40, 50_001] {
        let source = "---- MODULE Bounded ----\nEXTENDS Integers\nCONSTANT C\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nHook == TRUE\n====\n";
        let cfg = dive_config::parse(&format!(
            "CONSTANT C={{{}}}\nINIT Init\nNEXT Next\n_PERIODIC Hook\n",
            vec!["0"; size].join(",")
        ))
        .into_config()
        .expect("large config");
        let f = periodic_snapshot::bind(lowered(source), &cfg)
            .err()
            .expect("configuration reservation");
        assert!(f.output.is_empty());
        assert!(f.error.message.contains("DIVE-L0058"));
    }
}
