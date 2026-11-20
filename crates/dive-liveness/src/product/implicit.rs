//! The coordinator's reachable product, explored on the fly.
//!
//! Only product vertices are retained, in `(behavior vertex, tableau node)`
//! order. Product edges are recomputed from the behavior graph, the tableau
//! and the supplied labels whenever a search walks a row, in exactly the
//! order the materialized builder stores them: behavior edges in row order,
//! then tableau successors in construction order. Construction applies the
//! same vertex, edge, inspection and acceptance guards as that builder, so a
//! named resource result is unchanged; it only stops storing the edges.

use super::{
    AcceptancePolicy, BehaviorView, CompactAcceptance, Limits, Ordered, ProductError, TruthAccess,
    budget, csr, pair_order,
};
use crate::formula::{AtomKind, CycleConditions, Tableau};
use crate::search::{
    AcceptanceAccess, Adjacency, Lasso, Poll, SearchError, Step, edge_at, find_lasso_view,
};
use std::collections::{BTreeSet, HashSet};

/// Discovery uses a dense bitset while it costs at most this many tableau
/// nodes per behavior vertex (64 bytes), and an exact hash set beyond that.
const DENSE_NODES: usize = 512;

/// Optional product adjacency is retained only within this extra space.
const PRODUCT_ROWS_BYTES: usize = 8 << 20;

enum Seen {
    Dense { words: Vec<u64>, width: usize },
    Sparse(HashSet<(usize, usize)>),
}

impl Seen {
    fn new(vertices: usize, width: usize) -> Result<Self, ProductError> {
        if width <= DENSE_NODES
            && let Some(bits) = vertices.checked_mul(width)
        {
            let len = bits.div_ceil(64);
            let mut words = csr::reserve(len)?;
            while words.len() < len {
                csr::poll()?;
                words.resize(words.len().saturating_add(1 << 16).min(len), 0);
            }
            return Ok(Self::Dense { words, width });
        }
        Ok(Self::Sparse(HashSet::new()))
    }

    /// Records the pair and reports whether it was new.
    fn insert(&mut self, v: usize, n: usize, pairs: usize) -> Result<bool, ProductError> {
        match self {
            Self::Dense { words, width } => {
                let bit = v * *width + n;
                let (word, mask) = (bit / 64, 1_u64 << (bit % 64));
                let new = words[word] & mask == 0;
                words[word] |= mask;
                Ok(new)
            }
            Self::Sparse(set) => {
                if set.contains(&(v, n)) {
                    return Ok(false);
                }
                pairs
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(size_of::<(usize, usize)>()))
                    .ok_or(ProductError::Limit("product pair index bytes"))?;
                set.try_reserve(1)
                    .map_err(|_| ProductError::Limit("product pair index allocation"))?;
                set.insert((v, n));
                Ok(true)
            }
        }
    }
}

struct Discovery<'a, T: TruthAccess> {
    graph: BehaviorView<'a>,
    tableau: &'a Tableau,
    labels: &'a T,
    limits: Limits,
    policy: AcceptancePolicy,
    seen: Seen,
    pairs: Vec<(usize, usize)>,
    edge_count: usize,
    steps: usize,
}

impl<T: TruthAccess> Discovery<'_, T> {
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

    // The materialized builder's acceptance admission: promise columns plus
    // one logical cell per retained edge.
    fn acceptance_cells(&self, size: usize, edges: usize) -> Result<(), ProductError> {
        if let AcceptancePolicy::Buffers { bytes, .. } = self.policy {
            return budget::fits_layout(size, self.tableau.promise_count, bytes);
        }
        size.checked_mul(self.tableau.promise_count)
            .and_then(|n| n.checked_add(edges))
            .filter(|n| *n <= self.limits.acceptance_cells)
            .ok_or(ProductError::Limit("acceptance cells"))?;
        Ok(())
    }

    fn visit(&mut self, v: usize, n: usize) -> Result<(), ProductError> {
        let size = self
            .pairs
            .len()
            .checked_add(1)
            .ok_or(ProductError::Limit("vertices"))?;
        if !self.seen.insert(v, n, self.pairs.len())? {
            return Ok(());
        }
        if size > self.limits.vertices {
            return Err(ProductError::Limit("vertices"));
        }
        self.acceptance_cells(size, self.edge_count)?;
        csr::grow(&mut self.pairs, 1)?;
        self.pairs.push((v, n));
        Ok(())
    }

    fn expand(&mut self, id: usize) -> Result<(), ProductError> {
        self.inspect()?;
        let (v, n) = self.pairs[id];
        let node = &self.tableau.nodes[n];
        for e in 0..self.graph.row_len(v) {
            self.inspect()?;
            let target = self.graph.target(v, e);
            if !actions_hold(self.tableau, self.labels, n, v, e) {
                continue;
            }
            for &next in &node.successors {
                self.inspect()?;
                if !fits_state(self.tableau, self.labels, target, next) {
                    continue;
                }
                self.edge_count = self
                    .edge_count
                    .checked_add(1)
                    .filter(|n| *n <= self.limits.edges)
                    .ok_or(ProductError::Limit("edges"))?;
                self.acceptance_cells(self.pairs.len(), self.edge_count)?;
                self.visit(target, next)?;
            }
        }
        Ok(())
    }
}

fn fits_state(tableau: &Tableau, labels: &impl TruthAccess, v: usize, n: usize) -> bool {
    tableau.nodes[n].literals.iter().all(|lit| {
        lit.atom.kind != AtomKind::State || labels.state(lit.atom.index, v) == lit.positive
    })
}

// A node's action literals constrain the edge leaving its word position.
fn actions_hold(
    tableau: &Tableau,
    labels: &impl TruthAccess,
    n: usize,
    v: usize,
    e: usize,
) -> bool {
    tableau.nodes[n].literals.iter().all(|lit| {
        lit.atom.kind != AtomKind::Action || labels.action(lit.atom.index, v, e) == lit.positive
    })
}

/// A reachable product whose edges are recomputed on demand. Vertex ids,
/// roots, edge order and acceptance are those of the materialized product.
pub(crate) struct ImplicitProduct<'a, T: TruthAccess> {
    graph: BehaviorView<'a>,
    tableau: &'a Tableau,
    labels: &'a T,
    /// Behavior vertex of each product vertex.
    states: Vec<usize>,
    /// Tableau node of each product vertex; ascending within a behavior vertex.
    nodes: Vec<usize>,
    /// Product ids of behavior vertex `v` are `offsets[v]..offsets[v + 1]`.
    offsets: Vec<usize>,
    /// The discovered product is the whole behavior graph with identical ids.
    identity: bool,
    rows: Option<ProductRows>,
    roots: Vec<usize>,
    edge_count: usize,
    acceptance: CompactAcceptance,
    limits: Limits,
    policy: AcceptancePolicy,
    /// Setup work is cumulative; alternative scratch buffers live sequentially.
    solution_setup_work: std::cell::Cell<usize>,
}

struct ProductRows {
    offsets: Vec<u32>,
    edges: Vec<(u32, u32)>,
}

impl<'a, T: TruthAccess> ImplicitProduct<'a, T> {
    /// Discovers the reachable product of `graph` and `tableau`, as the
    /// materialized `CompactProduct` does.
    pub(crate) fn new(
        graph: impl Into<BehaviorView<'a>>,
        tableau: &'a Tableau,
        labels: &'a T,
        limits: Limits,
        policy: AcceptancePolicy,
    ) -> Result<Self, ProductError> {
        let graph = graph.into();
        super::validate(graph, tableau, labels)?;
        let mut discovery = Discovery {
            graph,
            tableau,
            labels,
            limits,
            policy,
            seen: Seen::new(graph.vertex_count(), tableau.nodes.len())?,
            pairs: Vec::new(),
            edge_count: 0,
            steps: 0,
        };
        let mut root_pairs = BTreeSet::new();
        for &v in graph.roots() {
            for &n in &tableau.roots {
                discovery.inspect()?;
                if fits_state(tableau, labels, v, n) {
                    discovery.visit(v, n)?;
                    root_pairs.insert((v, n));
                }
            }
        }
        let mut cursor = 0;
        while cursor < discovery.pairs.len() {
            discovery.expand(cursor)?;
            cursor += 1;
        }
        Self::finish(discovery, &root_pairs)
    }

    // Orders the discovered pairs, materializes promise columns under the
    // acceptance policy and indexes each behavior vertex's product ids.
    fn finish(
        discovery: Discovery<'a, T>,
        root_pairs: &BTreeSet<(usize, usize)>,
    ) -> Result<Self, ProductError> {
        let Discovery {
            graph,
            tableau,
            labels,
            limits,
            policy,
            seen,
            pairs,
            edge_count,
            ..
        } = discovery;
        drop(seen);
        // Discovery order never chooses the witness: ids follow the
        // Cartesian `(behavior vertex, tableau node)` order.
        let (ordered, scratch) = pair_order::cartesian(&pairs)?;
        drop(scratch);
        let input = Ordered {
            pairs: &pairs,
            order: &ordered,
        };
        let acceptance = match policy {
            AcceptancePolicy::Cells => CompactAcceptance::new(&input, tableau),
            AcceptancePolicy::Buffers { bytes, work } => {
                budget::materialize(&input, tableau, bytes, work)?
            }
        };
        let mut states = csr::reserve(ordered.len())?;
        let mut nodes = csr::reserve(ordered.len())?;
        for (position, &old) in ordered.iter().enumerate() {
            if position.is_multiple_of(1024) {
                csr::poll()?;
            }
            let (v, n) = pairs[old];
            states.push(v);
            nodes.push(n);
        }
        drop(ordered);
        drop(pairs);
        let vertices = graph.vertex_count();
        let mut offsets = csr::reserve(
            vertices
                .checked_add(1)
                .ok_or(ProductError::Limit("product adjacency bytes"))?,
        )?;
        offsets.push(0);
        let mut id = 0;
        for v in 0..vertices {
            if v.is_multiple_of(1024) {
                csr::poll()?;
            }
            while id < states.len() && states[id] == v {
                id += 1;
            }
            offsets.push(id);
        }
        if id != states.len() {
            return Err(ProductError::Shape("product vertex outside graph".into()));
        }
        let identity = states.len() == vertices
            && matches!(tableau.nodes.as_slice(), [node]
                if node.literals.is_empty() && node.successors.as_slice() == [0]);
        let mut roots = csr::reserve(root_pairs.len())?;
        let mut product = Self {
            graph,
            tableau,
            labels,
            states,
            nodes,
            offsets,
            identity,
            rows: None,
            roots: Vec::new(),
            edge_count,
            acceptance,
            limits,
            policy,
            solution_setup_work: std::cell::Cell::new(0),
        };
        for &(v, n) in root_pairs {
            csr::poll()?;
            roots.push(
                product
                    .id(v, n)
                    .ok_or_else(|| ProductError::Shape("product root was not discovered".into()))?,
            );
        }
        product.roots = roots;
        if !identity
            && std::env::var_os("DIVE_EXPERIMENT_PRODUCT_ROWS").as_deref()
                == Some(std::ffi::OsStr::new("1"))
        {
            product.rows = product.materialize_rows()?;
        }
        Ok(product)
    }

    fn materialize_rows(&self) -> Result<Option<ProductRows>, ProductError> {
        let offsets = self.states.len() + 1;
        let bytes = offsets.checked_mul(size_of::<u32>()).and_then(|bytes| {
            self.edge_count
                .checked_mul(size_of::<(u32, u32)>())
                .and_then(|edges| bytes.checked_add(edges))
        });
        if bytes.is_none_or(|bytes| bytes > PRODUCT_ROWS_BYTES) {
            return Ok(None);
        }
        let mut rows = ProductRows {
            offsets: Vec::new(),
            edges: Vec::new(),
        };
        if rows.offsets.try_reserve_exact(offsets).is_err()
            || rows.edges.try_reserve_exact(self.edge_count).is_err()
        {
            return Ok(None);
        }
        rows.offsets.push(0);
        for source in 0..self.states.len() {
            if source.is_multiple_of(1024) {
                csr::poll()?;
            }
            let mut cursor = self.cursor(source);
            while let Some((to, edge)) = self
                .advance(source, &mut cursor)
                .map_err(ProductError::search)?
            {
                if rows.edges.len().is_multiple_of(1024) {
                    csr::poll()?;
                }
                let (Ok(to), Ok(edge)) = (u32::try_from(to), u32::try_from(edge)) else {
                    return Ok(None);
                };
                if rows.edges.len() >= self.edge_count {
                    return Err(ProductError::Shape(
                        "product rows exceed discovery cardinality".into(),
                    ));
                }
                rows.edges.push((to, edge));
            }
            rows.offsets.push(
                u32::try_from(rows.edges.len())
                    .map_err(|_| ProductError::Limit("packed product edge index"))?,
            );
        }
        if rows.edges.len() != self.edge_count {
            return Err(ProductError::Shape(
                "product rows differ from discovery cardinality".into(),
            ));
        }
        Ok(Some(rows))
    }

    fn id(&self, v: usize, n: usize) -> Option<usize> {
        let start = self.offsets[v];
        let row = &self.nodes[start..self.offsets[v + 1]];
        match row {
            [] => None,
            [only] => (*only == n).then_some(start),
            _ => row.binary_search(&n).ok().map(|i| start + i),
        }
    }

    /// Product vertices and the edges a full row walk would yield.
    #[cfg(test)]
    pub(crate) fn dimensions(&self) -> (usize, usize) {
        (self.states.len(), self.edge_count)
    }

    /// Finds an accepting lasso under one alternative's cycle conditions, as
    /// the materialized `CompactProduct::find_lasso_with_conditions` does, and projects
    /// it onto behavior states and original edge positions.
    pub(crate) fn find_lasso_with_conditions(
        &self,
        labels: &impl TruthAccess,
        conditions: &CycleConditions,
    ) -> Result<Option<Lasso>, ProductError> {
        let lasso = if conditions.always.is_empty() && conditions.recurring.is_empty() {
            find_lasso_view(self, self).map_err(ProductError::search)?
        } else {
            let scratch = super::solution_scratch(
                &self.acceptance,
                labels,
                conditions,
                self.policy,
                || self.admit_cells(conditions),
                &self.solution_setup_work,
            )?;
            let conditioned = super::Conditioned {
                product: self,
                labels,
                conditions,
                scratch: std::cell::RefCell::new(scratch),
            };
            find_lasso_view(self, &conditioned).map_err(ProductError::search)?
        };
        lasso.map(|lasso| self.project(&lasso)).transpose()
    }

    // The materialized product's logical-cell admission for a solution.
    fn admit_cells(&self, conditions: &CycleConditions) -> Result<(), ProductError> {
        if self.policy != AcceptancePolicy::Cells {
            return Ok(());
        }
        self.acceptance
            .states
            .len()
            .checked_mul(self.states.len())
            .and_then(|states| {
                conditions
                    .recurring
                    .len()
                    .checked_add(1)
                    .and_then(|actions| actions.checked_mul(self.edge_count))
                    .and_then(|actions| states.checked_add(actions))
            })
            .filter(|cells| *cells <= self.limits.acceptance_cells)
            .ok_or(ProductError::Limit("acceptance cells"))?;
        Ok(())
    }

    fn project(&self, lasso: &Lasso) -> Result<Lasso, ProductError> {
        let mut poll = Poll::default();
        let project = |steps: &[Step], poll: &mut Poll| -> Result<Vec<Step>, ProductError> {
            let mut projected = csr::reserve(steps.len())?;
            for step in steps {
                let (_, edge) =
                    edge_at(self, step.from, step.edge, poll).map_err(ProductError::search)?;
                projected.push(Step {
                    from: self.states[step.from],
                    edge,
                    to: self.states[step.to],
                });
            }
            Ok(projected)
        };
        Ok(Lasso {
            root: self.states[lasso.root],
            entry: self.states[lasso.entry],
            prefix: project(&lasso.prefix, &mut poll)?,
            cycle: project(&lasso.cycle, &mut poll)?,
        })
    }
}

impl<T: TruthAccess> Adjacency for ImplicitProduct<'_, T> {
    /// `(behavior edge, tableau successor position)` of the next candidate.
    type Cursor = (usize, usize);

    fn vertex_count(&self) -> usize {
        self.states.len()
    }

    fn roots(&self) -> &[usize] {
        &self.roots
    }

    fn cursor(&self, _: usize) -> (usize, usize) {
        (0, 0)
    }

    fn advance(
        &self,
        source: usize,
        cursor: &mut (usize, usize),
    ) -> Result<Option<(usize, usize)>, SearchError> {
        if let Some(rows) = &self.rows {
            let at = rows.offsets[source] as usize + cursor.0;
            if at >= rows.offsets[source + 1] as usize {
                return Ok(None);
            }
            let (to, edge) = rows.edges[at];
            cursor.0 += 1;
            return Ok(Some((to as usize, edge as usize)));
        }
        if self.identity {
            let edge = cursor.0;
            if edge >= self.graph.row_len(source) {
                return Ok(None);
            }
            cursor.0 += 1;
            return Ok(Some((self.graph.target(source, edge), edge)));
        }
        let (v, n) = (self.states[source], self.nodes[source]);
        let successors = &self.tableau.nodes[n].successors;
        let row = self.graph.row_len(v);
        loop {
            let (e, j) = *cursor;
            if e >= row {
                return Ok(None);
            }
            if j >= successors.len()
                || (j == 0 && !actions_hold(self.tableau, self.labels, n, v, e))
            {
                *cursor = (e + 1, 0);
                continue;
            }
            *cursor = (e, j + 1);
            let target = self.graph.target(v, e);
            let next = successors[j];
            // Discovery already checked these state literals. From a reached
            // source, an admitted edge reaches every matching destination
            // pair. A missing pair therefore means the literals failed.
            if let Some(to) = self.id(target, next) {
                return Ok(Some((to, e)));
            }
        }
    }
}

// Labels are original behavior edges; every product edge may recur and only
// promises label vertices. Cycle conditions are added by `Conditioned`.
impl<T: TruthAccess> AcceptanceAccess for ImplicitProduct<'_, T> {
    fn state_count(&self) -> usize {
        self.acceptance.states.len()
    }

    fn action_count(&self) -> usize {
        0
    }

    fn recurrent(&self, _: usize, _: usize) -> bool {
        true
    }

    fn state(&self, condition: usize, vertex: usize) -> bool {
        super::bit(&self.acceptance.states[condition], vertex)
    }

    fn action(&self, _: usize, _: usize, _: usize) -> bool {
        false
    }
}

impl<T: TruthAccess> super::BehaviorEdges for ImplicitProduct<'_, T> {
    fn behavior_edge(&self, source: usize, edge: usize) -> (usize, usize) {
        (self.states[source], edge)
    }
}

#[cfg(test)]
mod tests;
