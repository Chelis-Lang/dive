//! Bounded products of a supplied behavior graph and a temporal tableau.
//!
//! State labels describe vertices; action labels describe each original edge.
//! A tableau node constrains the edge leaving its word position, including
//! loop closure. This module does not enumerate states or evaluate predicates.
//! Only its limits, errors and acceptance policy are public; the temporal route
//! builds the private on-the-fly product.

#[cfg(test)]
use crate::Graph;
#[cfg(test)]
use crate::formula::AtomKind;
use crate::formula::{CycleConditions, Recurrence, Tableau};
use crate::graph::GraphAccess;
#[cfg(test)]
use crate::search::{Acceptance, Lasso, Step, find_lasso, find_lasso_view};
use crate::search::{AcceptanceAccess, SearchError};
#[cfg(test)]
use std::collections::{BTreeSet, HashMap};

mod csr;
use csr::ProductRows;
#[cfg(test)]
use csr::{CsrGraph, FlatRows};
#[cfg(test)]
use csr::{OwnedRows, RowStorage};
mod implicit;
mod pair_order;
pub(crate) use implicit::ImplicitProduct;

mod behavior;
pub(crate) use behavior::BehaviorView;
mod topology;
pub(crate) use topology::CsrTopology;

mod budget;
pub use budget::AcceptancePolicy;
mod truth;
pub(crate) use truth::PackedTruth;
use truth::TruthAccess;

/// Dense truth values of all supplied atomic predicates, the tests' reference
/// representation beside the temporal route's [`PackedTruth`].
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Labels {
    /// One vertex-shaped row per state predicate.
    pub states: Vec<Vec<bool>>,
    /// One adjacency-shaped table per action predicate. Parallel edges retain
    /// distinct labels even when their source and destination agree.
    pub actions: Vec<Vec<Vec<bool>>>,
}

/// Resource bounds for explicit product construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum reachable behavior-vertex/tableau-node pairs.
    pub vertices: usize,
    /// Maximum retained product edges.
    pub edges: usize,
    /// Maximum boolean cells for promises, recurrence and recurrent-edge labels.
    pub acceptance_cells: usize,
    /// Maximum pair, behavior-edge and candidate-transition inspections.
    pub steps: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            vertices: 1_000_000,
            edges: 8_000_000,
            acceptance_cells: 8_000_000,
            steps: 32_000_000,
        }
    }
}

/// Malformed inputs, construction limits or a search inconsistency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductError {
    /// A tableau reference or predicate table has the wrong shape.
    Shape(String),
    /// A named resource bound was exceeded; never an empty-language result.
    Limit(&'static str),
    /// The validated product failed to provide its promised witness.
    Search(SearchError),
}

impl std::fmt::Display for ProductError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape(s) => write!(f, "invalid temporal product: {s}"),
            Self::Limit(s) => write!(f, "temporal product resource limit: {s}"),
            Self::Search(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for ProductError {}

impl ProductError {
    pub(crate) fn search(error: SearchError) -> Self {
        match error {
            SearchError::Budget => Self::Limit(crate::budget::EXHAUSTED),
            SearchError::Allocation => Self::Limit("search allocation"),
            other => Self::Search(other),
        }
    }
}

/// An explicit owned product, in `(behavior vertex, tableau node)` order, the
/// unit tests' independent reference for the compact and on-the-fly products.
/// Input graph edge positions are retained for witness projection.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Product {
    graph: Graph,
    acceptance: Acceptance,
    behavior_states: Vec<usize>,
    original_edges: Vec<Vec<usize>>,
}

#[cfg(test)]
struct Parts<A, G> {
    graph: G,
    acceptance: A,
    behavior_states: Vec<usize>,
}

// Borrowed only while finalizing one builder; both routes retain the same
// renumbering, adjacency order and original-edge projection. An on-the-fly
// product stores no rows; only recurrence conditions read them.
struct Ordered<'a> {
    pairs: &'a [(usize, usize)],
    order: &'a [usize],
    graph: Option<&'a dyn ProductRows>,
    edge_count: usize,
    scratch_len: usize,
}

#[cfg(test)]
struct Builder<'a, T: TruthAccess, R: RowStorage> {
    graph: BehaviorView<'a>,
    tableau: &'a Tableau,
    labels: &'a T,
    conditions: &'a [Recurrence],
    limits: Limits,
    acceptance_policy: AcceptancePolicy,
    behavior_edges: usize,
    scratch_len: usize,
    pairs: Vec<(usize, usize)>,
    ids: HashMap<(usize, usize), usize>,
    rows: R,
    edge_count: usize,
    steps: usize,
}

#[cfg(test)]
impl<T: TruthAccess, R: RowStorage> Builder<'_, T, R> {
    fn inspect(&mut self) -> Result<(), ProductError> {
        if crate::budget::exhausted() {
            return Err(ProductError::Limit(crate::budget::EXHAUSTED));
        }
        self.steps = self
            .steps
            .checked_add(1)
            .filter(|s| *s <= self.limits.steps)
            .ok_or(ProductError::Limit("transition steps"))?;
        Ok(())
    }

    fn acceptance_cells(&self, size: usize, edges: usize) -> Result<(), ProductError> {
        if let AcceptancePolicy::Buffers { bytes, .. } = self.acceptance_policy {
            return budget::fits_layout(
                size,
                edges,
                self.tableau.promise_count,
                self.conditions.len(),
                self.scratch_len,
                budget::ActionDomain::behavior(self.graph.vertex_count(), self.behavior_edges),
                bytes,
            );
        }
        size.checked_mul(self.tableau.promise_count)
            .and_then(|n| {
                edges
                    .checked_mul(self.conditions.len().checked_add(1)?)
                    .and_then(|e| n.checked_add(e))
            })
            .filter(|n| *n <= self.limits.acceptance_cells)
            .ok_or(ProductError::Limit("acceptance cells"))?;
        Ok(())
    }

    fn fits_state(&self, v: usize, n: usize) -> bool {
        self.tableau.nodes[n].literals.iter().all(|lit| {
            lit.atom.kind != AtomKind::State || self.labels.state(lit.atom.index, v) == lit.positive
        })
    }

    fn visit(&mut self, v: usize, n: usize) -> Result<usize, ProductError> {
        if let Some(&id) = self.ids.get(&(v, n)) {
            return Ok(id);
        }
        let id = self.pairs.len();
        let size = id
            .checked_add(1)
            .filter(|n| *n <= self.limits.vertices)
            .ok_or(ProductError::Limit("vertices"))?;
        self.acceptance_cells(size, self.edge_count)?;
        csr::grow(&mut self.pairs, 1)?;
        size.checked_mul(size_of::<((usize, usize), usize)>())
            .ok_or(ProductError::Limit("product pair index bytes"))?;
        self.ids
            .try_reserve(1)
            .map_err(|_| ProductError::Limit("product pair index allocation"))?;
        csr::poll()?;
        self.ids.insert((v, n), id);
        self.pairs.push((v, n));
        self.rows.add_vertex()?;
        Ok(id)
    }

    fn expand(&mut self, id: usize) -> Result<(), ProductError> {
        self.inspect()?;
        let (v, n) = self.pairs[id];
        for e in 0..self.graph.row_len(v) {
            self.inspect()?;
            let target = self.graph.target(v, e);
            // Current action literals constrain this outgoing edge, including
            // loop closure. A destination's action literals constrain its own
            // outgoing edges when that pair is expanded.
            if !self.tableau.nodes[n].literals.iter().all(|lit| {
                lit.atom.kind != AtomKind::Action
                    || self.labels.action(lit.atom.index, v, e) == lit.positive
            }) {
                continue;
            }
            for j in 0..self.tableau.nodes[n].successors.len() {
                self.inspect()?;
                let next = self.tableau.nodes[n].successors[j];
                if !self.fits_state(target, next) {
                    continue;
                }
                self.edge_count = self
                    .edge_count
                    .checked_add(1)
                    .filter(|n| *n <= self.limits.edges)
                    .ok_or(ProductError::Limit("edges"))?;
                self.acceptance_cells(self.pairs.len(), self.edge_count)?;
                let destination = self.visit(target, next)?;
                self.rows.push_edge(id, destination, e)?;
            }
        }
        self.rows.end_row(id)
    }

    fn finish<A>(
        self,
        roots: BTreeSet<usize>,
        acceptance: impl FnOnce(Ordered<'_>) -> Result<A, ProductError>,
    ) -> Result<Parts<A, R::Graph>, ProductError> {
        // Discovery order must never choose the witness. Renumber retained
        // pairs in the original Cartesian (behavior, tableau) order. Removing
        // unreachable ids preserves every reachable tie-break and edge order.
        // The private hash table never determines iteration or witness order.
        // Release it before allocating final order; reuse the merge scratch as
        // the existing renumber vector instead of retaining another owner.
        drop(self.ids);
        let (ordered, mut renumber) = pair_order::cartesian(&self.pairs)?;
        for (new, &old) in ordered.iter().enumerate() {
            if new.is_multiple_of(1024) {
                csr::poll()?;
            }
            renumber[old] = new;
        }
        let mut sorted_roots = BTreeSet::new();
        for old in roots {
            csr::poll()?;
            sorted_roots.insert(renumber[old]);
        }
        let mut roots = csr::reserve(sorted_roots.len())?;
        for root in sorted_roots {
            csr::poll()?;
            roots.push(root);
        }
        let graph = self.rows.finish(&ordered, &renumber, roots)?;
        let acceptance = acceptance(Ordered {
            pairs: &self.pairs,
            order: &ordered,
            graph: Some(&graph),
            edge_count: self.edge_count,
            scratch_len: self.scratch_len,
        })?;
        let mut behavior_states = csr::reserve(ordered.len())?;
        for (position, &old) in ordered.iter().enumerate() {
            if position.is_multiple_of(1024) {
                csr::poll()?;
            }
            behavior_states.push(self.pairs[old].0);
        }
        Ok(Parts {
            graph,
            acceptance,
            behavior_states,
        })
    }
}

fn validate(
    graph: BehaviorView<'_>,
    tableau: &Tableau,
    labels: &impl TruthAccess,
) -> Result<(), ProductError> {
    let bad = |s: &str| ProductError::Shape(s.into());
    let width = tableau.nodes.len();
    if tableau.roots.iter().any(|r| *r >= width) {
        return Err(bad("tableau root outside node table"));
    }
    for node in &tableau.nodes {
        if node.successors.iter().any(|n| *n >= width) {
            return Err(bad("tableau successor outside node table"));
        }
        if node.fulfilled.len() != tableau.promise_count {
            return Err(bad("tableau promise dimensions"));
        }
        for literal in &node.literals {
            if !labels.contains(literal.atom) {
                return Err(bad("atomic predicate outside label table"));
            }
        }
    }
    labels.validate_dimensions(graph)
}

#[cfg(test)]
impl Product {
    /// Constructs the synchronous product with validated labels. Callers must
    /// supply all behavior edges, including the intended stuttering edges.
    /// A dead end is not implicitly converted into a stuttering behavior.
    ///
    /// # Errors
    /// Malformed references, label dimensions or an explicit resource limit.
    pub(crate) fn new(
        graph: &Graph,
        tableau: &Tableau,
        labels: &Labels,
        limits: Limits,
    ) -> Result<Self, ProductError> {
        Self::with_recurrences(graph, tableau, labels, &[], limits)
    }

    /// Constructs the reachable product with additional infinitely-recurring
    /// boolean conditions.
    /// Conditions constrain the recurrent walk, including closing edges; they
    /// never filter a finite prefix. Each condition contributes one acceptance
    /// cell per retained edge, subject to the same explicit resource budget.
    ///
    /// # Errors
    /// Malformed inputs/condition atom references or explicit resource limits.
    pub(crate) fn with_recurrences(
        graph: &Graph,
        tableau: &Tableau,
        labels: &Labels,
        conditions: &[Recurrence],
        limits: Limits,
    ) -> Result<Self, ProductError> {
        Self::with_behavior(
            BehaviorView::Dense(graph),
            tableau,
            labels,
            conditions,
            limits,
        )
    }

    /// Constructs the same product from checked borrowed CSR adjacency.
    /// The caller supplies state/action labels and all intended stutters.
    /// This does not check a bound specification or certify graph completion.
    ///
    /// # Errors
    /// Malformed tableau/label dimensions or an explicit product resource limit.
    pub(crate) fn from_csr(
        graph: CsrTopology<'_>,
        tableau: &Tableau,
        labels: &Labels,
        limits: Limits,
    ) -> Result<Self, ProductError> {
        Self::with_csr_recurrences(graph, tableau, labels, &[], limits)
    }

    /// Constructs a borrowed-CSR product with recurring conditions.
    /// Conditions, edge positions and resource bounds have the same meaning as
    /// in [`Self::with_recurrences`]; no adjacency copy is made before building
    /// the product.
    ///
    /// # Errors
    /// Malformed tableau/labels, condition atom references or product limits.
    pub(crate) fn with_csr_recurrences(
        graph: CsrTopology<'_>,
        tableau: &Tableau,
        labels: &Labels,
        conditions: &[Recurrence],
        limits: Limits,
    ) -> Result<Self, ProductError> {
        Self::with_behavior((&graph).into(), tableau, labels, conditions, limits)
    }

    fn with_behavior(
        graph: BehaviorView<'_>,
        tableau: &Tableau,
        labels: &Labels,
        conditions: &[Recurrence],
        limits: Limits,
    ) -> Result<Self, ProductError> {
        let parts = build::<_, OwnedRows>(
            graph,
            tableau,
            labels,
            conditions,
            limits,
            (AcceptancePolicy::Legacy, 0),
            |input| {
                let graph = input.graph.ok_or_else(rowless)?;
                let states = (0..tableau.promise_count)
                    .map(|p| {
                        input
                            .order
                            .iter()
                            .map(|&old| tableau.nodes[input.pairs[old].1].fulfilled[p])
                            .collect()
                    })
                    .collect();
                let recurrent = (0..graph.vertex_count())
                    .map(|v| vec![true; graph.successors(v).len()])
                    .collect();
                let actions = conditions
                    .iter()
                    .map(|condition| {
                        input
                            .order
                            .iter()
                            .enumerate()
                            .map(|(id, &old)| {
                                let v = input.pairs[old].0;
                                graph
                                    .original_edges(id)
                                    .iter()
                                    .map(|&e| condition.evaluate(|atom| labels.truth(atom, v, e)))
                                    .collect()
                            })
                            .collect()
                    })
                    .collect();
                Ok(Acceptance {
                    recurrent,
                    states,
                    actions,
                })
            },
        )?;
        Ok(Self {
            graph: parts.graph.graph,
            acceptance: parts.acceptance,
            behavior_states: parts.behavior_states,
            original_edges: parts.graph.original_edges,
        })
    }

    /// Finds an accepting behavior and projects its edge positions back to
    /// the original graph. A repeated behavior vertex with another tableau
    /// node is retained; removing it could change temporal truth.
    /// `None` means the supplied graph/tableau has no accepting behavior,
    /// not that any bound specification was checked exhaustively.
    ///
    /// # Errors
    /// An internal acceptance or witness-construction inconsistency.
    pub(crate) fn find_lasso(&self) -> Result<Option<Lasso>, ProductError> {
        let Some(lasso) =
            find_lasso(&self.graph, &self.acceptance).map_err(ProductError::search)?
        else {
            return Ok(None);
        };
        Ok(Some(project_lasso(
            &lasso,
            &self.behavior_states,
            &self.original_edges,
        )?))
    }
}

#[cfg(test)]
fn build<A, R: RowStorage>(
    graph: BehaviorView<'_>,
    tableau: &Tableau,
    labels: &impl TruthAccess,
    conditions: &[Recurrence],
    limits: Limits,
    acceptance_budget: (AcceptancePolicy, usize),
    acceptance: impl FnOnce(Ordered<'_>) -> Result<A, ProductError>,
) -> Result<Parts<A, R::Graph>, ProductError> {
    validate(graph, tableau, labels)?;
    for atom in conditions.iter().flat_map(Recurrence::atoms) {
        if !labels.contains(atom) {
            return Err(ProductError::Shape(
                "recurrence predicate outside label table".into(),
            ));
        }
    }
    let mut builder = Builder {
        graph,
        tableau,
        labels,
        conditions,
        limits,
        acceptance_policy: acceptance_budget.0,
        behavior_edges: acceptance_budget.1,
        scratch_len: conditions
            .iter()
            .map(Recurrence::scratch_len)
            .max()
            .unwrap_or(0),
        pairs: Vec::new(),
        ids: HashMap::new(),
        rows: R::default(),
        edge_count: 0,
        steps: 0,
    };
    let mut roots = BTreeSet::new();
    for &v in graph.roots() {
        for &n in &tableau.roots {
            builder.inspect()?;
            if builder.fits_state(v, n) {
                roots.insert(builder.visit(v, n)?);
            }
        }
    }
    let mut cursor = 0;
    while cursor < builder.pairs.len() {
        builder.expand(cursor)?;
        cursor += 1;
    }
    builder.finish(roots, acceptance)
}

fn rowless() -> ProductError {
    ProductError::Shape("recurrence conditions need stored product rows".into())
}

// Each column packs one Boolean per product vertex or flattened product edge.
// Recurrent-edge truth is uniformly true for constructed products.
struct CompactAcceptance {
    states: Vec<Vec<u64>>,
    actions: Vec<Vec<u64>>,
    edge_offsets: Vec<usize>,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read only by the materialized reference product")
    )]
    behavior_actions: bool,
    /// Actual covered work already spent by the buffer-policy materializer.
    materialization_work: usize,
}

fn bit(words: &[u64], index: usize) -> bool {
    words[index / 64] & (1_u64 << (index % 64)) != 0
}

fn set_bit(words: &mut [u64], index: usize, value: bool) {
    if value {
        words[index / 64] |= 1_u64 << (index % 64);
    }
}

impl CompactAcceptance {
    // Exact exposed capacities of the covered acceptance columns and offsets.
    // Search-only Boolean scratch remains live beside these same buffers.
    fn buffer_bytes(&self) -> Option<usize> {
        let headers = self
            .states
            .capacity()
            .checked_add(self.actions.capacity())?
            .checked_mul(std::mem::size_of::<Vec<u64>>())?;
        let offsets = self
            .edge_offsets
            .capacity()
            .checked_mul(std::mem::size_of::<usize>())?;
        self.states.iter().chain(&self.actions).try_fold(
            headers.checked_add(offsets)?,
            |bytes, words| {
                bytes.checked_add(words.capacity().checked_mul(std::mem::size_of::<u64>())?)
            },
        )
    }

    fn new(
        input: &Ordered<'_>,
        tableau: &Tableau,
        labels: &impl TruthAccess,
        conditions: &[Recurrence],
    ) -> Result<Self, ProductError> {
        let states = (0..tableau.promise_count)
            .map(|p| {
                let mut words = vec![0; input.order.len().div_ceil(64)];
                for (v, &old) in input.order.iter().enumerate() {
                    set_bit(
                        &mut words,
                        v,
                        tableau.nodes[input.pairs[old].1].fulfilled[p],
                    );
                }
                words
            })
            .collect();
        let mut edge_offsets = Vec::new();
        let mut edges = 0;
        let graph = if conditions.is_empty() {
            None
        } else {
            Some(input.graph.ok_or_else(rowless)?)
        };
        if let Some(graph) = graph {
            edge_offsets.reserve_exact(graph.vertex_count() + 1);
            edge_offsets.push(0);
            for v in 0..graph.vertex_count() {
                let row = graph.successors(v);
                // The shared builder already checked every retained-edge addition.
                edges += row.len();
                edge_offsets.push(edges);
            }
        }
        let mut scratch = Vec::new();
        let actions = conditions
            .iter()
            .map(|condition| {
                let mut words = vec![0; edges.div_ceil(64)];
                for (id, &old) in input.order.iter().enumerate() {
                    let v = input.pairs[old].0;
                    let rows = graph.map_or(&[][..], |graph| graph.original_edges(id));
                    for (position, &e) in rows.iter().enumerate() {
                        let truth = condition
                            .evaluate_with_scratch(|atom| labels.truth(atom, v, e), &mut scratch);
                        set_bit(&mut words, edge_offsets[id] + position, truth);
                    }
                }
                words
            })
            .collect();
        Ok(Self {
            states,
            actions,
            edge_offsets,
            behavior_actions: false,
            materialization_work: 0,
        })
    }
}

/// Materialized product with flat acceptance bits, retained as the reference
/// for [`ImplicitProduct`] and the acceptance-budget controls. All legacy construction
/// guards run before materializing acceptance, just as for the owned reference.
#[cfg(test)]
pub(crate) struct CompactProduct {
    parts: Parts<CompactAcceptance, CsrGraph>,
    limits: Limits,
    policy: AcceptancePolicy,
    /// Setup work is cumulative; alternative scratch buffers live sequentially.
    solution_setup_work: std::cell::Cell<usize>,
}

#[cfg(test)]
impl CompactProduct {
    #[cfg(test)]
    fn with_recurrences(
        graph: &Graph,
        tableau: &Tableau,
        labels: &Labels,
        conditions: &[Recurrence],
        limits: Limits,
    ) -> Result<Self, ProductError> {
        Self::with_policy(
            graph,
            tableau,
            labels,
            conditions,
            limits,
            AcceptancePolicy::Legacy,
        )
    }

    #[cfg(test)]
    fn with_policy(
        graph: &Graph,
        tableau: &Tableau,
        labels: &Labels,
        conditions: &[Recurrence],
        limits: Limits,
        policy: AcceptancePolicy,
    ) -> Result<Self, ProductError> {
        // Production reuses the edge count already checked by atom admission.
        let edges = graph.edges.iter().map(Vec::len).sum();
        Self::with_policy_and_edges(graph, tableau, labels, conditions, limits, policy, edges)
    }

    pub(crate) fn with_policy_and_edges<'a>(
        graph: impl Into<BehaviorView<'a>>,
        tableau: &Tableau,
        labels: &impl TruthAccess,
        conditions: &[Recurrence],
        limits: Limits,
        policy: AcceptancePolicy,
        behavior_edges: usize,
    ) -> Result<Self, ProductError> {
        let graph = graph.into();
        let parts = build::<_, FlatRows>(
            graph,
            tableau,
            labels,
            conditions,
            limits,
            (policy, behavior_edges),
            |input| match policy {
                AcceptancePolicy::Legacy => {
                    CompactAcceptance::new(&input, tableau, labels, conditions)
                }
                AcceptancePolicy::Buffers { bytes, work } => budget::materialize(
                    &input,
                    tableau,
                    labels,
                    conditions,
                    bytes,
                    work,
                    (graph, behavior_edges),
                ),
            },
        )?;
        Ok(Self {
            parts,
            limits,
            policy,
            solution_setup_work: std::cell::Cell::new(0),
        })
    }

    pub(crate) fn find_lasso(&self) -> Result<Option<Lasso>, ProductError> {
        let lasso = find_lasso_view(&self.parts.graph, self).map_err(ProductError::search)?;
        lasso
            .map(|lasso| project_lasso_rows(&lasso, &self.parts.behavior_states, &self.parts.graph))
            .transpose()
    }

    pub(crate) fn find_lasso_with_conditions(
        &self,
        labels: &impl TruthAccess,
        conditions: &CycleConditions,
    ) -> Result<Option<Lasso>, ProductError> {
        if conditions.always.is_empty() && conditions.recurring.is_empty() {
            return self.find_lasso();
        }
        let scratch = solution_scratch(
            &self.parts.acceptance,
            labels,
            conditions,
            self.policy,
            || {
                if self.policy != AcceptancePolicy::Legacy {
                    return Ok(());
                }
                let edges = (0..self.parts.graph.vertex_count())
                    .try_fold(0_usize, |n, v| {
                        n.checked_add(self.parts.graph.successors(v).len())
                    })
                    .ok_or(ProductError::Limit("acceptance cells"))?;
                self.state_count()
                    .checked_mul(self.parts.graph.vertex_count())
                    .and_then(|states| {
                        self.action_count()
                            .checked_add(conditions.recurring.len())
                            .and_then(|actions| actions.checked_add(1))
                            .and_then(|actions| actions.checked_mul(edges))
                            .and_then(|actions| states.checked_add(actions))
                    })
                    .filter(|cells| *cells <= self.limits.acceptance_cells)
                    .ok_or(ProductError::Limit("acceptance cells"))?;
                Ok(())
            },
            &self.solution_setup_work,
        )?;
        let conditioned = Conditioned {
            product: self,
            labels,
            conditions,
            scratch: std::cell::RefCell::new(scratch),
        };
        let lasso =
            find_lasso_view(&self.parts.graph, &conditioned).map_err(ProductError::search)?;
        lasso
            .map(|lasso| project_lasso_rows(&lasso, &self.parts.behavior_states, &self.parts.graph))
            .transpose()
    }
}

#[cfg(test)]
impl AcceptanceAccess for CompactProduct {
    fn state_count(&self) -> usize {
        self.parts.acceptance.states.len()
    }

    fn action_count(&self) -> usize {
        self.parts.acceptance.actions.len()
    }

    fn recurrent(&self, _: usize, _: usize) -> bool {
        true
    }

    fn state(&self, condition: usize, vertex: usize) -> bool {
        bit(&self.parts.acceptance.states[condition], vertex)
    }

    fn action(&self, condition: usize, source: usize, edge: usize) -> bool {
        let labels = &self.parts.acceptance;
        let (source, edge) = if labels.behavior_actions {
            (
                self.parts.behavior_states[source],
                self.parts.graph.original_edges(source)[edge],
            )
        } else {
            (source, edge)
        };
        bit(
            &labels.actions[condition],
            labels.edge_offsets[source] + edge,
        )
    }
}

/// Checks one alternative's cycle predicates and secures its Boolean scratch
/// under the product's acceptance policy. `legacy` applies the representation's
/// logical-cell admission after the predicate references are checked.
fn solution_scratch(
    acceptance: &CompactAcceptance,
    labels: &impl TruthAccess,
    conditions: &CycleConditions,
    policy: AcceptancePolicy,
    legacy: impl FnOnce() -> Result<(), ProductError>,
    setup_work: &std::cell::Cell<usize>,
) -> Result<Vec<bool>, ProductError> {
    for atom in conditions
        .always
        .iter()
        .chain(&conditions.recurring)
        .flat_map(Recurrence::atoms)
    {
        if !labels.contains(atom) {
            return Err(ProductError::Shape(
                "cycle predicate outside label table".into(),
            ));
        }
    }
    let scratch_len = conditions
        .always
        .iter()
        .chain(&conditions.recurring)
        .map(Recurrence::scratch_len)
        .max()
        .unwrap_or(0);
    let check_bytes = |scratch: usize| -> Result<(), ProductError> {
        if let AcceptancePolicy::Buffers { bytes, .. } = policy {
            acceptance
                .buffer_bytes()
                .and_then(|held| held.checked_add(scratch))
                .filter(|needed| *needed <= bytes)
                .ok_or(ProductError::Limit("acceptance buffer bytes"))?;
        }
        Ok(())
    };
    legacy()?;
    check_bytes(scratch_len)?;
    let next_setup_work = if let AcceptancePolicy::Buffers { work, .. } = policy {
        // Each condition's scratch metadata was inspected above. The
        // scratch entries are initialized below and coexist with the
        // base acceptance buffers, so both setup costs share its budget.
        // Boolean evaluation during lasso search is not materialization.
        let setup = setup_work
            .get()
            .checked_add(conditions.always.len())
            .and_then(|used| used.checked_add(conditions.recurring.len()))
            .and_then(|used| used.checked_add(scratch_len))
            .ok_or(ProductError::Limit("acceptance materialization work"))?;
        acceptance
            .materialization_work
            .checked_add(setup)
            .filter(|used| *used <= work)
            .ok_or(ProductError::Limit("acceptance materialization work"))?;
        Some(setup)
    } else {
        None
    };
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(scratch_len)
        .map_err(|_| ProductError::Limit("solution scratch allocation"))?;
    check_bytes(scratch.capacity())?;
    scratch.resize(scratch_len, false);
    if let Some(work) = next_setup_work {
        setup_work.set(work);
    }
    Ok(scratch)
}

/// The behavior state and original edge of a product edge label.
trait BehaviorEdges {
    fn behavior_edge(&self, source: usize, edge: usize) -> (usize, usize);
}

#[cfg(test)]
impl BehaviorEdges for CompactProduct {
    fn behavior_edge(&self, source: usize, edge: usize) -> (usize, usize) {
        (
            self.parts.behavior_states[source],
            self.parts.graph.original_edges(source)[edge],
        )
    }
}

// One possible-error model borrows its group's shared temporal product. FG
// conditions restrict only the recurring walk; GF conditions add visit duties.
struct Conditioned<'a, P, T: TruthAccess> {
    product: &'a P,
    labels: &'a T,
    conditions: &'a CycleConditions,
    scratch: std::cell::RefCell<Vec<bool>>,
}

impl<P: BehaviorEdges, T: TruthAccess> Conditioned<'_, P, T> {
    fn truth(&self, condition: &Recurrence, source: usize, edge: usize) -> bool {
        let (state, edge) = self.product.behavior_edge(source, edge);
        condition.evaluate_with_scratch(
            |atom| self.labels.truth(atom, state, edge),
            &mut self.scratch.borrow_mut(),
        )
    }
}

impl<P: AcceptanceAccess + BehaviorEdges, T: TruthAccess> AcceptanceAccess
    for Conditioned<'_, P, T>
{
    fn state_count(&self) -> usize {
        self.product.state_count()
    }

    fn action_count(&self) -> usize {
        self.product.action_count() + self.conditions.recurring.len()
    }

    fn recurrent(&self, source: usize, edge: usize) -> bool {
        self.product.recurrent(source, edge)
            && self
                .conditions
                .always
                .iter()
                .all(|condition| self.truth(condition, source, edge))
    }

    fn state(&self, condition: usize, vertex: usize) -> bool {
        self.product.state(condition, vertex)
    }

    fn action(&self, condition: usize, source: usize, edge: usize) -> bool {
        let ordinary = self.product.action_count();
        if condition < ordinary {
            self.product.action(condition, source, edge)
        } else {
            self.truth(
                &self.conditions.recurring[condition - ordinary],
                source,
                edge,
            )
        }
    }
}

#[cfg(test)]
fn project_lasso(
    lasso: &Lasso,
    behavior_states: &[usize],
    original_edges: &[Vec<usize>],
) -> Result<Lasso, ProductError> {
    project_lasso_with(lasso, behavior_states, |source, edge| {
        original_edges[source][edge]
    })
}

#[cfg(test)]
fn project_lasso_rows(
    lasso: &Lasso,
    behavior_states: &[usize],
    graph: &impl ProductRows,
) -> Result<Lasso, ProductError> {
    project_lasso_with(lasso, behavior_states, |source, edge| {
        graph.original_edges(source)[edge]
    })
}

#[cfg(test)]
fn project_lasso_with(
    lasso: &Lasso,
    behavior_states: &[usize],
    original: impl Fn(usize, usize) -> usize,
) -> Result<Lasso, ProductError> {
    let project = |steps: &[Step]| -> Result<Vec<Step>, ProductError> {
        let mut projected = csr::reserve(steps.len())?;
        for (position, step) in steps.iter().enumerate() {
            if position.is_multiple_of(1024) {
                csr::poll()?;
            }
            projected.push(Step {
                from: behavior_states[step.from],
                edge: original(step.from, step.edge),
                to: behavior_states[step.to],
            });
        }
        Ok(projected)
    };
    Ok(Lasso {
        root: behavior_states[lasso.root],
        entry: behavior_states[lasso.entry],
        prefix: project(&lasso.prefix)?,
        cycle: project(&lasso.cycle)?,
    })
}

#[cfg(test)]
mod compact_tests;
#[cfg(test)]
mod semantics_tests;
