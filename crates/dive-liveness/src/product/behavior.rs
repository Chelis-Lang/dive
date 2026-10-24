//! Borrowed input topology only; product CSR and search keep their own interfaces.

use crate::{Graph, GraphError};
use dive_check::capture::BehaviorGraph;

#[derive(Clone, Copy, Debug)]
pub(crate) enum BehaviorView<'a> {
    Dense(&'a Graph),
    Captured(&'a BehaviorGraph),
}

impl<'a> From<&'a Graph> for BehaviorView<'a> {
    fn from(graph: &'a Graph) -> Self {
        Self::Dense(graph)
    }
}

impl<'a> BehaviorView<'a> {
    // Match Graph::new exactly: all roots first, then destinations in row order.
    // Captured states/concrete rows are validated later, at their original phase.
    pub(crate) fn checked_captured(graph: &'a BehaviorGraph) -> Result<Self, GraphError> {
        for &vertex in &graph.roots {
            if vertex >= graph.edges.len() {
                return Err(GraphError {
                    vertex,
                    source: None,
                });
            }
        }
        for (source, targets) in graph.edges.iter().enumerate() {
            for edge in targets {
                if edge.to >= graph.edges.len() {
                    return Err(GraphError {
                        vertex: edge.to,
                        source: Some(source),
                    });
                }
            }
        }
        Ok(Self::Captured(graph))
    }

    pub(crate) fn vertex_count(self) -> usize {
        match self {
            Self::Dense(graph) => graph.edges.len(),
            Self::Captured(graph) => graph.edges.len(),
        }
    }

    pub(crate) fn roots(self) -> &'a [usize] {
        match self {
            Self::Dense(graph) => &graph.roots,
            Self::Captured(graph) => &graph.roots,
        }
    }

    pub(crate) fn row_len(self, source: usize) -> usize {
        match self {
            Self::Dense(graph) => graph.edges[source].len(),
            Self::Captured(graph) => graph.edges[source].len(),
        }
    }

    pub(crate) fn target(self, source: usize, edge: usize) -> usize {
        match self {
            Self::Dense(graph) => graph.edges[source][edge],
            Self::Captured(graph) => graph.edges[source][edge].to,
        }
    }
}

#[cfg(test)]
mod tests;
