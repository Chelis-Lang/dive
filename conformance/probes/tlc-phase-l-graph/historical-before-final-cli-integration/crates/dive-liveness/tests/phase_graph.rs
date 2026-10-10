//! Actual concrete safety-pool capture and native null-tableau graph commits.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_check::CheckOptions;
use dive_liveness::phase_graph::{self, Error, Limits};
use std::sync::Arc;
fn model(source: &str, config: &str, opts: &CheckOptions) -> dive_model::Model {
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
    .expect("binding")
}
const SOURCE: &str = "---- MODULE PhaseGraph ----\nVARIABLE x\nInit == x = 0\nNext == x' \\in {0,1,2}\nLive == []<>(x = 2)\n====\n";
const CONFIG: &str = "INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n";

#[test]
fn real_graph_commits_keep_slot_order_bins_roots_and_actual_pool_evidence() {
    let mut baseline = None;
    for width in [1, 3, 4, 8] {
        for chaos in [None, Some(7), Some(19)] {
            let opts = CheckOptions {
                workers: width,
                chaos,
                ..CheckOptions::tlc(0, 0)
            };
            let m = model(SOURCE, CONFIG, &opts);
            let run =
                phase_graph::compile(&m, &opts, Limits::default()).expect("graph compilation");
            assert!(!run.model_proof());
            assert_eq!(run.capture.workers.used, width);
            assert_eq!(run.capture.workers.requested, width);
            assert_eq!(run.phase_work.actual_workers, 1);
            assert_eq!(
                (run.capture.stats.generated, run.capture.stats.distinct),
                (10, 3)
            );
            assert_eq!(run.nodes().len(), 3);
            assert_eq!(run.commits().len(), 3);
            assert!(
                run.commits()
                    .iter()
                    .all(|c| c.graph_insertion && !c.receipt.graph_insertion)
            );
            assert_eq!(
                run.commits()
                    .iter()
                    .map(|c| c.receipt.multiplier)
                    .collect::<Vec<_>>(),
                [1, 1, 1]
            );
            assert_eq!(run.configured_properties()[0].name, "Live");
            let fields = (run.nodes().to_vec(), run.roots().to_vec());
            if let Some(want) = &baseline {
                assert_eq!(&fields, want);
            } else {
                baseline = Some(fields);
            }
            for commit in run.commits() {
                let node = run
                    .nodes()
                    .iter()
                    .find(|n| n.fingerprint == commit.fingerprint)
                    .expect("written source");
                assert_eq!(
                    node.transitions
                        .iter()
                        .map(|t| t.target)
                        .collect::<Vec<_>>(),
                    commit
                        .receipt
                        .successors
                        .iter()
                        .map(|s| s.fingerprint)
                        .collect::<Vec<_>>()
                );
                assert!(node.transitions.iter().all(|t| t.tableau == -1));
            }
            let (graph, acceptance) = run.product_inputs(0).expect("PEM inputs");
            let witness = dive_liveness::search::find_lasso(&graph, &acceptance)
                .expect("search")
                .expect("real avoiding cycle");
            assert_ne!(witness.cycle, Vec::<dive_liveness::search::Step>::new());
            assert!(run.product_inputs(usize::MAX).is_err());
        }
    }
}

#[test]
fn independent_validation_still_decides_words_and_claims_not_graph_receipts() {
    let opts = CheckOptions {
        workers: 8,
        ..CheckOptions::tlc(0, 0)
    };
    let m = model(SOURCE, CONFIG, &opts);
    let run = phase_graph::compile(&m, &opts, Limits::default()).expect("graph");
    let (graph, acceptance) = run.product_inputs(0).expect("PEM");
    let witness = dive_liveness::search::find_lasso(&graph, &acceptance)
        .expect("search")
        .expect("witness");
    let mut ids = vec![witness.root];
    ids.extend(witness.prefix.iter().map(|edge| edge.to));
    let back_to = ids.len() - 1;
    ids.extend(
        witness
            .cycle
            .iter()
            .take(witness.cycle.len() - 1)
            .map(|edge| edge.to),
    );
    let states: Vec<_> = ids
        .iter()
        .map(|id| {
            let fp = run.nodes()[*id].fingerprint;
            let source = run
                .commits()
                .iter()
                .find(|c| c.fingerprint == fp)
                .expect("source")
                .source;
            let value = run.capture.graph.states[source][0]
                .as_ref()
                .expect("x")
                .as_int()
                .expect("integer")
                .to_i32()
                .expect("int");
            serde_json::json!({"x":value})
        })
        .collect();
    let mut doc = dive_trace::TraceDoc::behavior(states)
        .expect("trace")
        .with_properties(vec!["Live".into()]);
    doc.ending = Some(dive_trace::Ending::Lasso { back_to });
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
    let mut holds = doc;
    holds.states = vec![serde_json::from_value(serde_json::json!({"x":2})).expect("state")];
    holds.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
    assert_ne!(
        dive_trace::validate_doc(&m, &holds, &[], None).0.validation,
        dive_trace::Validation::Valid
    );
}

#[test]
fn early_bounds_and_refusals_preserve_requested_actual_width_and_no_graph_proof() {
    for width in [0, 65] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        let m = model(SOURCE, CONFIG, &opts);
        let failure = phase_graph::compile(&m, &opts, Limits::default()).expect_err("worker bound");
        let workers = failure.capture_workers.expect("known execution");
        assert_eq!((workers.requested, workers.used), (width, 0));
        assert_eq!(failure.phase_work.actual_workers, 0);
        assert!(failure.nodes.is_empty() && failure.commits.is_empty());
    }
    let opts = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let m = model(SOURCE, CONFIG, &opts);
    let failure = phase_graph::compile(
        &m,
        &opts,
        Limits {
            graph: dive_liveness::product::Limits {
                vertices: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .expect_err("vertex bound");
    assert!(matches!(failure.error, Error::Limit("graph vertices")));
    assert_eq!(failure.capture_workers.expect("pool").used, 4);
    assert_eq!(failure.capture_stats.expect("observed work").distinct, 3);
    assert_eq!(failure.phase_work.actual_workers, 0);
    assert!(failure.nodes.is_empty() && failure.commits.is_empty());
    let temporal = model(
        "---- MODULE Temporal ----\nVARIABLE x\nInit == x = 0\nNext == x' = x\nLive == [](x = 0)\n====\n",
        CONFIG,
        &opts,
    );
    let failure =
        phase_graph::compile(&temporal, &opts, Limits::default()).expect_err("tableau shape");
    assert!(matches!(failure.error, Error::Phase(_)));
    assert_eq!(failure.capture_workers.expect("no capture").requested, 4);
    assert_eq!(failure.phase_work.actual_workers, 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "complete selected field census retains every projection assertion"
)]
fn every_selected_pinned_graph_projection_is_accounted_for_at_all_widths() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/tlc-phase-l-graph");
    let capture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("capture.json")).expect("capture"))
            .expect("JSON");
    let rows = capture["rows"].as_array().expect("complete census");
    assert_eq!(rows.len(), 15);
    let mut complete_count = 0;
    let mut refused_count = 0;
    for row in rows {
        let name = row["name"].as_str().expect("name");
        let source =
            std::fs::read_to_string(base.join("models").join(name).join(format!("{name}.tla")))
                .expect("source");
        let cfg =
            std::fs::read_to_string(base.join("models").join(name).join(format!("{name}.cfg")))
                .expect("config");
        let fields = &row["fields"];
        let expected_complete = fields["completion"] == "graph-inserted-no-scc-proof";
        if expected_complete {
            complete_count += 1;
        } else {
            refused_count += 1;
        }
        for width in [1, 3, 4, 8] {
            let opts = CheckOptions {
                workers: width,
                chaos: Some(19),
                ..CheckOptions::tlc(0, 0)
            };
            let m = model(&source, &cfg, &opts);
            let result = phase_graph::compile(&m, &opts, Limits::default());
            if !expected_complete {
                let failure =
                    result.expect_err("selected unsupported/error observation cannot complete");
                assert!(
                    matches!(failure.error, Error::Phase(_)),
                    "{name}: {}",
                    failure.error
                );
                continue;
            }
            let run = result.unwrap_or_else(|e| panic!("{name}: {e:?}"));
            assert!(!run.model_proof());
            assert_eq!(run.capture.workers.used, width);
            assert_eq!(run.capture.stats.generated.to_string(), fields["generated"]);
            assert_eq!(run.capture.stats.distinct.to_string(), fields["distinct"]);
            assert_eq!(
                run.configured_properties()
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
                    .join(";"),
                fields["configured_names"]
            );
            assert_eq!(run.role_names().join(";"), fields["role_names"]);
            assert_eq!(
                run.roots()
                    .iter()
                    .map(|fp| format!("{fp}:-1"))
                    .collect::<Vec<_>>()
                    .join(";"),
                fields["roots"]
            );
            for commit in run.commits() {
                let prefix = format!("source.{}", commit.source);
                let receipt = &commit.receipt;
                let node = run
                    .nodes()
                    .iter()
                    .find(|n| n.fingerprint == commit.fingerprint)
                    .expect("written source");
                assert!(commit.graph_insertion && !receipt.graph_insertion);
                assert_eq!(
                    commit.fingerprint.to_string(),
                    fields[format!("{prefix}.fp")],
                    "{name}"
                );
                assert_eq!(
                    receipt.capacity.to_string(),
                    fields[format!("{prefix}.capacity")],
                    "{name}"
                );
                assert_eq!(
                    receipt.successors.len().to_string(),
                    fields[format!("{prefix}.count")],
                    "{name}"
                );
                assert_eq!(
                    receipt
                        .successors
                        .iter()
                        .map(|s| format!("{}:{}", s.slot, s.fingerprint))
                        .collect::<Vec<_>>()
                        .join(";"),
                    fields[format!("{prefix}.slots")],
                    "{name}"
                );
                let bits = |bs: &[bool]| {
                    bs.iter()
                        .map(|b| if *b { '1' } else { '0' })
                        .collect::<String>()
                };
                assert_eq!(
                    bits(&node.state),
                    fields[format!("{prefix}.states")],
                    "{name}"
                );
                assert_eq!(
                    node.transitions
                        .iter()
                        .map(|t| format!("{}:{}:{}", t.target, t.tableau, bits(&t.action)))
                        .collect::<Vec<_>>()
                        .join(";"),
                    fields[format!("{prefix}.transitions")],
                    "{name}"
                );
                assert_eq!(
                    receipt
                        .checks
                        .iter()
                        .map(|c| format!(
                            "g{}.{}{}:{}",
                            c.group,
                            if c.kind == dive_liveness::formula::AtomKind::Action {
                                "a"
                            } else {
                                "s"
                            },
                            c.bin,
                            c.holds
                        ))
                        .collect::<Vec<_>>()
                        .join(";"),
                    fields[format!("{prefix}.log")],
                    "{name}"
                );
                let next_multiplier = if receipt.capacity > 16 * receipt.multiplier {
                    receipt.multiplier + 1
                } else {
                    receipt.multiplier
                };
                assert_eq!(
                    next_multiplier.to_string(),
                    fields[format!("{prefix}.next_multiplier")],
                    "{name}"
                );
            }
        }
    }
    assert_eq!((complete_count, refused_count), (12, 3));
}

#[test]
fn bounded_random_finite_graphs_equal_independent_reachability_with_implicit_stutter() {
    let mut seed = 17u64;
    for case in 0..128 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let mask = seed >> 16;
        let mut edges = vec![vec![]; 3];
        let mut alternatives = vec![];
        for (a, row) in edges.iter_mut().enumerate() {
            for b in 0..3 {
                if mask & (1 << (a * 3 + b)) != 0 {
                    row.push(b);
                    alternatives.push(format!("(x = {a} /\\ x' = {b})"));
                }
            }
        }
        let next = if alternatives.is_empty() {
            "FALSE".into()
        } else {
            alternatives.join(" \\/ ")
        };
        let source = format!(
            "---- MODULE RandomGraph ----\nVARIABLE x\nInit == x = 0\nNext == {next}\nLive == []<>(x = 2)\n====\n"
        );
        let mut reached = [false; 3];
        let mut queue = vec![0];
        reached[0] = true;
        let mut at = 0;
        while at < queue.len() {
            let a = queue[at];
            for b in &edges[a] {
                if !reached[*b] {
                    reached[*b] = true;
                    queue.push(*b);
                }
            }
            at += 1;
        }
        let width = if case % 2 == 0 { 1 } else { 4 };
        let opts = CheckOptions {
            workers: width,
            chaos: Some(19),
            ..CheckOptions::tlc(0, 0)
        };
        let m = model(&source, CONFIG, &opts);
        let run = phase_graph::compile(&m, &opts, Limits::default()).expect("finite graph");
        let value = |id: usize| {
            run.capture.graph.states[id][0]
                .as_ref()
                .expect("x")
                .as_int()
                .expect("integer")
                .to_i32()
                .expect("small int")
        };
        assert_eq!(run.capture.graph.states.len(), queue.len());
        for source in 0..run.capture.graph.states.len() {
            let a = usize::try_from(value(source)).expect("node");
            assert!(reached[a]);
            let actual = run.capture.graph.edges[source]
                .iter()
                .map(|e| usize::try_from(value(e.to)).expect("dest"))
                .collect::<std::collections::BTreeSet<_>>();
            let expected = edges[a]
                .iter()
                .copied()
                .chain([a])
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(actual, expected, "case {case}");
        }
        assert!(!run.model_proof());
    }
}

#[test]
fn raw_graph_selection_hashes_and_all_before_observations_are_preserved() {
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/check-phase-l-graph.py");
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
fn foreign_model_ir_and_names_refuse_before_graph_planning() {
    for width in [1, 4, 8] {
        let opts = CheckOptions {
            workers: width,
            ..CheckOptions::tlc(0, 0)
        };
        for replace_ir in [true, false] {
            let mut m = model(SOURCE, CONFIG, &opts);
            if replace_ir {
                m.ir = Arc::new(m.ir.as_ref().clone());
            } else {
                m.names = Arc::new(dive_value::Interner::new());
            }
            let f = phase_graph::compile(&m, &opts, Limits::default())
                .expect_err("ownership refusal before planning");
            assert!(
                matches!(&f.error, Error::Input(message) if message.contains("foreign model IR or name table"))
            );
            assert_eq!(f.phase_work, dive_liveness::phase_l::Progress::default());
            assert!(f.capture_stats.is_none());
            let workers = f.capture_workers.expect("before pool setup");
            assert_eq!((workers.requested, workers.used), (width, 0));
            assert!(f.nodes.is_empty() && f.commits.is_empty());
            assert_eq!(m.evaluator.take_output(), Vec::<String>::new());
        }
    }
}

#[test]
fn deciding_conditional_planning_error_precedes_graph_capture() {
    let source = include_str!(
        "../../../conformance/probes/state-if-planning/models/StateThenError/StateThenError.tla"
    );
    for workers in [1, 4, 8] {
        let opts = CheckOptions {
            workers,
            chaos: Some(13),
            ..CheckOptions::tlc(0, 0)
        };
        let m = model(
            source,
            "INIT Init\nNEXT Next\nPROPERTY Prop\nCHECK_DEADLOCK FALSE\n",
            &opts,
        );
        let failure =
            phase_graph::compile(&m, &opts, Limits::default()).expect_err("pinned deciding error");
        let Error::Phase(phase) = &failure.error else {
            panic!("planning must decide before capture");
        };
        let dive_liveness::phase_l::Error::Planning(
            dive_liveness::tlc_plan::ModelPlanningError::Bound(
                dive_liveness::plan::PlanningError::Evaluation { error, .. },
            ),
        ) = &phase.error
        else {
            panic!("original typed planning error");
        };
        assert_eq!(error.ec().code(), 2179);
        assert!(failure.capture_stats.is_none());
        assert_eq!(
            failure
                .capture_workers
                .as_ref()
                .expect("pre-capture evidence")
                .used,
            0
        );
        assert_eq!(
            failure.phase_work,
            dive_liveness::phase_l::Progress::default()
        );
        assert_eq!(failure.nodes, Vec::<phase_graph::Node>::new());
        assert!(failure.commits.is_empty());
        assert_eq!(m.take_output(), Vec::<String>::new());
    }
}
