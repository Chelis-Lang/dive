//! Ordered concrete safety commits with interleaved native temporal graph work.
//!
//! This bounded execution slice implements multiple actual tableau/null OOS groups.
//! It has real source-prefix ordering and graph insertion, but certifies no TLC
//! SCC traversal, lasso choice, periodic schedule or model-checking verdict.

use crate::Graph;
use crate::periodic_backend::{Checkpoint, Decision, Growth, Policy};
use crate::phase_graph::{Commit, Error, Limits, Transition};
use crate::phase_l::{self, Incoming, Session};
use crate::search::Acceptance;
use crate::tlc_plan::{OrderOfSolution, PossibleErrorModel};
use crate::tlc_tableau::Tableau;
use dive_check::capture::{self, CommittedSource, Exploration, SourceObserver};
use dive_check::{CheckOptions, Stats, Workers};
use dive_model::Model;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

/// One registered product node, distinct from an actually written node.
#[derive(Clone, Debug)]
pub struct Node {
    /// Original TLC fingerprint identity.
    pub fingerprint: u64,
    /// Original particle identity; -1 for a null tableau.
    pub tableau: i32,
    /// State-bin results, committed only by a successful write.
    pub state: Vec<bool>,
    /// Actual transition insertion order and action bins.
    pub transitions: Vec<Transition>,
    /// Whether this node has actually been written.
    pub written: bool,
}
/// One OOS's actual committed graph, in product registration order.
#[derive(Clone, Debug, Default)]
pub struct Group {
    /// Registered nodes; unwritten records remain explicit on failure.
    pub nodes: Vec<Node>,
    /// Actual initial product registrations, separate from writes.
    pub roots: Vec<(u64, i32)>,
    /// Completed main-source receipts; recursive writes are separate.
    pub commits: Vec<Commit>,
    /// Product record indices in actual write order, including rewrites.
    pub writes: Vec<usize>,
    pems: Vec<PossibleErrorModel>,
    records: BTreeMap<(u64, i32), usize>,
    done: BTreeSet<u64>,
}
/// Completed bounded execution evidence, explicitly not a temporal verdict.
#[derive(Clone, Debug)]
pub struct Complete {
    /// Complete safety exploration using the actual requested pool.
    pub capture: Box<capture::Complete>,
    /// Actual groups in pinned OOS first-encounter order.
    pub groups: Vec<Group>,
    /// Original configured PROPERTY identities, before role splitting.
    pub configured_properties: Vec<dive_model::Named>,
    /// Original configuration name of each expanded temporal Action.
    /// This does not replace original before-splitting witness claims.
    pub expanded_property_names: Vec<String>,
    /// Derived PROPERTY-role names; never substituted for configured identities.
    pub role_names: Vec<String>,
    /// Predicate/collection stage actual work; executes inline in commit order.
    pub phase_work: phase_l::Progress,
    /// Actual recursive enumeration separate from main safety generated counts.
    pub recursive_work: RecursiveWork,
    /// Last observed source-prefix and actual speculative pool emissions.
    /// Absent when execution never reached a source callback.
    pub last_commit_work: Option<CommitWork>,
    limits: crate::product::Limits,
    tableaux: Vec<Option<Tableau>>,
}
impl Complete {
    /// Number of pinned PEMs in an actually compiled group.
    /// No supplied graph or edited public fields become proof authority.
    ///
    /// # Errors
    /// A group index does not name a compiled order of solution.
    pub fn pem_count(&self, group: usize) -> Result<usize, Error> {
        self.groups
            .get(group)
            .map(|g| g.pems.len())
            .ok_or_else(|| Error::Input("OOS group index".into()))
    }
    /// Project one actual committed OOS/PEM graph into bounded SCC inputs.
    /// This grants neither TLC witness-selection nor model-proof credit.
    ///
    /// # Errors
    /// Invalid indices, incomplete node references or retained-product limits.
    pub fn product_inputs(
        &self,
        group: usize,
        pem: usize,
    ) -> Result<(crate::Graph, crate::search::Acceptance), Error> {
        let g = self
            .groups
            .get(group)
            .ok_or_else(|| Error::Input("OOS group index".into()))?;
        project_inputs(
            g,
            pem,
            self.limits,
            self.tableaux.get(group).and_then(Option::as_ref),
        )
    }
    /// This execution slice does not certify a model-checking proof.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        false
    }
}

/// Failed execution with the actually persisted temporal prefix retained.
#[derive(Clone, Debug)]
pub struct Failure {
    /// Typed deciding phase/capture/bound error; never a success.
    pub error: Error,
    /// Logical source-prefix safety counters; initialization replay is not certified.
    pub stats: Option<Stats>,
    /// Actual execution width, absent if worker evidence was lost.
    pub workers: Option<Workers>,
    /// Completed temporal writes before the deciding failure.
    pub groups: Vec<Group>,
    /// Actual temporal predicate/collection work.
    pub phase_work: phase_l::Progress,
    /// Actual recursive work before the stop; no safety-count substitution.
    pub recursive_work: RecursiveWork,
    /// Last observed source-prefix and actual speculative pool emissions.
    /// Absent when execution never reached a source callback.
    pub last_commit_work: Option<CommitWork>,
}
/// Actual source callback evidence; logical counts never replace pool work.
#[derive(Clone, Copy, Debug)]
pub struct CommitWork {
    /// Logical source discovery identity.
    pub source: u64,
    /// Deciding or completed source-prefix counters.
    pub logical: Stats,
    /// Actual emitted successors in the expansion batch containing this source.
    /// May exceed the logical stopping prefix; excludes initializer work.
    pub batch_generated: u64,
}
/// Actual recursive tableau re-enumeration work, separate from safety counters.
#[derive(Clone, Copy, Debug, Default)]
pub struct RecursiveWork {
    /// Successors actually emitted, including the deciding bound admission.
    pub generated: u64,
    /// `StateVec` entries retained across calls; cumulative entry accounting.
    pub retained: usize,
    /// Recursive source expansions begun, including failed expansions.
    pub expansions: usize,
}

/// Actual first stored successor slot introducing an unfinished null-tableau record.
/// The producer retains this authority; callers cannot manufacture it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessorOrigin {
    source: usize,
    source_fingerprint: u64,
    successor: usize,
    slot: usize,
    ordinal: Option<u64>,
}
impl SuccessorOrigin {
    /// Original logical committed source index.
    #[must_use]
    pub fn source(&self) -> usize {
        self.source
    }
    /// Original source fingerprint, distinct from the introduced record.
    #[must_use]
    pub fn source_fingerprint(&self) -> u64 {
        self.source_fingerprint
    }
    /// Position within the actual retained slot-order successor vector.
    #[must_use]
    pub fn successor(&self) -> usize {
        self.successor
    }
    /// Actual `SetOfStates` table slot.
    #[must_use]
    pub fn slot(&self) -> usize {
        self.slot
    }
    /// Original generator ordinal; absent for first-admitted implicit stutter.
    #[must_use]
    pub fn ordinal(&self) -> Option<u64> {
        self.ordinal
    }
}
fn validate_origin(group: &Group, node: &Node, origin: &SuccessorOrigin) -> Result<(), Error> {
    let successor = group
        .commits
        .get(origin.source)
        .filter(|c| c.source == origin.source && c.fingerprint == origin.source_fingerprint)
        .and_then(|c| c.receipt.successors.get(origin.successor))
        .ok_or_else(|| Error::Input("unfinished record source-slot provenance".into()))?;
    if node.tableau != -1
        || successor.fingerprint != node.fingerprint
        || successor.slot != origin.slot
        || successor.ordinal != origin.ordinal
    {
        return Err(Error::Input(
            "unfinished record source-slot provenance".into(),
        ));
    }
    Ok(())
}

fn retained_origins(
    records: &Group,
    group: usize,
    origins: &BTreeMap<(usize, usize), SuccessorOrigin>,
) -> Result<Vec<Option<SuccessorOrigin>>, Error> {
    records
        .nodes
        .iter()
        .enumerate()
        .map(|(id, node)| {
            let origin = origins.get(&(group, id));
            if let Some(origin) = origin {
                validate_origin(records, node, origin)?;
            }
            Ok(origin.cloned())
        })
        .collect()
}

/// One check of actual committed records, never an exploration completion.
#[derive(Clone, Debug)]
pub struct PartialCheck {
    sources: usize,
    group: usize,
    pem: usize,
    written: Vec<bool>,
    successor_origins: Vec<Option<SuccessorOrigin>>,
    identities: Vec<crate::tlc_witness::Identity>,
    selection: crate::tlc_witness::PartialSelection,
}
impl PartialCheck {
    /// Number of entire ordered source callbacks committed before this check.
    #[must_use]
    pub fn sources(&self) -> usize {
        self.sources
    }
    /// Original OOS index.
    #[must_use]
    pub fn group(&self) -> usize {
        self.group
    }
    /// Original PEM index within this OOS.
    #[must_use]
    pub fn pem(&self) -> usize {
        self.pem
    }
    /// Original registered record identities, without compaction.
    #[must_use]
    pub fn identities(&self) -> &[crate::tlc_witness::Identity] {
        &self.identities
    }
    /// Actual write status at this checkpoint, separate from discovery.
    #[must_use]
    pub fn written(&self) -> &[bool] {
        &self.written
    }
    /// Producer-derived first successor-slot provenance for null-tableau records.
    /// Initial/tableau registrations have no such origin; absence is not proof.
    #[must_use]
    pub fn successor_origins(&self) -> &[Option<SuccessorOrigin>] {
        &self.successor_origins
    }
    /// Pinned SCC/candidate fields; candidates require independent validation.
    #[must_use]
    pub fn selection(&self) -> &crate::tlc_witness::PartialSelection {
        &self.selection
    }
    /// Absence of a candidate never certifies a partial model check.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        false
    }
}

/// Actual safety execution with diagnostic partial checks after source commits.
/// This does not select an ordinary clock schedule or certify temporal holds.
#[derive(Clone, Debug)]
pub struct PartialExecution {
    execution: Complete,
    checks: Vec<PartialCheck>,
    inspections: usize,
    graph_inspections: usize,
}
impl PartialExecution {
    /// Underlying actual safety exploration and graph construction.
    #[must_use]
    pub fn execution(&self) -> &Complete {
        &self.execution
    }
    /// Ordered checks, including successful partial checks followed by growth.
    #[must_use]
    pub fn checks(&self) -> &[PartialCheck] {
        &self.checks
    }
    /// Cumulative projection/retention reservations and completed selection work.
    #[must_use]
    pub fn inspections(&self) -> usize {
        self.inspections
    }
    /// Shared graph/partial-work charge, bounded by the single `graph.steps` quota.
    /// Session predicate/value work has its own unchanged Phase-L limits.
    #[must_use]
    pub fn graph_inspections(&self) -> usize {
        self.graph_inspections
    }
    /// No ordinary schedule or complete temporal-model proof is certified.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        false
    }
}

/// A stopped diagnostic run, retaining previous partial checks without success.
#[derive(Clone, Debug)]
pub struct PartialFailure {
    /// Original deciding execution failure and prefix evidence.
    pub failure: Box<Failure>,
    checks: Vec<PartialCheck>,
    inspections: usize,
    graph_inspections: usize,
    periodic_checks: Vec<Checkpoint>,
}
impl std::fmt::Display for PartialFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.failure.error.fmt(f)
    }
}
impl std::error::Error for PartialFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure.error)
    }
}
impl PartialFailure {
    /// Checks that actually finished before the deciding failure.
    #[must_use]
    pub fn checks(&self) -> &[PartialCheck] {
        &self.checks
    }
    /// Work charged before failure, including projection reservations.
    /// A failing SCC/witness stage can have additional unreported work; this is
    /// not an exact count of all inspections through that failing stage.
    #[must_use]
    pub fn inspections(&self) -> usize {
        self.inspections
    }
}
struct Observer<'a> {
    model: &'a Model,
    session: Session<'a>,
    tableaux: Vec<Option<Tableau>>,
    recursive_generated: u64,
    recursive_retained: usize,
    recursive_expansions: usize,
    orders: Vec<OrderOfSolution>,
    groups: Vec<Group>,
    limits: Limits,
    error: Option<Error>,
    vertices: usize,
    edges: usize,
    cells: usize,
    steps: usize,
    sources: usize,
    last_commit_work: Option<CommitWork>,
    partial: bool,
    partial_checks: Vec<PartialCheck>,
    partial_steps: usize,
    successor_origins: BTreeMap<(usize, usize), SuccessorOrigin>,
    periodic: Option<Policy>,
    periodic_checks: Vec<Checkpoint>,
    pointer_sizes: Vec<u64>,
    last_sizes: Vec<u64>,
    lifecycle: Option<crate::periodic_lifecycle::State>,
}
fn charge(
    total: &mut usize,
    extra: Option<usize>,
    limit: usize,
    name: &'static str,
) -> Result<(), Error> {
    *total = total
        .checked_add(extra.ok_or(Error::Limit(name))?)
        .filter(|n| *n <= limit)
        .ok_or(Error::Limit(name))?;
    Ok(())
}
impl Observer<'_> {
    fn partial_charge(&mut self, amount: Option<usize>) -> Result<(), Error> {
        charge(
            &mut self.steps,
            amount,
            self.limits.graph.steps,
            "combined graph/partial SCC inspections",
        )?;
        self.partial_steps = self
            .partial_steps
            .checked_add(amount.ok_or(Error::Limit("combined graph/partial SCC inspections"))?)
            .ok_or(Error::Limit("combined graph/partial SCC inspections"))?;
        Ok(())
    }
    fn partial_check(&mut self) -> Result<(), Error> {
        if !self.partial {
            return Ok(());
        }
        for group in 0..self.groups.len() {
            for pem in 0..self.groups[group].pems.len() {
                self.partial_charge(Some(self.groups[group].nodes.len()))?;
                let records = &self.groups[group];
                let edges = records
                    .nodes
                    .iter()
                    .try_fold(0usize, |n, node| n.checked_add(node.transitions.len()));
                let model = &records.pems[pem];
                let cells = model
                    .ae_state
                    .len()
                    .checked_add(
                        self.tableaux[group]
                            .as_ref()
                            .map_or(0, |t| t.promises.len()),
                    )
                    .and_then(|n| n.checked_mul(records.nodes.len()))
                    .and_then(|n| {
                        model
                            .ae_action
                            .len()
                            .checked_add(1)
                            .and_then(|a| a.checked_mul(edges?))
                            .and_then(|a| a.checked_add(n))
                    });
                // Reserve retained identities/write bits and supplied adjacency
                // before cloning or projecting. Actual acceptance/SCC/witness
                // work is charged separately below on the same cumulative meter.
                self.partial_charge(
                    records
                        .nodes
                        .len()
                        .checked_mul(4)
                        .and_then(|n| n.checked_add(edges?))
                        .and_then(|n| n.checked_add(records.roots.len()))
                        .and_then(|n| n.checked_add(cells?)),
                )?;
                let records = &self.groups[group];
                let (graph, labels) = project_inputs_inner(
                    records,
                    pem,
                    self.limits.graph,
                    self.tableaux[group].as_ref(),
                    true,
                )?;
                let successor_origins = retained_origins(records, group, &self.successor_origins)?;
                let written = records.nodes.iter().map(|n| n.written).collect::<Vec<_>>();
                let identities = records
                    .nodes
                    .iter()
                    .map(|n| crate::tlc_witness::Identity {
                        fingerprint: n.fingerprint,
                        tableau: n.tableau,
                    })
                    .collect::<Vec<_>>();
                let remaining = self
                    .limits
                    .graph
                    .steps
                    .checked_sub(self.steps)
                    .ok_or(Error::Limit("partial SCC cumulative inspections"))?;
                let limits = crate::product::Limits {
                    steps: remaining,
                    ..self.limits.graph
                };
                let selection = crate::tlc_witness::select_partial(
                    &graph,
                    &labels,
                    &identities,
                    &written,
                    limits,
                )
                .map_err(|error| match error {
                    crate::tlc_scc::Error::Limit(_) => {
                        Error::Limit("partial SCC cumulative inspections")
                    }
                    other => Error::Input(format!("partial SCC selection: {other}")),
                })?;
                self.partial_charge(Some(selection.inspections()))?;
                let accepting = selection.candidate().is_some();
                self.partial_checks.push(PartialCheck {
                    sources: self.sources,
                    group,
                    pem,
                    written,
                    successor_origins,
                    identities,
                    selection,
                });
                // One logical LiveWorker stops at the first deciding OOS/PEM.
                // This diagnostic continues later source commits deliberately;
                // it does not claim the actual ordinary stopping schedule.
                if accepting {
                    self.record_check_size(group);
                    return Ok(());
                }
            }
            self.record_check_size(group);
        }
        Ok(())
    }
    fn record_check_size(&mut self, group: usize) {
        if self.periodic.is_some() {
            self.last_sizes[group] = self.pointer_sizes[group];
        }
    }
    fn pointer_counts(&mut self) -> Result<Vec<u64>, Error> {
        self.partial_charge(self.groups.len().checked_mul(2))?;
        let mut counts = Vec::with_capacity(self.groups.len());
        for index in 0..self.groups.len() {
            let g = &self.groups[index];
            let work = g.nodes.len().checked_add(g.done.len());
            self.partial_charge(work)?;
            let g = &self.groups[index];
            // Count actual pointer keys: null placeholders/initial roots are not
            // entries; tableau registrations AND done-only buckets are entries.
            let mut keys = BTreeSet::new();
            for n in &g.nodes {
                if self.tableaux[index].is_some() || n.written {
                    keys.insert(n.fingerprint);
                }
            }
            if self.tableaux[index].is_some() {
                keys.extend(g.done.iter().copied());
            }
            let count = u32::try_from(keys.len())
                .ok()
                .filter(|n| i32::try_from(*n).is_ok())
                .ok_or(Error::Limit("pinned node-pointer table entry count"))?;
            counts.push(u64::from(count));
        }
        Ok(counts)
    }
    fn periodic_check(
        &mut self,
        stats: Stats,
        barrier: Option<capture::SourceBarrier>,
    ) -> Result<(), Error> {
        let Some(policy) = self.periodic else {
            return Ok(());
        };
        self.pointer_sizes = self.pointer_counts()?;
        // Charge both OOS vectors and checkpoint metadata BEFORE allocation.
        self.partial_charge(
            self.groups
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(1)),
        )?;
        let before: Vec<_> = self
            .pointer_sizes
            .iter()
            .zip(&self.last_sizes)
            .map(|(size, last)| Growth {
                size: *size,
                last: *last,
            })
            .collect();
        let due = before.iter().any(|g| g.due(policy.threshold));
        let perform =
            policy.force || (policy.schedule == crate::periodic_backend::Schedule::Default && due);
        let start = self.partial_checks.len();
        let result = if perform {
            self.partial_check()
        } else {
            Ok(())
        };
        let candidate = self
            .partial_checks
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(index, c)| c.selection.candidate().is_some().then_some(index));
        let decision = if result.is_err() {
            Decision::Failed
        } else if candidate.is_some() {
            Decision::Candidate
        } else if perform {
            Decision::NoCandidate
        } else {
            Decision::Skipped
        };
        let after = self
            .pointer_sizes
            .iter()
            .zip(&self.last_sizes)
            .map(|(size, last)| Growth {
                size: *size,
                last: *last,
            })
            .collect();
        self.periodic_checks.push(Checkpoint {
            sources: self.sources,
            stats,
            barrier,
            before,
            after,
            due,
            decision,
            candidate,
        });
        result?;
        if candidate.is_some() {
            return Err(Error::PartialCandidate);
        }
        Ok(())
    }
    fn lifecycle_poll(
        &mut self,
        source: &CommittedSource<'_>,
        pause: &capture::DispatchPause<'_>,
        state: &mut crate::periodic_lifecycle::State,
    ) -> Result<(), Error> {
        let now = crate::periodic_lifecycle::time_millis()?;
        let elapsed = now.wrapping_sub(state.last);
        if elapsed < 0 {
            return Err(Error::Execution(
                "native progress clock moved backwards".into(),
            ));
        }
        if elapsed < i64::from(state.options.progress_interval) && !state.force {
            return Ok(());
        }
        self.partial_charge(Some(1))?;
        state.last = now;
        let ratio_before = state.ratio;
        let force_before = state.force;
        let mut policy = crate::periodic::Policy {
            check_liveness: true,
            runtime_ratio: state.ratio,
            ratio_limit: state.options.ratio_limit,
            force_liveness: state.force,
            checkpoint: false,
            periodic: state.hook.is_some(),
        };
        let mut hooks = NativeHooks {
            observer: self,
            state,
            origin: HookOrigin::Source(source),
            pause,
            resumed: false,
        };
        let outcome = crate::periodic::run(&mut policy, &mut hooks);
        hooks.state.ratio = policy.runtime_ratio;
        hooks.state.force = policy.force_liveness;
        hooks
            .state
            .invocations
            .push(crate::periodic_lifecycle::Invocation {
                source: source.id,
                publication: source.barrier,
                stats: source.stats,
                workers: pause.workers(),
                generation: pause.generation(),
                now,
                ratio_before,
                ratio_after: policy.runtime_ratio,
                force_before,
                force_after: policy.force_liveness,
                resume_requested: hooks.resumed,
                outcome: outcome.as_ref().map(|o| *o).map_err(|e| format!("{e:?}")),
            });
        match outcome {
            Ok(crate::periodic::Outcome::Complete) => Ok(()),
            Ok(crate::periodic::Outcome::LivenessStopped(_)) => Err(Error::PartialCandidate),
            Ok(crate::periodic::Outcome::PeriodicAssumptionFalse) => {
                Err(Error::PeriodicAssumptionFalse)
            }
            Err(crate::periodic::Error::Callback(e)) => Err(e),
            Err(crate::periodic::Error::ClockWentBackwards) => Err(Error::Execution(
                "native liveness clock moved backwards".into(),
            )),
        }
    }
    fn lifecycle_service(
        &mut self,
        receipt: &capture::ExpansionQuiescence,
        pause: &capture::DispatchPause<'_>,
        state: &mut crate::periodic_lifecycle::State,
    ) -> Result<(), Error> {
        let initial = state.initial_stats.ok_or_else(|| {
            Error::Input("request service before successful initialization".into())
        })?;
        self.partial_charge(Some(1))?;
        let now = crate::periodic_lifecycle::time_millis()?;
        if now.wrapping_sub(state.last) < 0 {
            return Err(Error::Execution(
                "native progress clock moved backwards".into(),
            ));
        }
        // The actual pending request already passed producer-clock admission.
        // Do not apply the old per-source elapsed filter a second time. Initial
        // request.force is provenance, distinct from liveness force below.
        state.last = now;
        let ratio_before = state.ratio;
        let force_before = state.force;
        let graph_source = state.graph_source;
        let mut policy = crate::periodic::Policy {
            check_liveness: true,
            runtime_ratio: state.ratio,
            ratio_limit: state.options.ratio_limit,
            force_liveness: state.force,
            checkpoint: false,
            periodic: state.hook.is_some(),
        };
        let mut hooks = NativeHooks {
            observer: self,
            state,
            origin: HookOrigin::Request {
                request: receipt.request(),
                stats: receipt.committed_stats(),
                graph_source,
            },
            pause,
            resumed: false,
        };
        let outcome = crate::periodic::run(&mut policy, &mut hooks);
        hooks.state.ratio = policy.runtime_ratio;
        hooks.state.force = policy.force_liveness;
        hooks
            .state
            .request_services
            .push(crate::periodic_service::Invocation {
                request: receipt.request(),
                submitted: receipt.submitted(),
                committed: receipt.committed(),
                graph_source,
                initial,
                stats: receipt.committed_stats(),
                workers: pause.workers(),
                generation: pause.generation(),
                predicates: receipt.predicate_workers(),
                now,
                ratio: (ratio_before, policy.runtime_ratio),
                force: (force_before, policy.force_liveness),
                resumed: hooks.resumed,
                outcome: outcome.as_ref().map(|o| *o).map_err(|e| format!("{e:?}")),
            });
        match outcome {
            Ok(crate::periodic::Outcome::Complete) => Ok(()),
            Ok(crate::periodic::Outcome::LivenessStopped(_)) => Err(Error::PartialCandidate),
            Ok(crate::periodic::Outcome::PeriodicAssumptionFalse) => {
                Err(Error::PeriodicAssumptionFalse)
            }
            Err(crate::periodic::Error::Callback(e)) => Err(e),
            Err(crate::periodic::Error::ClockWentBackwards) => Err(Error::Execution(
                "native liveness clock moved backwards".into(),
            )),
        }
    }
    fn lifecycle_final(
        &mut self,
        state: &mut crate::periodic_lifecycle::State,
        end: &capture::ExplorationEnd,
    ) -> Result<(), Error> {
        state.eof = Some(end.stats());
        self.pointer_sizes = self.pointer_counts()?;
        for group in 0..self.groups.len() {
            for pem in 0..self.groups[group].pems.len() {
                // Reserve projection metadata before clones/retention, using the
                // original shared graph meter rather than a fresh SCC allowance.
                let records = &self.groups[group];
                let edge_count = records
                    .nodes
                    .iter()
                    .try_fold(0usize, |n, x| n.checked_add(x.transitions.len()));
                let cells = records
                    .nodes
                    .len()
                    .checked_mul(records.pems[pem].ae_state.len())
                    .and_then(|n| {
                        edge_count?
                            .checked_mul(records.pems[pem].ae_action.len().checked_add(1)?)?
                            .checked_add(n)
                    })
                    .and_then(|n| {
                        records
                            .nodes
                            .len()
                            .checked_mul(
                                self.tableaux[group]
                                    .as_ref()
                                    .map_or(0, |t| t.promises.len()),
                            )?
                            .checked_add(n)
                    });
                self.partial_charge(
                    records
                        .nodes
                        .len()
                        .checked_mul(3)
                        .and_then(|n| n.checked_add(edge_count?))
                        .and_then(|n| n.checked_add(cells?))
                        .and_then(|n| n.checked_add(records.roots.len()))
                        .and_then(|n| n.checked_add(1)),
                )?;
                let records = &self.groups[group];
                let (graph, labels) = project_inputs_inner(
                    records,
                    pem,
                    self.limits.graph,
                    self.tableaux[group].as_ref(),
                    false,
                )?;
                let identities = records
                    .nodes
                    .iter()
                    .map(|n| crate::tlc_witness::Identity {
                        fingerprint: n.fingerprint,
                        tableau: n.tableau,
                    })
                    .collect::<Vec<_>>();
                let remaining = self
                    .limits
                    .graph
                    .steps
                    .checked_sub(self.steps)
                    .ok_or(Error::Limit("final SCC cumulative inspections"))?;
                let selection = crate::tlc_witness::select(
                    &graph,
                    &labels,
                    &identities,
                    crate::product::Limits {
                        steps: remaining,
                        ..self.limits.graph
                    },
                )
                .map_err(|e| match e {
                    crate::tlc_scc::Error::Limit(_) => {
                        Error::Limit("final SCC cumulative inspections")
                    }
                    other => Error::Input(format!("final SCC selection: {other}")),
                })?;
                self.partial_charge(
                    selection
                        .traversal
                        .inspections
                        .checked_add(selection.candidate.as_ref().map_or(0, |c| c.inspections)),
                )?;
                let candidate = selection.candidate.is_some();
                state
                    .final_checks
                    .push(crate::periodic_lifecycle::FinalCheck {
                        group,
                        pem,
                        selection,
                    });
                if candidate {
                    self.record_check_size(group);
                    return Err(Error::PartialCandidate);
                }
            }
            self.record_check_size(group);
        }
        Ok(())
    }
    fn recursive_work(&self) -> RecursiveWork {
        RecursiveWork {
            generated: self.recursive_generated,
            retained: self.recursive_retained,
            expansions: self.recursive_expansions,
        }
    }
    fn register(&mut self, group: usize, fp: u64, tidx: i32) -> Result<(usize, bool), Error> {
        if let Some(id) = self.groups[group].records.get(&(fp, tidx)) {
            return Ok((*id, false));
        }
        charge(
            &mut self.vertices,
            Some(1),
            self.limits.graph.vertices,
            "execution graph vertices",
        )?;
        let g = &mut self.groups[group];
        let id = g.nodes.len();
        g.nodes.push(Node {
            fingerprint: fp,
            tableau: tidx,
            state: Vec::new(),
            transitions: Vec::new(),
            written: false,
        });
        g.records.insert((fp, tidx), id);
        Ok((id, true))
    }
    fn initial_inner(&mut self, state: &phase_l::State) -> Result<(), Error> {
        let fp = self
            .session
            .concrete_fingerprint(state)
            .map_err(Error::Phase)?;
        for group in 0..self.groups.len() {
            let count = self.tableaux[group].as_ref().map_or(1, |t| t.initial_count);
            charge(
                &mut self.cells,
                Some(count),
                self.limits.graph.acceptance_cells,
                "initial graph registrations",
            )?;
            for particle in 0..count {
                let tidx = if self.tableaux[group].is_some() {
                    i32::try_from(particle).map_err(|_| Error::Limit("tableau index"))?
                } else {
                    -1
                };
                if tidx == -1
                    || self
                        .session
                        .consistent(group, particle, state)
                        .map_err(Error::Phase)?
                {
                    self.register(group, fp, tidx)?;
                    self.groups[group].roots.push((fp, tidx));
                }
            }
        }
        Ok(())
    }
    #[allow(
        clippy::too_many_lines,
        reason = "preflight aggregate reservations before original concrete collection and per-group graph commit"
    )]
    fn source_inner(&mut self, source: &CommittedSource<'_>) -> Result<(), Error> {
        self.last_commit_work = Some(CommitWork {
            source: source.id,
            logical: source.stats,
            batch_generated: source.pool_generated,
        });
        if usize::try_from(source.id).ok() != Some(self.sources) {
            return Err(Error::Input("nonconsecutive actual source commits".into()));
        }
        if self
            .lifecycle
            .as_ref()
            .is_some_and(|state| state.retain_concrete_sources)
        {
            // Reserve the source provenance entry before graph evaluation or
            // retention. Material inspection remains in the original Session
            // meter; reconstruction does not introduce an unbounded second
            // traversal or infer values from fingerprints.
            self.partial_charge(Some(1))?;
            bounded(
                self.sources.checked_add(1),
                self.limits.capture.states,
                "concrete source reconstruction entries",
            )?;
        }
        let incoming_count = source.successors.len().checked_add(1);
        bounded(
            incoming_count,
            self.limits.phase_l.incoming,
            "source admissions before retention",
        )?;
        // Reserve a known main-source product upper bound before bins/consistency.
        // Dynamic recursive admissions are charged separately before retention.
        let products = self
            .tableaux
            .iter()
            .try_fold(0usize, |n, t| {
                n.checked_add(t.as_ref().map_or(1, |t| t.particles.len()))
            })
            .and_then(|n| n.checked_mul(incoming_count?));
        bounded(
            self.vertices
                .checked_add(products.ok_or(Error::Limit("source product reservation"))?),
            self.limits.graph.vertices,
            "source product reservation",
        )?;
        let transitions = self
            .tableaux
            .iter()
            .try_fold(0usize, |n, t| {
                n.checked_add(t.as_ref().map_or(Some(1), |t| {
                    t.particles
                        .iter()
                        .try_fold(0usize, |n, p| n.checked_add(p.successors.len()))
                })?)
            })
            .and_then(|n| n.checked_mul(incoming_count?));
        bounded(
            self.edges
                .checked_add(transitions.ok_or(Error::Limit("source transition reservation"))?),
            self.limits.graph.edges,
            "source transition reservation",
        )?;
        let cells = self.orders.iter().try_fold(0usize, |n, o| {
            n.checked_add(
                o.check_action
                    .len()
                    .checked_mul(incoming_count?)?
                    .checked_add(o.check_state.len())?,
            )
        });
        charge(
            &mut self.cells,
            cells,
            self.limits.graph.acceptance_cells,
            "execution graph bin cells",
        )?;
        let incoming: Vec<_> = source
            .successors
            .iter()
            .map(|edge| Incoming {
                state: edge.state.clone(),
                ordinal: edge.ordinal,
                label: Some(edge.label.clone()),
            })
            .collect();
        let mut receipt = self
            .session
            .collect(source.state, &incoming)
            .map_err(Error::Phase)?;
        let fp = self
            .session
            .concrete_fingerprint(source.state)
            .map_err(Error::Phase)?;
        for group in 0..self.groups.len() {
            self.session
                .group(source.state, &mut receipt, group)
                .map_err(Error::Phase)?;
            charge(
                &mut self.cells,
                receipt
                    .successors
                    .len()
                    .checked_add(receipt.checks.iter().filter(|c| c.group == group).count()),
                self.limits.graph.acceptance_cells,
                "retained group receipt cells",
            )?;
            let own = phase_l::Receipt {
                multiplier: receipt.multiplier,
                capacity: receipt.capacity,
                successors: receipt.successors.clone(),
                checks: receipt
                    .checks
                    .iter()
                    .filter(|c| c.group == group)
                    .cloned()
                    .collect(),
                graph_insertion: false,
            };
            let inserted = self.insert_group(group, fp, &own)?;
            self.groups[group].commits.push(Commit {
                source: self.sources,
                fingerprint: fp,
                receipt: own,
                graph_insertion: inserted,
            });
        }
        self.session.finish_source(&receipt).map_err(Error::Phase)?;
        if let Some(state) = self
            .lifecycle
            .as_mut()
            .filter(|state| state.retain_concrete_sources)
        {
            state
                .concrete_sources
                .push(crate::periodic_lifecycle::ConcreteSource {
                    identity: source.id,
                    fingerprint: fp,
                    values: source.state.clone(),
                });
        }
        self.sources += 1;
        Ok(())
    }
    fn bins(
        &self,
        group: usize,
        receipt: &phase_l::Receipt,
        target: Option<usize>,
    ) -> Result<Vec<bool>, Error> {
        let size = if target.is_some() {
            self.orders[group].check_action.len()
        } else {
            self.orders[group].check_state.len()
        };
        let mut result = Vec::with_capacity(size);
        for bin in 0..size {
            result.push(
                receipt
                    .checks
                    .iter()
                    .find(|c| c.group == group && c.bin == bin && c.successor == target)
                    .map(|c| c.holds)
                    .ok_or_else(|| Error::Input("missing committed bin".into()))?,
            );
        }
        Ok(result)
    }
    fn write(
        &mut self,
        group: usize,
        id: usize,
        node: Node,
        before: usize,
        unconditional_empty: bool,
    ) -> bool {
        if node.transitions.len() > before || (unconditional_empty && before == 0) {
            let g = &mut self.groups[group];
            g.nodes[id] = node;
            g.nodes[id].written = true;
            g.writes.push(id);
            true
        } else {
            false
        }
    }
    #[allow(
        clippy::too_many_lines,
        reason = "preserve bin, consistency, done marking, original slot and recursive commit order"
    )]
    fn insert_group(
        &mut self,
        group: usize,
        fp: u64,
        receipt: &phase_l::Receipt,
    ) -> Result<bool, Error> {
        let state_bins = self.bins(group, receipt, None)?;
        if self.tableaux[group].is_none() {
            let (id, _) = self.register(group, fp, -1)?;
            let mut node = self.groups[group].nodes[id].clone();
            let before = node.transitions.len();
            node.state = state_bins;
            for (slot, next) in receipt.successors.iter().enumerate() {
                charge(
                    &mut self.steps,
                    Some(1),
                    self.limits.graph.steps,
                    "execution graph inspections",
                )?;
                if self.partial
                    && !self.groups[group]
                        .records
                        .contains_key(&(next.fingerprint, -1))
                {
                    // Only an actual stored slot may introduce this unfinished
                    // record; never invent adjacency or concrete bin results.
                    self.partial_charge(Some(1))?;
                    let (target, _) = self.register(group, next.fingerprint, -1)?;
                    self.successor_origins.insert(
                        (group, target),
                        SuccessorOrigin {
                            source: self.sources,
                            source_fingerprint: fp,
                            successor: slot,
                            slot: next.slot,
                            ordinal: next.ordinal,
                        },
                    );
                }
                let written = self.groups[group]
                    .records
                    .get(&(next.fingerprint, -1))
                    .is_some_and(|id| self.groups[group].nodes[*id].written);
                if !written
                    || !node
                        .transitions
                        .iter()
                        .any(|t| t.target == next.fingerprint)
                {
                    charge(
                        &mut self.edges,
                        Some(1),
                        self.limits.graph.edges,
                        "execution graph transitions",
                    )?;
                    node.transitions.push(Transition {
                        target: next.fingerprint,
                        tableau: -1,
                        action: self.bins(group, receipt, Some(slot))?,
                    });
                }
            }
            return Ok(self.write(group, id, node, before, true));
        }
        let table = self.tableaux[group]
            .as_ref()
            .ok_or_else(|| Error::Input("tableau shape".into()))?;
        let particles = table.particles.len();
        let cells = particles.checked_mul(receipt.successors.len());
        charge(
            &mut self.cells,
            cells,
            self.limits.graph.acceptance_cells,
            "tableau consistency cells",
        )?;
        let mut consistent =
            Vec::with_capacity(cells.ok_or(Error::Limit("tableau consistency cells"))?);
        // Actual pinned tableau-node outer, slot inner consistency order.
        for particle in 0..particles {
            for successor in &receipt.successors {
                consistent.push(
                    self.session
                        .consistent(group, particle, &successor.state)
                        .map_err(Error::Phase)?,
                );
            }
        }
        self.groups[group].done.insert(fp);
        let ids: Vec<_> = self.groups[group]
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(id, node)| (node.fingerprint == fp).then_some(id))
            .collect();
        let mut inserted = false;
        for id in ids {
            let mut node = self.groups[group].nodes[id].clone();
            let before = node.transitions.len();
            node.state.clone_from(&state_bins);
            let nexts = self.next_particles(group, node.tableau)?;
            for (slot, successor) in receipt.successors.iter().enumerate() {
                let action = self.bins(group, receipt, Some(slot))?;
                for particle in &nexts {
                    charge(
                        &mut self.steps,
                        Some(1),
                        self.limits.graph.steps,
                        "execution graph inspections",
                    )?;
                    if consistent[*particle * receipt.successors.len() + slot] {
                        self.transition(
                            group,
                            &mut node,
                            successor.fingerprint,
                            *particle,
                            &action,
                            &successor.state,
                            0,
                        )?;
                    }
                }
            }
            inserted |= self.write(group, id, node, before, true);
        }
        Ok(inserted)
    }
    fn next_particles(&self, group: usize, particle: i32) -> Result<Vec<usize>, Error> {
        usize::try_from(particle)
            .ok()
            .and_then(|p| self.tableaux[group].as_ref()?.particles.get(p))
            .map(|p| p.successors.clone())
            .ok_or_else(|| Error::Input("tableau source reference".into()))
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "retain original concrete target and logical recursive depth separate from fingerprint/particle identity"
    )]
    fn transition(
        &mut self,
        group: usize,
        node: &mut Node,
        fp: u64,
        particle: usize,
        action: &[bool],
        concrete: &phase_l::State,
        depth: usize,
    ) -> Result<(), Error> {
        let tidx = i32::try_from(particle).map_err(|_| Error::Limit("tableau index"))?;
        let absent = !self.groups[group].records.contains_key(&(fp, tidx));
        if absent
            || !node
                .transitions
                .iter()
                .any(|t| t.target == fp && t.tableau == tidx)
        {
            charge(
                &mut self.edges,
                Some(1),
                self.limits.graph.edges,
                "execution graph transitions",
            )?;
            node.transitions.push(Transition {
                target: fp,
                tableau: tidx,
                action: action.to_vec(),
            });
            let (id, new) = self.register(group, fp, tidx)?;
            if new && self.groups[group].done.contains(&fp) {
                self.revisit(group, id, concrete, depth + 1)?;
            }
        }
        Ok(())
    }
    #[allow(
        clippy::too_many_lines,
        reason = "match recursive state bins, stutter and full action StateVec evaluation/error order"
    )]
    fn revisit(
        &mut self,
        group: usize,
        id: usize,
        source: &phase_l::State,
        depth: usize,
    ) -> Result<(), Error> {
        self.recursive_expansions += 1;
        if depth > 256 {
            return Err(Error::Limit("tableau recursive source depth"));
        }
        charge(
            &mut self.steps,
            Some(1),
            self.limits.graph.steps,
            "tableau recursive source visits",
        )?;
        let mut node = self.groups[group].nodes[id].clone();
        let before = node.transitions.len();
        node.state = self
            .session
            .direct_bins(group, source, None)
            .map_err(Error::Phase)?;
        let nexts = self.next_particles(group, node.tableau)?;
        let self_action = if nexts.is_empty() {
            Vec::new()
        } else {
            self.session
                .direct_bins(group, source, Some(source))
                .map_err(Error::Phase)?
        };
        for particle in &nexts {
            charge(
                &mut self.steps,
                Some(1),
                self.limits.graph.steps,
                "recursive stutter inspections",
            )?;
            if self
                .session
                .consistent(group, *particle, source)
                .map_err(Error::Phase)?
            {
                let accepting = self.tableaux[group]
                    .as_ref()
                    .is_some_and(|t| t.particles[*particle].accepting);
                let empty = self.orders[group].pems.iter().any(|p| {
                    p.ea_action.is_empty() && p.ae_action.is_empty() && p.ae_state.is_empty()
                });
                if accepting && empty {
                    return Err(Error::Phase(Box::new(phase_l::Failure {
                        error: phase_l::Error::Unsupported(
                            "tableau accepting-sink early witness reconstruction",
                        ),
                        progress: self.session.progress().clone(),
                        partial: None,
                    })));
                }
                let self_fp = node.fingerprint;
                self.transition(
                    group,
                    &mut node,
                    self_fp,
                    *particle,
                    &self_action,
                    source,
                    depth,
                )?;
            }
        }
        // TLC materializes each complete action StateVec BEFORE constraints or
        // action/consistency checks; no streaming predicate evaluation here.
        for action in &self.model.actions {
            let mut raw: Vec<phase_l::State> = Vec::new();
            let mut bound = None;
            let result = self
                .model
                .evaluator
                .next_states(action, source, 0, &mut |successor| {
                    self.recursive_generated = self.recursive_generated.saturating_add(1);
                    if self.recursive_generated > self.limits.capture.generated
                        || self.recursive_retained >= self.limits.capture.edges
                    {
                        bound = Some(Error::Limit(
                            "recursive action StateVec generation/retention",
                        ));
                        return dive_eval::next::Flow::Stop;
                    }
                    self.recursive_retained += 1;
                    raw.push(Arc::from(successor.state));
                    dive_eval::next::Flow::Continue
                });
            if let Some(error) = bound {
                return Err(error);
            }
            result.map_err(|error| {
                Error::Phase(Box::new(phase_l::Failure {
                    error: phase_l::Error::Evaluation(error.error),
                    progress: self.session.progress().clone(),
                    partial: None,
                }))
            })?;
            for target in raw {
                if target.iter().any(Option::is_none) {
                    return Err(Error::Phase(Box::new(phase_l::Failure {
                        error: phase_l::Error::Unsupported(
                            "incomplete recursive successor classification",
                        ),
                        progress: self.session.progress().clone(),
                        partial: None,
                    })));
                }
                let st = dive_eval::St::action(source.clone(), target.clone());
                let mut admitted = true;
                for (constraint, action) in self
                    .model
                    .constraints
                    .iter()
                    .map(|c| (c, false))
                    .chain(self.model.action_constraints.iter().map(|c| (c, true)))
                {
                    charge(
                        &mut self.steps,
                        Some(1),
                        self.limits.graph.steps,
                        "recursive constraint inspections",
                    )?;
                    let current = if action {
                        st.clone()
                    } else {
                        dive_eval::St::state(target.clone())
                    };
                    let value = self
                        .model
                        .eval(&constraint.pred, &current)
                        .map_err(|error| {
                            Error::Phase(Box::new(phase_l::Failure {
                                error: phase_l::Error::Evaluation(error),
                                progress: self.session.progress().clone(),
                                partial: None,
                            }))
                        })?;
                    let Some(holds) = value.as_bool() else {
                        return Err(Error::Phase(Box::new(phase_l::Failure {
                            error: phase_l::Error::Unsupported(
                                "non-Boolean recursive constraint classification",
                            ),
                            progress: self.session.progress().clone(),
                            partial: None,
                        })));
                    };
                    if !holds {
                        admitted = false;
                        break;
                    }
                }
                if !admitted {
                    continue;
                }
                let fp = self
                    .session
                    .concrete_fingerprint(&target)
                    .map_err(Error::Phase)?;
                let bins = self
                    .session
                    .direct_bins(group, source, Some(&target))
                    .map_err(Error::Phase)?;
                for particle in &nexts {
                    charge(
                        &mut self.steps,
                        Some(1),
                        self.limits.graph.steps,
                        "recursive successor inspections",
                    )?;
                    if self
                        .session
                        .consistent(group, *particle, &target)
                        .map_err(Error::Phase)?
                    {
                        self.transition(group, &mut node, fp, *particle, &bins, &target, depth)?;
                    }
                }
            }
        }
        self.write(group, id, node, before, false);
        Ok(())
    }
    fn retain(&mut self, result: Result<(), Error>) -> Result<(), String> {
        result.map_err(|error| {
            let message = error.to_string();
            self.error = Some(error);
            message
        })
    }
}
enum HookOrigin<'a, 'source> {
    Source(&'a CommittedSource<'source>),
    Request {
        request: dive_check::progress::Request,
        stats: Stats,
        graph_source: Option<u64>,
    },
}
struct NativeHooks<'a, 'model, 'source, 'pause> {
    observer: &'a mut Observer<'model>,
    state: &'a mut crate::periodic_lifecycle::State,
    origin: HookOrigin<'a, 'source>,
    pause: &'a capture::DispatchPause<'pause>,
    resumed: bool,
}
impl crate::periodic::Hooks for NativeHooks<'_, '_, '_, '_> {
    type Error = Error;
    fn growth_due(&mut self) -> Result<bool, Error> {
        self.observer.pointer_sizes = self.observer.pointer_counts()?;
        Ok(self
            .observer
            .pointer_sizes
            .iter()
            .zip(&self.observer.last_sizes)
            .any(|(size, last)| {
                Growth {
                    size: *size,
                    last: *last,
                }
                .due(self.state.options.backend.threshold)
            }))
    }
    fn suspend_workers(&mut self) -> Result<bool, Error> {
        if matches!(self.origin, HookOrigin::Source(source) if self.pause.workers() != source.barrier.workers())
        {
            return Err(Error::Input("dispatch pause/source pool mismatch".into()));
        }
        Ok(true)
    }
    fn check_liveness(&mut self, forced: bool) -> Result<i32, Error> {
        let saved = self.observer.periodic;
        self.observer.periodic = Some(Policy {
            force: forced,
            ..self.state.options.backend
        });
        let (stats, barrier) = match self.origin {
            HookOrigin::Source(source) => (source.stats, Some(source.barrier)),
            HookOrigin::Request { stats, .. } => (stats, None),
        };
        let result = self.observer.periodic_check(stats, barrier);
        self.observer.periodic = saved;
        match result {
            Ok(()) => Ok(0),
            Err(Error::PartialCandidate) => Ok(2116),
            Err(e) => Err(e),
        }
    }
    fn time_millis(&mut self) -> Result<i64, Error> {
        crate::periodic_lifecycle::time_millis()
    }
    fn update_runtime_ratio(&mut self, elapsed: i64) -> Result<f64, Error> {
        crate::periodic::calculate_runtime_ratio(
            self.state.ratio,
            self.state.start,
            crate::periodic_lifecycle::time_millis()?,
            self.state.options.progress_interval,
            elapsed,
        )
        .map_err(|_| Error::Execution("negative native liveness elapsed time".into()))
    }
    fn periodic_is_false(&mut self) -> Result<bool, Error> {
        let (pred, admission_work) = self
            .state
            .hook
            .as_ref()
            .ok_or_else(|| Error::Input("missing supplied PERIODIC expression".into()))?;
        // Charge before evaluation and retaining its receipt. The original
        // graph+partial+hook bookkeeping quota remains shared.
        self.observer
            .partial_charge(crate::periodic_hook::evaluation_charge(*admission_work))?;
        let result = self
            .observer
            .model
            .eval(pred, &dive_eval::St::empty())
            .and_then(|value| {
                self.observer
                    .model
                    .evaluator
                    .equal_values(&dive_value::Value::Bool(false), &value)
            })
            .map_err(|error| error.at(pred.expr));
        match self.origin {
            HookOrigin::Source(source) => {
                self.state
                    .hook_evaluations
                    .push(crate::periodic_hook::Evaluation {
                        source: source.id,
                        expression: pred.expr,
                        result: result.clone(),
                    });
            }
            HookOrigin::Request {
                request,
                graph_source,
                ..
            } => self
                .state
                .service_evaluations
                .push(crate::periodic_service::Evaluation {
                    request,
                    graph_source,
                    expression: pred.expr,
                    result: result.clone(),
                }),
        }
        result.map_err(Error::PeriodicEvaluation)
    }
    fn checkpoint(&mut self) -> Result<(), Error> {
        Err(Error::Input(
            "checkpoint transaction is not admitted by native lifecycle".into(),
        ))
    }
    fn resume_workers(&mut self) -> Result<(), Error> {
        self.resumed = true;
        Ok(())
    }
}

impl SourceObserver for Observer<'_> {
    fn progress_requests(&self) -> Option<dive_check::progress::Options> {
        self.lifecycle.as_ref().and_then(|state| state.requests)
    }
    fn expansion_requests_cancelled(&mut self, pending: Option<dive_check::progress::Request>) {
        if let Some(state) = &mut self.lifecycle {
            state.requests_cancelled = true;
            state.aborted_request = pending;
        }
    }
    fn expansion_quiescent(
        &mut self,
        receipt: capture::ExpansionQuiescence,
        pause: &capture::DispatchPause<'_>,
    ) -> Result<(), String> {
        // Charge before retaining each bounded at-most-eight-source receipt.
        let cost = 1usize
            .checked_add(receipt.claimed().len())
            .and_then(|c| c.checked_add(receipt.completed().len()))
            .and_then(|c| c.checked_add(receipt.failed().len()));
        let Some(mut state) = self.lifecycle.take() else {
            return self.retain(Err(Error::Input(
                "request pause without owned lifecycle".into(),
            )));
        };
        let result = self.partial_charge(cost).and_then(|()| {
            let result = if state.request_service {
                self.lifecycle_service(&receipt, pause, &mut state)
            } else {
                Ok(())
            };
            state
                .expansion_pauses
                .push(crate::periodic_lifecycle::ExpansionPause {
                    receipt,
                    workers: pause.workers(),
                    generation: pause.generation(),
                });
            result
        });
        self.lifecycle = Some(state);
        self.retain(result)
    }
    fn initial(
        &mut self,
        _id: u64,
        state: &phase_l::State,
        _ordinal: u64,
        _stats: Stats,
    ) -> Result<(), String> {
        let result = self.initial_inner(state).and_then(|()| {
            if self.periodic.is_none() {
                self.partial_check()
            } else {
                Ok(())
            }
        });
        self.retain(result)
    }
    fn initial_complete(&mut self, stats: Stats) -> Result<(), String> {
        let result = if let Some(state) = &mut self.lifecycle {
            state.initial_stats = Some(stats);
            Ok(())
        } else {
            self.periodic_check(stats, None)
        };
        self.retain(result)
    }
    fn requires_dispatch_pause(&self) -> bool {
        self.lifecycle.is_some()
    }
    fn paused_source(
        &mut self,
        source: CommittedSource<'_>,
        pause: &capture::DispatchPause<'_>,
    ) -> Result<(), String> {
        let Some(mut state) = self.lifecycle.take() else {
            return self.source(source);
        };
        let result = self.source_inner(&source).and_then(|()| {
            state.graph_source = Some(source.id);
            if state.request_service {
                Ok(())
            } else {
                self.lifecycle_poll(&source, pause, &mut state)
            }
        });
        self.lifecycle = Some(state);
        self.retain(result)
    }
    fn exploration_complete(&mut self, end: &capture::ExplorationEnd) -> Result<(), String> {
        let Some(mut state) = self.lifecycle.take() else {
            return Ok(());
        };
        let result = self.lifecycle_final(&mut state, end);
        self.lifecycle = Some(state);
        self.retain(result)
    }
    fn source(&mut self, source: CommittedSource<'_>) -> Result<(), String> {
        if self.lifecycle.is_some() {
            return self.retain(Err(Error::Input(
                "native lifecycle requires dispatch-pause authority".into(),
            )));
        }
        let result = self.source_inner(&source).and_then(|()| {
            if self.periodic.is_some() {
                self.periodic_check(source.stats, Some(source.barrier))
            } else {
                self.partial_check()
            }
        });
        self.retain(result)
    }
}

/// Execute ordered source callbacks, multiple OOSs and bounded tableau products.
/// Initialization registration occurs before initial invariant checking. Each
/// source evaluates a group's bins then inserts that group before the next group.
/// A deciding temporal error stops before later source safety/enumeration errors.
/// No temporal completion/verdict, exact TLC lasso or schedule claim is made.
///
/// # Errors
/// Preflight/ownership refusals, capture stops, deciding predicate errors, lost
/// worker evidence or named cumulative entry/work limits with persisted prefix.
#[allow(
    clippy::too_many_lines,
    reason = "preserve parameter, admission, execution and deciding error evidence explicitly"
)]
pub fn execute(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
) -> Result<Complete, Box<Failure>> {
    execute_inner(model, options, limits, false, None, None, None)
        .map(|(execution, _, _, _, _, _)| execution)
        .map_err(|failure| failure.failure)
}

/// Execute actual ordered graph commits and check each partial source prefix.
///
/// This diagnostic checks after initial registrations and whole source callbacks.
/// It deliberately continues after candidates to observe graph growth, so neither
/// its checks nor a completed safety search imply ordinary temporal completion.
/// No caller-supplied graph, done mask or claimed validation flag is accepted.
/// Candidates must be independently replayed before any violation certification.
/// Existing fixed graph limits apply; repeated checks share a cumulative work
/// ceiling. This is not the pinned wall-clock schedule.
///
/// # Errors
/// Original execution/admission errors, partial SCC failures and fixed limits,
/// retaining all previously completed checks and actual stopping evidence.
pub fn execute_partial(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
) -> Result<PartialExecution, Box<PartialFailure>> {
    execute_inner(model, options, limits, true, None, None, None).map(
        |(execution, checks, inspections, graph_inspections, _, _)| PartialExecution {
            execution,
            checks,
            inspections,
            graph_inspections,
        },
    )
}

pub(crate) fn execute_periodic_backend(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: Policy,
) -> crate::periodic_backend::Run {
    match execute_inner(model, options, limits, true, Some(policy), None, None) {
        Ok((complete, checks, inspections, graph_inspections, checkpoints, _)) => {
            crate::periodic_backend::Run {
                complete: Some(complete),
                failure: None,
                checks,
                inspections,
                graph_inspections,
                checkpoints,
                options: options.clone(),
                policy,
                limits,
            }
        }
        Err(failure) => crate::periodic_backend::Run {
            complete: None,
            failure: Some(failure.failure),
            checks: failure.checks,
            inspections: failure.inspections,
            graph_inspections: failure.graph_inspections,
            checkpoints: failure.periodic_checks,
            options: options.clone(),
            policy,
            limits,
        },
    }
}

pub(crate) fn execute_periodic_lifecycle(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: crate::periodic_lifecycle::Options,
) -> crate::periodic_lifecycle::Run {
    execute_periodic_lifecycle_hook(model, options, limits, policy, None)
}

pub(crate) fn execute_periodic_lifecycle_hook(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: crate::periodic_lifecycle::Options,
    hook: Option<(dive_model::Pred, usize)>,
) -> crate::periodic_lifecycle::Run {
    execute_periodic_lifecycle_requests(model, options, limits, policy, hook, None)
}

pub(crate) fn execute_periodic_lifecycle_requests(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: crate::periodic_lifecycle::Options,
    hook: Option<(dive_model::Pred, usize)>,
    requests: Option<dive_check::progress::Options>,
) -> crate::periodic_lifecycle::Run {
    execute_lifecycle_mode(model, options, limits, policy, hook, requests, false, false)
}
pub(crate) fn execute_periodic_service(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: crate::periodic_lifecycle::Options,
    hook: Option<(dive_model::Pred, usize)>,
    requests: dive_check::progress::Options,
    retain_sources: bool,
) -> crate::periodic_lifecycle::Run {
    execute_lifecycle_mode(
        model,
        options,
        limits,
        policy,
        hook,
        Some(requests),
        true,
        retain_sources,
    )
}
#[allow(
    clippy::too_many_arguments,
    reason = "separate existing service mode from opt-in reconstruction authority"
)]
fn execute_lifecycle_mode(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    policy: crate::periodic_lifecycle::Options,
    hook: Option<(dive_model::Pred, usize)>,
    requests: Option<dive_check::progress::Options>,
    service: bool,
    retain_sources: bool,
) -> crate::periodic_lifecycle::Run {
    let mut failed_state = None;
    let outcome = execute_inner(
        model,
        options,
        limits,
        true,
        Some(policy.backend),
        Some((policy, hook, requests, service, retain_sources)),
        Some(&mut failed_state),
    );
    let (backend, mut state) = match outcome {
        Ok((complete, checks, inspections, graph_inspections, checkpoints, state)) => (
            crate::periodic_backend::Run {
                complete: Some(complete),
                failure: None,
                checks,
                inspections,
                graph_inspections,
                checkpoints,
                options: options.clone(),
                policy: policy.backend,
                limits,
            },
            state,
        ),
        Err(failure) => (
            crate::periodic_backend::Run {
                complete: None,
                failure: Some(failure.failure),
                checks: failure.checks,
                inspections: failure.inspections,
                graph_inspections: failure.graph_inspections,
                checkpoints: failure.periodic_checks,
                options: options.clone(),
                policy: policy.backend,
                limits,
            },
            failed_state,
        ),
    };
    let (request_services, service_evaluations, concrete_sources) = state.as_mut().map_or_else(
        || (Vec::new(), Vec::new(), Vec::new()),
        |state| {
            (
                std::mem::take(&mut state.request_services),
                std::mem::take(&mut state.service_evaluations),
                std::mem::take(&mut state.concrete_sources),
            )
        },
    );
    let (
        invocations,
        final_checks,
        eof,
        hook_evaluations,
        expansion_pauses,
        request_options,
        requests_cancelled,
        aborted_request,
    ) = state.map_or_else(
        || {
            (
                Vec::new(),
                Vec::new(),
                None,
                Vec::new(),
                Vec::new(),
                requests,
                false,
                None,
            )
        },
        |s| {
            (
                s.invocations,
                s.final_checks,
                s.eof,
                s.hook_evaluations,
                s.expansion_pauses,
                s.requests,
                s.requests_cancelled,
                s.aborted_request,
            )
        },
    );
    crate::periodic_lifecycle::Run {
        concrete_sources,
        request_services,
        service_evaluations,
        backend,
        options: policy,
        invocations,
        final_checks,
        eof,
        hook_evaluations,
        expansion_pauses,
        request_options,
        requests_cancelled,
        aborted_request,
    }
}

type ExecutionParts = (
    Complete,
    Vec<PartialCheck>,
    usize,
    usize,
    Vec<Checkpoint>,
    Option<crate::periodic_lifecycle::State>,
);

type LifecycleInput = (
    crate::periodic_lifecycle::Options,
    Option<(dive_model::Pred, usize)>,
    Option<dive_check::progress::Options>,
    bool,
    bool,
);

#[allow(
    clippy::too_many_lines,
    reason = "preserve original execution error/prefix facts with optional partial checks"
)]
fn execute_inner(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
    partial: bool,
    periodic: Option<Policy>,
    lifecycle: Option<LifecycleInput>,
    lifecycle_failure: Option<&mut Option<crate::periodic_lifecycle::State>>,
) -> Result<ExecutionParts, Box<PartialFailure>> {
    let early = |error| {
        Box::new(PartialFailure {
            checks: Vec::new(),
            inspections: 0,
            graph_inspections: 0,
            periodic_checks: Vec::new(),
            failure: Box::new(Failure {
                error,
                stats: None,
                workers: Some(Workers {
                    requested: options.workers,
                    used: 0,
                    sequential: false,
                    reason: Some("parameter/ownership/planning check before execution".into()),
                }),
                groups: Vec::new(),
                phase_work: phase_l::Progress::default(),
                recursive_work: RecursiveWork::default(),
                last_commit_work: None,
            }),
        })
    };
    if options.workers == 0 || options.workers > limits.workers || options.workers > 64 {
        return Err(early(Error::Limit("execution requested worker width")));
    }
    if model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .is_err()
    {
        return Err(early(Error::Input(
            "foreign model IR/name table before execution".into(),
        )));
    }
    model
        .validate_context()
        .map_err(|error| early(Error::Input(error.to_string())))?;
    let lifecycle = lifecycle
        .map(|(options, hook, requests, service, retain_sources)| {
            if let Some(requests) = requests {
                requests.validate().map_err(Error::Input)?;
            }
            let mut state = crate::periodic_lifecycle::State::new(options)?;
            state.request_service = service;
            state.retain_concrete_sources = retain_sources;
            state.hook = hook;
            state.requests = requests;
            Ok(state)
        })
        .transpose()
        .map_err(early)?;
    let mut session = Session::new(model, options.fp_index, 1, limits.phase_l)
        .map_err(|error| early(Error::Phase(error)))?;
    if session.orders().is_empty() {
        return Err(early(Error::Phase(Box::new(phase_l::Failure {
            error: phase_l::Error::Unsupported("ordered execution requires temporal OOS groups"),
            progress: session.progress().clone(),
            partial: None,
        }))));
    }
    if lifecycle
        .as_ref()
        .is_some_and(|state| state.retain_concrete_sources)
    {
        session.audit_concrete_sources(limits.capture.states);
    }
    let orders = session.orders().to_vec();
    let tableaux = session.tableaux().to_vec();
    let groups = orders
        .iter()
        .map(|o| Group {
            pems: o.pems.clone(),
            ..Default::default()
        })
        .collect();
    let group_count = orders.len();
    let mut setup_steps = 0;
    if periodic.is_some() {
        charge(
            &mut setup_steps,
            group_count.checked_mul(2),
            limits.graph.steps,
            "periodic backend setup inspections",
        )
        .map_err(early)?;
    }
    let mut observer = Observer {
        model,
        tableaux,
        recursive_generated: 0,
        recursive_retained: 0,
        recursive_expansions: 0,
        session,
        orders,
        groups,
        limits,
        error: None,
        vertices: 0,
        edges: 0,
        cells: 0,
        steps: setup_steps,
        sources: 0,
        last_commit_work: None,
        partial,
        partial_checks: Vec::new(),
        partial_steps: setup_steps,
        successor_origins: BTreeMap::new(),
        periodic,
        periodic_checks: Vec::new(),
        lifecycle,
        pointer_sizes: Vec::new(),
        last_sizes: if periodic.is_some() {
            vec![1; group_count]
        } else {
            Vec::new()
        },
    };
    let captured =
        capture::explore_observed_with_evidence(model, options, limits.capture, &mut observer);
    let captured = match captured {
        Ok(Exploration::Complete(c)) => c,
        stopped => {
            let (error, stats, workers) = match stopped {
                Ok(Exploration::Stopped(stop)) => (
                    observer
                        .error
                        .take()
                        .unwrap_or_else(|| Error::Capture(stop.clone())),
                    stop.stats,
                    Some(stop.workers.clone()),
                ),
                Err(error) => (Error::Execution(error), None, None),
                Ok(Exploration::Complete(_)) => (
                    Error::Input("unexpected complete capture in stopped branch".into()),
                    None,
                    None,
                ),
            };
            if let Some(output) = lifecycle_failure {
                *output = observer.lifecycle.take();
            }
            return Err(Box::new(PartialFailure {
                checks: std::mem::take(&mut observer.partial_checks),
                inspections: observer.partial_steps,
                graph_inspections: observer.steps,
                periodic_checks: std::mem::take(&mut observer.periodic_checks),
                failure: Box::new(Failure {
                    error,
                    stats,
                    workers,
                    recursive_work: observer.recursive_work(),
                    last_commit_work: observer.last_commit_work,
                    groups: observer.groups,
                    phase_work: observer.session.progress().clone(),
                }),
            }));
        }
    };
    Ok((
        Complete {
            capture: captured,
            recursive_work: observer.recursive_work(),
            last_commit_work: observer.last_commit_work,
            groups: observer.groups,
            configured_properties: observer.session.configured_properties().to_vec(),
            expanded_property_names: observer.session.expanded_property_names().to_vec(),
            role_names: observer.session.role_names().to_vec(),
            phase_work: observer.session.progress().clone(),
            limits: limits.graph,
            tableaux: observer.tableaux,
        },
        observer.partial_checks,
        observer.partial_steps,
        observer.steps,
        observer.periodic_checks,
        observer.lifecycle,
    ))
}

#[allow(
    clippy::too_many_lines,
    reason = "bounded product acceptance projection retains explicit occurrence identities"
)]
fn project_inputs(
    group: &Group,
    pem: usize,
    limits: crate::product::Limits,
    tableau: Option<&Tableau>,
) -> Result<(Graph, Acceptance), Error> {
    project_inputs_inner(group, pem, limits, tableau, false)
}

#[allow(
    clippy::too_many_lines,
    reason = "retain original registered/write identities for final and partial projection"
)]
fn project_inputs_inner(
    group: &Group,
    pem: usize,
    limits: crate::product::Limits,
    tableau: Option<&Tableau>,
    partial: bool,
) -> Result<(Graph, Acceptance), Error> {
    let nodes = &group.nodes;
    let roots = &group.roots;
    let pems = &group.pems;
    if !partial && nodes.iter().any(|n| !n.written) {
        return Err(Error::Input(
            "unwritten product node after execution".into(),
        ));
    }
    let pem = pems
        .get(pem)
        .ok_or_else(|| Error::Input("PEM reference".into()))?;
    let edges_count = nodes
        .iter()
        .try_fold(0usize, |n, node| n.checked_add(node.transitions.len()));
    let edges_count = bounded(edges_count, limits.edges, "product edges")?;
    bounded(Some(nodes.len()), limits.vertices, "product vertices")?;
    let state_cells = pem
        .ae_state
        .len()
        .checked_add(tableau.map_or(0, |t| t.promises.len()))
        .ok_or(Error::Limit("product acceptance cells"))?;
    bounded(
        nodes.len().checked_mul(state_cells).and_then(|n| {
            edges_count
                .checked_mul(pem.ae_action.len().checked_add(1)?)?
                .checked_add(n)
        }),
        limits.acceptance_cells,
        "product acceptance cells",
    )?;
    bounded(
        nodes.len().checked_add(edges_count),
        limits.steps,
        "product inspections",
    )?;
    let ids: BTreeMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| ((node.fingerprint, node.tableau), i))
        .collect();
    let edges = nodes
        .iter()
        .map(|node| {
            node.transitions
                .iter()
                .map(|t| {
                    ids.get(&(t.target, t.tableau))
                        .copied()
                        .ok_or_else(|| Error::Input("unwritten graph target".into()))
                })
                .collect()
        })
        .collect::<Result<Vec<Vec<_>>, _>>()?;
    let roots = roots
        .iter()
        .map(|fp| {
            ids.get(fp)
                .copied()
                .ok_or_else(|| Error::Input("unwritten graph root".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let graph = Graph::new(edges, roots).map_err(|e| Error::Input(e.to_string()))?;
    let action = |bins: &[usize]| {
        nodes
            .iter()
            .map(|node| {
                node.transitions
                    .iter()
                    .map(|t| {
                        bins.iter().try_fold(true, |v, bin| {
                            t.action
                                .get(*bin)
                                .copied()
                                .map(|b| v && b)
                                .ok_or_else(|| Error::Input("PEM action bin".into()))
                        })
                    })
                    .collect()
            })
            .collect::<Result<Vec<Vec<bool>>, Error>>()
    };
    let recurrent = action(&pem.ea_action)?;
    let actions = pem
        .ae_action
        .iter()
        .map(|bin| action(&[*bin]))
        .collect::<Result<Vec<_>, _>>()?;
    let mut states = pem
        .ae_state
        .iter()
        .map(|bin| {
            nodes
                .iter()
                .map(|node| {
                    if partial && !node.written {
                        return Ok(false);
                    }
                    node.state
                        .get(*bin)
                        .copied()
                        .ok_or_else(|| Error::Input("PEM state bin".into()))
                })
                .collect()
        })
        .collect::<Result<Vec<Vec<bool>>, Error>>()?;
    if let Some(tableau) = tableau {
        charge(
            &mut 0usize,
            nodes.len().checked_mul(tableau.promises.len()),
            limits.acceptance_cells,
            "product promise cells",
        )?;
        for promise in 0..tableau.promises.len() {
            states.push(
                nodes
                    .iter()
                    .map(|n| {
                        usize::try_from(n.tableau)
                            .ok()
                            .and_then(|id| tableau.particles.get(id))
                            .and_then(|p| p.fulfilling.get(promise))
                            .copied()
                            .ok_or_else(|| Error::Input("product promise identity".into()))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
    }
    Ok((
        graph,
        Acceptance {
            recurrent,
            states,
            actions,
        },
    ))
}

fn bounded(n: Option<usize>, maximum: usize, name: &'static str) -> Result<usize, Error> {
    n.filter(|n| *n <= maximum).ok_or(Error::Limit(name))
}

#[cfg(test)]
mod partial_origin_tests {
    use super::{Group, Node, SuccessorOrigin, validate_origin};
    use crate::{phase_graph::Commit, phase_l};

    #[test]
    fn placeholder_requires_actual_matching_committed_successor_slot() {
        let node = Node {
            fingerprint: 71,
            tableau: -1,
            state: Vec::new(),
            transitions: Vec::new(),
            written: false,
        };
        let origin = SuccessorOrigin {
            source: 0,
            source_fingerprint: 21,
            successor: 0,
            slot: 9,
            ordinal: Some(8),
        };
        let mut group = Group::default();
        assert!(validate_origin(&group, &node, &origin).is_err());
        group.commits.push(Commit {
            source: 0,
            fingerprint: 21,
            receipt: phase_l::Receipt {
                multiplier: 1,
                capacity: 16,
                successors: vec![phase_l::Representative {
                    slot: 9,
                    fingerprint: 71,
                    state: Vec::new().into(),
                    ordinal: Some(8),
                    label: None,
                }],
                checks: Vec::new(),
                graph_insertion: false,
            },
            graph_insertion: true,
        });
        assert!(validate_origin(&group, &node, &origin).is_ok());
        for edited in [
            SuccessorOrigin {
                source: 1,
                ..origin.clone()
            },
            SuccessorOrigin {
                source_fingerprint: 22,
                ..origin.clone()
            },
            SuccessorOrigin {
                successor: 1,
                ..origin.clone()
            },
            SuccessorOrigin {
                slot: 10,
                ..origin.clone()
            },
            SuccessorOrigin {
                ordinal: None,
                ..origin
            },
        ] {
            assert!(validate_origin(&group, &node, &edited).is_err());
        }
        let invented = Node {
            fingerprint: 72,
            ..node
        };
        assert!(
            validate_origin(
                &group,
                &invented,
                &SuccessorOrigin {
                    source: 0,
                    source_fingerprint: 21,
                    successor: 0,
                    slot: 9,
                    ordinal: Some(8),
                }
            )
            .is_err()
        );
    }
}
