//! Private row consumer; incomplete certificates never install a callback.

use std::sync::Mutex;

use dive_check::{CheckOptions, capture};
use dive_eval::{ErrorClass, EvalError, St};
use dive_model::Model;

use super::{
    AnalysisError, CaptureRowEvidence, EvaluationErrors, PreparedProperty, StateEvaluation,
    StateEvaluationSite,
};
use crate::formula::{AtomKind, FormulaError};
use crate::tlc_plan::demand::{self, Program};

#[cfg(test)]
#[path = "row_demand_tests.rs"]
mod tests;

const CACHE: &str = "certified temporal row cache is incomplete";

pub(super) fn prepare(
    model: &Model,
    plans: &[PreparedProperty],
    options: &CheckOptions,
    fairness: usize,
    limits: crate::formula::Limits,
    graph_labels: super::GraphLabelPolicy,
    authority: Option<&super::owned::Authority<'_>>,
) -> Result<Option<Program>, AnalysisError> {
    if !matches!(graph_labels, super::GraphLabelPolicy::NoCellLimit)
        || !authority.is_some_and(|authority| authority.owns(model))
        || options.workers != 1
        || plans.iter().flat_map(|p| &p.plan.atoms).any(|atom| {
            atom.kind != AtomKind::State || !matches!(atom.test, crate::plan::AtomTest::Predicate)
        })
    {
        return Ok(None);
    }
    if crate::budget::exhausted() {
        return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
    }
    let Some(stability) = stable_producers(model, plans, limits.expansion_steps) else {
        return Ok(None);
    };
    if crate::budget::exhausted() {
        return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
    }
    // These are the actual predicates already lowered with their scopes. The
    // independent source-owner and canonical-producer facts are both present.
    let inputs = plans
        .iter()
        .map(|p| (&p.plan.formula, p.plan.atoms.len()))
        .collect::<Vec<_>>();
    match demand::compile(&inputs, fairness, limits) {
        Ok(mut program) => {
            if crate::budget::exhausted() {
                return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
            }
            if let Some(program) = &mut program {
                program.bind_stability(stability);
            }
            Ok(program)
        }
        Err(FormulaError::Limit(crate::budget::EXHAUSTED)) => {
            Err(AnalysisError::Limit(crate::budget::EXHAUSTED))
        }
        // Optional proof work/term bounds do not change the old backend's
        // admission, and no new predicate call has occurred on this path.
        Err(FormulaError::Limit(_)) => Ok(None),
        Err(error) => Err(AnalysisError::Tableau(error)),
    }
}

// No new evaluations: inspect every actual producer/retained scope, including
// safety/constraint work which can share values with a later predicate.
fn stable_producers(
    model: &Model,
    plans: &[PreparedProperty],
    work: usize,
) -> Option<dive_eval::ProducerStability> {
    let mut producers = model
        .init
        .iter()
        .map(|init| (init.pred.expr, init.pred.scope.clone()))
        .collect::<Vec<_>>();
    producers.extend(
        model
            .next
            .iter()
            .map(|pred| (pred.expr, pred.scope.clone())),
    );
    producers.extend(
        model
            .init_actions
            .iter()
            .chain(&model.actions)
            .map(|action| (action.expr(), action.scope())),
    );
    for named in model
        .invariants
        .iter()
        .chain(&model.implied_inits)
        .chain(&model.implied_actions)
        .chain(&model.constraints)
        .chain(&model.action_constraints)
        .chain(model.view.iter())
    {
        producers.push((named.pred.expr, named.pred.scope.clone()));
    }
    if let Some(alias) = &model.alias {
        let alias = alias.as_ref().ok()?;
        producers.push((alias.pred.expr, alias.pred.scope.clone()));
    }
    for named in model.postconditions.as_ref().ok()? {
        producers.push((named.pred.expr, named.pred.scope.clone()));
    }
    for &def in &model.assumptions {
        producers.push((model.ir.def(def).body, model.evaluator.scope(def).ok()?));
    }
    let predicates = plans
        .iter()
        .flat_map(|prepared| &prepared.plan.atoms)
        .map(|atom| (atom.pred.expr, atom.pred.scope.clone()))
        .collect::<Vec<_>>();
    let proof = model
        .evaluator
        .canonical_producer_stability(&producers, &predicates, work)?;
    proof.belongs_to(&model.evaluator).then_some(proof)
}

pub(super) struct Cache {
    rows: Vec<Option<Vec<u64>>>,
    offsets: Vec<usize>,
    width: usize,
}

impl Cache {
    pub(super) fn new(program: &Program) -> Self {
        Self {
            rows: Vec::new(),
            offsets: program.offsets.clone(),
            width: program.slots.len(),
        }
    }

    pub(super) fn complete(&self, source: usize) -> bool {
        self.rows.get(source).is_some_and(Option::is_some)
    }

    pub(super) fn read(
        &self,
        property: usize,
        atom: usize,
        source: usize,
    ) -> Result<bool, AnalysisError> {
        let start = *self
            .offsets
            .get(property)
            .ok_or(AnalysisError::Inconsistent(CACHE))?;
        let end = self
            .offsets
            .get(property + 1)
            .copied()
            .unwrap_or(self.width);
        let index = start
            .checked_add(atom)
            .filter(|&index| index < end)
            .ok_or(AnalysisError::Inconsistent(CACHE))?;
        let row = self
            .rows
            .get(source)
            .and_then(Option::as_ref)
            .ok_or(AnalysisError::Inconsistent(CACHE))?;
        Ok(row[index / 64] & (1_u64 << (index % 64)) != 0)
    }

    fn insert(&mut self, source: usize, values: &[Option<bool>]) -> Result<(), EvalError> {
        if values.len() != self.width || values.iter().any(Option::is_none) || self.complete(source)
        {
            return Err(EvalError::bug(CACHE));
        }
        let mut bits = Vec::new();
        let words = self.width / 64 + usize::from(!self.width.is_multiple_of(64));
        bits.try_reserve_exact(words)
            .map_err(|_| EvalError::resource_limit("temporal row cache allocation"))?;
        bits.resize(words, 0_u64);
        for (index, value) in values.iter().enumerate() {
            if *value == Some(true) {
                bits[index / 64] |= 1_u64 << (index % 64);
            }
        }
        let length = source.checked_add(1).ok_or_else(|| EvalError::bug(CACHE))?;
        if self.rows.len() < length {
            self.rows
                .try_reserve_exact(length - self.rows.len())
                .map_err(|_| EvalError::resource_limit("temporal row cache allocation"))?;
            self.rows.resize_with(length, || None);
        }
        self.rows[source] = Some(bits);
        Ok(())
    }
}

struct Failed {
    global: usize,
    cause: EvalError,
    core: bool,
}

pub(super) struct Observe<'a> {
    pub(super) model: &'a Model,
    pub(super) plans: &'a [PreparedProperty],
    pub(super) program: &'a Program,
    pub(super) cache: &'a Mutex<Cache>,
    failed: Option<Failed>,
}

impl<'a> Observe<'a> {
    pub(super) fn new(
        model: &'a Model,
        plans: &'a [PreparedProperty],
        program: &'a Program,
        cache: &'a Mutex<Cache>,
    ) -> Self {
        Self {
            model,
            plans,
            program,
            cache,
            failed: None,
        }
    }

    pub(super) fn error(
        &mut self,
        result: &dive_check::CheckResult,
        decision: Option<&capture::CheckpointDecision>,
        errors: &mut EvaluationErrors,
    ) -> Option<AnalysisError> {
        let failed = self.failed.take()?;
        if failed.cause.class() != ErrorClass::Eval {
            return None;
        }
        let row = if failed.core {
            let decision = decision?;
            if decision.phase() != capture::CheckpointPhase::SourceRow
                || decision.cause() != &failed.cause
                || result.stats != Some(decision.stats())
                || result
                    .counterexample
                    .as_ref()
                    .is_none_or(|path| path.states.is_empty())
            {
                return Some(AnalysisError::Inconsistent(
                    "capture row decision does not match original demand",
                ));
            }
            Some(Box::new(CaptureRowEvidence {
                source: decision.source(),
                result: Box::new(result.clone()),
                _proof: self.program.certificate()?,
            }))
        } else {
            None
        };
        let slot = self.program.slots[failed.global];
        let pred = &self.plans[slot.property].plan.atoms[slot.atom];
        let property = self.model.property_claims()[slot.property].name.clone();
        let source = Box::new(crate::plan::Source::new(self.model, pred.pred.location));
        let error = AnalysisError::Atom {
            property: property.clone(),
            source: source.clone(),
            detail: failed.cause.to_string(),
        };
        errors.state = Some(Box::new(StateEvaluation {
            cause: failed.cause,
            property,
            source,
            core: failed.core,
            site: StateEvaluationSite::CaptureRow,
            row,
        }));
        Some(error)
    }
}

impl capture::CheckpointObserver for Observe<'_> {
    fn reached(&mut self, event: capture::ReachedCheckpoint<'_>) -> Result<(), EvalError> {
        if event.phase() == capture::CheckpointPhase::Init {
            return Ok(());
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.program.slots.len())
            .map_err(|_| EvalError::resource_limit("temporal row scratch allocation"))?;
        values.resize(self.program.slots.len(), None);
        let state = St::state(event.state().clone());
        let outcome = self.program.evaluate(|global| {
            let slot = self.program.slots[global];
            let pred = &self.plans[slot.property].plan.atoms[slot.atom];
            match pred.evaluate_detailed(self.model, &state) {
                Ok(value) => {
                    if values[global].is_some_and(|previous| previous != value) {
                        return Err(EvalError::bug("unstable certified temporal predicate"));
                    }
                    values[global] = Some(value);
                    Ok(value)
                }
                Err(failure) => {
                    let core = matches!(&failure, crate::plan::AtomError::Core(_));
                    let cause = failure.into_error();
                    self.failed = Some(Failed {
                        global,
                        cause: cause.clone(),
                        core,
                    });
                    if core {
                        Err(cause)
                    } else {
                        Err(EvalError::unsupported(cause.to_string()))
                    }
                }
            }
        });
        outcome?;
        self.cache
            .lock()
            .map_err(|_| EvalError::bug("temporal cache lock poisoned"))?
            .insert(event.source(), &values)
    }
}

pub(super) struct Projection {
    pub(super) graph: crate::Graph,
    pub(super) sources: Vec<usize>,
    pub(super) edges: Vec<Vec<usize>>,
}

impl Projection {
    pub(super) fn new(
        graph: &capture::BehaviorGraph,
        cache: &Cache,
    ) -> Result<Self, AnalysisError> {
        let mut sources = Vec::new();
        let mut mapped = vec![None; graph.states.len()];
        for (source, target) in mapped.iter_mut().enumerate() {
            if crate::budget::exhausted() {
                return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
            }
            if cache.complete(source) {
                *target = Some(sources.len());
                sources.push(source);
            }
        }
        let mut rows = Vec::new();
        let mut edges = Vec::new();
        for &source in &sources {
            if crate::budget::exhausted() {
                return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
            }
            let mut row = Vec::new();
            let mut originals = Vec::new();
            for (position, edge) in graph.edges[source].iter().enumerate() {
                if let Some(target) = mapped[edge.to] {
                    row.push(target);
                    originals.push(position);
                }
            }
            rows.push(row);
            edges.push(originals);
        }
        let roots = graph
            .roots
            .iter()
            .filter_map(|&root| mapped[root])
            .collect();
        let graph = crate::Graph::new(rows, roots)
            .map_err(|_| AnalysisError::Inconsistent("temporal row projection"))?;
        Ok(Self {
            graph,
            sources,
            edges,
        })
    }
}

// Keep the caller's observer borrow inside a concrete forwarding owner. Its
// short borrow can share the local checkpoint lifetime without extending the
// mutable borrow of Observe through result/decision inspection.
struct BorrowedObserver<'a>(&'a mut dyn capture::Observer);

impl capture::Observer for BorrowedObserver<'_> {
    fn stop_after_level(&self) -> bool {
        self.0.stop_after_level()
    }

    fn init(
        &mut self,
        id: usize,
        state: &dive_eval::next::StateVals,
        counters: dive_check::Stats,
    ) -> Result<(), String> {
        self.0.init(id, state, counters)
    }

    fn edge(
        &mut self,
        from: usize,
        edge: &capture::Edge,
        successor: &dive_eval::next::StateVals,
        stats: dive_check::Stats,
    ) -> Result<(), String> {
        self.0.edge(from, edge, successor, stats)
    }

    fn level_done(
        &mut self,
        level: u64,
        graph: &capture::BehaviorGraph,
        stats: dive_check::Stats,
    ) -> Result<(), String> {
        self.0.level_done(level, graph, stats)
    }
}

impl Observe<'_> {
    pub(super) fn explore(
        mut self,
        options: &CheckOptions,
        limits: capture::Limits,
        observer: Option<&mut dyn capture::Observer>,
        errors: &mut EvaluationErrors,
        exploration: &mut Option<Box<super::ExplorationEvidence>>,
    ) -> Result<capture::ControlledExploration, AnalysisError> {
        let model = self.model;
        let run = {
            let mut forwarded = observer.map(BorrowedObserver);
            let observer = forwarded
                .as_mut()
                .map(|observer| observer as &mut dyn capture::Observer);
            capture::explore_checkpoints_with_evidence(model, options, limits, observer, &mut self)
        }
        .map_err(AnalysisError::CaptureThread)?;
        let (outcome, decision) = run.into_parts();
        if let capture::ControlledExploration::Stopped(result) = &outcome
            && let Some(error) = self.error(result, decision.as_deref(), errors)
        {
            let stats = result
                .stats
                .ok_or(AnalysisError::Inconsistent("row error without statistics"))?;
            *exploration = Some(Box::new(super::ExplorationEvidence {
                stats,
                workers: result.workers.clone(),
                collision: result.collision.clone(),
                output: result.output.clone(),
                warnings: result.warnings.clone(),
            }));
            return Err(error);
        }
        Ok(outcome)
    }
}
