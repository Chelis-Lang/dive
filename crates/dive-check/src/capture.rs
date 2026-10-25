//! Bounded behavior-graph capture through the safety engine.
//!
//! A complete graph is exploration evidence, not a temporal verdict. This
//! capture path uses the shared ordered safety engines. It retains every accepted
//! generated edge, including duplicates, and adds explicit implicit stutters.

use std::collections::BTreeMap;
use std::sync::Arc;

use dive_eval::next::{StateVals, Successor};
use dive_model::{Model, UnsupportedKind};

use crate::engine;
use crate::{CheckOptions, CheckResult, Collision, Stats, Verdict, Workers};

mod checkpoints;
pub(crate) use checkpoints::Hooks as CheckpointHooks;
pub use checkpoints::{
    CheckpointDecision, CheckpointExploration, CheckpointObserver, CheckpointPhase,
    ReachedCheckpoint, explore_checkpoints_with_evidence, explore_checkpoints_with_native_hooks,
};

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
    /// Maximum generated and implicit-stutter edges combined.
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
        let mut edges = 0usize;
        let mut concrete_successors = 0usize;
        // The already-admitted retention limits bound this structural scan.
        // No values are hashed, normalized, compared or evaluated here.
        for (source, (row, concrete)) in graph.edges.iter().zip(&graph.concrete).enumerate() {
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
    /// Requests a prefix stop after a successful completed-level callback.
    /// Use [`explore_controlled_with_evidence`]; exhausted frontiers cannot stop
    /// as prefixes. The default preserves all existing observation behavior.
    fn stop_after_level(&self) -> bool {
        false
    }

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

/// Actual committed-level exploration prefix, explicitly not completion.
#[derive(Clone, Debug)]
pub struct Prefix {
    level: u64,
    result: Box<CheckResult>,
    /// Original run settings.
    pub options: CheckOptions,
    /// Retained graph; unexpanded rows remain empty.
    pub graph: BehaviorGraph,
    /// Actual counters at the committed boundary.
    pub stats: Stats,
    /// Fingerprint qualifications, absent for exact storage.
    pub collision: Option<Collision>,
    /// Actual requested and used workers after joined work.
    pub workers: Workers,
    /// Startup and exploration output in order.
    pub output: Vec<String>,
    /// Actual startup warnings.
    pub warnings: Vec<(u32, String)>,
}
impl Prefix {
    /// Last fully committed expansion level. No complete verdict is implied.
    #[must_use]
    pub const fn level(&self) -> u64 {
        self.level
    }
}

/// Controlled capture distinguishes a real prefix from completed exploration.
#[derive(Clone, Debug)]
pub enum ControlledExploration {
    /// Actual complete exploration, without a temporal verdict.
    Complete(Box<Complete>),
    /// Requested stop after a fully committed, unexhausted level.
    Prefix(Box<Prefix>),
    /// Safety, resource or unsupported stop.
    Stopped(Box<CheckResult>),
}
impl From<Exploration> for ControlledExploration {
    fn from(value: Exploration) -> Self {
        match value {
            Exploration::Complete(c) => Self::Complete(c),
            Exploration::Stopped(s) => Self::Stopped(s),
        }
    }
}
impl ControlledExploration {
    fn legacy(self) -> Exploration {
        match self {
            Self::Complete(c) => Exploration::Complete(c),
            Self::Stopped(s) => Exploration::Stopped(s),
            Self::Prefix(p) => {
                let mut result = p.result;
                result.verdict = Verdict::Unsupported {
                    feature: "prefix stop requires controlled capture entrypoint".into(),
                };
                Exploration::Stopped(result)
            }
        }
    }
}

fn elapsed(result: &mut Exploration, duration: std::time::Duration) {
    match result {
        Exploration::Stopped(stop) => stop.elapsed = duration,
        Exploration::Complete(complete) => complete.safety_result.elapsed = duration,
    }
}

/// Graph capture cannot turn an incomplete or unsupported search into success.
#[derive(Clone, Debug)]
pub enum Exploration {
    /// Complete safety exploration; explicitly not a temporal verdict.
    Complete(Box<Complete>),
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
            self.root(target);
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
/// is retained. VIEW, active quotient symmetry, randomness and stateful checker
/// reads remain unsupported; D-005 may select full concrete identities instead. Capture uses ordered, bounded source waves with the requested
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
    explore_with_native_hooks(model, opts, limits, dive_eval::NativeHooks::Callback)
}

/// Explicit operative native-hook selection for an owning driver.
///
/// Legacy capture APIs always use callbacks. This selects execution policy only;
/// it grants no private model ownership, purity or permission to skip work.
///
/// # Errors
/// The existing exploration thread failure; execution evidence may be lost.
pub fn explore_with_native_hooks(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    native_hooks: dive_eval::NativeHooks,
) -> Result<Exploration, String> {
    let started = std::time::Instant::now();
    if let Some(stopped) = context_stop(model, opts) {
        return Ok(stopped);
    }
    let run = || explore_inner(model, opts, limits, false, None, None, native_hooks).legacy();
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
/// VIEW, active quotient symmetry, randomness and checker-state reads remain
/// named refusals. Whole-run D-005 unreduced symmetry uses concrete identities.
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
    let mut result = run_on_thread(|| {
        explore_inner(
            model,
            opts,
            limits,
            true,
            None,
            None,
            dive_eval::NativeHooks::Callback,
        )
        .legacy()
    })?;
    elapsed(&mut result, started.elapsed());
    Ok(result)
}

/// Capture and observe successful initialization and completed levels at one worker.
/// Both the inline and ordered pool executors deliver the same events. Failure
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
    if opts.workers != 1 {
        return Ok(stopped(
            Verdict::Unsupported {
                feature: "level observation currently requires one worker".into(),
            },
            model,
            opts,
        ));
    }
    let started = std::time::Instant::now();
    let mut result = run_on_thread(|| {
        explore_inner(
            model,
            opts,
            limits,
            false,
            Some(observer),
            None,
            dive_eval::NativeHooks::Callback,
        )
        .legacy()
    })?;
    elapsed(&mut result, started.elapsed());
    Ok(result)
}

/// Captures with completed-level prefix control at the requested worker width.
/// Every callback follows joined, committed safety work. A prefix is never
/// complete; callback errors retain the existing safety stop behavior.
///
/// # Errors
/// The dedicated exploration thread failed or panicked; work is unknown.
pub fn explore_controlled_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: &mut dyn Observer,
) -> Result<ControlledExploration, String> {
    explore_controlled_with_native_hooks(
        model,
        opts,
        limits,
        observer,
        dive_eval::NativeHooks::Callback,
    )
}

/// Explicit operative native-hook selection for an owning driver.
///
/// Legacy capture APIs always use callbacks. This selects execution policy only;
/// it grants no private model ownership, purity or permission to skip work.
///
/// # Errors
/// The existing exploration thread failure; execution evidence may be lost.
pub fn explore_controlled_with_native_hooks(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: &mut dyn Observer,
    native_hooks: dive_eval::NativeHooks,
) -> Result<ControlledExploration, String> {
    if let Some(stop) = context_stop(model, opts) {
        return Ok(stop.into());
    }
    let started = std::time::Instant::now();
    let mut result = run_on_thread(|| {
        explore_inner(
            model,
            opts,
            limits,
            false,
            Some(observer),
            None,
            native_hooks,
        )
    })?;
    match &mut result {
        ControlledExploration::Complete(c) => c.safety_result.elapsed = started.elapsed(),
        ControlledExploration::Stopped(s) => s.elapsed = started.elapsed(),
        ControlledExploration::Prefix(p) => p.result.elapsed = started.elapsed(),
    }
    Ok(result)
}

fn run_on_thread<T: Send>(run: impl FnOnce() -> T + Send) -> Result<T, String> {
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
    clippy::too_many_arguments,
    reason = "keep admission, startup, explicit operative policy and failed execution evidence in order"
)]
fn explore_inner<'obs>(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    parallel: bool,
    observer: Option<&'obs mut dyn Observer>,
    primary: Option<&mut CheckpointHooks<'obs>>,
    native_hooks: dive_eval::NativeHooks,
) -> ControlledExploration {
    let refuse = |s: String| {
        ControlledExploration::from(stopped(Verdict::Unsupported { feature: s }, model, opts))
    };
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
    if model.symmetry.is_some() && model.symmetry_liveness().requested() != opts.symmetry_liveness {
        return refuse("graph capture symmetry-liveness policy mismatch".into());
    }
    if model.view.is_some() {
        return refuse(
            "graph capture with VIEW requires quotient edge and closing-state evidence".into(),
        );
    }
    if model.symmetry_reduction.is_some()
        || (model.symmetry.is_some() && !model.unreduced_symmetry_liveness())
    {
        if !model.symmetry_liveness().genuine_temporal() {
            return refuse(
                "graph capture with SYMMETRY needs a genuine temporal unreduced policy".into(),
            );
        }
        return refuse(match opts.symmetry_liveness {
            dive_model::SymmetryLiveness::Refuse => {
                "SYMMETRY with liveness explicitly refused".into()
            }
            dive_model::SymmetryLiveness::Tlc => {
                "TLC quotient liveness with SYMMETRY is not implemented".into()
            }
            dive_model::SymmetryLiveness::Unreduced => {
                "graph capture with active or inconsistent SYMMETRY".into()
            }
        });
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
    if let Some(s) = model.stateful_checking() {
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
    // Keep the original fresh capture-host installation/reset above. The
    // owned scope restores Callback on THIS host, never the discarded bind host.
    let _native_hooks = if native_hooks == dive_eval::NativeHooks::Unrestricted {
        model.evaluator.native_hook_scope(native_hooks)
    } else {
        None
    };
    model.evaluator.intern_checker_names();
    let config = engine::run_config(opts, model.settings.check_deadlock);
    model.evaluator.intern_bfs_config(&config);
    host.set_config(Some(config));
    engine::startup_counters(&host);
    let mut result = crate::bare(Verdict::Success, opts, &model.ir);
    result.symmetry_liveness = Some(model.symmetry_liveness().clone());
    crate::messages(&model.output, &mut result);
    let assumption_failure = crate::budget::check_assumptions(model, &opts.budget);
    result.output.extend(model.take_output());
    if let Some(verdict) = assumption_failure {
        result.verdict = verdict;
        return ControlledExploration::Stopped(Box::new(result));
    }
    let outcome = engine::search(
        model,
        opts,
        &host,
        engine::SearchHooks {
            capture: Some((limits, parallel)),
            observer,
            primary,
            ..Default::default()
        },
    );
    result.verdict = outcome.verdict;
    result.stats = outcome.stats;
    result.tlc_prints_stats = outcome.tlc_prints_stats;
    result.counterexample = outcome.counterexample;
    result.collision = outcome.collision;
    result.workers = outcome.workers;
    result.output.extend(outcome.output);
    result.notes = outcome.notes;
    crate::note_canonical_stop(opts, &result.verdict, &mut result.notes);
    if result.verdict != Verdict::Success {
        return ControlledExploration::Stopped(Box::new(result));
    }
    let (Some(graph), Some(stats)) = (outcome.graph, result.stats) else {
        result.verdict = bug("completed exploration did not return graph/statistics");
        return ControlledExploration::Stopped(Box::new(result));
    };
    if let Some(level) = outcome.prefix_level {
        if stats.left_on_queue == 0 {
            result.verdict = bug("prefix has exhausted frontier");
            return ControlledExploration::Stopped(Box::new(result));
        }
        return ControlledExploration::Prefix(Box::new(Prefix {
            level,
            result: Box::new(result.clone()),
            options: opts.clone(),
            graph,
            stats,
            collision: result.collision,
            workers: result.workers,
            output: result.output,
            warnings: result.warnings,
        }));
    }
    let completion = match GraphCompletion::finalize(&graph, stats, limits) {
        Ok(completion) => completion,
        Err(verdict) => {
            result.verdict = verdict;
            return ControlledExploration::Stopped(Box::new(result));
        }
    };
    ControlledExploration::Complete(Box::new(Complete {
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
    result.symmetry_liveness = Some(model.symmetry_liveness().clone());
    crate::messages(&model.output, &mut result);
    Exploration::Stopped(Box::new(result))
}

#[cfg(test)]
#[allow(clippy::panic, reason = "controlled internal thread failure")]
mod evidence_controls {
    use super::{BehaviorGraph, Edge, GraphCompletion, Limits};
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
        };
        assert!(GraphCompletion::finalize(&empty, Stats::default(), limits).is_ok());
    }
    #[test]
    fn panic_does_not_fabricate_a_zero_worker_stop() {
        let result: Result<super::Exploration, String> =
            super::run_on_thread(|| panic!("controlled capture panic"));
        assert!(matches!(result, Err(message) if message == "exploration thread panicked"));
    }
}
