//! Ordered concrete safety commits with interleaved native temporal graph work.
//!
//! This bounded execution slice implements multiple actual tableau/null OOS groups.
//! It has real source-prefix ordering and graph insertion, but certifies no TLC
//! SCC traversal, lasso choice, periodic schedule or model-checking verdict.

use crate::Graph;
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
    /// Original configured/action property identities.
    pub configured_properties: Vec<dive_model::Named>,
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
impl SourceObserver for Observer<'_> {
    fn initial(
        &mut self,
        _id: u64,
        state: &phase_l::State,
        _ordinal: u64,
        _stats: Stats,
    ) -> Result<(), String> {
        let result = self.initial_inner(state);
        self.retain(result)
    }
    fn source(&mut self, source: CommittedSource<'_>) -> Result<(), String> {
        let result = self.source_inner(&source);
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
    let early = |error| {
        Box::new(Failure {
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
    let session = Session::new(model, options.fp_index, 1, limits.phase_l)
        .map_err(|error| early(Error::Phase(error)))?;
    if session.orders().is_empty() {
        return Err(early(Error::Phase(Box::new(phase_l::Failure {
            error: phase_l::Error::Unsupported("ordered execution requires temporal OOS groups"),
            progress: session.progress().clone(),
            partial: None,
        }))));
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
        steps: 0,
        sources: 0,
        last_commit_work: None,
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
            return Err(Box::new(Failure {
                error,
                stats,
                workers,
                recursive_work: observer.recursive_work(),
                last_commit_work: observer.last_commit_work,
                groups: observer.groups,
                phase_work: observer.session.progress().clone(),
            }));
        }
    };
    Ok(Complete {
        capture: captured,
        recursive_work: observer.recursive_work(),
        last_commit_work: observer.last_commit_work,
        groups: observer.groups,
        configured_properties: observer.session.configured_properties().to_vec(),
        role_names: observer.session.role_names().to_vec(),
        phase_work: observer.session.progress().clone(),
        limits: limits.graph,
        tableaux: observer.tableaux,
    })
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
    let nodes = &group.nodes;
    let roots = &group.roots;
    let pems = &group.pems;
    if nodes.iter().any(|n| !n.written) {
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
