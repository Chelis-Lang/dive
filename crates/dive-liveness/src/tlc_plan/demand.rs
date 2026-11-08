//! Ordered, residual-free TLC demand with complete original-result coverage.
//!
//! The identity is the original prepared plan and its atom slot, never `ExprId`.
//! All scope and `AtomTest` owners remain in that original prepared plan. The
//! compiler executes no predicate and creates no substitute predicate scopes.

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

#[derive(Clone, Debug)]
pub(crate) struct Certificate {
    _stability: dive_eval::ProducerStability,
}

/// Ordered known bins before a single solution's opaque tableau boundary.
/// This can prove a reached error, never complete labels or tableau truth.
pub(crate) struct SourceProgram {
    checks: Vec<Check>,
    pub(crate) slots: Vec<Slot>,
    stability: Option<dive_eval::ProducerStability>,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceCertificate {
    _stability: dive_eval::ProducerStability,
}

impl SourceProgram {
    pub(crate) fn bind_stability(&mut self, stability: dive_eval::ProducerStability) {
        self.stability = Some(stability);
    }

    pub(crate) fn certificate(&self) -> Option<SourceCertificate> {
        self.stability.clone().map(|stability| SourceCertificate {
            _stability: stability,
        })
    }

    /// Executes only actual known ordered bins. Success stops at the opaque
    /// boundary and establishes no complete row, residual or holding result.
    pub(crate) fn evaluate<E>(
        &self,
        mut predicate: impl FnMut(usize) -> Result<bool, E>,
    ) -> Result<(), E> {
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

/// Complete original combined conversion, before any row/runtime certificate.
pub(crate) struct Normalized {
    build: Build,
    dnf: usize,
    slots: Vec<Slot>,
}

/// `formulas` are complete violation envelopes and original atom counts.
/// Their outer conjunction is a producer invariant, not a source whitelist.
/// Fairness is imported once. Startup supplies actual bound temporal roles;
/// the separate row consumer supplies original configured-claim plans.
pub(crate) fn normalize(
    formulas: &[(&Formula, usize)],
    fairness: usize,
    limits: Limits,
) -> Result<Option<Normalized>, FormulaError> {
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
    let mut claims = Vec::new();
    let mut behaviors = Vec::new();
    for (property, &(formula, atoms)) in formulas.iter().enumerate() {
        let offset = slots.len();
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
    Ok(Some(Normalized { build, dnf, slots }))
}

impl Normalized {
    /// Only TLC's syntactic normalization result, never semantic tautology.
    pub(crate) fn zero_solutions(&self) -> bool {
        self.build.nodes[self.dnf] == Node::Bool(false)
    }

    pub(crate) fn into_source_program(self) -> Result<Option<SourceProgram>, FormulaError> {
        if self.zero_solutions() {
            return Ok(None);
        }
        let Self {
            mut build,
            dnf,
            slots,
            ..
        } = self;
        let branches = if let Node::Or(branches) = &build.nodes[dnf] {
            branches.clone()
        } else {
            vec![dnf]
        };
        let [branch] = branches.as_slice() else {
            return Ok(None);
        };
        let branch = build.flatten(*branch, 0)?;
        let conjuncts = if let Node::And(children) = &build.nodes[branch] {
            children.clone()
        } else {
            vec![branch]
        };
        let mut ae_actions = Vec::new();
        let mut ae_states = Vec::new();
        let mut ea_actions = Vec::new();
        for conjunct in conjuncts {
            if let Node::Eventually(always) = build.nodes[conjunct]
                && let Node::Always(body) = build.nodes[always]
                && build.level_action(body, 0)?.0 < 3
            {
                unique(&mut ea_actions, Check::copy(&mut build, body)?);
            } else if let Some(body) = build.ae_body(conjunct)
                && build.level_action(body, 0)?.0 <= 2
            {
                if build.level_action(body, 0)?.0 == 2 {
                    unique(&mut ae_actions, Check::copy(&mut build, body)?);
                } else {
                    unique(&mut ae_states, Check::copy(&mut build, body)?);
                }
            } else if build.level_action(conjunct, 0)?.1 {
                return Ok(None);
            }
            // A state-only residual is an opaque barrier after these bins.
            // It never contributes a truth or MUST-completeness certificate.
        }
        // Liveness.addToBin inserts AEAction, AEState, then EAAction;
        // LiveCheck executes the entire action bin before the state bin.
        for check in ea_actions {
            unique(&mut ae_actions, check);
        }
        ae_actions.extend(ae_states);
        if ae_actions.is_empty() {
            return Ok(None);
        }
        Ok(Some(SourceProgram {
            checks: ae_actions,
            slots,
            stability: None,
        }))
    }
}

pub(crate) fn compile_source(
    formulas: &[(&Formula, usize)],
    fairness: usize,
    limits: Limits,
) -> Result<Option<SourceProgram>, FormulaError> {
    normalize(formulas, fairness, limits)?
        .map(Normalized::into_source_program)
        .transpose()
        .map(Option::flatten)
}
