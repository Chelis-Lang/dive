//! Deterministic reachable-component decomposition (ADR 0018 §5).

/// A malformed graph reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphError {
    /// The referenced vertex, outside the graph.
    pub vertex: usize,
    /// The source vertex, or `None` for an initial root.
    pub source: Option<usize>,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "vertex {} referenced from {:?} is outside the graph",
            self.vertex, self.source
        )
    }
}

impl std::error::Error for GraphError {}

/// A directed graph with dense vertex ids and explicit initial roots.
/// Edge and root order are retained. Component members and the component
/// list are returned in vertex order, independently of DFS visitation order.
///
/// ```
/// use dive_liveness::Graph;
/// let graph = Graph::new(vec![vec![1], vec![2], vec![1]], vec![0])?;
/// assert_eq!(graph.components(), vec![vec![0], vec![1, 2]]);
/// assert_eq!(graph.cyclic_components(), vec![vec![1, 2]]);
/// # Ok::<(), dive_liveness::GraphError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    pub(crate) edges: Vec<Vec<usize>>,
    pub(crate) roots: Vec<usize>,
}

impl Graph {
    /// Checks all references before accepting the graph.
    ///
    /// # Errors
    /// Any edge or root whose id is not in `0..edges.len()`.
    pub fn new(edges: Vec<Vec<usize>>, roots: Vec<usize>) -> Result<Self, GraphError> {
        for &vertex in &roots {
            if vertex >= edges.len() {
                return Err(GraphError {
                    vertex,
                    source: None,
                });
            }
        }
        for (source, targets) in edges.iter().enumerate() {
            for &vertex in targets {
                if vertex >= edges.len() {
                    return Err(GraphError {
                        vertex,
                        source: Some(source),
                    });
                }
            }
        }
        Ok(Self { edges, roots })
    }

    /// Strongly connected components reachable from the initial roots.
    /// Uses iterative Tarjan traversal, so graph depth does not consume the
    /// caller's stack. This is decomposition only: a cyclic component is
    /// not a liveness violation until its acceptance conditions are checked.
    #[must_use]
    pub fn components(&self) -> Vec<Vec<usize>> {
        let n = self.edges.len();
        let mut index = vec![None; n];
        let mut low = vec![0; n];
        let mut active = vec![false; n];
        let mut stack = Vec::new();
        let mut dfs: Vec<(usize, usize)> = Vec::new();
        let mut next_index = 0;
        let mut out = Vec::new();
        for &root in &self.roots {
            if index[root].is_some() {
                continue;
            }
            index[root] = Some(next_index);
            low[root] = next_index;
            next_index += 1;
            active[root] = true;
            stack.push(root);
            dfs.push((root, 0));
            while let Some((v, edge)) = dfs.last_mut() {
                let v = *v;
                if let Some(&target) = self.edges[v].get(*edge) {
                    *edge += 1;
                    if let Some(i) = index[target] {
                        if active[target] {
                            low[v] = low[v].min(i);
                        }
                    } else {
                        index[target] = Some(next_index);
                        low[target] = next_index;
                        next_index += 1;
                        active[target] = true;
                        stack.push(target);
                        dfs.push((target, 0));
                    }
                    continue;
                }
                dfs.pop();
                if index[v] == Some(low[v]) {
                    let mut component = Vec::new();
                    while let Some(member) = stack.pop() {
                        active[member] = false;
                        component.push(member);
                        if member == v {
                            break;
                        }
                    }
                    component.sort_unstable();
                    out.push(component);
                }
                if let Some(&(parent, _)) = dfs.last() {
                    low[parent] = low[parent].min(low[v]);
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Reachable components containing a cycle, including singleton
    /// self-loops. Fairness and temporal acceptance are not tested here.
    #[must_use]
    pub fn cyclic_components(&self) -> Vec<Vec<usize>> {
        self.components()
            .into_iter()
            .filter(|c| c.len() > 1 || self.edges[c[0]].contains(&c[0]))
            .collect()
    }

    /// Restricts recurrent edges while preserving reachability through an
    /// unrestricted finite prefix. Every vertex reachable in the original
    /// graph becomes a root of the restricted graph. Use this for
    /// eventually-always conditions: they constrain the cycle, not how the
    /// behavior first reaches it. The closure is called in vertex/edge order.
    ///
    /// ```
    /// use dive_liveness::Graph;
    /// let graph = Graph::new(vec![vec![1], vec![1]], vec![0])?;
    /// let recurrent = graph.filter_for_cycles(|from, _| from != 0);
    /// assert_eq!(recurrent.cyclic_components(), vec![vec![1]]);
    /// # Ok::<(), dive_liveness::GraphError>(())
    /// ```
    #[must_use]
    pub fn filter_for_cycles(&self, mut keep: impl FnMut(usize, usize) -> bool) -> Self {
        let mut reached = vec![false; self.edges.len()];
        let mut todo = self.roots.clone();
        while let Some(vertex) = todo.pop() {
            if reached[vertex] {
                continue;
            }
            reached[vertex] = true;
            todo.extend(self.edges[vertex].iter().copied());
        }
        let roots = reached
            .into_iter()
            .enumerate()
            .filter_map(|(v, yes)| yes.then_some(v))
            .collect();
        let edges = self
            .edges
            .iter()
            .enumerate()
            .map(|(from, edges)| edges.iter().copied().filter(|to| keep(from, *to)).collect())
            .collect();
        Self { edges, roots }
    }
}
