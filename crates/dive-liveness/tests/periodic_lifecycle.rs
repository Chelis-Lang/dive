//! Native source pause, real clock polling, stopping and actual EOF controls.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "controlled source lifecycle assertions"
)]
use dive_check::CheckOptions;
use dive_liveness::{periodic_backend, periodic_lifecycle, phase_execution, phase_graph};
use std::sync::Arc;
fn model(next: &str, invariant: &str, property: &str, opts: &CheckOptions) -> dive_model::Model {
    let source = format!(
        "---- MODULE Lifecycle ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == {next}\nInv == {invariant}\nLive == {property}\n====\n"
    );
    let a = dive_sema::analyze_source("Lifecycle.tla", &source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .expect("config");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            no_deadlock: opts.no_deadlock,
            host: None,
        },
    )
    .expect("binding")
}
#[test]
fn eof_runs_real_final_scc_even_if_no_timer_check_occurred() {
    for width in [1, 3, 4, 8] {
        for (property, candidate) in [("[]<>(x=0)", false), ("<>(x=1)", true)] {
            let opts = CheckOptions {
                workers: width,
                chaos: Some(7),
                ..CheckOptions::tlc(0, 0)
            };
            let m = model("UNCHANGED x", "TRUE", property, &opts);
            let r = periodic_lifecycle::execute(
                &m,
                &opts,
                phase_graph::Limits::default(),
                periodic_lifecycle::Options {
                    progress_interval: i32::MAX,
                    ..periodic_lifecycle::Options::default()
                },
            );
            record(&format!("eof-{candidate}-w{width}"), &r);
            assert!(r.invocations().is_empty());
            assert_eq!(r.safety_eof().expect("actual EOF").left_on_queue, 0);
            assert!(!r.final_checks().is_empty());
            assert_eq!(
                r.final_checks()
                    .iter()
                    .any(|c| c.selection().candidate.is_some()),
                candidate
            );
            assert_eq!(r.backend().complete().is_some(), !candidate);
            assert!(!r.model_proof());
        }
    }
}
#[test]
fn source_timer_candidates_stop_before_eof_and_retain_force_and_pool_facts() {
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            chaos: Some(7),
            ..CheckOptions::tlc(0, 0)
        };
        let m = model("UNCHANGED x", "TRUE", "<>(x=1)", &opts);
        let r = periodic_lifecycle::execute(
            &m,
            &opts,
            phase_graph::Limits::default(),
            periodic_lifecycle::Options {
                progress_interval: 0,
                force: true,
                ..periodic_lifecycle::Options::default()
            },
        );
        record(&format!("timer-w{width}"), &r);
        assert!(r.safety_eof().is_none());
        assert!(r.final_checks().is_empty());
        assert_eq!(r.invocations().len(), 1);
        let call = &r.invocations()[0];
        assert_eq!(call.workers(), width);
        assert_eq!(call.force(), (true, true));
        assert!(matches!(
            call.outcome(),
            Ok(dive_liveness::periodic::Outcome::LivenessStopped(2116))
        ));
        assert!(r.backend().stop().is_some());
        assert!(!r.model_proof());
    }
}
#[test]
fn bounds_safety_and_invalid_admission_do_not_create_eof_or_final_credit() {
    for width in [1, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let m = model("x'=x+1", "x=0", "[]<>(x=0)", &opts);
        let r = periodic_lifecycle::execute(
            &m,
            &opts,
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
        );
        record(&format!("safety-w{width}"), &r);
        assert!(r.backend().stop().is_some());
        assert!(r.safety_eof().is_none());
        assert!(r.final_checks().is_empty());
        let m = model("x'=x+1", "TRUE", "[]<>(x=0)", &opts);
        let mut limits = phase_graph::Limits::default();
        limits.capture.states = 3;
        let r = periodic_lifecycle::execute(
            &m,
            &opts,
            limits,
            periodic_lifecycle::Options {
                progress_interval: i32::MAX,
                ..periodic_lifecycle::Options::default()
            },
        );
        record(&format!("bounds-w{width}"), &r);
        assert!(r.backend().stop().is_some());
        assert!(r.safety_eof().is_none());
        assert!(r.final_checks().is_empty());
        for policy in [
            periodic_lifecycle::Options {
                progress_interval: -1,
                ..periodic_lifecycle::Options::default()
            },
            periodic_lifecycle::Options {
                backend: periodic_backend::Policy {
                    force: true,
                    ..periodic_backend::Policy::default()
                },
                ..periodic_lifecycle::Options::default()
            },
        ] {
            let r = periodic_lifecycle::execute(&m, &opts, phase_graph::Limits::default(), policy);
            record(
                &format!(
                    "invalid-{}-w{width}",
                    if policy.progress_interval < 0 {
                        "interval"
                    } else {
                        "force"
                    }
                ),
                &r,
            );
            assert_eq!(
                r.backend()
                    .stop()
                    .expect("parameter stop")
                    .workers
                    .as_ref()
                    .expect("known worker fact")
                    .used,
                0
            );
            assert!(r.invocations().is_empty());
            assert!(r.safety_eof().is_none());
        }
    }
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
fn internally_triggered_candidates_replay_fresh_and_reject_claim_and_word_mutants() {
    use dive_trace::{Ending, Trace, Validation};
    let mut count = 0;
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            chaos: Some(7),
            ..CheckOptions::tlc(0, 0)
        };
        let m = model("UNCHANGED x", "TRUE", "<>(x=1)", &opts);
        let run = periodic_lifecycle::execute(
            &m,
            &opts,
            phase_graph::Limits::default(),
            periodic_lifecycle::Options {
                progress_interval: 0,
                force: true,
                ..periodic_lifecycle::Options::default()
            },
        );
        record(&format!("fresh-timer-w{width}"), &run);
        let fresh = model("UNCHANGED x", "TRUE", "<>(x=1)", &opts);
        let check = run
            .backend()
            .checks()
            .iter()
            .find(|c| c.selection().candidate().is_some())
            .expect("actual candidate");
        let (states, back_to) = concrete_word(run.backend(), check);
        let truth = dive_trace::temporal::evaluate_lasso(
            &fresh,
            &fresh.implied_temporals[0].pred,
            &states,
            back_to,
        )
        .expect("independent word");
        assert!(!truth[0]);
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
        let doc = trace.to_doc(&fresh).expect("document");
        let decoded = dive_trace::TraceDoc::from_json(
            &serde_json::from_slice(&serde_json::to_vec(&doc.to_json()).expect("serialize"))
                .expect("JSON"),
        )
        .expect("decode");
        assert_eq!(
            dive_trace::validate_doc(&fresh, &decoded, &[], None)
                .0
                .validation,
            Validation::Valid
        );
        for mutation in 0..3 {
            let mut bad = decoded.clone();
            match mutation {
                0 => bad.properties = vec!["Forged".into()],
                1 => {
                    bad.states[0] =
                        serde_json::from_value(serde_json::json!({"x":99})).expect("wrong state");
                }
                _ => {
                    bad.ending = Some(Ending::Lasso {
                        back_to: usize::MAX,
                    });
                }
            }
            assert_ne!(
                dive_trace::validate_doc(&fresh, &bad, &[], None)
                    .0
                    .validation,
                Validation::Valid
            );
        }
        count += 1;
    }
    assert_eq!(count, 4);
}

fn concrete_final_word(
    run: &periodic_backend::Run,
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
    reason = "retain complete producer/refusal and fresh profile-specific EOF replay controls in order"
)]
fn actual_eof_witnesses_reconstruct_and_replay_in_both_profiles_with_closing_mutants() {
    use dive_trace::{Ending, Trace, Validation};
    let mut count = 0;
    for width in [1, 3, 4, 8] {
        let default_opts = CheckOptions {
            workers: width,
            ..CheckOptions::dive(0)
        };
        let default_model = model("x'=IF x=0 THEN 1 ELSE 2", "TRUE", "<>(x=3)", &default_opts);
        let refused = periodic_lifecycle::execute(
            &default_model,
            &default_opts,
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
        );
        assert_eq!(
            refused
                .backend()
                .stop()
                .expect("default profile refusal")
                .workers
                .as_ref()
                .expect("known admission width")
                .used,
            0
        );
        record(&format!("default-refusal-w{width}"), &refused);
        assert!(refused.invocations().is_empty());
        assert!(refused.safety_eof().is_none());
        for validator_base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            let opts = CheckOptions {
                workers: width,
                chaos: Some(7),
                ..CheckOptions::tlc(0, 0)
            };
            let m = model("x'=IF x=0 THEN 1 ELSE 2", "TRUE", "<>(x=3)", &opts);
            let run = periodic_lifecycle::execute(
                &m,
                &opts,
                phase_graph::Limits::default(),
                periodic_lifecycle::Options {
                    progress_interval: i32::MAX,
                    ..periodic_lifecycle::Options::default()
                },
            );
            record(
                &format!("fresh-eof-{:?}-w{width}", validator_base.profile),
                &run,
            );
            assert!(run.invocations().is_empty());
            assert!(
                run.safety_eof().is_some(),
                "{:?}",
                run.backend().stop().map(|s| &s.error)
            );
            let check = run
                .final_checks()
                .iter()
                .find(|c| c.selection().candidate.is_some())
                .expect("actual EOF candidate");
            let (states, back_to) = concrete_final_word(run.backend(), check);
            assert!(states.len() >= 3);
            assert_ne!(back_to, 0);
            let validator_opts = CheckOptions {
                workers: width,
                ..validator_base
            };
            let fresh = model(
                "x'=IF x=0 THEN 1 ELSE 2",
                "TRUE",
                "<>(x=3)",
                &validator_opts,
            );
            let truth = dive_trace::temporal::evaluate_lasso(
                &fresh,
                &fresh.implied_temporals[0].pred,
                &states,
                back_to,
            )
            .expect("fresh word");
            assert!(!truth[0]);
            let trace = Trace {
                labels: vec![None; states.len()],
                length: Some(states.len()),
                states,
                ending: Ending::Lasso { back_to },
                properties: vec!["Live".into()],
            };
            let memory = dive_trace::validate(&fresh, &trace, None).validation;
            assert_eq!(memory, Validation::Valid);
            let doc = trace.to_doc(&fresh).expect("document");
            let decoded = dive_trace::TraceDoc::from_json(
                &serde_json::from_slice(&serde_json::to_vec(&doc.to_json()).expect("serialize"))
                    .expect("JSON"),
            )
            .expect("decode");
            let json = dive_trace::validate_doc(&fresh, &decoded, &[], None)
                .0
                .validation;
            assert_eq!(json, Validation::Valid);
            let mut bad = decoded.clone();
            bad.states[0] =
                serde_json::from_value(serde_json::json!({"x":99})).expect("wrong Init");
            let wrong_init = dive_trace::validate_doc(&fresh, &bad, &[], None)
                .0
                .validation;
            assert!(matches!(wrong_init, Validation::Invalid(_)));
            let mut bad = decoded.clone();
            bad.ending = Some(Ending::Lasso { back_to: 0 });
            let failure = dive_trace::validate_doc(&fresh, &bad, &[], None)
                .0
                .validation;
            assert!(
                matches!(failure, Validation::Invalid(ref e) if e.check == dive_trace::Check::Next),
                "{failure:?}"
            );
            let mut bad = decoded.clone();
            bad.ending = Some(Ending::Lasso {
                back_to: usize::MAX,
            });
            let malformed = dive_trace::validate_doc(&fresh, &bad, &[], None)
                .0
                .validation;
            assert!(matches!(malformed, Validation::Invalid(_)));
            let mut bad = decoded;
            bad.properties = vec!["Forged".into()];
            let wrong_property = dive_trace::validate_doc(&fresh, &bad, &[], None)
                .0
                .validation;
            assert_ne!(wrong_property, Validation::Valid);
            record_replay(
                &format!("fresh-eof-{:?}-w{width}", validator_base.profile),
                &serde_json::json!({"width":width,"producer_profile":"tlc","validator_profile":format!("{:?}", validator_base.profile),
                    "word_initial":truth[0],"memory":format!("{memory:?}"),"json":format!("{json:?}"),
                    "wrong_init":format!("{wrong_init:?}"),"wrong_closing_next":format!("{failure:?}"),
                    "malformed_ending":format!("{malformed:?}"),"wrong_property":format!("{wrong_property:?}"),"model_proof":false}),
            );
            assert!(!run.model_proof());
            count += 1;
        }
    }
    assert_eq!(count, 8);
}

fn record(name: &str, run: &periodic_lifecycle::Run) {
    let Some(directory) = std::env::var_os("DIVE_LIFECYCLE_CAPTURE") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).expect("capture directory");
    let stats = |s: dive_check::Stats| serde_json::json!({"generated":s.generated,"distinct":s.distinct,"queue":s.left_on_queue,"depth":s.depth,"initial_generated":s.initial_generated});
    let row = serde_json::json!({"name":name,"model_proof":false,"ordinary_schedule_certified":false,
        "safety_complete":run.backend().complete().is_some(),"safety_eof":run.safety_eof().map(stats),
        "stop":run.backend().stop().map(|s| serde_json::json!({"code":s.error.code(),"text":s.error.to_string(),"stats":s.stats.map(stats),"workers":s.workers.as_ref().map(|w|serde_json::json!({"requested":w.requested,"used":w.used,"sequential":w.sequential}))})),
        "invocations":run.invocations().iter().map(|i| { let ratio=i.runtime_ratio();let force=i.force();serde_json::json!({"source":i.source(),"stats":stats(i.stats()),"workers":i.workers(),"generation":i.generation(),"time_millis":i.time_millis(),"ratio_bits":[ratio.0.to_bits(),ratio.1.to_bits()],"force":[force.0,force.1],"resume_requested":i.resume_requested(),"outcome":format!("{:?}",i.outcome())}) }).collect::<Vec<_>>(),
        "final_checks":run.final_checks().iter().map(|c|serde_json::json!({"order":c.order(),"candidate":c.selection().candidate.is_some(),"inspections":c.selection().traversal.inspections})).collect::<Vec<_>>(),
        "graph_inspections":run.backend().graph_inspections(),"partial_candidate":run.backend().checks().iter().any(|c|c.selection().candidate().is_some())});
    std::fs::write(
        directory.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&row).expect("JSON"),
    )
    .expect("capture row");
}

fn record_replay(name: &str, row: &serde_json::Value) {
    let Some(directory) = std::env::var_os("DIVE_LIFECYCLE_CAPTURE") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory).join("replay");
    std::fs::create_dir_all(&directory).expect("replay directory");
    std::fs::write(
        directory.join(format!("{name}.json")),
        serde_json::to_vec_pretty(row).expect("replay JSON"),
    )
    .expect("replay row");
}
