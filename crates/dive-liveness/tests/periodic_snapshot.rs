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

fn assert_fresh_candidate(run: &periodic_lifecycle::Run, fresh: &dive_model::Model, width: usize) {
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
            dir.join(format!("replay-w{width}.json")),
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
        .join("../../conformance/probes/periodic-snapshot");
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
