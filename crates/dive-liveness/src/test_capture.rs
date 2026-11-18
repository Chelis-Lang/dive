//! A small test-only behavior-graph producer for the analyzer's tests.
//!
//! It plays the producer the way Core does:
//! a sequential breadth-first search over the bound model's `Init` and
//! `Next` actions, recording each root, then each source's concrete
//! successors in action order followed by its implicit stutter, with TLC's
//! counters (generated candidates including duplicates, distinct states,
//! depth from 1 at `Init`). Invariants and deadlock are checked; any other
//! model feature this producer does not implement (constraints, implied
//! properties, `VIEW`, symmetry) panics, so no test can pass on an
//! approximated graph. Integration tests include this file by path.
#![allow(
    clippy::expect_used,
    clippy::panic,
    dead_code,
    clippy::unnecessary_wraps,
    reason = "test-only graph producer; signatures match the producer the tests used before"
)]

use std::collections::VecDeque;

use dive_eval::St;
use dive_eval::next::{Flow, StateVals, Successor};
use dive_model::Model;
use dive_runtime::capture::{
    self, ControlledExploration, Exploration, GraphBuilder, Limits, Observer,
};
use dive_runtime::{CheckOptions, CheckResult, Stats, Verdict};

/// The completed or stopped exploration of `model`.
pub fn explore(model: &Model, options: &CheckOptions, limits: Limits) -> Exploration {
    match run(model, options, limits, None) {
        ControlledExploration::Complete(complete) => Exploration::Complete(complete),
        ControlledExploration::Stopped(result) => Exploration::Stopped(result),
        ControlledExploration::Prefix(_) => panic!("an unobserved exploration has no prefix"),
    }
}

/// The exploration of `model`, publishing each completed level to
/// `observer` and stopping after a level when it asks to.
///
/// # Errors
/// Never; the signature matches the producer the tests used before.
pub fn explore_controlled_with_evidence(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    observer: &mut dyn Observer,
) -> Result<ControlledExploration, String> {
    Ok(run(model, options, limits, Some(observer)))
}

struct Search<'m> {
    model: &'m Model,
    builder: GraphBuilder,
    /// Distinct states by id; the tests' graphs are small.
    ids: Vec<StateVals>,
    generated: u64,
    initial_generated: u64,
    depth: u64,
}

impl Search<'_> {
    fn stats(&self, left_on_queue: u64) -> Stats {
        Stats {
            generated: self.generated,
            distinct: self.ids.len() as u64,
            left_on_queue,
            depth: self.depth,
            initial_generated: self.initial_generated,
        }
    }

    /// One generated candidate: TLC's `addElement`. Returns the stopping
    /// verdict, if any.
    fn on_state(
        &mut self,
        parent: Option<u64>,
        succ: &Successor<'_>,
        level: u64,
        next: &mut VecDeque<(u64, StateVals)>,
    ) -> Option<Verdict> {
        self.generated += 1;
        if !succ.complete() {
            return Some(Verdict::Unsupported {
                feature: format!("test capture: incomplete state {:?}", succ.unassigned()),
            });
        }
        let state: StateVals = succ.state.into();
        let known = self.ids.iter().position(|seen| *seen == state);
        let fresh = known.is_none();
        let id = if let Some(id) = known {
            id as u64
        } else {
            self.ids.push(state.clone());
            self.depth = self.depth.max(level);
            (self.ids.len() - 1) as u64
        };
        if let Err(verdict) = self.builder.record(
            id,
            state.clone(),
            fresh.then_some(id),
            parent,
            succ.ordinal,
            succ.action.label(),
        ) {
            return Some(verdict);
        }
        if fresh {
            let st = St::state(state.clone());
            for invariant in &self.model.invariants {
                match self.model.eval_boolean(&invariant.pred, &st) {
                    Ok(true) => {}
                    Ok(false) => {
                        return Some(Verdict::InvariantViolated {
                            property: invariant.name.clone(),
                            initial: parent.is_none(),
                        });
                    }
                    Err(error) => {
                        return Some(Verdict::Unsupported {
                            feature: format!("test capture: invariant error {error}"),
                        });
                    }
                }
            }
            next.push_back((id, state));
        }
        None
    }

    fn result(&self, verdict: Verdict, options: &CheckOptions, left: u64) -> CheckResult {
        let mut result = dive_runtime::bare(verdict, options, &self.model.ir);
        result.symmetry_liveness = Some(self.model.symmetry_liveness().clone());
        result.stats = Some(self.stats(left));
        result
    }
}

/// Publishes the roots, or one completed level's rows, to `observer`.
fn publish(
    observer: &mut dyn Observer,
    graph: &capture::BehaviorGraph,
    sources: std::ops::Range<usize>,
    level: Option<u64>,
    stats: Stats,
) -> Result<bool, String> {
    let Some(level) = level else {
        for &id in &graph.roots {
            observer.init(id, &graph.states[id], stats)?;
        }
        return Ok(false);
    };
    for from in sources {
        let mut concrete = graph.concrete[from].iter();
        for edge in &graph.edges[from] {
            let successor = if edge.ordinal.is_some() {
                &concrete.next().expect("concrete successor").state
            } else {
                &graph.states[from]
            };
            observer.edge(from, edge, successor, stats)?;
        }
    }
    observer.level_done(level, graph, stats)?;
    Ok(observer.stop_after_level())
}

#[allow(clippy::too_many_lines, reason = "one ordered breadth-first search")]
fn run(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    mut observer: Option<&mut dyn Observer>,
) -> ControlledExploration {
    assert!(
        model.constraints.is_empty()
            && model.action_constraints.is_empty()
            && model.implied_inits.is_empty()
            && model.implied_actions.is_empty()
            && model.view.is_none()
            && model.symmetry.is_none(),
        "the test capture implements Init, Next, invariants and deadlock only"
    );
    let mut search = Search {
        model,
        builder: GraphBuilder::new(limits),
        ids: Vec::new(),
        generated: 0,
        initial_generated: 0,
        depth: 0,
    };
    let mut next = VecDeque::new();
    let mut stop = None;
    let init =
        model.evaluator.init_states(
            &model.init_actions,
            &mut |succ: &Successor<'_>| match search.on_state(None, succ, 1, &mut next) {
                None => Flow::Continue,
                Some(verdict) => {
                    stop = Some(verdict);
                    Flow::Stop
                }
            },
        );
    search.initial_generated = search.generated;
    if let Err(error) = init {
        stop.get_or_insert(Verdict::Unsupported {
            feature: format!("test capture: Init error {}", error.error),
        });
    }
    if let Some(verdict) = stop {
        let left = next.len() as u64;
        return ControlledExploration::Stopped(Box::new(search.result(verdict, options, left)));
    }
    if let Some(observer) = observer.as_deref_mut() {
        let stats = search.stats(next.len() as u64);
        if let Err(message) = publish(observer, search.builder.graph(), 0..0, None, stats) {
            let verdict = Verdict::Unsupported {
                feature: format!("behavior observer: {message}"),
            };
            return ControlledExploration::Stopped(Box::new(search.result(
                verdict,
                options,
                next.len() as u64,
            )));
        }
    }
    let check_deadlock = model.settings.check_deadlock;
    let mut level = 1;
    while !next.is_empty() {
        let current = std::mem::take(&mut next);
        let count = current.len();
        let first = current.front().map_or(0, |(id, _)| *id);
        for (i, (id, s0)) in current.into_iter().enumerate() {
            let before = search.generated;
            let mut stop = None;
            let r = model
                .evaluator
                .successors(
                    &model.actions,
                    &s0,
                    &mut |succ: &Successor<'_>| match search.on_state(
                        Some(id),
                        succ,
                        level + 1,
                        &mut next,
                    ) {
                        None => Flow::Continue,
                        Some(verdict) => {
                            stop = Some(verdict);
                            Flow::Stop
                        }
                    },
                );
            if let Err(error) = r {
                stop.get_or_insert(Verdict::Unsupported {
                    feature: format!("test capture: Next error {}", error.error),
                });
            }
            if stop.is_none() && check_deadlock && search.generated == before {
                stop = Some(Verdict::Deadlock);
            }
            if stop.is_none()
                && let Err(verdict) = search.builder.stutter(id)
            {
                stop = Some(verdict);
            }
            if let Some(verdict) = stop {
                let left = (count - i - 1 + next.len()) as u64;
                return ControlledExploration::Stopped(Box::new(
                    search.result(verdict, options, left),
                ));
            }
        }
        let left = next.len() as u64;
        if let Some(observer) = observer.as_deref_mut() {
            let first = usize::try_from(first).expect("source id");
            let stats = search.stats(left);
            match publish(
                observer,
                search.builder.graph(),
                first..first + count,
                Some(level),
                stats,
            ) {
                Ok(true) if left > 0 => {
                    let safety = search.result(Verdict::Success, options, left);
                    let Search { builder, .. } = search;
                    return match builder.prefix(level, safety.clone(), options.clone()) {
                        Ok(prefix) => ControlledExploration::Prefix(Box::new(prefix)),
                        Err(verdict) => ControlledExploration::Stopped(Box::new(CheckResult {
                            verdict,
                            ..safety
                        })),
                    };
                }
                Ok(_) => {}
                Err(message) => {
                    let verdict = Verdict::Unsupported {
                        feature: format!("behavior observer: {message}"),
                    };
                    return ControlledExploration::Stopped(Box::new(
                        search.result(verdict, options, left),
                    ));
                }
            }
        }
        level += 1;
    }
    let safety = search.result(Verdict::Success, options, 0);
    let Search { builder, .. } = search;
    match builder.complete(safety.clone(), options.clone()) {
        Ok(complete) => ControlledExploration::Complete(Box::new(complete)),
        Err(verdict) => ControlledExploration::Stopped(Box::new(CheckResult { verdict, ..safety })),
    }
}
