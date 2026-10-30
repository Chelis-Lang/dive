//! Checked CSR plus on-demand reads match the existing capture/analyzer.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite equivalence fixtures"
)]
use dive_check::budget::{Budget, EvaluationControl, Phase};
use dive_check::{CheckOptions, Stats, capture};
use dive_eval::next::StateVals;
use dive_liveness::analyze::periodic::{
    Checks, Schedule,
    external::{GraphCheck, NativeAnalysis, NativeFinal},
};
use dive_liveness::analyze::{self, AnalysisError, GraphLabelPolicy, Limits};
use dive_liveness::product::AcceptancePolicy;
use std::sync::Arc;

struct Fixture {
    offsets: Vec<usize>,
    targets: Vec<u32>,
    ordinals: Vec<Option<u64>>,
    actions: Vec<Option<u32>>,
    edge_rows: Vec<u32>,
    representatives: Vec<u32>,
    roots: Vec<usize>,
    labels: Vec<String>,
    values: Vec<StateVals>,
    closed: usize,
}
impl Fixture {
    fn new(graph: &capture::BehaviorGraph) -> Self {
        let mut out = Self {
            offsets: vec![0],
            targets: vec![],
            ordinals: vec![],
            actions: vec![],
            edge_rows: vec![],
            representatives: (0..graph.states.len())
                .map(|at| u32::try_from(at).expect("small graph"))
                .collect(),
            roots: graph.roots.clone(),
            labels: vec![],
            values: graph.states.clone(),
            closed: 0,
        };
        for (source, row) in graph.edges.iter().enumerate() {
            let mut original = 0;
            for edge in row {
                out.targets
                    .push(u32::try_from(edge.to).expect("small graph"));
                out.ordinals.push(edge.ordinal);
                if edge.ordinal.is_some() {
                    let target = &graph.concrete[source][original];
                    original += 1;
                    let row_id = u32::try_from(out.values.len()).expect("small rows");
                    out.values.push(target.state.clone());
                    out.edge_rows.push(row_id);
                    if target.label.is_empty() {
                        out.actions.push(None);
                    } else {
                        let label = if let Some(at) =
                            out.labels.iter().position(|label| label == &target.label)
                        {
                            at
                        } else {
                            out.labels.push(target.label.clone());
                            out.labels.len() - 1
                        };
                        out.actions
                            .push(Some(u32::try_from(label).expect("small labels")));
                    }
                } else {
                    out.actions.push(None);
                    out.edge_rows.push(out.representatives[source]);
                }
            }
            if !row.is_empty() {
                out.closed += 1;
            }
            out.offsets.push(out.targets.len());
        }
        out
    }
    fn input(&self) -> capture::CsrCaptureInput<'_> {
        capture::CsrCaptureInput {
            offsets: &self.offsets,
            targets: &self.targets,
            ordinals: &self.ordinals,
            actions: &self.actions,
            edge_rows: &self.edge_rows,
            representatives: &self.representatives,
            roots: &self.roots,
            action_labels: &self.labels,
            retained_rows: self.values.len(),
            closed_rows: self.closed,
        }
    }
    fn read(&self, at: usize) -> Result<StateVals, AnalysisError> {
        self.values
            .get(at)
            .cloned()
            .ok_or(AnalysisError::Inconsistent("fixture row"))
    }
}

fn model(body: &str, options: &CheckOptions) -> dive_model::Model {
    let source = format!(
        "---- MODULE Native ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = IF x < 3 THEN x + 1 ELSE x\n{body}\n====\n"
    );
    let parsed = dive_sema::analyze_source("Native.tla", &source, &dive_sema::Options::default());
    assert!(parsed.accepted(), "{body}: {:?}", parsed.diagnostics());
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
        .into_config()
        .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&parsed).expect("IR")),
        &cfg,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}
fn summary(
    result: Result<(NativeAnalysis, Checks), Box<analyze::periodic::PeriodicFailure>>,
) -> String {
    match result {
        Ok((NativeAnalysis::Final(NativeFinal::Holds(facts)), checks)) => {
            format!("holds {:?} {checks:?}", facts.stats())
        }
        Ok((NativeAnalysis::Final(NativeFinal::Candidate(word, facts)), checks)) => format!(
            "final {} {:?} {:?} {} {:?} {:?} {checks:?}",
            word.target_property,
            word.properties,
            word.states,
            word.back_to,
            word.edges,
            facts.stats()
        ),
        Ok((NativeAnalysis::Prefix(prefix), checks)) => format!(
            "prefix {} {:?} {:?} {} {:?} {:?} {} {checks:?}",
            prefix.word.target_property,
            prefix.word.properties,
            prefix.word.states,
            prefix.word.back_to,
            prefix.word.edges,
            prefix.exploration.stats(),
            prefix.exploration.level()
        ),
        Err(error) => format!(
            "failure {} {:?} {:?} {:?}",
            error.failure.failure().error,
            error.failure.failure().stage,
            error.capture_completed,
            error.checks
        ),
    }
}
struct Forward<'a, 'm> {
    check: &'a mut GraphCheck<'m>,
    control: &'a EvaluationControl,
    stop: bool,
}
impl capture::Observer for Forward<'_, '_> {
    fn level_done(
        &mut self,
        level: u64,
        graph: &capture::BehaviorGraph,
        stats: Stats,
    ) -> Result<(), String> {
        let rows = Fixture::new(graph);
        let graph =
            capture::CsrCapture::checked(rows.input(), capture::Limits::default(), self.control)
                .map_err(|error| format!("{error:?}"))?;
        self.stop = self
            .check
            .level_native(level, &graph, stats, &mut |at, _| rows.read(at));
        Ok(())
    }
    fn stop_after_level(&self) -> bool {
        self.stop
    }
}
fn native(
    model: &dive_model::Model,
    options: &CheckOptions,
    schedule: Option<Schedule>,
) -> Result<(NativeAnalysis, Checks), Box<analyze::periodic::PeriodicFailure>> {
    let limits = Limits::default();
    let mut check = GraphCheck::prepare(
        model,
        options,
        limits,
        AcceptancePolicy::compact_default(),
        GraphLabelPolicy::NoCellLimit,
        schedule,
    )?;
    let control = EvaluationControl::new(&options.budget, Phase::Capture).expect("control");
    let captured = capture::explore_controlled_with_evidence(
        model,
        options,
        limits.capture,
        &mut Forward {
            check: &mut check,
            control: &control,
            stop: false,
        },
    )
    .expect("capture thread");
    match captured {
        capture::ControlledExploration::Complete(complete) => {
            let rows = Fixture::new(&complete.graph);
            let complete = capture::CsrComplete::from_producer(
                rows.input(),
                complete.safety_result().clone(),
                options.clone(),
                limits.capture,
                &control,
            )
            .expect("EOF");
            check.finish_native(complete, &mut |at, _| rows.read(at))
        }
        capture::ControlledExploration::Prefix(prefix) => {
            let rows = Fixture::new(&prefix.graph);
            let prefix = capture::CsrPrefix::from_producer(
                rows.input(),
                prefix.level(),
                prefix.evidence().safety_result().clone(),
                options.clone(),
                limits.capture,
                &control,
            )
            .expect("prefix");
            check.finish_native_prefix(prefix, &mut |at, _| rows.read(at))
        }
        capture::ControlledExploration::Stopped(result) => Err(check.stopped(*result)),
    }
}

struct LegacyForward<'a, 'm> {
    check: &'a mut GraphCheck<'m>,
    stop: bool,
}
impl capture::Observer for LegacyForward<'_, '_> {
    fn level_done(
        &mut self,
        level: u64,
        graph: &capture::BehaviorGraph,
        stats: Stats,
    ) -> Result<(), String> {
        self.stop = self.check.level(level, graph, stats);
        Ok(())
    }
    fn stop_after_level(&self) -> bool {
        self.stop
    }
}
fn legacy(
    model: &dive_model::Model,
    options: &CheckOptions,
    schedule: Option<Schedule>,
) -> Result<(NativeAnalysis, Checks), Box<analyze::periodic::PeriodicFailure>> {
    let limits = Limits::default();
    let mut check = GraphCheck::prepare(
        model,
        options,
        limits,
        AcceptancePolicy::compact_default(),
        GraphLabelPolicy::NoCellLimit,
        schedule,
    )?;
    let captured = capture::explore_controlled_with_evidence(
        model,
        options,
        limits.capture,
        &mut LegacyForward {
            check: &mut check,
            stop: false,
        },
    )
    .expect("capture thread");
    let result = match captured {
        capture::ControlledExploration::Complete(complete) => check.finish(*complete),
        capture::ControlledExploration::Prefix(prefix) => check.finish_prefix(*prefix),
        capture::ControlledExploration::Stopped(result) => Err(check.stopped(*result)),
    };
    result.map(|(analysis, checks)| (analysis.into_native(), checks))
}

#[test]
fn final_prefix_atom_errors_and_action_labels_match_reference() {
    let schedule = Schedule {
        min_states: 2,
        growth_factor: 1,
    };
    for mut options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
        options.budget = Budget::unlimited();
        for body in [
            "P == <>[](x = 3)",
            "P == <>(x = 4)",
            "P == []<><<x' = x + 1>>_x",
            "P == []<>(1 \\div (x-x) = 0)",
        ] {
            for periodic in [false, true] {
                let model = model(body, &options);
                // The controlled external-graph entrypoint is the unchanged
                // reference for this storage adapter. The separate fused row-
                // demand producer has its own earlier-error policy; this test
                // does not claim to replace that producer or its cache.
                let expected = summary(legacy(&model, &options, periodic.then_some(schedule)));
                let actual = summary(native(&model, &options, periodic.then_some(schedule)));
                assert_eq!(actual, expected, "{body} periodic={periodic}");
            }
        }
    }
}

#[test]
fn native_readiness_matches_final_only_exhausted_and_growth_boundaries() {
    let mut options = CheckOptions::dive(0);
    options.budget = Budget::unlimited();
    let model = model("P == <>(x = 4)", &options);
    let limits = Limits::default();
    let prepare = |schedule| {
        GraphCheck::prepare(
            &model,
            &options,
            limits,
            AcceptancePolicy::compact_default(),
            GraphLabelPolicy::NoCellLimit,
            schedule,
        )
        .expect("planning")
    };
    let stats = Stats {
        distinct: 2,
        generated: 2,
        left_on_queue: 1,
        depth: 1,
        initial_generated: 1,
    };
    assert!(!prepare(None).native_level_due(stats));
    let periodic = prepare(Some(Schedule {
        min_states: 2,
        growth_factor: 1,
    }));
    assert!(!periodic.native_level_due(Stats {
        distinct: 1,
        ..stats
    }));
    assert!(periodic.native_level_due(stats));
    assert!(!periodic.native_level_due(Stats {
        left_on_queue: 0,
        ..stats
    }));
}

#[test]
fn native_prefix_rejects_another_evaluator_owner() {
    let options = CheckOptions::dive(0);
    let model = model("P == <>(x = 4)", &options);
    let mut check = GraphCheck::prepare(
        &model,
        &options,
        Limits::default(),
        AcceptancePolicy::compact_default(),
        GraphLabelPolicy::NoCellLimit,
        Some(Schedule {
            min_states: 2,
            growth_factor: 1,
        }),
    )
    .expect("prepare");
    let control = EvaluationControl::new(&Budget::unlimited(), Phase::Capture).expect("control");
    let graph = capture::CsrCapture::checked(
        capture::CsrCaptureInput {
            offsets: &[0, 2, 2],
            targets: &[1, 0],
            ordinals: &[Some(0), None],
            actions: &[None, None],
            edge_rows: &[1, 0],
            representatives: &[0, 1],
            roots: &[0],
            action_labels: &[],
            retained_rows: 2,
            closed_rows: 1,
        },
        capture::Limits::default(),
        &control,
    )
    .expect("prefix graph");
    std::thread::scope(|scope| {
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(0);
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let evaluator = &model.evaluator;
        scope.spawn(move || {
            let _owner = evaluator
                .host_control(None)
                .expect("other thread owns evaluator");
            ready_tx.send(()).expect("owner ready");
            release_rx.recv().expect("owner release");
        });
        ready_rx.recv().expect("owner installed");
        let mut reads = 0;
        let decided = check.level_native(
            1,
            &graph,
            Stats {
                distinct: 2,
                generated: 2,
                left_on_queue: 1,
                depth: 2,
                initial_generated: 1,
            },
            &mut |at, _| {
                reads += 1;
                Ok(vec![Some(dive_value::Value::from(
                    i64::try_from(at).expect("small"),
                ))]
                .into())
            },
        );
        release_tx.send(()).expect("release owner");
        assert!(decided, "owner conflict must stop this prefix check");
        assert_eq!(
            reads, 0,
            "prefix must reject owner conflict before reading or evaluating atoms"
        );
    });
}
