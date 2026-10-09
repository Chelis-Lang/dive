//! Bounded products of a supplied behavior graph and a temporal tableau.
//!
//! State labels describe vertices; action labels describe each original edge.
//! A tableau node constrains the edge leaving its word position, including
//! loop closure. These APIs do not enumerate states or evaluate predicates.

use crate::Graph;
use crate::formula::{AtomKind, Tableau};
use crate::search::{Acceptance, Lasso, SearchError, Step, find_lasso};
use std::collections::BTreeSet;

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
    /// Maximum behavior-vertex/tableau-node pairs, including unreachable pairs.
    pub vertices: usize,
    /// Maximum retained product edges.
    pub edges: usize,
    /// Maximum boolean cells for product promise and recurrent-edge labels.
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
    width: usize,
    original_edges: Vec<Vec<usize>>,
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
        validate(graph, tableau, labels)?;
        let width = tableau.nodes.len();
        let size = graph
            .edges
            .len()
            .checked_mul(width)
            .filter(|n| *n <= limits.vertices)
            .ok_or(ProductError::Limit("vertices"))?;
        let promise_cells = size
            .checked_mul(tableau.promise_count)
            .filter(|n| *n <= limits.acceptance_cells)
            .ok_or(ProductError::Limit("acceptance cells"))?;
        let fits_state = |v: usize, n: usize| {
            tableau.nodes[n].literals.iter().all(|lit| {
                lit.atom.kind != AtomKind::State || labels.states[lit.atom.index][v] == lit.positive
            })
        };
        let mut edges = vec![Vec::new(); size];
        let mut original_edges = vec![Vec::new(); size];
        let mut edge_count = 0_usize;
        let mut steps = 0_usize;
        let mut inspect = || {
            steps = steps
                .checked_add(1)
                .filter(|s| *s <= limits.steps)
                .ok_or(ProductError::Limit("transition steps"))?;
            Ok::<(), ProductError>(())
        };
        for (v, targets) in graph.edges.iter().enumerate() {
            for (n, node) in tableau.nodes.iter().enumerate() {
                inspect()?;
                if !fits_state(v, n) {
                    continue;
                }
                let id = v * width + n;
                for (e, &target) in targets.iter().enumerate() {
                    inspect()?;
                    // Action literals belong to the CURRENT tableau node,
                    // and depend on this particular outgoing behavior edge.
                    if !node.literals.iter().all(|lit| {
                        lit.atom.kind != AtomKind::Action
                            || labels.actions[lit.atom.index][v][e] == lit.positive
                    }) {
                        continue;
                    }
                    for &next in &node.successors {
                        inspect()?;
                        if !fits_state(target, next) {
                            continue;
                        }
                        edge_count = edge_count
                            .checked_add(1)
                            .filter(|s| *s <= limits.edges)
                            .ok_or(ProductError::Limit("edges"))?;
                        if promise_cells
                            .checked_add(edge_count)
                            .is_none_or(|n| n > limits.acceptance_cells)
                        {
                            return Err(ProductError::Limit("acceptance cells"));
                        }
                        edges[id].push(target * width + next);
                        original_edges[id].push(e);
                    }
                }
            }
        }
        let mut roots = BTreeSet::new();
        for &v in &graph.roots {
            for &n in &tableau.roots {
                inspect()?;
                if fits_state(v, n) {
                    roots.insert(v * width + n);
                }
            }
        }
        let roots = roots.into_iter().collect();
        let recurrent = edges.iter().map(|r| vec![true; r.len()]).collect();
        let states = (0..tableau.promise_count)
            .map(|p| {
                (0..size)
                    .map(|v| tableau.nodes[v % width].fulfilled[p])
                    .collect()
            })
            .collect();
        let graph = Graph::new(edges, roots).map_err(|e| ProductError::Shape(e.to_string()))?;
        Ok(Self {
            graph,
            acceptance: Acceptance {
                recurrent,
                states,
                actions: vec![],
            },
            width,
            original_edges,
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
                    from: s.from / self.width,
                    edge: self.original_edges[s.from][s.edge],
                    to: s.to / self.width,
                })
                .collect()
        };
        Ok(Some(Lasso {
            root: lasso.root / self.width,
            entry: lasso.entry / self.width,
            prefix: project(&lasso.prefix),
            cycle: project(&lasso.cycle),
        }))
    }
}
