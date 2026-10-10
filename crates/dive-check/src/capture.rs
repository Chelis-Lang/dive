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
    /// Maximum distinct stored states.
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

/// A fully explored behavior graph in the engine's discovery order.
/// States use the bound model's IR variable order. Fingerprint-mode identity
/// has the same collision qualifications as the safety engine.
#[derive(Clone, Debug)]
pub struct BehaviorGraph {
    /// Concrete state representatives, including all variable assignments.
    pub states: Vec<StateVals>,
    /// Outgoing edges in generator order, then the implicit stutter.
    pub edges: Vec<Vec<Edge>>,
    /// Distinct in-model initial states in discovery order.
    pub roots: Vec<usize>,
}

/// Evidence from a completed safety exploration. Temporal predicates were
/// not evaluated; callers must complete planning and acceptance checking.
#[derive(Clone, Debug)]
pub struct Complete {
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
}

impl<K: Ord> Collector<K> {
    pub(crate) fn new(limits: Limits) -> Self {
        Self {
            ids: BTreeMap::new(),
            graph: BehaviorGraph {
                states: vec![],
                edges: vec![],
                roots: vec![],
            },
            limits,
            count: 0,
        }
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
        let target = self.state(key, vals, id)?;
        if let Some(from) = parent {
            self.edge(
                from,
                Edge {
                    to: target,
                    ordinal: Some(succ.ordinal),
                    label: Some(succ.action.label().to_owned()),
                },
            )?;
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
    let run = || explore_inner(model, opts, limits);
    let mut result = run_on_thread(run)?;
    if let Exploration::Stopped(stop) = &mut result {
        stop.elapsed = started.elapsed();
    }
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

fn explore_inner(model: &Model, opts: &CheckOptions, limits: Limits) -> Exploration {
    let refuse = |s: String| stopped(Verdict::Unsupported { feature: s }, model, opts);
    if opts.frontier_memory_bytes.is_some() {
        return refuse("frontier spill with behavior graph capture is not yet certified".into());
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
    let outcome = engine::search_graph(model, opts, &host, limits);
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
    match (outcome.graph, result.stats) {
        (Some(graph), Some(stats)) => Exploration::Complete(Box::new(Complete {
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
