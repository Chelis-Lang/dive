//! Producer evidence survives graph detachment but cannot promote failures.
#![allow(clippy::expect_used, clippy::panic, reason = "actual capture controls")]

#[path = "../src/test_capture.rs"]
mod test_capture;

use std::sync::Arc;

use dive_model::Model;
use dive_runtime::{
    CheckOptions, Stats, Verdict,
    capture::{self, GraphBuilder, Limits},
};

fn model(options: &CheckOptions) -> Model {
    let source = "---- MODULE CaptureEvidence ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\n====\n";
    let analysis = dive_sema::analyze_source(
        "CaptureEvidence.tla",
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted());
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}

fn complete(model: &Model, options: &CheckOptions) -> capture::Complete {
    match test_capture::explore(model, options, Limits::default()) {
        capture::Exploration::Complete(complete) => *complete,
        capture::Exploration::Stopped(result) => panic!("finite complete: {result:?}"),
    }
}

fn rebuild(graph: &capture::BehaviorGraph) -> GraphBuilder {
    let mut builder = GraphBuilder::new(Limits::default());
    let mut next = 0;
    for &root in &graph.roots {
        let fresh = (root == next).then(|| {
            next += 1;
            root as u64
        });
        builder
            .record(
                root as u64,
                graph.states[root].clone(),
                fresh,
                None,
                0,
                "Init",
            )
            .expect("producer root");
    }
    for (source, row) in graph.edges.iter().enumerate() {
        let mut original = graph.concrete[source].iter();
        for edge in row {
            if edge.ordinal.is_none() {
                builder.stutter(source as u64).expect("stutter");
                continue;
            }
            let value = original.next().expect("original target");
            let fresh = (edge.to == next).then(|| {
                next += 1;
                edge.to as u64
            });
            builder
                .record(
                    edge.to as u64,
                    value.state.clone(),
                    fresh,
                    Some(source as u64),
                    value.ordinal,
                    &value.label,
                )
                .expect("edge");
        }
    }
    builder
}

struct Stop;
impl capture::Observer for Stop {
    fn level_done(&mut self, _: u64, _: &capture::BehaviorGraph, _: Stats) -> Result<(), String> {
        Ok(())
    }
    fn stop_after_level(&self) -> bool {
        true
    }
}

#[test]
fn original_facts_remain_sealed_when_public_capture_fields_are_edited_or_dropped() {
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        let model = model(&options);
        let mut captured = complete(&model, &options);
        let expected = captured.stats;
        let receipt = captured.completion().clone();
        let safety = format!("{:?}", captured.safety_result());
        let weak = Arc::downgrade(&captured.graph.states[0]);
        captured.stats = Stats::default();
        captured.options.seed = 99;
        captured.output.push("edited public output".into());
        captured.workers.used = 0;
        let evidence = captured.into_evidence();
        assert!(
            weak.upgrade().is_none(),
            "metadata must not retain graph state rows"
        );
        assert_eq!(evidence.stats(), expected);
        assert_eq!(evidence.completion(), &receipt);
        assert_eq!(evidence.options().seed, 7);
        assert_eq!(format!("{:?}", evidence.safety_result()), safety);
        assert_eq!(evidence.safety_result().stats, Some(expected));
    }
}

#[test]
fn external_factories_reject_exact_original_failure_even_with_admissible_counters() {
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        let model = model(&options);
        let captured = complete(&model, &options);
        let failures = [
            Verdict::Deadlock,
            Verdict::InvariantViolated {
                property: "Inv".into(),
                initial: false,
            },
            Verdict::Unsupported {
                feature: "test unsupported".into(),
            },
            Verdict::ResourceLimit {
                message: "test capture limit".into(),
            },
            Verdict::Stopped(Box::new(dive_runtime::budget::Stopped {
                resource: dive_runtime::budget::Resource::Memory,
                limit: 10,
                observed: 11,
                elapsed_ms: 12,
                phase: dive_runtime::budget::Phase::Capture,
            })),
        ];
        for verdict in failures {
            let mut safety = captured.safety_result().clone();
            safety.verdict = verdict.clone();
            assert_eq!(
                rebuild(&captured.graph)
                    .complete(safety.clone(), options.clone())
                    .err(),
                Some(verdict.clone())
            );
            safety
                .stats
                .as_mut()
                .expect("actual counters")
                .left_on_queue = 1;
            assert_eq!(
                rebuild(&captured.graph)
                    .prefix(1, safety, options.clone())
                    .err(),
                Some(verdict)
            );
        }
    }
}

#[test]
fn successful_factories_keep_existing_shape_checks_and_real_unexhausted_prefixes() {
    for mut options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        for width in [1, 4, 8] {
            options.workers = width;
            let model = model(&options);
            let captured = complete(&model, &options);
            let safety = captured.safety_result().clone();
            let rebuilt = rebuild(&captured.graph)
                .complete(safety.clone(), options.clone())
                .expect("valid completed factory");
            assert_eq!(rebuilt.completion(), captured.completion());
            assert_eq!(rebuilt.stats, captured.stats);
            for edit in 0..4 {
                let mut safety = safety.clone();
                match edit {
                    0 => safety.stats = None,
                    1 => safety.stats.as_mut().expect("stats").distinct += 1,
                    2 => safety.stats.as_mut().expect("stats").left_on_queue = 1,
                    _ => safety.stats.as_mut().expect("stats").generated = u64::MAX,
                }
                assert!(
                    rebuild(&captured.graph)
                        .complete(safety, options.clone())
                        .is_err()
                );
            }
            let mut missing_stutter = captured.graph.clone();
            missing_stutter.edges[0].pop();
            assert!(
                rebuild(&missing_stutter)
                    .complete(safety.clone(), options.clone())
                    .is_err()
            );
            assert!(
                rebuild(&captured.graph)
                    .prefix(1, safety, options.clone())
                    .is_err(),
                "exhaustion is not a prefix"
            );
            let capture::ControlledExploration::Prefix(mut prefix) =
                test_capture::explore_controlled_with_evidence(
                    &model,
                    &options,
                    Limits::default(),
                    &mut Stop,
                )
                .expect("joined producer")
            else {
                panic!("real committed prefix")
            };
            let expected = prefix.stats;
            let level = prefix.level();
            assert!(expected.left_on_queue > 0);
            assert_eq!(prefix.evidence().safety_result().verdict, Verdict::Success);
            assert_eq!(prefix.evidence().safety_result().workers, prefix.workers);
            assert_eq!(
                prefix.evidence().safety_result().collision,
                prefix.collision
            );
            prefix.options.seed = 99;
            prefix.stats = Stats::default();
            let evidence = prefix.into_evidence();
            assert_eq!(evidence.stats(), expected);
            assert_eq!(evidence.level(), level);
            assert_eq!(evidence.options().seed, 7);
        }
    }
}
