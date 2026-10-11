//! Pinned greedy postfix, fingerprint-bucket closure and product-prefix selection.
//!
//! Supplied product graphs grant no model proof. A returned candidate still
//! requires independent validation of original concrete states and all claims.
use crate::{Graph, product::Limits, search::Acceptance, tlc_scc};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use tlc_scc::{Error, Traversal};

/// Original product identity, keeping tableau identity separate from fingerprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Identity {
    /// Original TLC state fingerprint.
    pub fingerprint: u64,
    /// Original particle number, or -1 for a null tableau.
    pub tableau: i32,
}
/// Actual pinned witness-selection fields; never a validated concrete witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Deciding component anchor.
    pub anchor: usize,
    /// Original DFS cycle stack in forward order, excluding its final node.
    pub cycle_stack: Vec<usize>,
    /// Final greedy DFS node, separate from the stack.
    pub postfix_end: usize,
    /// Closure path in reverse fingerprint order, excluding the anchor.
    pub closure_reverse: Vec<u64>,
    /// Prefix from anchor to initial state in reverse product-node order.
    pub prefix_reverse: Vec<usize>,
    /// Actual prefix/postfix selection work, separate from SCC and model counts.
    pub inspections: usize,
}
/// Complete supplied-graph SCC traversal and its first selected candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// Actual SCC visitation evidence.
    pub traversal: Traversal,
    /// Absent only after complete supplied-graph traversal with no acceptance.
    pub candidate: Option<Candidate>,
}

/// A partial supplied-graph selection with no completion/proof claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialSelection {
    selection: Selection,
    inspections: usize,
}
impl PartialSelection {
    /// Original SCC order through the deciding accepting component, if any.
    #[must_use]
    pub fn traversal(&self) -> &tlc_scc::Traversal {
        &self.selection.traversal
    }
    /// A candidate requiring independent replay; absence does not mean holds.
    #[must_use]
    pub fn candidate(&self) -> Option<&Candidate> {
        self.selection.candidate.as_ref()
    }
    /// A partial supplied graph never certifies completed model checking.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        false
    }
    /// Combined SCC and witness/identity work, including a nonaccepting check.
    #[must_use]
    pub fn inspections(&self) -> usize {
        self.inspections
    }
}
struct Meter {
    used: usize,
    maximum: usize,
}
impl Meter {
    fn take(&mut self, amount: usize) -> Result<(), Error> {
        self.used = self
            .used
            .checked_add(amount)
            .filter(|n| *n <= self.maximum)
            .ok_or(Error::Limit("witness inspections/entries"))?;
        Ok(())
    }
}
fn prefix(graph: &Graph, anchor: usize, meter: &mut Meter) -> Result<Vec<usize>, Error> {
    let mut parent = vec![None; graph.edges.len()];
    let mut queue = VecDeque::new();
    for &root in &graph.roots {
        meter.take(1)?;
        if root == anchor {
            return Ok(vec![root]);
        }
        parent[root] = Some(root);
        queue.push_back(root);
    }
    while let Some(id) = queue.pop_front() {
        meter.take(1)?;
        for &target in &graph.edges[id] {
            meter.take(1)?;
            if target == id {
                continue;
            }
            if target == anchor {
                let mut path = vec![anchor];
                let mut current = id;
                loop {
                    meter.take(1)?;
                    path.push(current);
                    let previous = parent[current].ok_or(Error::Invariant("prefix parent"))?;
                    if previous == current {
                        return Ok(path);
                    }
                    current = previous;
                }
            }
            if parent[target].is_none() {
                meter.take(1)?;
                parent[target] = Some(id);
                queue.push_back(target);
            }
        }
    }
    Err(Error::Invariant("accepting anchor has no initial prefix"))
}
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "retain pinned greedy state/promise/action discharge and backtracking order"
)]
fn greedy(
    graph: &Graph,
    labels: &Acceptance,
    anchor: usize,
    inside: &[bool],
    meter: &mut Meter,
) -> Result<(Vec<usize>, usize), Error> {
    let mut state_done = vec![false; labels.states.len()];
    let mut action_done = vec![false; labels.actions.len()];
    let mut left = state_done
        .len()
        .checked_add(action_done.len())
        .ok_or(Error::Limit("witness acceptance flags"))?;
    let mut seen = vec![false; graph.edges.len()];
    seen[anchor] = true;
    let mut stack = Vec::new();
    let mut current = anchor;
    while left > 0 {
        meter.take(1)?;
        let before = left;
        loop {
            meter.take(1)?;
            for (done, row) in state_done.iter_mut().zip(&labels.states) {
                meter.take(1)?;
                if !*done && row[current] {
                    *done = true;
                    left -= 1;
                }
            }
            if left == 0 {
                break;
            }
            let before_action = left;
            let mut last_eligible = None;
            let mut last_unseen = None;
            let mut gained = None;
            for (edge, &target) in graph.edges[current].iter().enumerate() {
                meter.take(1)?;
                if !inside[target] || !labels.recurrent[current][edge] {
                    continue;
                }
                last_eligible = Some(target);
                for (done, rows) in action_done.iter_mut().zip(&labels.actions) {
                    meter.take(1)?;
                    if !*done && rows[current][edge] {
                        *done = true;
                        left -= 1;
                    }
                }
                if left < before_action {
                    gained = Some(target);
                    break;
                }
                if !seen[target] {
                    last_unseen = Some(target);
                }
            }
            if let Some(target) = gained {
                meter.take(1)?;
                stack.push(current);
                current = target;
                seen.fill(false);
                break;
            }
            if left < before {
                let target =
                    last_eligible.ok_or(Error::Invariant("progress without internal successor"))?;
                meter.take(1)?;
                stack.push(current);
                current = target;
                seen.fill(false);
                break;
            }
            // The pinned backtracking scan deliberately has no EA test. A
            // candidate made invalid by that choice must fail independent validation.
            while last_unseen.is_none() {
                meter.take(1)?;
                current = stack
                    .pop()
                    .ok_or(Error::Invariant("greedy component exhausted"))?;
                for &target in &graph.edges[current] {
                    meter.take(1)?;
                    if inside[target] && !seen[target] {
                        last_unseen = Some(target);
                        break;
                    }
                }
            }
            let target = last_unseen.ok_or(Error::Invariant("missing unvisited successor"))?;
            meter.take(1)?;
            stack.push(current);
            current = target;
            seen[target] = true;
        }
    }
    Ok((stack, current))
}
fn closure(
    graph: &Graph,
    labels: &Acceptance,
    names: &[Identity],
    members: &[usize],
    anchor: usize,
    end: usize,
    meter: &mut Meter,
) -> Result<Vec<u64>, Error> {
    if anchor == end {
        return Ok(Vec::new());
    }
    // TNPT vectors retain component insertion (stack-pop) order within each
    // fingerprint; closure visits all those particles together, not just end.
    let mut buckets: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for &id in members {
        meter.take(1)?;
        buckets.entry(names[id].fingerprint).or_default().push(id);
    }
    let start = names[end].fingerprint;
    let mut queue = VecDeque::from([start]);
    let mut parents = BTreeMap::from([(start, start)]);
    while let Some(fp) = queue.pop_front() {
        meter.take(1)?;
        for &id in buckets.get(&fp).ok_or(Error::Invariant("closure bucket"))? {
            for (edge, &target) in graph.edges[id].iter().enumerate() {
                meter.take(1)?;
                if target == id || !labels.recurrent[id][edge] {
                    continue;
                }
                if target == anchor {
                    let mut result = Vec::new();
                    let mut current = fp;
                    loop {
                        meter.take(1)?;
                        result.push(current);
                        if current == start {
                            return Ok(result);
                        }
                        current = *parents
                            .get(&current)
                            .ok_or(Error::Invariant("closure parent"))?;
                    }
                }
                let next = names[target].fingerprint;
                if buckets.contains_key(&next) && !parents.contains_key(&next) {
                    meter.take(1)?;
                    parents.insert(next, fp);
                    queue.push_back(next);
                }
            }
        }
    }
    Err(Error::Invariant("no pinned postfix closure"))
}

/// Select the first accepting component and exact pinned product witness fields.
///
/// Bounds cover entries/cells/inspections of each SCC and witness stage; they
/// are not aggregate retained-byte accounting. No graph caller can obtain a
/// model-proof claim, and candidates must be independently replayed.
///
/// # Errors
/// Invalid identities/labels, resource ceilings or a pinned traversal invariant.
#[allow(
    clippy::too_many_lines,
    reason = "explicit preflight, SCC and candidate stage boundaries"
)]
pub fn select(
    graph: &Graph,
    labels: &Acceptance,
    identities: &[Identity],
    limits: Limits,
) -> Result<Selection, Error> {
    let traversal = tlc_scc::check(graph, labels, limits)?;
    select_with_traversal(graph, labels, identities, limits, traversal)
        .map(|(selection, _)| selection)
}

/// Pinned candidate selection after a partial SCC check.
///
/// An absent candidate is only a result for this partial supplied graph. This
/// diagnostic never establishes completed model checking. Unwritten records
/// retain their identities and cannot participate in the selected component.
///
/// # Errors
/// Invalid write/label/identity dimensions, bounds or traversal invariants.
pub fn select_partial(
    graph: &Graph,
    labels: &Acceptance,
    identities: &[Identity],
    written: &[bool],
    limits: Limits,
) -> Result<PartialSelection, Error> {
    let partial = tlc_scc::check_partial(graph, labels, written, limits)?;
    let scc_work = partial.traversal().inspections;
    let remaining = limits
        .steps
        .checked_sub(scc_work)
        .ok_or(Error::Limit("partial SCC/witness cumulative inspections"))?;
    let (selection, witness_work) = select_with_traversal(
        graph,
        labels,
        identities,
        Limits {
            steps: remaining,
            ..limits
        },
        partial.into_traversal(),
    )?;
    let inspections = scc_work
        .checked_add(witness_work)
        .ok_or(Error::Limit("partial SCC/witness cumulative inspections"))?;
    Ok(PartialSelection {
        selection,
        inspections,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "share pinned candidate logic without replacing partial SCC admission"
)]
fn select_with_traversal(
    graph: &Graph,
    labels: &Acceptance,
    identities: &[Identity],
    limits: Limits,
    traversal: tlc_scc::Traversal,
) -> Result<(Selection, usize), Error> {
    let mut meter = Meter {
        used: 0,
        maximum: limits.steps,
    };
    if identities.len() != graph.edges.len() {
        return Err(Error::Input("product identities length".into()));
    }
    meter.take(identities.len())?;
    let mut distinct = BTreeSet::new();
    for &id in identities {
        meter.take(1)?;
        if id.tableau < -1 || !distinct.insert(id) {
            return Err(Error::Input("invalid/duplicate product identity".into()));
        }
    }
    let Some(component) = traversal.accepting.map(|id| &traversal.components[id]) else {
        return Ok((
            Selection {
                traversal,
                candidate: None,
            },
            meter.used,
        ));
    };
    meter.take(graph.edges.len())?;
    let mut inside = vec![false; graph.edges.len()];
    for &id in &component.members {
        meter.take(1)?;
        inside[id] = true;
    }
    let anchor = component.anchor;
    let prefix_reverse = prefix(graph, anchor, &mut meter)?;
    let (cycle_stack, postfix_end) = greedy(graph, labels, anchor, &inside, &mut meter)?;
    let closure_reverse = closure(
        graph,
        labels,
        identities,
        &component.members,
        anchor,
        postfix_end,
        &mut meter,
    )?;
    let candidate = Candidate {
        anchor,
        prefix_reverse,
        cycle_stack,
        postfix_end,
        closure_reverse,
        inspections: meter.used,
    };
    Ok((
        Selection {
            traversal,
            candidate: Some(candidate),
        },
        meter.used,
    ))
}
