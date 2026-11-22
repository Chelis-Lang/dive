//! Bounded boolean/always/eventually formula tableaux.
//!
//! This is a language kernel, independent of bound models and TLC's tableau
//! numbering. Atoms are supplied state or action predicates. Each `Eventually`
//! creates a Buchi promise; accepting runs must fulfill each infinitely often.
//! Model lowering, fairness expansion and TLC-exact planning remain separate.

use std::collections::{BTreeMap, BTreeSet};

mod recurrence;
mod solution;
pub(crate) use recurrence::Recurrence;
pub(crate) use solution::{
    CycleConditions, FactoredSolutions, SolutionGroup, StreettPair, decompose, streett,
};

/// The evaluation domain of an atomic predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomKind {
    /// Evaluated on one state.
    State,
    /// Evaluated on the current and following states.
    Action,
}

/// An index into a caller's predicate table, distinguished by evaluation domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Atom {
    /// Whether the predicate reads one state or a pair.
    pub kind: AtomKind,
    /// The caller's predicate index.
    pub index: usize,
}

/// A node of a flat, acyclic temporal formula. Child indices must precede
/// their parent; this prevents cycles and avoids recursive ownership/drop.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Node {
    /// A boolean constant.
    Bool(bool),
    /// A supplied predicate.
    Atom(Atom),
    /// Logical negation.
    Not(usize),
    /// Conjunction; the empty conjunction is true.
    And(Vec<usize>),
    /// Disjunction; the empty disjunction is false.
    Or(Vec<usize>),
    /// Always, `[]`.
    Always(usize),
    /// Eventually, `<>`.
    Eventually(usize),
}

/// A checked flat formula. No formula evaluation or model binding is performed
/// by construction; callers lower implication, leads-to and fairness explicitly.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Formula {
    nodes: Vec<Node>,
    root: usize,
}

/// Malformed formulas or an explicit construction resource bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormulaError {
    /// A root or child reference is invalid.
    Reference(String),
    /// A named construction limit was reached. This is never an empty language.
    Limit(&'static str),
}

impl std::fmt::Display for FormulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reference(s) => write!(f, "invalid temporal formula: {s}"),
            Self::Limit(s) => write!(f, "temporal formula resource limit: {s}"),
        }
    }
}
impl std::error::Error for FormulaError {}

impl Formula {
    /// Returns the checked arena in construction order.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Returns the root index in the checked arena.
    #[must_use]
    pub fn root(&self) -> usize {
        self.root
    }

    pub(crate) fn terms(&self) -> usize {
        self.nodes.iter().fold(self.nodes.len(), |count, n| {
            let children = match n {
                Node::Bool(_) | Node::Atom(_) => 0,
                Node::Not(_) | Node::Always(_) | Node::Eventually(_) => 1,
                Node::And(cs) | Node::Or(cs) => cs.len(),
            };
            count.saturating_add(children)
        })
    }
    /// Directly evaluates the formula on an ultimately periodic word, without
    /// a tableau or product search. Each row supplies atom truth for a word
    /// position, separately for state and action domains. Action rows include
    /// the truth on each outgoing (closing) edge.
    ///
    /// ```
    /// use dive_liveness::formula::{Atom, AtomKind, Formula, Node};
    /// let f = Formula::new(vec![Node::Atom(Atom {kind: AtomKind::State,index: 0}),
    ///     Node::Eventually(0)], 1)?;
    /// let states = vec![vec![true],vec![false]];
    /// let actions = vec![vec![],vec![]];
    /// assert_eq!(f.evaluate_word(&states,&actions,1,4)?, vec![true,false]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// Empty/malformed word, missing atom values or the explicit cell bound.
    pub fn evaluate_word(
        &self,
        states: &[Vec<bool>],
        actions: &[Vec<bool>],
        back_to: usize,
        max_cells: usize,
    ) -> Result<Vec<bool>, FormulaError> {
        if states.is_empty() || states.len() != actions.len() || back_to >= states.len() {
            return Err(FormulaError::Reference("invalid periodic word".into()));
        }
        states
            .len()
            .checked_mul(self.nodes.len())
            .filter(|n| *n <= max_cells)
            .ok_or(FormulaError::Limit("word truth cells"))?;
        let mut truth: Vec<Vec<bool>> = Vec::new();
        truth
            .try_reserve_exact(self.nodes.len())
            .map_err(|_| FormulaError::Limit("word truth allocation"))?;
        for node in &self.nodes {
            word_poll()?;
            let mut values = Vec::new();
            values
                .try_reserve_exact(states.len())
                .map_err(|_| FormulaError::Limit("word truth allocation"))?;
            match node {
                Node::Always(child) | Node::Eventually(child) => {
                    let all = matches!(node, Node::Always(_));
                    for chunk in truth[*child].chunks(1024) {
                        word_poll()?;
                        values.extend_from_slice(chunk);
                    }
                    let mut cycle = all;
                    for (pos, &value) in values.iter().enumerate().skip(back_to) {
                        if pos.is_multiple_of(1024) {
                            word_poll()?;
                        }
                        if all {
                            cycle &= value;
                        } else {
                            cycle |= value;
                        }
                    }
                    for chunk in values[back_to..].chunks_mut(1024) {
                        word_poll()?;
                        chunk.fill(cycle);
                    }
                    for pos in (0..back_to).rev() {
                        if pos.is_multiple_of(1024) {
                            word_poll()?;
                        }
                        values[pos] = if all {
                            truth[*child][pos] && values[pos + 1]
                        } else {
                            truth[*child][pos] || values[pos + 1]
                        };
                    }
                }
                _ => {
                    for pos in 0..states.len() {
                        if pos.is_multiple_of(1024) {
                            word_poll()?;
                        }
                        values.push(match node {
                            Node::Bool(value) => *value,
                            Node::Atom(atom) => {
                                let rows = if atom.kind == AtomKind::State {
                                    states
                                } else {
                                    actions
                                };
                                rows[pos].get(atom.index).copied().ok_or_else(|| {
                                    FormulaError::Reference(format!(
                                        "missing word atom {}",
                                        atom.index
                                    ))
                                })?
                            }
                            Node::Not(child) => !truth[*child][pos],
                            Node::And(children) => children.iter().all(|child| truth[*child][pos]),
                            Node::Or(children) => children.iter().any(|child| truth[*child][pos]),
                            // Temporal nodes use the separately checked branch above.
                            Node::Always(_) | Node::Eventually(_) => unreachable_word_node()?,
                        });
                    }
                }
            }
            truth.push(values);
        }
        // Transfer the selected row rather than duplicating a potentially long word.
        Ok(std::mem::take(&mut truth[self.root]))
    }

    /// Checks the root and the strict topological order of every child.
    ///
    /// # Errors
    /// A missing root or a child that does not precede its parent.
    pub fn new(nodes: Vec<Node>, root: usize) -> Result<Self, FormulaError> {
        if root >= nodes.len() {
            return Err(FormulaError::Reference("root outside node table".into()));
        }
        for (i, n) in nodes.iter().enumerate() {
            let valid = match n {
                Node::Bool(_) | Node::Atom(_) => true,
                Node::Not(c) | Node::Always(c) | Node::Eventually(c) => *c < i,
                Node::And(cs) | Node::Or(cs) => cs.iter().all(|c| *c < i),
            };
            if !valid {
                return Err(FormulaError::Reference(format!(
                    "a child of node {i} does not precede it"
                )));
            }
        }
        Ok(Self { nodes, root })
    }
}

fn word_poll() -> Result<(), FormulaError> {
    if crate::budget::exhausted() {
        Err(FormulaError::Limit(crate::budget::EXHAUSTED))
    } else {
        Ok(())
    }
}

fn unreachable_word_node() -> Result<bool, FormulaError> {
    Err(FormulaError::Reference(
        "unexpected temporal word node".into(),
    ))
}

/// Explicit limits on formula normalization and tableau construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum normalized formula nodes and input nodes plus child references.
    pub formula_nodes: usize,
    /// Maximum tableau nodes and simultaneously pending expansion alternatives.
    pub tableau_nodes: usize,
    /// Maximum worklist items processed across all state expansions.
    pub expansion_steps: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            formula_nodes: 4096,
            tableau_nodes: 4096,
            expansion_steps: 1_000_000,
        }
    }
}

/// A predicate and the truth value required at a tableau state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Literal {
    /// The caller's state or action predicate.
    pub atom: Atom,
    /// Its required truth value.
    pub positive: bool,
}

/// A state in a generalized Buchi tableau.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableauNode {
    /// Required atomic values, sorted by atom. Contradictory states are absent.
    pub literals: Vec<Literal>,
    /// Permitted following tableau states, in deterministic construction order.
    pub successors: Vec<usize>,
    /// One bit per eventuality promise: this node discharges it or does not
    /// request it. Every promise must recur on an accepting infinite run.
    pub fulfilled: Vec<bool>,
}

/// A generalized Buchi automaton for a supplied formula. A node's literals
/// constrain the current word position. An accepting path starts in `roots`
/// and visits `fulfilled[p]` infinitely often for every promise `p`.
/// No model-checking verdict is implied by successful construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tableau {
    /// All reachable obligation states.
    pub nodes: Vec<TableauNode>,
    /// Initial alternatives, in source expansion order.
    pub roots: Vec<usize>,
    /// Number of eventuality acceptance conditions.
    pub promise_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Nnf {
    Bool(bool),
    Lit(Literal),
    And(Vec<usize>),
    Or(Vec<usize>),
    Always(usize),
    Eventually(usize),
}

struct Normalizer<'a> {
    formula: &'a Formula,
    limit: usize,
    nodes: Vec<Nnf>,
    interned: BTreeMap<Nnf, usize>,
    memo: BTreeMap<(usize, bool), usize>,
}

impl Normalizer<'_> {
    fn intern(&mut self, n: Nnf) -> Result<usize, FormulaError> {
        if let Some(&i) = self.interned.get(&n) {
            return Ok(i);
        }
        if self.nodes.len() >= self.limit {
            return Err(FormulaError::Limit("formula nodes"));
        }
        let i = self.nodes.len();
        self.interned.insert(n.clone(), i);
        self.nodes.push(n);
        Ok(i)
    }

    fn disjunction(&mut self, children: Vec<usize>) -> Result<Nnf, FormulaError> {
        if children.len() > 1 {
            let recurring = children
                .iter()
                .map(|&i| {
                    if let Nnf::Always(c) = self.nodes[i]
                        && let Nnf::Eventually(inner) = self.nodes[c]
                    {
                        Some(inner)
                    } else {
                        None
                    }
                })
                .collect::<Option<Vec<_>>>();
            if let Some(recurring) = recurring {
                // On an infinite word, at least one of finitely many
                // predicates holds infinitely often iff their disjunction
                // holds infinitely often. Preserve child order and atom ids;
                // no bound-predicate equivalence or context merging is used.
                let union = self.intern(Nnf::Or(recurring))?;
                let eventually = self.intern(Nnf::Eventually(union))?;
                return Ok(Nnf::Always(eventually));
            }
        }
        Ok(Nnf::Or(children))
    }

    fn normalize(&mut self, i: usize, negative: bool, depth: usize) -> Result<usize, FormulaError> {
        if crate::budget::exhausted() {
            return Err(FormulaError::Limit(crate::budget::EXHAUSTED));
        }
        if depth > 256 {
            return Err(FormulaError::Limit("normalization depth"));
        }
        if let Some(&n) = self.memo.get(&(i, negative)) {
            return Ok(n);
        }
        let n = match &self.formula.nodes[i] {
            Node::Bool(b) => Nnf::Bool(*b != negative),
            Node::Atom(atom) => Nnf::Lit(Literal {
                atom: *atom,
                positive: !negative,
            }),
            Node::Not(c) => {
                let n = self.normalize(*c, !negative, depth + 1)?;
                self.memo.insert((i, negative), n);
                return Ok(n);
            }
            Node::And(cs) | Node::Or(cs) => {
                let and = matches!(self.formula.nodes[i], Node::And(_)) != negative;
                let children = cs
                    .iter()
                    .map(|c| self.normalize(*c, negative, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                if and {
                    Nnf::And(children)
                } else {
                    self.disjunction(children)?
                }
            }
            Node::Always(c) | Node::Eventually(c) => {
                let always = matches!(self.formula.nodes[i], Node::Always(_)) != negative;
                let c = self.normalize(*c, negative, depth + 1)?;
                if always {
                    Nnf::Always(c)
                } else {
                    Nnf::Eventually(c)
                }
            }
        };
        let n = self.intern(n)?;
        self.memo.insert((i, negative), n);
        Ok(n)
    }
}

#[derive(Clone, Default)]
struct Work {
    pending: Vec<usize>,
    old: BTreeSet<usize>,
    next: BTreeSet<usize>,
    literals: BTreeSet<Literal>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    literals: Vec<Literal>,
    next: Vec<usize>,
    fulfilled: Vec<bool>,
}

struct Builder {
    nnf: Vec<Nnf>,
    promises: Vec<(usize, usize)>,
    limits: Limits,
    steps: usize,
    keys: Vec<Key>,
    interned: BTreeMap<Key, usize>,
    nodes: Vec<TableauNode>,
}

impl Builder {
    fn push_work(&self, work: &mut Vec<Work>, branch: Work) -> Result<(), FormulaError> {
        if work.len() >= self.limits.tableau_nodes {
            return Err(FormulaError::Limit("expansion frontier"));
        }
        work.push(branch);
        Ok(())
    }

    fn intern(&mut self, key: Key) -> Result<usize, FormulaError> {
        if let Some(&i) = self.interned.get(&key) {
            return Ok(i);
        }
        if self.nodes.len() >= self.limits.tableau_nodes {
            return Err(FormulaError::Limit("tableau nodes"));
        }
        let i = self.nodes.len();
        self.nodes.push(TableauNode {
            literals: key.literals.clone(),
            fulfilled: key.fulfilled.clone(),
            successors: Vec::new(),
        });
        self.keys.push(key.clone());
        self.interned.insert(key, i);
        Ok(i)
    }

    fn expand(&mut self, pending: Vec<usize>) -> Result<Vec<usize>, FormulaError> {
        let mut work = vec![Work {
            pending: pending.into_iter().rev().collect(),
            ..Work::default()
        }];
        let mut roots = Vec::new();
        while let Some(mut w) = work.pop() {
            if crate::budget::exhausted() {
                return Err(FormulaError::Limit(crate::budget::EXHAUSTED));
            }
            self.steps = self
                .steps
                .checked_add(1)
                .ok_or(FormulaError::Limit("expansion steps"))?;
            if self.steps > self.limits.expansion_steps {
                return Err(FormulaError::Limit("expansion steps"));
            }
            let Some(i) = w.pending.pop() else {
                let key = Key {
                    literals: w.literals.into_iter().collect(),
                    next: w.next.into_iter().collect(),
                    fulfilled: self
                        .promises
                        .iter()
                        .map(|(p, child)| !w.old.contains(p) || w.old.contains(child))
                        .collect(),
                };
                let n = self.intern(key)?;
                if !roots.contains(&n) {
                    roots.push(n);
                }
                continue;
            };
            if !w.old.insert(i) {
                self.push_work(&mut work, w)?;
                continue;
            }
            match &self.nnf[i] {
                Nnf::Bool(false) => continue,
                Nnf::Bool(true) => {}
                Nnf::Lit(lit) => {
                    if w.literals.contains(&Literal {
                        atom: lit.atom,
                        positive: !lit.positive,
                    }) {
                        continue;
                    }
                    w.literals.insert(*lit);
                }
                Nnf::And(cs) => w.pending.extend(cs.iter().rev()),
                Nnf::Or(cs) => {
                    for &c in cs.iter().rev() {
                        let mut branch = w.clone();
                        branch.pending.push(c);
                        self.push_work(&mut work, branch)?;
                    }
                    continue;
                }
                Nnf::Always(c) => {
                    w.next.insert(i);
                    w.pending.push(*c);
                }
                Nnf::Eventually(c) => {
                    let mut later = w.clone();
                    later.next.insert(i);
                    self.push_work(&mut work, later)?;
                    w.pending.push(*c);
                }
            }
            self.push_work(&mut work, w)?;
        }
        Ok(roots)
    }
}

/// Normalizes negations and constructs the formula's generalized Buchi tableau.
/// Always expands to its child now and itself later; eventually branches into
/// fulfillment now or deferral, with an infinitely-recurring fulfillment promise.
/// Atoms retain their state/action domains. This is dive's own language kernel;
/// it does not claim TLC's node numbering or error/evaluation order.
///
/// # Errors
/// Explicit formula/depth/tableau/expansion limits. A limit never yields an empty
/// tableau that could be mistaken for a proof of absence of violations.
pub fn compile(formula: &Formula, limits: Limits) -> Result<Tableau, FormulaError> {
    if formula.terms() > limits.formula_nodes {
        return Err(FormulaError::Limit("formula nodes"));
    }
    compile_residual(formula, limits)
}

// Decomposition has already checked the original input's terms. Its synthetic
// explicit atomic negations must not be charged as a second, larger user input.
// The normalized-node, expansion, frontier and tableau limits still run here.
pub(crate) fn compile_residual(formula: &Formula, limits: Limits) -> Result<Tableau, FormulaError> {
    let mut norm = Normalizer {
        formula,
        limit: limits.formula_nodes,
        nodes: Vec::new(),
        interned: BTreeMap::new(),
        memo: BTreeMap::new(),
    };
    let root = norm.normalize(formula.root, false, 0)?;
    // Normalization can replace a disjunction of recurrence formulas. Only
    // promises reachable from the resulting root constrain the language.
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(i) = pending.pop() {
        if !reachable.insert(i) {
            continue;
        }
        match &norm.nodes[i] {
            Nnf::And(cs) | Nnf::Or(cs) => pending.extend(cs),
            Nnf::Always(c) | Nnf::Eventually(c) => pending.push(*c),
            Nnf::Bool(_) | Nnf::Lit(_) => {}
        }
    }
    let promises = norm
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            if let Nnf::Eventually(c) = n
                && reachable.contains(&i)
            {
                Some((i, *c))
            } else {
                None
            }
        })
        .collect();
    let mut b = Builder {
        nnf: norm.nodes,
        promises,
        limits,
        steps: 0,
        keys: Vec::new(),
        interned: BTreeMap::new(),
        nodes: Vec::new(),
    };
    let roots = b.expand(vec![root])?;
    // Expansion starts with empty current-state facts and depends only on the
    // pending obligations. Distinct nodes can have different literals/promise
    // bits but identical next obligations; re-expanding those cannot add nodes
    // or change successor order. Keep a reference to the first finished result
    // rather than retaining another adjacency list for every cache entry.
    let mut expanded = BTreeMap::from([(vec![root], None)]);
    let mut cursor = 0;
    while cursor < b.nodes.len() {
        let pending = &b.keys[cursor].next;
        b.nodes[cursor].successors = if let Some(previous) = expanded.get(pending) {
            previous.map_or_else(|| roots.clone(), |i: usize| b.nodes[i].successors.clone())
        } else {
            let pending = pending.clone();
            let successors = b.expand(pending.clone())?;
            expanded.insert(pending, Some(cursor));
            successors
        };
        cursor += 1;
    }
    Ok(Tableau {
        promise_count: b.promises.len(),
        nodes: b.nodes,
        roots,
    })
}

#[cfg(test)]
mod tableau_tests;

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn word_truth_honors_exact_cells_and_the_actual_owner_budget() -> Result<(), FormulaError> {
        let formula = Formula::new(vec![Node::Bool(true), Node::Always(0)], 1)?;
        let rows = vec![vec![]; 65];
        assert!(matches!(
            formula.evaluate_word(&rows, &rows, 0, 129),
            Err(FormulaError::Limit("word truth cells"))
        ));
        assert_eq!(formula.evaluate_word(&rows, &rows, 0, 130)?, vec![true; 65]);
        {
            let watchdog = crate::budget::expired_for_test();
            let _scope = crate::budget::enter(watchdog.budget());
            assert!(matches!(
                formula.evaluate_word(&rows, &rows, 0, usize::MAX),
                Err(FormulaError::Limit(crate::budget::EXHAUSTED))
            ));
        }
        assert_eq!(
            formula.evaluate_word(&rows, &rows, 0, usize::MAX)?,
            vec![true; 65]
        );
        Ok(())
    }
}
