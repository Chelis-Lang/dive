//! A graph built by an outside producer ([`GraphBuilder`]) and checked by
//! [`GraphCheck`] gives the same verdict on every schedule and on a
//! final-only check (`docs/developer/core/11-liveness.md` §5).
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite regression assertions"
)]
use std::sync::Arc;

use dive_liveness::analyze::{
    self, Analysis, GraphLabelPolicy, Limits,
    periodic::{Checks, PeriodicAnalysis, PeriodicFailure, Schedule, external::GraphCheck},
};
use dive_liveness::product::AcceptancePolicy;
use dive_runtime::{
    CheckOptions, Stats,
    capture::{self, BehaviorGraph, GraphBuilder},
};

const EVERY_LEVEL: Schedule = Schedule {
    min_states: 2,
    growth_factor: 1,
};

/// Specifications over `x` and `y`, each with its properties.
const SPECS: &[(&str, &str)] = &[
    // Fails: without fairness the behavior may stutter at x = 1.
    (
        "Init == x = 0 /\\ y = 0\nNext == x' = (x + 1) % 3 /\\ y' = y\nP == []<>(x = 0)",
        "PROPERTY P\n",
    ),
    // Fails on a prefix: the chain stutters at 2.
    (
        "Init == x = 0 /\\ y = 0\nNext == x' = (IF x < 2 THEN x + 1 ELSE x) /\\ y' = y\nP == <>(x = 3)",
        "PROPERTY P\n",
    ),
    // Weak fairness makes the progress property hold; without it, it fails.
    (
        "Init == x = 0 /\\ y = 0\nA == x < 3 /\\ x' = x + 1 /\\ y' = y\nB == y' = 1 - y /\\ x' = x\nNext == A \\/ B\nSpec == Init /\\ [][Next]_<<x, y>> /\\ WF_<<x, y>>(A)\nP == <>(x = 3)",
        "SPECIFICATION Spec\nPROPERTY P\n",
    ),
    (
        "Init == x = 0 /\\ y = 0\nA == x < 3 /\\ x' = x + 1 /\\ y' = y\nB == y' = 1 - y /\\ x' = x\nNext == A \\/ B\nP == <>(x = 3)",
        "PROPERTY P\n",
    ),
    // Leads-to, two properties, the second false.
    (
        "Init == x = 0 /\\ y = 0\nNext == \\/ x' = (x + 1) % 4 /\\ y' = y\n        \\/ y' = 1 - y /\\ x' = x\nP == (x = 1) ~> (x = 2)\nQ == []<>(y = 1)",
        "PROPERTY P\nPROPERTY Q\n",
    ),
    // An action property and strong fairness.
    (
        "Init == x = 0 /\\ y = 0\nA == x' = (x + 1) % 3 /\\ y' = y\nB == y' = 1 - y /\\ x' = x\nSpec == Init /\\ [][A \\/ B]_<<x, y>> /\\ SF_<<x, y>>(A)\nP == []<><<A>>_<<x, y>>\nQ == <>[][B]_<<x, y>>",
        "SPECIFICATION Spec\nPROPERTY P\nPROPERTY Q\n",
    ),
];

fn model(body: &str, properties: &str, options: &CheckOptions) -> dive_model::Model {
    let source =
        format!("---- MODULE External ----\nEXTENDS Naturals\nVARIABLES x, y\n{body}\n====\n");
    let analysis =
        dive_sema::analyze_source("External.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let init = if properties.contains("SPECIFICATION") {
        ""
    } else {
        "INIT Init\nNEXT Next\n"
    };
    let cfg = dive_config::parse(&format!("{init}{properties}"))
        .into_config()
        .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
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

/// Rebuilds `graph` through a builder, recording in the producer's order:
/// the initial states, then each source's successors and its stutter.
fn rebuild(graph: &BehaviorGraph) -> GraphBuilder {
    let mut builder = GraphBuilder::new(capture::Limits::default());
    let mut next = 0u64;
    let id = |to: usize, next: &mut u64| {
        let to = to as u64;
        if to == *next {
            *next += 1;
            Some(to)
        } else {
            None
        }
    };
    for &root in &graph.roots {
        let fresh = id(root, &mut next);
        builder
            .record(
                root as u64,
                graph.states[root].clone(),
                fresh,
                None,
                0,
                "Init",
            )
            .expect("root");
    }
    for (source, (row, concrete)) in graph.edges.iter().zip(&graph.concrete).enumerate() {
        let mut originals = concrete.iter();
        for edge in row {
            if edge.ordinal.is_none() {
                builder.stutter(source as u64).expect("stutter");
                continue;
            }
            let original = originals.next().expect("concrete row");
            let fresh = id(edge.to, &mut next);
            builder
                .record(
                    edge.to as u64,
                    original.state.clone(),
                    fresh,
                    Some(source as u64),
                    original.ordinal,
                    &original.label,
                )
                .expect("edge");
        }
    }
    builder
}

fn same_graph(a: &BehaviorGraph, b: &BehaviorGraph) {
    assert_eq!(a.states, b.states);
    assert_eq!(a.edges, b.edges);
    assert_eq!(a.roots, b.roots);
    assert_eq!(a.concrete.len(), b.concrete.len());
    for (x, y) in a.concrete.iter().zip(&b.concrete) {
        let key = |r: &Vec<capture::ConcreteSuccessor>| {
            r.iter()
                .map(|c| (c.state.clone(), c.ordinal, c.label.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(key(x), key(y));
    }
}

/// A comparable summary of a periodic result.
fn summary(r: &Result<(PeriodicAnalysis, Checks), PeriodicFailure>) -> String {
    match r {
        Ok((PeriodicAnalysis::Final(Analysis::Holds(c)), checks)) => {
            format!("holds {:?} {checks:?}", c.stats)
        }
        Ok((PeriodicAnalysis::Final(Analysis::Candidate(c)), checks)) => format!(
            "final {} {:?} {:?} {} {:?} {:?} {checks:?}",
            c.target_property, c.properties, c.states, c.back_to, c.edges, c.exploration.stats
        ),
        Ok((PeriodicAnalysis::Prefix(c), checks)) => format!(
            "prefix {} {:?} {:?} {} {:?} {:?} {} {checks:?}",
            c.target_property,
            c.properties,
            c.states,
            c.back_to,
            c.edges,
            c.exploration.stats,
            c.exploration.level()
        ),
        Err(f) => format!(
            "failure {} {:?} {:?} {:?}",
            f.failure.failure().error,
            f.failure.failure().stage,
            f.capture_completed,
            f.checks
        ),
    }
}

/// Plays the producer with the reference capture: every completed level is
/// handed to the graph check, and the search stops when it decides.
struct Forward<'a, 'm> {
    check: &'a mut GraphCheck<'m>,
    stop: bool,
}
impl capture::Observer for Forward<'_, '_> {
    fn level_done(
        &mut self,
        level: u64,
        graph: &BehaviorGraph,
        stats: Stats,
    ) -> Result<(), String> {
        self.stop = self.check.level(level, graph, stats);
        Ok(())
    }
    fn stop_after_level(&self) -> bool {
        self.stop
    }
}

fn external(
    model: &dive_model::Model,
    options: &CheckOptions,
    schedule: Option<Schedule>,
) -> Result<(PeriodicAnalysis, Checks), PeriodicFailure> {
    let limits = Limits::default();
    let mut check = GraphCheck::prepare(
        model,
        options,
        limits,
        AcceptancePolicy::compact_default(),
        GraphLabelPolicy::Cells(limits.atom_cells),
        schedule,
    )
    .map_err(|f| *f)?;
    let captured = {
        let mut forward = Forward {
            check: &mut check,
            stop: false,
        };
        dive_check::capture::explore_controlled_with_evidence(
            model,
            options,
            limits.capture,
            &mut forward,
        )
        .expect("capture thread")
    };
    match captured {
        capture::ControlledExploration::Complete(complete) => {
            let rebuilt = rebuild(&complete.graph);
            same_graph(rebuilt.graph(), &complete.graph);
            let complete = rebuilt
                .complete(complete.safety_result().clone(), options.clone())
                .expect("complete");
            check.finish(complete).map_err(|f| *f)
        }
        capture::ControlledExploration::Prefix(prefix) => {
            check.finish_prefix(*prefix).map_err(|f| *f)
        }
        capture::ControlledExploration::Stopped(result) => Err(*check.stopped(*result)),
    }
}

/// The verdict alone: `None` holds, `Some(p)` names the reported property.
fn verdict(r: &Result<(PeriodicAnalysis, Checks), PeriodicFailure>) -> Option<String> {
    match r {
        Ok((PeriodicAnalysis::Final(Analysis::Holds(_)), _)) => None,
        Ok((PeriodicAnalysis::Final(Analysis::Candidate(c)), _)) => Some(c.target_property.clone()),
        Ok((PeriodicAnalysis::Prefix(c), _)) => Some(c.target_property.clone()),
        Err(f) => panic!(
            "unexpected failure {}: {}",
            f.failure.failure().error,
            summary(r)
        ),
    }
}

/// The verdict of each of [`SPECS`], in order.
const VERDICTS: &[Option<&str>] = &[Some("P"), Some("P"), None, Some("P"), Some("P"), Some("Q")];

#[test]
fn external_graphs_give_the_same_verdict_on_every_schedule_and_final_only() {
    for ((body, properties), expected) in SPECS.iter().zip(VERDICTS) {
        for options in [CheckOptions::dive(7), CheckOptions::tlc(0, 7)] {
            for schedule in [Some(EVERY_LEVEL), Some(Schedule::DEFAULT), None] {
                let m = model(body, properties, &options);
                let ours = external(&m, &options, schedule);
                assert_eq!(
                    verdict(&ours).as_deref(),
                    *expected,
                    "{body} {schedule:?}: {}",
                    summary(&ours)
                );
                if schedule.is_none() {
                    let Ok((PeriodicAnalysis::Final(_), checks)) = &ours else {
                        panic!("a final-only check decides only on the complete graph");
                    };
                    assert!(checks.exhausted && checks.final_check);
                    assert_eq!(checks.prefixes, 0);
                }
            }
        }
    }
}

#[test]
fn a_safety_stop_is_never_a_temporal_verdict() {
    let options = CheckOptions::dive(7);
    let m = model(
        "Init == x = 0 /\\ y = 0\nNext == x' = x + 1 /\\ y' = y\nInv == x < 3\nP == []<>(x >= 0)",
        "INVARIANT Inv\nPROPERTY P\n",
        &options,
    );
    let r = external(&m, &options, Some(EVERY_LEVEL));
    let Err(failure) = &r else {
        panic!("an invariant violation ends the capture: {}", summary(&r));
    };
    assert!(matches!(
        failure.failure.failure().error,
        analyze::AnalysisError::Exploration(_)
    ));
    assert_eq!(failure.capture_completed, Some(false));
}

#[test]
fn row_order_follows_the_reference_row_program_conditions() {
    let (body, properties) = SPECS[0];
    let (action_body, action_properties) = SPECS[5];
    let cases = [
        (body, properties, 1, GraphLabelPolicy::NoCellLimit, true),
        (body, properties, 2, GraphLabelPolicy::NoCellLimit, false),
        (body, properties, 1, GraphLabelPolicy::Cells(1 << 20), false),
        (
            action_body,
            action_properties,
            1,
            GraphLabelPolicy::NoCellLimit,
            false,
        ),
    ];
    for (body, properties, workers, labels, expected) in cases {
        let options = CheckOptions {
            workers,
            ..CheckOptions::dive(7)
        };
        let m = model(body, properties, &options);
        let check = GraphCheck::prepare(
            &m,
            &options,
            Limits::default(),
            AcceptancePolicy::compact_default(),
            labels,
            None,
        )
        .expect("planned");
        assert_eq!(check.row_order(), expected, "{body} {workers}");
    }
}
