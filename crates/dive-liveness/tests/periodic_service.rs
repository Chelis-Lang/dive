//! Genuine request service, empty-source provenance and stopping controls.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::panic,
    reason = "paired source-owned service controls with independent replay"
)]
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::{sync::Arc, time::Duration};

fn snapshot(init: &str, next: &str, hook: &str, live: &str) -> periodic_snapshot::Snapshot {
    let source = format!(
        "---- MODULE Service ----\nEXTENDS Integers\nVARIABLE x\nInit == {init}\nNext == {next}\nLive == {live}\nHook == {hook}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Service.tla", &source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("control IR"));
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("config");
    periodic_snapshot::bind(ir, &cfg).expect("pure actual configured snapshot")
}
fn options(width: usize) -> dive_check::CheckOptions {
    dive_check::CheckOptions {
        workers: width,
        chaos: Some(7),
        ..dive_check::CheckOptions::tlc(0, 0)
    }
}
fn requests() -> dive_check::progress::Options {
    dive_check::progress::Options {
        interval: Duration::from_secs(3600),
        force: true,
    }
}
#[test]
fn initial_empty_false_service_has_no_invented_source_and_no_elapsed_suppression() {
    for width in [1, 3, 4, 8] {
        let snapshot = snapshot("x=0", "UNCHANGED x", "FALSE", "[]<>(x=0)");
        let result = snapshot
            .execute_service(
                &options(width),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options {
                    progress_interval: i32::MAX,
                    ..periodic_lifecycle::Options::default()
                },
                requests(),
            )
            .expect("admitted");
        let run = result.lifecycle();
        record_service(&format!("empty-false-w{width}"), run);
        assert!(run.invocations().is_empty());
        assert!(run.hook_evaluations().is_empty());
        assert_eq!(run.request_services().len(), 1);
        assert_eq!(run.service_evaluations().len(), 1);
        let service = &run.request_services()[0];
        assert_eq!(service.graph_source(), None);
        assert!(service.committed().is_empty());
        assert!(service.request().forced());
        assert_eq!(service.initial_stats().distinct, 1);
        assert_eq!(service.stats().distinct, 1);
        assert_eq!(service.pool().0, width);
        assert!(!service.resume_requested());
        assert!(matches!(
            service.outcome(),
            Ok(dive_liveness::periodic::Outcome::PeriodicAssumptionFalse)
        ));
        assert_eq!(run.service_evaluations()[0].graph_source(), None);
        assert_eq!(
            run.service_evaluations()[0].request().sequence(),
            service.request().sequence()
        );
        assert_eq!(
            run.service_evaluations()[0].expression(),
            snapshot.predicate().expr
        );
        assert_eq!(
            run.service_evaluations()[0]
                .result()
                .as_ref()
                .expect("FALSE equals"),
            &true
        );
        assert!(run.safety_eof().is_none());
        assert!(run.final_checks().is_empty());
        assert!(!service.model_proof());
        assert!(!run.model_proof());
    }
}
#[test]
fn actual_empty_true_service_resumes_before_real_safety_eof() {
    for width in [1, 3, 4, 8] {
        let snapshot = snapshot("x=0", "UNCHANGED x", "TRUE", "[]<>(x=0)");
        let result = snapshot
            .execute_service(
                &options(width),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options::default(),
                requests(),
            )
            .expect("admitted");
        let run = result.lifecycle();
        record_service(&format!("empty-true-w{width}"), run);
        assert_eq!(run.request_services().len(), 1);
        let service = &run.request_services()[0];
        assert_eq!(service.graph_source(), None);
        assert!(service.committed().is_empty());
        assert!(service.resume_requested());
        assert!(matches!(
            service.outcome(),
            Ok(dive_liveness::periodic::Outcome::Complete)
        ));
        assert_eq!(run.safety_eof().expect("real EOF").left_on_queue, 0);
        assert!(run.backend().complete().is_some());
        assert!(!run.final_checks().is_empty());
        assert!(!run.model_proof());
        assert!(run.invocations().is_empty());
    }
}
#[test]
fn failed_init_never_services_the_initial_request() {
    for width in [1, 4, 8] {
        let snapshot = snapshot("x=1 \\div 0", "UNCHANGED x", "FALSE", "[]<>(x=0)");
        let result = snapshot
            .execute_service(
                &options(width),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options::default(),
                requests(),
            )
            .expect("admitted");
        let run = result.lifecycle();
        record_service(&format!("failed-init-w{width}"), run);
        assert!(run.backend().stop().is_some());
        assert!(run.request_services().is_empty());
        assert!(run.service_evaluations().is_empty());
        assert!(run.expansion_pauses().is_empty());
        assert!(run.safety_eof().is_none());
    }
}

#[test]
fn requested_default_or_mixed_evaluator_mode_refuses_before_service_setup() {
    let snapshot = snapshot("x=0", "UNCHANGED x", "FALSE", "[]<>(x=0)");
    for options in [
        dive_check::CheckOptions::dive(0),
        dive_check::CheckOptions {
            eval: dive_eval::Options::dive(),
            ..options(4)
        },
        dive_check::CheckOptions {
            eval: dive_eval::Options {
                warnings: false,
                ..dive_eval::Options::tlc()
            },
            ..options(4)
        },
        dive_check::CheckOptions {
            eval: dive_eval::Options {
                max_depth: 1,
                ..dive_eval::Options::tlc()
            },
            ..options(4)
        },
        dive_check::CheckOptions {
            eval: dive_eval::Options {
                comparisons: dive_eval::Comparisons::Tla,
                ..dive_eval::Options::tlc()
            },
            ..options(4)
        },
        dive_check::CheckOptions {
            eval: dive_eval::Options {
                exploration: dive_eval::ExplorationOrder::Value,
                ..dive_eval::Options::tlc()
            },
            ..options(4)
        },
    ] {
        let error = snapshot
            .execute_service(
                &options,
                phase_graph::Limits::default(),
                periodic_lifecycle::Options::default(),
                requests(),
            )
            .err()
            .expect("mode refusal");
        assert!(
            error
                .to_string()
                .contains("matching TLC checker/evaluator mode")
        );
        assert_eq!(snapshot.model().output, [] as [dive_model::Message; 0]);
    }
}

#[test]
fn empty_request_preserves_actual_hook_evaluation_error_and_does_not_resume() {
    for width in [1, 3, 4, 8] {
        let snapshot = snapshot("x=0", "UNCHANGED x", "1 \\div 0", "[]<>(x=0)");
        let result = snapshot
            .execute_service(
                &options(width),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options::default(),
                requests(),
            )
            .expect("admitted");
        let run = result.lifecycle();
        record_service(&format!("empty-error-w{width}"), run);
        assert_eq!(run.request_services().len(), 1);
        assert_eq!(run.service_evaluations().len(), 1);
        let service = &run.request_services()[0];
        assert_eq!(service.graph_source(), None);
        assert!(service.committed().is_empty());
        assert!(service.outcome().is_err());
        assert!(!service.resume_requested());
        assert_eq!(
            run.service_evaluations()[0]
                .result()
                .as_ref()
                .expect_err("typed eval error")
                .ec()
                .code(),
            2179
        );
        assert!(run.safety_eof().is_none());
        assert!(run.final_checks().is_empty());
    }
}

#[test]
fn real_nonempty_request_service_observes_the_appended_transaction_once() {
    for width in [1, 3, 4, 8] {
        let snapshot = snapshot(
            "x \\in 0..8",
            "IF x=0 THEN x' \\in 0..8 ELSE \\E i \\in 1..4096: x'=x",
            "TRUE",
            "[]<>(x>=0)",
        );
        let result = snapshot
            .execute_service(
                &options(width),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options {
                    progress_interval: i32::MAX,
                    ..periodic_lifecycle::Options::default()
                },
                dive_check::progress::Options {
                    interval: Duration::from_millis(1),
                    force: false,
                },
            )
            .expect("admitted");
        let run = result.lifecycle();
        record_service(&format!("nonempty-safe-w{width}"), run);
        assert!(run.invocations().is_empty());
        assert!(run.hook_evaluations().is_empty());
        assert_eq!(run.request_services().len(), run.expansion_pauses().len());
        assert_eq!(
            run.request_services().len(),
            run.service_evaluations().len()
        );
        let groups = &run
            .backend()
            .complete()
            .expect("actual complete graph")
            .groups;
        let committed_ids = groups[0]
            .commits
            .iter()
            .map(|c| c.source as u64)
            .collect::<std::collections::BTreeSet<_>>();
        let mut nonempty = 0;
        for (service, pause) in run.request_services().iter().zip(run.expansion_pauses()) {
            let receipt = pause.receipt();
            assert_eq!(service.request().sequence(), receipt.request().sequence());
            assert_eq!(service.submitted(), receipt.submitted());
            assert_eq!(service.committed(), receipt.committed());
            assert_eq!(service.committed(), receipt.frontier_appended());
            assert_eq!(service.stats(), receipt.committed_stats());
            assert_eq!(service.pool(), pause.pool());
            assert!(service.resume_requested());
            if let Some(source_id) = service.graph_source() {
                assert!(committed_ids.contains(&source_id));
            }
            if !service.committed().is_empty() {
                nonempty += 1;
                assert_eq!(service.graph_source(), Some(service.committed().end - 1));
            }
        }
        assert!(nonempty > 0, "actual nonempty service required w{width}");
        assert_eq!(run.safety_eof().expect("real EOF").distinct, 9);
        assert!(!run.model_proof());
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
fn appended_request_candidate_precedes_hook_and_replays_against_fresh_binding() {
    for width in [1, 3, 4, 8] {
        let bound = snapshot("x=0", "\\E i \\in 1..1024: x'=x+1", "TRUE", "<>(x=2)");
        let run = bound
            .execute_service(
                &options(width),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options {
                    force: true,
                    progress_interval: i32::MAX,
                    ..periodic_lifecycle::Options::default()
                },
                dive_check::progress::Options {
                    interval: Duration::from_millis(1),
                    force: true,
                },
            )
            .expect("actual admitted service");
        let run = run.lifecycle();
        record_service(&format!("candidate-w{width}"), run);
        let service = run.request_services().last().expect("deciding request");
        assert!(!service.committed().is_empty());
        assert!(!service.resume_requested());
        assert!(matches!(
            service.outcome(),
            Ok(dive_liveness::periodic::Outcome::LivenessStopped(_))
        ));
        assert_eq!(
            run.request_services().len(),
            run.service_evaluations().len() + 1
        );
        assert_eq!(
            run.expansion_pauses()
                .last()
                .expect("actual append")
                .receipt()
                .frontier_appended(),
            service.committed()
        );
        assert!(run.safety_eof().is_none());
        let check = run
            .backend()
            .checks()
            .iter()
            .find(|c| c.selection().candidate().is_some())
            .expect("candidate");
        let (states, back_to) = concrete_word(run.backend(), check);
        let fresh = snapshot("x=0", "\\E i \\in 1..1024: x'=x+1", "TRUE", "<>(x=2)");
        assert!(
            !dive_trace::temporal::evaluate_lasso(
                fresh.model(),
                &fresh.model().implied_temporals[0].pred,
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
            dive_trace::validate(fresh.model(), &trace, None).validation,
            dive_trace::Validation::Valid
        );
        let doc = trace.to_doc(fresh.model()).expect("transport");
        let doc = dive_trace::TraceDoc::from_json(
            &serde_json::from_slice(&serde_json::to_vec(&doc.to_json()).expect("serialize"))
                .expect("JSON"),
        )
        .expect("decode");
        assert_eq!(
            dive_trace::validate_doc(fresh.model(), &doc, &[], None)
                .0
                .validation,
            dive_trace::Validation::Valid
        );
        let mut outcomes = Vec::new();
        for mutation in 0..4 {
            let mut bad = doc.clone();
            match mutation {
                0 => bad.properties = vec!["Forged".into()],
                1 => {
                    bad.states[0] =
                        serde_json::from_value(serde_json::json!({"x":99})).expect("wrong Init");
                }
                2 => {
                    bad.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
                    let last = bad.states.last().expect("concrete selected state")["x"]
                        .as_i64()
                        .expect("bounded integer");
                    bad.states.push(
                        serde_json::from_value(serde_json::json!({"x":last+1}))
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
            let outcome = dive_trace::validate_doc(fresh.model(), &bad, &[], None)
                .0
                .validation;
            assert!(
                matches!(outcome, dive_trace::Validation::Invalid(_)),
                "{outcome:?}"
            );
            outcomes.push(format!("{outcome:?}"));
            if mutation == 2 {
                assert!(format!("{outcome:?}").contains("Next"));
            }
        }
        if let Some(directory) = std::env::var_os("DIVE_REQUEST_SERVICE_CAPTURE") {
            let path = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&path).expect("capture dir");
            let row = serde_json::json!({"width":width,"producer_profile":"Tlc","validator_profile":"Tlc","memory":"Valid","json":"Valid",
                "wrong_property":outcomes[0],"wrong_init":outcomes[1],"wrong_closing_next":outcomes[2],"malformed_ending":outcomes[3],"model_proof":false,"ordinary_schedule_certified":false});
            std::fs::write(
                path.join(format!("candidate-replay-w{width}.json")),
                serde_json::to_vec_pretty(&row).expect("JSON"),
            )
            .expect("capture replay");
        }
        assert!(!run.model_proof());
    }
}

#[test]
fn graph_predicate_failure_prevents_later_request_hook_service() {
    for width in [1, 3, 4, 8] {
        let bound = snapshot("x=0", "x'=x+1", "TRUE", "[]<>(1 \\div x=0)");
        let run = bound
            .execute_service(
                &options(width),
                phase_graph::Limits::default(),
                periodic_lifecycle::Options::default(),
                requests(),
            )
            .expect("admitted");
        let run = run.lifecycle();
        record_service(&format!("graph-failure-w{width}"), run);
        let stop = run.backend().stop().expect("actual graph predicate stop");
        let phase_graph::Error::Phase(f) = &stop.error else {
            panic!("typed predicate failure: {:?}", stop.error)
        };
        let dive_liveness::phase_l::Error::Evaluation(error) = &f.error else {
            panic!("actual evaluation")
        };
        assert_eq!(error.ec().code(), 2179);
        assert!(run.safety_eof().is_none());
        // Initial genuine empty service is allowed, but the deciding graph
        // source never earns a later hook/controller receipt.
        assert!(
            run.request_services()
                .iter()
                .all(|s| s.graph_source().is_none())
        );
        assert!(
            run.service_evaluations()
                .iter()
                .all(|s| s.graph_source().is_none())
        );
        assert!(!run.model_proof());
    }
}

#[test]
fn request_service_retention_and_partial_work_share_the_existing_graph_quota() {
    let bound = snapshot("x=0", "UNCHANGED x", "TRUE", "[]<>(x=0)");
    let mut witnessed = None;
    for budget in 1..128 {
        let mut limits = phase_graph::Limits::default();
        limits.graph.steps = budget;
        let old = bound
            .execute(
                &options(1),
                limits,
                periodic_lifecycle::Options {
                    progress_interval: i32::MAX,
                    ..periodic_lifecycle::Options::default()
                },
            )
            .expect("original ordinal API");
        let new = bound
            .execute_service(
                &options(1),
                limits,
                periodic_lifecycle::Options {
                    progress_interval: i32::MAX,
                    ..periodic_lifecycle::Options::default()
                },
                requests(),
            )
            .expect("same ceilings");
        if old.lifecycle().backend().complete().is_some()
            && new.lifecycle().backend().stop().is_some()
        {
            assert!(matches!(
                new.lifecycle()
                    .backend()
                    .stop()
                    .expect("bounded stop")
                    .error,
                phase_graph::Error::Limit(_)
            ));
            assert!(new.lifecycle().backend().complete().is_none());
            assert!(!new.lifecycle().model_proof());
            record_service(&format!("quota-{budget}"), new.lifecycle());
            witnessed = Some(budget);
            break;
        }
    }
    assert!(
        witnessed.is_some(),
        "same quota must charge additional genuine service work"
    );
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
fn request_service_eof_candidates_have_separate_fresh_replay() {
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
                .execute_service(
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
            record_service(
                &format!("eof-{:?}-w{width}", replay.profile),
                run.lifecycle(),
            );
            if let Some(dir) = std::env::var_os("DIVE_REQUEST_SERVICE_CAPTURE") {
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

fn lowered(source: &str) -> Arc<dive_ir::Ir> {
    let analysis = dive_sema::analyze_source("Bounded.tla", source, &dive_sema::Options::default());
    Arc::new(dive_ir::lower(&analysis).expect("control IR"))
}
fn record_service(label: &str, run: &periodic_lifecycle::Run) {
    let Some(directory) = std::env::var_os("DIVE_REQUEST_SERVICE_CAPTURE") else {
        return;
    };
    let path = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&path).expect("capture directory");
    let services=run.request_services().iter().map(|s|serde_json::json!({
        "request_sequence":s.request().sequence(),"request_millis":s.request().millis(),"request_forced":s.request().forced(),
        "submitted":s.submitted(),"committed":s.committed(),"graph_source":s.graph_source(),
        "initial_stats":stats(s.initial_stats()),"stats":stats(s.stats()),"pool":s.pool(),"predicate_workers":s.predicate_workers(),
        "service_millis":s.time_millis(),"runtime_ratio":s.runtime_ratio(),"force":s.force(),
        "resumed":s.resume_requested(),"outcome":format!("{:?}",s.outcome()),"model_proof":s.model_proof()
    })).collect::<Vec<_>>();
    let evaluations=run.service_evaluations().iter().map(|e|serde_json::json!({"request_sequence":e.request().sequence(),
        "graph_source":e.graph_source(),"expression":e.expression().0,"result":format!("{:?}",e.result())})).collect::<Vec<_>>();
    let pauses=run.expansion_pauses().iter().map(|p|{let r=p.receipt();serde_json::json!({"request_sequence":r.request().sequence(),
        "submitted":r.submitted(),"claimed":r.claimed(),"completed":r.completed(),"failed":r.failed(),
        "committed":r.committed(),"frontier_appended":r.frontier_appended(),"stats":stats(r.committed_stats()),"pool":p.pool()})}).collect::<Vec<_>>();
    let row = serde_json::json!({"label":label,"services":services,"evaluations":evaluations,"pauses":pauses,
        "legacy_ordinal_invocations":run.invocations().len(),"legacy_hook_evaluations":run.hook_evaluations().len(),
        "safety_eof":run.safety_eof().map(stats),"final_checks":run.final_checks().len(),"partial_checks":run.backend().checks().len(),
        "stop":run.backend().stop().map(|s|s.error.to_string()),"model_proof":run.model_proof(),"ordinary_schedule_certified":false});
    std::fs::write(
        path.join(format!("{label}.json")),
        serde_json::to_vec_pretty(&row).expect("capture JSON"),
    )
    .expect("capture row");
}

fn stats(s: dive_check::Stats) -> serde_json::Value {
    serde_json::json!({"generated":s.generated,"distinct":s.distinct,"left_on_queue":s.left_on_queue,"depth":s.depth,"initial_generated":s.initial_generated})
}

#[test]
fn earlier_safety_winner_stops_without_a_later_service_or_eof() {
    let source = "---- MODULE Bounded ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=1\nInv == x<1\nLive == []<>(x>=0)\nHook == TRUE\n====\n";
    let cfg=dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n").into_config().expect("config");
    for width in [1, 3, 4, 8] {
        let bound = periodic_snapshot::bind(lowered(source), &cfg).expect("owned snapshot");
        let policy = periodic_lifecycle::Options {
            progress_interval: i32::MAX,
            ..periodic_lifecycle::Options::default()
        };
        let old = bound
            .execute(&options(width), phase_graph::Limits::default(), policy)
            .expect("original stopping authority");
        let new = bound
            .execute_service(
                &options(width),
                phase_graph::Limits::default(),
                policy,
                dive_check::progress::Options {
                    interval: Duration::from_millis(1),
                    force: true,
                },
            )
            .expect("request stopping authority");
        let (phase_graph::Error::Capture(a), phase_graph::Error::Capture(b)) = (
            &old.lifecycle().backend().stop().expect("oldstop").error,
            &new.lifecycle().backend().stop().expect("newstop").error,
        ) else {
            panic!("safety winner")
        };
        assert_eq!(a.verdict, b.verdict);
        assert_eq!(a.stats, b.stats);
        assert!(matches!(
            b.verdict,
            dive_check::Verdict::InvariantViolated { .. }
        ));
        // Expansion-cut cancellation is distinct from this later safety winner.
        // The ordinary lane-close guard still shuts down the timer on return.
        assert!(!new.lifecycle().requests_cancelled());
        assert!(
            new.lifecycle()
                .request_services()
                .iter()
                .all(|s| s.graph_source().is_none() && s.committed().is_empty())
        );
        assert!(new.lifecycle().safety_eof().is_none());
        record_service(&format!("safety-stop-w{width}"), new.lifecycle());
    }
}

#[test]
fn view_keyed_representatives_still_refuse_without_service_or_workers() {
    let source = "---- MODULE Bounded ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nV == x\nLive == []<>(x=0)\nHook == TRUE\n====\n";
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nVIEW V\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("cfg");
    let bound = periodic_snapshot::bind(lowered(source), &cfg).expect("pure bound VIEW");
    let run = bound
        .execute_service(
            &options(4),
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
            requests(),
        )
        .expect("diagnostic refusal");
    assert!(run.lifecycle().request_services().is_empty());
    assert!(run.lifecycle().service_evaluations().is_empty());
    assert!(run.lifecycle().expansion_pauses().is_empty());
    assert!(run.lifecycle().safety_eof().is_none());
    assert!(
        run.lifecycle()
            .backend()
            .stop()
            .expect("early unsupported")
            .error
            .to_string()
            .contains("VIEW/symmetry")
    );
}
