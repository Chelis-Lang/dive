//! Independent reconstruction of pinned TLC source-state demand.
//! No checker graph, producer plan/cache, occurrence selector or boolean is read.

use crate::endpoint::TemporalStateEndpoint;
use dive_eval::{ConstBinding, EvalError, SlotView, St};
use dive_ir::{Arg, Expr, JunctionKind, Level, OpRef};
use dive_model::{Model, Pred};
use dive_value::Value;

mod metadata;
mod normalize;
use normalize::Build;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Bool(bool),
    Atom(usize),
    Not(usize),
    And(Vec<usize>),
    Or(Vec<usize>),
    Always(usize),
    Eventually(usize),
}

#[derive(Debug)]
pub(crate) enum FormulaError {
    Limit(&'static str),
    Reference(String),
    Evaluation(EvalError),
}
impl std::fmt::Display for FormulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit(s) => write!(f, "endpoint validation bound: {s}"),
            Self::Reference(s) => f.write_str(s),
            Self::Evaluation(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for FormulaError {}
impl From<EvalError> for FormulaError {
    fn from(e: EvalError) -> Self {
        Self::Evaluation(e)
    }
}
fn refused(s: impl Into<String>) -> FormulaError {
    FormulaError::Reference(s.into())
}
fn allocation() -> FormulaError {
    FormulaError::Evaluation(EvalError::resource_limit("temporal endpoint allocation"))
}
pub(super) fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), FormulaError> {
    values.try_reserve(additional).map_err(|_| allocation())
}
fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), FormulaError> {
    reserve(values, 1)?;
    values.push(value);
    Ok(())
}

fn enter_active(
    active: &mut Vec<Pred>,
    pred: &Pred,
    control: Option<&dyn dive_eval::HostControl>,
) -> Result<(), FormulaError> {
    if !pred.scope.same_analysis_context(&pred.scope) {
        return Err(refused(
            "dynamic @ context has no endpoint analysis identity",
        ));
    }
    for previous in active.iter() {
        if let Some(control) = control {
            control.check().map_err(EvalError::interrupted)?;
        }
        if previous.expr == pred.expr && previous.scope.same_analysis_context(&pred.scope) {
            return Err(refused("cyclic endpoint source context"));
        }
    }
    push(active, pred.clone())
}

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
        enum Task {
            Enter(usize),
            Finish(u8, usize),
        }
        let mut tasks = Vec::new();
        let mut values = Vec::new();
        let mut nodes = Vec::new();
        push(&mut tasks, Task::Enter(root))?;
        while let Some(task) = tasks.pop() {
            build.step(0)?;
            let node = match task {
                Task::Enter(at) => match build.node(at)? {
                    Node::Bool(value) => Boolean::Constant(value),
                    Node::Atom(atom) => Boolean::Atom(atom),
                    Node::Not(child) => {
                        push(&mut tasks, Task::Finish(0, 1))?;
                        push(&mut tasks, Task::Enter(child))?;
                        continue;
                    }
                    Node::And(children) | Node::Or(children) => {
                        let kind = if matches!(build.nodes[at], Node::And(_)) {
                            1
                        } else {
                            2
                        };
                        push(&mut tasks, Task::Finish(kind, children.len()))?;
                        for child in children.into_iter().rev() {
                            push(&mut tasks, Task::Enter(child))?;
                        }
                        continue;
                    }
                    Node::Always(_) | Node::Eventually(_) => {
                        return Err(refused("temporal demand bin"));
                    }
                },
                Task::Finish(kind, count) => {
                    let start = values
                        .len()
                        .checked_sub(count)
                        .ok_or_else(|| refused("missing endpoint check children"))?;
                    let mut children = Vec::new();
                    reserve(&mut children, count)?;
                    children.extend(values.drain(start..));
                    match kind {
                        0 => Boolean::Not(children[0]),
                        1 => Boolean::And(children),
                        _ => Boolean::Or(children),
                    }
                }
            };
            let index = nodes.len();
            push(&mut nodes, node)?;
            push(&mut values, index)?;
        }
        let root = values
            .pop()
            .ok_or_else(|| refused("missing endpoint check"))?;
        Ok(Self { nodes, root })
    }
    fn must_read(&self, build: &mut Build) -> Result<Vec<usize>, FormulaError> {
        let mut known: Vec<Option<bool>> = Vec::new();
        let mut must: Vec<Vec<usize>> = Vec::new();
        reserve(&mut known, self.nodes.len())?;
        reserve(&mut must, self.nodes.len())?;
        for node in &self.nodes {
            build.step(0)?;
            let (value, reads) = match node {
                Boolean::Constant(value) => (Some(*value), Vec::new()),
                Boolean::Atom(atom) => {
                    let mut reads = Vec::new();
                    push(&mut reads, *atom)?;
                    (None, reads)
                }
                Boolean::Not(child) => {
                    let mut reads = Vec::new();
                    reserve(&mut reads, must[*child].len())?;
                    reads.extend_from_slice(&must[*child]);
                    (known[*child].map(|v| !v), reads)
                }
                Boolean::And(children) | Boolean::Or(children) => {
                    let and = matches!(node, Boolean::And(_));
                    let mut reads = Vec::new();
                    let mut forced = true;
                    let mut all_known = true;
                    let mut absorbing = false;
                    for &child in children {
                        build.step(0)?;
                        if forced {
                            for &atom in &must[child] {
                                build.step(0)?;
                                if !reads.contains(&atom) {
                                    push(&mut reads, atom)?;
                                }
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
            push(&mut known, value)?;
            push(&mut must, reads)?;
        }
        Ok(must.swap_remove(self.root))
    }
    fn evaluate(
        &self,
        predicate: &mut impl FnMut(usize) -> Result<bool, EvalError>,
        control: Option<&dyn dive_eval::HostControl>,
    ) -> Result<bool, FormulaError> {
        enum Task {
            Enter(usize),
            Not,
            Junction { at: usize, next: usize, and: bool },
        }
        let mut tasks = Vec::new();
        let mut values = Vec::new();
        push(&mut tasks, Task::Enter(self.root))?;
        while let Some(task) = tasks.pop() {
            if let Some(control) = control {
                control.check().map_err(EvalError::interrupted)?;
            }
            match task {
                Task::Enter(at) => match &self.nodes[at] {
                    Boolean::Constant(value) => push(&mut values, *value)?,
                    Boolean::Atom(atom) => push(&mut values, predicate(*atom)?)?,
                    Boolean::Not(child) => {
                        push(&mut tasks, Task::Not)?;
                        push(&mut tasks, Task::Enter(*child))?;
                    }
                    Boolean::And(_) | Boolean::Or(_) => push(
                        &mut tasks,
                        Task::Junction {
                            at,
                            next: 0,
                            and: matches!(self.nodes[at], Boolean::And(_)),
                        },
                    )?,
                },
                Task::Not => {
                    let value = values
                        .pop()
                        .ok_or_else(|| refused("missing endpoint boolean"))?;
                    push(&mut values, !value)?;
                }
                Task::Junction { at, next, and } => {
                    if next > 0 {
                        let value = values
                            .pop()
                            .ok_or_else(|| refused("missing endpoint junction truth"))?;
                        if value != and {
                            push(&mut values, value)?;
                            continue;
                        }
                    }
                    let (Boolean::And(children) | Boolean::Or(children)) = &self.nodes[at] else {
                        return Err(refused("invalid endpoint boolean junction"));
                    };
                    if let Some(&child) = children.get(next) {
                        push(
                            &mut tasks,
                            Task::Junction {
                                at,
                                next: next + 1,
                                and,
                            },
                        )?;
                        push(&mut tasks, Task::Enter(child))?;
                    } else {
                        push(&mut values, and)?;
                    }
                }
            }
        }
        values
            .pop()
            .ok_or_else(|| refused("missing endpoint truth"))
    }
}

struct Lower<'a> {
    model: &'a Model,
    build: Build<'a>,
    atoms: Vec<Pred>,
    constant: Vec<bool>,
    control: Option<&'a dyn dive_eval::HostControl>,
}
impl Lower<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "one iterative temporal lowering dispatch preserves original scoped occurrences"
    )]
    fn lower(&mut self, pred: &Pred, _depth: usize) -> Result<usize, FormulaError> {
        enum Finish {
            Unary(u8),
            Junction(bool, usize),
            Binary(u8),
        }
        enum Task {
            Enter(Pred),
            Leave,
            Finish(Finish),
        }
        let mut tasks = Vec::new();
        let mut values = Vec::new();
        let mut active = Vec::new();
        push(&mut tasks, Task::Enter(pred.clone()))?;
        while let Some(task) = tasks.pop() {
            self.build.step(0)?;
            match task {
                Task::Leave => {
                    active.pop();
                }
                Task::Finish(finish) => {
                    let value = match finish {
                        Finish::Unary(kind) => {
                            let child = take_node(&mut values)?;
                            self.build.push(match kind {
                                0 => Node::Not(child),
                                1 => Node::Always(child),
                                _ => Node::Eventually(child),
                            })?
                        }
                        Finish::Junction(and, count) => {
                            let start = values
                                .len()
                                .checked_sub(count)
                                .ok_or_else(|| refused("missing endpoint lower children"))?;
                            let mut children = Vec::new();
                            reserve(&mut children, count)?;
                            children.extend(values.drain(start..));
                            self.build.junction(and, children)?
                        }
                        Finish::Binary(kind) => {
                            let right = take_node(&mut values)?;
                            let left = take_node(&mut values)?;
                            if kind == 2 {
                                let yes = self.build.junction(true, vec![left, right])?;
                                let no_left = self.build.push(Node::Not(left))?;
                                let no_right = self.build.push(Node::Not(right))?;
                                let no = self.build.junction(true, vec![no_left, no_right])?;
                                self.build.junction(false, vec![yes, no])?
                            } else {
                                let left = self.build.push(Node::Not(left))?;
                                let right = if kind == 1 {
                                    self.build.push(Node::Eventually(right))?
                                } else {
                                    right
                                };
                                let disj = self.build.junction(false, vec![left, right])?;
                                if kind == 1 {
                                    self.build.push(Node::Always(disj))?
                                } else {
                                    disj
                                }
                            }
                        }
                    };
                    push(&mut values, value)?;
                }
                Task::Enter(pred) => {
                    enter_active(&mut active, &pred, self.control)?;
                    push(&mut tasks, Task::Leave)?;
                    self.model
                        .evaluator
                        .validate_predicate(&pred.scope, pred.expr)?;
                    let level = metadata::level(self.model, &pred, self.control)?;
                    if level <= Level::State {
                        let id = self.atoms.len();
                        push(&mut self.atoms, pred)?;
                        push(&mut self.constant, level == Level::Constant)?;
                        let atom = self.build.push(Node::Atom(id))?;
                        push(&mut values, atom)?;
                        continue;
                    }
                    let ev = &self.model.evaluator;
                    let scope = &pred.scope;
                    match self.model.ir.expr(pred.expr) {
                        Expr::Subst { body, .. } => push(
                            &mut tasks,
                            Task::Enter(Pred::new(
                                *body,
                                metadata::substitution(self.model, &pred)?,
                            )),
                        )?,
                        Expr::Let { body, .. } => push(
                            &mut tasks,
                            Task::Enter(Pred::new(*body, ev.scope_let(scope, pred.expr)?)),
                        )?,
                        Expr::Local(local) => match ev.scope_slot(scope, *local) {
                            SlotView::Lazy(expr, scope) => {
                                push(&mut tasks, Task::Enter(Pred::new(expr, scope)))?
                            }
                            _ => return Err(refused("opaque temporal local")),
                        },
                        Expr::SubstRef {
                            decl,
                            primed: false,
                        } => match ev.scope_subst_slot(scope, *decl) {
                            SlotView::Lazy(expr, scope) => {
                                push(&mut tasks, Task::Enter(Pred::new(expr, scope)))?
                            }
                            _ => return Err(refused("opaque temporal substitution")),
                        },
                        Expr::Def(def) => {
                            let target = ev.redirected(*def);
                            let definition = self.model.ir.def(target);
                            if definition.recursive
                                || definition.std.is_some()
                                || ev.is_value_override(target)
                            {
                                return Err(refused("opaque temporal definition"));
                            }
                            push(
                                &mut tasks,
                                Task::Enter(Pred::new(
                                    definition.body,
                                    ev.scope_enter(scope, target)?,
                                )),
                            )?;
                        }
                        Expr::Apply { callee, args } => {
                            push(&mut tasks, Task::Enter(self.call(&pred, callee, args)?))?
                        }
                        Expr::Selected(expr) | Expr::Label { body: expr, .. } => {
                            push(&mut tasks, Task::Enter(Pred::new(*expr, scope.clone())))?
                        }
                        Expr::Not(expr) | Expr::Always(expr) | Expr::Eventually(expr) => {
                            let kind = match self.model.ir.expr(pred.expr) {
                                Expr::Not(_) => 0,
                                Expr::Always(_) => 1,
                                _ => 2,
                            };
                            push(&mut tasks, Task::Finish(Finish::Unary(kind)))?;
                            push(&mut tasks, Task::Enter(Pred::new(*expr, scope.clone())))?;
                        }
                        Expr::Junction { kind, items, .. } => {
                            push(
                                &mut tasks,
                                Task::Finish(Finish::Junction(
                                    *kind == JunctionKind::And,
                                    items.len(),
                                )),
                            )?;
                            for &expr in items.iter().rev() {
                                push(&mut tasks, Task::Enter(Pred::new(expr, scope.clone())))?;
                            }
                        }
                        Expr::Implies(left, right)
                        | Expr::LeadsTo(left, right)
                        | Expr::Equiv(left, right) => {
                            let kind = match self.model.ir.expr(pred.expr) {
                                Expr::Implies(..) => 0,
                                Expr::LeadsTo(..) => 1,
                                _ => 2,
                            };
                            push(&mut tasks, Task::Finish(Finish::Binary(kind)))?;
                            push(&mut tasks, Task::Enter(Pred::new(*right, scope.clone())))?;
                            push(&mut tasks, Task::Enter(Pred::new(*left, scope.clone())))?;
                        }
                        other => {
                            return Err(refused(format!(
                                "endpoint temporal constructor {}",
                                other.tag()
                            )));
                        }
                    }
                }
            }
        }
        values
            .pop()
            .ok_or_else(|| refused("missing endpoint formula"))
    }
    fn call(&self, pred: &Pred, callee: &OpRef, args: &[Arg]) -> Result<Pred, FormulaError> {
        let ev = &self.model.evaluator;
        let scope = &pred.scope;
        let (def, called) = match callee {
            OpRef::Def(def) => {
                let target = ev.redirected(*def);
                (target, ev.scope_apply(scope, target, args)?)
            }
            OpRef::Param(local) => ev
                .scope_apply_param(scope, *local, args)
                .ok_or_else(|| refused("opaque temporal operator actual"))??,
            OpRef::Const(constant) => {
                let Some(ConstBinding::Def(def)) = ev.constant_binding(*constant) else {
                    return Err(refused("opaque configured temporal value"));
                };
                let target = ev.redirected(def);
                (target, ev.scope_apply(scope, target, args)?)
            }
            _ => return Err(refused("opaque temporal call")),
        };
        let target = self.model.ir.def(def);
        if target.recursive || target.std.is_some() || ev.is_value_override(def) {
            return Err(refused("unsupported temporal callee"));
        }
        Ok(Pred::new(target.body, called))
    }

    fn canonical(&self) -> Result<(), FormulaError> {
        if let Some(control) = self.control {
            control.check().map_err(EvalError::interrupted)?;
        }
        let mut producers = Vec::new();
        for init in &self.model.init {
            push(&mut producers, (init.pred.expr, init.pred.scope.clone()))?;
        }
        if let Some(next) = &self.model.next {
            push(&mut producers, (next.expr, next.scope.clone()))?;
        }
        for action in self.model.init_actions.iter().chain(&self.model.actions) {
            push(&mut producers, (action.expr(), action.scope()))?;
        }
        for named in self
            .model
            .invariants
            .iter()
            .chain(&self.model.implied_inits)
            .chain(&self.model.implied_actions)
            .chain(&self.model.constraints)
            .chain(&self.model.action_constraints)
            .chain(self.model.view.iter())
        {
            push(&mut producers, (named.pred.expr, named.pred.scope.clone()))?;
        }
        if let Some(alias) = &self.model.alias {
            let named = alias.as_ref().map_err(|_| refused("unbound alias"))?;
            push(&mut producers, (named.pred.expr, named.pred.scope.clone()))?;
        }
        for named in self
            .model
            .postconditions
            .as_ref()
            .map_err(|_| refused("unbound postcondition"))?
        {
            push(&mut producers, (named.pred.expr, named.pred.scope.clone()))?;
        }
        for &def in &self.model.assumptions {
            push(
                &mut producers,
                (
                    self.model.ir.def(def).body,
                    self.model.evaluator.scope(def)?,
                ),
            )?;
        }
        let mut atoms = Vec::new();
        for pred in &self.atoms {
            push(&mut atoms, (pred.expr, pred.scope.clone()))?;
        }
        let proof = self.model.evaluator.canonical_producer_stability_budgeted(
            &producers,
            &atoms,
            self.control,
        )?;
        if !proof.is_some_and(|p| p.belongs_to(&self.model.evaluator)) {
            return Err(refused(
                "uncertified endpoint producer/capture/normalization closure",
            ));
        }
        Ok(())
    }
}

fn take_node(values: &mut Vec<usize>) -> Result<usize, FormulaError> {
    values.pop().ok_or_else(|| {
        FormulaError::Evaluation(EvalError::bug(
            "missing independent endpoint continuation result",
        ))
    })
}

fn unique(checks: &mut Vec<Check>, check: Check) -> Result<(), FormulaError> {
    if !checks.contains(&check) {
        push(checks, check)?;
    }
    Ok(())
}
fn bins(build: &mut Build<'_>, dnf: usize) -> Result<Vec<Check>, FormulaError> {
    if build.nodes[dnf] == Node::Bool(false) {
        return Err(refused("TLC startup tautology has no row endpoint"));
    }
    let branches = if let Node::Or(cs) = &build.nodes[dnf] {
        let mut branches = Vec::new();
        reserve(&mut branches, cs.len())?;
        branches.extend_from_slice(cs);
        branches
    } else {
        vec![dnf]
    };
    let mut actions = Vec::new();
    let mut states = Vec::new();
    for branch in branches {
        let branch = build.flatten(branch, 0)?;
        let cs = if let Node::And(cs) = &build.nodes[branch] {
            let mut children = Vec::new();
            reserve(&mut children, cs.len())?;
            children.extend_from_slice(cs);
            children
        } else {
            vec![branch]
        };
        for c in cs {
            if let Node::Eventually(a) = build.nodes[c]
                && let Node::Always(body) = build.nodes[a]
                && build.level_action(body, 0)?.0 < 3
            {
                unique(&mut actions, Check::copy(build, body)?)?;
            } else if let Some(body) = build.ae_body(c)
                && build.level_action(body, 0)?.0 <= 1
            {
                unique(&mut states, Check::copy(build, body)?)?;
            } else {
                return Err(refused(
                    "residual/tableau or successor-dependent endpoint solution",
                ));
            }
        }
    }
    reserve(&mut actions, states.len())?;
    actions.extend(states);
    Ok(actions)
}

pub(crate) struct Program {
    atoms: Vec<Pred>,
    checks: Vec<Check>,
}

/// Reconstruct before any finite-path or endpoint expression is evaluated.
pub(crate) fn prepare(
    model: &Model,
    states: &[crate::State],
    control: Option<&dyn dive_eval::HostControl>,
) -> Result<Program, FormulaError> {
    model.validate_context()?;
    if model.view.is_some() || model.symmetry.is_some() || model.has_runtime_names() {
        return Err(refused(
            "endpoint requires original concrete source states and stable names",
        ));
    }
    for state in states {
        for value in state.iter().flatten() {
            canonical_value(model, value, control)?;
        }
    }
    let mut lower = Lower {
        model,
        build: Build::new(control),
        atoms: Vec::new(),
        constant: Vec::new(),
        control,
    };
    let mut behavior = Vec::new();
    for n in &model.temporals {
        let root = lower.lower(&n.pred, 0)?;
        push(&mut behavior, root)?;
    }
    let mut claims = Vec::new();
    for n in model.property_claims() {
        let root = lower.lower(&n.pred, 0)?;
        let root = lower.build.push(Node::Not(root))?;
        push(&mut claims, root)?;
    }
    if claims.is_empty() {
        return Err(refused("no configured temporal endpoint demand"));
    }
    // No expression, constant or provider has been evaluated by lowering.
    lower.canonical()?;
    for (i, pred) in lower.atoms.iter().enumerate() {
        if lower.constant[i] {
            if let Some(control) = control {
                control.check().map_err(EvalError::interrupted)?;
            }
            let value = model
                .evaluator
                .eval_scoped(&pred.scope, pred.expr, &St::constant())?;
            let Value::Bool(value) = value else {
                return Err(refused("nonboolean constant temporal source"));
            };
            for index in 0..lower.build.nodes.len() {
                lower.build.step(0)?;
                if lower.build.nodes[index] == Node::Atom(i) {
                    lower.build.nodes[index] = Node::Bool(value);
                }
            }
        }
    }
    let claim = lower.build.junction(false, claims)?;
    push(&mut behavior, claim)?;
    let root = lower.build.junction(true, behavior)?;
    let root = lower.build.simplified(root)?;
    let dnf = lower.build.dnf(root, 0)?;
    let checks = bins(&mut lower.build, dnf)?;
    let mut must = Vec::new();
    reserve(&mut must, lower.atoms.len())?;
    must.resize(lower.atoms.len(), false);
    for check in &checks {
        for atom in check.must_read(&mut lower.build)? {
            must[atom] = true;
        }
    }
    if lower
        .constant
        .iter()
        .enumerate()
        .any(|(i, c)| !*c && !must[i])
    {
        return Err(refused("incomplete independent temporal MUST coverage"));
    }
    Ok(Program {
        atoms: lower.atoms,
        checks,
    })
}

impl Program {
    /// The request supplies no selector: execute all own bins until first error.
    pub(crate) fn replay(
        &self,
        model: &Model,
        state: &crate::State,
        ec: &str,
        endpoint: &TemporalStateEndpoint,
        control: Option<&dyn dive_eval::HostControl>,
    ) -> Result<Result<(), String>, FormulaError> {
        let st = St::state(state.clone());
        for check in &self.checks {
            let result = check.evaluate(
                &mut |at| -> Result<bool, EvalError> {
                    if let Some(control) = control {
                        control.check().map_err(EvalError::interrupted)?;
                    }
                    let pred = &self.atoms[at];
                    match model.evaluator.eval_scoped(&pred.scope, pred.expr, &st)? {
                        Value::Bool(value) => Ok(value),
                        _ => Err(EvalError::unsupported("nonboolean endpoint atom")),
                    }
                },
                control,
            );
            if let Err(error) = result {
                let FormulaError::Evaluation(error) = error else {
                    return Err(error);
                };
                if error.class() != dive_eval::ErrorClass::Eval || !error.is_eval_exception() {
                    return Err(FormulaError::Evaluation(error));
                }
                if error.ec().name() != ec {
                    return Ok(Err(format!(
                        "first independent temporal error is {}, not {ec}",
                        error.ec().name()
                    )));
                }
                if TemporalStateEndpoint::from_error(model, &error).as_ref() != Some(endpoint) {
                    return Ok(Err(
                        "first independent temporal error has different source context".into(),
                    ));
                }
                return Ok(Ok(()));
            }
        }
        Ok(Err(
            "independent temporal demand completed without the claimed error".into(),
        ))
    }
}

fn canonical_value(
    model: &Model,
    value: &Value,
    control: Option<&dyn dive_eval::HostControl>,
) -> Result<(), FormulaError> {
    let mut pending = Vec::new();
    push(&mut pending, value.clone())?;
    while let Some(value) = pending.pop() {
        if let Some(control) = control {
            control.check().map_err(EvalError::interrupted)?;
        }
        match &value {
            Value::Bool(_) | Value::Int(_) | Value::BigInt(_) => {}
            Value::Str(atom) | Value::Model(atom) => {
                if model
                    .names
                    .resolve(atom.tok())
                    .is_none_or(|s| s.as_str() != atom.as_str())
                {
                    return Err(refused("foreign endpoint name"));
                }
            }
            Value::Fcn(function) => {
                if !function.is_tuple_literal() && !function.is_record_literal() {
                    return Err(refused("nonliteral endpoint function"));
                }
                if function.as_tuple().is_none()
                    && function.order() != model.evaluator.options().ctx.order
                {
                    return Err(refused("endpoint record order"));
                }
                for (key, value) in function.iter() {
                    push(&mut pending, key)?;
                    push(&mut pending, value.clone())?;
                }
            }
            Value::Set(set) => {
                if set.kind() != dive_value::SetKind::Enum
                    || set.order() != model.evaluator.options().ctx.order
                {
                    return Err(refused("uncanonical/opaque endpoint set"));
                }
                for value in set
                    .stored_elements()
                    .ok_or_else(|| refused("endpoint set storage"))?
                {
                    push(&mut pending, value.clone())?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boolean_replay_is_iterative_and_retains_shortcircuit_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut nodes = vec![Boolean::Atom(0), Boolean::Atom(1)];
        let mut root = 0;
        for _ in 0..1024 {
            let next = nodes.len();
            nodes.push(Boolean::Not(root));
            root = next;
        }
        let last = nodes.len();
        nodes.push(Boolean::Or(vec![root, 1]));
        let check = Check { nodes, root: last };
        let mut calls = Vec::new();
        assert!(check.evaluate(
            &mut |at| {
                calls.push(at);
                if at == 1 {
                    Err(EvalError::bug("skipped endpoint atom"))
                } else {
                    Ok(true)
                }
            },
            None
        )?);
        assert_eq!(calls, vec![0]);
        let stop = Stop;
        assert!(
            matches!(check.evaluate(&mut |_|Ok(true),Some(&stop)),Err(FormulaError::Evaluation(error)) if error.interruption().is_some())
        );
        Ok(())
    }
    #[test]
    fn deep_metadata_and_actual_value_storage_have_no_new_count_boundary()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut body = String::from("A0 == x=0\n");
        for i in 1..600 {
            body.push_str(&format!("A{i} == A{}\n", i - 1));
        }
        let source = format!(
            "---- MODULE DeepMetadata ----\nVARIABLE x\nInit == x=0\nNext == x'=x\n{body}P == []<>A599\n====\n"
        );
        let analysis =
            dive_sema::analyze_source("DeepMetadata.tla", &source, &dive_sema::Options::default());
        assert!(analysis.accepted(), "{analysis:?}");
        let ir = std::sync::Arc::new(dive_ir::lower(&analysis)?);
        let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .ok_or("config")?;
        let model = dive_model::bind(ir.clone(), &config, &dive_model::BindOptions::tlc())?;
        let def = ir.root_def("A599").ok_or("deep actual alias")?;
        let scope = model.evaluator.scope(def)?;
        assert!(scope.same_analysis_context(&scope.clone()));
        let pred = Pred::new(ir.def(def).body, scope);
        assert_eq!(metadata::level(&model, &pred, None)?, Level::State);
        assert!(
            matches!(metadata::level(&model,&pred,Some(&Stop)),Err(FormulaError::Evaluation(error)) if error.interruption().is_some())
        );
        let value = (0..300).fold(Value::Int(1), |value, _| Value::tuple(vec![value]));
        canonical_value(&model, &value, None)?;
        assert!(
            matches!(reserve(&mut Vec::<u64>::new(),usize::MAX),Err(FormulaError::Evaluation(error)) if error.class()==dive_eval::ErrorClass::ResourceLimit)
        );
        Ok(())
    }
    #[derive(Debug)]
    struct Stop;
    impl dive_eval::HostControl for Stop {
        fn check(&self) -> Result<(), dive_eval::Interrupted> {
            Err(dive_eval::Interrupted::new(938))
        }
    }
}
