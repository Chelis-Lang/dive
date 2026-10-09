//! Bounded products of a supplied behavior graph and a temporal tableau.
//!
//! State labels describe vertices; action labels describe each original edge.
//! A tableau node constrains the edge leaving its word position, including
//! loop closure. These APIs do not enumerate states or evaluate predicates.

use crate::Graph;
use crate::formula::{AtomKind, Recurrence, Tableau};
use crate::search::{Acceptance, Lasso, SearchError, Step, find_lasso};
use std::collections::{BTreeMap, BTreeSet};

/// Truth values of all supplied atomic predicates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Labels {
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

/// An explicit product, in `(behavior vertex, tableau node)` order.
/// Input graph edge positions are retained for witness projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    graph: Graph,
    acceptance: Acceptance,
    behavior_states: Vec<usize>,
    original_edges: Vec<Vec<usize>>,
}

struct Builder<'a> {
    graph: &'a Graph,
    tableau: &'a Tableau,
    labels: &'a Labels,
    conditions: &'a [Recurrence],
    limits: Limits,
    pairs: Vec<(usize, usize)>,
    ids: BTreeMap<(usize, usize), usize>,
    edges: Vec<Vec<usize>>,
    original_edges: Vec<Vec<usize>>,
    edge_count: usize,
    steps: usize,
}

impl Builder<'_> {
    fn inspect(&mut self) -> Result<(), ProductError> {
        self.steps = self
            .steps
            .checked_add(1)
            .filter(|s| *s <= self.limits.steps)
            .ok_or(ProductError::Limit("transition steps"))?;
        Ok(())
    }

    fn acceptance_cells(&self, size: usize, edges: usize) -> Result<(), ProductError> {
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
            lit.atom.kind != AtomKind::State
                || self.labels.states[lit.atom.index][v] == lit.positive
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
        self.ids.insert((v, n), id);
        self.pairs.push((v, n));
        self.edges.push(Vec::new());
        self.original_edges.push(Vec::new());
        Ok(id)
    }

    fn expand(&mut self, id: usize) -> Result<(), ProductError> {
        self.inspect()?;
        let (v, n) = self.pairs[id];
        for e in 0..self.graph.edges[v].len() {
            self.inspect()?;
            let target = self.graph.edges[v][e];
            // Current action literals constrain this outgoing edge, including
            // loop closure. A destination's action literals constrain its own
            // outgoing edges when that pair is expanded.
            if !self.tableau.nodes[n].literals.iter().all(|lit| {
                lit.atom.kind != AtomKind::Action
                    || self.labels.actions[lit.atom.index][v][e] == lit.positive
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
                self.edges[id].push(destination);
                self.original_edges[id].push(e);
            }
        }
        Ok(())
    }

    fn finish(self, roots: BTreeSet<usize>) -> Result<Product, ProductError> {
        // Discovery order must never choose the witness. Renumber retained
        // pairs in the original Cartesian (behavior, tableau) order. Removing
        // unreachable ids preserves every reachable tie-break and edge order.
        let ordered: Vec<usize> = self.ids.into_values().collect();
        let mut renumber = vec![0; self.pairs.len()];
        for (new, &old) in ordered.iter().enumerate() {
            renumber[old] = new;
        }
        let mut edges = self.edges;
        let mut original = self.original_edges;
        let edges: Vec<Vec<usize>> = ordered
            .iter()
            .map(|&old| {
                let mut row = std::mem::take(&mut edges[old]);
                for target in &mut row {
                    *target = renumber[*target];
                }
                row
            })
            .collect();
        let original_edges: Vec<Vec<usize>> = ordered
            .iter()
            .map(|&old| std::mem::take(&mut original[old]))
            .collect();
        let states = (0..self.tableau.promise_count)
            .map(|p| {
                ordered
                    .iter()
                    .map(|&old| self.tableau.nodes[self.pairs[old].1].fulfilled[p])
                    .collect()
            })
            .collect();
        let recurrent = edges.iter().map(|row| vec![true; row.len()]).collect();
        let actions = self
            .conditions
            .iter()
            .map(|condition| {
                ordered
                    .iter()
                    .enumerate()
                    .map(|(id, &old)| {
                        let v = self.pairs[old].0;
                        original_edges[id]
                            .iter()
                            .map(|&e| {
                                condition.evaluate(|atom| match atom.kind {
                                    AtomKind::State => self.labels.states[atom.index][v],
                                    AtomKind::Action => self.labels.actions[atom.index][v][e],
                                })
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
        let roots = roots
            .into_iter()
            .map(|old| renumber[old])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let graph = Graph::new(edges, roots).map_err(|e| ProductError::Shape(e.to_string()))?;
        Ok(Product {
            graph,
            acceptance: Acceptance {
                recurrent,
                states,
                actions,
            },
            behavior_states: ordered.iter().map(|&old| self.pairs[old].0).collect(),
            original_edges,
        })
    }
}

fn validate(graph: &Graph, tableau: &Tableau, labels: &Labels) -> Result<(), ProductError> {
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
            let count = match literal.atom.kind {
                AtomKind::State => labels.states.len(),
                AtomKind::Action => labels.actions.len(),
            };
            if literal.atom.index >= count {
                return Err(bad("atomic predicate outside label table"));
            }
        }
    }
    if labels
        .states
        .iter()
        .any(|row| row.len() != graph.edges.len())
    {
        return Err(bad("state predicate dimensions"));
    }
    if labels.actions.iter().any(|rows| {
        rows.len() != graph.edges.len()
            || rows
                .iter()
                .zip(&graph.edges)
                .any(|(a, b)| a.len() != b.len())
    }) {
        return Err(bad("action predicate adjacency dimensions"));
    }
    Ok(())
}

impl Product {
    /// Constructs the synchronous product with validated labels. Callers must
    /// supply all behavior edges, including the intended stuttering edges.
    /// A dead end is not implicitly converted into a stuttering behavior.
    ///
    /// # Errors
    /// Malformed references, label dimensions or an explicit resource limit.
    pub fn new(
        graph: &Graph,
        tableau: &Tableau,
        labels: &Labels,
        limits: Limits,
    ) -> Result<Self, ProductError> {
        Self::with_recurrences(graph, tableau, labels, &[], limits)
    }

    /// Constructs the reachable product with additional infinitely-recurring
    /// boolean conditions extracted by [`crate::formula::Formula::factor_recurrences`].
    /// Conditions constrain the recurrent walk, including closing edges; they
    /// never filter a finite prefix. Each condition contributes one acceptance
    /// cell per retained edge, subject to the same explicit resource budget.
    ///
    /// # Errors
    /// Malformed inputs/condition atom references or explicit resource limits.
    pub fn with_recurrences(
        graph: &Graph,
        tableau: &Tableau,
        labels: &Labels,
        conditions: &[Recurrence],
        limits: Limits,
    ) -> Result<Self, ProductError> {
        validate(graph, tableau, labels)?;
        for atom in conditions.iter().flat_map(Recurrence::atoms) {
            let count = match atom.kind {
                AtomKind::State => labels.states.len(),
                AtomKind::Action => labels.actions.len(),
            };
            if atom.index >= count {
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
            pairs: Vec::new(),
            ids: BTreeMap::new(),
            edges: Vec::new(),
            original_edges: Vec::new(),
            edge_count: 0,
            steps: 0,
        };
        let mut roots = BTreeSet::new();
        for &v in &graph.roots {
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
        builder.finish(roots)
    }

    /// Finds an accepting behavior and projects its edge positions back to
    /// the original graph. A repeated behavior vertex with another tableau
    /// node is retained; removing it could change temporal truth.
    /// `None` means the supplied graph/tableau has no accepting behavior,
    /// not that any bound specification was checked exhaustively.
    ///
    /// # Errors
    /// An internal acceptance or witness-construction inconsistency.
    pub fn find_lasso(&self) -> Result<Option<Lasso>, ProductError> {
        let Some(lasso) =
            find_lasso(&self.graph, &self.acceptance).map_err(ProductError::Search)?
        else {
            return Ok(None);
        };
        let project = |steps: &[Step]| {
            steps
                .iter()
                .map(|s| Step {
                    from: self.behavior_states[s.from],
                    edge: self.original_edges[s.from][s.edge],
                    to: self.behavior_states[s.to],
                })
                .collect()
        };
        Ok(Some(Lasso {
            root: self.behavior_states[lasso.root],
            entry: self.behavior_states[lasso.entry],
            prefix: project(&lasso.prefix),
            cycle: project(&lasso.cycle),
        }))
    }
}
