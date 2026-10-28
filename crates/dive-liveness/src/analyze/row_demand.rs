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

/// Whether a row program can apply to these plans: one worker, uncapped
/// labels and only state-predicate atoms. An owned model whose producers
/// are stable is further required; this is the part that needs no model.
pub(super) fn may_apply(
    plans: &[PreparedProperty],
    options: &CheckOptions,
    graph_labels: super::GraphLabelPolicy,
) -> bool {
    matches!(graph_labels, super::GraphLabelPolicy::NoCellLimit)
        && options.workers == 1
        && plans.iter().flat_map(|p| &p.plan.atoms).all(|atom| {
            atom.kind == AtomKind::State && matches!(atom.test, crate::plan::AtomTest::Predicate)
        })
}

pub(super) fn prepare(
    model: &Model,
    plans: &[PreparedProperty],
    options: &CheckOptions,
    fairness: usize,
    limits: crate::formula::Limits,
    graph_labels: super::GraphLabelPolicy,
    owned_control: (
        Option<&super::owned::Authority<'_>>,
        &dive_check::budget::EvaluationControl,
    ),
) -> Result<Option<Program>, AnalysisError> {
    let (authority, control) = owned_control;
    if !may_apply(plans, options, graph_labels)
        || !authority.is_some_and(|authority| authority.owns(model))
    {
        return Ok(None);
    }
    if crate::budget::exhausted() {
        return Err(AnalysisError::Limit(crate::budget::EXHAUSTED));
    }
    let stability = stable_producers(model, plans, control).map_err(|error| {
        if let Some(stop) = super::evaluator_interruption(control, &error) {
            return stop;
        }
        match error.class() {
            ErrorClass::ResourceLimit => {
                AnalysisError::Limit("canonical producer proof allocation")
            }
            _ => AnalysisError::Inconsistent(
                "canonical producer inspection failed without matching control authority",
            ),
        }
    })?;
    let Some(stability) = stability else {
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
    control: &dive_check::budget::EvaluationControl,
) -> Result<Option<dive_eval::ProducerStability>, EvalError> {
    let mut producers = Vec::new();
    for init in &model.init {
        root(&mut producers, init.pred.expr, init.pred.scope.clone())?;
    }
    if let Some(next) = &model.next {
        root(&mut producers, next.expr, next.scope.clone())?;
    }
    for action in model.init_actions.iter().chain(&model.actions) {
        root(&mut producers, action.expr(), action.scope())?;
    }
    for named in model
        .invariants
        .iter()
        .chain(&model.implied_inits)
        .chain(&model.implied_actions)
        .chain(&model.constraints)
        .chain(&model.action_constraints)
        .chain(model.view.iter())
    {
        root(&mut producers, named.pred.expr, named.pred.scope.clone())?;
    }
    if let Some(alias) = &model.alias {
        let Ok(alias) = alias.as_ref() else {
            return Ok(None);
        };
        root(&mut producers, alias.pred.expr, alias.pred.scope.clone())?;
    }
    let Ok(postconditions) = &model.postconditions else {
        return Ok(None);
    };
    for named in postconditions {
        root(&mut producers, named.pred.expr, named.pred.scope.clone())?;
    }
    for &def in &model.assumptions {
        root(
            &mut producers,
            model.ir.def(def).body,
            model.evaluator.scope(def)?,
        )?;
    }
    let mut predicates = Vec::new();
    for atom in plans.iter().flat_map(|prepared| &prepared.plan.atoms) {
        root(&mut predicates, atom.pred.expr, atom.pred.scope.clone())?;
    }
    let proof = model.evaluator.canonical_producer_stability_budgeted(
        &producers,
        &predicates,
        Some(control),
    )?;
    Ok(proof.filter(|proof| proof.belongs_to(&model.evaluator)))
}
fn root(
    roots: &mut Vec<(dive_ir::ExprId, dive_eval::Scope)>,
    expr: dive_ir::ExprId,
    scope: dive_eval::Scope,
) -> Result<(), EvalError> {
    roots
        .try_reserve(1)
        .map_err(|_| EvalError::resource_limit("canonical producer roots allocation"))?;
    roots.push((expr, scope));
    Ok(())
}

pub(super) struct Cache {
    active: bool,
    rows: Vec<Option<Vec<u64>>>,
    offsets: Vec<usize>,
    width: usize,
}

impl Cache {
    pub(super) fn new(program: &Program) -> Self {
        Self {
            active: true,
            rows: Vec::new(),
            offsets: program.offsets.clone(),
            width: program.slots.len(),
        }
    }

    pub(super) fn active(&self) -> bool {
        self.active
    }

    fn disable(&mut self) {
        self.active = false;
        self.rows = Vec::new();
    }

    pub(super) fn complete(&self, source: usize) -> bool {
        self.active && self.rows.get(source).is_some_and(Option::is_some)
    }

    pub(super) fn read(
        &self,
        property: usize,
        atom: usize,
        source: usize,
    ) -> Result<bool, AnalysisError> {
        if !self.active {
            return Err(AnalysisError::Inconsistent(CACHE));
        }
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
        if !self.active
            || values.len() != self.width
            || values.iter().any(Option::is_none)
            || self.complete(source)
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

fn promotable(model: &Model, failed: &Failed) -> bool {
    if !failed.core
        || failed.cause.class() != ErrorClass::Eval
        || !failed.cause.is_eval_exception()
        || !matches!(
            failed.cause.ec(),
            dive_eval::Ec::DivisionByZero | dive_eval::Ec::ModuleEvaluating
        )
    {
        return false;
    }
    let Some(node) = failed
        .cause
        .origin()
        .and_then(|origin| model.ir.nodes().get(origin.index()))
    else {
        return false;
    };
    node.origin
        .file
        .and_then(|file| model.ir.files().get(file.index()))
        .is_some()
        && node
            .origin
            .instance
            .is_none_or(|def| model.ir.defs().get(def.index()).is_some())
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
        if event.phase() == capture::CheckpointPhase::Init
            || !self
                .cache
                .lock()
                .map_err(|_| EvalError::bug("temporal cache lock poisoned"))?
                .active()
        {
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
                    Err(cause)
                }
            }
        });
        if let Err(cause) = outcome {
            // A semantic refusal cannot change the old label schedule. The
            // private canonical proof makes prior optional work effect/cache
            // stable; abandon its truth without restarting exploration.
            let Some(failed) = self.failed.take() else {
                return Err(cause);
            };
            if promotable(self.model, &failed) {
                self.failed = Some(failed);
                return Err(cause);
            }
            if !matches!(cause.class(), ErrorClass::Eval | ErrorClass::Unsupported) {
                return Err(cause);
            }
            self.cache
                .lock()
                .map_err(|_| EvalError::bug("temporal cache lock poisoned"))?
                .disable();
            return Ok(());
        }
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
        if !cache.active() {
            return Err(AnalysisError::Inconsistent(CACHE));
        }
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
        native_hooks: dive_eval::NativeHooks,
    ) -> Result<capture::ControlledExploration, AnalysisError> {
        let model = self.model;
        let run = {
            let mut forwarded = observer.map(BorrowedObserver);
            let observer = forwarded
                .as_mut()
                .map(|observer| observer as &mut dyn capture::Observer);
            capture::explore_checkpoints_with_native_hooks(
                model,
                options,
                limits,
                observer,
                &mut self,
                native_hooks,
            )
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
