//! Bounded behavior-graph capture through the safety engine.
//!
//! A complete graph is exploration evidence, not a temporal verdict. This
//! capture path uses the shared ordered safety engines. It retains every accepted
//! generated edge, including duplicates, and adds explicit implicit stutters.

mod pure;

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
/// This excludes the engine-added stutter; phase-L collection adds that itself.
#[derive(Clone, Debug)]
pub struct ConcreteSuccessor {
    /// Original concrete IR-ordered values, including already seen states.
    pub state: StateVals,
    /// Original source-wide generation ordinal.
    pub ordinal: u64,
    /// Original action label.
    pub label: String,
}

/// Ordered source publication authority held at an observer callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationKind {
    /// Original ordered wave path. A later safety cut may leave other source
    /// outputs absent; pool quiescence does not imply whole-wave completion.
    OrderedWave,
    /// Actual completed contiguous prefix committed under a request pause.
    RequestPrefix,
}

/// Producer-owned pool-thread quiescence for a submitted source range. Every
/// admitted job has returned and the pool is joined; some source IDs may remain
/// unclaimed or have no returned output. This is not whole-wave completion,
/// TLC's `StateQueue.suspendAll`, a clock sample or a model proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceBarrier {
    workers: usize,
    first: u64,
    end: u64,
    prefix: Option<(u64, u64, usize)>,
}
impl SourceBarrier {
    pub(crate) fn joined(workers: usize, first: u64, end: u64) -> Self {
        Self {
            workers,
            first,
            end,
            prefix: None,
        }
    }
    pub(crate) fn joined_prefix(
        workers: usize,
        first: u64,
        end: u64,
        start: u64,
        upto: u64,
        predicates: usize,
    ) -> Self {
        Self {
            workers,
            first,
            end,
            prefix: Some((start, upto, predicates)),
        }
    }
    /// Typed distinction between the original ordered wave and an actual
    /// completed-prefix request transaction. Neither grants ordinary proof.
    #[must_use]
    pub fn publication(&self) -> PublicationKind {
        if self.prefix.is_some() {
            PublicationKind::RequestPrefix
        } else {
            PublicationKind::OrderedWave
        }
    }
    /// Selected completed-prefix transaction, if publication is happening under
    /// a request pause. The full submitted wave remains separately identified.
    #[must_use]
    pub fn published_prefix(&self) -> Option<std::ops::Range<u64>> {
        self.prefix.map(|(start, end, _)| start..end)
    }
    /// Actual inline predicate width for a request-prefix transaction: zero
    /// when no new-state invariant work was needed, otherwise one coordinator.
    #[must_use]
    pub fn prefix_predicate_workers(&self) -> Option<usize> {
        self.prefix.map(|(_, _, width)| width)
    }
    /// Source observer publication occurs before the successful transaction's
    /// trace/frontier append. An observer stop does not certify that append.
    #[must_use]
    pub fn frontier_append_pending(&self) -> bool {
        true
    }
    /// Actual pool width, separate from logical committed-source counts.
    #[must_use]
    pub fn workers(&self) -> usize {
        self.workers
    }
    /// First identity in the submitted claimable range. It need not have a
    /// returned output unless separately covered by actual publication.
    #[must_use]
    pub fn first_source(&self) -> u64 {
        self.first
    }
    /// Exclusive last submitted discovery identity; later sources may be speculative.
    #[must_use]
    pub fn end_source(&self) -> u64 {
        self.end
    }
}

/// Borrowed authority held while the actual pool dispatch mutex is paused.
/// This is distinct from a copied joined-wave receipt and from Java queue
/// suspension. It cannot be constructed, cloned or retained by an observer.
pub struct DispatchPause<'a> {
    workers: usize,
    generation: u64,
    authority: std::marker::PhantomData<&'a mut ()>,
}
impl DispatchPause<'_> {
    pub(crate) fn new(workers: usize, generation: u64) -> Self {
        Self {
            workers,
            generation,
            authority: std::marker::PhantomData,
        }
    }
    /// Actual running pool width, including the coordinator.
    #[must_use]
    pub fn workers(&self) -> usize {
        self.workers
    }
    /// Last actually dispatched job generation; no job is running in this pause.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }
}
/// Successful safety EOF after the pool has joined and closed. This is not a
/// temporal check or proof; a source's zero queue counter cannot create it.
pub struct ExplorationEnd {
    stats: Stats,
    workers: Workers,
}
impl ExplorationEnd {
    /// Actual completed exploration counters.
    #[must_use]
    pub fn stats(&self) -> Stats {
        self.stats
    }
    /// Actual closed-pool execution evidence.
    #[must_use]
    pub fn workers(&self) -> &Workers {
        &self.workers
    }
}

/// One logically committed source after all of its ordinary safety checks.
/// Speculative pool work can include later sources; only `stats` describes the
/// logical commit prefix. This is execution evidence, not a temporal proof.
pub struct CommittedSource<'a> {
    /// Engine discovery identity of the actual expanded source.
    pub id: u64,
    /// One-based BFS depth of the expanded source.
    pub level: u64,
    /// Original concrete source values.
    pub state: &'a StateVals,
    /// Original accepted successor values, including duplicates/seen states.
    pub successors: &'a [ConcreteSuccessor],
    /// Logical counters after this source, before any later source is committed.
    pub stats: Stats,
    /// Generated successors actually evaluated speculatively in this pool level.
    /// This is not a TLC stopping counter and may exceed the logical prefix.
    pub pool_generated: u64,
    /// Actual submitted generation/check jobs joined before this callback.
    /// It does not assert every source in the wave was logically committed.
    pub barrier: SourceBarrier,
}

/// An explicitly supplied consumer at the ordered initialization/source seam.
/// Callbacks are invoked serially after conservative model-scope admission.
/// They do not authorize a temporal verdict: errors stop capture as a named
/// unsupported result, and successful callbacks certify only execution order.
/// Consumers must meter their own work and retain any deciding typed error.
pub trait SourceObserver: Send {
    /// Opt in to producer-clock requests between actual expansion claims.
    /// A joined request transaction can publish its returned contiguous source
    /// prefix before the quiescence callback; later outputs stay speculative.
    fn progress_requests(&self) -> Option<crate::progress::Options> {
        None
    }
    /// An actual expansion cut cancels producer requests before metadata service.
    /// This notification cannot fail or replace the existing ordered safety error.
    /// Claims stay open to drain earlier unclaimed sources; this is not lane closure.
    fn expansion_requests_cancelled(&mut self, _pending: Option<crate::progress::Request>) {}
    /// Actual joined expansion pause after the returned contiguous prefix passes
    /// ordered safety/graph publication and successful frontier append. An empty
    /// prefix leaves the existing graph unchanged. Success resumes unclaimed
    /// expansion work, not a Java queue or a model-completion claim.
    ///
    /// # Errors
    /// A named diagnostic stop; no completed graph or temporal proof is exposed.
    fn expansion_quiescent(
        &mut self,
        _receipt: ExpansionQuiescence,
        _pause: &DispatchPause<'_>,
    ) -> Result<(), String> {
        Ok(())
    }
    /// A newly enqueued initial state, before its initial invariants/implied checks.
    /// The counters describe the observed prefix; TLC's later error replay is
    /// separate and is not inferred by this interface.
    ///
    /// # Errors
    /// Return the deciding consumer diagnostic; no completed graph is exposed.
    fn initial(
        &mut self,
        _id: u64,
        _state: &StateVals,
        _ordinal: u64,
        _stats: Stats,
    ) -> Result<(), String> {
        Ok(())
    }
    /// Successful initialization after all initial invariants/implied checks,
    /// before the worker pool exists. Never called after an Init failure.
    ///
    /// # Errors
    /// Return the deciding consumer diagnostic; expansion has not begun.
    fn initial_complete(&mut self, _stats: Stats) -> Result<(), String> {
        Ok(())
    }
    /// Opt in to holding actual pool dispatch authority during source callbacks.
    /// The default preserves the existing observer behavior.
    fn requires_dispatch_pause(&self) -> bool {
        false
    }
    /// A source callback under the real pool dispatch pause. The callback must
    /// not reenter the pool; no worker or new job can execute until it returns.
    ///
    /// # Errors
    /// Retain a deciding consumer diagnostic before later source commits.
    fn paused_source(
        &mut self,
        source: CommittedSource<'_>,
        _pause: &DispatchPause<'_>,
    ) -> Result<(), String> {
        self.source(source)
    }
    /// Successful actual safety EOF after the pool closes. Never called after
    /// assumption, Init, source, safety, worker or resource failure.
    ///
    /// # Errors
    /// A final consumer check failed; no completed graph is exposed.
    fn exploration_complete(&mut self, _end: &ExplorationEnd) -> Result<(), String> {
        Ok(())
    }
    /// A complete source before later sources' safety stopping events are chosen.
    ///
    /// # Errors
    /// Return the deciding consumer diagnostic; no completed graph is exposed.
    fn source(&mut self, source: CommittedSource<'_>) -> Result<(), String>;
}

/// Real expansion-claim barrier and joined pool evidence after prefix publication.
/// Completed IDs can be noncontiguous. Only `committed` records have passed
/// ordered safety/publication and successful frontier append; later completed
/// outputs remain speculative. This receipt still grants no ordinary proof.
#[derive(Clone, Debug)]
pub struct ExpansionQuiescence {
    pub(crate) request: crate::progress::Request,
    pub(crate) submitted: std::ops::Range<u64>,
    pub(crate) completed: Vec<u64>,
    pub(crate) claimed: Vec<u64>,
    pub(crate) failed: Vec<u64>,
    pub(crate) expanded_generated: u64,
    pub(crate) logical: Stats,
    pub(crate) committed: std::ops::Range<u64>,
    pub(crate) logical_after: Stats,
    pub(crate) predicate_workers: usize,
}
impl ExpansionQuiescence {
    /// Producer-owned timer/initial-force request serviced under actual authority.
    #[must_use]
    pub fn request(&self) -> crate::progress::Request {
        self.request
    }
    /// Source wave identities, including work not yet claimed.
    #[must_use]
    pub fn submitted(&self) -> std::ops::Range<u64> {
        self.submitted.clone()
    }
    /// Returned source expansions, including earlier committed prefixes.
    /// Completion alone does not imply ordered publication.
    #[must_use]
    pub fn completed(&self) -> &[u64] {
        &self.completed
    }
    /// Actual source claims, including a skipped later source after a safety cut.
    /// Claims and successful completed output are separate observations.
    #[must_use]
    pub fn claimed(&self) -> &[u64] {
        &self.claimed
    }
    /// Completed expansion outputs that contain an actual deciding safety error.
    /// Other completed outputs may still await invariant checks/publication.
    #[must_use]
    pub fn failed(&self) -> &[u64] {
        &self.failed
    }
    /// Generated successors in retained expansion outputs, including speculative
    /// later sources. This is not the logical stopping counter.
    #[must_use]
    pub fn expanded_generated(&self) -> u64 {
        self.expanded_generated
    }
    /// Actual logical statistics preceding publication of this wave.
    #[must_use]
    pub fn logical_stats(&self) -> Stats {
        self.logical
    }
    /// Newly published contiguous source IDs, or an empty range when the next
    /// source has not returned. Their actual observer writes precede this receipt.
    #[must_use]
    pub fn committed(&self) -> std::ops::Range<u64> {
        self.committed.clone()
    }
    /// Logical statistics after successful ordered prefix publication.
    #[must_use]
    pub fn committed_stats(&self) -> Stats {
        self.logical_after
    }
    /// Sources whose successful transaction physically appended its winning
    /// states to the next frontier. These are source IDs, not new-state IDs.
    /// A stopping observer emits no successful append receipt.
    #[must_use]
    pub fn frontier_appended(&self) -> std::ops::Range<u64> {
        self.committed.clone()
    }
    /// Coordinator invariant-predicate width for this publication (zero if
    /// no newly discovered states required such work, one otherwise).
    /// Expansion pool width remains separately reported by the pause.
    #[must_use]
    pub fn predicate_workers(&self) -> usize {
        self.predicate_workers
    }
    /// This receipt grants no ordinary verdict credit. Actual ordered observer
    /// writes are identified separately by `committed`.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        false
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
    /// Actual ordinary safety-engine outcome, retained without reconstructing
    /// counts, diagnostics, stopping facts or worker metadata from the graph.
    /// This receipt does not certify any temporal obligation.
    #[must_use]
    pub fn safety_result(&self) -> &CheckResult {
        &self.safety_result
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

/// Capture through the actual pool while consuming initialization and source
/// commits in reference order. This grants no temporal verdict or replay proof.
/// The observer's diagnostic stops the engine before later safety events, and
/// the stopped result exposes no completed graph. Model admission precedes any
/// callback; zero/overbound widths run no callback or worker.
///
/// # Errors
/// An exploration thread or worker fails, losing actual execution evidence.
pub fn explore_observed_with_evidence(
    model: &Model,
    opts: &CheckOptions,
    limits: Limits,
    observer: &mut dyn SourceObserver,
) -> Result<Exploration, String> {
    if let Some(stopped) = context_stop(model, opts) {
        return Ok(stopped);
    }
    if opts.workers == 0 || opts.workers > 64 {
        return explore_concrete_parallel_with_evidence(model, opts, limits);
    }
    if opts.sequential || engine::plan_workers(model, opts).sequential {
        return Ok(stopped(Verdict::Unsupported {
            feature: "ordered source observation requires the concrete safety pool; inline source callbacks are not certified".into(),
        }, model, opts));
    }
    let started = std::time::Instant::now();
    let mut result = run_on_thread(|| explore_inner(model, opts, limits, true, Some(observer)))?;
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
    mut observer: Option<&mut dyn SourceObserver>,
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
    if engine::uses_random(&model.ir) {
        return refuse("graph capture with random operators".into());
    }
    if !model.sequential_names_only
        && let Some(s) = model.sequential.first()
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
    if parallel && !pure::model(model) {
        return refuse(
            "concrete pool capture effects, runtime names, opaque captures or invalid context"
                .into(),
        );
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
    let outcome = if let Some(consumer) = observer.as_mut() {
        engine::search_graph_observed(model, opts, &host, limits, Some(&mut **consumer))
    } else if parallel {
        engine::search_graph_parallel(model, opts, &host, limits)
    } else {
        engine::search_graph(model, opts, &host, limits)
    };
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
        return Exploration::Stopped(Box::new(result));
    }
    if let (Some(consumer), Some(stats)) = (observer, result.stats) {
        let end = ExplorationEnd {
            stats,
            workers: result.workers.clone(),
        };
        if let Err(message) = consumer.exploration_complete(&end) {
            result.verdict = Verdict::Unsupported {
                feature: format!("ordered completion observer: {message}"),
            };
            return Exploration::Stopped(Box::new(result));
        }
    }
    match (outcome.graph, result.stats) {
        (Some(graph), Some(stats)) => Exploration::Complete(Box::new(Complete {
            safety_result: Box::new(result.clone()),
            options: opts.clone(),
            graph,
            stats,
            collision: result.collision,
            workers: result.workers,
            output: result.output,
            warnings: result.warnings,
        })),
        _ => stopped(
            bug("completed exploration did not return graph/statistics"),
            model,
            opts,
        ),
    }
}

fn stopped(verdict: Verdict, model: &Model, opts: &CheckOptions) -> Exploration {
    let mut result = crate::bare(verdict, opts, &model.ir);
    crate::messages(&model.output, &mut result);
    Exploration::Stopped(Box::new(result))
}

#[cfg(test)]
#[allow(clippy::panic, reason = "controlled internal thread failure")]
mod evidence_controls {
    #[test]
    fn panic_does_not_fabricate_a_zero_worker_stop() {
        let result = super::run_on_thread(|| panic!("controlled capture panic"));
        assert!(matches!(result, Err(message) if message == "exploration thread panicked"));
    }
}
