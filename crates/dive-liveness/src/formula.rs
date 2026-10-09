//! Bounded boolean/always/eventually formula tableaux.
//!
//! This is a language kernel, independent of bound models and TLC's tableau
//! numbering. Atoms are supplied state or action predicates. Each `Eventually`
//! creates a Buchi promise; accepting runs must fulfill each infinitely often.
//! Model lowering, fairness expansion and TLC-exact planning remain separate.

use std::collections::{BTreeMap, BTreeSet};

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
#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

    fn normalize(&mut self, i: usize, negative: bool, depth: usize) -> Result<usize, FormulaError> {
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
                    Nnf::Or(children)
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
    let terms = formula.nodes.iter().fold(formula.nodes.len(), |count, n| {
        let children = match n {
            Node::Bool(_) | Node::Atom(_) => 0,
            Node::Not(_) | Node::Always(_) | Node::Eventually(_) => 1,
            Node::And(cs) | Node::Or(cs) => cs.len(),
        };
        count.saturating_add(children)
    });
    if terms > limits.formula_nodes {
        return Err(FormulaError::Limit("formula nodes"));
    }
    let mut norm = Normalizer {
        formula,
        limit: limits.formula_nodes,
        nodes: Vec::new(),
        interned: BTreeMap::new(),
        memo: BTreeMap::new(),
    };
    let root = norm.normalize(formula.root, false, 0)?;
    let promises = norm
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            if let Nnf::Eventually(c) = n {
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
    let mut cursor = 0;
    while cursor < b.nodes.len() {
        b.nodes[cursor].successors = b.expand(b.keys[cursor].next.clone())?;
        cursor += 1;
    }
    Ok(Tableau {
        promise_count: b.promises.len(),
        nodes: b.nodes,
        roots,
    })
}
