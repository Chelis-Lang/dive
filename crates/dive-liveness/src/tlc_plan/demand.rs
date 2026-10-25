//! Ordered, residual-free TLC demand with complete original-result coverage.
//!
//! The identity is the original prepared plan and its atom slot, never `ExprId`.
//! All scope and `AtomTest` owners remain in that original prepared plan. The
//! compiler executes no predicate and creates no substitute predicate scopes.

use std::collections::BTreeSet;

use super::Build;
use crate::formula::{Formula, FormulaError, Limits, Node};

#[cfg(test)]
#[path = "demand_tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Boolean {
    Constant(bool),
    Atom(usize),
    Not(usize),
    And(Vec<usize>),
    Or(Vec<usize>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Check {
    nodes: Vec<Boolean>,
    root: usize,
}

impl Check {
    fn copy(build: &mut Build, root: usize) -> Result<Self, FormulaError> {
        fn visit(
            build: &mut Build,
            at: usize,
            depth: usize,
            nodes: &mut Vec<Boolean>,
        ) -> Result<usize, FormulaError> {
            build.step(depth)?;
            let node = match build.nodes[at].clone() {
                Node::Bool(value) => Boolean::Constant(value),
                Node::Atom(atom) => Boolean::Atom(atom.index),
                Node::Not(child) => Boolean::Not(visit(build, child, depth + 1, nodes)?),
                Node::And(children) | Node::Or(children) => {
                    let and = matches!(build.nodes[at], Node::And(_));
                    let children = children
                        .into_iter()
                        .map(|child| visit(build, child, depth + 1, nodes))
                        .collect::<Result<Vec<_>, _>>()?;
                    if and {
                        Boolean::And(children)
                    } else {
                        Boolean::Or(children)
                    }
                }
                Node::Always(_) | Node::Eventually(_) => {
                    return Err(FormulaError::Reference("temporal demand bin".into()));
                }
            };
            let index = nodes.len();
            nodes.push(node);
            Ok(index)
        }
        let mut nodes = Vec::new();
        let root = visit(build, root, 0, &mut nodes)?;
        Ok(Self { nodes, root })
    }

    fn must_read(&self, build: &mut Build) -> Result<BTreeSet<usize>, FormulaError> {
        let mut known: Vec<Option<bool>> = Vec::new();
        let mut must: Vec<BTreeSet<usize>> = Vec::new();
        for node in &self.nodes {
            build.step(0)?;
            let (value, reads) = match node {
                Boolean::Constant(value) => (Some(*value), BTreeSet::new()),
                Boolean::Atom(atom) => (None, BTreeSet::from([*atom])),
                Boolean::Not(child) => (known[*child].map(|value| !value), must[*child].clone()),
                Boolean::And(children) | Boolean::Or(children) => {
                    let and = matches!(node, Boolean::And(_));
                    let mut reads = BTreeSet::new();
                    let mut forced = true;
                    let mut all_known = true;
                    let mut absorbing = false;
                    for &child in children {
                        build.step(0)?;
                        if forced {
                            for &atom in &must[child] {
                                build.step(0)?;
                                reads.insert(atom);
                            }
                        }
                        forced &= known[child] == Some(and);
                        all_known &= known[child].is_some();
                        absorbing |= known[child] == Some(!and);
                    }
                    let value = if absorbing {
                        Some(!and)
                    } else if all_known {
                        Some(and)
                    } else {
                        None
                    };
                    (value, reads)
                }
            };
            known.push(value);
            must.push(reads);
        }
        Ok(must.swap_remove(self.root))
    }

    fn evaluate<E>(
        &self,
        at: usize,
        predicate: &mut impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<bool, E> {
        match &self.nodes[at] {
            Boolean::Constant(value) => Ok(*value),
            Boolean::Atom(atom) => predicate(*atom),
            Boolean::Not(child) => Ok(!self.evaluate(*child, predicate)?),
            Boolean::And(children) => {
                for &child in children {
                    if !self.evaluate(child, predicate)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Boolean::Or(children) => {
                for &child in children {
                    if self.evaluate(child, predicate)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}

/// A slot is the actual original prepared predicate, including scope/AtomTest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) property: usize,
    pub(crate) atom: usize,
}

/// A complete certificate; construction is private and predicates are untouched.
pub(crate) struct Program {
    checks: Vec<Check>,
    pub(crate) slots: Vec<Slot>,
    pub(crate) offsets: Vec<usize>,
    stability: Option<dive_eval::ProducerStability>,
}

#[derive(Clone, Debug)]
pub(crate) struct Certificate {
    _stability: dive_eval::ProducerStability,
}

impl Program {
    pub(crate) fn bind_stability(&mut self, stability: dive_eval::ProducerStability) {
        self.stability = Some(stability);
    }

    pub(crate) fn certificate(&self) -> Option<Certificate> {
        self.stability.clone().map(|stability| Certificate {
            _stability: stability,
        })
    }
    pub(crate) fn evaluate<E>(
        &self,
        mut predicate: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<(), E> {
        // MUST is not an execution order. Each original Boolean bin retains
        // source short circuits; a false bin never skips the next bin.
        for check in &self.checks {
            check.evaluate(check.root, &mut predicate)?;
        }
        Ok(())
    }
}

fn import(
    build: &mut Build,
    formula: &Formula,
    at: usize,
    offset: usize,
    atoms: usize,
    depth: usize,
) -> Result<usize, FormulaError> {
    build.step(depth)?;
    let node = match formula.nodes()[at].clone() {
        Node::Atom(mut atom) => {
            if atom.index >= atoms {
                return Err(FormulaError::Reference("original demand atom slot".into()));
            }
            atom.index = atom
                .index
                .checked_add(offset)
                .ok_or(FormulaError::Limit("demand atoms"))?;
            Node::Atom(atom)
        }
        Node::Bool(value) => Node::Bool(value),
        Node::Not(child) | Node::Always(child) | Node::Eventually(child) => {
            let child = import(build, formula, child, offset, atoms, depth + 1)?;
            match formula.nodes()[at] {
                Node::Not(_) => Node::Not(child),
                Node::Always(_) => Node::Always(child),
                _ => Node::Eventually(child),
            }
        }
        Node::And(children) | Node::Or(children) => {
            let children = children
                .into_iter()
                .map(|child| import(build, formula, child, offset, atoms, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            if matches!(formula.nodes()[at], Node::And(_)) {
                Node::And(children)
            } else {
                Node::Or(children)
            }
        }
    };
    build.push(node)
}

fn unique(checks: &mut Vec<Check>, check: Check) {
    // The postorder copy retains structural equality and original occurrence
    // identity. DNF copies compare equal; unrelated original slots do not.
    if !checks.contains(&check) {
        checks.push(check);
    }
}

/// `formulas` are the actual `lower_violation` products and original atom counts.
/// Their outer conjunction is a producer invariant, not a source-form whitelist.
/// Fairness is imported once; separately recreated fairness scopes in later
/// backend plans cannot be guessed equal and will fail complete MUST coverage.
pub(crate) fn compile(
    formulas: &[(&Formula, usize)],
    fairness: usize,
    limits: Limits,
) -> Result<Option<Program>, FormulaError> {
    if formulas.is_empty() {
        return Ok(None);
    }
    let mut build = Build {
        nodes: Vec::new(),
        terms: 0,
        steps: 0,
        limits,
    };
    let mut slots = Vec::new();
    let mut offsets = Vec::new();
    let mut claims = Vec::new();
    let mut behaviors = Vec::new();
    for (property, &(formula, atoms)) in formulas.iter().enumerate() {
        let offset = slots.len();
        offsets.push(offset);
        for atom in 0..atoms {
            build.step(0)?;
            slots.push(Slot { property, atom });
        }
        let Node::And(parts) = &formula.nodes()[formula.root()] else {
            return Ok(None);
        };
        if parts.len() != fairness + 1 {
            return Ok(None);
        }
        if property == 0 {
            for &part in &parts[..fairness] {
                behaviors.push(import(&mut build, formula, part, offset, atoms, 0)?);
            }
        }
        let Node::Not(claim) = formula.nodes()[parts[fairness]] else {
            return Ok(None);
        };
        let claim = import(&mut build, formula, claim, offset, atoms, 0)?;
        claims.push(build.push(Node::Not(claim))?);
    }
    let claim = if claims.len() == 1 {
        claims[0]
    } else {
        build.junction(false, claims)?
    };
    let root = if behaviors.is_empty() {
        claim
    } else {
        behaviors.push(claim);
        build.junction(true, behaviors)?
    };
    let simplified = build.simplified(root)?;
    let dnf = build.dnf(simplified, 0)?;
    if build.nodes[dnf] == Node::Bool(false) {
        // Pinned TLC has a startup2253 branch before ASSUME/Init. This row
        // consumer does not implement that separate binding-order policy.
        return Ok(None);
    }
    let Some(actions) = ordered_bins(&mut build, dnf)? else {
        return Ok(None);
    };
    let mut must = BTreeSet::new();
    for check in &actions {
        for atom in check.must_read(&mut build)? {
            build.step(0)?;
            must.insert(atom);
        }
    }
    if !(0..slots.len()).all(|atom| must.contains(&atom)) {
        return Ok(None);
    }
    Ok(Some(Program {
        checks: actions,
        slots,
        offsets,
        stability: None,
    }))
}

fn ordered_bins(build: &mut Build, dnf: usize) -> Result<Option<Vec<Check>>, FormulaError> {
    let branches = if let Node::Or(branches) = &build.nodes[dnf] {
        branches.clone()
    } else {
        vec![dnf]
    };
    let mut actions = Vec::new();
    let mut states = Vec::new();
    for branch in branches {
        let branch = build.flatten(branch, 0)?;
        let conjuncts = if let Node::And(children) = &build.nodes[branch] {
            children.clone()
        } else {
            vec![branch]
        };
        let mut eventual_always = Vec::new();
        let mut always_eventual = Vec::new();
        for conjunct in conjuncts {
            if let Node::Eventually(always) = build.nodes[conjunct]
                && let Node::Always(body) = build.nodes[always]
                && build.level_action(body, 0)?.0 < 3
            {
                eventual_always.push(Check::copy(build, body)?);
            } else if let Some(body) = build.ae_body(conjunct)
                && build.level_action(body, 0)?.0 <= 1
            {
                always_eventual.push(Check::copy(build, body)?);
            } else {
                return Ok(None); // Actual residual/tableau or successor dependence.
            }
        }
        // All residuals are absent, so TLC groups all PEMs into one solution.
        for check in always_eventual {
            unique(&mut states, check);
        }
        for check in eventual_always {
            unique(&mut actions, check);
        }
    }
    actions.extend(states);
    Ok(Some(actions))
}
