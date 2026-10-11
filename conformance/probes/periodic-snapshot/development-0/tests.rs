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
        (13, "Hook", "TRUE"),
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
    for index in [5, 6, 7, 17, 18, 19, 20, 21, 22, 24, 25] {
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
