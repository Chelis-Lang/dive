//! Actual configuration snapshot identities, callback-free prebind and paused use.
#![allow(clippy::expect_used, reason = "paired bounded snapshot controls")]
use std::sync::Arc;

use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use dive_model::{BindOptions, ErrorKind};

fn case(index: usize) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/probes/periodic-snapshot/models")
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
        assert_eq!(failure.output, [] as [dive_model::Message; 0]);
    }
    for index in [0, 12, 16] {
        let (ir, cfg) = case(index);
        let failure = periodic_snapshot::bind(ir, &cfg)
            .err()
            .expect("must refuse");
        assert!(matches!(failure.error.kind, ErrorKind::Unsupported { .. }));
        assert_eq!(failure.output, [] as [dive_model::Message; 0]);
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
        assert_eq!(run.configured_name(), "Hook");
        assert_eq!(run.definition(), snapshot.definition());
        assert_eq!(run.expression(), snapshot.predicate().expr);
        let run = run.lifecycle();
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
        assert_eq!(f.output, [] as [dive_model::Message; 0]);
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
        for node in edited["exprs"].as_array_mut().expect("nodes") {
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
        assert_eq!(snapshot.model().output, [] as [dive_model::Message; 0]);
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
        assert_eq!(f.output, [] as [dive_model::Message; 0]);
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
    assert_eq!(failure.output, [] as [dive_model::Message; 0]);
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
    assert_eq!(failure.output, [] as [dive_model::Message; 0]);
    assert!(failure.error.message.contains("C001"));
    let large = format!("Huge == \"{}\"", "x".repeat(50_001));
    let f = periodic_snapshot::bind(lowered(&source_with(&large)), &configured())
        .err()
        .expect("text bound");
    assert_eq!(f.output, [] as [dive_model::Message; 0]);
    assert!(f.error.message.contains("DIVE-L0058"));
    let mut cfg = configured();
    cfg.periodic.as_mut().expect("periodic").text = "x".repeat(50_001);
    assert_eq!(
        periodic_snapshot::bind(original, &cfg)
            .err()
            .expect("name bound")
            .output,
        [] as [dive_model::Message; 0]
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
        assert_eq!(f.output, [] as [dive_model::Message; 0]);
        assert!(f.error.message.contains("DIVE-L0058"));
    }
}
#[test]
fn substituted_constant_and_forged_recursive_cycles_refuse_before_startup() {
    let source = "---- MODULE Bounded ----\nEXTENDS Integers\nCONSTANT C\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nHook == TRUE\nReplacement == C\n====\n";
    let cfg =
        dive_config::parse("CONSTANT C <- Replacement\nINIT Init\nNEXT Next\n_PERIODIC Hook\n")
            .into_config()
            .expect("cycle config");
    let failure = periodic_snapshot::bind(lowered(source), &cfg)
        .err()
        .expect("configured cycle refused");
    assert_eq!(failure.output, [] as [dive_model::Message; 0]);
    assert!(failure.error.message.contains("DIVE-L0058"));
    let ir = lowered(&source_with("RECURSIVE Loop\nLoop == Loop"));
    let mut edited = serde_json::to_value(ir.as_ref()).expect("IR");
    for def in edited["defs"].as_array_mut().expect("defs") {
        def["recursive"] = serde_json::json!(false);
        def["depth"] = serde_json::json!(0);
    }
    let failure = periodic_snapshot::bind(
        Arc::new(serde_json::from_value(edited).expect("edited recursion")),
        &configured(),
    )
    .err()
    .expect("no metadata escape");
    assert_eq!(failure.output, [] as [dive_model::Message; 0]);
}

#[test]
fn exposed_snapshot_cannot_be_used_as_an_ordinary_checker_or_capture_model() {
    let (ir, cfg) = case(1);
    let snapshot = periodic_snapshot::bind(ir.clone(), &cfg).expect("snapshot");
    assert_eq!(
        snapshot.model().unsupported.last().expect("marker").feature,
        dive_model::PeriodicSnapshot::ORDINARY_REFUSAL
    );
    let options = dive_check::CheckOptions::tlc(0, 0);
    let result = dive_check::check(&ir, &cfg, &options);
    assert!(matches!(
        result.verdict,
        dive_check::Verdict::Unsupported { .. }
    ));
    assert!(result.stats.is_none());
    let captured = dive_check::capture::explore(
        snapshot.model(),
        &options,
        dive_check::capture::Limits::default(),
    );
    assert!(
        matches!(captured, dive_check::capture::Exploration::Stopped(stop)
        if matches!(stop.verdict, dive_check::Verdict::Unsupported { .. }))
    );
}

#[test]
fn configured_candidate_precedes_false_hook_and_replays_on_fresh_binding() {
    let source = "---- MODULE Bounded ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == <>(x=3)\nHook == FALSE\n====\n";
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    for width in [1, 3, 4, 8] {
        let snapshot = periodic_snapshot::bind(lowered(source), &cfg).expect("snapshot");
        let run = snapshot
            .execute(
                &dive_check::CheckOptions {
                    workers: width,
                    ..dive_check::CheckOptions::tlc(0, 0)
                },
                phase_graph::Limits::default(),
                periodic_lifecycle::Options {
                    force: true,
                    progress_interval: 0,
                    ..periodic_lifecycle::Options::default()
                },
            )
            .expect("run");
        assert!(run.lifecycle().hook_evaluations().is_empty());
        assert!(matches!(
            run.lifecycle().invocations()[0].outcome(),
            Ok(dive_liveness::periodic::Outcome::LivenessStopped(2116))
        ));
        let fresh = periodic_snapshot::bind(lowered(source), &cfg)
            .expect("fresh independently bound model");
        assert_fresh_candidate(run.lifecycle(), fresh.model(), width);
    }
}

#[test]
fn real_request_pause_retains_uncommitted_facts_and_existing_candidate_replay() {
    let source = "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x+1\nLive == <>(x=2)\nHook == FALSE\n====\n";
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    for width in [1, 3, 4, 8] {
        let snapshot = periodic_snapshot::bind(lowered(source), &cfg).expect("snapshot");
        let run = snapshot
            .execute_requests(
                &dive_check::CheckOptions {
                    workers: width,
                    chaos: Some(7),
                    ..dive_check::CheckOptions::tlc(0, 0)
                },
                phase_graph::Limits::default(),
                periodic_lifecycle::Options {
                    progress_interval: 0,
                    force: true,
                    ..periodic_lifecycle::Options::default()
                },
                dive_check::progress::Options {
                    interval: std::time::Duration::from_secs(1),
                    force: true,
                },
            )
            .expect("request path");
        let pause = run
            .lifecycle()
            .expansion_pauses()
            .first()
            .expect("actual initial request service");
        assert_eq!(pause.pool().0, width);
        assert!(pause.receipt().request().forced());
        assert_eq!(pause.receipt().claimed(), &[] as &[u64]);
        assert_eq!(pause.receipt().completed(), &[] as &[u64]);
        assert_eq!(pause.receipt().logical_stats().generated, 1);
        assert!(!pause.receipt().model_proof());
        assert_eq!(
            run.lifecycle()
                .request_options()
                .expect("actual policy")
                .interval,
            std::time::Duration::from_secs(1)
        );
        assert!(run.lifecycle().hook_evaluations().is_empty());
        let fresh = periodic_snapshot::bind(lowered(source), &cfg).expect("fresh binding");
        assert_fresh_candidate(run.lifecycle(), fresh.model(), width);
        record_requests(&format!("candidate-w{width}"), run.lifecycle());
    }
}

#[test]
fn repeated_timer_claim_pauses_preserve_complete_safety_graph_and_ordinary_refusal() {
    let source = "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..8\nNext == x' \\in 0..40\nLive == []<>(x>=0)\nHook == TRUE\n====\n";
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    for width in [1, 4, 8] {
        let options = dive_check::CheckOptions {
            workers: width,
            chaos: Some(3),
            ..dive_check::CheckOptions::tlc(0, 0)
        };
        let policy = periodic_lifecycle::Options {
            progress_interval: i32::MAX,
            ..periodic_lifecycle::Options::default()
        };
        let baseline = periodic_snapshot::bind(lowered(source), &cfg)
            .expect("baseline")
            .execute(&options, phase_graph::Limits::default(), policy)
            .expect("baseline execution");
        let snapshot = periodic_snapshot::bind(lowered(source), &cfg).expect("snapshot");
        let run = snapshot
            .execute_requests(
                &options,
                phase_graph::Limits::default(),
                policy,
                dive_check::progress::Options {
                    interval: std::time::Duration::from_millis(1),
                    force: true,
                },
            )
            .expect("request run");
        let actual = run
            .lifecycle()
            .backend()
            .complete()
            .expect("actual complete safety graph");
        let expected = baseline
            .lifecycle()
            .backend()
            .complete()
            .expect("baseline graph");
        assert_eq!(actual.capture.stats, expected.capture.stats);
        assert_eq!(actual.capture.graph.edges, expected.capture.graph.edges);
        assert_eq!(actual.capture.graph.states, expected.capture.graph.states);
        let mut last = 0;
        for pause in run.lifecycle().expansion_pauses() {
            assert_eq!(pause.pool().0, width);
            assert!(pause.receipt().request().sequence() > last);
            last = pause.receipt().request().sequence();
            assert!(pause.receipt().claimed().len() <= dive_check::capture::MAX_EXPANSION_UNITS);
            assert!(
                pause
                    .receipt()
                    .completed()
                    .iter()
                    .all(|id| pause.receipt().claimed().contains(id))
            );
            assert!(!pause.receipt().model_proof());
        }
        assert!(last > 0);
        assert_eq!(
            run.lifecycle()
                .safety_eof()
                .expect("real EOF")
                .left_on_queue,
            0
        );
        assert!(!run.lifecycle().model_proof());
        record_requests(&format!("complete-w{width}"), run.lifecycle());
        assert!(matches!(
            dive_check::capture::explore(
                snapshot.model(),
                &options,
                dive_check::capture::Limits::default()
            ),
            dive_check::capture::Exploration::Stopped(_)
        ));
    }
}

fn assert_completed_prefix_graph(store: dive_store::StoreMode, width: usize) {
    let source = "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..8\nNext == x' \\in 0..512\nInv == x>=0\nLive == []<>(x>=0)\nHook == TRUE\n====\n";
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n")
        .into_config().expect("config");
    let mut published = 0;
    let options = dive_check::CheckOptions {
        workers: width,
        store,
        chaos: Some(19),
        ..dive_check::CheckOptions::tlc(0, 0)
    };
    let policy = periodic_lifecycle::Options {
        progress_interval: i32::MAX,
        ..periodic_lifecycle::Options::default()
    };
    let snapshot = periodic_snapshot::bind(lowered(source), &cfg).expect("snapshot");
    let baseline = snapshot
        .execute(&options, phase_graph::Limits::default(), policy)
        .expect("whole waves");
    let run = snapshot
        .execute_requests(
            &options,
            phase_graph::Limits::default(),
            policy,
            dive_check::progress::Options {
                interval: std::time::Duration::from_millis(1),
                force: true,
            },
        )
        .expect("requests");
    let actual = run
        .lifecycle()
        .backend()
        .complete()
        .expect("complete safety graph");
    let expected = baseline
        .lifecycle()
        .backend()
        .complete()
        .expect("baseline graph");
    assert_eq!(actual.capture.stats, expected.capture.stats);
    assert_eq!(actual.capture.graph.states, expected.capture.graph.states);
    assert_eq!(actual.capture.graph.edges, expected.capture.graph.edges);
    assert_eq!(actual.capture.output, expected.capture.output);
    assert_eq!(actual.capture.warnings, expected.capture.warnings);
    for pause in run.lifecycle().expansion_pauses() {
        let receipt = pause.receipt();
        assert_eq!(pause.pool().0, width);
        assert_eq!(receipt.frontier_appended(), receipt.committed());
        for id in receipt.committed() {
            assert!(receipt.completed().contains(&id));
        }
        assert!(receipt.committed_stats().generated >= receipt.logical_stats().generated);
        assert!(receipt.committed_stats().distinct >= receipt.logical_stats().distinct);
        assert!(receipt.predicate_workers() <= 1);
        if !receipt.committed().is_empty() {
            published += 1;
        }
        assert!(!receipt.model_proof());
    }
    record_requests(&format!("published-{store:?}-w{width}"), run.lifecycle());
    // This bounded real-timer campaign must exercise publication, rather than
    // count only its forced empty request. Exact pause timing is not asserted.
    assert!(
        published > 0,
        "no real returned-prefix publication exercised"
    );
}

#[test]
fn completed_prefix_graph_exact_w1() {
    assert_completed_prefix_graph(dive_store::StoreMode::Exact, 1);
}

#[test]
fn completed_prefix_graph_exact_w3() {
    assert_completed_prefix_graph(dive_store::StoreMode::Exact, 3);
}

#[test]
fn completed_prefix_graph_exact_w4() {
    assert_completed_prefix_graph(dive_store::StoreMode::Exact, 4);
}

#[test]
fn completed_prefix_graph_exact_w8() {
    assert_completed_prefix_graph(dive_store::StoreMode::Exact, 8);
}

#[test]
fn completed_prefix_graph_fingerprint_w1() {
    assert_completed_prefix_graph(dive_store::StoreMode::Fingerprint, 1);
}

#[test]
fn completed_prefix_graph_fingerprint_w3() {
    assert_completed_prefix_graph(dive_store::StoreMode::Fingerprint, 3);
}

#[test]
fn completed_prefix_graph_fingerprint_w4() {
    assert_completed_prefix_graph(dive_store::StoreMode::Fingerprint, 4);
}

#[test]
fn completed_prefix_graph_fingerprint_w8() {
    assert_completed_prefix_graph(dive_store::StoreMode::Fingerprint, 8);
}

#[test]
#[allow(
    clippy::panic,
    clippy::too_many_lines,
    reason = "one complete paired disposition matrix keeps stopping evidence auditable"
)]
fn prefix_and_whole_wave_preserve_constraints_and_ordered_safety_stops() {
    for (label, next, extra, deadlock, completes) in [
        (
            "constraint",
            "x' \\in 0..64",
            "CONSTRAINT Keep\n",
            false,
            true,
        ),
        (
            "action-constraint",
            "x' \\in 0..64",
            "ACTION_CONSTRAINT Changes\n",
            false,
            true,
        ),
        (
            "invariant",
            "x' \\in 0..64",
            "INVARIANT Inv\n",
            false,
            false,
        ),
        (
            "action-property",
            "x' \\in 0..64",
            "PROPERTY ActionSafe\n",
            false,
            false,
        ),
        ("deadlock", "x<10 /\\ x'=x+1", "", true, false),
        (
            "predicate-error",
            "x' \\in 0..64",
            "INVARIANT Bad\n",
            false,
            false,
        ),
    ] {
        let source = format!(
            "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..8\nNext == {next}\nKeep == x<31\nChanges == x'<31\nInv == x<31\nActionSafe == [][x'<31]_x\nBad == IF x<31 THEN TRUE ELSE 1 \\div 0 = 0\nLive == []<>(x>=0)\nHook == TRUE\n====\n"
        );
        let cfg = dive_config::parse(&format!(
            "INIT Init\nNEXT Next\nPROPERTY Live\n{extra}_PERIODIC Hook\nCHECK_DEADLOCK {}\n",
            if deadlock { "TRUE" } else { "FALSE" }
        ))
        .into_config()
        .expect("config");
        for width in [1, 3, 4, 8] {
            let options = dive_check::CheckOptions {
                workers: width,
                chaos: Some(19),
                ..dive_check::CheckOptions::tlc(0, 0)
            };
            let policy = periodic_lifecycle::Options {
                progress_interval: i32::MAX,
                ..periodic_lifecycle::Options::default()
            };
            let baseline = periodic_snapshot::bind(lowered(&source), &cfg)
                .expect("baseline binding")
                .execute(&options, phase_graph::Limits::default(), policy)
                .expect("whole wave");
            let run = periodic_snapshot::bind(lowered(&source), &cfg)
                .expect("request binding")
                .execute_requests(
                    &options,
                    phase_graph::Limits::default(),
                    policy,
                    dive_check::progress::Options {
                        interval: std::time::Duration::from_millis(1),
                        force: true,
                    },
                )
                .expect("requests");
            if completes {
                let expected = baseline
                    .lifecycle()
                    .backend()
                    .complete()
                    .expect("baseline safety EOF");
                let actual = run
                    .lifecycle()
                    .backend()
                    .complete()
                    .expect("actual safety EOF");
                assert_eq!(actual.capture.stats, expected.capture.stats);
                assert_eq!(actual.capture.graph.states, expected.capture.graph.states);
                assert_eq!(actual.capture.graph.edges, expected.capture.graph.edges);
            } else {
                let expected = baseline
                    .lifecycle()
                    .backend()
                    .stop()
                    .expect("original stop");
                let actual = run.lifecycle().backend().stop().expect("actual stop");
                let (phase_graph::Error::Capture(expected), phase_graph::Error::Capture(actual)) =
                    (&expected.error, &actual.error)
                else {
                    panic!(
                        "expected actual safety stopping path, got {:?} / {:?}",
                        expected.error, actual.error
                    );
                };
                assert_eq!(actual.verdict, expected.verdict);
                assert_eq!(actual.stats, expected.stats);
                assert_eq!(
                    actual.counterexample.as_ref().map(|c| c
                        .states
                        .iter()
                        .map(|s| (&s.label, &s.action, &s.values, &s.text))
                        .collect::<Vec<_>>()),
                    expected.counterexample.as_ref().map(|c| c
                        .states
                        .iter()
                        .map(|s| (&s.label, &s.action, &s.values, &s.text))
                        .collect::<Vec<_>>())
                );
                assert!(run.lifecycle().safety_eof().is_none());
            }
            assert!(!run.lifecycle().model_proof());
            record_requests(&format!("safety-{label}-w{width}"), run.lifecycle());
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "retain paired valid/refused replay and publication evidence together"
)]
fn deciding_candidates_preserve_publication_stage_and_replay_independently() {
    for (duplicate_bound, guarded) in [(1024, false), (8192, false), (1024, true)] {
        let source = "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == \\E i \\in 1..BOUND : x'=x+1\nLive == <>(x=2)\nHook == FALSE\n====\n";
        let source = source.replace("BOUND", &duplicate_bound.to_string());
        let source = if guarded {
            source.replace(
                "Next == \\E i \\in 1..1024 : x'=x+1",
                "Next == x'=x+1 \\/ (x'=x+1 /\\ \\E i \\in 1..1024 : TRUE)",
            )
        } else {
            source
        };
        let cfg = dive_config::parse(
            "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
        )
        .into_config()
        .expect("cfg");
        for width in [1, 3, 4, 8] {
            let snapshot = periodic_snapshot::bind(lowered(&source), &cfg).expect("snapshot");
            let run = snapshot
                .execute_requests(
                    &dive_check::CheckOptions {
                        workers: width,
                        chaos: Some(7),
                        ..dive_check::CheckOptions::tlc(0, 0)
                    },
                    phase_graph::Limits::default(),
                    periodic_lifecycle::Options {
                        force: true,
                        progress_interval: 0,
                        ..periodic_lifecycle::Options::default()
                    },
                    dive_check::progress::Options {
                        interval: std::time::Duration::from_millis(1),
                        force: true,
                    },
                )
                .expect("run");
            let invocation = run
                .lifecycle()
                .invocations()
                .first()
                .expect("actual candidate invocation");
            assert!(invocation.publication().frontier_append_pending());
            assert_eq!(
                invocation.publication().prefix_predicate_workers(),
                invocation.publication().published_prefix().map(|_| 0)
            );
            assert_eq!(
                invocation.stats().generated,
                if guarded { 3 } else { duplicate_bound + 1 }
            );
            assert_eq!(invocation.stats().distinct, 2);
            assert_eq!(invocation.stats().left_on_queue, 1);
            assert!(run.lifecycle().hook_evaluations().is_empty());
            assert!(run.lifecycle().safety_eof().is_none());
            assert!(
                run.lifecycle()
                    .expansion_pauses()
                    .iter()
                    .all(|p| p.receipt().committed().is_empty())
            );
            let fresh =
                periodic_snapshot::bind(lowered(&source), &cfg).expect("fresh independent binding");
            if duplicate_bound == 1024 || guarded {
                assert_fresh_candidate_named(
                    run.lifecycle(),
                    fresh.model(),
                    width,
                    if guarded {
                        "prefix-guarded"
                    } else {
                        "prefix-within-budget"
                    },
                );
            } else {
                // The original 8192-element development observation exceeds the
                // independent validator's unchanged work bound. Never promote it.
                let check = run
                    .lifecycle()
                    .backend()
                    .checks()
                    .iter()
                    .find(|c| c.selection().candidate().is_some())
                    .expect("candidate");
                let (states, back_to) = concrete_word(run.lifecycle().backend(), check);
                let trace = dive_trace::Trace {
                    labels: vec![None; states.len()],
                    length: Some(states.len()),
                    states,
                    ending: dive_trace::Ending::Lasso { back_to },
                    properties: vec!["Live".into()],
                };
                let memory = dive_trace::validate(fresh.model(), &trace, None).validation;
                let document = trace.to_doc(fresh.model()).expect("transport doc");
                let json = dive_trace::validate_doc(fresh.model(), &document, &[], None)
                    .0
                    .validation;
                for validation in [&memory, &json] {
                    assert!(
                        matches!(validation, dive_trace::Validation::Unsupported(message) if message.contains("quantifier domain materialization work"))
                    );
                }
                if let Some(dir) = std::env::var_os("DIVE_PERIODIC_SNAPSHOT_CAPTURE") {
                    let dir = std::path::PathBuf::from(dir);
                    std::fs::create_dir_all(&dir).expect("refusal capture directory");
                    let row = serde_json::json!({"width":width,"source":source,"memory":format!("{memory:?}"),"json":format!("{json:?}"),"model_proof":false,"ordinary_schedule_certified":false});
                    std::fs::write(
                        dir.join(format!("prefix-validator-refused-w{width}.json")),
                        serde_json::to_vec_pretty(&row).expect("refusal JSON"),
                    )
                    .expect("refusal capture");
                }
            }
            record_requests(
                &format!("prefix-candidate-{duplicate_bound}-guarded{guarded}-w{width}"),
                run.lifecycle(),
            );
        }
    }
}

#[test]
fn failed_init_and_invalid_request_policy_cannot_create_quiescence() {
    for (invariant, interval) in [
        ("x>=0", std::time::Duration::ZERO),
        ("x<0", std::time::Duration::from_millis(1)),
    ] {
        let source = format!(
            "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nInv == {invariant}\nNext == UNCHANGED x\nLive == []<>(x=0)\nHook == TRUE\n====\n"
        );
        let cfg = dive_config::parse(
            "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
        )
        .into_config()
        .expect("cfg");
        let snapshot = periodic_snapshot::bind(lowered(&source), &cfg).expect("snapshot");
        let run = snapshot
            .execute_requests(
                &dive_check::CheckOptions::tlc(0, 0),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options::default(),
                dive_check::progress::Options {
                    interval,
                    force: true,
                },
            )
            .expect("diagnostic result");
        assert!(run.lifecycle().expansion_pauses().is_empty());
        assert!(run.lifecycle().backend().complete().is_none());
        assert!(run.lifecycle().safety_eof().is_none());
        assert!(!run.lifecycle().model_proof());
    }
}

fn assert_fresh_candidate(run: &periodic_lifecycle::Run, fresh: &dive_model::Model, width: usize) {
    assert_fresh_candidate_named(run, fresh, width, "replay");
}
fn assert_fresh_candidate_named(
    run: &periodic_lifecycle::Run,
    fresh: &dive_model::Model,
    width: usize,
    label: &str,
) {
    let check = run
        .backend()
        .checks()
        .iter()
        .find(|c| c.selection().candidate().is_some())
        .expect("actual candidate");
    let (states, back_to) = concrete_word(run.backend(), check);
    assert!(
        !dive_trace::temporal::evaluate_lasso(
            fresh,
            &fresh.implied_temporals[0].pred,
            &states,
            back_to
        )
        .expect("independent word")[0]
    );
    let trace = dive_trace::Trace {
        labels: vec![None; states.len()],
        length: Some(states.len()),
        states,
        ending: dive_trace::Ending::Lasso { back_to },
        properties: vec!["Live".into()],
    };
    assert_eq!(
        dive_trace::validate(fresh, &trace, None).validation,
        dive_trace::Validation::Valid
    );
    let document = trace.to_doc(fresh).expect("document");
    let decoded = dive_trace::TraceDoc::from_json(
        &serde_json::from_slice(&serde_json::to_vec(&document.to_json()).expect("serialize"))
            .expect("json"),
    )
    .expect("decode");
    assert_eq!(
        dive_trace::validate_doc(fresh, &decoded, &[], None)
            .0
            .validation,
        dive_trace::Validation::Valid
    );
    let mut outcomes = Vec::new();
    for mutation in 0..4 {
        let mut bad = decoded.clone();
        match mutation {
            0 => bad.properties = vec!["Forged".into()],
            1 => {
                bad.states[0] =
                    serde_json::from_value(serde_json::json!({"x":99})).expect("wrong Init");
            }
            2 => {
                // The actual first-prefix candidate stutters at x=0.
                // Merely closing at index zero is therefore still valid.
                bad.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
                assert_eq!(
                    dive_trace::validate_doc(fresh, &bad, &[], None)
                        .0
                        .validation,
                    dive_trace::Validation::Valid
                );
                // 0 -> 1 satisfies Next, while closing 1 -> 0 does not.
                bad.states.push(
                    serde_json::from_value(serde_json::json!({"x":1}))
                        .expect("valid forward successor"),
                );
                bad.labels.push(None);
                bad.length = Some(bad.states.len());
            }
            _ => {
                bad.ending = Some(dive_trace::Ending::Lasso {
                    back_to: usize::MAX,
                });
            }
        }
        let outcome = dive_trace::validate_doc(fresh, &bad, &[], None)
            .0
            .validation;
        assert_ne!(outcome, dive_trace::Validation::Valid);
        if mutation == 2 {
            assert!(format!("{outcome:?}").contains("Next"));
        }
        outcomes.push(format!("{outcome:?}"));
    }
    if let Some(dir) = std::env::var_os("DIVE_PERIODIC_SNAPSHOT_CAPTURE") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("capture dir");
        let row = serde_json::json!({"width":width,"producer_profile":"tlc","fresh_validator_profile":"tlc","memory":"Valid","json":"Valid","wrong_property":outcomes[0],"wrong_init":outcomes[1],"wrong_closing_next":outcomes[2],"malformed_ending":outcomes[3],"model_proof":false,"ordinary_schedule_certified":false});
        std::fs::write(
            dir.join(format!("{label}-w{width}.json")),
            serde_json::to_vec_pretty(&row).expect("json"),
        )
        .expect("capture replay");
    }
}

fn concrete_word(
    run: &dive_liveness::periodic_backend::Run,
    check: &dive_liveness::phase_execution::PartialCheck,
) -> (Vec<dive_eval::next::StateVals>, usize) {
    use std::collections::BTreeMap;
    let c = check.selection().candidate().expect("candidate");
    let ids = check.identities();
    let groups = run
        .stop()
        .map(|s| &s.groups)
        .or_else(|| run.complete().map(|s| &s.groups))
        .expect("actual groups");
    let representatives: BTreeMap<_, _> = groups[check.group()]
        .commits
        .iter()
        .map(|commit| {
            let original = commit
                .receipt
                .successors
                .iter()
                .find(|s| s.fingerprint == commit.fingerprint)
                .expect("actual source stutter");
            (commit.fingerprint, original.state.clone())
        })
        .collect();
    let mut prefix = Vec::new();
    for i in c.prefix_reverse.iter().rev() {
        let fp = ids[*i].fingerprint;
        if prefix.last() != Some(&fp) {
            prefix.push(fp);
        }
    }
    let anchor = ids[c.anchor].fingerprint;
    assert_eq!(prefix.last(), Some(&anchor));
    let back = prefix.len() - 1;
    let mut reverse = c.closure_reverse.clone();
    for i in c.cycle_stack.iter().rev() {
        let fp = ids[*i].fingerprint;
        if reverse.last() != Some(&fp) {
            reverse.push(fp);
        }
    }
    reverse.dedup();
    if reverse.last() == Some(&anchor) {
        reverse.pop();
    }
    prefix.extend(reverse.into_iter().rev());
    (
        prefix
            .into_iter()
            .map(|fp| representatives[&fp].clone())
            .collect(),
        back,
    )
}

#[test]
fn retain_every_primary_native_disposition_without_promoting_refusals() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/probes/periodic-snapshot");
    for (group, count) in [("", 26), ("precedence", 12)] {
        let directory = base.join(group);
        for index in 0..count {
            let model = directory.join("models").join(format!("case-{index:03}"));
            let analysis = dive_sema::analyze_file(
                &model.join("Snapshot.tla"),
                &dive_sema::Options::default(),
            )
            .expect("source");
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let parsed = dive_config::parse(
                &std::fs::read_to_string(model.join("Snapshot.cfg")).expect("cfg"),
            );
            let record = if let Some(cfg) = parsed.into_config() {
                match periodic_snapshot::bind(ir, &cfg) {
                    Ok(snapshot) => {
                        let value = snapshot
                            .model()
                            .eval(snapshot.predicate(), &dive_eval::St::empty());
                        let result = match value {
                            Ok(value) => {
                                serde_json::json!({"value":dive_value::print::to_tlc_string_lenient(&value,snapshot.model().evaluator.options().ctx)})
                            }
                            Err(error) => {
                                assert!(group.is_empty() && index == 9);
                                assert_eq!(error.ec().code(), 2179);
                                serde_json::json!({"evaluation_code":error.ec().code(),"text":error.to_string()})
                            }
                        };
                        assert!(
                            group.is_empty() && [1, 2, 3, 4, 8, 9, 10, 11, 22].contains(&index),
                            "unexpected admission {group}/{index}"
                        );
                        serde_json::json!({"status":"selected","configured":snapshot.configured_name(),"selected":snapshot.model().ir.def(snapshot.definition()).name,"definition":snapshot.definition().0,"expression":snapshot.predicate().expr.0,"result":result,"startup_output":snapshot.model().output.iter().map(|m|format!("{m:?}")).collect::<Vec<_>>()})
                    }
                    Err(failure) => {
                        let code = match failure.error.kind {
                            ErrorKind::Tlc(code) => Some(code.code()),
                            _ => None,
                        };
                        if group == "precedence" && index != 5 && index < 8 {
                            assert_eq!(
                                code,
                                Some([2222, 2231, 2232, 2229, 2229, 0, 2229, 2229][index])
                            );
                        }
                        assert_eq!(failure.output, [] as [dive_model::Message; 0]);
                        serde_json::json!({"status":"refused","binding_code":code,"text":failure.to_string(),"startup_output":[]})
                    }
                }
            } else {
                assert!(
                    group.is_empty() && index == 23,
                    "unexpected config rejection {group}/{index}"
                );
                serde_json::json!({"status":"config-rejected"})
            };
            if let Some(output) = std::env::var_os("DIVE_PERIODIC_SNAPSHOT_CAPTURE") {
                let dir = std::path::PathBuf::from(output);
                std::fs::create_dir_all(&dir).expect("capture");
                let name = if group.is_empty() {
                    "original"
                } else {
                    "precedence"
                };
                let row = serde_json::json!({"selection":name,"index":index,"outcome":record,"model_proof":false,"ordinary_schedule_certified":false});
                std::fs::write(
                    dir.join(format!("{name}-{index:03}.json")),
                    serde_json::to_vec_pretty(&row).expect("json"),
                )
                .expect("write");
            }
        }
    }
}

fn record_requests(name: &str, run: &periodic_lifecycle::Run) {
    if let Some(dir) = std::env::var_os("DIVE_PROGRESS_QUEUE_NATIVE_CAPTURE") {
        let path = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&path).expect("native capture directory");
        let stats = |s: dive_check::Stats| serde_json::json!({"generated":s.generated,"distinct":s.distinct,"queue":s.left_on_queue,"depth":s.depth,"initial_generated":s.initial_generated});
        let pauses: Vec<_> = run.expansion_pauses().iter().map(|pause| {
            let receipt = pause.receipt();
            serde_json::json!({"request":receipt.request().sequence(),"millis":receipt.request().millis(),
                "forced":receipt.request().forced(),"pool":pause.pool(),"submitted_start":receipt.submitted().start,
                "submitted_end":receipt.submitted().end,"claimed":receipt.claimed(),"completed":receipt.completed(),
                "failed":receipt.failed(),"expanded_generated":receipt.expanded_generated(),
                "logical_stats":stats(receipt.logical_stats()),"committed_start":receipt.committed().start,"committed_end":receipt.committed().end,"committed_stats":stats(receipt.committed_stats()),"frontier_appended_start":receipt.frontier_appended().start,"frontier_appended_end":receipt.frontier_appended().end,"predicate_workers":receipt.predicate_workers(),"phase_l_written":!receipt.committed().is_empty(),"model_proof":false})
        }).collect();
        let invocations: Vec<_> = run.invocations().iter().map(|i| serde_json::json!({"source":i.source(),"stats":stats(i.stats()),"publication":format!("{:?}",i.publication().publication()),"prefix":i.publication().published_prefix().map(|r|[r.start,r.end]),"frontier_append_pending":i.publication().frontier_append_pending(),"predicate_workers":i.publication().prefix_predicate_workers(),"outcome":format!("{:?}",i.outcome()),"model_proof":false})).collect();
        let record = serde_json::json!({"pauses":pauses,"invocations":invocations,"stop_stats":run.backend().stop().and_then(|s|s.stats).map(stats),"eof":run.safety_eof().map(stats),"complete":run.backend().complete().is_some(),
            "stop":run.backend().stop().map(|s| s.error.to_string()),"requests_cancelled":run.requests_cancelled(),"aborted_request":run.aborted_request().map(|r| r.sequence()),"model_proof":false,"ordinary_schedule_certified":false});
        std::fs::write(
            path.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&record).expect("capture JSON"),
        )
        .expect("native capture");
    }
}

fn request_final_word(
    run: &dive_liveness::periodic_backend::Run,
    check: &periodic_lifecycle::FinalCheck,
) -> (Vec<dive_eval::next::StateVals>, usize) {
    use std::collections::BTreeMap;
    let c = check
        .selection()
        .candidate
        .as_ref()
        .expect("actual final candidate");
    let groups = run
        .stop()
        .map(|s| &s.groups)
        .or_else(|| run.complete().map(|s| &s.groups))
        .expect("actual groups");
    let ids = groups[check.order().0]
        .nodes
        .iter()
        .map(|n| dive_liveness::tlc_witness::Identity {
            fingerprint: n.fingerprint,
            tableau: n.tableau,
        })
        .collect::<Vec<_>>();
    let representatives: BTreeMap<_, _> = groups[check.order().0]
        .commits
        .iter()
        .map(|commit| {
            let original = commit
                .receipt
                .successors
                .iter()
                .find(|s| s.fingerprint == commit.fingerprint)
                .expect("actual source stutter");
            (commit.fingerprint, original.state.clone())
        })
        .collect();
    let mut prefix = Vec::new();
    for i in c.prefix_reverse.iter().rev() {
        let fp = ids[*i].fingerprint;
        if prefix.last() != Some(&fp) {
            prefix.push(fp);
        }
    }
    let anchor = ids[c.anchor].fingerprint;
    assert_eq!(prefix.last(), Some(&anchor));
    let back = prefix.len() - 1;
    let mut reverse = c.closure_reverse.clone();
    for i in c.cycle_stack.iter().rev() {
        let fp = ids[*i].fingerprint;
        if reverse.last() != Some(&fp) {
            reverse.push(fp);
        }
    }
    reverse.dedup();
    if reverse.last() == Some(&anchor) {
        reverse.pop();
    }
    prefix.extend(reverse.into_iter().rev());
    (
        prefix
            .into_iter()
            .map(|fp| representatives[&fp].clone())
            .collect(),
        back,
    )
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "fresh EOF word/claim/closing controls at all widths and both replay profiles"
)]
fn actual_request_path_eof_candidates_replay_separately_from_timer_candidates() {
    use dive_trace::{Ending, Trace, Validation};
    let source = "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 2\nLive == <>(x=3)\nHook == TRUE\n====\n";
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    for width in [1, 3, 4, 8] {
        for replay in [
            dive_check::CheckOptions::tlc(0, 0),
            dive_check::CheckOptions::dive(0),
        ] {
            let snapshot = periodic_snapshot::bind(lowered(source), &cfg).expect("snapshot");
            let run = snapshot
                .execute_requests(
                    &dive_check::CheckOptions {
                        workers: width,
                        chaos: Some(7),
                        ..dive_check::CheckOptions::tlc(0, 0)
                    },
                    phase_graph::Limits::default(),
                    periodic_lifecycle::Options {
                        progress_interval: i32::MAX,
                        ..periodic_lifecycle::Options::default()
                    },
                    dive_check::progress::Options {
                        interval: std::time::Duration::from_secs(1),
                        force: true,
                    },
                )
                .expect("request execution");
            assert!(run.lifecycle().safety_eof().is_some());
            assert!(run.lifecycle().invocations().is_empty());
            let check = run
                .lifecycle()
                .final_checks()
                .iter()
                .find(|c| c.selection().candidate.is_some())
                .expect("actual final candidate");
            let (states, back_to) = request_final_word(run.lifecycle().backend(), check);
            let mut fresh_cfg = cfg.clone();
            fresh_cfg.periodic = None;
            let fresh = dive_model::bind(
                lowered(source),
                &fresh_cfg,
                &dive_model::BindOptions {
                    eval: replay.eval,
                    no_deadlock: replay.no_deadlock,
                    host: None,
                },
            )
            .expect("fresh validator binding");
            assert!(
                !dive_trace::temporal::evaluate_lasso(
                    &fresh,
                    &fresh.implied_temporals[0].pred,
                    &states,
                    back_to
                )
                .expect("independent word")[0]
            );
            let trace = Trace {
                labels: vec![None; states.len()],
                length: Some(states.len()),
                states,
                ending: Ending::Lasso { back_to },
                properties: vec!["Live".into()],
            };
            assert_eq!(
                dive_trace::validate(&fresh, &trace, None).validation,
                Validation::Valid
            );
            let document = trace.to_doc(&fresh).expect("doc");
            let decoded = dive_trace::TraceDoc::from_json(
                &serde_json::from_slice(
                    &serde_json::to_vec(&document.to_json()).expect("serialize"),
                )
                .expect("JSON"),
            )
            .expect("decode");
            assert_eq!(
                dive_trace::validate_doc(&fresh, &decoded, &[], None)
                    .0
                    .validation,
                Validation::Valid
            );
            let mut outcomes = Vec::new();
            for mutation in 0..4 {
                let mut bad = decoded.clone();
                match mutation {
                    0 => {
                        bad.states[0] = serde_json::from_value(serde_json::json!({"x":99}))
                            .expect("wrong Init");
                    }
                    1 => bad.properties = vec!["Forged".into()],
                    2 => bad.ending = Some(Ending::Lasso { back_to: 0 }),
                    _ => {
                        bad.ending = Some(Ending::Lasso {
                            back_to: usize::MAX,
                        });
                    }
                }
                let outcome = dive_trace::validate_doc(&fresh, &bad, &[], None)
                    .0
                    .validation;
                assert!(matches!(outcome, Validation::Invalid(_)), "{outcome:?}");
                if mutation == 2 {
                    assert!(format!("{outcome:?}").contains("Next"));
                }
                outcomes.push(format!("{outcome:?}"));
            }
            assert!(!run.lifecycle().model_proof());
            record_requests(
                &format!("eof-{:?}-w{width}", replay.profile),
                run.lifecycle(),
            );
            if let Some(dir) = std::env::var_os("DIVE_PROGRESS_QUEUE_NATIVE_CAPTURE") {
                let path = std::path::PathBuf::from(dir);
                std::fs::create_dir_all(&path).expect("capture dir");
                let row = serde_json::json!({"width":width,"validator_profile":format!("{:?}",replay.profile),"memory":"Valid","json":"Valid","wrong_init":outcomes[0],"wrong_property":outcomes[1],"wrong_closing_next":outcomes[2],"malformed_ending":outcomes[3],"model_proof":false,"ordinary_schedule_certified":false});
                std::fs::write(
                    path.join(format!("eof-replay-{:?}-w{width}.json", replay.profile)),
                    serde_json::to_vec_pretty(&row).expect("JSON"),
                )
                .expect("record");
            }
        }
    }
}

#[test]
#[allow(
    clippy::panic,
    reason = "unexpected failure variants are test failures"
)]
fn request_cancel_after_expansion_cut_preserves_earliest_ordered_error() {
    let source = "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..8\nNext == IF x=0 \\/ x=8 THEN x'=1 \\div 0 ELSE UNCHANGED x\nLive == []<>(x>=0)\nHook == TRUE\n====\n";
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    for width in [1, 3, 4, 8] {
        for chaos in [0, 3, 7, 19] {
            let options = dive_check::CheckOptions {
                workers: width,
                chaos: Some(chaos),
                ..dive_check::CheckOptions::tlc(0, 0)
            };
            let policy = periodic_lifecycle::Options {
                progress_interval: i32::MAX,
                ..periodic_lifecycle::Options::default()
            };
            let baseline = periodic_snapshot::bind(lowered(source), &cfg)
                .expect("baseline")
                .execute(&options, phase_graph::Limits::default(), policy)
                .expect("baseline run");
            let run = periodic_snapshot::bind(lowered(source), &cfg)
                .expect("snapshot")
                .execute_requests(
                    &options,
                    phase_graph::Limits::default(),
                    policy,
                    dive_check::progress::Options {
                        interval: std::time::Duration::from_millis(1),
                        force: true,
                    },
                )
                .expect("request run");
            let expected = baseline
                .lifecycle()
                .backend()
                .stop()
                .expect("real expansion error");
            let actual = run
                .lifecycle()
                .backend()
                .stop()
                .expect("real expansion error");
            let phase_graph::Error::Capture(expected) = &expected.error else {
                panic!("unexpected baseline error: {:?}", expected.error);
            };
            let phase_graph::Error::Capture(actual) = &actual.error else {
                panic!("unexpected request error: {:?}", actual.error);
            };
            assert_eq!(actual.verdict, expected.verdict);
            assert_eq!(actual.stats, expected.stats);
            assert_eq!(
                actual.counterexample.as_ref().map(|c| c
                    .states
                    .iter()
                    .map(|s| (&s.label, &s.action, &s.values, &s.text))
                    .collect::<Vec<_>>()),
                expected.counterexample.as_ref().map(|c| c
                    .states
                    .iter()
                    .map(|s| (&s.label, &s.action, &s.values, &s.text))
                    .collect::<Vec<_>>())
            );
            assert!(run.lifecycle().requests_cancelled());
            assert!(run.lifecycle().hook_evaluations().is_empty());
            assert!(run.lifecycle().invocations().is_empty());
            assert!(run.lifecycle().safety_eof().is_none());
            assert!(!run.lifecycle().model_proof());
            record_requests(&format!("cut-w{width}-chaos{chaos}"), run.lifecycle());
        }
    }
}

#[test]
fn request_metadata_consumes_the_existing_shared_quota_before_retention() {
    let source = "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=0)\nHook == TRUE\n====\n";
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    let options = dive_check::CheckOptions::tlc(0, 0);
    let policy = periodic_lifecycle::Options {
        progress_interval: i32::MAX,
        ..periodic_lifecycle::Options::default()
    };
    let mut refusal = None;
    for budget in 1..128 {
        let mut limits = phase_graph::Limits::default();
        limits.graph.steps = budget;
        let baseline = periodic_snapshot::bind(lowered(source), &cfg)
            .expect("baseline")
            .execute(&options, limits, policy)
            .expect("baseline run");
        let run = periodic_snapshot::bind(lowered(source), &cfg)
            .expect("snapshot")
            .execute_requests(
                &options,
                limits,
                policy,
                dive_check::progress::Options {
                    interval: std::time::Duration::from_secs(1),
                    force: true,
                },
            )
            .expect("request run");
        if baseline.lifecycle().backend().complete().is_some()
            && run.lifecycle().backend().stop().is_some()
        {
            assert!(matches!(
                run.lifecycle().backend().stop().expect("quota stop").error,
                phase_graph::Error::Limit(_)
            ));
            // The extra request charge can exhaust later final checking after
            // a genuine safety EOF. Preserve that fact; it is not model proof.
            assert!(run.lifecycle().backend().complete().is_none());
            assert!(!run.lifecycle().model_proof());
            record_requests(&format!("quota-{budget}"), run.lifecycle());
            refusal = Some(budget);
            break;
        }
    }
    assert!(
        refusal.is_some(),
        "request work must share the original cumulative graph budget"
    );
}
