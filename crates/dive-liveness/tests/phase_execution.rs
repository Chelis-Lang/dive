//! Actual ordered safety-pool capture and multiple temporal graph commits.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_check::{CheckOptions, Verdict};
use dive_liveness::phase_execution;
use dive_liveness::phase_graph::{Error, Limits};
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

const CONFIG: &str = "INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n";
const SOURCE: &str = "---- MODULE Execution ----\nVARIABLE x\nInit == x = 0\nNext == x' \\in {0,1,2}\nLive == []<>(x = 2)\n====\n";
#[test]
fn real_ordered_null_graph_is_identical_to_prior_completed_graph_at_every_width() {
    for width in [1, 3, 4, 8] {
        for chaos in [None, Some(7), Some(19)] {
            let opts = CheckOptions {
                workers: width,
                chaos,
                ..CheckOptions::tlc(0, 0)
            };
            let m = model(SOURCE, CONFIG, &opts);
            let old = dive_liveness::phase_graph::compile(&m, &opts, Limits::default())
                .expect("baseline");
            let run = phase_execution::execute(&m, &opts, Limits::default()).expect("ordered");
            assert_eq!(run.capture.stats, old.capture.stats);
            assert_eq!(run.capture.workers.used, width);
            assert_eq!(run.groups.len(), 1);
            let g = &run.groups[0];
            assert!(g.nodes.iter().all(|n| n.written));
            for node in &g.nodes {
                let before = old
                    .nodes()
                    .iter()
                    .find(|n| n.fingerprint == node.fingerprint)
                    .expect("node");
                assert_eq!(node.state, before.state);
                assert_eq!(node.transitions, before.transitions);
            }
            assert_eq!(g.roots.iter().map(|r| r.0).collect::<Vec<_>>(), old.roots());
            let (graph, labels) = run.product_inputs(0, 0).expect("actual product");
            assert!(
                dive_liveness::search::find_lasso(&graph, &labels)
                    .expect("SCC")
                    .is_some()
            );
            assert!(!run.model_proof());
        }
    }
}
#[test]
fn earlier_temporal_failure_beats_later_safety_or_enumerator_failure() {
    for next in [
        "x < 2 /\\ x' = x + 1",
        "x' = IF x = 0 THEN 0 ELSE 1 \\div 0",
    ] {
        for width in [1, 3, 4, 8] {
            let opts = CheckOptions {
                workers: width,
                ..CheckOptions::tlc(0, 0)
            };
            let src = format!(
                "---- MODULE Earlier ----\nEXTENDS Integers\nVARIABLE x\nInit == x \\in {{0,1}}\nNext == {next}\nInv == x < 2\nLive == []<>(1 \\div x = 0)\n====\n"
            );
            let m = model(&src, &(CONFIG.to_owned() + "INVARIANT Inv\n"), &opts);
            let fail = phase_execution::execute(&m, &opts, Limits::default())
                .expect_err("predicate error");
            let Error::Phase(phase) = &fail.error else {
                panic!("typed temporal failure")
            };
            let dive_liveness::phase_l::Error::Evaluation(error) = &phase.error else {
                panic!("eval")
            };
            assert_eq!(error.ec().code(), 2179);
            let stats = fail.stats.expect("source stop prefix");
            assert_eq!(
                (
                    stats.generated,
                    stats.distinct,
                    stats.left_on_queue,
                    stats.depth
                ),
                (3, 2, 1, 1)
            );
            assert_eq!(fail.workers.expect("pool").used, width);
            assert!(fail.groups.iter().all(|g| g.commits.is_empty()));
            assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
        }
    }
}
#[test]
fn actual_tableau_initial_consistency_precedes_initial_state_invariant() {
    for width in [1, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let src = "---- MODULE InitOrder ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' = x\nInv == x < 0\nLive == [](1 \\div x = 0 => <>(x = 2))\n====\n";
        let m = model(src, &(CONFIG.to_owned() + "INVARIANT Inv\n"), &opts);
        let fail =
            phase_execution::execute(&m, &opts, Limits::default()).expect_err("initial error");
        let Error::Phase(phase) = &fail.error else {
            panic!("phase error")
        };
        let dive_liveness::phase_l::Error::Evaluation(error) = &phase.error else {
            panic!("eval")
        };
        assert_eq!(error.ec().code(), 2179);
        assert_eq!(fail.workers.expect("initial executes inline").used, 1);
        assert_eq!(fail.phase_work.sources, 0);
        assert!(fail.groups.iter().all(|g| g.commits.is_empty()));
        // These are callback-prefix counts, not TLC's doInit error replay totals.
        let stats = fail.stats.expect("prefix");
        assert_eq!(
            (stats.generated, stats.distinct, stats.left_on_queue),
            (1, 1, 1)
        );
    }
}
#[test]
fn two_real_tableau_oos_and_done_revisitation_have_complete_product_inputs() {
    for width in [1, 3, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let src = "---- MODULE Many ----\nVARIABLE x\nInit == x = 0\nNext == x' \\in {0,1,2}\nP == <>(x = 1)\nQ == <>(x = 2)\n====\n";
        let m = model(
            src,
            "INIT Init\nNEXT Next\nPROPERTIES P Q\nCHECK_DEADLOCK FALSE\n",
            &opts,
        );
        let run = phase_execution::execute(&m, &opts, Limits::default()).expect("two OOS");
        assert_eq!(run.groups.len(), 2);
        for g in 0..2 {
            assert!(
                run.groups[g]
                    .nodes
                    .iter()
                    .all(|n| n.written && n.tableau >= 0)
            );
            let (graph, labels) = run.product_inputs(g, 0).expect("product");
            assert!(
                dive_liveness::search::find_lasso(&graph, &labels)
                    .expect("SCC")
                    .is_some()
            );
        }
        assert_eq!(
            run.configured_properties
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["P", "Q"]
        );
    }
    let opts = CheckOptions::tlc(0, 0);
    let src = "---- MODULE Revisit ----\nVARIABLE x\nInit == x = 0\nNext == x' \\in {0,1}\nLive == [](x = 0 => <>(x = 1))\n====\n";
    let m = model(src, CONFIG, &opts);
    let run = phase_execution::execute(&m, &opts, Limits::default()).expect("done-state product");
    assert!(
        run.groups
            .iter()
            .any(|g| g.writes.len() > run.capture.graph.states.len())
    );
    let (graph, labels) = run.product_inputs(0, 0).expect("product");
    assert!(
        dive_liveness::search::find_lasso(&graph, &labels)
            .expect("SCC")
            .is_some()
    );
}
#[test]
fn earlier_source_safety_and_preflight_refusal_do_not_earn_temporal_completion() {
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let src = "---- MODULE Safety ----\nEXTENDS Integers\nVARIABLE x\nInit == x = 0\nNext == x' = 1\nInv == x = 0\nLive == []<>(1 \\div x = 0)\n====\n";
    let m = model(src, &(CONFIG.to_owned() + "INVARIANT Inv\n"), &opts);
    let f = phase_execution::execute(&m, &opts, Limits::default()).expect_err("safety wins");
    assert!(
        matches!(f.error,Error::Capture(ref c) if matches!(c.verdict,Verdict::InvariantViolated { .. }))
    );
    assert_eq!(f.phase_work.predicates, 0);
    assert!(f.groups.iter().all(|g| g.commits.is_empty()));
    for width in [0, 65] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let m = model(SOURCE, CONFIG, &opts);
        let f = phase_execution::execute(&m, &opts, Limits::default()).expect_err("width");
        assert_eq!(f.workers.expect("preflight").used, 0);
        assert!(f.stats.is_none() && f.groups.is_empty());
    }
    let m = model(SOURCE, CONFIG, &opts);
    let f = phase_execution::execute(
        &m,
        &opts,
        Limits {
            graph: dive_liveness::product::Limits {
                vertices: 0,
                ..dive_liveness::product::Limits::default()
            },
            ..Limits::default()
        },
    )
    .expect_err("root bound");
    assert_eq!(f.phase_work.predicates, 0);
    assert!(f.groups.iter().all(|g| g.nodes.is_empty()));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "retain complete pinned model census and inspect all graph field contracts"
)]
fn complete_pinned_graph_census_retains_errors_and_matches_every_completed_field() {
    use std::collections::BTreeMap;
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/tlc-phase-execution");
    let capture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("capture.json")).expect("capture"))
            .expect("JSON");
    let rows = capture["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 26);
    let mut completed = 0;
    let mut refused = 0;
    let mut errors = 0;
    for row in rows {
        let name = row["name"].as_str().expect("name");
        let src =
            std::fs::read_to_string(base.join("models").join(name).join(format!("{name}.tla")))
                .expect("source");
        let cfg =
            std::fs::read_to_string(base.join("models").join(name).join(format!("{name}.cfg")))
                .expect("cfg");
        for width in [1, 3, 4, 8] {
            let opts = CheckOptions {
                workers: width,
                ..CheckOptions::tlc(0, 0)
            };
            let m = match bind(&src, &cfg, &opts) {
                Ok(m) => m,
                Err(e) => {
                    assert_ne!(row["exit"], 0, "unexpected binding failure {name}: {e:?}");
                    errors += usize::from(width == 1);
                    continue;
                }
            };
            let native = phase_execution::execute(&m, &opts, Limits::default());
            if row["exit"] != 0 {
                errors += usize::from(width == 1);
                assert!(native.is_err(), "reference-error census row {name}");
                continue;
            }
            if row["fields"]["groups"] == "0" {
                refused += usize::from(width == 1);
                assert!(native.is_err(), "no-OOS role boundary {name}");
                continue;
            }
            let run = native.unwrap_or_else(|e| panic!("{name} width {width}: {:?}", e.error));
            let raw = row["fields"].as_object().expect("fields");
            assert_eq!(run.groups.len().to_string(), raw["groups"], "{name}");
            assert_eq!(
                run.capture.stats.generated.to_string(),
                raw["generated"],
                "{name} generated"
            );
            assert_eq!(
                run.capture.stats.distinct.to_string(),
                raw["distinct"],
                "{name} distinct"
            );
            let mut fields = BTreeMap::new();
            for (g, group) in run.groups.iter().enumerate() {
                fields.insert(
                    format!("final.g{g}.roots"),
                    group
                        .roots
                        .iter()
                        .map(|(fp, t)| format!("{fp}:{t}"))
                        .collect::<Vec<_>>()
                        .join(";"),
                );
                let mut records: BTreeMap<u64, Vec<String>> = BTreeMap::new();
                for node in &group.nodes {
                    records
                        .entry(node.fingerprint)
                        .or_default()
                        .push(format!("{}:{}", node.tableau, node.written));
                    if !node.written {
                        continue;
                    }
                    let prefix = format!("final.g{g}.node.{}.{}", node.fingerprint, node.tableau);
                    fields.insert(
                        format!("{prefix}.states"),
                        node.state
                            .iter()
                            .map(|b| if *b { '1' } else { '0' })
                            .collect::<String>(),
                    );
                    fields.insert(
                        format!("{prefix}.transitions"),
                        node.transitions
                            .iter()
                            .map(|t| {
                                format!(
                                    "{}:{}:{}",
                                    t.target,
                                    t.tableau,
                                    t.action
                                        .iter()
                                        .map(|b| if *b { '1' } else { '0' })
                                        .collect::<String>()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(";"),
                    );
                }
                for (fp, records) in records {
                    if group.nodes.iter().any(|n| n.tableau >= 0) {
                        fields.insert(
                            format!("final.g{g}.record_order.{fp}"),
                            records
                                .iter()
                                .map(|s| s.split(':').next().expect("index"))
                                .collect::<Vec<_>>()
                                .join(";"),
                        );
                    }
                    fields.insert(format!("final.g{g}.records.{fp}"), {
                        let mut sorted = records.clone();
                        sorted.sort_by_key(|s| {
                            s.split(':')
                                .next()
                                .expect("index")
                                .parse::<i32>()
                                .expect("index")
                        });
                        sorted.join(";")
                    });
                }
            }
            // Java prints empty record rows for states not registered in an OOS.
            // Those rows remain in the capture and are checked as absence.
            for (key, value) in raw {
                if key.starts_with("final.") {
                    assert_eq!(
                        fields.get(key).map_or("", String::as_str),
                        value.as_str().expect("text"),
                        "{name} {key}"
                    );
                }
            }
            for (key, value) in &fields {
                assert_eq!(
                    raw.get(key).and_then(serde_json::Value::as_str),
                    Some(value.as_str()),
                    "unexpected {name} {key}"
                );
            }
            assert!(!run.model_proof());
            completed += usize::from(width == 1);
        }
    }
    assert_eq!((completed, refused, errors), (15, 3, 8));
}

#[test]
fn actual_product_candidates_require_independent_complete_claims_and_closing_edges() {
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let src = "---- MODULE Word ----\nVARIABLE x\nInit == x = 0\nNext == x' \\in {0,1,2}\nP == <>(x = 3)\nQ == <>(x = 4)\n====\n";
    let cfg = "INIT Init\nNEXT Next\nPROPERTIES P Q\nCHECK_DEADLOCK FALSE\n";
    let m = model(src, cfg, &opts);
    let run = phase_execution::execute(&m, &opts, Limits::default()).expect("actual product");
    for group in 0..run.groups.len() {
        let (graph, labels) = run.product_inputs(group, 0).expect("SCC inputs");
        let lasso = dive_liveness::search::find_lasso(&graph, &labels)
            .expect("search")
            .expect("candidate");
        let mut positions = vec![lasso.root];
        positions.extend(lasso.prefix.iter().map(|e| e.to));
        let back_to = positions.len() - 1;
        positions.extend(lasso.cycle.iter().take(lasso.cycle.len() - 1).map(|e| e.to));
        let states=positions.iter().map(|id| {
            let fp=run.groups[group].nodes[*id].fingerprint;
            let source=run.groups[group].commits.iter().find(|c|c.fingerprint==fp).expect("source fingerprint").source;
            serde_json::json!({"x":run.capture.graph.states[source][0].as_ref().expect("x").as_int().expect("int").to_i32().expect("small")})
        }).collect();
        let doc = dive_trace::TraceDoc::behavior(states)
            .expect("doc")
            .with_ending(dive_trace::Ending::Lasso { back_to })
            .with_properties(vec!["P".into(), "Q".into()]);
        assert_eq!(
            dive_trace::validate_doc(&m, &doc, &[], None).0.validation,
            dive_trace::Validation::Valid
        );
        let mut forged = doc.clone();
        forged.states[0] = serde_json::from_value(serde_json::json!({"x":99})).expect("state");
        assert_ne!(
            dive_trace::validate_doc(&m, &forged, &[], None)
                .0
                .validation,
            dive_trace::Validation::Valid
        );
        let mut omitted = doc.clone();
        omitted.properties.pop();
        assert_ne!(
            dive_trace::validate_doc(&m, &omitted, &[], None)
                .0
                .validation,
            dive_trace::Validation::Valid
        );
        let mut bad_next = doc.clone();
        bad_next
            .states
            .push(serde_json::from_value(serde_json::json!({"x":99})).expect("state"));
        assert_ne!(
            dive_trace::validate_doc(&m, &bad_next, &[], None)
                .0
                .validation,
            dive_trace::Validation::Valid
        );
        let mut bad_back = doc;
        bad_back.ending = Some(dive_trace::Ending::Lasso {
            back_to: usize::MAX,
        });
        assert_ne!(
            dive_trace::validate_doc(&m, &bad_back, &[], None)
                .0
                .validation,
            dive_trace::Validation::Valid
        );
    }
}
#[test]
fn scoped_impurity_and_foreign_ownership_stop_before_callbacks_or_pool() {
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let src = "---- MODULE Opaque ----\nEXTENDS TLC\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nLive == []<>(x = 1)\nQ(P(_)) == P(x)\n====\n";
    let mut m = model(src, CONFIG, &opts);
    let d = m.ir.root_def("Q").expect("Q");
    let sc = m
        .evaluator
        .scope_apply(
            &m.evaluator.scope(d).expect("owned application scope"),
            d,
            &[dive_ir::Arg::Op {
                op: dive_ir::OpRef::Std(dive_ir::StdOp::TlcPrintT),
                level: dive_ir::Level::Constant,
            }],
        )
        .expect("scope");
    m.invariants.push(dive_model::Named {
        name: "hidden PrintT".into(),
        pred: dive_model::Pred::new(m.ir.def(d).body, sc),
    });
    let f = phase_execution::execute(&m, &opts, Limits::default()).expect_err("captured effect");
    assert!(matches!(f.error, Error::Capture(_)));
    assert_eq!(f.workers.expect("preflight").used, 0);
    assert_eq!(f.phase_work.predicates, 0);
    assert!(f.groups.iter().all(|g| g.nodes.is_empty()));
    assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
    for names in [false, true] {
        let mut m = model(SOURCE, CONFIG, &opts);
        if names {
            m.names = Arc::new(dive_value::Interner::new());
        } else {
            m.ir = Arc::new((*m.ir).clone());
        }
        let f = phase_execution::execute(&m, &opts, Limits::default()).expect_err("ownership");
        assert_eq!(f.workers.expect("preflight").used, 0);
        assert!(f.groups.is_empty() && f.stats.is_none());
        assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
    }
}

#[test]
fn every_raw_execution_and_before_source_observation_is_hash_verified() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/check-phase-execution.py");
    assert!(
        std::process::Command::new("python3")
            .arg("-I")
            .arg(script)
            .status()
            .expect("verifier")
            .success()
    );
}

#[test]
fn malformed_bound_roles_refuse_before_temporal_planning_and_pool() {
    for width in [1, 4, 8] {
        let options = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let valid = model(SOURCE, CONFIG, &options);
        let foreign = model(SOURCE, CONFIG, &options);
        for case in 0..4 {
            let mut m = valid.clone();
            match case {
                0 => m.init_actions = foreign.init_actions.clone(),
                1 => m.implied_temporals[0].pred = foreign.implied_temporals[0].pred.clone(),
                2 => m.implied_temporals[0].pred.location = dive_ir::ExprId(u32::MAX),
                3 => m.implied_temporals[0].pred.expr = dive_ir::ExprId(u32::MAX),
                _ => panic!("case"),
            }
            let failed = phase_execution::execute(&m, &options, Limits::default())
                .expect_err("metadata admission");
            assert!(matches!(failed.error, Error::Input(_)));
            assert_eq!(failed.workers.expect("zero-work admission").used, 0);
            assert!(failed.stats.is_none() && failed.groups.is_empty());
            assert_eq!(failed.phase_work.predicates, 0);
            assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
            let failed = match dive_liveness::phase_l::Session::new(&m, 0, 1, Default::default()) {
                Err(failure) => failure,
                Ok(_) => panic!("public session accepted malformed metadata"),
            };
            assert!(matches!(
                failed.error,
                dive_liveness::phase_l::Error::Input(_)
            ));
            assert_eq!(failed.progress.predicates, 0);
        }
    }
}
