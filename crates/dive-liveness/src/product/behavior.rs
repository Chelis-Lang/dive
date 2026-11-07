//! Borrowed input topology only; product CSR and search keep their own interfaces.

use super::CsrTopology;
use crate::{Graph, GraphError};
use dive_runtime::capture::BehaviorGraph;

#[derive(Clone, Copy, Debug)]
pub(crate) enum BehaviorView<'a> {
    Dense(&'a Graph),
    Captured(&'a BehaviorGraph),
    Csr(&'a CsrTopology<'a>),
}

impl<'a> From<&'a CsrTopology<'a>> for BehaviorView<'a> {
    fn from(graph: &'a CsrTopology<'a>) -> Self {
        Self::Csr(graph)
    }
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
            Self::Csr(graph) => graph.vertex_count(),
        }
    }

    pub(crate) fn roots(self) -> &'a [usize] {
        match self {
            Self::Dense(graph) => &graph.roots,
            Self::Captured(graph) => &graph.roots,
            Self::Csr(graph) => graph.roots(),
        }
    }

    pub(crate) fn row_len(self, source: usize) -> usize {
        match self {
            Self::Dense(graph) => graph.edges[source].len(),
            Self::Captured(graph) => graph.edges[source].len(),
            Self::Csr(graph) => graph.row_len(source),
        }
    }

    pub(crate) fn target(self, source: usize, edge: usize) -> usize {
        match self {
            Self::Dense(graph) => graph.edges[source][edge],
            Self::Captured(graph) => graph.edges[source][edge].to,
            Self::Csr(graph) => graph.target(source, edge),
        }
    }
}

#[cfg(test)]
mod tests;
