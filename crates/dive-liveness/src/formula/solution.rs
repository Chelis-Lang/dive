//! Separate recurrence and persistence from temporal memory, retaining every
//! Boolean alternative. This follows TLC's `OrderOfSolution` / `PossibleErrorModel`
//! semantics, without reproducing Java grouping or tableau numbering.

use std::collections::{BTreeSet, HashMap};

use super::{Formula, FormulaError, Limits, Nnf, Node, Normalizer, Recurrence};

#[derive(Debug)]
pub(crate) struct CycleConditions {
    /// Each Boolean condition must hold on every recurrent edge. Prefixes
    /// remain unrestricted: these are eventually-always obligations.
    pub always: Vec<Recurrence>,
    /// Each Boolean condition must hold infinitely often. State conditions
    /// read the source of each edge; action conditions include its destination.
    pub recurring: Vec<Recurrence>,
}

#[derive(Debug)]
pub(crate) struct SolutionGroup {
    pub core: Formula,
    pub alternatives: Vec<CycleConditions>,
}

struct Budget {
    remaining: usize,
}

impl Budget {
    fn step(&mut self) -> Result<(), FormulaError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(FormulaError::Limit("solution decomposition work"))?;
        Ok(())
    }
}

fn normalized(formula: &Formula, limits: Limits) -> Result<Formula, FormulaError> {
    let mut normalize = Normalizer {
        formula,
        limit: limits.formula_nodes,
        nodes: Vec::new(),
        interned: std::collections::BTreeMap::new(),
        memo: std::collections::BTreeMap::new(),
    };
    let root = normalize.normalize(formula.root, false, 0)?;
    let mut mapped = vec![0; normalize.nodes.len()];
    let mut nodes = Vec::new();
    for (index, source) in normalize.nodes.iter().enumerate() {
        let node = match source {
            Nnf::Bool(b) => Node::Bool(*b),
            Nnf::Lit(literal) => {
                let atom = Node::Atom(literal.atom);
                if literal.positive {
                    atom
                } else {
                    let child = nodes.len();
                    nodes.push(atom);
                    Node::Not(child)
                }
            }
            Nnf::And(cs) => Node::And(cs.iter().map(|c| mapped[*c]).collect()),
            Nnf::Or(cs) => Node::Or(cs.iter().map(|c| mapped[*c]).collect()),
            Nnf::Always(c) => Node::Always(mapped[*c]),
            Nnf::Eventually(c) => Node::Eventually(mapped[*c]),
        };
        mapped[index] = nodes.len();
        nodes.push(node);
    }
    Formula::new(nodes, mapped[root])
}

fn core(formula: &Formula, roots: &[usize], budget: &mut Budget) -> Result<Formula, FormulaError> {
    let mut seen = BTreeSet::new();
    let mut pending = roots.to_vec();
    while let Some(index) = pending.pop() {
        budget.step()?;
        if !seen.insert(index) {
            continue;
        }
        match &formula.nodes[index] {
            Node::Not(c) | Node::Always(c) | Node::Eventually(c) => pending.push(*c),
            Node::And(cs) | Node::Or(cs) => pending.extend(cs),
            Node::Bool(_) | Node::Atom(_) => {}
        }
    }
    let mut mapped = vec![0; formula.nodes.len()];
    let mut nodes = Vec::new();
    for index in seen {
        budget.step()?;
        let node = match &formula.nodes[index] {
            Node::Bool(b) => Node::Bool(*b),
            Node::Atom(a) => Node::Atom(*a),
            Node::Not(c) => Node::Not(mapped[*c]),
            Node::Always(c) => Node::Always(mapped[*c]),
            Node::Eventually(c) => Node::Eventually(mapped[*c]),
            Node::And(cs) => Node::And(cs.iter().map(|c| mapped[*c]).collect()),
            Node::Or(cs) => Node::Or(cs.iter().map(|c| mapped[*c]).collect()),
        };
        mapped[index] = nodes.len();
        nodes.push(node);
    }
    let root = nodes.len();
    nodes.push(Node::And(roots.iter().map(|r| mapped[*r]).collect()));
    Formula::new(nodes, root)
}

fn classify(formula: &Formula, root: usize) -> Option<(bool, Recurrence)> {
    let (always, body) = match formula.nodes[root] {
        Node::Always(child) => match formula.nodes[child] {
            Node::Eventually(body) => (false, body),
            _ => return None,
        },
        Node::Eventually(child) => match formula.nodes[child] {
            Node::Always(body) => (true, body),
            _ => return None,
        },
        _ => return None,
    };
    // Nested temporal operands retain the general tableau route. This is a
    // semantic split of Boolean conditions, never an admission restriction.
    Recurrence::condition(formula, &[body]).map(|condition| (always, condition))
}

pub(crate) fn decompose(
    formula: &Formula,
    limits: Limits,
) -> Result<Vec<SolutionGroup>, FormulaError> {
    struct Branch {
        pending: Vec<usize>,
        conjuncts: Vec<usize>,
    }
    if formula.terms() > limits.formula_nodes {
        return Err(FormulaError::Limit("formula nodes"));
    }
    let formula = normalized(formula, limits)?;
    let mut budget = Budget {
        remaining: limits.expansion_steps,
    };
    let mut pending = vec![Branch {
        pending: vec![formula.root],
        conjuncts: Vec::new(),
    }];
    let mut groups: Vec<SolutionGroup> = Vec::new();
    // This map is an index only. The ordered group vector retains construction
    // order, independently of randomized hash-table iteration.
    let mut indices: HashMap<Formula, usize> = HashMap::new();
    while let Some(mut branch) = pending.pop() {
        let mut impossible = false;
        while let Some(index) = branch.pending.pop() {
            budget.step()?;
            match &formula.nodes[index] {
                Node::Bool(false) => {
                    impossible = true;
                    break;
                }
                Node::Bool(true) => {}
                Node::And(cs) => branch.pending.extend(cs.iter().rev()),
                Node::Or(cs) => {
                    // All alternatives survive. Sharing a temporal core never
                    // turns disjunction into conjunction of its conditions.
                    for &child in cs.iter().rev() {
                        budget.step()?;
                        let mut next = branch.pending.clone();
                        next.push(child);
                        pending.push(Branch {
                            pending: next,
                            conjuncts: branch.conjuncts.clone(),
                        });
                    }
                    impossible = true;
                    break;
                }
                _ => branch.conjuncts.push(index),
            }
        }
        if impossible {
            continue;
        }
        let mut residual = Vec::new();
        let mut conditions = CycleConditions {
            always: Vec::new(),
            recurring: Vec::new(),
        };
        for index in branch.conjuncts {
            budget.step()?;
            match classify(&formula, index) {
                Some((true, condition)) => conditions.always.push(condition),
                Some((false, condition)) => conditions.recurring.push(condition),
                None => residual.push(index),
            }
        }
        let core = core(&formula, &residual, &mut budget)?;
        if let Some(&index) = indices.get(&core) {
            groups[index].alternatives.push(conditions);
        } else {
            indices.insert(core.clone(), groups.len());
            groups.push(SolutionGroup {
                core,
                alternatives: vec![conditions],
            });
        }
    }
    Ok(groups)
}

#[cfg(test)]
mod tests;
