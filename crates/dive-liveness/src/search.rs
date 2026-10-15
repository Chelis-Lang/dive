//! Accepting cycles on an already constructed, labelled product graph.
//!
//! This is the graph-search part of ADR 0018 §5. It neither constructs a
//! temporal tableau nor evaluates specification predicates, and cannot by
//! itself establish a model-checking verdict. Callers supply the product
//! graph, including stutter edges, and its acceptance labels.

use crate::Graph;

/// Acceptance labels for one order-of-solution / possible-error-model pair.
/// Every state condition, action condition and tableau promise must occur
/// infinitely often. Promises use the same representation as state conditions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Acceptance {
    /// Which edges may occur in the recurrent cycle (EA conditions).
    /// The finite prefix is unrestricted. Rows correspond to graph vertices;
    /// entries correspond to their outgoing edges, duplicates included.
    pub recurrent: Vec<Vec<bool>>,
    /// Required node visits: one vertex-labelled row per AE-state condition
    /// or tableau promise. All rows must be discharged on the cycle.
    pub states: Vec<Vec<bool>>,
    /// Required edge visits: one adjacency-shaped table per AE-action
    /// condition. All tables must be discharged on the cycle.
    pub actions: Vec<Vec<Vec<bool>>>,
}

/// A malformed label table or an internal search inconsistency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchError {
    /// Label dimensions do not match the graph.
    Shape(String),
    /// An accepting component failed to provide its promised witness.
    /// This is a bug, never an absence-of-violation result.
    Invariant(&'static str),
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape(s) => write!(f, "invalid acceptance labels: {s}"),
            Self::Invariant(s) => write!(f, "lasso search invariant: {s}"),
        }
    }
}

impl std::error::Error for SearchError {}

/// One edge of a witness. Its position distinguishes parallel edges with
/// different action labels, even when they have the same destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// Source vertex.
    pub from: usize,
    /// Position in the source vertex's original adjacency list.
    pub edge: usize,
    /// Destination vertex.
    pub to: usize,
}

/// A finite prefix and a nonempty, accepting closed walk. Repeating `cycle`
/// forever satisfies the supplied recurrence and visit conditions.
/// Projection to concrete states and property attribution are separate work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lasso {
    /// An initial graph root.
    pub root: usize,
    /// The selected component's entry vertex.
    pub entry: usize,
    /// A shortest unrestricted path from `root` to `entry`.
    pub prefix: Vec<Step>,
    /// A deterministic short closed walk from `entry` back to itself.
    /// It is not guaranteed to be the globally shortest covering walk.
    pub cycle: Vec<Step>,
}

impl Acceptance {
    fn validate(&self, graph: &Graph) -> Result<(), SearchError> {
        let adjacency = |rows: &[Vec<bool>], what: &str| {
            if rows.len() != graph.edges.len()
                || rows
                    .iter()
                    .zip(&graph.edges)
                    .any(|(a, b)| a.len() != b.len())
            {
                Err(SearchError::Shape(format!("{what} adjacency dimensions")))
            } else {
                Ok(())
            }
        };
        adjacency(&self.recurrent, "recurrent")?;
        for (i, row) in self.states.iter().enumerate() {
            if row.len() != graph.edges.len() {
                return Err(SearchError::Shape(format!("state condition {i} length")));
            }
        }
        for (i, rows) in self.actions.iter().enumerate() {
            adjacency(rows, &format!("action condition {i}"))?;
        }
        Ok(())
    }
}

/// Internal truth access for owned labels and the coordinator's compact view.
/// Inputs have already passed their representation's shape validation.
pub(crate) trait AcceptanceAccess {
    fn state_count(&self) -> usize;
    fn action_count(&self) -> usize;
    fn recurrent(&self, source: usize, edge: usize) -> bool;
    fn state(&self, condition: usize, vertex: usize) -> bool;
    fn action(&self, condition: usize, source: usize, edge: usize) -> bool;

    fn accepts(&self, graph: &Graph, component: &[usize], members: &[usize], stamp: usize) -> bool {
        (0..self.state_count()).all(|condition| component.iter().any(|&v| self.state(condition, v)))
            && (0..self.action_count()).all(|condition| {
                component.iter().any(|&v| {
                    graph.edges[v].iter().enumerate().any(|(e, t)| {
                        members[*t] == stamp && self.recurrent(v, e) && self.action(condition, v, e)
                    })
                })
            })
    }
}

impl AcceptanceAccess for Acceptance {
    fn state_count(&self) -> usize {
        self.states.len()
    }

    fn action_count(&self) -> usize {
        self.actions.len()
    }

    fn recurrent(&self, source: usize, edge: usize) -> bool {
        self.recurrent[source][edge]
    }

    fn state(&self, condition: usize, vertex: usize) -> bool {
        self.states[condition][vertex]
    }

    fn action(&self, condition: usize, source: usize, edge: usize) -> bool {
        self.actions[condition][source][edge]
    }
}

/// Finds a deterministic accepting lasso for the supplied labels, or `None`
/// when no reachable cycle satisfies them. Prefixes are searched without EA
/// pruning; cycles are searched with it. The entry minimizes unrestricted
/// distance from a root, then vertex id. BFS parents use sorted frontiers and
/// original edge positions. SCC traversal order does not determine the answer.
/// Product vertex ids must be ordered by `(state id, tableau node id)` for
/// that tie-break to implement ADR 0018's product-order rule.
///
/// ```
/// use dive_liveness::{Graph, search::{Acceptance, find_lasso}};
/// let graph = Graph::new(vec![vec![1], vec![2], vec![1]], vec![0])?;
/// let labels = Acceptance {
///     recurrent: vec![vec![false], vec![true], vec![true]],
///     states: vec![vec![false, true, false], vec![false, false, true]],
///     actions: vec![],
/// };
/// let lasso = find_lasso(&graph, &labels)?.ok_or("expected accepting loop")?;
/// assert_eq!((lasso.root, lasso.entry), (0, 1));
/// assert_eq!(lasso.prefix.len(), 1);
/// assert_eq!(lasso.cycle.iter().map(|e| e.to).collect::<Vec<_>>(), [2, 1]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// Malformed label dimensions or an internal witness-construction bug.
pub fn find_lasso(graph: &Graph, labels: &Acceptance) -> Result<Option<Lasso>, SearchError> {
    labels.validate(graph)?;
    find_lasso_view(graph, labels)
}

pub(crate) fn find_lasso_view(
    graph: &Graph,
    labels: &impl AcceptanceAccess,
) -> Result<Option<Lasso>, SearchError> {
    let prefix = Paths::new(graph, &graph.roots, |_, _, _| true);
    let restricted = Graph {
        edges: graph
            .edges
            .iter()
            .enumerate()
            .map(|(v, targets)| {
                targets
                    .iter()
                    .enumerate()
                    .filter_map(|(e, t)| labels.recurrent(v, e).then_some(*t))
                    .collect()
            })
            .collect(),
        // A finite prefix may cross edges that fail EA. Such destinations
        // must remain candidates for a recurrent component.
        roots: prefix
            .distance
            .iter()
            .enumerate()
            .filter_map(|(v, d)| d.is_some().then_some(v))
            .collect(),
    };
    let mut selected: Option<(usize, usize, Vec<usize>)> = None;
    let mut membership = vec![0; graph.edges.len()];
    for (i, component) in restricted.cyclic_components().into_iter().enumerate() {
        let stamp = i + 1;
        for &v in &component {
            membership[v] = stamp;
        }
        if !labels.accepts(graph, &component, &membership, stamp) {
            continue;
        }
        let mut best = None;
        for &v in &component {
            let Some(d) = prefix.distance[v] else {
                return Err(SearchError::Invariant("component is not reachable"));
            };
            if best.is_none_or(|b| (d, v) < b) {
                best = Some((d, v));
            }
        }
        if let Some((d, v)) = best
            && selected
                .as_ref()
                .is_none_or(|(best, entry, _)| (d, v) < (*best, *entry))
        {
            selected = Some((d, v, component));
        }
    }
    let Some((_, entry, component)) = selected else {
        return Ok(None);
    };
    let mut inside = vec![false; graph.edges.len()];
    for v in component {
        inside[v] = true;
    }
    let steps = prefix.path(entry)?;
    let root = steps.first().map_or(entry, |s| s.from);
    let cycle = covering_cycle(graph, labels, &inside, entry)?;
    Ok(Some(Lasso {
        root,
        entry,
        prefix: steps,
        cycle,
    }))
}

struct Paths {
    distance: Vec<Option<usize>>,
    parent: Vec<Option<Step>>,
}

impl Paths {
    fn new(
        graph: &Graph,
        roots: &[usize],
        mut keep: impl FnMut(usize, usize, usize) -> bool,
    ) -> Self {
        let mut distance = vec![None; graph.edges.len()];
        let mut parent = vec![None; graph.edges.len()];
        let mut frontier = roots.to_vec();
        frontier.sort_unstable();
        frontier.dedup();
        for &v in &frontier {
            distance[v] = Some(0);
        }
        let mut depth = 0;
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for v in frontier {
                for (edge, &to) in graph.edges[v].iter().enumerate() {
                    if keep(v, edge, to) && distance[to].is_none() {
                        distance[to] = Some(depth + 1);
                        parent[to] = Some(Step { from: v, edge, to });
                        next.push(to);
                    }
                }
            }
            next.sort_unstable();
            frontier = next;
            depth += 1;
        }
        Self { distance, parent }
    }

    fn path(&self, target: usize) -> Result<Vec<Step>, SearchError> {
        if self.distance[target].is_none() {
            return Err(SearchError::Invariant("witness target is unreachable"));
        }
        let mut path = Vec::new();
        let mut v = target;
        while let Some(step) = self.parent[v] {
            path.push(step);
            v = step.from;
        }
        path.reverse();
        Ok(path)
    }
}

struct Outstanding {
    states: Vec<bool>,
    actions: Vec<bool>,
}

impl Outstanding {
    fn visit(&mut self, labels: &impl AcceptanceAccess, v: usize) {
        for (condition, missing) in self.states.iter_mut().enumerate() {
            *missing &= !labels.state(condition, v);
        }
    }

    fn traverse(&mut self, labels: &impl AcceptanceAccess, steps: &[Step]) {
        for s in steps {
            self.visit(labels, s.to);
            for (condition, missing) in self.actions.iter_mut().enumerate() {
                *missing &= !labels.action(condition, s.from, s.edge);
            }
        }
    }

    fn done(&self) -> bool {
        !self.states.iter().chain(&self.actions).any(|b| *b)
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "explicit deterministic covering-walk construction"
)]
fn covering_cycle(
    graph: &Graph,
    labels: &impl AcceptanceAccess,
    inside: &[bool],
    entry: usize,
) -> Result<Vec<Step>, SearchError> {
    // A self-loop that discharges everything is the canonical first choice.
    // The caller decides whether it projects to physical stuttering.
    for (edge, &to) in graph.edges[entry].iter().enumerate() {
        if to == entry
            && labels.recurrent(entry, edge)
            && (0..labels.state_count()).all(|condition| labels.state(condition, entry))
            && (0..labels.action_count()).all(|condition| labels.action(condition, entry, edge))
        {
            return Ok(vec![Step {
                from: entry,
                edge,
                to,
            }]);
        }
    }
    let mut missing = Outstanding {
        states: vec![true; labels.state_count()],
        actions: vec![true; labels.action_count()],
    };
    missing.visit(labels, entry);
    let mut current = entry;
    let mut cycle = Vec::new();
    let keep = |v: usize, edge: usize, to: usize| inside[to] && labels.recurrent(v, edge);
    while !missing.done() {
        let paths = Paths::new(graph, &[current], keep);
        // (distance to discharge, source/node id, optional edge position).
        let mut best = None;
        for (v, distance) in paths.distance.iter().enumerate() {
            let Some(distance) = distance else {
                continue;
            };
            if missing
                .states
                .iter()
                .enumerate()
                .any(|(condition, m)| *m && labels.state(condition, v))
            {
                let candidate = (*distance, v, None);
                if best.is_none_or(|b| candidate < b) {
                    best = Some(candidate);
                }
            }
            for (edge, &to) in graph.edges[v].iter().enumerate() {
                if keep(v, edge, to)
                    && missing
                        .actions
                        .iter()
                        .enumerate()
                        .any(|(condition, m)| *m && labels.action(condition, v, edge))
                {
                    let candidate = (*distance + 1, v, Some(edge));
                    if best.is_none_or(|b| candidate < b) {
                        best = Some(candidate);
                    }
                }
            }
        }
        let Some((_, v, edge)) = best else {
            return Err(SearchError::Invariant(
                "component cannot discharge a condition",
            ));
        };
        let mut steps = paths.path(v)?;
        if let Some(edge) = edge {
            steps.push(Step {
                from: v,
                edge,
                to: graph.edges[v][edge],
            });
        }
        current = steps.last().map_or(v, |s| s.to);
        missing.traverse(labels, &steps);
        cycle.extend(steps);
    }
    if cycle.is_empty() {
        // With no outstanding visits, still construct a nonempty cycle.
        // Choose the shortest one from entry; ties retain the first edge.
        // A reverse BFS finds return distances once, rather than running
        // a forward BFS for every outgoing edge of a high-degree entry.
        let mut reverse = Graph {
            edges: vec![vec![]; graph.edges.len()],
            roots: vec![entry],
        };
        for (v, targets) in graph.edges.iter().enumerate() {
            if !inside[v] {
                continue;
            }
            for (edge, &to) in targets.iter().enumerate() {
                if keep(v, edge, to) {
                    reverse.edges[to].push(v);
                }
            }
        }
        let back = Paths::new(&reverse, &[entry], |_, _, _| true);
        let best = graph.edges[entry]
            .iter()
            .enumerate()
            .filter(|(edge, to)| keep(entry, *edge, **to))
            .filter_map(|(edge, to)| back.distance[*to].map(|d| (d, edge, *to)))
            .min();
        let Some((_, edge, to)) = best else {
            return Err(SearchError::Invariant(
                "cyclic component has no closed walk",
            ));
        };
        let mut steps = vec![Step {
            from: entry,
            edge,
            to,
        }];
        steps.extend(Paths::new(graph, &[to], keep).path(entry)?);
        return Ok(steps);
    }
    cycle.extend(Paths::new(graph, &[current], keep).path(entry)?);
    Ok(cycle)
}
