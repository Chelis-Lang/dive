//! Borrowed CSR preserves actual capture metadata and cannot promote failures.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "deterministic capture controls"
)]

#[path = "../src/test_capture.rs"]
mod test_capture;

use std::sync::Arc;

use dive_runtime::{
    CheckOptions, Stats, Verdict,
    budget::{Budget, EvaluationControl, Phase},
    capture::{self, CsrCapture, CsrCaptureInput, CsrComplete, CsrPrefix, Limits},
};

#[derive(Clone)]
struct Arrays {
    offsets: Vec<usize>,
    targets: Vec<u32>,
    ordinals: Vec<Option<u64>>,
    actions: Vec<Option<u32>>,
    edge_rows: Vec<u32>,
    representatives: Vec<u32>,
    roots: Vec<usize>,
    labels: Vec<String>,
    retained: usize,
    closed: usize,
}

impl Arrays {
    fn capture(graph: &capture::BehaviorGraph) -> Self {
        let mut result = Self {
            offsets: vec![0],
            targets: vec![],
            ordinals: vec![],
            actions: vec![],
            edge_rows: vec![],
            representatives: (0..u32::try_from(graph.states.len()).expect("small states"))
                .collect(),
            roots: graph.roots.clone(),
            labels: vec![],
            retained: graph.states.len(),
            closed: 0,
        };
        for (source, row) in graph.edges.iter().enumerate() {
            if !row.is_empty() {
                result.closed += 1;
            }
            for edge in row {
                result
                    .targets
                    .push(u32::try_from(edge.to).expect("small target"));
                result.ordinals.push(edge.ordinal);
                if let Some(label) = &edge.label {
                    result
                        .actions
                        .push(Some(u32::try_from(result.labels.len()).expect("label")));
                    result.labels.push(label.clone());
                    result
                        .edge_rows
                        .push(u32::try_from(result.retained).expect("row"));
                    result.retained += 1;
                } else {
                    result.actions.push(None);
                    result
                        .edge_rows
                        .push(u32::try_from(source).expect("stutter row"));
                }
            }
            result.offsets.push(result.targets.len());
        }
        result
    }

    fn input(&self) -> CsrCaptureInput<'_> {
        CsrCaptureInput {
            offsets: &self.offsets,
            targets: &self.targets,
            ordinals: &self.ordinals,
            actions: &self.actions,
            edge_rows: &self.edge_rows,
            representatives: &self.representatives,
            roots: &self.roots,
            action_labels: &self.labels,
            retained_rows: self.retained,
            closed_rows: self.closed,
        }
    }
}

fn model(options: &CheckOptions) -> dive_model::Model {
    let source = "---- MODULE CSR ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == \\/ x'=(IF x<2 THEN x+1 ELSE 0) \\/ x'=x\n====\n";
    let analysis = dive_sema::analyze_source("CSR.tla", source, &dive_sema::Options::default());
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

fn complete(options: &CheckOptions) -> capture::Complete {
    match test_capture::explore(&model(options), options, Limits::default()) {
        capture::Exploration::Complete(c) => *c,
        capture::Exploration::Stopped(s) => panic!("finite complete: {s:?}"),
    }
}

fn control() -> Arc<EvaluationControl> {
    EvaluationControl::new(&Budget::unlimited(), Phase::Capture).expect("control")
}

#[test]
fn real_capture_metadata_and_detached_facts_survive_csr() {
    for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        let original = complete(&options);
        let arrays = Arrays::capture(&original.graph);
        let csr = CsrComplete::from_producer(
            arrays.input(),
            original.safety_result().clone(),
            options.clone(),
            Limits::default(),
            &control(),
        )
        .expect("CSR complete");
        assert_eq!(csr.evidence().completion(), original.completion());
        assert_eq!(csr.evidence().stats(), original.stats);
        let graph = csr.graph();
        for (source, row) in original.graph.edges.iter().enumerate() {
            let range = graph.outgoing_range(source).expect("source");
            assert_eq!(range.len(), row.len());
            assert_eq!(
                graph.original_count(source),
                Some(original.graph.concrete[source].len())
            );
            for (physical, old) in range.zip(row) {
                assert_eq!(graph.target(physical), Some(old.to));
                assert_eq!(graph.ordinal(physical), Some(old.ordinal));
                assert_eq!(graph.label(physical), Some(old.label.as_deref()));
            }
            assert!(
                graph.original_row(source, row.len() - 1).is_none(),
                "stutter is not an original"
            );
        }
        assert!(graph.outgoing_range(graph.state_count()).is_none());
        assert!(graph.original_row(usize::MAX, usize::MAX).is_none());
        let evidence = csr.into_evidence();
        assert_eq!(evidence.completion(), original.completion());
        assert_eq!(evidence.options().seed, 7);
    }
}

#[test]
fn duplicate_targets_and_ordinal_gaps_keep_physical_original_positions() {
    let original = complete(&CheckOptions::dive(0));
    let mut arrays = Arrays::capture(&original.graph);
    arrays.ordinals[0] = Some(9);
    arrays.ordinals[1] = Some(27);
    arrays.targets[1] = arrays.targets[0];
    let graph =
        CsrCapture::checked(arrays.input(), Limits::default(), &control()).expect("gaps valid");
    assert_eq!(graph.target(0), graph.target(1));
    assert_ne!(graph.original_row(0, 0), graph.original_row(0, 1));
    assert_eq!(graph.ordinal(0), Some(Some(9)));
    assert_eq!(graph.ordinal(1), Some(Some(27)));
    assert!(graph.original_row(0, 9).is_none());
    arrays.actions[0] = None;
    let unnamed =
        CsrCapture::checked(arrays.input(), Limits::default(), &control()).expect("unnamed");
    assert_eq!(unnamed.label(0), Some(Some("")));
}

#[test]
fn malformed_metadata_never_mints_completion() {
    let options = CheckOptions::dive(0);
    let original = complete(&options);
    let arrays = Arrays::capture(&original.graph);
    let changes: &[fn(&mut Arrays)] = &[
        |a| {
            a.offsets.pop();
        },
        |a| a.offsets[0] = 1,
        |a| a.offsets[1] = usize::MAX,
        |a| {
            a.targets.pop();
        },
        |a| {
            a.ordinals.pop();
        },
        |a| {
            a.actions.pop();
        },
        |a| {
            a.edge_rows.pop();
        },
        |a| a.closed = 0,
        |a| a.closed = usize::MAX,
        |a| a.targets[0] = u32::MAX,
        |a| a.edge_rows[0] = u32::MAX,
        |a| a.representatives[0] = u32::MAX,
        |a| a.ordinals[0] = None,
        |a| a.ordinals[1] = a.ordinals[0],
        |a| a.ordinals[0] = Some(u64::MAX),
        |a| a.actions[0] = Some(u32::MAX),
        |a| {
            a.roots.clear();
        },
        |a| a.roots.push(0),
        |a| a.roots[0] = usize::MAX,
        |a| {
            let last = a.offsets[1] - 1;
            a.edge_rows[last] = u32::MAX;
        },
        |a| {
            let last = a.offsets[1] - 1;
            a.actions[last] = Some(0);
        },
        |a| {
            let last = a.offsets[1] - 1;
            a.ordinals[last] = Some(2);
        },
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut altered = arrays.clone();
        change(&mut altered);
        assert!(
            CsrComplete::from_producer(
                altered.input(),
                original.safety_result().clone(),
                options.clone(),
                Limits::default(),
                &control()
            )
            .is_err(),
            "mutation {index}"
        );
    }
    for stats in [
        Stats::default(),
        Stats {
            left_on_queue: 1,
            ..original.stats
        },
    ] {
        let mut safety = original.safety_result().clone();
        safety.stats = Some(stats);
        assert!(
            CsrComplete::from_producer(
                arrays.input(),
                safety,
                options.clone(),
                Limits::default(),
                &control()
            )
            .is_err()
        );
    }
    let mut safety = original.safety_result().clone();
    safety.stats = None;
    assert!(
        CsrComplete::from_producer(
            arrays.input(),
            safety,
            options,
            Limits::default(),
            &control()
        )
        .is_err()
    );
}

#[test]
fn original_failures_win_over_malformed_metadata_for_both_factories() {
    let options = CheckOptions::dive(0);
    let original = complete(&options);
    let mut arrays = Arrays::capture(&original.graph);
    arrays.offsets.clear();
    for verdict in [
        Verdict::Deadlock,
        Verdict::Unsupported {
            feature: "probe".into(),
        },
        Verdict::ResourceLimit {
            message: "probe".into(),
        },
        Verdict::InternalError {
            message: "probe".into(),
        },
    ] {
        let mut safety = original.safety_result().clone();
        safety.verdict = verdict.clone();
        assert_eq!(
            CsrComplete::from_producer(
                arrays.input(),
                safety.clone(),
                options.clone(),
                Limits::default(),
                &control()
            )
            .err(),
            Some(verdict.clone())
        );
        assert_eq!(
            CsrPrefix::from_producer(
                arrays.input(),
                1,
                safety,
                options.clone(),
                Limits::default(),
                &control()
            )
            .err(),
            Some(verdict)
        );
    }
}

struct Stop;
impl capture::Observer for Stop {
    fn stop_after_level(&self) -> bool {
        true
    }
}

#[test]
fn actual_prefix_stays_incomplete_and_cannot_be_promoted() {
    let options = CheckOptions::dive(0);
    let capture::ControlledExploration::Prefix(original) =
        test_capture::explore_controlled_with_evidence(
            &model(&options),
            &options,
            Limits::default(),
            &mut Stop,
        )
        .expect("exploration")
    else {
        panic!("prefix")
    };
    let arrays = Arrays::capture(&original.graph);
    let safety = original.evidence().safety_result().clone();
    let prefix = CsrPrefix::from_producer(
        arrays.input(),
        original.level(),
        safety.clone(),
        options.clone(),
        Limits::default(),
        &control(),
    )
    .expect("CSR prefix");
    assert_eq!(prefix.evidence().stats(), original.stats);
    assert!(prefix.graph().closed_rows() < prefix.graph().state_count());
    assert!(
        CsrComplete::from_producer(
            arrays.input(),
            safety,
            options.clone(),
            Limits::default(),
            &control()
        )
        .is_err()
    );
    let mut forged = original.evidence().safety_result().clone();
    forged.stats.as_mut().expect("stats").left_on_queue = 0;
    assert!(
        CsrComplete::from_producer(
            arrays.input(),
            forged.clone(),
            options.clone(),
            Limits::default(),
            &control()
        )
        .is_err()
    );
    assert!(
        CsrPrefix::from_producer(
            arrays.input(),
            1,
            forged,
            options,
            Limits::default(),
            &control()
        )
        .is_err()
    );
}

#[test]
fn armed_control_returns_actual_stop_and_never_completion() {
    let options = CheckOptions::dive(0);
    let original = complete(&options);
    let arrays = Arrays::capture(&original.graph);
    let watch = Budget::new(Some(std::time::Duration::ZERO), None)
        .start()
        .expect("watchdog")
        .expect("finite");
    let control = EvaluationControl::new(watch.budget(), Phase::Capture).expect("armed control");
    let error = CsrComplete::from_producer(
        arrays.input(),
        original.safety_result().clone(),
        options,
        Limits::default(),
        &control,
    )
    .expect_err("expired");
    let Verdict::Stopped(stop) = error else {
        panic!("actual resource stop: {error:?}")
    };
    assert_eq!(stop.resource, dive_runtime::budget::Resource::Time);
    assert_eq!(stop.phase, Phase::Capture);
    assert_eq!(stop.limit, 0);
}

#[test]
fn every_entry_limit_remains_a_resource_refusal() {
    let options = CheckOptions::dive(0);
    let original = complete(&options);
    let arrays = Arrays::capture(&original.graph);
    for limits in [
        Limits {
            states: 1,
            ..Limits::default()
        },
        Limits {
            edges: 1,
            ..Limits::default()
        },
        Limits {
            generated: 1,
            ..Limits::default()
        },
        Limits {
            successors_per_unit: 1,
            ..Limits::default()
        },
    ] {
        let result = CsrComplete::from_producer(
            arrays.input(),
            original.safety_result().clone(),
            options.clone(),
            limits,
            &control(),
        );
        assert!(
            matches!(result, Err(Verdict::ResourceLimit { .. })),
            "{result:?}"
        );
    }
}
