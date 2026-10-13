//! Bounded behavior-graph capture through the safety engine.
//!
//! A complete graph is exploration evidence, not a temporal verdict. This
//! capture path uses the shared ordered safety engines. It retains every accepted
//! generated edge, including duplicates, and adds explicit implicit stutters.

use std::collections::BTreeMap;
use std::sync::Arc;

use dive_eval::AssumptionOutcome;
use dive_eval::next::{StateVals, Successor};
use dive_model::{Model, UnsupportedKind};

use crate::engine;
use crate::{CheckOptions, CheckResult, Collision, Stats, Verdict, Workers};

/// Maximum concurrently expanded source units during parallel capture.
/// Fixed waves use this cap regardless of physical worker pool width.
pub const MAX_EXPANSION_UNITS: usize = 8;

/// Explicit entry-count bounds for retained graph data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum retained graph representatives. The ordinary engine may insert
    /// the deciding state before collector admission fails; stopping distinct
    /// counts can therefore exceed this bound by that state. This is not a
    /// complete seen-set or aggregate heap budget.
    pub states: usize,
    /// Maximum retained original Init occurrences and generated/implicit-stutter
    /// edges combined. Original successor values share their edge's entry.
    pub edges: usize,
    /// Maximum generated states, including duplicates and out-of-model states.
    pub generated: u64,
    /// Maximum emitted successors of one expanded source. This separate scratch
    /// admission bound applies before successor checks in either engine. Parallel
    /// capture buffers at most eight sources at once, regardless of worker count.
    /// TLC setting: no such capture bound; exceeding it is a resource refusal,
    /// never a complete exploration or evidence of TLC agreement.
    pub successors_per_unit: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            states: 1_000_000,
            edges: 8_000_000,
            generated: 32_000_000,
            successors_per_unit: 65_536,
        }
    }
}

/// A generated transition or an implicit full-state stutter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    /// Dense destination state id.
    pub to: usize,
    /// Original successor ordinal, or `None` for the implicit stutter.
    pub ordinal: Option<u64>,
    /// TLC's action label, absent for an implicit stutter.
    pub label: Option<String>,
}

/// An accepted generated successor, before stored representative replacement.
/// This excludes the engine-added stutter.
#[derive(Clone, Debug)]
pub struct ConcreteSuccessor {
    /// Original concrete IR-ordered values, including already seen states.
    pub state: StateVals,
    /// Original source-wide generation ordinal.
    pub ordinal: u64,
    /// Original action label.
    pub label: String,
}

/// An accepted original Init occurrence, including a seen-state occurrence.
#[derive(Clone, Debug)]
pub struct ConcreteInitial {
    /// The stored root identity, without replacing the original values.
    pub to: usize,
    /// Original IR-ordered concrete values.
    pub state: StateVals,
    /// Original Init generation ordinal.
    pub ordinal: u64,
    /// Original Init action label.
    pub label: String,
}

/// Actual retained safety graph completion. Public graph fields, copied
/// statistics and a zero queue cannot construct this producer-owned receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphCompletion {
    states: usize,
    edges: usize,
    concrete_successors: usize,
    roots: usize,
    limits: Limits,
}
impl GraphCompletion {
    fn finalize(graph: &BehaviorGraph, counters: Stats, limits: Limits) -> Result<Self, Verdict> {
        let states = graph.states.len();
        if states > limits.states || counters.generated > limits.generated {
            return Err(limit("completion entries"));
        }
        if counters.distinct != u64::try_from(states).map_err(|_| bug("state count range"))?
            || counters.left_on_queue != 0
            || graph.edges.len() != states
            || graph.concrete.len() != states
            || (!graph.states.is_empty() && graph.roots.is_empty())
        {
            return Err(bug("incomplete graph/statistics at successful EOF"));
        }
        let mut previous = None;
        for &root in &graph.roots {
            if root >= states || previous.is_some_and(|p| p >= root) {
                return Err(bug("invalid completed initial roots"));
            }
            previous = Some(root);
        }
        let mut initial_roots = vec![false; states];
        for initial in &graph.initials {
            let Some(present) = initial_roots.get_mut(initial.to) else {
                return Err(bug("original initial identity outside graph"));
            };
            if graph.roots.binary_search(&initial.to).is_err() {
                return Err(bug("original initial identity is not a root"));
            }
            *present = true;
        }
        if graph.initials.len() as u64 > counters.initial_generated
            || graph.roots.iter().any(|root| !initial_roots[*root])
        {
            return Err(bug("inconsistent original initial occurrences"));
        }
        // Original Init occurrences share the existing graph-entry bound.
        let mut retained = graph.initials.len();
        if retained > limits.edges {
            return Err(limit("completion initial occurrences"));
        }
        let mut edges = 0usize;
        let mut concrete_successors = 0usize;
        // The already-admitted retention limits bound this structural scan.
        // No values are hashed, normalized, compared or evaluated here.
        for (source, (row, concrete)) in graph.edges.iter().zip(&graph.concrete).enumerate() {
            retained = retained
                .checked_add(row.len())
                .filter(|&n| n <= limits.edges)
                .ok_or_else(|| limit("completion retained occurrences"))?;
            edges = edges
                .checked_add(row.len())
                .filter(|&n| n <= limits.edges)
                .ok_or_else(|| limit("completion edges"))?;
            concrete_successors = concrete_successors
                .checked_add(concrete.len())
                .ok_or_else(|| limit("completion concrete successors"))?;
            if row.len()
                != concrete
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| limit("completion row"))?
                || row
                    .last()
                    .is_none_or(|e| e.to != source || e.ordinal.is_some() || e.label.is_some())
            {
                return Err(bug("missing completed source/stutter row"));
            }
            for (edge, original) in row.iter().zip(concrete) {
                if edge.to >= states
                    || edge.ordinal != Some(original.ordinal)
                    || edge.label.as_deref() != Some(original.label.as_str())
                {
                    return Err(bug("inconsistent completed concrete transition"));
                }
            }
        }
        Ok(Self {
            states,
            edges,
            concrete_successors,
            roots: graph.roots.len(),
            limits,
        })
    }
    /// Retained representatives, generated edges plus implicit stutters,
    /// original concrete successors, and distinct initial roots, respectively.
    #[must_use]
    pub fn entries(&self) -> (usize, usize, usize, usize) {
        (
            self.states,
            self.edges,
            self.concrete_successors,
            self.roots,
        )
    }
    /// Actual supplied capture entry limits; these are not aggregate byte bounds.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }
}

/// A coordinator consumer's decision at a completed BFS level.
/// Stopping retains a prefix and never certifies exhaustive exploration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LevelControl {
    /// Continue exploring the next frontier.
    #[default]
    Continue,
    /// Stop after this completed level, without evaluating postconditions.
    Stop,
}

/// Receives committed initialization and completed BFS levels on the coordinator.
///
/// All methods are inert by default. Initial events follow successful Init;
/// edge events follow a fully committed level, in source and generator order.
/// A failed or partial level emits no edge or level event. Callback errors stop
/// exploration without a completed graph. These hooks establish no temporal verdict.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::{capture::{self, BehaviorGraph, Observer}, CheckOptions, Stats};
/// #[derive(Default)]
/// struct Levels(usize);
/// impl Observer for Levels {
///     fn level_done(&mut self, _: u64, _: &BehaviorGraph, _: Stats) -> Result<(), String> {
///         self.0 += 1;
///         Ok(())
///     }
/// }
/// let analysis = dive_sema::analyze_source("M.tla",
///     "---- MODULE M ----\nVARIABLE x\nInit == x=TRUE\nNext == x'=x\n====\n",
///     &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().expect("config");
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
/// let mut levels = Levels::default();
/// let result = capture::explore_observed_with_evidence(
///     &model, &CheckOptions::tlc(0, 7), Default::default(), &mut levels)?;
/// assert!(matches!(result, capture::Exploration::Complete(_)));
/// assert_eq!(levels.0, 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait Observer: Send {
    /// A distinct accepted initial state after every Init check succeeded.
    /// Counters describe the complete initialization, including excluded candidates.
    ///
    /// # Errors
    /// Return a diagnostic to stop exploration at the initialization boundary.
    fn init(&mut self, _id: usize, _state: &StateVals, _stats: Stats) -> Result<(), String> {
        Ok(())
    }

    /// An accepted transition or implicit stutter from a completed level.
    /// `successor` is the original concrete state, including seen-state occurrences.
    /// Counters include all generated candidates at the completed level.
    ///
    /// # Errors
    /// Return a diagnostic to stop exploration at this completed-level boundary.
    fn edge(
        &mut self,
        _from: usize,
        _edge: &Edge,
        _successor: &StateVals,
        _stats: Stats,
    ) -> Result<(), String> {
        Ok(())
    }

    /// A fully committed level and its borrowed graph, after frontier append.
    /// Unexpanded frontier states have empty outgoing rows. A prefix without a
    /// violation does not prove a temporal property; exhaustion is still required.
    ///
    /// # Errors
    /// Return a diagnostic to stop exploration at this completed-level boundary.
    fn level_done(
        &mut self,
        _level: u64,
        _graph: &BehaviorGraph,
        _stats: Stats,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Controls exploration after all edge and level callbacks succeeded.
    /// Errors and panics remain failures, distinct from a deliberate stop.
    ///
    /// # Errors
    /// Return a diagnostic when this consumer cannot complete its work.
    fn level_control(
        &mut self,
        _level: u64,
        _graph: &BehaviorGraph,
        _stats: Stats,
    ) -> Result<LevelControl, String> {
        Ok(LevelControl::Continue)
    }
}

/// A fully explored behavior graph in the engine's discovery order.
/// States use the bound model's IR variable order. Fingerprint-mode identity
/// has the same collision qualifications as the safety engine.
#[derive(Clone, Debug)]
pub struct BehaviorGraph {
    /// Concrete state representatives, including all variable assignments.
    pub states: Vec<StateVals>,
    /// Outgoing edges in generator order, then the implicit stutter.
    pub edges: Vec<Vec<Edge>>,
    /// Accepted original generated successors in source-wide generator order.
    /// Rows include duplicates and seen successors, and exclude implicit stutter.
    /// Their concrete values must not be reconstructed from `states[edge.to]`.
    pub concrete: Vec<Vec<ConcreteSuccessor>>,
    /// Distinct in-model initial states in discovery order.
    pub roots: Vec<usize>,
    /// Accepted original Init occurrences in generator order, including aliases.
    /// Retention shares the existing graph-edge/occurrence entry bound.
    pub initials: Vec<ConcreteInitial>,
}

/// Evidence from a completed safety exploration. Standard capture does not
/// evaluate temporal predicates. An observed capture can evaluate caller-owned
/// predicates, but this evidence never certifies temporal acceptance.
#[derive(Clone, Debug)]
pub struct Complete {
    safety_result: Box<CheckResult>,
    completion: GraphCompletion,
    /// Run settings, including the reported seed, profile and store mode.
    pub options: CheckOptions,
    /// The complete graph, including seen-state edges and implicit stutters.
    pub graph: BehaviorGraph,
    /// The shared engine's counters. Implicit stutters do not inflate them.
    pub stats: Stats,
    /// The safety engine's collision figures, absent for exact mode.
    pub collision: Option<Collision>,
    /// Requested and actual execution pool widths. Fixed waves contain at most
    /// eight source states; a smaller frontier can use fewer concurrent units.
    pub workers: Workers,
    /// Start-up, assumption and search output in order.
    pub output: Vec<String>,
    /// Binding warnings, retaining their TLC message codes.
    pub warnings: Vec<(u32, String)>,
}

impl Complete {
    /// Immutable producer completion fact, independent of later edits to the
    /// publicly inspectable graph or counters. Edited fields cannot recertify it.
    #[must_use]
    pub fn completion(&self) -> &GraphCompletion {
        &self.completion
    }
    /// Actual ordinary safety-engine outcome, retained without reconstructing
    /// counts, diagnostics, stopping facts or worker metadata from the graph.
    /// This receipt does not certify any temporal obligation.
    #[must_use]
    pub fn safety_result(&self) -> &CheckResult {
        &self.safety_result
    }
}

/// Prepares an actual replayed occurrence using ordinary capture's exact or
/// fingerprint identity kernel, discarding the key. This may perform the same
/// set normalization as producer key preparation; constructor classes remain
/// intact. It never inserts a state, changes counters, invokes observers or
/// grants capture/host authority. VIEW/SYMMETRY and foreign profiles refuse.
///
/// `options` must be the actual producer options. `work_limit` bounds
/// callback-free transport and finite value expansion; the returned charge
/// must be deducted from the consumer's remaining work budget.
///
/// # Errors
/// Context/profile mismatch, unavailable callback-free bounded preparation,
/// or the original typed value-key error.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::{CheckOptions, capture::prepare_replay_identity};
/// let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\n\
///               Init == x=0\nNext == x'=x\n====\n";
/// let ir = Arc::new(dive_ir::lower(&dive_sema::analyze_source(
///     "M.tla", source, &Default::default()))?);
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().ok_or("invalid config")?;
/// let options = CheckOptions::dive(7);
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions {
///     eval: options.eval, no_deadlock: false, host: None,
/// })?;
/// let original = Arc::from([Some(dive_value::Value::Int(0))]);
/// let charged = prepare_replay_identity(&model, &options, &original, 1000)?;
/// assert!(charged <= 1000);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[allow(
    clippy::too_many_lines,
    reason = "keep shared identity guards and callback-free work admission together"
)]
pub fn prepare_replay_identity(
    model: &Model,
    options: &CheckOptions,
    state: &StateVals,
    work_limit: usize,
) -> Result<usize, dive_eval::EvalError> {
    use dive_eval::EvalError;
    use dive_value::{SetKind, Value, codec};
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|error| EvalError::unsupported(error.to_string()))?;
    model
        .validate_context()
        .map_err(|error| EvalError::unsupported(error.to_string()))?;
    if model.view.is_some()
        || model.symmetry.is_some()
        || model.evaluator.options() != &options.eval
    {
        return Err(EvalError::unsupported(
            "concrete identity preparation with VIEW/SYMMETRY or foreign profile",
        ));
    }
    let mut charge = 0usize;
    let mut pending = Vec::new();
    for value in state.iter() {
        let value = value
            .as_ref()
            .ok_or_else(|| EvalError::bug("incomplete identity occurrence"))?;
        let remaining = work_limit
            .checked_sub(charge)
            .ok_or_else(|| EvalError::resource_limit("concrete identity preparation work"))?;
        let bound = codec::frontier_size_bound(value, remaining).map_err(|error| {
            if error.kind() == dive_value::ErrorKind::DiveLimit {
                EvalError::resource_limit(error.message())
            } else {
                EvalError::unsupported("concrete identity preparation requires deferred callbacks")
            }
        })?;
        charge = charge
            .checked_add(bound)
            .ok_or_else(|| EvalError::resource_limit("concrete identity preparation work"))?;
        pending.push(value.clone());
    }
    while let Some(value) = pending.pop() {
        match value {
            Value::Fcn(function) => {
                pending.extend(
                    function
                        .iter()
                        .flat_map(|(key, value)| [key, value.clone()]),
                );
            }
            Value::Set(set) => {
                let elements = if matches!(set.kind(), SetKind::Enum | SetKind::Raw) {
                    Some(set.elements(options.eval.ctx)?)
                } else {
                    None
                };
                // Infinite classes reach the actual key error without an
                // invented cardinality operation. Finite lazy expansion is
                // bounded before the shared key kernel can allocate it.
                if !matches!(
                    set.kind(),
                    SetKind::Nat | SetKind::Int | SetKind::Strings | SetKind::Seq
                ) {
                    let count = usize::try_from(set.size(options.eval.ctx)?).map_err(|_| {
                        EvalError::resource_limit("concrete identity expansion work")
                    })?;
                    let cost = count
                        .checked_mul(codec::frontier_size_bound(
                            &Value::Set(set.clone()),
                            work_limit,
                        )?)
                        .ok_or_else(|| {
                            EvalError::resource_limit("concrete identity expansion work")
                        })?;
                    charge = charge
                        .checked_add(cost)
                        .filter(|n| *n <= work_limit)
                        .ok_or_else(|| {
                            EvalError::resource_limit("concrete identity expansion work")
                        })?;
                }
                pending.extend(set.components().into_iter().map(|(_, value)| value));
                if let Some(elements) = elements {
                    pending.extend(elements.iter().cloned());
                }
            }
            _ => {}
        }
    }
    engine::prepare_identity(model, options, state)?;
    Ok(charge)
}

/// Actual search evidence at a deliberate completed-level stop.
/// This cannot be used as a [`Complete`] or as evidence that properties hold.
#[derive(Clone, Debug)]
pub struct Prefix {
    /// The completed level at which the consumer stopped.
    pub level: u64,
    /// Run settings, including store, seed and profile.
    pub options: CheckOptions,
    /// Committed graph; unexpanded frontier rows may be empty.
    pub graph: BehaviorGraph,
    /// Actual completed-boundary counters, including the remaining frontier.
    pub stats: Stats,
    /// Fingerprint collision qualification, absent for exact storage.
    pub collision: Option<Collision>,
    /// Requested and actual workers.
    pub workers: Workers,
    /// Startup and exploration output in execution order.
    pub output: Vec<String>,
    /// Startup warnings with stable TLC codes.
    pub warnings: Vec<(u32, String)>,
    /// Actual capture time through this boundary.
    pub elapsed: std::time::Duration,
}

fn elapsed(result: &mut Exploration, duration: std::time::Duration) {
    match result {
        Exploration::Stopped(stop) => stop.elapsed = duration,
        Exploration::Complete(complete) => complete.safety_result.elapsed = duration,
        Exploration::Prefix(prefix) => prefix.elapsed = duration,
    }
}

/// Graph capture cannot turn an incomplete or unsupported search into success.
#[derive(Clone, Debug)]
pub enum Exploration {
    /// Complete safety exploration; explicitly not a temporal verdict.
    Complete(Box<Complete>),
    /// A deliberate completed-level consumer stop; never an exhaustive graph.
    Prefix(Box<Prefix>),
    /// A safety stop, unsupported feature, resource bound or internal failure.
    /// No partial graph is exposed as complete.
    Stopped(Box<CheckResult>),
}

pub(crate) struct Collector<K> {
    ids: BTreeMap<K, usize>,
    graph: BehaviorGraph,
    limits: Limits,
    count: usize,
    concrete_pool: bool,
}

impl<K: Ord> Collector<K> {
    pub(crate) fn new(limits: Limits) -> Self {
        Self {
            ids: BTreeMap::new(),
            graph: BehaviorGraph {
                states: vec![],
                edges: vec![],
                concrete: vec![],
                roots: vec![],
                initials: vec![],
            },
            limits,
            count: 0,
            concrete_pool: false,
        }
    }

    pub(crate) fn with_concrete_pool(mut self, concrete_pool: bool) -> Self {
        self.concrete_pool = concrete_pool;
        self
    }

    pub(crate) fn concrete_pool(&self) -> bool {
        self.concrete_pool
    }

    pub(crate) fn graph(&self) -> &BehaviorGraph {
        &self.graph
    }

    pub(crate) fn limits(&self) -> Limits {
        self.limits
    }

    pub(crate) fn allow_emitted(&self, n: u64, ordinal: Option<u64>) -> Result<(), Verdict> {
        self.allow_generated(n)?;
        if ordinal.is_some_and(|ordinal| ordinal >= self.limits.successors_per_unit) {
            Err(limit("successors per source (scratch admission)"))
        } else {
            Ok(())
        }
    }

    pub(crate) fn state(
        &mut self,
        key: K,
        vals: StateVals,
        id: Option<u64>,
    ) -> Result<usize, Verdict> {
        if let Some(id) = id {
            let id = usize::try_from(id).map_err(|_| bug("state id exceeds address space"))?;
            if id != self.graph.states.len() || self.ids.contains_key(&key) {
                return Err(bug("graph discovery ids differ from trace log"));
            }
            if id >= self.limits.states {
                return Err(limit("states"));
            }
            self.ids.insert(key, id);
            self.graph.states.push(vals);
            self.graph.edges.push(vec![]);
            self.graph.concrete.push(vec![]);
            Ok(id)
        } else {
            self.ids
                .get(&key)
                .copied()
                .ok_or_else(|| bug("seen graph state has no discovery id"))
        }
    }

    pub(crate) fn edge(&mut self, from: u64, edge: Edge) -> Result<(), Verdict> {
        if self.count >= self.limits.edges {
            return Err(limit("edges"));
        }
        let from = usize::try_from(from).map_err(|_| bug("edge source exceeds address space"))?;
        let row = self
            .graph
            .edges
            .get_mut(from)
            .ok_or_else(|| bug("edge source was not discovered"))?;
        row.push(edge);
        self.count += 1;
        Ok(())
    }

    /// Derive per-source speculative retention quotas independent of worker
    /// width. Reserve every implicit stutter before starting the worker pool.
    pub(crate) fn quotas(&self, sources: usize, generated: u64) -> Result<(usize, u64), Verdict> {
        let edges = self
            .limits
            .edges
            .checked_sub(self.count)
            .and_then(|n| n.checked_sub(sources))
            .ok_or_else(|| limit("implicit stutters"))?;
        let sources_u64 = u64::try_from(sources).map_err(|_| limit("source count"))?;
        let remaining = self
            .limits
            .generated
            .checked_sub(generated)
            .ok_or_else(|| limit("generated states"))?;
        Ok((edges / sources, remaining / sources_u64))
    }

    pub(crate) fn allow_generated(&self, n: u64) -> Result<(), Verdict> {
        if n > self.limits.generated {
            Err(limit("generated states"))
        } else {
            Ok(())
        }
    }

    pub(crate) fn record(
        &mut self,
        key: K,
        vals: StateVals,
        id: Option<u64>,
        parent: Option<u64>,
        succ: &Successor<'_>,
    ) -> Result<(), Verdict> {
        self.record_fields(key, vals, id, parent, succ.ordinal, succ.action.label())
    }

    pub(crate) fn record_fields(
        &mut self,
        key: K,
        vals: StateVals,
        id: Option<u64>,
        parent: Option<u64>,
        ordinal: u64,
        label: &str,
    ) -> Result<(), Verdict> {
        let concrete = vals.clone();
        let target = self.state(key, vals, id)?;
        if let Some(from) = parent {
            self.edge(
                from,
                Edge {
                    to: target,
                    ordinal: Some(ordinal),
                    label: Some(label.to_owned()),
                },
            )?;
            // edge() charges the retained-entry bound before this clone is
            // retained. The original source value survives identity deduplication.
            let from = usize::try_from(from).map_err(|_| bug("source exceeds address space"))?;
            let row = self
                .graph
                .concrete
                .get_mut(from)
                .ok_or_else(|| bug("concrete edge source was not discovered"))?;
            row.push(ConcreteSuccessor {
                state: concrete,
                ordinal,
                label: label.to_owned(),
            });
        } else {
            if self.count >= self.limits.edges {
                return Err(limit("original initial occurrences"));
            }
            self.root(target);
            self.count += 1;
            // Shared entry admission is charged before this original is retained.
            // The receipt changes neither identity nor generated counters.
            self.graph.initials.push(ConcreteInitial {
                to: target,
                state: concrete,
                ordinal,
                label: label.to_owned(),
            });
        }
        Ok(())
    }

    pub(crate) fn stutter(&mut self, from: u64) -> Result<(), Verdict> {
        let to = usize::try_from(from).map_err(|_| bug("stutter id exceeds address space"))?;
        self.edge(
            from,
            Edge {
                to,
                ordinal: None,
                label: None,
            },
        )
    }

    pub(crate) fn root(&mut self, id: usize) {
        if let Err(at) = self.graph.roots.binary_search(&id) {
            self.graph.roots.insert(at, id);
        }
    }

    pub(crate) fn finish(self) -> BehaviorGraph {
        self.graph
    }
}

fn bug(s: &str) -> Verdict {
    Verdict::InternalError {
        message: format!("graph capture: {s}"),
    }
}
pub(crate) fn limit(s: &str) -> Verdict {
    Verdict::ResourceLimit {
        message: format!("behavior graph capture {s}"),
    }
}

/// Explores a freshly bound model through the shared safety engine. The
/// model must use the same evaluator options; callers must not concurrently
/// evaluate it or its clones. This installs the run host, checks assumptions,
/// and includes the model's recorded startup output.
///
/// Temporal property refusals are deferred explicitly; every other refusal
/// is retained. VIEW, symmetry, randomness and stateful checker reads remain
/// unsupported. Capture uses ordered, bounded source waves with the requested
/// pool width, or the explicitly requested sequential mode. The ordinary
/// safety checker is unchanged.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::{capture::{explore, Exploration, Limits}, CheckOptions};
/// let source = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
/// let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().expect("config");
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
/// let Exploration::Complete(c) = explore(&model, &CheckOptions::tlc(0, 0), Limits::default()) else {
///     panic!("the supported finite model must explore completely");
/// };
/// assert_eq!(c.graph.states.len(), 1);
/// assert_eq!(c.graph.edges[0].len(), 2); // generated step and implicit stutter
/// assert_eq!(c.options.seed, 0);
/// ```
#[must_use]
pub fn explore(model: &Model, opts: &CheckOptions, limits: Limits) -> Exploration {
    let started = std::time::Instant::now();
    explore_with_evidence(model, opts, limits).unwrap_or_else(|message| {
        let mut result = stopped(bug(&message), model, opts);
        if let Exploration::Stopped(stop) = &mut result {
            stop.elapsed = started.elapsed();
        }
        result
    })
}

/// Like [`explore`], distinguishing a thread failure that lost execution
/// evidence from an observed safety stop. Unknown work must not be reported
/// as a proved zero-worker search.
///
/// # Errors
/// The exploration thread failed to start or panicked; actual work is unknown.
pub fn explore_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
) -> Result<Exploration, String> {
    let started = std::time::Instant::now();
    if let Some(stopped) = context_stop(model, opts) {
        return Ok(stopped);
    }
    let run = || explore_inner(model, opts, limits, false, None);
    let mut result = run_on_thread(run)?;
    elapsed(&mut result, started.elapsed());
    Ok(result)
}

/// Capture accepted concrete successors through the actual deterministic
/// safety worker pool, preserving original seen-state values before replacement.
/// This supplies exploration evidence only; it evaluates no temporal predicates.
///
/// Per-level speculative quotas divide the remaining entry/generation bounds by
/// source count, independently of worker width. These conservative named bounds
/// may refuse a skewed level that fits the inline capture's global budget.
/// VIEW, symmetry, randomness and checker-state reads remain named refusals.
/// The staged pool has a fixed structural maximum of 64 workers.
///
/// Captured operator/function/lazy-set values are refused by a conservative
/// no-evaluation scoped proof; no provider is forced to guess purity.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::{capture, CheckOptions};
/// let source = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\n====\n";
/// let a = dive_sema::analyze_source("M.tla", source, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().expect("config");
/// let m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
/// let options = CheckOptions { workers: 4, ..CheckOptions::tlc(0, 0) };
/// let result = capture::explore_concrete_parallel_with_evidence(&m, &options, Default::default())?;
/// let capture::Exploration::Complete(c) = result else { return Err("incomplete".into()) };
/// assert_eq!(c.workers.used, 4);
/// assert_eq!(c.graph.concrete[0].len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// A worker or exploration thread fails and loses execution evidence.
pub fn explore_concrete_parallel_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
) -> Result<Exploration, String> {
    if let Some(stopped) = context_stop(model, opts) {
        return Ok(stopped);
    }
    if opts.workers > 64 {
        let mut result = crate::bare(
            limit("concrete pool worker width exceeds 64"),
            opts,
            &model.ir,
        );
        result.workers.requested = opts.workers;
        return Ok(Exploration::Stopped(Box::new(result)));
    }
    if opts.workers == 0 {
        let mut result = crate::bare(
            Verdict::Unsupported {
                feature: "concrete parallel capture worker width is zero".into(),
            },
            opts,
            &model.ir,
        );
        result.workers.requested = 0;
        return Ok(Exploration::Stopped(Box::new(result)));
    }
    let started = std::time::Instant::now();
    let mut result = run_on_thread(|| explore_inner(model, opts, limits, true, None))?;
    elapsed(&mut result, started.elapsed());
    Ok(result)
}

/// Capture and observe successful initialization and completed levels.
/// Both the inline and ordered pool executors deliver the same events at the
/// requested width, preserving the normal capture admission and worker planning. Failure
/// stops exploration without exposing a completed graph; no temporal verdict is implied.
///
/// # Errors
/// An exploration thread or worker fails, losing actual execution evidence.
pub fn explore_observed_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: &mut dyn Observer,
) -> Result<Exploration, String> {
    if let Some(stopped) = context_stop(model, opts) {
        return Ok(stopped);
    }
    let started = std::time::Instant::now();
    let mut result = run_on_thread(|| explore_inner(model, opts, limits, false, Some(observer)))?;
    elapsed(&mut result, started.elapsed());
    Ok(result)
}

fn run_on_thread(run: impl FnOnce() -> Exploration + Send) -> Result<Exploration, String> {
    std::thread::scope(|scope| {
        match std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, run)
        {
            Ok(thread) => thread
                .join()
                .map_err(|_| "exploration thread panicked".to_owned()),
            Err(e) => Err(format!("cannot start exploration thread: {e}")),
        }
    })
}

fn context_stop(model: &Model, opts: &CheckOptions) -> Option<Exploration> {
    if let Err(error) = model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
    {
        // No binding output belongs to a foreign arena/name table. Return a
        // checked no-work report before reading any model roles or starting host effects.
        return Some(Exploration::Stopped(Box::new(crate::bare(
            crate::context_verdict(&error),
            opts,
            &model.ir,
        ))));
    }
    if let Err(error) = model.validate_context() {
        let verdict = if error.class() == dive_eval::ErrorClass::ResourceLimit {
            Verdict::ResourceLimit {
                message: error.to_string(),
            }
        } else {
            Verdict::Unsupported {
                feature: error.to_string(),
            }
        };
        return Some(Exploration::Stopped(Box::new(crate::bare(
            verdict, opts, &model.ir,
        ))));
    }
    if opts.seen_disk.is_some() {
        return Some(stopped(Verdict::Unsupported { feature: "disk seen-state retention with behavior graph/source capture is not implemented".into() }, model, opts));
    }
    None
}

#[allow(
    clippy::too_many_lines,
    reason = "keep admission, startup, observer dispatch and failed execution evidence in order"
)]
fn explore_inner(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    parallel: bool,
    observer: Option<&mut dyn Observer>,
) -> Exploration {
    let refuse = |s: String| stopped(Verdict::Unsupported { feature: s }, model, opts);
    if opts.frontier_memory_bytes.is_some() {
        return refuse("frontier spill with behavior graph capture is not yet certified".into());
    }
    if opts.predecessor_memory_bytes.is_some() || opts.predecessor_directory.is_some() {
        return refuse("predecessor spill with behavior graph capture is not implemented".into());
    }
    if parallel
        && model
            .evaluator
            .validate_model_context(&model.ir, &model.names)
            .is_err()
    {
        return refuse("concrete pool capture foreign IR or name table".into());
    }
    if model.evaluator.options() != &opts.eval {
        return refuse("graph capture evaluator profile mismatch".into());
    }
    if model.view.is_some() || model.symmetry.is_some() {
        return refuse("graph capture with VIEW or SYMMETRY".into());
    }
    match engine::bound_configured_random(model) {
        Ok(true) => {
            return refuse("graph capture with configured native random replacements".into());
        }
        Err(error) => return refuse(format!("graph capture invalid native authority: {error}")),
        Ok(false) => {}
    }
    if engine::uses_random(&model.ir) {
        return refuse("graph capture with random operators".into());
    }
    if !model.sequential_names_only
        && let Some(s) = model
            .sequential
            .iter()
            .find(|reason| reason.as_str() != dive_model::TLC_CDOT)
    {
        return refuse(format!("graph capture with stateful checking: {s}"));
    }
    if let Some(u) = model
        .unsupported
        .iter()
        .find(|u| u.kind != UnsupportedKind::TemporalProperty)
    {
        return refuse(u.feature.clone());
    }
    if parallel && (!model.sequential.is_empty() || crate::effectful(model)) {
        return refuse("concrete pool capture with effects or runtime names".into());
    }
    let host = Arc::new(engine::run_host(opts));
    model
        .evaluator
        .set_host(Arc::new(crate::host::EngineHost::new(host.clone())));
    model.evaluator.intern_checker_names();
    let config = engine::run_config(opts, model.settings.check_deadlock);
    model.evaluator.intern_bfs_config(&config);
    host.set_config(Some(config));
    engine::startup_counters(&host);
    let mut result = crate::bare(Verdict::Success, opts, &model.ir);
    crate::messages(&model.output, &mut result);
    let assumptions = model.check_assumptions();
    result.output.extend(model.take_output());
    match assumptions {
        AssumptionOutcome::AllTrue => {}
        AssumptionOutcome::False { message, .. } => {
            result.verdict = Verdict::AssumptionFalse { message };
            return Exploration::Stopped(Box::new(result));
        }
        AssumptionOutcome::Error { error, message, .. } => {
            result.verdict = crate::class::assumption_error(&error, message);
            return Exploration::Stopped(Box::new(result));
        }
    }
    let outcome = engine::search(
        model,
        opts,
        &host,
        engine::SearchHooks {
            capture: Some((limits, parallel)),
            observer,
            ..Default::default()
        },
    );
    result.frontier = outcome.frontier;
    result.predecessor = outcome.predecessor;
    result.verdict = outcome.verdict;
    result.stats = outcome.stats;
    result.tlc_prints_stats = outcome.tlc_prints_stats;
    result.counterexample = outcome.counterexample;
    result.collision = outcome.collision;
    result.workers = outcome.workers;
    result.output.extend(outcome.output);
    result.notes = outcome.notes;
    crate::note_canonical_stop(opts, &result.verdict, &mut result.notes);
    if let Some(level) = outcome.boundary {
        if !result.frontier.failures.is_empty() || !result.predecessor.failures.is_empty() {
            return Exploration::Stopped(Box::new(result));
        }
        let (Some(graph), Some(stats)) = (outcome.graph, result.stats) else {
            result.verdict = bug("observer boundary did not retain graph/statistics");
            return Exploration::Stopped(Box::new(result));
        };
        return Exploration::Prefix(Box::new(Prefix {
            level,
            options: opts.clone(),
            graph,
            stats,
            collision: result.collision,
            workers: result.workers,
            output: result.output,
            warnings: result.warnings,
            elapsed: std::time::Duration::ZERO,
        }));
    }
    if result.verdict != Verdict::Success {
        return Exploration::Stopped(Box::new(result));
    }
    let (Some(graph), Some(stats)) = (outcome.graph, result.stats) else {
        result.verdict = bug("completed exploration did not return graph/statistics");
        return Exploration::Stopped(Box::new(result));
    };
    let completion = match GraphCompletion::finalize(&graph, stats, limits) {
        Ok(completion) => completion,
        Err(verdict) => {
            result.verdict = verdict;
            return Exploration::Stopped(Box::new(result));
        }
    };
    Exploration::Complete(Box::new(Complete {
        completion,
        safety_result: Box::new(result.clone()),
        options: opts.clone(),
        graph,
        stats,
        collision: result.collision,
        workers: result.workers,
        output: result.output,
        warnings: result.warnings,
    }))
}

fn stopped(verdict: Verdict, model: &Model, opts: &CheckOptions) -> Exploration {
    let mut result = crate::bare(verdict, opts, &model.ir);
    crate::messages(&model.output, &mut result);
    Exploration::Stopped(Box::new(result))
}

#[cfg(test)]
#[allow(clippy::panic, reason = "controlled internal thread failure")]
mod evidence_controls {
    use super::{BehaviorGraph, ConcreteInitial, Edge, GraphCompletion, Limits};
    use crate::Stats;
    #[test]
    fn completed_graph_requires_actual_rows_roots_stutters_and_statistics() {
        let graph = BehaviorGraph {
            states: vec![Vec::new().into()],
            edges: vec![vec![Edge {
                to: 0,
                ordinal: None,
                label: None,
            }]],
            concrete: vec![vec![]],
            roots: vec![0],
            initials: vec![ConcreteInitial {
                to: 0,
                state: Vec::new().into(),
                ordinal: 0,
                label: "Init".into(),
            }],
        };
        let stats = Stats {
            generated: 1,
            distinct: 1,
            initial_generated: 1,
            depth: 1,
            left_on_queue: 0,
        };
        let limits = Limits::default();
        assert!(GraphCompletion::finalize(&graph, stats, limits).is_ok());
        for edit in 0..7 {
            let mut bad = graph.clone();
            match edit {
                0 => bad.edges.clear(),
                1 => bad.edges[0].clear(),
                2 => bad.edges[0][0].to = 1,
                3 => bad.concrete.clear(),
                4 => bad.roots.clear(),
                5 => bad.roots.push(0),
                _ => bad.roots[0] = 1,
            }
            assert!(GraphCompletion::finalize(&bad, stats, limits).is_err());
        }
        for stats in [
            Stats {
                distinct: 2,
                ..stats
            },
            Stats {
                left_on_queue: 1,
                ..stats
            },
        ] {
            assert!(GraphCompletion::finalize(&graph, stats, limits).is_err());
        }
        assert!(GraphCompletion::finalize(&graph, stats, Limits { edges: 0, ..limits }).is_err());
        let empty = BehaviorGraph {
            states: vec![],
            edges: vec![],
            concrete: vec![],
            roots: vec![],
            initials: vec![],
        };
        assert!(GraphCompletion::finalize(&empty, Stats::default(), limits).is_ok());
    }
    #[test]
    fn panic_does_not_fabricate_a_zero_worker_stop() {
        let result = super::run_on_thread(|| panic!("controlled capture panic"));
        assert!(matches!(result, Err(message) if message == "exploration thread panicked"));
    }
}
