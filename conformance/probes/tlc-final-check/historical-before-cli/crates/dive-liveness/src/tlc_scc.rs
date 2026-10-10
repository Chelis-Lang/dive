//! Bounded pinned final SCC visitation on an already constructed product graph.
//!
//! This uses the stack/link/root-queue contract of `LiveWorker.checkSccs`,
//! including reverse successor visitation and immediate component acceptance.
//! Supplying a graph is not a model proof or a TLC witness-selection certificate.

use crate::Graph;
use crate::product::Limits;
use crate::search::Acceptance;
use std::collections::VecDeque;

/// A malformed graph label, fixed resource ceiling or internal inconsistency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Supplied label dimensions or references are invalid.
    Input(String),
    /// A fixed entry/cell/inspection ceiling was exceeded.
    Limit(&'static str),
    /// A validated stack protocol failed; never an empty-language result.
    Invariant(&'static str),
}
impl Error {
    /// Stable diagnostic code; supplied graph projections imply no model proof.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Input(_) => "DIVE-L0060",
            Self::Limit(_) => "DIVE-L0061",
            Self::Invariant(_) => "DIVE-L0062",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input(e) => write!(f, "invalid final SCC input: {e}"),
            Self::Limit(e) => write!(f, "final SCC resource limit: {e}"),
            Self::Invariant(e) => write!(f, "final SCC invariant: {e}"),
        }
    }
}
impl std::error::Error for Error {}

/// One checked component, retaining anchor and actual stack-pop order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    /// The DFS root of this component, not a shortest-prefix choice.
    pub anchor: usize,
    /// Component members in actual `comStack` pop order; the anchor is last.
    pub members: Vec<usize>,
    /// Whether the actual acceptance conditions hold.
    pub accepting: bool,
}
/// Final SCC traversal evidence, independent of model enumeration/validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Traversal {
    /// First-visit order; duplicate root/stack admissions are not visits.
    pub discovery: Vec<usize>,
    /// Checked components through the first accepting one, in deciding order.
    pub components: Vec<Component>,
    /// Index of the first accepting component, or none after a complete search.
    pub accepting: Option<usize>,
    /// Actual admitted input/stack/edge/acceptance inspections.
    pub inspections: usize,
}

#[derive(Clone, Copy)]
enum Item {
    Node { id: usize, closing: bool },
    Low(usize),
}
struct Meter {
    used: usize,
    maximum: usize,
}
impl Meter {
    fn take(&mut self, amount: Option<usize>) -> Result<(), Error> {
        self.used = amount
            .and_then(|n| self.used.checked_add(n))
            .filter(|n| *n <= self.maximum)
            .ok_or(Error::Limit("SCC inspections"))?;
        Ok(())
    }
}
fn bounded(n: Option<usize>, limit: usize, name: &'static str) -> Result<usize, Error> {
    n.filter(|n| *n <= limit).ok_or(Error::Limit(name))
}
fn pop_low(stack: &mut Vec<Item>) -> Result<usize, Error> {
    match stack.pop() {
        Some(Item::Low(n)) => Ok(n),
        _ => Err(Error::Invariant("missing stack low link")),
    }
}
fn pop_node(stack: &mut Vec<Item>) -> Result<(usize, bool), Error> {
    match stack.pop() {
        Some(Item::Node { id, closing }) => Ok((id, closing)),
        _ => Err(Error::Invariant("missing stack node")),
    }
}
fn push(stack: &mut Vec<Item>, item: Item, meter: &mut Meter) -> Result<(), Error> {
    meter.take(Some(1))?;
    stack.push(item);
    Ok(())
}

fn validate(graph: &Graph, labels: &Acceptance, limits: Limits) -> Result<Meter, Error> {
    bounded(Some(graph.edges.len()), limits.vertices, "SCC vertices")?;
    let mut meter = Meter {
        used: 0,
        maximum: limits.steps,
    };
    meter.take(graph.edges.len().checked_add(graph.roots.len()))?;
    let edges = graph
        .edges
        .iter()
        .try_fold(0usize, |n, row| n.checked_add(row.len()));
    let edges = bounded(edges, limits.edges, "SCC edges")?;
    meter.take(Some(edges))?;
    // Charge outer shape inspections before walking supplied label vectors.
    meter.take(
        labels
            .states
            .len()
            .checked_add(labels.actions.len())
            .and_then(|n| n.checked_add(labels.recurrent.len())),
    )?;
    let adjacency = |rows: &[Vec<bool>]| -> Result<usize, Error> {
        if rows.len() != graph.edges.len()
            || rows
                .iter()
                .zip(&graph.edges)
                .any(|(a, b)| a.len() != b.len())
        {
            return Err(Error::Input("adjacency dimensions".into()));
        }
        Ok(edges)
    };
    let mut cells = adjacency(&labels.recurrent)?;
    for row in &labels.states {
        meter.take(Some(1))?;
        if row.len() != graph.edges.len() {
            return Err(Error::Input("state label dimensions".into()));
        }
        cells = cells
            .checked_add(row.len())
            .ok_or(Error::Limit("SCC acceptance cells"))?;
    }
    for rows in &labels.actions {
        meter.take(Some(rows.len()))?;
        cells = cells
            .checked_add(adjacency(rows)?)
            .ok_or(Error::Limit("SCC acceptance cells"))?;
    }
    bounded(Some(cells), limits.acceptance_cells, "SCC acceptance cells")?;
    meter.take(Some(cells))?;
    Ok(meter)
}

fn accepts(
    graph: &Graph,
    labels: &Acceptance,
    members: &[usize],
    membership: &mut [bool],
    meter: &mut Meter,
) -> Result<bool, Error> {
    if let [only] = members {
        // Pinned isStuttering returns the EA label of the *first* matching
        // self-edge, rather than searching for a later EA-true self-edge.
        let mut stuttering = false;
        for (e, target) in graph.edges[*only].iter().enumerate() {
            meter.take(Some(1))?;
            if target == only {
                stuttering = labels.recurrent[*only][e];
                break;
            }
        }
        if !stuttering {
            return Ok(false);
        }
    }
    for &id in members {
        meter.take(Some(1))?;
        membership[id] = true;
    }
    let mut result = true;
    for row in &labels.states {
        let mut found = false;
        for &id in members {
            meter.take(Some(1))?;
            found |= row[id];
        }
        result &= found;
    }
    for rows in &labels.actions {
        let mut found = false;
        for &id in members {
            for (e, &target) in graph.edges[id].iter().enumerate() {
                meter.take(Some(1))?;
                found |= membership[target] && labels.recurrent[id][e] && rows[id][e];
            }
        }
        result &= found;
    }
    for &id in members {
        membership[id] = false;
    }
    Ok(result)
}

/// Traverse a complete supplied product using the pinned final SCC order.
///
/// The finite prefix may cross EA-false edges: their still-undiscovered targets
/// enter the root FIFO. Every component is checked immediately, and traversal
/// stops at the first accepting component. No shortest-lasso choice is made.
/// These are fixed entry/cell/inspection bounds, not aggregate byte accounting.
///
/// # Errors
/// Malformed labels, a resource bound or a stack invariant failure. A bound
/// never returns a completed traversal or absence-of-counterexample claim.
#[allow(
    clippy::too_many_lines,
    reason = "retain the pinned stack/link protocol and deciding component order"
)]
pub fn check(graph: &Graph, labels: &Acceptance, limits: Limits) -> Result<Traversal, Error> {
    let mut meter = validate(graph, labels, limits)?;
    // None is an unvisited file pointer; MAX is a component already checked.
    let mut links = vec![None; graph.edges.len()];
    let mut membership = vec![false; graph.edges.len()];
    let mut roots: VecDeque<usize> = graph.roots.iter().copied().collect();
    let mut dfs = Vec::new();
    let mut component = Vec::new();
    let mut discovery = Vec::new();
    let mut components = Vec::new();
    while let Some(root) = roots.pop_front() {
        meter.take(Some(1))?;
        dfs.clear();
        push(
            &mut dfs,
            Item::Node {
                id: root,
                closing: false,
            },
            &mut meter,
        )?;
        push(&mut dfs, Item::Low(0), &mut meter)?;
        let mut next = 0usize;
        while dfs.len() >= 2 {
            meter.take(Some(1))?;
            let low = pop_low(&mut dfs)?;
            let (id, closing) = pop_node(&mut dfs)?;
            if closing {
                if links[id] == Some(low) {
                    let mut members = Vec::new();
                    loop {
                        meter.take(Some(1))?;
                        let member = component
                            .pop()
                            .ok_or(Error::Invariant("empty component stack"))?;
                        links[member] = Some(usize::MAX);
                        members.push(member);
                        if member == id {
                            break;
                        }
                    }
                    let accepting = accepts(graph, labels, &members, &mut membership, &mut meter)?;
                    components.push(Component {
                        anchor: id,
                        members,
                        accepting,
                    });
                    if accepting {
                        return Ok(Traversal {
                            discovery,
                            accepting: Some(components.len() - 1),
                            components,
                            inspections: meter.used,
                        });
                    }
                }
                let parent = pop_low(&mut dfs)?;
                push(&mut dfs, Item::Low(parent.min(low)), &mut meter)?;
            } else if let Some(link) = links[id] {
                push(&mut dfs, Item::Low(low.min(link)), &mut meter)?;
            } else {
                links[id] = Some(next);
                discovery.push(id);
                push(&mut dfs, Item::Low(low), &mut meter)?;
                push(&mut dfs, Item::Node { id, closing: true }, &mut meter)?;
                meter.take(Some(1))?;
                component.push(id);
                let mut child_low = next;
                next = next
                    .checked_add(1)
                    .ok_or(Error::Limit("SCC link indices"))?;
                for (edge, &target) in graph.edges[id].iter().enumerate() {
                    meter.take(Some(1))?;
                    if labels.recurrent[id][edge] {
                        if let Some(link) = links[target] {
                            child_low = child_low.min(link);
                        } else {
                            push(
                                &mut dfs,
                                Item::Node {
                                    id: target,
                                    closing: false,
                                },
                                &mut meter,
                            )?;
                        }
                    } else if links[target].is_none() {
                        meter.take(Some(1))?;
                        roots.push_back(target);
                    }
                }
                push(&mut dfs, Item::Low(child_low), &mut meter)?;
            }
        }
    }
    if !component.is_empty() {
        return Err(Error::Invariant("unfinished component stack"));
    }
    Ok(Traversal {
        discovery,
        components,
        accepting: None,
        inspections: meter.used,
    })
}
