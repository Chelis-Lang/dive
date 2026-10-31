//! Independent iterative port of pinned TLC tagged DNF ordering.
//! Only the reference's Java-int DNF product overflow is a semantic bound.

use super::{FormulaError, Node, push, reserve};

pub(super) struct Build<'a> {
    pub(super) nodes: Vec<Node>,
    pub(super) atom_levels: Vec<u8>,
    control: Option<&'a dyn dive_eval::HostControl>,
}

#[derive(Clone, Copy)]
enum Pass {
    Negate(bool),
    Simplify,
    Dnf,
    Flatten,
}
enum Task {
    Enter(usize, Pass),
    Unary(Node, Pass),
    Junction {
        pass: Pass,
        and: bool,
        children: Vec<usize>,
        next: usize,
        done: Vec<usize>,
    },
}

impl<'a> Build<'a> {
    pub(super) fn new(control: Option<&'a dyn dive_eval::HostControl>) -> Self {
        Self {
            nodes: Vec::new(),
            atom_levels: Vec::new(),
            control,
        }
    }
    pub(super) fn step(&self, _depth: usize) -> Result<(), FormulaError> {
        if let Some(control) = self.control {
            control
                .check()
                .map_err(|e| FormulaError::Evaluation(dive_eval::EvalError::interrupted(e)))?;
        }
        Ok(())
    }
    pub(super) fn push(&mut self, node: Node) -> Result<usize, FormulaError> {
        let index = self.nodes.len();
        push(&mut self.nodes, node)?;
        Ok(index)
    }
    pub(super) fn node(&self, at: usize) -> Result<Node, FormulaError> {
        let node = self
            .nodes
            .get(at)
            .ok_or_else(|| super::refused("missing endpoint formula node"))?;
        Ok(match node {
            Node::And(children) => Node::And(copy(children)?),
            Node::Or(children) => Node::Or(copy(children)?),
            other => other.clone(),
        })
    }
    // addConj/addDisj flatten only same-kind junctions, in insertion order.
    pub(super) fn junction(
        &mut self,
        and: bool,
        children: Vec<usize>,
    ) -> Result<usize, FormulaError> {
        let mut flattened = Vec::new();
        let mut pending = children;
        pending.reverse();
        while let Some(child) = pending.pop() {
            self.step(0)?;
            match &self.nodes[child] {
                Node::And(cs) if and => {
                    reserve(&mut pending, cs.len())?;
                    pending.extend(cs.iter().rev());
                }
                Node::Or(cs) if !and => {
                    reserve(&mut pending, cs.len())?;
                    pending.extend(cs.iter().rev());
                }
                _ => push(&mut flattened, child)?,
            }
        }
        self.push(if and {
            Node::And(flattened)
        } else {
            Node::Or(flattened)
        })
    }
    #[allow(
        clippy::too_many_lines,
        reason = "explicit continuations retain ordered negation and absorbing-child simplification"
    )]
    fn transform(&mut self, root: usize, pass: Pass) -> Result<usize, FormulaError> {
        let mut tasks = Vec::new();
        let mut values = Vec::new();
        push(&mut tasks, Task::Enter(root, pass))?;
        while let Some(task) = tasks.pop() {
            self.step(0)?;
            match task {
                Task::Enter(at, pass) => {
                    let node = self.node(at)?;
                    match (&node, pass) {
                        (Node::Not(child), Pass::Negate(negative)) => {
                            push(&mut tasks, Task::Enter(*child, Pass::Negate(!negative)))?;
                        }
                        (Node::Atom(_), Pass::Negate(true)) => {
                            let value = self.push(Node::Not(at))?;
                            push(&mut values, value)?;
                        }
                        (Node::Bool(value), Pass::Negate(negative)) => {
                            let value = self.push(Node::Bool(*value != negative))?;
                            push(&mut values, value)?;
                        }
                        (Node::And(_) | Node::Or(_), _) => {
                            let and = matches!(node, Node::And(_));
                            let (Node::And(children) | Node::Or(children)) = node else {
                                return Err(super::refused("endpoint junction"));
                            };
                            push(
                                &mut tasks,
                                Task::Junction {
                                    pass,
                                    and,
                                    children,
                                    next: 0,
                                    done: Vec::new(),
                                },
                            )?;
                        }
                        (
                            Node::Not(child) | Node::Always(child) | Node::Eventually(child),
                            Pass::Negate(_) | Pass::Simplify | Pass::Flatten,
                        ) => {
                            let child = *child;
                            push(&mut tasks, Task::Unary(node, pass))?;
                            push(&mut tasks, Task::Enter(child, pass))?;
                        }
                        // LNAll/LNEven do not descend in toDNF.
                        _ => push(&mut values, at)?,
                    }
                }
                Task::Unary(node, pass) => {
                    let mut child = values
                        .pop()
                        .ok_or_else(|| super::refused("missing endpoint child"))?;
                    let always = matches!(node, Node::Always(_));
                    let value = match pass {
                        Pass::Negate(negative) => {
                            if always == negative {
                                Node::Eventually(child)
                            } else {
                                Node::Always(child)
                            }
                        }
                        Pass::Simplify if matches!(node, Node::Always(_) | Node::Eventually(_)) => {
                            match self.nodes[child] {
                                Node::Always(c) if always => child = c,
                                Node::Eventually(c) if !always => child = c,
                                _ => {}
                            }
                            if always {
                                Node::Always(child)
                            } else {
                                Node::Eventually(child)
                            }
                        }
                        _ => match node {
                            Node::Not(_) => Node::Not(child),
                            Node::Always(_) => Node::Always(child),
                            _ => Node::Eventually(child),
                        },
                    };
                    let value = self.push(value)?;
                    push(&mut values, value)?;
                }
                Task::Junction {
                    pass,
                    and,
                    children,
                    next,
                    mut done,
                } => {
                    if next > 0 {
                        let child = values
                            .pop()
                            .ok_or_else(|| super::refused("missing endpoint junction child"))?;
                        if matches!(pass, Pass::Simplify) {
                            if let Node::Bool(value) = self.nodes[child] {
                                if value != and {
                                    let value = self.push(Node::Bool(value))?;
                                    push(&mut values, value)?;
                                    continue; // Do not visit later absorbing siblings.
                                }
                            } else {
                                push(&mut done, child)?;
                            }
                        } else {
                            push(&mut done, child)?;
                        }
                    }
                    if let Some(&child) = children.get(next) {
                        push(
                            &mut tasks,
                            Task::Junction {
                                pass,
                                and,
                                children,
                                next: next + 1,
                                done,
                            },
                        )?;
                        push(&mut tasks, Task::Enter(child, pass))?;
                    } else {
                        let value = match pass {
                            Pass::Negate(negative) => self.junction(and != negative, done)?,
                            Pass::Simplify => match done.len() {
                                0 => self.push(Node::Bool(and))?,
                                1 => done[0],
                                _ => self.junction(and, done)?,
                            },
                            Pass::Flatten if done.len() == 1 => done[0],
                            Pass::Flatten => self.junction(and, done)?,
                            Pass::Dnf => self.dnf_junction(and, done)?,
                        };
                        push(&mut values, value)?;
                    }
                }
            }
        }
        values
            .pop()
            .ok_or_else(|| super::refused("missing endpoint transformation"))
    }
    pub(super) fn simplified(&mut self, root: usize) -> Result<usize, FormulaError> {
        let pushed = self.transform(root, Pass::Negate(false))?;
        self.transform(pushed, Pass::Simplify)
    }
    pub(super) fn ae_body(&self, i: usize) -> Option<usize> {
        if let Node::Always(c) = self.nodes[i]
            && let Node::Eventually(body) = self.nodes[c]
        {
            Some(body)
        } else {
            None
        }
    }
    pub(super) fn dnf(&mut self, root: usize, _depth: usize) -> Result<usize, FormulaError> {
        self.transform(root, Pass::Dnf)
    }
    fn dnf_junction(&mut self, and: bool, converted: Vec<usize>) -> Result<usize, FormulaError> {
        if !and {
            let mut plain = Vec::new();
            let mut ae = Vec::new();
            for child in converted {
                self.step(0)?;
                let children = if let Node::Or(cs) = &self.nodes[child] {
                    copy(cs)?
                } else {
                    let mut cs = Vec::new();
                    push(&mut cs, child)?;
                    cs
                };
                for child in children {
                    if let Some(body) = self.ae_body(child) {
                        push(&mut ae, body)?;
                    } else {
                        push(&mut plain, child)?;
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
                let all = self.push(Node::Always(even))?;
                push(&mut plain, all)?;
            }
            return self.junction(false, plain);
        }
        // All children have been converted before the outer product check.
        let mut columns = Vec::new();
        let mut single = Vec::new();
        let mut total = 1usize;
        for child in converted {
            self.step(0)?;
            match self.node(child)? {
                Node::Or(cs) => {
                    total = total
                        .checked_mul(cs.len())
                        .filter(|n| *n <= 2_147_483_647)
                        .ok_or(FormulaError::Limit("TLC DNF cardinality"))?;
                    push(&mut columns, cs)?;
                    push(&mut single, child)?;
                }
                Node::And(cs) => {
                    for c in cs {
                        let mut column = Vec::new();
                        push(&mut column, c)?;
                        push(&mut columns, column)?;
                        push(&mut single, c)?;
                    }
                }
                _ => {
                    let mut column = Vec::new();
                    push(&mut column, child)?;
                    push(&mut columns, column)?;
                    push(&mut single, child)?;
                }
            }
        }
        if total == 1 {
            return self.junction(true, single);
        }
        let mut branches = Vec::new();
        reserve(&mut branches, total)?;
        for position in 0..total {
            self.step(0)?;
            let mut stride = total;
            let mut children = Vec::new();
            reserve(&mut children, columns.len())?;
            for column in &columns {
                // An empty factor produces zero branches; this loop is not entered.
                stride /= column.len();
                push(&mut children, column[(position / stride) % column.len()])?;
            }
            let branch = self.junction(true, children)?;
            push(&mut branches, branch)?;
        }
        self.junction(false, branches)
    }
    pub(super) fn flatten(&mut self, root: usize, _depth: usize) -> Result<usize, FormulaError> {
        self.transform(root, Pass::Flatten)
    }
    pub(super) fn level_action(
        &mut self,
        root: usize,
        _depth: usize,
    ) -> Result<(u8, bool), FormulaError> {
        let mut pending = Vec::new();
        push(&mut pending, root)?;
        let mut level = 0;
        let mut action = false;
        while let Some(at) = pending.pop() {
            self.step(0)?;
            match &self.nodes[at] {
                Node::Bool(_) => {}
                Node::Atom(atom) => {
                    let atom_level = self.atom_levels.get(*atom).copied().unwrap_or(1);
                    level = level.max(atom_level);
                    action |= atom_level == 2;
                }
                Node::Always(c) | Node::Eventually(c) => {
                    level = 3;
                    push(&mut pending, *c)?;
                }
                Node::Not(c) => push(&mut pending, *c)?,
                Node::And(cs) | Node::Or(cs) => {
                    reserve(&mut pending, cs.len())?;
                    pending.extend(cs.iter().rev());
                }
            }
        }
        Ok((level, action))
    }
}
fn copy(values: &[usize]) -> Result<Vec<usize>, FormulaError> {
    let mut out = Vec::new();
    reserve(&mut out, values.len())?;
    out.extend_from_slice(values);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = Result<(), Box<dyn std::error::Error>>;
    #[test]
    fn deep_negation_and_large_junction_have_no_planner_count_cutoff() -> TestResult {
        let mut build = Build::new(None);
        let atom = build.push(Node::Atom(0))?;
        let mut root = atom;
        for _ in 0..1024 {
            root = build.push(Node::Not(root))?;
        }
        let root = build.simplified(root)?;
        assert_eq!(root, atom);
        let children = vec![atom; 5000];
        let root = build.junction(true, children)?;
        let root = build.dnf(root, 0)?;
        assert!(matches!(&build.nodes[root],Node::And(cs) if cs.len()==5000));
        let check = super::super::Check::copy(&mut build, root)?;
        let mut calls = 0;
        assert!(check.evaluate(
            &mut |_| {
                calls += 1;
                Ok(true)
            },
            None
        )?);
        assert_eq!(calls, 5000);
        Ok(())
    }
    #[test]
    fn simplify_skips_absorbed_siblings_and_dnf_retains_reference_factor_order() -> TestResult {
        let mut build = Build::new(None);
        let no = build.push(Node::Bool(false))?;
        // An unreachable malformed reference catches an eager bottom-up walk.
        let root = build.push(Node::And(vec![no, usize::MAX]))?;
        let root = build.transform(root, Pass::Simplify)?;
        assert_eq!(build.nodes[root], Node::Bool(false));
        let mut atoms = Vec::new();
        for i in 0..4 {
            atoms.push(build.push(Node::Atom(i))?);
        }
        let left = build.push(Node::Or(atoms[..2].to_vec()))?;
        let right = build.push(Node::Or(atoms[2..].to_vec()))?;
        let root = build.push(Node::And(vec![left, right]))?;
        let root = build.dnf(root, 0)?;
        let Node::Or(branches) = &build.nodes[root] else {
            return Err("DNF disjunction".into());
        };
        let actual = branches
            .iter()
            .map(|&i| match &build.nodes[i] {
                Node::And(cs) => cs.clone(),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                vec![atoms[0], atoms[2]],
                vec![atoms[0], atoms[3]],
                vec![atoms[1], atoms[2]],
                vec![atoms[1], atoms[3]]
            ]
        );
        let singleton = build.push(Node::Or(vec![atoms[0]]))?;
        let root = build.push(Node::And(vec![singleton]))?;
        let root = build.dnf(root, 0)?;
        let Node::And(children) = &build.nodes[root] else {
            return Err("singleton DNF conjunction".into());
        };
        assert_eq!(children.len(), 1);
        assert_eq!(build.nodes[children[0]], Node::Or(vec![atoms[0]]));
        assert_eq!(build.nodes[atoms[0]], Node::Atom(0));
        assert_eq!(build.nodes[singleton], Node::Or(vec![atoms[0]]));
        let empty = build.push(Node::Or(Vec::new()))?;
        let root = build.push(Node::And(vec![empty]))?;
        let root = build.dnf(root, 0)?;
        assert_eq!(build.nodes[root], Node::Or(Vec::new()));
        Ok(())
    }
    #[test]
    fn only_disjunctive_java_int_product_is_reference_overflow() -> TestResult {
        let mut build = Build::new(None);
        let a = build.push(Node::Atom(0))?;
        let b = build.push(Node::Atom(1))?;
        let factor = build.push(Node::Or(vec![a, b]))?;
        let root = build.push(Node::And(vec![factor; 31]))?;
        assert!(matches!(
            build.dnf(root, 0),
            Err(FormulaError::Limit("TLC DNF cardinality"))
        ));
        Ok(())
    }
}
