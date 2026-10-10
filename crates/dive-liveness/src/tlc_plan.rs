//! Bounded `Liveness.processLiveness` normalization and grouping projections.
//!
//! This module does not construct TLC tableaux, execute graph checks, select
//! witnesses, or certify a model. Input formulas must already contain the
//! caller's pure, correctly bound predicates and expanded fairness. Predicate
//! visits get fresh tags before rewriting. Repeated visits to one supplied arena
//! node retain the last tag at every reference, matching TLC object sharing;
//! separately constructed truth-equivalent predicates are never merged. The existing independent word interpreter can evaluate
//! every intermediate stage using the retained occurrence-to-input mapping.

use crate::formula::{Atom, AtomKind, Formula, FormulaError, Limits, Node};
use std::collections::BTreeMap;

/// A supplied temporal formula and the identity of its input table.
#[derive(Clone, Copy, Debug)]
pub struct Property<'a> {
    /// Supplied name, retained without attributing DNF branches. Model role
    /// labels and original configured identities are separate tables.
    pub name: &'a str,
    /// Pure bound formula, prior to negation.
    pub formula: &'a Formula,
}

/// An occurrence tag mapped back to a caller's original atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occurrence {
    /// Input index: fairness first, followed by configured properties.
    pub input: usize,
    /// Original atom index and state/action domain in that input.
    pub atom: Atom,
}

/// Ordered index arrays into one order-of-solution's predicate bins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PossibleErrorModel {
    /// `[]<>action`, in branch encounter order.
    pub ae_action: Vec<usize>,
    /// `[]<>state`, in branch encounter order.
    pub ae_state: Vec<usize>,
    /// `<>[]predicate` (state or action), in branch encounter order.
    pub ea_action: Vec<usize>,
}

/// TLC's first-encounter grouping, before binary/tableau construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderOfSolution {
    /// Other, action-free temporal conjuncts in syntax order. Empty means
    /// TLC's null temporal formula; one entry is a singleton, multiple entries
    /// denote an ordered conjunction. Tableau identities remain unimplemented.
    pub temporal: Vec<usize>,
    /// Ordered unique state checks; indices reference the projection arena.
    pub check_state: Vec<usize>,
    /// Ordered unique action checks; indices reference the projection arena.
    pub check_action: Vec<usize>,
    /// Branches in their original DNF encounter order.
    pub pems: Vec<PossibleErrorModel>,
}

/// Auditable intermediate expressions and pre-tableau grouping.
#[derive(Clone, Debug)]
pub struct Projection {
    arena: Vec<Node>,
    /// Supplied temporal-input names in order, never inferred from DNF.
    /// The model wrapper supplies role labels; original configured identities
    /// are retained separately in [`BoundProjection::configured_properties`].
    pub properties: Vec<String>,
    /// Predicate occurrence mapping; arena atom indices index this table.
    /// TLC's tag is the table index plus one. Repeated visits of one input
    /// arena node leave earlier unused tag slots, as in TLC's mutable tag pass.
    pub occurrences: Vec<Occurrence>,
    /// Combined fairness AND negated properties, tagged before rewriting.
    /// `None` means both input lists were empty.
    pub tagged: Option<usize>,
    /// Negations pushed to predicates and boolean constants.
    pub pushed: Option<usize>,
    /// Pinned LN simplification, without extra boolean/temporal identities.
    pub simplified: Option<usize>,
    /// Ordered DNF, including TLC's recurrence-disjunction collection.
    pub dnf: Option<usize>,
    /// Pre-tableau order-of-solution and possible-error-model projections.
    pub orders: Vec<OrderOfSolution>,
}

impl Projection {
    /// Checked arena in construction order. All projection indices refer here.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.arena
    }

    /// Builds a checked formula rooted at an intermediate stage or bin entry.
    /// Its atoms are occurrence tags, mapped through [`Self::occurrences`].
    ///
    /// # Errors
    /// A root outside the retained arena.
    pub fn formula(&self, root: usize) -> Result<Formula, FormulaError> {
        Formula::new(self.arena.clone(), root)
    }
}

/// A normalization resource/reference failure or TLC's named formula refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionError {
    /// Checked-arena or explicitly bounded construction failure.
    Formula(FormulaError),
    /// Action-containing temporal syntax outside TLC's EA/AE special forms.
    ActionInTemporal,
}
impl ProjectionError {
    /// Stable planning diagnostic, separate from model-checking verdict codes.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ActionInTemporal => "DIVE-L0020",
            Self::Formula(FormulaError::Limit(_)) => "DIVE-L0021",
            Self::Formula(FormulaError::Reference(_)) => "DIVE-L0022",
        }
    }
    /// Pinned reference code for a matched classifier/cardinality refusal.
    /// Dive's own term/work/depth limits have no invented TLC code.
    #[must_use]
    pub fn tlc_code(&self) -> Option<u32> {
        match self {
            Self::ActionInTemporal => Some(2214),
            Self::Formula(FormulaError::Limit("TLC DNF cardinality")) => Some(2213),
            Self::Formula(_) => None,
        }
    }
}
impl From<FormulaError> for ProjectionError {
    fn from(error: FormulaError) -> Self {
        Self::Formula(error)
    }
}
impl std::fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Formula(e) => e.fmt(f),
            Self::ActionInTemporal => {
                write!(f, "unsupported TLC liveness planning: action outside EA/AE")
            }
        }
    }
}
impl std::error::Error for ProjectionError {}

struct Build {
    nodes: Vec<Node>,
    occurrences: Vec<Occurrence>,
    // Each input arena node is one reference object. Repeated visits assign a
    // fresh tag but replace every reference to that object with its final tag.
    // Mapping entries/references are bounded by already charged input/import
    // terms; this is not an aggregate metadata-byte budget.
    shared_atoms: BTreeMap<(usize, usize), Vec<usize>>,
    terms: usize,
    steps: usize,
    limits: Limits,
}

impl Build {
    fn step(&mut self, depth: usize) -> Result<(), FormulaError> {
        if depth > 256 {
            return Err(FormulaError::Limit("TLC planning depth"));
        }
        self.steps = self
            .steps
            .checked_add(1)
            .ok_or(FormulaError::Limit("TLC planning work"))?;
        if self.steps > self.limits.expansion_steps {
            return Err(FormulaError::Limit("TLC planning work"));
        }
        Ok(())
    }

    fn room(&self, count: usize) -> Result<(), FormulaError> {
        self.terms
            .checked_add(count)
            .filter(|n| *n <= self.limits.formula_nodes)
            .ok_or(FormulaError::Limit("TLC planning terms"))?;
        Ok(())
    }

    fn push(&mut self, node: Node) -> Result<usize, FormulaError> {
        let children = match &node {
            Node::And(cs) | Node::Or(cs) => cs.len(),
            Node::Not(_) | Node::Always(_) | Node::Eventually(_) => 1,
            Node::Bool(_) | Node::Atom(_) => 0,
        };
        self.room(1 + children)?;
        let index = self.nodes.len();
        self.terms += 1 + children;
        self.nodes.push(node);
        Ok(index)
    }

    // addConj/addDisj flatten only same-kind junctions, in insertion order.
    fn junction(&mut self, and: bool, children: Vec<usize>) -> Result<usize, FormulaError> {
        let mut flattened = Vec::new();
        let mut pending: Vec<_> = children.into_iter().rev().collect();
        while let Some(child) = pending.pop() {
            self.step(0)?;
            if let Node::And(cs) | Node::Or(cs) = &self.nodes[child]
                && matches!(self.nodes[child], Node::And(_)) == and
            {
                self.room(
                    flattened
                        .len()
                        .saturating_add(pending.len())
                        .saturating_add(cs.len())
                        .saturating_add(1),
                )?;
                pending.extend(cs.iter().rev());
            } else {
                self.room(flattened.len().saturating_add(2))?;
                flattened.push(child);
            }
        }
        self.push(if and {
            Node::And(flattened)
        } else {
            Node::Or(flattened)
        })
    }

    fn import(
        &mut self,
        f: &Formula,
        i: usize,
        input: usize,
        depth: usize,
    ) -> Result<usize, FormulaError> {
        self.step(depth)?;
        let node = match &f.nodes()[i] {
            Node::Atom(a) => {
                self.room(1)?;
                let index = self.occurrences.len();
                self.occurrences.push(Occurrence { input, atom: *a });
                let key = (input, i);
                let count = self.shared_atoms.get(&key).map_or(0, Vec::len);
                for at in 0..count {
                    self.step(depth)?;
                    let reference = self
                        .shared_atoms
                        .get(&key)
                        .and_then(|references| references.get(at))
                        .copied()
                        .ok_or(FormulaError::Reference("TLC shared predicate tag".into()))?;
                    if let Node::Atom(previous) = &mut self.nodes[reference] {
                        previous.index = index;
                    }
                }
                self.shared_atoms
                    .entry(key)
                    .or_default()
                    .push(self.nodes.len());
                Node::Atom(Atom {
                    kind: a.kind,
                    index,
                })
            }
            Node::Bool(b) => Node::Bool(*b),
            Node::Not(c) => Node::Not(self.import(f, *c, input, depth + 1)?),
            Node::Always(c) => Node::Always(self.import(f, *c, input, depth + 1)?),
            Node::Eventually(c) => Node::Eventually(self.import(f, *c, input, depth + 1)?),
            Node::And(cs) | Node::Or(cs) => {
                self.room(cs.len().saturating_add(1))?;
                let children = cs
                    .iter()
                    .map(|c| self.import(f, *c, input, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                // Preserve constructor shape until pushNeg uses addConj/addDisj.
                if matches!(f.nodes()[i], Node::And(_)) {
                    Node::And(children)
                } else {
                    Node::Or(children)
                }
            }
        };
        self.push(node)
    }

    fn negation(&mut self, i: usize, negative: bool, depth: usize) -> Result<usize, FormulaError> {
        self.step(depth)?;
        match self.nodes[i].clone() {
            Node::Atom(_) => {
                if negative {
                    self.push(Node::Not(i))
                } else {
                    Ok(i)
                }
            }
            Node::Bool(b) => self.push(Node::Bool(b != negative)),
            Node::Not(c) => self.negation(c, !negative, depth + 1),
            Node::And(cs) | Node::Or(cs) => {
                let and = matches!(self.nodes[i], Node::And(_)) != negative;
                let children = cs
                    .into_iter()
                    .map(|c| self.negation(c, negative, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                self.junction(and, children)
            }
            Node::Always(c) | Node::Eventually(c) => {
                let always = matches!(self.nodes[i], Node::Always(_)) != negative;
                let child = self.negation(c, negative, depth + 1)?;
                self.push(if always {
                    Node::Always(child)
                } else {
                    Node::Eventually(child)
                })
            }
        }
    }

    fn simplify(&mut self, i: usize, depth: usize) -> Result<usize, FormulaError> {
        self.step(depth)?;
        match self.nodes[i].clone() {
            Node::Bool(_) | Node::Atom(_) => Ok(i),
            Node::Not(c) => {
                // After pushNeg this is only an atomic negation.
                let child = self.simplify(c, depth + 1)?;
                self.push(Node::Not(child))
            }
            Node::And(cs) | Node::Or(cs) => {
                let and = matches!(self.nodes[i], Node::And(_));
                let mut children = Vec::new();
                for c in cs {
                    let child = self.simplify(c, depth + 1)?;
                    if let Node::Bool(b) = self.nodes[child] {
                        if b != and {
                            return self.push(Node::Bool(b));
                        }
                    } else {
                        children.push(child);
                    }
                }
                match children.len() {
                    0 => self.push(Node::Bool(and)),
                    1 => Ok(children[0]),
                    _ => self.junction(and, children),
                }
            }
            Node::Always(c) | Node::Eventually(c) => {
                let always = matches!(self.nodes[i], Node::Always(_));
                let mut child = self.simplify(c, depth + 1)?;
                match self.nodes[child] {
                    Node::Always(c) if always => child = c,
                    Node::Eventually(c) if !always => child = c,
                    _ => {}
                }
                // In particular []TRUE and <>FALSE are NOT folded by TLC.
                self.push(if always {
                    Node::Always(child)
                } else {
                    Node::Eventually(child)
                })
            }
        }
    }

    fn ae_body(&self, i: usize) -> Option<usize> {
        if let Node::Always(c) = self.nodes[i]
            && let Node::Eventually(body) = self.nodes[c]
        {
            Some(body)
        } else {
            None
        }
    }

    fn dnf(&mut self, i: usize, depth: usize) -> Result<usize, FormulaError> {
        self.step(depth)?;
        match self.nodes[i].clone() {
            Node::Or(cs) => {
                let mut plain = Vec::new();
                let mut ae = Vec::new();
                for c in cs {
                    let c = self.dnf(c, depth + 1)?;
                    let children = if let Node::Or(cs) = &self.nodes[c] {
                        cs.clone()
                    } else {
                        vec![c]
                    };
                    for child in children {
                        if let Some(body) = self.ae_body(child) {
                            ae.push(body);
                        } else {
                            plain.push(child);
                        }
                    }
                }
                if !ae.is_empty() {
                    let body = if ae.len() == 1 {
                        ae[0]
                    } else {
                        self.junction(false, ae)?
                    };
                    let even = self.push(Node::Eventually(body))?;
                    plain.push(self.push(Node::Always(even))?);
                }
                self.junction(false, plain)
            }
            Node::And(cs) => {
                // TLC converts every child first, before computing the outer
                // product. Preserve nested-failure precedence in that order.
                let converted = cs
                    .into_iter()
                    .map(|c| self.dnf(c, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut columns = Vec::new();
                let mut total = 1usize;
                for c in converted {
                    match &self.nodes[c] {
                        Node::Or(cs) => {
                            total = total
                                .checked_mul(cs.len())
                                // Pinned LNConj.toDNF multiplies Java ints.
                                .filter(|n| *n <= 2_147_483_647)
                                .ok_or(FormulaError::Limit("TLC DNF cardinality"))?;
                            columns.push(cs.clone());
                        }
                        Node::And(cs) => {
                            for &c in cs {
                                columns.push(vec![c]);
                            }
                        }
                        _ => columns.push(vec![c]),
                    }
                }
                // Charge the whole prospective cartesian output before allocating it.
                let terms = total
                    .checked_mul(columns.len().saturating_add(2))
                    .and_then(|n| n.checked_add(1))
                    .ok_or(FormulaError::Limit("TLC DNF cardinality"))?;
                self.room(terms)?;
                self.steps
                    .checked_add(terms)
                    .filter(|n| *n <= self.limits.expansion_steps)
                    .ok_or(FormulaError::Limit("TLC planning work"))?;
                self.steps += terms;
                if total == 1 {
                    return self.junction(true, columns.into_iter().flatten().collect());
                }
                let mut branches = Vec::with_capacity(total);
                for position in 0..total {
                    let mut stride = total;
                    let mut children = Vec::with_capacity(columns.len());
                    for column in &columns {
                        if column.is_empty() {
                            return Err(FormulaError::Reference("empty DNF factor".into()));
                        }
                        stride /= column.len();
                        children.push(column[(position / stride) % column.len()]);
                    }
                    branches.push(self.junction(true, children)?);
                }
                self.junction(false, branches)
            }
            // LNAll/LNEven do not recurse in toDNF. Nested temporal boolean
            // bodies retain their syntactic shape for tableau construction.
            _ => Ok(i),
        }
    }

    fn flatten(&mut self, i: usize, depth: usize) -> Result<usize, FormulaError> {
        self.step(depth)?;
        match self.nodes[i].clone() {
            Node::And(cs) | Node::Or(cs) => {
                let and = matches!(self.nodes[i], Node::And(_));
                let children = cs
                    .into_iter()
                    .map(|c| self.flatten(c, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                if children.len() == 1 {
                    Ok(children[0])
                } else {
                    self.junction(and, children)
                }
            }
            Node::Not(c) | Node::Always(c) | Node::Eventually(c) => {
                let child = self.flatten(c, depth + 1)?;
                let n = match self.nodes[i] {
                    Node::Not(_) => Node::Not(child),
                    Node::Always(_) => Node::Always(child),
                    _ => Node::Eventually(child),
                };
                self.push(n)
            }
            _ => Ok(i),
        }
    }

    fn level_action(&mut self, i: usize, depth: usize) -> Result<(u8, bool), FormulaError> {
        self.step(depth)?;
        match self.nodes[i].clone() {
            Node::Bool(_) => Ok((0, false)),
            Node::Atom(a) => Ok(if a.kind == AtomKind::State {
                (1, false)
            } else {
                (2, true)
            }),
            Node::Not(c) => self.level_action(c, depth + 1),
            Node::Always(c) | Node::Eventually(c) => Ok((3, self.level_action(c, depth + 1)?.1)),
            Node::And(cs) | Node::Or(cs) => {
                let mut level = 0;
                let mut action = false;
                for c in cs {
                    let (l, a) = self.level_action(c, depth + 1)?;
                    level = level.max(l);
                    action |= a;
                }
                Ok((level, action))
            }
        }
    }

    fn same(&mut self, a: usize, b: usize, depth: usize) -> Result<bool, FormulaError> {
        self.step(depth)?;
        if a == b {
            return Ok(true);
        }
        match (self.nodes[a].clone(), self.nodes[b].clone()) {
            (Node::Bool(a), Node::Bool(b)) => Ok(a == b),
            (Node::Atom(a), Node::Atom(b)) => Ok(a == b),
            (Node::Not(a), Node::Not(b))
            | (Node::Always(a), Node::Always(b))
            | (Node::Eventually(a), Node::Eventually(b)) => self.same(a, b, depth + 1),
            (Node::And(a), Node::And(b)) | (Node::Or(a), Node::Or(b)) => {
                self.same_list(&a, &b, depth + 1)
            }
            _ => Ok(false),
        }
    }
    fn same_list(&mut self, a: &[usize], b: &[usize], depth: usize) -> Result<bool, FormulaError> {
        self.step(depth)?;
        if a.len() != b.len() {
            return Ok(false);
        }
        for (&a, &b) in a.iter().zip(b) {
            if !self.same(a, b, depth + 1)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn bin(&mut self, i: usize, bin: &mut Vec<usize>) -> Result<usize, FormulaError> {
        for (index, &other) in bin.iter().enumerate() {
            if self.same(i, other, 0)? {
                return Ok(index);
            }
        }
        let index = bin.len();
        bin.push(i);
        Ok(index)
    }

    fn groups(&mut self, dnf: usize) -> Result<Vec<OrderOfSolution>, ProjectionError> {
        if self.nodes[dnf] == Node::Bool(false) {
            return Ok(Vec::new());
        }
        let branches = if let Node::Or(cs) = &self.nodes[dnf] {
            cs.clone()
        } else {
            vec![dnf]
        };
        let mut groups: Vec<OrderOfSolution> = Vec::new();
        for branch in branches {
            let branch = self.flatten(branch, 0)?;
            let children = if let Node::And(cs) = &self.nodes[branch] {
                cs.clone()
            } else {
                vec![branch]
            };
            let (mut ea, mut ae_state, mut ae_action, mut temporal) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for c in children {
                if let Node::Eventually(a) = self.nodes[c]
                    && let Node::Always(body) = self.nodes[a]
                    && self.level_action(body, 0)?.0 < 3
                {
                    ea.push(body);
                    continue;
                }
                if let Some(body) = self.ae_body(c) {
                    let level = self.level_action(body, 0)?.0;
                    if level <= 1 {
                        ae_state.push(body);
                        continue;
                    }
                    if level == 2 {
                        ae_action.push(body);
                        continue;
                    }
                }
                if self.level_action(c, 0)?.1 {
                    return Err(ProjectionError::ActionInTemporal);
                }
                temporal.push(c);
            }
            let mut found = None;
            for (index, group) in groups.iter().enumerate() {
                if self.same_list(&temporal, &group.temporal, 0)? {
                    found = Some(index);
                    break;
                }
            }
            let index = found.unwrap_or(groups.len());
            if found.is_none() {
                groups.push(OrderOfSolution {
                    temporal,
                    check_state: Vec::new(),
                    check_action: Vec::new(),
                    pems: Vec::new(),
                });
            }
            let group = &mut groups[index];
            // Match processLiveness's bin calls: AEAction, AEState, EAAction.
            let ae_action = ae_action
                .into_iter()
                .map(|c| self.bin(c, &mut group.check_action))
                .collect::<Result<Vec<_>, _>>()?;
            let ae_state = ae_state
                .into_iter()
                .map(|c| self.bin(c, &mut group.check_state))
                .collect::<Result<Vec<_>, _>>()?;
            let ea_action = ea
                .into_iter()
                .map(|c| self.bin(c, &mut group.check_action))
                .collect::<Result<Vec<_>, _>>()?;
            group.pems.push(PossibleErrorModel {
                ae_action,
                ae_state,
                ea_action,
            });
        }
        Ok(groups)
    }
}

/// Combines, tags, normalizes and groups pure supplied temporal formulas using
/// the pinned TLC transformation order. This returns a planning projection,
/// never a model-checking result. Retained formula-arena nodes/references and
/// transformation work consume the existing formula/expansion budgets. Group
/// metadata is bounded by charged DNF branches/conjuncts, rather than a separate
/// aggregate byte budget. No resource defaults are increased.
///
/// ```
/// use dive_liveness::formula::{Atom, AtomKind, Formula, Limits, Node};
/// use dive_liveness::tlc_plan::{process, Property};
/// let property = Formula::new(vec![
///     Node::Atom(Atom {kind: AtomKind::State, index: 0}),
///     Node::Eventually(0), Node::Always(1)], 2)?;
/// let projection = process(&[], &[Property {name: "Live", formula: &property}],
///     Limits::default())?;
/// assert_eq!(projection.properties, ["Live"]);
/// assert_eq!(projection.orders[0].pems[0].ea_action, [0]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// Explicit depth, retained-term, DNF cardinality or work bounds, and TLC's
/// unsupported action-in-temporal formula format. Binding/effect refusals belong
/// to [`crate::plan::lower`] and cannot be bypassed through a model callback here.
pub fn process(
    fairness: &[&Formula],
    properties: &[Property<'_>],
    limits: Limits,
) -> Result<Projection, ProjectionError> {
    let input_terms = fairness
        .iter()
        .chain(properties.iter().map(|p| &p.formula))
        .try_fold(0usize, |n, f| {
            n.checked_add(f.terms())
                .ok_or(FormulaError::Limit("TLC planning input terms"))
        })?;
    if input_terms > limits.formula_nodes {
        return Err(FormulaError::Limit("TLC planning input terms").into());
    }
    let mut b = Build {
        nodes: Vec::new(),
        occurrences: Vec::new(),
        shared_atoms: BTreeMap::new(),
        terms: 0,
        steps: 0,
        limits,
    };
    let mut conjuncts = Vec::new();
    for (input, f) in fairness.iter().enumerate() {
        let i = b.import(f, f.root(), input, 0)?;
        conjuncts.push(i);
    }
    let mut checks = Vec::new();
    for (index, p) in properties.iter().enumerate() {
        let i = b.import(p.formula, p.formula.root(), fairness.len() + index, 0)?;
        checks.push(b.push(Node::Not(i))?);
    }
    if !checks.is_empty() {
        let i = if checks.len() == 1 {
            checks[0]
        } else {
            b.junction(false, checks)?
        };
        conjuncts.push(i);
    }
    let tagged = if conjuncts.is_empty() {
        None
    } else if fairness.is_empty() {
        Some(conjuncts[0])
    } else {
        Some(b.junction(true, conjuncts)?)
    };
    let pushed = tagged.map(|i| b.negation(i, false, 0)).transpose()?;
    let simplified = pushed.map(|i| b.simplify(i, 0)).transpose()?;
    let dnf = simplified.map(|i| b.dnf(i, 0)).transpose()?;
    let orders = dnf.map(|i| b.groups(i)).transpose()?.unwrap_or_default();
    Ok(Projection {
        arena: b.nodes,
        properties: properties.iter().map(|p| p.name.to_owned()).collect(),
        occurrences: b.occurrences,
        tagged,
        pushed,
        simplified,
        dnf,
        orders,
    })
}

/// A bound normalization projection with the original predicate tables.
#[derive(Clone, Debug)]
pub struct BoundProjection {
    /// Normalized occurrences and ordered pre-tableau groups.
    pub projection: Projection,
    /// Fairness plans followed by property plans, preserving configured order.
    /// Each occurrence's `input` selects a plan and `atom.index` its predicate.
    pub inputs: Vec<crate::plan::Plan>,
    /// Original configured PROPERTY identities and retained predicates, before
    /// role splitting. These are not replaced by helper/role labels or DNF
    /// groups, and are not certified merely by successful projection.
    pub configured_properties: Vec<dive_model::Named>,
}

/// A model-binding/purity refusal or a bounded normalization failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelPlanningError {
    /// Existing planner diagnostic, with original source context.
    Bound(crate::plan::PlanningError),
    /// Normalization/grouping failure; no model evaluation is performed here.
    Projection {
        /// Bounded structural diagnostic, including named classifier refusals.
        error: ProjectionError,
        /// First configured conjunct's request location, not an invented
        /// failing operand. Absent only for a model with no temporal inputs.
        request: Option<Box<crate::plan::Source>>,
    },
}
impl ModelPlanningError {
    /// Stable diagnostic code from the original binding or projection stage.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Bound(e) => e.code(),
            Self::Projection { error, .. } => error.code(),
        }
    }
}
impl std::fmt::Display for ModelPlanningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bound(e) => e.fmt(f),
            Self::Projection { error, request } => {
                write!(f, "{}: {error}", error.code())?;
                if let Some(source) = request {
                    write!(f, " (requested model formula at {})", source.location)?;
                }
                Ok(())
            }
        }
    }
}
impl std::error::Error for ModelPlanningError {}

/// Lowers every configured fairness conjunct and temporal property through the
/// existing pure bound planner, then projects TLC's normalization/grouping.
/// No opaque operator, live checker lookup, or unsupported captured scope is
/// accepted merely because normalization succeeds. This remains a diagnostic
/// planning API; strict integrated liveness execution is still unsupported.
///
/// # Errors
/// Existing source-bearing purity/scope/argument/evaluation refusals; the
/// existing per-predicate lowering budgets; aggregate input, normalization,
/// depth, retained-term and DNF/work bounds. No resource default is increased.
pub fn lower_model(
    model: &dive_model::Model,
    limits: Limits,
) -> Result<BoundProjection, ModelPlanningError> {
    let failure = |error| ModelPlanningError::Projection {
        error,
        request: model
            .temporals
            .iter()
            .chain(&model.implied_temporals)
            .next()
            .map(|n| Box::new(crate::plan::Source::new(model, n.pred.expr))),
    };
    let count = model
        .temporals
        .len()
        .checked_add(model.implied_temporals.len())
        .filter(|n| {
            model
                .configured_properties
                .len()
                .checked_add(*n)
                .is_some_and(|total| total <= limits.formula_nodes)
        })
        .filter(|n| *n <= limits.formula_nodes)
        .ok_or_else(|| failure(FormulaError::Limit("TLC planning input count").into()))?;
    let mut inputs = Vec::with_capacity(count);
    let mut terms = 0usize;
    for named in model.temporals.iter().chain(&model.implied_temporals) {
        let plan = crate::plan::lower_tlc(model, &named.pred, limits)
            .map_err(ModelPlanningError::Bound)?;
        terms = terms
            .checked_add(plan.formula.terms())
            .filter(|n| *n <= limits.formula_nodes)
            .ok_or_else(|| failure(FormulaError::Limit("TLC planning input terms").into()))?;
        inputs.push(plan);
    }
    let fairness = inputs[..model.temporals.len()]
        .iter()
        .map(|p| &p.formula)
        .collect::<Vec<_>>();
    let properties = inputs[model.temporals.len()..]
        .iter()
        .zip(&model.implied_temporals)
        .map(|(p, n)| Property {
            name: &n.name,
            formula: &p.formula,
        })
        .collect::<Vec<_>>();
    let projection = process(&fairness, &properties, limits).map_err(failure)?;
    Ok(BoundProjection {
        projection,
        inputs,
        configured_properties: model.configured_properties.clone(),
    })
}
