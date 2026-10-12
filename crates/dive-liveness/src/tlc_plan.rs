//! TLC's liveness formula classifier, for `--compat=tlc`.
//!
//! TLC's `Liveness.processLiveness` negates the configured properties,
//! conjoins them with fairness, pushes negation inward, simplifies and
//! converts to disjunctive normal form. It then refuses two shapes that dive's
//! general tableau checks without trouble:
//!
//! - an action-level predicate inside a temporal formula outside the
//!   `[]<>A` and `<>[]A` forms (TLC error 2214), and
//! - a DNF whose branch count overflows a Java `int` (TLC error 2213).
//!
//! [`classify`] reproduces only that verdict. It builds no tableau and
//! keeps no TLC grouping, tagging or ordering: dive checks liveness with
//! [`crate::formula::compile`] and [`crate::product`].

use crate::formula::{AtomKind, Formula, FormulaError, Limits, Node};

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

    fn import(&mut self, f: &Formula, i: usize, depth: usize) -> Result<usize, FormulaError> {
        self.step(depth)?;
        let node = match &f.nodes()[i] {
            Node::Atom(a) => Node::Atom(*a),
            Node::Bool(b) => Node::Bool(*b),
            Node::Not(c) => Node::Not(self.import(f, *c, depth + 1)?),
            Node::Always(c) => Node::Always(self.import(f, *c, depth + 1)?),
            Node::Eventually(c) => Node::Eventually(self.import(f, *c, depth + 1)?),
            Node::And(cs) | Node::Or(cs) => {
                self.room(cs.len().saturating_add(1))?;
                let children = cs
                    .iter()
                    .map(|c| self.import(f, *c, depth + 1))
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
                let mut single = Vec::new();
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
                            single.push(c);
                        }
                        Node::And(cs) => {
                            for &c in cs {
                                columns.push(vec![c]);
                                single.push(c);
                            }
                        }
                        _ => {
                            columns.push(vec![c]);
                            single.push(c);
                        }
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
                    // LNConj.toDNF returns new LNConj(nes) for total=1.
                    // Unlike the expanded product, it keeps a singleton
                    // LNDisj factor intact until flattenSingleJunctions.
                    return self.junction(true, single);
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

    /// processLiveness's per-branch EA/AE split. Any other conjunct that
    /// mentions an action is TLC's 2214 refusal.
    fn branches(&mut self, dnf: usize) -> Result<(), ProjectionError> {
        if self.nodes[dnf] == Node::Bool(false) {
            return Ok(());
        }
        let branches = if let Node::Or(cs) = &self.nodes[dnf] {
            cs.clone()
        } else {
            vec![dnf]
        };
        for branch in branches {
            let branch = self.flatten(branch, 0)?;
            let children = if let Node::And(cs) = &self.nodes[branch] {
                cs.clone()
            } else {
                vec![branch]
            };
            for c in children {
                if let Node::Eventually(a) = self.nodes[c]
                    && let Node::Always(body) = self.nodes[a]
                    && self.level_action(body, 0)?.0 < 3
                {
                    continue;
                }
                if let Some(body) = self.ae_body(c)
                    && self.level_action(body, 0)?.0 <= 2
                {
                    continue;
                }
                if self.level_action(c, 0)?.1 {
                    return Err(ProjectionError::ActionInTemporal);
                }
            }
        }
        Ok(())
    }
}

/// Applies TLC's liveness formula classifier to fairness conjuncts and
/// configured properties (each prior to negation), as `--compat=tlc` requires.
/// `Ok` means TLC would go on to check these formulas; it is not a verdict.
///
/// ```
/// use dive_liveness::formula::{Atom, AtomKind, Formula, Limits, Node};
/// use dive_liveness::tlc_plan::{ProjectionError, classify};
/// let state = Atom { kind: AtomKind::State, index: 0 };
/// let action = Atom { kind: AtomKind::Action, index: 1 };
/// // <>[]p is accepted.
/// let live = Formula::new(vec![Node::Atom(state), Node::Always(0), Node::Eventually(1)], 2)?;
/// assert_eq!(classify(&[], &[&live], Limits::default()), Ok(()));
/// // [](p => <>A) puts an action inside a temporal formula: TLC error 2214.
/// let bad = Formula::new(vec![
///     Node::Atom(state), Node::Not(0), Node::Atom(action), Node::Eventually(2),
///     Node::Or(vec![1, 3]), Node::Always(4)], 5)?;
/// let error = classify(&[], &[&bad], Limits::default()).unwrap_err();
/// assert_eq!(error, ProjectionError::ActionInTemporal);
/// assert_eq!(error.tlc_code(), Some(2214));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// TLC's 2214 and 2213 refusals, and dive's own depth, term and work bounds
/// (which carry no TLC code).
pub fn classify(
    fairness: &[&Formula],
    properties: &[&Formula],
    limits: Limits,
) -> Result<(), ProjectionError> {
    let input_terms = fairness.iter().chain(properties).try_fold(0usize, |n, f| {
        n.checked_add(f.terms())
            .ok_or(FormulaError::Limit("TLC planning input terms"))
    })?;
    if input_terms > limits.formula_nodes {
        return Err(FormulaError::Limit("TLC planning input terms").into());
    }
    let mut b = Build {
        nodes: Vec::new(),
        terms: 0,
        steps: 0,
        limits,
    };
    let mut conjuncts = Vec::new();
    for f in fairness {
        conjuncts.push(b.import(f, f.root(), 0)?);
    }
    let mut checks = Vec::new();
    for p in properties {
        let i = b.import(p, p.root(), 0)?;
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
    let tagged = match conjuncts.len() {
        0 => return Ok(()),
        _ if fairness.is_empty() => conjuncts[0],
        _ => b.junction(true, conjuncts)?,
    };
    let pushed = b.negation(tagged, false, 0)?;
    let simplified = b.simplify(pushed, 0)?;
    let dnf = b.dnf(simplified, 0)?;
    b.branches(dnf)
}
