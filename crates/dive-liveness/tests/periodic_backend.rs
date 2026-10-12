//! Actual source-commit partial checks and fresh independent witness controls.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "adversarial test assertions"
)]
use dive_check::CheckOptions;
use dive_liveness::{
    periodic_backend::{self, Decision, Policy, Schedule},
    phase_execution,
    phase_graph::Limits,
};
use std::sync::Arc;
fn model(source: &str, config: &str, opts: &CheckOptions) -> dive_model::Model {
    bind(source, config, opts).expect("binding")
}
fn bind(
    source: &str,
    config: &str,
    opts: &CheckOptions,
) -> Result<dive_model::Model, Box<dive_model::Failure>> {
    let name = source.split_whitespace().nth(2).expect("module");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(config)
        .into_config()
        .expect("configuration");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            no_deadlock: opts.no_deadlock,
            host: None,
        },
    )
}

fn stats_json(s: dive_check::Stats) -> serde_json::Value {
    serde_json::json!({"generated":s.generated,"distinct":s.distinct,"queue":s.left_on_queue,
        "depth":s.depth,"initial_generated":s.initial_generated})
}
fn field<'a>(row: &'a serde_json::Value, key: &str) -> &'a str {
    row["fields"][key].as_str().expect(key)
}
#[test]
#[allow(
    clippy::too_many_lines,
    clippy::case_sensitive_file_extension_comparisons,
    reason = "exact case-sensitive fixture field names and complete stop/snapshot/pool obligations"
)]
fn actual_growth_snapshots_and_stops_match_all_clean_primary_rows() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/probes/periodic-backend");
    let capture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("capture-growth.json")).expect("capture"))
            .expect("JSON");
    let rows = capture["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 36);
    let mut compared = 0;
    for row in &rows[..32] {
        let name = row["name"].as_str().expect("name");
        let model_name = name.split('-').next().expect("model");
        let dir = base.join("models").join(model_name);
        let source =
            std::fs::read_to_string(dir.join(format!("{model_name}.tla"))).expect("source");
        let config = std::fs::read_to_string(dir.join(format!("{model_name}.cfg"))).expect("cfg");
        assert_eq!(row["exit"], 0);
        for workers in [1, 3, 4, 8] {
            let opts = CheckOptions {
                workers,
                chaos: Some(7),
                ..CheckOptions::tlc(0, 0)
            };
            let m = model(&source, &config, &opts);
            let run = periodic_backend::execute(
                &m,
                &opts,
                Limits::default(),
                Policy {
                    force: name.ends_with("forced"),
                    schedule: if name.contains("-final-") {
                        Schedule::Final
                    } else {
                        Schedule::Default
                    },
                    ..Policy::default()
                },
            );
            assert!(!run.model_proof());
            let expected: Vec<_> = row["fields"]
                .as_object()
                .expect("fields")
                .keys()
                .filter(|k| k.starts_with("check.") && k.ends_with(".code"))
                .collect();
            assert_eq!(
                run.checkpoints().len(),
                expected.len(),
                "{name}:{workers}: {run:?}"
            );
            for checkpoint in run.checkpoints() {
                let key = if checkpoint.sources() == 0 {
                    "check.initial".into()
                } else {
                    format!("check.source.{}", checkpoint.sources() - 1)
                };
                assert_eq!(
                    checkpoint.growth_due().to_string(),
                    field(row, &format!("{key}.before.due")),
                    "{name}:{key}"
                );
                for (g, (before, after)) in checkpoint
                    .before()
                    .iter()
                    .zip(checkpoint.after())
                    .enumerate()
                {
                    for (part, values) in [("before", before), ("after", after)] {
                        assert_eq!(
                            values.size().to_string(),
                            field(row, &format!("{key}.{part}.g{g}.size")),
                            "{name}:{key}:{g}:size"
                        );
                        assert_eq!(
                            values.last_check_size().to_string(),
                            field(row, &format!("{key}.{part}.g{g}.last")),
                            "{name}:{key}:{g}:last"
                        );
                    }
                }
                let after_due = checkpoint.after().iter().any(|g| g.due(0.1));
                assert_eq!(
                    after_due.to_string(),
                    field(row, &format!("{key}.after.due")),
                    "{name}:{key}: after growth"
                );
                let candidate = checkpoint.decision() == Decision::Candidate;
                assert_eq!(
                    candidate,
                    field(row, &format!("{key}.code")) == "2116",
                    "{name}:{key}"
                );
                if let Some(barrier) = checkpoint.barrier() {
                    assert!(checkpoint.sources() > 0);
                    assert_eq!(barrier.workers(), workers);
                    let last = run
                        .stop()
                        .and_then(|s| s.last_commit_work)
                        .or_else(|| run.complete().and_then(|c| c.last_commit_work));
                    let source = checkpoint.sources() as u64 - 1;
                    assert!(barrier.first_source() <= source && source < barrier.end_source());
                    assert!(barrier.end_source() - barrier.first_source() <= 8);
                    assert!(last.is_some());
                } else {
                    assert_eq!(checkpoint.sources(), 0);
                }
                assert!(!checkpoint.model_proof());
            }
            let final_checkpoint = run.checkpoints().last().expect("initial checkpoint");
            if final_checkpoint.decision() == Decision::Candidate {
                assert!(run.complete().is_none());
                let stop = run.stop().expect("actual stopped capture");
                assert!(matches!(
                    stop.error,
                    dive_liveness::phase_graph::Error::PartialCandidate
                ));
                assert_eq!(stop.workers.as_ref().expect("workers").used, workers);
                assert_eq!(stop.stats, Some(final_checkpoint.stats()));
                assert_eq!(stop.groups[0].commits.len(), final_checkpoint.sources());
                assert!(
                    run.checks()[final_checkpoint.candidate_check().expect("candidate index")]
                        .selection()
                        .candidate()
                        .is_some()
                );
            } else {
                assert!(run.stop().is_none(), "{name}: {run:?}");
                let complete = run.complete().expect("safety exhausted only");
                assert_eq!(complete.capture.workers.used, workers);
                assert!(!complete.model_proof());
            }
            assert!(run.inspections() <= run.graph_inspections());
            if let Ok(directory) = std::env::var("DIVE_PERIODIC_BACKEND_CAPTURE") {
                let checkpoints = run.checkpoints().iter().map(|c| serde_json::json!({
                    "sources":c.sources(), "model_proof":c.model_proof(),
                    "stats":stats_json(c.stats()), "growth_due":c.growth_due(),
                    "decision":format!("{:?}",c.decision()), "candidate_check":c.candidate_check(),
                    "before":c.before().iter().map(|g|serde_json::json!({"size":g.size(),"last":g.last_check_size()})).collect::<Vec<_>>(),
                    "after":c.after().iter().map(|g|serde_json::json!({"size":g.size(),"last":g.last_check_size()})).collect::<Vec<_>>(),
                    "barrier":c.barrier().map(|b|serde_json::json!({"workers":b.workers(),"first":b.first_source(),"end":b.end_source()})),
                })).collect::<Vec<_>>();
                let record = serde_json::json!({"name":name,"workers":workers,"model_proof":run.model_proof(),
                    "complete_safety":run.complete().is_some(),"checkpoints":checkpoints,
                    "inspections":run.inspections(),"graph_inspections":run.graph_inspections(),
                    "stop":run.stop().map(|f|serde_json::json!({"code":f.error.code(),"error":f.error.to_string(),"stats":f.stats.map(stats_json),"workers":f.workers.as_ref().map(|w|serde_json::json!({"requested":w.requested,"used":w.used,"sequential":w.sequential,"reason":w.reason}))})),
                });
                let path = std::path::Path::new(&directory).join(format!("{name}-w{workers}.json"));
                std::fs::write(
                    path,
                    serde_json::to_vec_pretty(&record).expect("capture JSON"),
                )
                .expect("closed diagnostic capture");
            }
            compared += 1;
        }
    }
    assert_eq!(compared, 128);
    // Controlled real Java IO failures are retained, never promoted to matches.
    for row in &rows[32..] {
        assert_eq!(row["exit"], 1);
        assert_eq!(
            field(row, "shutdown.g0.last"),
            field(row, "check.source.0.before.g0.last")
        );
    }
}

#[test]
fn actual_complete_initialization_and_one_shared_budget_are_required() {
    let opts = CheckOptions::tlc(0, 0);
    let src = "---- MODULE Safe ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x<2 /\\ x'=x+1\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nLive == <>(x=2)\n====\n";
    let cfg = "SPECIFICATION Spec\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n";
    let m = model(src, cfg, &opts);
    let full = periodic_backend::execute(&m, &opts, Limits::default(), Policy::default());
    assert!(full.complete().is_some(), "{full:?}");
    assert!(
        full.checkpoints()
            .iter()
            .all(|c| c.decision() != Decision::Candidate)
    );
    for steps in [0, full.inspections(), full.graph_inspections() - 1] {
        let run = periodic_backend::execute(
            &m,
            &opts,
            Limits {
                graph: dive_liveness::product::Limits {
                    steps,
                    ..Default::default()
                },
                ..Default::default()
            },
            Policy::default(),
        );
        assert!(run.complete().is_none());
        assert!(matches!(
            run.stop().expect("bound").error,
            dive_liveness::phase_graph::Error::Limit(_)
        ));
        assert!(!run.model_proof());
        assert!(run.graph_inspections() <= steps || run.checkpoints().last().is_none());
    }
    let exact = periodic_backend::execute(
        &m,
        &opts,
        Limits {
            graph: dive_liveness::product::Limits {
                steps: full.graph_inspections(),
                ..Default::default()
            },
            ..Default::default()
        },
        Policy::default(),
    );
    assert!(exact.complete().is_some());
    assert_eq!(exact.graph_inspections(), full.graph_inspections());
    // Initial failure cannot produce an initialization-complete receipt.
    let bad = "---- MODULE BadInit ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nInv == x=1\nLive == []<>(x=1)\n====\n";
    let cfg = "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n";
    let bad = model(bad, cfg, &opts);
    let run = periodic_backend::execute(
        &bad,
        &opts,
        Limits::default(),
        Policy {
            force: true,
            ..Default::default()
        },
    );
    assert!(run.complete().is_none());
    assert!(run.checkpoints().is_empty());
    assert!(run.checks().is_empty());
}

fn concrete_word(
    run: &periodic_backend::Run,
    check: &phase_execution::PartialCheck,
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
fn source_stopping_candidates_require_fresh_word_memory_json_and_mutant_validation() {
    use dive_trace::{Ending, Trace, Validation};
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/probes/periodic-backend/models");
    let mut count = 0;
    for name in [
        "PartialRoots",
        "PartialChild",
        "PartialSelf",
        "PartialLater",
        "PartialEA",
        "PartialMultiple",
        "PartialRevisit",
        "PartialGrowthSafe",
    ] {
        let dir = base.join(name);
        let src = std::fs::read_to_string(dir.join(format!("{name}.tla"))).expect("source");
        let cfg = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("config");
        let opts = CheckOptions::tlc(0, 0);
        let m = model(&src, &cfg, &opts);
        let run = periodic_backend::execute(
            &m,
            &opts,
            Limits::default(),
            Policy {
                force: true,
                ..Default::default()
            },
        );
        let fresh = model(&src, &cfg, &opts);
        for check in run
            .checks()
            .iter()
            .filter(|c| c.selection().candidate().is_some())
        {
            let (states, back_to) = concrete_word(&run, check);
            let word = fresh
                .implied_temporals
                .iter()
                .map(|p| {
                    dive_trace::temporal::evaluate_lasso(&fresh, &p.pred, &states, back_to)
                        .expect("independent word")
                })
                .collect::<Vec<_>>();
            assert!(word.iter().any(|w| !w[0]), "{name}");
            let trace = Trace {
                labels: vec![None; states.len()],
                length: Some(states.len()),
                states,
                ending: Ending::Lasso { back_to },
                properties: vec!["Live".into()],
            };
            assert_eq!(
                dive_trace::validate(&fresh, &trace, None).validation,
                Validation::Valid,
                "{name}"
            );
            let doc = trace.to_doc(&fresh).expect("document");
            let json = serde_json::to_vec(&doc.to_json()).expect("JSON");
            let decoded =
                dive_trace::TraceDoc::from_json(&serde_json::from_slice(&json).expect("decode"))
                    .expect("document");
            assert_eq!(
                dive_trace::validate_doc(&fresh, &decoded, &[], None)
                    .0
                    .validation,
                Validation::Valid
            );
            let mut wrong = decoded.clone();
            wrong.properties = vec!["Forged".into()];
            assert_ne!(
                dive_trace::validate_doc(&fresh, &wrong, &[], None)
                    .0
                    .validation,
                Validation::Valid
            );
            let mut wrong = decoded.clone();
            wrong.states[0] = serde_json::from_value(serde_json::json!({"x":99})).expect("state");
            assert_ne!(
                dive_trace::validate_doc(&fresh, &wrong, &[], None)
                    .0
                    .validation,
                Validation::Valid
            );
            let mut wrong = decoded;
            wrong.ending = Some(Ending::Lasso {
                back_to: usize::MAX,
            });
            assert_ne!(
                dive_trace::validate_doc(&fresh, &wrong, &[], None)
                    .0
                    .validation,
                Validation::Valid
            );
            count += 1;
        }
        assert!(!run.model_proof());
    }
    assert_eq!(count, 7);
}
#[test]
fn ownership_effect_and_preexecution_bounds_never_gain_checkpoint_credit() {
    let opts = CheckOptions::tlc(0, 0);
    let src = "---- MODULE Guard ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=1)\n====\n";
    let cfg = "INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n";
    let mut m = model(src, cfg, &opts);
    let foreign = model(src, cfg, &opts);
    m.names = foreign.names.clone();
    let r = periodic_backend::execute(&m, &opts, Limits::default(), Policy::default());
    assert!(r.checkpoints().is_empty());
    assert!(r.checks().is_empty());
    assert_eq!(
        r.stop()
            .expect("ownership")
            .workers
            .as_ref()
            .expect("width")
            .used,
        0
    );
    let m = model(src, cfg, &opts);
    let opts = CheckOptions { workers: 0, ..opts };
    let r = periodic_backend::execute(&m, &opts, Limits::default(), Policy::default());
    assert!(r.checkpoints().is_empty());
    assert_eq!(
        r.stop()
            .expect("width")
            .workers
            .as_ref()
            .expect("width")
            .used,
        0
    );
    let opts = CheckOptions::tlc(0, 0);
    let src = "---- MODULE Effect ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(PrintT(TRUE))\n====\n";
    let m = model(src, cfg, &opts);
    let r = periodic_backend::execute(&m, &opts, Limits::default(), Policy::default());
    assert!(r.checkpoints().is_empty());
    assert!(r.checks().is_empty());
    assert_eq!(m.take_output(), Vec::<String>::new());
    assert!(!r.model_proof());
}
