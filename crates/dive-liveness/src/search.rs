//! Accepting cycles on an already constructed, labelled product graph.
//!
//! This is the graph-search part of the temporal check. It neither constructs a
//! temporal tableau nor evaluates specification predicates, and cannot by
//! itself establish a model-checking verdict. The on-the-fly product supplies
//! its graph, including stutter edges, and its acceptance labels. Only
//! [`SearchError`] is public, as the cause carried by product errors.

use crate::Graph;
use crate::graph::GraphAccess;

#[cfg(test)]
mod lasso_tests;

/// Dense acceptance labels for one order-of-solution / possible-error-model
/// pair, the search tests' reference representation. Every state condition,
/// action condition and tableau promise must occur infinitely often. Promises
/// use the same representation as state conditions.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Acceptance {
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

/// Malformed labels, a search inconsistency or an exhausted run budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchError {
    /// Label dimensions do not match the graph.
    Shape(String),
    /// An accepting component failed to provide its promised witness.
    /// This is a bug, never an absence-of-violation result.
    Invariant(&'static str),
    /// The installed run budget expired during search; no verdict is implied.
    Budget,
    /// A checked search buffer could not be allocated.
    Allocation,
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape(s) => write!(f, "invalid acceptance labels: {s}"),
            Self::Invariant(s) => write!(f, "lasso search invariant: {s}"),
            Self::Budget => f.write_str("lasso search stopped by its run budget"),
            Self::Allocation => f.write_str("lasso search buffer allocation failed"),
        }
    }
}

impl std::error::Error for SearchError {}

// Poll at amortized work boundaries. A budget is control flow, never an
// acceptance label: exhaustion propagates as an error through every phase.
#[derive(Default)]
pub(crate) struct Poll {
    remaining: usize,
}

impl Poll {
    pub(crate) fn step(&mut self) -> Result<(), SearchError> {
        if self.remaining == 0 {
            if crate::budget::exhausted() {
                return Err(SearchError::Budget);
            }
            self.remaining = 1024;
        }
        self.remaining -= 1;
        Ok(())
    }

    pub(crate) fn any<T>(
        &mut self,
        values: impl IntoIterator<Item = T>,
        mut predicate: impl FnMut(T) -> bool,
    ) -> Result<bool, SearchError> {
        for value in values {
            self.step()?;
            if predicate(value) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn grow<T>(values: &mut Vec<T>, additional: usize) -> Result<(), SearchError> {
    values
        .len()
        .checked_add(additional)
        .and_then(|n| n.checked_mul(size_of::<T>()))
        .ok_or(SearchError::Allocation)?;
    values
        .try_reserve(additional)
        .map_err(|_| SearchError::Allocation)
}

fn initialized<T: Clone>(len: usize, value: T, poll: &mut Poll) -> Result<Vec<T>, SearchError> {
    let mut values = Vec::new();
    grow(&mut values, len)?;
    while values.len() < len {
        poll.step()?;
        values.resize(values.len().saturating_add(1024).min(len), value.clone());
    }
    Ok(values)
}

/// One edge of a witness. Its position distinguishes parallel edges with
/// different action labels, even when they have the same destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Step {
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
pub(crate) struct Lasso {
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

#[cfg(test)]
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
/// `edge` is the label an [`Adjacency`] yields with the edge: its row position
/// for stored adjacency, or the original behavior edge for an on-the-fly product.
pub(crate) trait AcceptanceAccess {
    fn state_count(&self) -> usize;
    fn action_count(&self) -> usize;
    fn recurrent(&self, source: usize, edge: usize) -> bool;
    fn state(&self, condition: usize, vertex: usize) -> bool;
    fn action(&self, condition: usize, source: usize, edge: usize) -> bool;

    fn accepts(
        &self,
        graph: &(impl Adjacency + ?Sized),
        component: &[usize],
        members: &[usize],
        stamp: usize,
        poll: &mut Poll,
    ) -> Result<bool, SearchError> {
        for condition in 0..self.state_count() {
            if !poll.any(component, |&v| self.state(condition, v))? {
                return Ok(false);
            }
        }
        for condition in 0..self.action_count() {
            let mut found = false;
            'members: for &v in component {
                let mut edges = Edges::new(graph, v);
                while let Some((_, t, label)) = edges.next()? {
                    poll.step()?;
                    if members[t] == stamp
                        && self.recurrent(v, label)
                        && self.action(condition, v, label)
                    {
                        found = true;
                        break 'members;
                    }
                }
            }
            if !found {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Out-edges in row order, possibly computed on demand. A cursor resumes a
/// row without storing it; each edge carries its target and the label that
/// [`AcceptanceAccess`] receives. Its position is the number of earlier edges.
pub(crate) trait Adjacency {
    type Cursor: Copy;
    fn vertex_count(&self) -> usize;
    fn roots(&self) -> &[usize];
    fn cursor(&self, source: usize) -> Self::Cursor;
    /// The next `(target, label)` of `source`, or `None` at the end of its row.
    ///
    /// # Errors
    /// An internal inconsistency of a computed row, never an absent edge.
    fn advance(
        &self,
        source: usize,
        cursor: &mut Self::Cursor,
    ) -> Result<Option<(usize, usize)>, SearchError>;
}

// Stored rows label each edge with its own row position.
impl<G: GraphAccess + ?Sized> Adjacency for G {
    type Cursor = usize;

    fn vertex_count(&self) -> usize {
        GraphAccess::vertex_count(self)
    }

    fn roots(&self) -> &[usize] {
        GraphAccess::roots(self)
    }

    fn cursor(&self, _: usize) -> usize {
        0
    }

    fn advance(
        &self,
        source: usize,
        cursor: &mut usize,
    ) -> Result<Option<(usize, usize)>, SearchError> {
        let Some(&target) = self.successors(source).get(*cursor) else {
            return Ok(None);
        };
        let label = *cursor;
        *cursor += 1;
        Ok(Some((target, label)))
    }
}

/// One row walk yielding `(position, target, label)`.
pub(crate) struct Edges<'g, G: Adjacency + ?Sized> {
    graph: &'g G,
    source: usize,
    cursor: G::Cursor,
    position: usize,
}

impl<'g, G: Adjacency + ?Sized> Edges<'g, G> {
    pub(crate) fn new(graph: &'g G, source: usize) -> Self {
        Self {
            graph,
            source,
            cursor: graph.cursor(source),
            position: 0,
        }
    }

    pub(crate) fn next(&mut self) -> Result<Option<(usize, usize, usize)>, SearchError> {
        let Some((target, label)) = self.graph.advance(self.source, &mut self.cursor)? else {
            return Ok(None);
        };
        let position = self.position;
        self.position += 1;
        Ok(Some((position, target, label)))
    }
}

/// The target and label of the edge at `position` in `source`'s row.
pub(crate) fn edge_at(
    graph: &(impl Adjacency + ?Sized),
    source: usize,
    position: usize,
    poll: &mut Poll,
) -> Result<(usize, usize), SearchError> {
    let mut edges = Edges::new(graph, source);
    while let Some((at, target, label)) = edges.next()? {
        poll.step()?;
        if at == position {
            return Ok((target, label));
        }
    }
    Err(SearchError::Invariant("witness edge is outside its row"))
}

#[cfg(test)]
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

/// Checks dense test labels against `graph`, then searches as
/// [`find_lasso_view`] does.
#[cfg(test)]
pub(crate) fn find_lasso(graph: &Graph, labels: &Acceptance) -> Result<Option<Lasso>, SearchError> {
    labels.validate(graph)?;
    find_lasso_view(graph, labels)
}

/// Shortest unrestricted prefix paths for one immutable product and root set.
/// Alternative fairness conditions only change the recurrent walk.
pub(crate) struct LassoSearch<'a, G: Adjacency + ?Sized> {
    graph: &'a G,
    prefix: Option<Paths>,
}

impl<'a, G: Adjacency + ?Sized> LassoSearch<'a, G> {
    pub(crate) fn new(graph: &'a G) -> Self {
        Self {
            graph,
            prefix: None,
        }
    }
    /// Finds a deterministic accepting lasso for the supplied labels, or `None`
    /// when no reachable cycle satisfies them. Prefixes are searched without EA
    /// pruning; cycles are searched with it. The entry minimizes unrestricted
    /// distance from a root, then vertex id. BFS parents use sorted frontiers and
    /// original edge positions. SCC traversal order does not determine the answer.
    /// Product vertex ids must be ordered by `(state id, tableau node id)` for
    /// that tie-break to implement the product-order rule.
    ///
    /// # Errors
    /// An internal witness-construction bug or an exhausted installed run budget.
    /// A budget error makes no claim about acceptance.
    pub(crate) fn find(
        &mut self,
        labels: &impl AcceptanceAccess,
    ) -> Result<Option<Lasso>, SearchError> {
        let mut poll = Poll::default();
        poll.step()?;
        let graph = self.graph;
        let prefix = match &mut self.prefix {
            Some(paths) => paths,
            slot @ None => {
                slot.insert(Paths::new(graph, graph.roots(), |_, _, _| true, &mut poll)?)
            }
        };
        // A finite prefix may cross edges that fail EA. Such destinations must
        // remain candidates for a recurrent component, so every reachable vertex
        // is a root of the EA-restricted decomposition, in vertex order.
        let mut selected: Option<(usize, usize, Vec<usize>)> = None;
        let mut membership = initialized(graph.vertex_count(), 0, &mut poll)?;
        let mut stamp = 0_usize;
        restricted_components(
            graph,
            |v| prefix.distance(v).is_some(),
            |v, e| labels.recurrent(v, e),
            &mut poll,
            |component, poll| {
                if component.len() == 1 {
                    let v = component[0];
                    let mut edges = Edges::new(graph, v);
                    let mut cyclic = false;
                    while let Some((_, to, label)) = edges.next()? {
                        poll.step()?;
                        if to == v && labels.recurrent(v, label) {
                            cyclic = true;
                            break;
                        }
                    }
                    if !cyclic {
                        return Ok(());
                    }
                }
                // Components are disjoint, so a fresh stamp never matches a stale one.
                stamp += 1;
                for &v in &component {
                    poll.step()?;
                    membership[v] = stamp;
                }
                if !labels.accepts(graph, &component, &membership, stamp, poll)? {
                    return Ok(());
                }
                let mut best = None;
                for &v in &component {
                    poll.step()?;
                    let Some(d) = prefix.distance(v) else {
                        return Err(SearchError::Invariant("component is not reachable"));
                    };
                    if best.is_none_or(|b| (d, v) < b) {
                        best = Some((d, v));
                    }
                }
                // `(distance, vertex)` is a strict total order over disjoint
                // components, so the selection is independent of emission order.
                if let Some((d, v)) = best
                    && selected
                        .as_ref()
                        .is_none_or(|(best, entry, _)| (d, v) < (*best, *entry))
                {
                    selected = Some((d, v, component));
                }
                Ok(())
            },
        )?;
        drop(membership);
        let Some((_, entry, component)) = selected else {
            if crate::budget::exhausted() {
                return Err(SearchError::Budget);
            }
            return Ok(None);
        };
        let mut inside = initialized(graph.vertex_count(), false, &mut poll)?;
        for v in component {
            poll.step()?;
            inside[v] = true;
        }
        let steps = prefix.path(entry, &mut poll)?;
        self.prefix = None;
        let root = steps.first().map_or(entry, |s| s.from);
        let cycle = covering_cycle(graph, labels, &inside, entry, &mut poll)?;
        // Catch a trip during the final bounded unit before returning a witness.
        if crate::budget::exhausted() {
            return Err(SearchError::Budget);
        }
        Ok(Some(Lasso {
            root,
            entry,
            prefix: steps,
            cycle,
        }))
    }
}

#[cfg(test)]
pub(crate) fn find_lasso_view(
    graph: &(impl Adjacency + ?Sized),
    labels: &impl AcceptanceAccess,
) -> Result<Option<Lasso>, SearchError> {
    LassoSearch::new(graph).find(labels)
}

const NONE: usize = usize::MAX;

// Iterative Tarjan decomposition of the EA-restricted graph (recurrent edges
// only), rooted at every prefix-reachable vertex in vertex order. Restricted
// edges are filtered while walking the rows; no restricted copy is stored.
// Each component is passed on with its members in vertex order.
pub(crate) fn restricted_components<G: Adjacency + ?Sized>(
    graph: &G,
    reachable: impl Fn(usize) -> bool,
    recurrent: impl Fn(usize, usize) -> bool,
    poll: &mut Poll,
    mut visit: impl FnMut(Vec<usize>, &mut Poll) -> Result<(), SearchError>,
) -> Result<(), SearchError> {
    let n = graph.vertex_count();
    let mut index = initialized(n, NONE, poll)?;
    let mut low = initialized(n, 0, poll)?;
    let mut active = initialized(n, false, poll)?;
    let mut stack = Vec::new();
    let mut dfs: Vec<(usize, G::Cursor)> = Vec::new();
    let mut next_index = 0;
    for root in 0..n {
        poll.step()?;
        if !reachable(root) || index[root] != NONE {
            continue;
        }
        index[root] = next_index;
        low[root] = next_index;
        next_index += 1;
        active[root] = true;
        grow(&mut stack, 1)?;
        grow(&mut dfs, 1)?;
        stack.push(root);
        dfs.push((root, graph.cursor(root)));
        while let Some((v, cursor)) = dfs.last_mut() {
            poll.step()?;
            let v = *v;
            if let Some((target, label)) = graph.advance(v, cursor)? {
                if !recurrent(v, label) {
                    continue;
                }
                if index[target] == NONE {
                    index[target] = next_index;
                    low[target] = next_index;
                    next_index += 1;
                    active[target] = true;
                    grow(&mut stack, 1)?;
                    grow(&mut dfs, 1)?;
                    stack.push(target);
                    dfs.push((target, graph.cursor(target)));
                } else if active[target] {
                    low[v] = low[v].min(index[target]);
                }
                continue;
            }
            dfs.pop();
            if index[v] == low[v] {
                let mut component = Vec::new();
                while let Some(member) = stack.pop() {
                    poll.step()?;
                    active[member] = false;
                    grow(&mut component, 1)?;
                    component.push(member);
                    if member == v {
                        break;
                    }
                }
                component.sort_unstable();
                visit(component, poll)?;
            }
            if let Some(&(parent, _)) = dfs.last() {
                low[parent] = low[parent].min(low[v]);
            }
        }
    }
    poll.step()
}

// Breadth-first distances and canonical parents. `NONE` marks an unreached
// vertex; a parent is its `(source, row position)`.
struct Paths {
    distance: Vec<usize>,
    parent: Vec<(usize, usize)>,
}

impl Paths {
    fn new<G: Adjacency + ?Sized>(
        graph: &G,
        roots: &[usize],
        mut keep: impl FnMut(usize, usize, usize) -> bool,
        poll: &mut Poll,
    ) -> Result<Self, SearchError> {
        let mut distance = initialized(graph.vertex_count(), NONE, poll)?;
        let mut parent = initialized(graph.vertex_count(), (NONE, NONE), poll)?;
        let mut frontier = Vec::new();
        grow(&mut frontier, roots.len())?;
        frontier.extend_from_slice(roots);
        frontier.sort_unstable();
        frontier.dedup();
        for &v in &frontier {
            poll.step()?;
            distance[v] = 0;
        }
        let mut depth = 0;
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for v in frontier {
                let mut edges = Edges::new(graph, v);
                while let Some((edge, to, label)) = edges.next()? {
                    poll.step()?;
                    if keep(v, label, to) && distance[to] == NONE {
                        distance[to] = depth + 1;
                        parent[to] = (v, edge);
                        grow(&mut next, 1)?;
                        next.push(to);
                    }
                }
            }
            next.sort_unstable();
            frontier = next;
            depth += 1;
        }
        Ok(Self { distance, parent })
    }

    fn distance(&self, v: usize) -> Option<usize> {
        Some(self.distance[v]).filter(|d| *d != NONE)
    }

    fn path(&self, target: usize, poll: &mut Poll) -> Result<Vec<Step>, SearchError> {
        if self.distance[target] == NONE {
            return Err(SearchError::Invariant("witness target is unreachable"));
        }
        let mut path = Vec::new();
        let mut v = target;
        while self.distance[v] != 0 {
            let (from, edge) = self.parent[v];
            if from == NONE {
                return Err(SearchError::Invariant("witness parent is missing"));
            }
            poll.step()?;
            grow(&mut path, 1)?;
            path.push(Step { from, edge, to: v });
            v = from;
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
    fn visit(
        &mut self,
        labels: &impl AcceptanceAccess,
        v: usize,
        poll: &mut Poll,
    ) -> Result<(), SearchError> {
        for (condition, missing) in self.states.iter_mut().enumerate() {
            poll.step()?;
            *missing &= !labels.state(condition, v);
        }
        Ok(())
    }

    fn traverse(
        &mut self,
        graph: &(impl Adjacency + ?Sized),
        labels: &impl AcceptanceAccess,
        steps: &[Step],
        poll: &mut Poll,
    ) -> Result<(), SearchError> {
        for s in steps {
            self.visit(labels, s.to, poll)?;
            let (_, label) = edge_at(graph, s.from, s.edge, poll)?;
            for (condition, missing) in self.actions.iter_mut().enumerate() {
                poll.step()?;
                *missing &= !labels.action(condition, s.from, label);
            }
        }
        Ok(())
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
    graph: &(impl Adjacency + ?Sized),
    labels: &impl AcceptanceAccess,
    inside: &[bool],
    entry: usize,
    poll: &mut Poll,
) -> Result<Vec<Step>, SearchError> {
    // A self-loop that discharges everything is the canonical first choice.
    // The caller decides whether it projects to physical stuttering.
    let mut edges = Edges::new(graph, entry);
    while let Some((edge, to, label)) = edges.next()? {
        poll.step()?;
        if to == entry
            && labels.recurrent(entry, label)
            && !poll.any(0..labels.state_count(), |condition| {
                !labels.state(condition, entry)
            })?
            && !poll.any(0..labels.action_count(), |condition| {
                !labels.action(condition, entry, label)
            })?
        {
            return Ok(vec![Step {
                from: entry,
                edge,
                to,
            }]);
        }
    }
    let mut missing = Outstanding {
        states: initialized(labels.state_count(), true, poll)?,
        actions: initialized(labels.action_count(), true, poll)?,
    };
    missing.visit(labels, entry, poll)?;
    let mut current = entry;
    let mut cycle = Vec::new();
    let keep = |v: usize, label: usize, to: usize| inside[to] && labels.recurrent(v, label);
    while !missing.done() {
        let paths = Paths::new(graph, &[current], keep, poll)?;
        // (distance to discharge, source/node id, optional edge position).
        let mut best = None;
        for v in 0..graph.vertex_count() {
            poll.step()?;
            let Some(distance) = paths.distance(v) else {
                continue;
            };
            if poll.any(missing.states.iter().enumerate(), |(condition, m)| {
                *m && labels.state(condition, v)
            })? {
                let candidate = (distance, v, None);
                if best.is_none_or(|b| candidate < b) {
                    best = Some(candidate);
                }
            }
            let mut edges = Edges::new(graph, v);
            while let Some((edge, to, label)) = edges.next()? {
                poll.step()?;
                if keep(v, label, to)
                    && poll.any(missing.actions.iter().enumerate(), |(condition, m)| {
                        *m && labels.action(condition, v, label)
                    })?
                {
                    let candidate = (distance + 1, v, Some(edge));
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
        let mut steps = paths.path(v, poll)?;
        if let Some(edge) = edge {
            let (to, _) = edge_at(graph, v, edge, poll)?;
            grow(&mut steps, 1)?;
            steps.push(Step { from: v, edge, to });
        }
        current = steps.last().map_or(v, |s| s.to);
        missing.traverse(graph, labels, &steps, poll)?;
        grow(&mut cycle, steps.len())?;
        cycle.extend(steps);
    }
    if cycle.is_empty() {
        // With no outstanding visits, still construct a nonempty cycle.
        // Choose the shortest one from entry; ties retain the first edge.
        // A reverse BFS finds return distances once, rather than running
        // a forward BFS for every outgoing edge of a high-degree entry.
        let mut reverse = Graph {
            edges: initialized(graph.vertex_count(), Vec::new(), poll)?,
            roots: vec![entry],
        };
        for (v, &within) in inside.iter().enumerate() {
            poll.step()?;
            if !within {
                continue;
            }
            let mut edges = Edges::new(graph, v);
            while let Some((_, to, label)) = edges.next()? {
                poll.step()?;
                if keep(v, label, to) {
                    grow(&mut reverse.edges[to], 1)?;
                    reverse.edges[to].push(v);
                }
            }
        }
        let back = Paths::new(&reverse, &[entry], |_, _, _| true, poll)?;
        drop(reverse);
        let mut best = None;
        let mut edges = Edges::new(graph, entry);
        while let Some((edge, to, label)) = edges.next()? {
            poll.step()?;
            if keep(entry, label, to)
                && let Some(distance) = back.distance(to)
            {
                let candidate = (distance, edge, to);
                if best.is_none_or(|old| candidate < old) {
                    best = Some(candidate);
                }
            }
        }
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
        let tail = Paths::new(graph, &[to], keep, poll)?.path(entry, poll)?;
        grow(&mut steps, tail.len())?;
        steps.extend(tail);
        return Ok(steps);
    }
    let tail = Paths::new(graph, &[current], keep, poll)?.path(entry, poll)?;
    grow(&mut cycle, tail.len())?;
    cycle.extend(tail);
    Ok(cycle)
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "finite graph controls")]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn impossible_search_buffers_fail_before_mutating_existing_values() {
        let mut values = vec![7_usize];
        let capacity = values.capacity();
        assert_eq!(grow(&mut values, usize::MAX), Err(SearchError::Allocation));
        assert_eq!(values, [7]);
        assert_eq!(values.capacity(), capacity);
        assert!(matches!(
            crate::product::ProductError::search(SearchError::Allocation),
            crate::product::ProductError::Limit("search allocation")
        ));
    }

    #[test]
    fn a_budget_trip_during_labelled_search_is_control_flow() {
        struct InstallsBudget<'a> {
            budget: &'a dive_runtime::budget::Budget,
            scope: RefCell<Option<crate::budget::Scope>>,
            accesses: Cell<usize>,
        }
        impl AcceptanceAccess for InstallsBudget<'_> {
            fn state_count(&self) -> usize {
                0
            }
            fn action_count(&self) -> usize {
                0
            }
            fn recurrent(&self, _: usize, _: usize) -> bool {
                self.accesses.set(self.accesses.get() + 1);
                if self.scope.borrow().is_none() {
                    self.scope.replace(Some(crate::budget::enter(self.budget)));
                }
                true
            }
            fn state(&self, _: usize, _: usize) -> bool {
                true
            }
            fn action(&self, _: usize, _: usize, _: usize) -> bool {
                true
            }
        }
        let watchdog = crate::budget::expired_for_test();
        let graph = Graph::new((0..4096).map(|v| vec![(v + 1) % 4096]).collect(), vec![0])
            .expect("one large accepting SCC");
        let labels = InstallsBudget {
            budget: watchdog.budget(),
            scope: RefCell::new(None),
            accesses: Cell::new(0),
        };
        // The budget changes during recurrent-edge construction, after prefix
        // BFS has succeeded. Truth remains TRUE for every inspected edge.
        assert_eq!(find_lasso_view(&graph, &labels), Err(SearchError::Budget));
        assert!((1..=1024).contains(&labels.accesses.get()));
        assert!(matches!(
            crate::product::ProductError::search(SearchError::Budget),
            crate::product::ProductError::Limit(crate::budget::EXHAUSTED)
        ));
    }

    #[test]
    fn a_single_large_scc_has_fallible_poll_points() {
        let graph =
            Graph::new((0..4096).map(|v| vec![(v + 1) % 4096]).collect(), vec![0]).expect("cycle");
        let mut steps = 0;
        let result = graph.try_components(
            || {
                steps += 1;
                if steps == 2048 {
                    Err("stopped inside SCC")
                } else {
                    Ok(())
                }
            },
            None,
        );
        assert_eq!(result, Err("stopped inside SCC"));
        assert_eq!(steps, 2048);
        assert_eq!(graph.components().len(), 1);
    }
}
