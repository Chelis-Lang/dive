//! Pure bound-predicate lowering to the formula language kernel.
//!
//! This M6 L1 foundation expands WF/SF through enabledness and non-stuttering
//! occurrences. TLC's DNF/order, temporal quantification and numbering remain
//! pending. Refusals name the
//! source expression; callers must not remove a model's liveness refusal merely
//! because an individual predicate lowers successfully.

use std::collections::BTreeSet;

use dive_eval::{ConstBinding, EvalError, Scope, SlotView, St, StateRef};
use dive_ir::{Arg, Effects, Expr, ExprId, JunctionKind, Level, OpRef, Origin, QuantKind, StdOp};
use dive_model::{Model, Pred};
use dive_value::Value;

use crate::formula::{Atom, AtomKind, Formula, FormulaError, Limits, Node};

/// The semantics of a bound atom in the temporal language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtomTest {
    /// Evaluate the original predicate as a boolean.
    Predicate,
    /// Existential successor existence, optionally requiring subscript change.
    Enabled {
        /// State-level subscript for `<<A>>_v`, if requested.
        sub: Option<ExprId>,
    },
    /// The action holds and the state-level subscript changes on this edge.
    Nonstuttering {
        /// State-level subscript.
        sub: ExprId,
    },
}

/// A pure state or action predicate retained in its bound scope.
#[derive(Clone, Debug)]
pub struct PredicateAtom {
    /// The expression and the operator/instance context in which to evaluate it.
    pub pred: Pred,
    /// The predicate's evaluation domain.
    pub kind: AtomKind,
    /// Predicate, enabledness or non-stuttering action semantics.
    pub test: AtomTest,
}

impl PredicateAtom {
    /// Evaluates an atom in the model's retained scope. ENABLED uses TLC's
    /// checker-side search, independently of the retained behavior graph.
    ///
    /// # Errors
    /// Missing state, nonboolean predicate or a core evaluation error.
    pub fn evaluate(&self, model: &Model, st: &St) -> Result<bool, EvalError> {
        let ev = &model.evaluator;
        if let AtomTest::Enabled { sub } = self.test {
            let StateRef::Vals(current) = &st.s0 else {
                return Err(EvalError::general("ENABLED atom requires a current state"));
            };
            return ev.enabled_scoped(&self.pred.scope, self.pred.expr, sub, current.clone());
        }
        let holds = match model.eval(&self.pred, st)? {
            Value::Bool(b) => b,
            v => {
                return Err(EvalError::general(format!(
                    "temporal predicate is {}, not boolean",
                    v.kind_name()
                )));
            }
        };
        if !holds {
            return Ok(false);
        }
        if let AtomTest::Nonstuttering { sub } = self.test {
            let (StateRef::Vals(before), StateRef::Vals(after)) = (&st.s0, &st.s1) else {
                return Err(EvalError::general(
                    "non-stuttering atom requires two states",
                ));
            };
            let a = ev.eval_scoped(&self.pred.scope, sub, &St::state(before.clone()))?;
            let b = ev.eval_scoped(&self.pred.scope, sub, &St::state(after.clone()))?;
            return ev.equal_values(&a, &b).map(|same| !same);
        }
        Ok(holds)
    }
}

/// A formula over bound predicates. Atom indices refer to `atoms`.
#[derive(Clone, Debug)]
pub struct Plan {
    /// The checked boolean/always/eventually formula.
    pub formula: Formula,
    /// Pure predicates in deterministic encounter order. No atom is evaluated
    /// while lowering, except constant-level boolean expressions.
    pub atoms: Vec<PredicateAtom>,
}

/// A source expression with its retained span and display location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// The source expression.
    pub expr: ExprId,
    /// Its source span.
    pub origin: Origin,
    /// Human-readable module and line/column location.
    pub location: String,
}

impl Source {
    pub(crate) fn new(model: &Model, expr: ExprId) -> Self {
        Self {
            expr,
            origin: model.ir.node(expr).origin,
            location: model.evaluator.location(expr),
        }
    }
}

/// A named planning refusal with both call-site and operand context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The root predicate the caller requested.
    pub request: Source,
    /// The source operand at which planning stopped.
    pub cause: Source,
    /// The feature or evaluation diagnostic.
    pub detail: String,
}

/// A refusal or explicit planning bound. No variant is a model-checking verdict.
/// Diagnostic data is boxed so successful planning results remain small.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanningError {
    /// A named feature or evaluation that cannot be planned exactly yet.
    Unsupported(Box<Refusal>),
    /// Formula construction or resource limit.
    Formula {
        /// The construction failure.
        error: FormulaError,
        /// The root predicate whose planning failed.
        source: Box<Source>,
    },
}

impl PlanningError {
    /// A stable diagnostic code: unsupported planning, resource limit or
    /// an internal formula-reference inconsistency.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "DIVE-L0020",
            Self::Formula {
                error: FormulaError::Limit(_),
                ..
            } => "DIVE-L0021",
            Self::Formula {
                error: FormulaError::Reference(_),
                ..
            } => "DIVE-L0022",
        }
    }
    fn formula(model: &Model, expr: ExprId, error: FormulaError) -> Self {
        Self::Formula {
            error,
            source: Box::new(Source::new(model, expr)),
        }
    }
}
impl std::fmt::Display for PlanningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(r) => write!(
                f,
                "{}: unsupported liveness planning in {}: {} (at {})",
                self.code(),
                r.request.location,
                r.detail,
                r.cause.location
            ),
            Self::Formula { error, source } => {
                write!(f, "{} at {}: {error}", self.code(), source.location)
            }
        }
    }
}
impl std::error::Error for PlanningError {}

struct Lower<'a> {
    model: &'a Model,
    limits: Limits,
    nodes: Vec<Node>,
    atoms: Vec<PredicateAtom>,
    steps: usize,
    root_expr: ExprId,
}

impl Lower<'_> {
    fn refused(&self, expr: ExprId, detail: impl Into<String>) -> PlanningError {
        PlanningError::Unsupported(Box::new(Refusal {
            request: Source::new(self.model, self.root_expr),
            cause: Source::new(self.model, expr),
            detail: detail.into(),
        }))
    }

    fn push(&mut self, n: Node) -> Result<usize, PlanningError> {
        if self.nodes.len() >= self.limits.formula_nodes {
            return Err(PlanningError::formula(
                self.model,
                self.root_expr,
                FormulaError::Limit("lowered formula nodes"),
            ));
        }
        let i = self.nodes.len();
        self.nodes.push(n);
        Ok(i)
    }

    fn step(&mut self) -> Result<(), PlanningError> {
        self.steps = self.steps.saturating_add(1);
        if self.steps > self.limits.expansion_steps {
            return Err(PlanningError::formula(
                self.model,
                self.root_expr,
                FormulaError::Limit("predicate lowering steps"),
            ));
        }
        Ok(())
    }

    fn bool_constant(&self, expr: ExprId, scope: &Scope) -> Result<bool, PlanningError> {
        match self
            .model
            .evaluator
            .eval_scoped(scope, expr, &St::constant())
        {
            Ok(Value::Bool(b)) => Ok(b),
            Ok(v) => Err(self.refused(
                expr,
                format!(
                    "constant temporal operand is {}, not boolean",
                    v.kind_name()
                ),
            )),
            Err(e) => Err(self.refused(expr, format!("constant temporal operand: {e}"))),
        }
    }

    // Conservative: parameters/substitutions could hide temporal operands or
    // checker reads. Do not classify them from uninstantiated SANY levels.
    fn pure_atom(&mut self, expr: ExprId, scope: &Scope) -> Result<(), PlanningError> {
        let mut todo = vec![expr];
        let mut seen = BTreeSet::new();
        while let Some(e) = todo.pop() {
            self.step()?;
            if !seen.insert(e) {
                continue;
            }
            let ir = &self.model.ir;
            let n = ir.node(e);
            if n.effects
                .intersects(Effects::IMPURE | Effects::TEMPORAL | Effects::USES_ENABLED)
            {
                return Err(self.refused(
                    e,
                    "effectful, temporal or ENABLED operand in a pure predicate atom",
                ));
            }
            match &n.expr {
                Expr::Local(r)
                    if !matches!(
                        self.model.evaluator.scope_slot(scope, *r),
                        SlotView::Value(_)
                    ) =>
                {
                    return Err(self.refused(e, "opaque scoped predicate atom"));
                }
                Expr::SubstRef { .. } => {
                    return Err(self.refused(e, "opaque substituted predicate atom"));
                }
                Expr::Const(c) => match self.model.evaluator.constant_binding(*c) {
                    Some(ConstBinding::Value(_)) => {}
                    Some(ConstBinding::Def(d)) => {
                        let def = ir.def(self.model.evaluator.redirected(d));
                        if def.level != Level::Constant || def.recursive || !def.params.is_empty() {
                            return Err(self.refused(
                                e,
                                "nonconstant/recursive/parameterized constant override",
                            ));
                        }
                        todo.push(def.body);
                    }
                    None => return Err(self.refused(e, "unbound predicate constant")),
                },
                _ => {}
            }
            let mut operators = Vec::new();
            match &n.expr {
                Expr::Def(d) => operators.push(OpRef::Def(*d)),
                Expr::Apply { callee, args } => {
                    operators.push(callee.clone());
                    operators.extend(args.iter().filter_map(|a| {
                        if let Arg::Op { op, .. } = a {
                            Some(op.clone())
                        } else {
                            None
                        }
                    }));
                }
                _ => {}
            }
            for op in operators {
                let standard = match op {
                    OpRef::Std(s) => Some(s),
                    OpRef::Def(d) => {
                        let d = self.model.evaluator.redirected(d);
                        if self.model.evaluator.is_value_override(d) {
                            continue;
                        }
                        let def = ir.def(d);
                        if def.std.is_none() && (def.recursive || !def.params.is_empty()) {
                            return Err(
                                self.refused(e, "recursive/parameterized predicate definition")
                            );
                        }
                        if def.std.is_none() {
                            todo.push(def.body);
                        }
                        def.std
                    }
                    OpRef::Param(_) | OpRef::Const(_) | OpRef::Subst(_) => {
                        return Err(self.refused(e, "dynamically bound predicate operator"));
                    }
                    _ => None,
                };
                if standard == Some(StdOp::TlcTLCGet) {
                    return Err(self.refused(e, "TLCGet requires the liveness search host"));
                }
            }
            todo.extend(n.expr.children());
        }
        Ok(())
    }

    fn special_atom(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        kind: AtomKind,
        test: AtomTest,
    ) -> Result<usize, PlanningError> {
        let index = self.atoms.len();
        self.atoms.push(PredicateAtom {
            pred: Pred::new(expr, scope.clone()),
            kind,
            test,
        });
        self.push(Node::Atom(Atom { kind, index }))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "explicit supported temporal constructors and refusal paths"
    )]
    fn expr(&mut self, expr: ExprId, scope: &Scope, depth: usize) -> Result<usize, PlanningError> {
        self.step()?;
        if depth > 256 {
            return Err(PlanningError::formula(
                self.model,
                self.root_expr,
                FormulaError::Limit("predicate lowering depth"),
            ));
        }
        let ir = &self.model.ir;
        let ev = &self.model.evaluator;
        let node = ir.node(expr);
        if node.effects.intersects(Effects::IMPURE) || ev.scope_shares_side_effects(scope) {
            return Err(self.refused(expr, "effects or dynamic bindings in temporal planning"));
        }
        if node.level <= Level::Action
            && !node
                .effects
                .intersects(Effects::TEMPORAL | Effects::USES_ENABLED)
        {
            self.pure_atom(expr, scope)?;
            if node.level == Level::Constant {
                let b = self.bool_constant(expr, scope)?;
                return self.push(Node::Bool(b));
            }
            let kind = if node.level == Level::Action {
                AtomKind::Action
            } else {
                AtomKind::State
            };
            let index = self.atoms.len();
            self.atoms.push(PredicateAtom {
                pred: Pred::new(expr, scope.clone()),
                kind,
                test: AtomTest::Predicate,
            });
            return self.push(Node::Atom(Atom { kind, index }));
        }
        let depth = depth + 1;
        let n = match &node.expr {
            Expr::Always(c) => Node::Always(self.expr(*c, scope, depth)?),
            Expr::Eventually(c) => Node::Eventually(self.expr(*c, scope, depth)?),
            Expr::Not(c) => Node::Not(self.expr(*c, scope, depth)?),
            Expr::Junction { kind, items, .. } => {
                let cs = items
                    .iter()
                    .map(|e| self.expr(*e, scope, depth))
                    .collect::<Result<Vec<_>, _>>()?;
                if *kind == JunctionKind::And {
                    Node::And(cs)
                } else {
                    Node::Or(cs)
                }
            }
            Expr::Implies(a, b) | Expr::Equiv(a, b) | Expr::LeadsTo(a, b) => {
                let a = self.expr(*a, scope, depth)?;
                let b = self.expr(*b, scope, depth)?;
                let not_a = self.push(Node::Not(a))?;
                if matches!(node.expr, Expr::Equiv(..)) {
                    let not_b = self.push(Node::Not(b))?;
                    let both = self.push(Node::And(vec![a, b]))?;
                    let neither = self.push(Node::And(vec![not_a, not_b]))?;
                    Node::Or(vec![both, neither])
                } else if matches!(node.expr, Expr::LeadsTo(..)) {
                    let eventual = self.push(Node::Eventually(b))?;
                    let implication = self.push(Node::Or(vec![not_a, eventual]))?;
                    Node::Always(implication)
                } else {
                    Node::Or(vec![not_a, b])
                }
            }
            Expr::Quant { kind, bounds, body } if *kind != QuantKind::Choose => {
                for bound in bounds {
                    let Some(dom) = bound.domain else {
                        return Err(self.refused(expr, "unbounded temporal quantifier"));
                    };
                    if ir.node(dom).level != Level::Constant {
                        return Err(self.refused(dom, "nonconstant temporal quantifier domain"));
                    }
                    self.pure_atom(dom, scope)?;
                }
                let scopes = ev
                    .scope_bindings_bounded(scope, expr, &St::constant(), self.limits.formula_nodes)
                    .map_err(|e| {
                        if e.class() == dive_eval::ErrorClass::ResourceLimit {
                            PlanningError::formula(
                                self.model,
                                self.root_expr,
                                FormulaError::Limit("temporal quantifier bindings"),
                            )
                        } else {
                            self.refused(expr, format!("temporal quantifier bindings: {e}"))
                        }
                    })?;
                let children = scopes
                    .iter()
                    .map(|s| self.expr(*body, s, depth))
                    .collect::<Result<Vec<_>, _>>()?;
                if *kind == QuantKind::Forall {
                    Node::And(children)
                } else {
                    Node::Or(children)
                }
            }
            Expr::Def(d) => {
                let d = ev.redirected(*d);
                if ev.is_value_override(d) || ir.def(d).recursive || !ir.def(d).params.is_empty() {
                    return Err(self.refused(
                        expr,
                        "overridden, recursive or parameterized temporal definition",
                    ));
                }
                let sc = ev
                    .scope_enter(scope, d)
                    .map_err(|e| self.refused(expr, e.to_string()))?;
                return self.expr(ir.def(d).body, &sc, depth);
            }
            Expr::Selected(c) | Expr::Label { body: c, .. } => return self.expr(*c, scope, depth),
            Expr::If { cond, then, other } if ir.node(*cond).level == Level::Constant => {
                self.pure_atom(*cond, scope)?;
                let branch = if self.bool_constant(*cond, scope)? {
                    *then
                } else {
                    *other
                };
                return self.expr(branch, scope, depth);
            }
            Expr::If { cond, then, other } => {
                if ir.node(*cond).level == Level::Action {
                    return Err(self.refused(
                        *cond,
                        "TLC liveness rejects action-level conditional guards",
                    ));
                }
                // Temporal truth is pointwise: C /\ T \/ ~C /\ E.
                let guard = self.expr(*cond, scope, depth)?;
                let yes = self.expr(*then, scope, depth)?;
                let no = self.expr(*other, scope, depth)?;
                let yes = self.push(Node::And(vec![guard, yes]))?;
                let not_guard = self.push(Node::Not(guard))?;
                let no = self.push(Node::And(vec![not_guard, no]))?;
                Node::Or(vec![yes, no])
            }
            Expr::Enabled(action) => {
                self.pure_atom(*action, scope)?;
                return self.special_atom(
                    *action,
                    scope,
                    AtomKind::State,
                    AtomTest::Enabled { sub: None },
                );
            }
            Expr::Wf { sub, action } | Expr::Sf { sub, action } => {
                if ir.node(*sub).level > Level::State {
                    return Err(self.refused(*sub, "action-level fairness subscript"));
                }
                self.pure_atom(*action, scope)?;
                self.pure_atom(*sub, scope)?;
                let en = self.special_atom(
                    *action,
                    scope,
                    AtomKind::State,
                    AtomTest::Enabled { sub: Some(*sub) },
                )?;
                let action = self.special_atom(
                    *action,
                    scope,
                    AtomKind::Action,
                    AtomTest::Nonstuttering { sub: *sub },
                )?;
                let disabled = self.push(Node::Not(en))?;
                let recurring = self.push(Node::Eventually(action))?;
                let recurring = self.push(Node::Always(recurring))?;
                let not_enabled = if matches!(node.expr, Expr::Wf { .. }) {
                    let some = self.push(Node::Eventually(disabled))?;
                    self.push(Node::Always(some))?
                } else {
                    let all = self.push(Node::Always(disabled))?;
                    self.push(Node::Eventually(all))?
                };
                Node::Or(vec![not_enabled, recurring])
            }
            other => {
                return Err(self.refused(expr, format!("temporal constructor {}", other.tag())));
            }
        };
        self.push(n)
    }
}

/// Lowers a pure bound temporal predicate into the language kernel, retaining
/// each atom's scope. Supports boolean combinations, `[]`, `<>`, `~>`, pure
/// constant conditionals, direct ENABLED, WF/SF over pure actions and
/// state-level subscripts, and zero-arity temporal definitions. Planning alone
/// does not validate a behavior or enable model-checking liveness.
///
/// # Errors
/// Effects, nested opaque ENABLED, dynamic/opaque scopes, unsupported temporal
/// constructors, constant evaluation failures and explicit construction bounds.
pub fn lower(model: &Model, pred: &Pred, limits: Limits) -> Result<Plan, PlanningError> {
    let mut cx = Lower {
        model,
        limits,
        nodes: Vec::new(),
        atoms: Vec::new(),
        steps: 0,
        root_expr: pred.expr,
    };
    let root = cx.expr(pred.expr, &pred.scope, 0)?;
    let formula =
        Formula::new(cx.nodes, root).map_err(|e| PlanningError::formula(model, pred.expr, e))?;
    Ok(Plan {
        formula,
        atoms: cx.atoms,
    })
}

/// Plans the behavior temporal conjuncts AND the negation of one property.
/// An accepting behavior of this formula is a counterexample candidate.
/// The same pure subset and resource limits as [`lower`] apply to the whole
/// combined formula; unsupported behavior conjuncts cannot be dropped.
///
/// # Errors
/// Any named planning refusal or construction bound in a behavior conjunct
/// or the requested property.
pub fn lower_violation(
    model: &Model,
    property: &Pred,
    limits: Limits,
) -> Result<Plan, PlanningError> {
    let mut cx = Lower {
        model,
        limits,
        nodes: Vec::new(),
        atoms: Vec::new(),
        steps: 0,
        root_expr: property.expr,
    };
    let mut conjuncts = Vec::new();
    for behavior in &model.temporals {
        cx.root_expr = behavior.pred.expr;
        conjuncts.push(cx.expr(behavior.pred.expr, &behavior.pred.scope, 0)?);
    }
    cx.root_expr = property.expr;
    let property = cx.expr(property.expr, &property.scope, 0)?;
    conjuncts.push(cx.push(Node::Not(property))?);
    let root = cx.push(Node::And(conjuncts))?;
    let formula =
        Formula::new(cx.nodes, root).map_err(|e| PlanningError::formula(model, cx.root_expr, e))?;
    Ok(Plan {
        formula,
        atoms: cx.atoms,
    })
}
