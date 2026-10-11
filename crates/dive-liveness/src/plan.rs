//! Pure bound-predicate lowering to the formula language kernel.
//!
//! This M6 L1 foundation expands WF/SF through enabledness and non-stuttering
//! occurrences. TLC's DNF/order and numbering remain
//! pending. Refusals name the
//! source expression; callers must not remove a model's liveness refusal merely
//! because an individual predicate lowers successfully.

use dive_eval::{ConstBinding, Ec, ErrorClass, EvalError, Scope, SlotView, St, StateRef};
use dive_ir::{
    Arg, DefId, Effects, Expr, ExprId, JunctionKind, Level, OpRef, Origin, QuantKind, StdOp,
};
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
    /// TLC retains directly syntactic `~ENABLED A` as one state atom.
    NegatedEnabled {
        /// Pure action in the original predicate scope.
        action: ExprId,
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
    /// Maximum concrete bindings per predicate binder.
    pub binding_limit: usize,
}

impl PredicateAtom {
    /// Evaluates an atom in the model's retained scope. ENABLED uses TLC's
    /// checker-side search, independently of the retained behavior graph.
    ///
    /// # Errors
    /// Missing state, nonboolean predicate or a core evaluation error.
    pub fn evaluate(&self, model: &Model, st: &St) -> Result<bool, EvalError> {
        let ev = &model.evaluator;
        ev.validate_model_context(&model.ir, &model.names)?;
        ev.validate_predicate(&self.pred.scope, self.pred.expr)?;
        if let AtomTest::Enabled { sub } = self.test {
            ev.check_finite_bindings(&self.pred.scope, self.pred.expr, st, self.binding_limit)?;
            let StateRef::Vals(current) = &st.s0 else {
                return Err(EvalError::general("ENABLED atom requires a current state"));
            };
            return ev.enabled_scoped(&self.pred.scope, self.pred.expr, sub, current.clone());
        }
        if let AtomTest::NegatedEnabled { action } = self.test {
            ev.validate_predicate(&self.pred.scope, action)?;
            ev.check_finite_bindings(&self.pred.scope, action, st, self.binding_limit)?;
            let StateRef::Vals(current) = &st.s0 else {
                return Err(EvalError::general(
                    "negated ENABLED atom requires a current state",
                ));
            };
            return ev
                .enabled_scoped(&self.pred.scope, action, None, current.clone())
                .map(|b| !b);
        }
        if let AtomTest::Nonstuttering { sub } = self.test {
            ev.validate_predicate(&self.pred.scope, sub)?;
            let (StateRef::Vals(before), StateRef::Vals(after)) = (&st.s0, &st.s1) else {
                return Err(EvalError::general(
                    "non-stuttering atom requires two states",
                ));
            };
            ev.check_finite_bindings(
                &self.pred.scope,
                sub,
                &St::state(before.clone()),
                self.binding_limit,
            )?;
            ev.check_finite_bindings(
                &self.pred.scope,
                sub,
                &St::state(after.clone()),
                self.binding_limit,
            )?;
            let a = ev.eval_scoped(&self.pred.scope, sub, &St::state(before.clone()))?;
            let b = ev.eval_scoped(&self.pred.scope, sub, &St::state(after.clone()))?;
            if ev.equal_values(&a, &b)? {
                return Ok(false);
            }
        }
        ev.check_finite_bindings(&self.pred.scope, self.pred.expr, st, self.binding_limit)?;
        let holds = match model.eval(&self.pred, st)? {
            Value::Bool(b) => b,
            v => {
                return Err(EvalError::general(format!(
                    "temporal predicate is {}, not boolean",
                    v.kind_name()
                )));
            }
        };

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
            origin: model
                .ir
                .nodes()
                .get(expr.index())
                .map_or(Origin::NONE, |n| n.origin),
            location: if model.ir.nodes().get(expr.index()).is_some() {
                model.evaluator.location(expr)
            } else {
                format!("unchecked expression reference {}", expr.index())
            },
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
    /// An actual constant evaluation failure during TLC source conversion.
    /// The original typed error is retained; this is never a model verdict.
    Evaluation {
        /// TLC error class, code, text and expression origin.
        error: EvalError,
        /// Source operand evaluated during conversion.
        source: Box<Source>,
    },
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
            Self::Evaluation { .. } => "DIVE-L0023",
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
            Self::Evaluation { error, source } => {
                write!(f, "{} at {}: {error}", self.code(), source.location)
            }
            Self::Formula { error, source } => {
                write!(f, "{} at {}: {error}", self.code(), source.location)
            }
        }
    }
}
impl std::error::Error for PlanningError {}

struct Lower<'a> {
    tlc_shape: bool,
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

    fn checked_definition(&self, expr: ExprId, def: DefId) -> Result<DefId, PlanningError> {
        if self.model.ir.defs().get(def.index()).is_none() {
            return Err(self.refused(expr, "missing scoped definition"));
        }
        let target = self.model.evaluator.redirected(def);
        if self.model.ir.defs().get(target.index()).is_none() {
            return Err(self.refused(expr, "missing redirected scoped definition"));
        }
        Ok(target)
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
            Ok(_)
                if self.tlc_shape
                    && matches!(&self.model.ir.nodes()[expr.index()].expr, Expr::Num(_)) =>
            {
                let Expr::Num(text) = &self.model.ir.nodes()[expr.index()].expr else {
                    return Err(self.refused(expr, "constant expression shape changed"));
                };
                Err(PlanningError::Evaluation {
                    error: EvalError::coded(
                        Ec::ExpectedValue,
                        format!("TLC expected a boolean value, but did not find one.\n{text}"),
                    )
                    .at(expr),
                    source: Box::new(Source::new(self.model, expr)),
                })
            }
            Ok(v) => Err(self.refused(
                expr,
                format!(
                    "constant temporal operand is {}, not boolean",
                    v.kind_name()
                ),
            )),
            Err(error) if self.tlc_shape && error.class() == ErrorClass::Eval => {
                Err(PlanningError::Evaluation {
                    error,
                    source: Box::new(Source::new(self.model, expr)),
                })
            }
            Err(e) => Err(self.refused(expr, format!("constant temporal operand: {e}"))),
        }
    }

    // Constants remain value-bound. Pure state/action arguments stay lazy so priming
    // inside the callee still reads the caller's successor, not a frozen value.
    fn scoped_call(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        def: DefId,
        args: &[Arg],
        depth: usize,
    ) -> Result<Scope, PlanningError> {
        self.step()?;
        if depth > 256 || args.len() > self.limits.formula_nodes {
            return Err(PlanningError::formula(
                self.model,
                self.root_expr,
                FormulaError::Limit("scoped call bindings/depth"),
            ));
        }
        let ev = &self.model.evaluator;
        let def = self.checked_definition(expr, def)?;
        let d = self.model.ir.def(def);
        if d.recursive || d.std.is_some() || ev.is_value_override(def) {
            return Err(self.refused(expr, "recursive/overridden/higher-order temporal call"));
        }
        let mut constant = true;
        for arg in args {
            let Arg::Expr { expr: arg, .. } = arg else {
                if let Arg::Op { op, .. } = arg {
                    self.operator_context(expr, op, scope, depth + 1)?;
                }
                constant = false;
                continue;
            };
            let level = self.scoped_level(*arg, scope, depth + 1)?;
            // A pure value/action helper may read an action argument in
            // the actual edge. Temporal helper arguments remain state-level.
            let maximum = if d.level == Level::Temporal {
                Level::State
            } else {
                Level::Action
            };
            if level == Level::Temporal {
                if scope.has_substitutions() {
                    return Err(self.refused(*arg, "temporal argument inside INSTANCE context"));
                }
                self.forwarded_context(*arg, scope, depth + 1)?;
            } else {
                if level > maximum {
                    return Err(self.refused(*arg, "action/temporal operator argument"));
                }
                self.pure_scoped(*arg, scope, depth + 1)?;
            }
            constant &= level == Level::Constant;
        }
        if constant {
            let values = args
                .iter()
                .map(|arg| match arg {
                    Arg::Expr { expr: arg, .. } => {
                        ev.eval_scoped(scope, *arg, &St::constant()).map_err(|e| {
                            self.refused(*arg, format!("constant temporal argument: {e}"))
                        })
                    }
                    Arg::Op { .. } => Err(self.refused(expr, "operator argument in temporal call")),
                })
                .collect::<Result<Vec<_>, _>>()?;
            ev.scope_call_values(scope, def, values)
                .map_err(|e| self.refused(expr, e.to_string()))
        } else {
            ev.scope_apply(scope, def, args)
                .map_err(|e| self.refused(expr, e.to_string()))
        }
    }

    fn operator_context(
        &mut self,
        expr: ExprId,
        operator: &OpRef,
        scope: &Scope,
        depth: usize,
    ) -> Result<(), PlanningError> {
        self.step()?;
        let mut remaining = self.limits.expansion_steps.saturating_sub(self.steps);
        let native =
            self.model
                .evaluator
                .scope_native_operator_analysis(scope, operator, &mut remaining);
        self.steps = self.limits.expansion_steps - remaining;
        let (def, analysis) = if native
            .map_err(|e| self.refused(expr, e.to_string()))?
            .is_some()
        {
            (None, scope.clone())
        } else {
            let (def, analysis) = self
                .model
                .evaluator
                .scope_operator_analysis(scope, operator)
                .map_err(|e| self.refused(expr, e.to_string()))?
                .ok_or_else(|| {
                    self.refused(expr, "opaque/recursive/higher-order operator capture")
                })?;
            (Some(def), analysis)
        };
        let mut remaining = self.limits.expansion_steps.saturating_sub(self.steps);
        let captures = self
            .model
            .evaluator
            .scope_operator_captures(&analysis, &mut remaining);
        self.steps = self.limits.expansion_steps - remaining;
        let captures = captures
            .map_err(|e| self.refused(expr, e.to_string()))?
            .ok_or_else(|| self.refused(expr, "opaque captured operator context"))?;
        for (body, captured) in captures {
            self.forwarded_context(body, &captured, depth + 1)?;
        }
        if let Some(def) = def {
            self.forwarded_context(self.model.ir.def(def).body, &analysis, depth + 1)?;
        }
        Ok(())
    }

    // Native actuals retain their original runtime call and scope; they do
    // not acquire a synthetic user body or a temporal/action argument.
    fn native_parameter(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        local: dive_ir::LocalRef,
        args: &[Arg],
        depth: usize,
    ) -> Result<Option<Level>, PlanningError> {
        let mut remaining = self.limits.expansion_steps.saturating_sub(self.steps);
        let native = self.model.evaluator.scope_native_operator_analysis(
            scope,
            &OpRef::Param(local),
            &mut remaining,
        );
        self.steps = self.limits.expansion_steps - remaining;
        let Some(native) = native.map_err(|e| self.refused(expr, e.to_string()))? else {
            return Ok(None);
        };
        if i32::try_from(args.len()).ok() != Some(native.java_arity()) {
            return Err(self.refused(expr, "native operator argument arity"));
        }
        self.operator_context(expr, &OpRef::Param(local), scope, depth + 1)?;
        let mut level = Level::Constant;
        for arg in args {
            let Arg::Expr { expr: body, .. } = arg else {
                return Err(self.refused(expr, "native higher-order argument"));
            };
            self.forwarded_context(*body, scope, depth + 1)?;
            let actual = self.scoped_level(*body, scope, depth + 1)?;
            if actual > Level::State {
                return Err(self.refused(*body, "native action/temporal argument"));
            }
            level = level.max(actual);
        }
        Ok(Some(level))
    }

    fn parameter_call(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        local: dive_ir::LocalRef,
        args: &[Arg],
        depth: usize,
    ) -> Result<(DefId, Scope), PlanningError> {
        self.operator_context(expr, &OpRef::Param(local), scope, depth + 1)?;
        for arg in args {
            let Arg::Expr { expr: body, .. } = arg else {
                return Err(self.refused(expr, "higher-order operator actual"));
            };
            self.forwarded_context(*body, scope, depth + 1)?;
        }
        self.model
            .evaluator
            .scope_apply_param(scope, local, args)
            .ok_or_else(|| self.refused(expr, "opaque operator parameter"))?
            .map_err(|e| self.refused(expr, e.to_string()))
    }

    // Admission proof for a temporal expression argument. Keep its lexical
    // scope; do not force it as a constant or trust a formal's static level.
    // This walk does not interpret temporal truth or materialize binders.
    #[allow(
        clippy::too_many_lines,
        reason = "explicit retained temporal context admission and refusal paths"
    )]
    fn forwarded_context(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<(), PlanningError> {
        let mut todo = vec![(expr, scope.clone(), depth)];
        while let Some((e, sc, depth)) = todo.pop() {
            self.step()?;
            if depth > 256 {
                return Err(PlanningError::formula(
                    self.model,
                    self.root_expr,
                    FormulaError::Limit("forwarded temporal context depth"),
                ));
            }
            let ir = &self.model.ir;
            let ev = &self.model.evaluator;
            let n = ir.nodes().get(e.index()).ok_or_else(|| {
                self.refused(self.root_expr, "missing forwarded temporal expression")
            })?;
            if n.effects.intersects(Effects::IMPURE | Effects::LIVE_TLCGET)
                || ev.scope_shares_side_effects(&sc)
            {
                return Err(self.refused(e, "effectful forwarded temporal context"));
            }
            match &n.expr {
                Expr::Unbound(_) | Expr::Unevaluable(_) => {
                    return Err(self.refused(e, "unbound/unevaluable operator capture"));
                }
                Expr::Local(r) => match ev.scope_slot(&sc, *r) {
                    SlotView::Value(_) | SlotView::AnalysisBound => continue,
                    SlotView::Lazy(body, inner) => {
                        todo.push((body, inner, depth + 1));
                        continue;
                    }
                    _ => return Err(self.refused(e, "opaque forwarded temporal local")),
                },
                Expr::SubstRef { decl, .. } => match ev.scope_subst_slot(&sc, *decl) {
                    SlotView::Value(_) | SlotView::AnalysisBound => continue,
                    SlotView::Lazy(body, inner) => {
                        todo.push((body, inner, depth + 1));
                        continue;
                    }
                    _ => return Err(self.refused(e, "opaque forwarded temporal substitution")),
                },
                Expr::Let { body, .. } => {
                    todo.push((
                        *body,
                        ev.scope_let(&sc, e)
                            .map_err(|err| self.refused(e, err.to_string()))?,
                        depth + 1,
                    ));
                    continue;
                }
                Expr::Subst { body, .. } => {
                    todo.push((
                        *body,
                        ev.scope_subst(&sc, e)
                            .map_err(|err| self.refused(e, err.to_string()))?,
                        depth + 1,
                    ));
                    continue;
                }
                Expr::Quant { body, .. }
                | Expr::FcnCons { body, .. }
                | Expr::SetMap { expr: body, .. }
                | Expr::SetFilter { pred: body, .. } => {
                    let inner = ev
                        .scope_binder_analysis(&sc, e)
                        .map_err(|err| self.refused(e, err.to_string()))?;
                    for c in n.expr.children() {
                        todo.push((
                            c,
                            if c == *body {
                                inner.clone()
                            } else {
                                sc.clone()
                            },
                            depth + 1,
                        ));
                    }
                    continue;
                }
                _ => {}
            }
            let call = match &n.expr {
                Expr::Def(d) => Some((OpRef::Def(*d), &[][..])),
                Expr::Apply { callee, args } => Some((callee.clone(), args.as_ref())),
                _ => None,
            };
            if let Some((op, args)) = call {
                for arg in args {
                    if let Arg::Op { op, .. } = arg {
                        self.operator_context(e, op, &sc, depth + 1)?;
                    }
                }
                match op {
                    OpRef::Param(local) => {
                        if self
                            .native_parameter(e, &sc, local, args, depth + 1)?
                            .is_some()
                        {
                            continue;
                        }
                        let (def, called) = self.parameter_call(e, &sc, local, args, depth + 1)?;
                        todo.push((ir.def(def).body, called, depth + 1));
                        continue;
                    }
                    OpRef::Def(d) => {
                        let d = self.checked_definition(e, d)?;
                        let def = ir.def(d);
                        if def.recursive || ev.is_value_override(d) {
                            return Err(
                                self.refused(e, "recursive/overridden forwarded temporal call")
                            );
                        }
                        if def.std.is_none() {
                            todo.push((
                                def.body,
                                ev.scope_apply(&sc, d, args)
                                    .map_err(|err| self.refused(e, err.to_string()))?,
                                depth + 1,
                            ));
                            continue;
                        }
                        if def.std.is_some_and(|op| {
                            op.impure()
                                || op.creates_runtime_names()
                                || matches!(
                                    op.support(),
                                    dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
                                )
                        }) {
                            return Err(
                                self.refused(e, "native callback/runtime-name operator capture")
                            );
                        }
                        if def.std == Some(StdOp::TlcTLCGet) {
                            return Err(
                                self.refused(e, "checker introspection in temporal argument")
                            );
                        }
                    }
                    OpRef::Std(op)
                        if op.impure()
                            || op.creates_runtime_names()
                            || matches!(
                                op.support(),
                                dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
                            ) =>
                    {
                        return Err(
                            self.refused(e, "native callback/runtime-name operator capture")
                        );
                    }
                    OpRef::Std(StdOp::TlcTLCGet) | OpRef::Const(_) | OpRef::Subst(_) => {
                        return Err(self.refused(e, "opaque/checker forwarded temporal operator"));
                    }
                    _ => {}
                }
            }
            todo.extend(
                n.expr
                    .children()
                    .into_iter()
                    .map(|c| (c, sc.clone(), depth + 1)),
            );
        }
        Ok(())
    }

    // SANY's levels on a formal parameter assume a constant. Follow its actual
    // context before deciding whether an atom is constant, state or action.
    #[allow(
        clippy::too_many_lines,
        reason = "scoped actual levels require explicit structural and ownership refusal cases"
    )]
    fn scoped_level(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<Level, PlanningError> {
        self.step()?;
        if depth > 256 {
            return Err(PlanningError::formula(
                self.model,
                self.root_expr,
                FormulaError::Limit("scoped level depth"),
            ));
        }
        let ir = &self.model.ir;
        let ev = &self.model.evaluator;
        let node = ir
            .nodes()
            .get(expr.index())
            .ok_or_else(|| self.refused(expr, "missing scoped expression"))?;
        match &node.expr {
            Expr::Prime(body) => {
                return Ok(
                    if self.scoped_level(*body, scope, depth + 1)? == Level::Constant {
                        Level::Constant
                    } else {
                        Level::Action
                    },
                );
            }

            Expr::Builtin {
                op:
                    dive_ir::BuiltinOp::Always
                    | dive_ir::BuiltinOp::Eventually
                    | dive_ir::BuiltinOp::LeadsTo,
                ..
            }
            | Expr::Always(_)
            | Expr::Eventually(_)
            | Expr::LeadsTo(_, _)
            | Expr::WhilePlus(_, _)
            | Expr::Wf { .. }
            | Expr::Sf { .. }
            | Expr::TemporalQuant { .. } => return Ok(Level::Temporal),
            Expr::Builtin {
                op: dive_ir::BuiltinOp::Prime | dive_ir::BuiltinOp::Unchanged,
                ..
            }
            | Expr::Var { primed: true, .. }
            | Expr::Unchanged(_)
            | Expr::StepAngle { .. }
            | Expr::StepBox { .. } => return Ok(Level::Action),

            Expr::Apply {
                callee: OpRef::Param(local),
                args,
            } => {
                if let Some(level) = self.native_parameter(expr, scope, *local, args, depth + 1)? {
                    return Ok(level);
                }
                let (def, called) = self.parameter_call(expr, scope, *local, args, depth + 1)?;
                return self.scoped_level(ir.def(def).body, &called, depth + 1);
            }
            Expr::Subst { body, .. } => {
                let sc = ev
                    .scope_subst(scope, expr)
                    .map_err(|e| self.refused(expr, e.to_string()))?;
                return self.scoped_level(*body, &sc, depth + 1);
            }
            Expr::SubstRef { decl, primed } => {
                let level = match ev.scope_subst_slot(scope, *decl) {
                    SlotView::Value(_) => Level::Constant,
                    SlotView::AnalysisBound => Level::State,
                    SlotView::Lazy(e, sc) => self.scoped_level(e, &sc, depth + 1)?,
                    _ => return Err(self.refused(expr, "opaque substituted level")),
                };
                return Ok(if *primed && level != Level::Constant {
                    Level::Action
                } else {
                    level
                });
            }
            Expr::Let { body, .. } => {
                let sc = ev
                    .scope_let(scope, expr)
                    .map_err(|e| self.refused(expr, e.to_string()))?;
                return self.scoped_level(*body, &sc, depth + 1);
            }
            Expr::Local(r) => {
                return match ev.scope_slot(scope, *r) {
                    SlotView::Value(_) => Ok(Level::Constant),
                    SlotView::AnalysisBound => Ok(Level::State),
                    SlotView::Lazy(e, sc) => self.scoped_level(e, &sc, depth + 1),
                    _ => Err(self.refused(expr, "opaque scoped level")),
                };
            }
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => {
                let d = self.checked_definition(expr, *d)?;
                if ir.def(d).std.is_none() && !ev.is_value_override(d) {
                    let sc = self.scoped_call(expr, scope, d, args, depth + 1)?;
                    return self.scoped_level(ir.def(d).body, &sc, depth + 1);
                }
            }
            Expr::Def(d) => {
                let d = self.checked_definition(expr, *d)?;
                if ir.def(d).std.is_none() && !ev.is_value_override(d) {
                    let sc = self.scoped_call(expr, scope, d, &[], depth + 1)?;
                    return self.scoped_level(ir.def(d).body, &sc, depth + 1);
                }
            }
            Expr::Quant { body, .. } if node.level != Level::Temporal => {
                let (scopes, domain_level) = self.predicate_scopes(expr, scope, depth + 1)?;
                let mut level = node.level.max(domain_level);
                for sc in scopes {
                    level = level.max(self.scoped_level(*body, &sc, depth + 1)?);
                }
                return Ok(level);
            }
            Expr::SetMap { expr: body, .. }
            | Expr::SetFilter { pred: body, .. }
            | Expr::FcnCons { body, .. } => {
                let (scopes, domain_level) = self.predicate_scopes(expr, scope, depth + 1)?;
                let mut level = node.level.max(domain_level);
                for sc in scopes {
                    level = level.max(self.scoped_level(*body, &sc, depth + 1)?);
                }
                return Ok(level);
            }
            _ => {}
        }
        if node.level == Level::Temporal {
            return Ok(Level::Temporal);
        }
        let mut level = node.level;
        for child in node.expr.children() {
            level = level.max(self.scoped_level(child, scope, depth + 1)?);
        }
        Ok(level)
    }

    // Only static analysis uses the unknown bindings. A retained atom keeps
    // its original scope and reads bounded concrete domains at each state/edge.
    fn predicate_scopes(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<(Vec<Scope>, Level), PlanningError> {
        self.step()?;
        let bounds: &[dive_ir::Bound] = match self.model.ir.expr(expr) {
            Expr::Quant { bounds, .. }
            | Expr::SetMap { bounds, .. }
            | Expr::FcnCons { bounds, .. } => bounds,
            Expr::SetFilter { bound, .. } => std::slice::from_ref(bound),
            _ => return Err(self.refused(expr, "predicate scopes require a binder")),
        };
        let mut domain_level = Level::Constant;
        for bound in bounds {
            let dom = bound
                .domain
                .ok_or_else(|| self.refused(expr, "unbounded predicate domain"))?;
            let level = self.scoped_level(dom, scope, depth + 1)?;
            if level > Level::State {
                return Err(self.refused(dom, "action/temporal predicate domain"));
            }
            self.pure_scoped(dom, scope, depth + 1)?;
            domain_level = domain_level.max(level);
        }
        if domain_level == Level::Constant {
            return self
                .finite_scopes(expr, scope, depth + 1)
                .map(|sc| (sc, domain_level));
        }
        self.model
            .evaluator
            .scope_binder_analysis(scope, expr)
            .map(|sc| (vec![sc], domain_level))
            .map_err(|e| self.refused(expr, e.to_string()))
    }

    fn finite_scopes(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<Vec<Scope>, PlanningError> {
        self.step()?;
        let bounds: &[dive_ir::Bound] = match self.model.ir.expr(expr) {
            Expr::Quant { bounds, .. }
            | Expr::SetMap { bounds, .. }
            | Expr::FcnCons { bounds, .. } => bounds,
            Expr::SetFilter { bound, .. } => std::slice::from_ref(bound),
            _ => return Err(self.refused(expr, "finite scopes require a finite binder")),
        };
        for bound in bounds {
            let Some(dom) = bound.domain else {
                return Err(self.refused(expr, "unbounded predicate quantifier"));
            };
            if self.scoped_level(dom, scope, depth + 1)? != Level::Constant {
                return Err(self.refused(dom, "nonconstant predicate quantifier domain"));
            }
            self.pure_scoped(dom, scope, depth + 1)?;
        }
        self.model
            .evaluator
            .scope_bindings_bounded(scope, expr, &St::constant(), self.limits.formula_nodes)
            .map_err(|e| {
                if e.class() == dive_eval::ErrorClass::ResourceLimit {
                    PlanningError::formula(
                        self.model,
                        self.root_expr,
                        FormulaError::Limit("predicate quantifier bindings"),
                    )
                } else {
                    self.refused(expr, format!("predicate quantifier bindings: {e}"))
                }
            })
    }

    fn pure_atom(&mut self, expr: ExprId, scope: &Scope) -> Result<(), PlanningError> {
        self.pure_scoped(expr, scope, 0)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "scoped purity walk and explicit refusal paths"
    )]
    fn pure_scoped(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<(), PlanningError> {
        // The same expression may occur under distinct parameter values. Do not
        // deduplicate by expression ID: every context must be checked.
        let mut todo = vec![(expr, scope.clone(), depth)];
        while let Some((e, scope, depth)) = todo.pop() {
            self.step()?;
            if depth > 256 {
                return Err(PlanningError::formula(
                    self.model,
                    self.root_expr,
                    FormulaError::Limit("scoped predicate depth"),
                ));
            }
            let ir = &self.model.ir;
            let ev = &self.model.evaluator;
            let n = ir
                .nodes()
                .get(e.index())
                .ok_or_else(|| self.refused(e, "missing scoped expression"))?;
            if n.effects
                .intersects(Effects::IMPURE | Effects::TEMPORAL | Effects::USES_ENABLED)
                || ev.scope_shares_side_effects(&scope)
            {
                return Err(self.refused(
                    e,
                    "effectful, temporal or ENABLED operand in a pure predicate atom",
                ));
            }
            match &n.expr {
                Expr::Let { body, .. } => {
                    let sc = ev
                        .scope_let(&scope, e)
                        .map_err(|error| self.refused(e, error.to_string()))?;
                    todo.push((*body, sc, depth + 1));
                    continue;
                }
                Expr::Quant { body, .. } => {
                    let (scopes, _) = self.predicate_scopes(e, &scope, depth + 1)?;
                    todo.extend(scopes.into_iter().map(|sc| (*body, sc, depth + 1)));
                    continue;
                }
                Expr::SetMap { expr: body, .. }
                | Expr::SetFilter { pred: body, .. }
                | Expr::FcnCons { body, .. } => {
                    let (scopes, _) = self.predicate_scopes(e, &scope, depth + 1)?;
                    todo.extend(scopes.into_iter().map(|sc| (*body, sc, depth + 1)));
                    continue;
                }
                Expr::Local(r) => match ev.scope_slot(&scope, *r) {
                    SlotView::Value(_) | SlotView::AnalysisBound => {}
                    SlotView::Lazy(body, sc) => {
                        todo.push((body, sc, depth + 1));
                        continue;
                    }
                    _ => return Err(self.refused(e, "opaque scoped predicate atom")),
                },
                Expr::Subst { body, .. } => {
                    let sc = ev
                        .scope_subst(&scope, e)
                        .map_err(|error| self.refused(e, error.to_string()))?;
                    todo.push((*body, sc, depth + 1));
                    continue;
                }
                Expr::SubstRef { decl, .. } => match ev.scope_subst_slot(&scope, *decl) {
                    SlotView::Value(_) | SlotView::AnalysisBound => {}
                    SlotView::Lazy(body, sc) => {
                        todo.push((body, sc, depth + 1));
                        continue;
                    }
                    _ => return Err(self.refused(e, "opaque substituted predicate atom")),
                },
                Expr::Const(c) => match ev.constant_binding(*c) {
                    Some(ConstBinding::Value(_)) => {}
                    Some(ConstBinding::Def(d)) => {
                        let d = self.checked_definition(e, d)?;
                        let def = ir.def(d);
                        if def.level != Level::Constant || def.recursive || !def.params.is_empty() {
                            return Err(self.refused(
                                e,
                                "nonconstant/recursive/parameterized constant override",
                            ));
                        }
                        let sc = ev
                            .scope_enter(&scope, d)
                            .map_err(|err| self.refused(e, err.to_string()))?;
                        todo.push((def.body, sc, depth + 1));
                    }
                    None => return Err(self.refused(e, "unbound predicate constant")),
                },
                _ => {}
            }
            let call = match &n.expr {
                Expr::Def(d) => Some((OpRef::Def(*d), &[][..])),
                Expr::Apply { callee, args } => Some((callee.clone(), args.as_ref())),
                _ => None,
            };
            if let Some((op, args)) = call {
                let standard = match op {
                    OpRef::Std(s) => Some(s),
                    OpRef::Def(d) => {
                        let d = self.checked_definition(e, d)?;
                        let def = ir.def(d);
                        if !ev.is_value_override(d) && def.std.is_none() {
                            let sc = self.scoped_call(e, &scope, d, args, depth + 1)?;
                            todo.push((def.body, sc, depth + 1));
                            continue;
                        }
                        def.std
                    }
                    OpRef::Param(local) => {
                        if self
                            .native_parameter(e, &scope, local, args, depth + 1)?
                            .is_some()
                        {
                            continue;
                        }
                        let (def, called) =
                            self.parameter_call(e, &scope, local, args, depth + 1)?;
                        todo.push((ir.def(def).body, called, depth + 1));
                        continue;
                    }
                    OpRef::Const(_) | OpRef::Subst(_) => {
                        return Err(self.refused(e, "dynamically bound predicate operator"));
                    }
                    _ => None,
                };
                if standard == Some(StdOp::TlcTLCGet) {
                    return Err(self.refused(e, "TLCGet requires the liveness search host"));
                }
                if args.iter().any(|a| matches!(a, Arg::Op { .. })) {
                    return Err(self.refused(e, "higher-order predicate argument"));
                }
            }
            todo.extend(
                n.expr
                    .children()
                    .into_iter()
                    .map(|c| (c, scope.clone(), depth + 1)),
            );
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
            binding_limit: self.limits.formula_nodes,
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
        let node = ir
            .nodes()
            .get(expr.index())
            .ok_or_else(|| self.refused(expr, "missing scoped expression"))?;
        if node.effects.intersects(Effects::IMPURE) || ev.scope_shares_side_effects(scope) {
            return Err(self.refused(expr, "effects or dynamic bindings in temporal planning"));
        }
        if let Expr::Subst { body, .. } = &node.expr {
            let sc = ev
                .scope_subst(scope, expr)
                .map_err(|e| self.refused(expr, e.to_string()))?;
            return self.expr(*body, &sc, depth + 1);
        }
        if let Expr::SubstRef {
            decl,
            primed: false,
        } = &node.expr
            && let SlotView::Lazy(body, sc) = ev.scope_subst_slot(scope, *decl)
        {
            return self.expr(body, &sc, depth + 1);
        }
        if let Expr::Let { body, .. } = &node.expr {
            let sc = ev
                .scope_let(scope, expr)
                .map_err(|e| self.refused(expr, e.to_string()))?;
            return self.expr(*body, &sc, depth + 1);
        }
        if let Expr::Local(r) = &node.expr
            && let SlotView::Lazy(body, sc) = ev.scope_slot(scope, *r)
        {
            return self.expr(body, &sc, depth + 1);
        }
        if let Expr::Apply {
            callee: OpRef::Def(d),
            args,
        } = &node.expr
        {
            let d = self.checked_definition(expr, *d)?;
            if ir.def(d).std.is_none() && !ev.is_value_override(d) {
                let sc = self.scoped_call(expr, scope, d, args, depth + 1)?;
                let saved = (self.nodes.len(), self.atoms.len());
                match self.expr(ir.def(d).body, &sc, depth + 1) {
                    // TLC catches actual nonrecursive operator conversion errors,
                    // then retains the original state/action call. Unsupported
                    // features and resource refusals must never use that fallback.
                    Err(PlanningError::Evaluation { .. })
                        if self.tlc_shape
                            && self.scoped_level(expr, scope, depth)? <= Level::Action =>
                    {
                        self.nodes.truncate(saved.0);
                        self.atoms.truncate(saved.1);
                    }
                    result => return result,
                }
            }
        }
        if let Expr::Apply {
            callee: OpRef::Param(local),
            args,
        } = &node.expr
            && self
                .native_parameter(expr, scope, *local, args, depth + 1)?
                .is_none()
        {
            let (def, called) = self.parameter_call(expr, scope, *local, args, depth + 1)?;
            return self.expr(ir.def(def).body, &called, depth + 1);
        }
        let level = self.scoped_level(expr, scope, depth)?;
        if self.tlc_shape
            && let Expr::Not(child) = &node.expr
            && let Expr::Enabled(action) = ir.expr(*child)
        {
            // Pinned OPCODE_lnot falls back to one LNStateAST for this
            // state-level expression. Keep the source ~ENABLED expression,
            // but evaluate its pure action with the existing bounded search.
            self.pure_atom(*action, scope)?;
            return self.special_atom(
                expr,
                scope,
                AtomKind::State,
                AtomTest::NegatedEnabled { action: *action },
            );
        }

        if self.tlc_shape
            && let Expr::If { cond, then, other } = &node.expr
        {
            // Liveness.astToLive attempts the guard at the empty state, then
            // converts guard/then/else in source order before level fallback.
            if self.scoped_level(*cond, scope, depth)? <= Level::Action {
                self.pure_atom(*cond, scope)?;
                match ev.eval_scoped(scope, *cond, &St::constant()) {
                    Ok(Value::Bool(chosen)) => {
                        return self.expr(if chosen { *then } else { *other }, scope, depth + 1);
                    }
                    Err(error) if error.class() != ErrorClass::Eval => {
                        return Err(self.refused(*cond, format!("conditional guard: {error}")));
                    }
                    _ => {}
                }
            }
            if level <= Level::Action {
                let saved = (self.nodes.len(), self.atoms.len());
                self.expr(*cond, scope, depth + 1)?;
                self.expr(*then, scope, depth + 1)?;
                self.expr(*other, scope, depth + 1)?;
                // Only the whole state/action IF is retained by TLC's final
                // level fallback. Temporary branch atoms cannot enter tagging.
                self.nodes.truncate(saved.0);
                self.atoms.truncate(saved.1);
            }
        }
        if self.tlc_shape && level <= Level::Action {
            let children = match &node.expr {
                Expr::Junction { items, .. } => items.to_vec(),
                Expr::Not(child) => vec![*child],
                Expr::Implies(left, right) => vec![*left, *right],
                _ => Vec::new(),
            };
            if !children.is_empty() {
                let saved = (self.nodes.len(), self.atoms.len());
                for child in children {
                    self.expr(child, scope, depth + 1)?;
                }
                self.nodes.truncate(saved.0);
                self.atoms.truncate(saved.1);
            }
        }
        if level <= Level::Action
            && !node
                .effects
                .intersects(Effects::TEMPORAL | Effects::USES_ENABLED)
        {
            self.pure_atom(expr, scope)?;
            if level == Level::Constant {
                let b = self.bool_constant(expr, scope)?;
                return self.push(Node::Bool(b));
            }
            let kind = if level == Level::Action {
                AtomKind::Action
            } else {
                AtomKind::State
            };
            let index = self.atoms.len();
            self.atoms.push(PredicateAtom {
                pred: Pred::new(expr, scope.clone()),
                kind,
                test: AtomTest::Predicate,
                binding_limit: self.limits.formula_nodes,
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
            Expr::Quant { kind, body, .. } if *kind != QuantKind::Choose => {
                let scopes = self.finite_scopes(expr, scope, depth)?;
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
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => {
                let d = self.checked_definition(expr, *d)?;
                let sc = self.scoped_call(expr, scope, d, args, depth)?;
                return self.expr(ir.def(d).body, &sc, depth);
            }
            Expr::Def(d) => {
                let d = self.checked_definition(expr, *d)?;
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
            Expr::If { cond, then, other }
                if self.scoped_level(*cond, scope, depth)? == Level::Constant =>
            {
                self.pure_atom(*cond, scope)?;
                let branch = if self.bool_constant(*cond, scope)? {
                    *then
                } else {
                    *other
                };
                return self.expr(branch, scope, depth);
            }
            Expr::If { cond, then, other } => {
                if self.scoped_level(*cond, scope, depth)? == Level::Action {
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
                if self.scoped_level(*sub, scope, depth)? > Level::State {
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
                if self.tlc_shape && matches!(node.expr, Expr::Wf { .. }) {
                    // Pinned astToLive WF shape: []<>(~ENABLED <A>_v \/ <A>_v).
                    // The language kernel's equivalent disjunction remains unchanged.
                    let disjunction = self.push(Node::Or(vec![disabled, action]))?;
                    let eventual = self.push(Node::Eventually(disjunction))?;
                    return self.push(Node::Always(eventual));
                }
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
/// state-level subscripts, finite constant domains and temporal definitions
/// with pure constant/state arguments, plus action arguments to value helpers. Planning alone
/// does not validate a behavior or enable model-checking liveness.
///
/// # Errors
/// Effects, nested opaque ENABLED, dynamic/opaque scopes, unsupported temporal
/// constructors, constant evaluation failures and explicit construction bounds.
pub fn lower(model: &Model, pred: &Pred, limits: Limits) -> Result<Plan, PlanningError> {
    lower_with_shape(model, pred, limits, false)
}

pub(crate) fn lower_tlc(model: &Model, pred: &Pred, limits: Limits) -> Result<Plan, PlanningError> {
    lower_with_shape(model, pred, limits, true)
}

fn lower_with_shape(
    model: &Model,
    pred: &Pred,
    limits: Limits,
    tlc_shape: bool,
) -> Result<Plan, PlanningError> {
    let mut cx = Lower {
        tlc_shape,
        model,
        limits,
        nodes: Vec::new(),
        atoms: Vec::new(),
        steps: 0,
        root_expr: pred.expr,
    };
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|e| cx.refused(pred.expr, e.to_string()))?;
    model
        .evaluator
        .validate_predicate(&pred.scope, pred.expr)
        .map_err(|e| cx.refused(pred.expr, e.to_string()))?;
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
        tlc_shape: false,
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
