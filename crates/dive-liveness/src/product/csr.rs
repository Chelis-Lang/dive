//! Private product row storage. Lookup, admission and traversal are shared;
//! only the coordinator's adjacency uses ranges over flat discovery buffers.

use super::ProductError;
#[cfg(test)]
use super::{Graph, GraphAccess};

#[cfg(test)]
pub(super) trait ProductRows: GraphAccess {
    fn original_edges(&self, source: usize) -> &[usize];
}

#[cfg(test)]
pub(super) trait RowStorage: Default {
    type Graph: ProductRows;

    fn add_vertex(&mut self) -> Result<(), ProductError>;
    fn push_edge(
        &mut self,
        source: usize,
        target: usize,
        original: usize,
    ) -> Result<(), ProductError>;
    fn end_row(&mut self, source: usize) -> Result<(), ProductError>;
    fn finish(
        self,
        order: &[usize],
        renumber: &[usize],
        roots: Vec<usize>,
    ) -> Result<Self::Graph, ProductError>;
}

// The original owned representation, retained as the finite-test reference
// for the flat rows. Its row allocations and order are unchanged.
#[cfg(test)]
#[derive(Default)]
pub(super) struct OwnedRows {
    edges: Vec<Vec<usize>>,
    original_edges: Vec<Vec<usize>>,
}

#[cfg(test)]
pub(super) struct OwnedGraph {
    pub(super) graph: Graph,
    pub(super) original_edges: Vec<Vec<usize>>,
}

#[cfg(test)]
impl GraphAccess for OwnedGraph {
    fn vertex_count(&self) -> usize {
        self.graph.edges.len()
    }
    fn roots(&self) -> &[usize] {
        &self.graph.roots
    }
    fn successors(&self, source: usize) -> &[usize] {
        &self.graph.edges[source]
    }
}

#[cfg(test)]
impl ProductRows for OwnedGraph {
    fn original_edges(&self, source: usize) -> &[usize] {
        &self.original_edges[source]
    }
}

#[cfg(test)]
impl RowStorage for OwnedRows {
    type Graph = OwnedGraph;

    fn add_vertex(&mut self) -> Result<(), ProductError> {
        self.edges.push(Vec::new());
        self.original_edges.push(Vec::new());
        Ok(())
    }

    fn push_edge(
        &mut self,
        source: usize,
        target: usize,
        original: usize,
    ) -> Result<(), ProductError> {
        self.edges[source].push(target);
        self.original_edges[source].push(original);
        Ok(())
    }

    fn end_row(&mut self, _: usize) -> Result<(), ProductError> {
        Ok(())
    }

    fn finish(
        self,
        order: &[usize],
        renumber: &[usize],
        roots: Vec<usize>,
    ) -> Result<Self::Graph, ProductError> {
        let mut edges = self.edges;
        let mut original = self.original_edges;
        let edges = order
            .iter()
            .map(|&old| {
                let mut row = std::mem::take(&mut edges[old]);
                for target in &mut row {
                    *target = renumber[*target];
                }
                row
            })
            .collect();
        let original_edges = order
            .iter()
            .map(|&old| std::mem::take(&mut original[old]))
            .collect();
        let graph = Graph::new(edges, roots).map_err(|e| ProductError::Shape(e.to_string()))?;
        Ok(OwnedGraph {
            graph,
            original_edges,
        })
    }
}

const SIZE: ProductError = ProductError::Limit("product adjacency bytes");
const ALLOCATION: ProductError = ProductError::Limit("product adjacency allocation");

pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, ProductError> {
    poll()?;
    count.checked_mul(size_of::<T>()).ok_or(SIZE)?;
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| ALLOCATION)?;
    Ok(values)
}

pub(super) fn grow<T>(values: &mut Vec<T>, additional: usize) -> Result<(), ProductError> {
    values
        .len()
        .checked_add(additional)
        .and_then(|n| n.checked_mul(size_of::<T>()))
        .ok_or(SIZE)?;
    poll()?;
    values.try_reserve(additional).map_err(|_| ALLOCATION)?;
    poll()
}

pub(super) fn poll() -> Result<(), ProductError> {
    if crate::budget::exhausted() {
        Err(ProductError::Limit(crate::budget::EXHAUSTED))
    } else {
        Ok(())
    }
}

/// Root/pair discovery can precede row expansion. `vertices` counts admitted
/// pairs; offsets are appended only after each cursor's expansion completes.
#[cfg(test)]
#[derive(Default)]
pub(super) struct FlatRows {
    vertices: usize,
    offsets: Vec<usize>,
    targets: Vec<usize>,
    original_edges: Vec<usize>,
}

#[cfg(test)]
pub(super) struct CsrGraph {
    ranges: Vec<std::ops::Range<usize>>,
    targets: Vec<usize>,
    original_edges: Vec<usize>,
    roots: Vec<usize>,
}

#[cfg(test)]
impl CsrGraph {
    fn dimensions(
        offsets: &[usize],
        targets: &[usize],
        original_edges: &[usize],
        roots: &[usize],
    ) -> Result<usize, ProductError> {
        if offsets.first() != Some(&0)
            || offsets.last() != Some(&targets.len())
            || targets.len() != original_edges.len()
            || offsets.windows(2).any(|w| w[0] > w[1])
        {
            return Err(ProductError::Shape("product adjacency offsets".into()));
        }
        let vertices = offsets.len() - 1;
        if roots.iter().any(|&v| v >= vertices) {
            return Err(ProductError::Shape("product adjacency vertex".into()));
        }
        Ok(vertices)
    }

    fn from_discovery(
        offsets: Vec<usize>,
        mut targets: Vec<usize>,
        original_edges: Vec<usize>,
        roots: Vec<usize>,
        order: &[usize],
        renumber: &[usize],
    ) -> Result<Self, ProductError> {
        let vertices = Self::dimensions(&offsets, &targets, &original_edges, &roots)?;
        if order.len() != vertices || renumber.len() != vertices {
            return Err(ProductError::Shape("product adjacency row count".into()));
        }
        if crate::budget::exhausted() {
            return Err(ProductError::Limit(crate::budget::EXHAUSTED));
        }
        let mut ranges = reserve(vertices)?;
        for (new, &old) in order.iter().enumerate() {
            if new.is_multiple_of(1024) && crate::budget::exhausted() {
                return Err(ProductError::Limit(crate::budget::EXHAUSTED));
            }
            // V references in range and this exact inverse relation prove a
            // permutation without allocating another seen table. Combined with
            // the checked offset partition, every edge belongs to one row.
            if renumber.get(old) != Some(&new) {
                return Err(ProductError::Shape("product adjacency permutation".into()));
            }
            ranges.push(offsets[old]..offsets[old + 1]);
        }
        // Only O(V) ranges overlap the discovery offsets. The two E-sized
        // buffers below retain their original allocation and physical row order.
        drop(offsets);
        for (position, target) in targets.iter_mut().enumerate() {
            if position.is_multiple_of(1024) && crate::budget::exhausted() {
                return Err(ProductError::Limit(crate::budget::EXHAUSTED));
            }
            *target = *renumber
                .get(*target)
                .filter(|&&v| v < vertices)
                .ok_or_else(|| ProductError::Shape("product adjacency vertex".into()))?;
        }
        if crate::budget::exhausted() {
            return Err(ProductError::Limit(crate::budget::EXHAUSTED));
        }
        Ok(Self {
            ranges,
            targets,
            original_edges,
            roots,
        })
    }

    // Preserve the literal malformed-offset controls through the same actual
    // discovery admission path; no arbitrary range constructor is exposed.
    #[cfg(test)]
    fn checked(
        offsets: Vec<usize>,
        targets: Vec<usize>,
        original_edges: Vec<usize>,
        roots: Vec<usize>,
    ) -> Result<Self, ProductError> {
        let vertices = Self::dimensions(&offsets, &targets, &original_edges, &roots)?;
        let mut identity = reserve(vertices)?;
        identity.extend(0..vertices);
        Self::from_discovery(
            offsets,
            targets,
            original_edges,
            roots,
            &identity,
            &identity,
        )
    }

    #[cfg(test)]
    pub(super) fn owned_graph(&self) -> Graph {
        Graph {
            edges: (0..self.vertex_count())
                .map(|v| self.successors(v).to_vec())
                .collect(),
            roots: self.roots.clone(),
        }
    }

    #[cfg(test)]
    pub(super) fn owned_original_edges(&self) -> Vec<Vec<usize>> {
        (0..self.vertex_count())
            .map(|v| self.original_edges(v).to_vec())
            .collect()
    }

    fn range(&self, source: usize) -> std::ops::Range<usize> {
        self.ranges[source].clone()
    }
}

#[cfg(test)]
impl GraphAccess for CsrGraph {
    fn vertex_count(&self) -> usize {
        self.ranges.len()
    }
    fn roots(&self) -> &[usize] {
        &self.roots
    }
    fn successors(&self, source: usize) -> &[usize] {
        &self.targets[self.range(source)]
    }
}

#[cfg(test)]
impl ProductRows for CsrGraph {
    fn original_edges(&self, source: usize) -> &[usize] {
        &self.original_edges[self.range(source)]
    }
}

#[cfg(test)]
impl RowStorage for FlatRows {
    type Graph = CsrGraph;

    fn add_vertex(&mut self) -> Result<(), ProductError> {
        self.vertices = self.vertices.checked_add(1).ok_or(SIZE)?;
        Ok(())
    }

    fn push_edge(
        &mut self,
        source: usize,
        target: usize,
        original: usize,
    ) -> Result<(), ProductError> {
        let current = self.offsets.len().saturating_sub(1);
        if source != current || source >= self.vertices || target >= self.vertices {
            return Err(ProductError::Shape("product expansion edge".into()));
        }
        // Both required capacities are secured before publishing either entry.
        grow(&mut self.targets, 1)?;
        grow(&mut self.original_edges, 1)?;
        self.targets.push(target);
        self.original_edges.push(original);
        Ok(())
    }

    fn end_row(&mut self, source: usize) -> Result<(), ProductError> {
        let next = source.checked_add(1).ok_or(SIZE)?;
        if source >= self.vertices
            || (!self.offsets.is_empty() && self.offsets.len() != next)
            || (self.offsets.is_empty() && source != 0)
        {
            return Err(ProductError::Shape("product expansion row order".into()));
        }
        let additional = if self.offsets.is_empty() { 2 } else { 1 };
        grow(&mut self.offsets, additional)?;
        if self.offsets.is_empty() {
            self.offsets.push(0);
        }
        self.offsets.push(self.targets.len());
        Ok(())
    }

    fn finish(
        self,
        order: &[usize],
        renumber: &[usize],
        roots: Vec<usize>,
    ) -> Result<CsrGraph, ProductError> {
        let count = self.vertices.checked_add(1).ok_or(SIZE)?;
        let mut old_offsets = self.offsets;
        if self.vertices == 0 {
            old_offsets = reserve(1)?;
            old_offsets.push(0);
        }
        if old_offsets.len() != count
            || order.len() != self.vertices
            || renumber.len() != self.vertices
        {
            return Err(ProductError::Shape("product adjacency row count".into()));
        }
        CsrGraph::from_discovery(
            old_offsets,
            self.targets,
            self.original_edges,
            roots,
            order,
            renumber,
        )
    }
}

#[cfg(test)]
mod tests;
