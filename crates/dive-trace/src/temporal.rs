//! Direct temporal semantics on an ultimately periodic word (ADR 0018 §8).
//!
//! This module has no tableau, graph search, or successor enumerator. Each
//! subformula yields a truth value at every position of the given word.

use dive_eval::{ConstBinding, Scope, SlotView, St};
use dive_ir::{Arg, DefId, Effects, Expr, ExprId, JunctionKind, Level, OpRef, QuantKind, StdOp};
use dive_model::{Model, Pred};
use dive_value::Value;

use crate::State;
use crate::exists::{Decision, Limits, exists_successor};

/// Why temporal evaluation cannot establish a truth value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemporalError {
    /// The word or a predicate is malformed.
    Invalid(String),
    /// A named feature cannot be decided independently yet.
    Unsupported(String),
}

impl std::fmt::Display for TemporalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(s) => write!(f, "invalid temporal word: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported temporal validation: {s}"),
        }
    }
}

impl std::error::Error for TemporalError {}

/// Evaluates `formula` at every position of an ultimately periodic word.
/// The last position is followed by `back_to`; setting it to the last
/// position describes infinite stuttering. This evaluates the formula only:
/// callers must separately check initial states, transitions and constraints.
///
/// ```
/// use std::sync::Arc;
/// use dive_trace::temporal::evaluate_lasso;
/// use dive_model::Pred;
/// use dive_value::Value;
/// let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\n\
///            Next == x' = ~x\nP == []<>x\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", src, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis)?);
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().unwrap();
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc())?;
/// let p = model.ir.root_def("P").unwrap();
/// let pred = Pred::new(model.ir.def(p).body, model.evaluator.scope(p).expect("valid scope"));
/// let states = vec![Arc::from([Some(Value::Bool(true))]),
///                   Arc::from([Some(Value::Bool(false))])];
/// assert_eq!(evaluate_lasso(&model, &pred, &states, 0)?, vec![true, true]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// Invalid words or nonboolean atoms; unsupported fairness whose enabledness
/// lacks a complete finite cover, side effects,
/// unbounded/dynamic temporal quantification, opaque scoped atoms, checker introspection or
/// resource bounds. Direct `ENABLED` uses independent finite covers; opaque
/// embedded enabledness or missing covers remain explicit refusals.
pub fn evaluate_lasso(
    model: &Model,
    formula: &Pred,
    states: &[State],
    back_to: usize,
) -> Result<Vec<bool>, TemporalError> {
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
    model
        .evaluator
        .validate_predicate(&formula.scope, formula.expr)
        .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
    if states.is_empty() || back_to >= states.len() {
        return Err(TemporalError::Invalid("invalid lasso back edge".into()));
    }
    if states
        .iter()
        .any(|s| s.len() != model.ir.vars().len() || s.iter().any(Option::is_none))
    {
        return Err(TemporalError::Invalid(
            "a lasso state does not assign every variable".into(),
        ));
    }
    let mut cx = Word {
        model,
        states,
        back_to,
        cells: 0,
    };
    cx.eval(formula.expr, &formula.scope, 0)
}

/// Preflight for finite predicates used to certify a periodic behavior.
/// Refuse search-dependent or opaque reads before evaluating anything.
pub(crate) fn check_periodic_context(model: &Model, pred: &Pred) -> Result<(), TemporalError> {
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
    model
        .evaluator
        .validate_predicate(&pred.scope, pred.expr)
        .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
    let node = model.ir.nodes().get(pred.expr.index()).ok_or_else(|| {
        TemporalError::Unsupported("missing periodic predicate expression".into())
    })?;
    if node
        .effects
        .intersects(Effects::IMPURE | Effects::USES_ENABLED)
        || model.evaluator.scope_shares_side_effects(&pred.scope)
    {
        return Err(TemporalError::Unsupported(
            "side effects, dynamic bindings or ENABLED in periodic behavior validation".into(),
        ));
    }
    Word {
        model,
        states: &[],
        back_to: 0,
        cells: 0,
    }
    .check_atom_context(pred.expr, &pred.scope)
}

struct Word<'a> {
    model: &'a Model,
    states: &'a [State],
    back_to: usize,
    cells: usize,
}

impl Word<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "one explicit arm per supported temporal constructor"
    )]
    fn eval(
        &mut self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
    ) -> Result<Vec<bool>, TemporalError> {
        self.cells = self.cells.saturating_add(self.states.len());
        if depth > 256 || self.cells > 1_000_000 {
            return Err(TemporalError::Unsupported(
                "temporal validation resource bound".into(),
            ));
        }
        let ev = &self.model.evaluator;
        let node = self
            .model
            .ir
            .nodes()
            .get(expr.index())
            .ok_or_else(|| TemporalError::Unsupported("missing temporal expression".into()))?;
        if node.effects.intersects(Effects::IMPURE) || ev.scope_shares_side_effects(scope) {
            return Err(TemporalError::Unsupported(
                "side effects or dynamically bound constants in temporal validation".into(),
            ));
        }
        if let Expr::Subst { body, bindings } = &node.expr {
            if !crate::context_admission::substitution_is_admitted(self.model, scope, bindings) {
                return Err(TemporalError::Unsupported(
                    crate::context_admission::STRICT_SUBSTITUTION.into(),
                ));
            }
            let sc = ev
                .scope_subst(scope, expr)
                .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
            return self.eval(*body, &sc, depth + 1);
        }
        if let Expr::SubstRef {
            decl,
            primed: false,
        } = &node.expr
            && let SlotView::Lazy(body, sc) = ev.scope_subst_slot(scope, *decl)
        {
            return self.eval(body, &sc, depth + 1);
        }
        if let Expr::Let { body, .. } = &node.expr {
            let sc = ev
                .scope_let(scope, expr)
                .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
            return self.eval(*body, &sc, depth + 1);
        }
        if let Expr::Local(r) = &node.expr
            && let SlotView::Lazy(body, sc) = ev.scope_slot(scope, *r)
        {
            return self.eval(body, &sc, depth + 1);
        }
        if let Expr::Apply {
            callee: OpRef::Def(d),
            args,
        } = &node.expr
        {
            let d = self.checked_definition(*d)?;
            if self.model.ir.def(d).std.is_none() && !ev.is_value_override(d) {
                let sc = self.scoped_call(scope, d, args, depth + 1, &mut 0)?;
                return self.eval(self.model.ir.def(d).body, &sc, depth + 1);
            }
        }
        if let Expr::Apply {
            callee: OpRef::Param(local),
            args,
        } = &node.expr
        {
            if self
                .native_parameter(scope, *local, args, depth + 1, &mut 0)?
                .is_some()
            {
                return self.atom(expr, scope);
            }
            let (def, called) = self.parameter_call(scope, *local, args, depth + 1, &mut 0)?;
            return self.eval(self.model.ir.def(def).body, &called, depth + 1);
        }
        if let Expr::StepAngle { action, sub } | Expr::StepBox { action, sub } = &node.expr {
            return self.temporal_action(
                *action,
                *sub,
                scope,
                matches!(node.expr, Expr::StepBox { .. }),
            );
        }
        if self.scoped_level(expr, scope, depth, &mut 0)? <= Level::Action
            && !node
                .effects
                .intersects(Effects::TEMPORAL | Effects::USES_ENABLED)
        {
            return self.atom(expr, scope);
        }
        let child = depth + 1;
        match &node.expr {
            Expr::Enabled(action) => self
                .states
                .iter()
                .map(|state| {
                    let pred = Pred::new(*action, scope.clone());
                    exists_successor(self.model, &pred, state, None, Limits::default())
                        .map(|d| matches!(d, Decision::Enabled { .. }))
                        .map_err(|e| TemporalError::Unsupported(format!("ENABLED: {e}")))
                })
                .collect(),
            Expr::Always(e) | Expr::Eventually(e) => {
                let always = matches!(node.expr, Expr::Always(_));
                let values = self.eval(*e, scope, child)?;
                Ok(self.future(values, always))
            }
            Expr::Not(e) => Ok(self
                .eval(*e, scope, child)?
                .into_iter()
                .map(|b| !b)
                .collect()),
            Expr::Implies(a, b) | Expr::Equiv(a, b) | Expr::LeadsTo(a, b) => {
                let a = self.eval(*a, scope, child)?;
                let b = self.eval(*b, scope, child)?;
                let equivalent = matches!(node.expr, Expr::Equiv(..));
                let leads = matches!(node.expr, Expr::LeadsTo(..));
                let b = if leads { self.future(b, false) } else { b };
                let values = a
                    .into_iter()
                    .zip(b)
                    .map(|(a, b)| if equivalent { a == b } else { !a || b })
                    .collect();
                Ok(if leads {
                    self.future(values, true)
                } else {
                    values
                })
            }
            Expr::Junction { kind, items, .. } => {
                let conjunction = matches!(kind, JunctionKind::And);
                let mut values = vec![conjunction; self.states.len()];
                for item in items {
                    let rhs = self.eval(*item, scope, child)?;
                    for (a, b) in values.iter_mut().zip(rhs) {
                        *a = if conjunction { *a && b } else { *a || b };
                    }
                }
                Ok(values)
            }
            Expr::Quant { kind, body, .. } if *kind != QuantKind::Choose => {
                let scopes = self.finite_scopes(expr, scope, child, &mut 0)?;
                let conjunction = *kind == QuantKind::Forall;
                let mut values = vec![conjunction; self.states.len()];
                for scope in scopes {
                    let rhs = self.eval(*body, &scope, child)?;
                    for (a, b) in values.iter_mut().zip(rhs) {
                        *a = if conjunction { *a && b } else { *a || b };
                    }
                }
                Ok(values)
            }
            Expr::If { cond, then, other } => {
                if self.scoped_level(*cond, scope, child, &mut 0)? == Level::Action {
                    return Err(TemporalError::Unsupported(
                        "TLC liveness rejects action-level conditional guards".into(),
                    ));
                }
                let guards = self.eval(*cond, scope, child)?;
                let yes = self.eval(*then, scope, child)?;
                let no = self.eval(*other, scope, child)?;
                Ok(guards
                    .into_iter()
                    .zip(yes)
                    .zip(no)
                    .map(|((guard, yes), no)| if guard { yes } else { no })
                    .collect())
            }
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => {
                let d = self.checked_definition(*d)?;
                let scope = self.scoped_call(scope, d, args, child, &mut 0)?;
                self.eval(self.model.ir.def(d).body, &scope, child)
            }
            Expr::Def(d) => {
                let d = self.checked_definition(*d)?;
                if ev.is_value_override(d) || self.model.ir.def(d).recursive {
                    return Err(TemporalError::Unsupported(
                        "overridden or recursive temporal definition".into(),
                    ));
                }
                let scope = ev
                    .scope_enter(scope, d)
                    .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
                self.eval(self.model.ir.def(d).body, &scope, child)
            }
            Expr::Selected(e) | Expr::Label { body: e, .. } => self.eval(*e, scope, child),
            Expr::Wf { sub, action } | Expr::Sf { sub, action } => {
                self.fairness(*action, *sub, scope, matches!(node.expr, Expr::Sf { .. }))
            }
            other => Err(TemporalError::Unsupported(format!(
                "temporal constructor {}",
                other.tag()
            ))),
        }
    }

    // GF <<A>>_v implies both WF_v(A) and SF_v(A). This sufficient
    // condition needs no ENABLED search or witness. Checking only the finite
    // prefix would be unsound: the occurrence must be on the repeated cycle.
    fn fairness(
        &self,
        action: ExprId,
        sub: ExprId,
        scope: &Scope,
        strong: bool,
    ) -> Result<Vec<bool>, TemporalError> {
        let occurrences = self.temporal_action(action, sub, scope, false)?;
        let recurrent = occurrences[self.back_to..].iter().any(|occurs| *occurs);
        if recurrent {
            Ok(vec![true; self.states.len()])
        } else {
            let action = Pred::new(action, scope.clone());
            let sub = Pred::new(sub, scope.clone());
            let disabled: Vec<bool> = self.states[self.back_to..]
                .iter()
                .map(|state| {
                    exists_successor(self.model, &action, state, Some(&sub), Limits::default())
                        .map(|d| matches!(d, Decision::Disabled))
                        .map_err(|e| TemporalError::Unsupported(format!("fairness ENABLED: {e}")))
                })
                .collect::<Result<_, _>>()?;
            // WF = GF(~enabled) OR GF(occurs); SF = FG(~enabled) OR GF(occurs).
            // On this repeated finite cycle GF and FG reduce to any and all.
            let holds = if strong {
                disabled.iter().all(|b| *b)
            } else {
                disabled.iter().any(|b| *b)
            };
            Ok(vec![holds; self.states.len()])
        }
    }

    // Independent LNAction semantics: a subscript stutter determines the
    // value before the body is evaluated, including the actual closing edge.
    fn temporal_action(
        &self,
        action: ExprId,
        sub: ExprId,
        scope: &Scope,
        boxed: bool,
    ) -> Result<Vec<bool>, TemporalError> {
        self.check_atom_context(action, scope)?;
        self.check_atom_context(sub, scope)?;
        if self.scoped_level(sub, scope, 0, &mut 0)? > Level::State {
            return Err(TemporalError::Unsupported(
                "action-level temporal subscript".into(),
            ));
        }
        let ev = &self.model.evaluator;
        let mut out = Vec::with_capacity(self.states.len());
        for (i, state) in self.states.iter().enumerate() {
            let next = if i + 1 == self.states.len() {
                self.back_to
            } else {
                i + 1
            };
            let read = |at: usize| {
                let st = St::state(self.states[at].clone());
                ev.check_finite_bindings(scope, sub, &st, 4096)?;
                ev.eval_scoped(scope, sub, &st)
            };
            let same = (|| {
                let before = read(i)?;
                let after = read(next)?;
                ev.equal_values(&before, &after)
            })()
            .map_err(|e: dive_eval::EvalError| {
                TemporalError::Unsupported(format!("temporal subscript at position {i}: {e}"))
            })?;
            if same {
                out.push(boxed);
                continue;
            }
            let st = St::action(state.clone(), self.states[next].clone());
            ev.check_finite_bindings(scope, action, &st, 4096)
                .map_err(|e| {
                    TemporalError::Unsupported(format!("temporal action at position {i}: {e}"))
                })?;
            match ev.eval_scoped(scope, action, &st) {
                Ok(Value::Bool(b)) => out.push(b),
                Ok(v) => {
                    return Err(TemporalError::Invalid(format!(
                        "temporal action at position {i} is {}, not boolean",
                        v.kind_name()
                    )));
                }
                Err(e) => {
                    return Err(TemporalError::Unsupported(format!(
                        "temporal action at position {i}: {e}"
                    )));
                }
            }
        }
        Ok(out)
    }

    fn atom(&self, expr: ExprId, scope: &Scope) -> Result<Vec<bool>, TemporalError> {
        self.check_atom_context(expr, scope)?;
        let mut out = Vec::with_capacity(self.states.len());
        for (i, state) in self.states.iter().enumerate() {
            let next = if i + 1 == self.states.len() {
                self.back_to
            } else {
                i + 1
            };
            let st = St::action(state.clone(), self.states[next].clone());
            self.model
                .evaluator
                .check_finite_bindings(scope, expr, &st, 4096)
                .map_err(|e| {
                    TemporalError::Unsupported(format!("temporal atom at position {i}: {e}"))
                })?;
            match self.model.evaluator.eval_scoped(scope, expr, &st) {
                Ok(Value::Bool(b)) => out.push(b),
                Ok(v) => {
                    return Err(TemporalError::Invalid(format!(
                        "temporal atom at position {i} is {}, not boolean",
                        v.kind_name()
                    )));
                }
                Err(e) => {
                    return Err(TemporalError::Unsupported(format!(
                        "temporal atom at position {i}: {e}"
                    )));
                }
            }
        }
        Ok(out)
    }

    fn checked_definition(&self, def: DefId) -> Result<DefId, TemporalError> {
        if self.model.ir.defs().get(def.index()).is_none() {
            return Err(TemporalError::Unsupported(
                "missing scoped temporal definition".into(),
            ));
        }
        let target = self.model.evaluator.redirected(def);
        if self.model.ir.defs().get(target.index()).is_none() {
            return Err(TemporalError::Unsupported(
                "missing redirected temporal definition".into(),
            ));
        }
        Ok(target)
    }

    fn scoped_call(
        &self,
        scope: &Scope,
        def: DefId,
        args: &[Arg],
        depth: usize,
        steps: &mut usize,
    ) -> Result<Scope, TemporalError> {
        let ev = &self.model.evaluator;
        let def = self.checked_definition(def)?;
        let d = self.model.ir.def(def);
        if depth > 256
            || args.len() > 4096
            || d.recursive
            || d.std.is_some()
            || ev.is_value_override(def)
        {
            return Err(TemporalError::Unsupported(
                "recursive/overridden/higher-order or bounded temporal call".into(),
            ));
        }
        let mut constant = true;
        for arg in args {
            let Arg::Expr { expr, .. } = arg else {
                if let Arg::Op { op, .. } = arg {
                    self.operator_context(op, scope, depth + 1, steps)?;
                }
                constant = false;
                continue;
            };
            let level = self.scoped_level(*expr, scope, depth + 1, steps)?;
            let maximum = if d.level == Level::Temporal {
                Level::State
            } else {
                Level::Action
            };
            if level == Level::Temporal {
                if scope.has_substitutions() {
                    return Err(TemporalError::Unsupported(
                        "temporal argument inside INSTANCE context".into(),
                    ));
                }
                self.forwarded_context(*expr, scope, depth + 1, steps)?;
            } else {
                if level > maximum {
                    return Err(TemporalError::Unsupported(
                        "action/temporal operator argument".into(),
                    ));
                }
                self.check_scoped_context(*expr, scope, depth + 1, steps)?;
            }
            constant &= level == Level::Constant;
        }
        if constant {
            let values = args
                .iter()
                .map(|arg| match arg {
                    Arg::Expr { expr, .. } => {
                        ev.eval_scoped(scope, *expr, &St::constant()).map_err(|e| {
                            TemporalError::Unsupported(format!("constant temporal argument: {e}"))
                        })
                    }
                    Arg::Op { .. } => Err(TemporalError::Unsupported(
                        "operator temporal argument".into(),
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?;
            ev.scope_call_values(scope, def, values)
                .map_err(|e| TemporalError::Unsupported(e.to_string()))
        } else {
            ev.scope_apply(scope, def, args)
                .map_err(|e| TemporalError::Unsupported(e.to_string()))
        }
    }

    fn operator_context(
        &self,
        operator: &OpRef,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
    ) -> Result<(), TemporalError> {
        *steps = steps.saturating_add(1);
        let mut remaining = 100_000_usize.saturating_sub(*steps);
        let native =
            self.model
                .evaluator
                .scope_native_operator_analysis(scope, operator, &mut remaining);
        *steps = 100_000 - remaining;
        let (def, analysis) = if native
            .map_err(|e| TemporalError::Unsupported(e.to_string()))?
            .is_some()
        {
            (None, scope.clone())
        } else {
            let (def, analysis) = self
                .model
                .evaluator
                .scope_operator_analysis(scope, operator)
                .map_err(|e| TemporalError::Unsupported(e.to_string()))?
                .ok_or_else(|| {
                    TemporalError::Unsupported(
                        "opaque/recursive/higher-order operator capture".into(),
                    )
                })?;
            (Some(def), analysis)
        };
        let mut remaining = 100_000_usize.saturating_sub(*steps);
        let captures = self
            .model
            .evaluator
            .scope_operator_captures(&analysis, &mut remaining);
        *steps = 100_000 - remaining;
        let captures = captures
            .map_err(|e| TemporalError::Unsupported(e.to_string()))?
            .ok_or_else(|| TemporalError::Unsupported("opaque captured operator context".into()))?;
        for (body, captured) in captures {
            self.forwarded_context(body, &captured, depth + 1, steps)?;
        }
        if let Some(def) = def {
            self.forwarded_context(self.model.ir.def(def).body, &analysis, depth + 1, steps)?;
        }
        Ok(())
    }

    // Independently prove the original scoped native call, never a planner
    // annotation or an invented native user body.
    fn native_parameter(
        &self,
        scope: &Scope,
        local: dive_ir::LocalRef,
        args: &[Arg],
        depth: usize,
        steps: &mut usize,
    ) -> Result<Option<Level>, TemporalError> {
        let mut remaining = 100_000_usize.saturating_sub(*steps);
        let native = self.model.evaluator.scope_native_operator_analysis(
            scope,
            &OpRef::Param(local),
            &mut remaining,
        );
        *steps = 100_000 - remaining;
        let Some(native) = native.map_err(|e| TemporalError::Unsupported(e.to_string()))? else {
            return Ok(None);
        };
        if i32::try_from(args.len()).ok() != Some(native.java_arity()) {
            return Err(TemporalError::Unsupported(
                "native operator argument arity".into(),
            ));
        }
        self.operator_context(&OpRef::Param(local), scope, depth + 1, steps)?;
        let mut level = Level::Constant;
        for arg in args {
            let Arg::Expr { expr, .. } = arg else {
                return Err(TemporalError::Unsupported(
                    "native higher-order argument".into(),
                ));
            };
            self.forwarded_context(*expr, scope, depth + 1, steps)?;
            let actual = self.scoped_level(*expr, scope, depth + 1, steps)?;
            if actual > Level::State {
                return Err(TemporalError::Unsupported(
                    "native action/temporal argument".into(),
                ));
            }
            level = level.max(actual);
        }
        Ok(Some(level))
    }

    fn parameter_call(
        &self,
        scope: &Scope,
        local: dive_ir::LocalRef,
        args: &[Arg],
        depth: usize,
        steps: &mut usize,
    ) -> Result<(DefId, Scope), TemporalError> {
        self.operator_context(&OpRef::Param(local), scope, depth + 1, steps)?;
        for arg in args {
            let Arg::Expr { expr, .. } = arg else {
                return Err(TemporalError::Unsupported(
                    "higher-order operator actual".into(),
                ));
            };
            self.forwarded_context(*expr, scope, depth + 1, steps)?;
        }
        self.model
            .evaluator
            .scope_apply_param(scope, local, args)
            .ok_or_else(|| TemporalError::Unsupported("opaque operator parameter".into()))?
            .map_err(|e| TemporalError::Unsupported(e.to_string()))
    }

    // Admission proof for a temporal expression argument. Keep its lexical
    // scope; do not force it as a constant or trust a formal's static level.
    // This walk does not interpret temporal truth or materialize binders.
    #[allow(
        clippy::too_many_lines,
        reason = "explicit retained temporal context admission and refusal paths"
    )]
    fn forwarded_context(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
    ) -> Result<(), TemporalError> {
        let mut todo = vec![(expr, scope.clone(), depth)];
        while let Some((e, sc, depth)) = todo.pop() {
            *steps = steps.saturating_add(1);
            if depth > 256 || *steps > 100_000 {
                return Err(TemporalError::Unsupported(
                    "forwarded temporal context resource bound".into(),
                ));
            }
            let ir = &self.model.ir;
            let ev = &self.model.evaluator;
            let n = ir.nodes().get(e.index()).ok_or_else(|| {
                TemporalError::Unsupported("missing forwarded temporal expression".into())
            })?;
            if n.effects.intersects(Effects::IMPURE | Effects::LIVE_TLCGET)
                || ev.scope_shares_side_effects(&sc)
            {
                return Err(TemporalError::Unsupported(
                    "effectful forwarded temporal context".into(),
                ));
            }
            match &n.expr {
                Expr::Unbound(_) | Expr::Unevaluable(_) => {
                    return Err(TemporalError::Unsupported(
                        "unbound/unevaluable operator capture".into(),
                    ));
                }
                Expr::Local(r) => match ev.scope_slot(&sc, *r) {
                    SlotView::Value(_) | SlotView::AnalysisBound => continue,
                    SlotView::Lazy(body, inner) => {
                        todo.push((body, inner, depth + 1));
                        continue;
                    }
                    _ => {
                        return Err(TemporalError::Unsupported(
                            "opaque forwarded temporal local".into(),
                        ));
                    }
                },
                Expr::SubstRef { decl, .. } => match ev.scope_subst_slot(&sc, *decl) {
                    SlotView::Value(_) | SlotView::AnalysisBound => continue,
                    SlotView::Lazy(body, inner) => {
                        todo.push((body, inner, depth + 1));
                        continue;
                    }
                    _ => {
                        return Err(TemporalError::Unsupported(
                            "opaque forwarded temporal substitution".into(),
                        ));
                    }
                },
                Expr::Let { body, .. } => {
                    todo.push((
                        *body,
                        ev.scope_let(&sc, e)
                            .map_err(|err| TemporalError::Unsupported(err.to_string()))?,
                        depth + 1,
                    ));
                    continue;
                }
                Expr::Subst { body, bindings } => {
                    if !crate::context_admission::substitution_is_admitted(
                        self.model, &sc, bindings,
                    ) {
                        return Err(TemporalError::Unsupported(
                            crate::context_admission::STRICT_SUBSTITUTION.into(),
                        ));
                    }
                    todo.push((
                        *body,
                        ev.scope_subst(&sc, e)
                            .map_err(|err| TemporalError::Unsupported(err.to_string()))?,
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
                        .map_err(|err| TemporalError::Unsupported(err.to_string()))?;
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
                        self.operator_context(op, &sc, depth + 1, steps)?;
                    }
                }
                match op {
                    OpRef::Param(local) => {
                        if self
                            .native_parameter(&sc, local, args, depth + 1, steps)?
                            .is_some()
                        {
                            continue;
                        }
                        let (def, called) =
                            self.parameter_call(&sc, local, args, depth + 1, steps)?;
                        todo.push((ir.def(def).body, called, depth + 1));
                        continue;
                    }
                    OpRef::Def(d) => {
                        let d = self.checked_definition(d)?;
                        let def = ir.def(d);
                        if def.recursive || ev.is_value_override(d) {
                            return Err(TemporalError::Unsupported(
                                "recursive/overridden forwarded temporal call".into(),
                            ));
                        }
                        if def.std.is_none() {
                            todo.push((
                                def.body,
                                ev.scope_apply(&sc, d, args)
                                    .map_err(|err| TemporalError::Unsupported(err.to_string()))?,
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
                            return Err(TemporalError::Unsupported(
                                "native callback/runtime-name operator capture".into(),
                            ));
                        }
                        if def.std == Some(StdOp::TlcTLCGet) {
                            return Err(TemporalError::Unsupported(
                                "checker introspection in temporal argument".into(),
                            ));
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
                        return Err(TemporalError::Unsupported(
                            "native callback/runtime-name operator capture".into(),
                        ));
                    }
                    OpRef::Std(StdOp::TlcTLCGet) | OpRef::Const(_) | OpRef::Subst(_) => {
                        return Err(TemporalError::Unsupported(
                            "opaque/checker forwarded temporal operator".into(),
                        ));
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

    // Check actual argument contexts independently of planner annotations.
    #[allow(
        clippy::too_many_lines,
        reason = "independent scoped level resolution and explicit refusal paths"
    )]
    fn scoped_level(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
    ) -> Result<Level, TemporalError> {
        *steps = steps.saturating_add(1);
        if depth > 256 || *steps > 100_000 {
            return Err(TemporalError::Unsupported(
                "scoped temporal level resource bound".into(),
            ));
        }
        let ev = &self.model.evaluator;
        let ir = &self.model.ir;
        let node = ir.nodes().get(expr.index()).ok_or_else(|| {
            TemporalError::Unsupported("missing scoped temporal expression".into())
        })?;
        match &node.expr {
            Expr::Prime(body) => {
                return Ok(
                    if self.scoped_level(*body, scope, depth + 1, steps)? == Level::Constant {
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
                if let Some(level) = self.native_parameter(scope, *local, args, depth + 1, steps)? {
                    return Ok(level);
                }
                let (def, called) = self.parameter_call(scope, *local, args, depth + 1, steps)?;
                return self.scoped_level(ir.def(def).body, &called, depth + 1, steps);
            }
            Expr::Subst { body, bindings } => {
                if !crate::context_admission::substitution_is_admitted(self.model, scope, bindings)
                {
                    return Err(TemporalError::Unsupported(
                        crate::context_admission::STRICT_SUBSTITUTION.into(),
                    ));
                }
                let sc = ev
                    .scope_subst(scope, expr)
                    .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
                return self.scoped_level(*body, &sc, depth + 1, steps);
            }
            Expr::SubstRef { decl, primed } => {
                let level = match ev.scope_subst_slot(scope, *decl) {
                    SlotView::Value(_) => Level::Constant,
                    SlotView::AnalysisBound => Level::State,
                    SlotView::Lazy(e, sc) => self.scoped_level(e, &sc, depth + 1, steps)?,
                    _ => {
                        return Err(TemporalError::Unsupported(
                            "opaque substituted level".into(),
                        ));
                    }
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
                    .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
                return self.scoped_level(*body, &sc, depth + 1, steps);
            }
            Expr::Local(r) => {
                return match ev.scope_slot(scope, *r) {
                    SlotView::Value(_) => Ok(Level::Constant),
                    SlotView::AnalysisBound => Ok(Level::State),
                    SlotView::Lazy(e, sc) => self.scoped_level(e, &sc, depth + 1, steps),
                    _ => Err(TemporalError::Unsupported(
                        "opaque scoped temporal level".into(),
                    )),
                };
            }
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => {
                let d = self.checked_definition(*d)?;
                if ir.def(d).std.is_none() && !ev.is_value_override(d) {
                    let sc = self.scoped_call(scope, d, args, depth + 1, steps)?;
                    return self.scoped_level(ir.def(d).body, &sc, depth + 1, steps);
                }
            }
            Expr::Def(d) => {
                let d = self.checked_definition(*d)?;
                if ir.def(d).std.is_none() && !ev.is_value_override(d) {
                    let sc = self.scoped_call(scope, d, &[], depth + 1, steps)?;
                    return self.scoped_level(ir.def(d).body, &sc, depth + 1, steps);
                }
            }
            Expr::Quant { body, .. } if node.level != Level::Temporal => {
                let (scopes, domain_level) =
                    self.predicate_scopes(expr, scope, depth + 1, steps)?;
                let mut level = node.level.max(domain_level);
                for sc in scopes {
                    level = level.max(self.scoped_level(*body, &sc, depth + 1, steps)?);
                }
                return Ok(level);
            }
            Expr::SetMap { expr: body, .. }
            | Expr::SetFilter { pred: body, .. }
            | Expr::FcnCons { body, .. } => {
                let (scopes, domain_level) =
                    self.predicate_scopes(expr, scope, depth + 1, steps)?;
                let mut level = node.level.max(domain_level);
                for sc in scopes {
                    level = level.max(self.scoped_level(*body, &sc, depth + 1, steps)?);
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
            level = level.max(self.scoped_level(child, scope, depth + 1, steps)?);
        }
        Ok(level)
    }

    // Analysis binds pure unknowns; evaluation always uses the original scope.
    fn predicate_scopes(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
    ) -> Result<(Vec<Scope>, Level), TemporalError> {
        let bounds: &[dive_ir::Bound] = match self.model.ir.expr(expr) {
            Expr::Quant { bounds, .. }
            | Expr::SetMap { bounds, .. }
            | Expr::FcnCons { bounds, .. } => bounds,
            Expr::SetFilter { bound, .. } => std::slice::from_ref(bound),
            _ => {
                return Err(TemporalError::Unsupported(
                    "predicate scopes require a binder".into(),
                ));
            }
        };
        let mut domain_level = Level::Constant;
        for bound in bounds {
            let dom = bound
                .domain
                .ok_or_else(|| TemporalError::Unsupported("unbounded predicate domain".into()))?;
            let level = self.scoped_level(dom, scope, depth + 1, steps)?;
            if level > Level::State {
                return Err(TemporalError::Unsupported(
                    "action/temporal predicate domain".into(),
                ));
            }
            self.check_scoped_context(dom, scope, depth + 1, steps)?;
            domain_level = domain_level.max(level);
        }
        if domain_level == Level::Constant {
            return self
                .finite_scopes(expr, scope, depth + 1, steps)
                .map(|sc| (sc, domain_level));
        }
        self.model
            .evaluator
            .scope_binder_analysis(scope, expr)
            .map(|sc| (vec![sc], domain_level))
            .map_err(|e| TemporalError::Unsupported(e.to_string()))
    }

    fn finite_scopes(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
    ) -> Result<Vec<Scope>, TemporalError> {
        let bounds: &[dive_ir::Bound] = match self.model.ir.expr(expr) {
            Expr::Quant { bounds, .. }
            | Expr::SetMap { bounds, .. }
            | Expr::FcnCons { bounds, .. } => bounds,
            Expr::SetFilter { bound, .. } => std::slice::from_ref(bound),
            _ => {
                return Err(TemporalError::Unsupported(
                    "finite scopes require a finite binder".into(),
                ));
            }
        };
        for bound in bounds {
            let Some(dom) = bound.domain else {
                return Err(TemporalError::Unsupported(
                    "unbounded predicate quantifier".into(),
                ));
            };
            if self.scoped_level(dom, scope, depth + 1, steps)? != Level::Constant {
                return Err(TemporalError::Unsupported(
                    "nonconstant predicate quantifier domain".into(),
                ));
            }
            self.check_scoped_context(dom, scope, depth + 1, steps)?;
        }
        self.model
            .evaluator
            .scope_bindings_bounded(scope, expr, &St::constant(), 4096)
            .map_err(|e| TemporalError::Unsupported(format!("predicate quantifier bindings: {e}")))
    }

    fn check_native(
        &self,
        op: Option<StdOp>,
        args: &[Arg],
        scope: &Scope,
    ) -> Result<(), TemporalError> {
        if op == Some(StdOp::TlcTLCGet) {
            return Err(TemporalError::Unsupported(
                "TLCGet: checker introspection is not a function of a lasso".into(),
            ));
        }
        let opts = self.model.evaluator.options();
        if (opts.ctx.order == dive_value::ValueOrder::Tlc
            || opts.exploration == dive_eval::ExplorationOrder::Tlc)
            && op.is_some_and(|op| {
                op.creates_runtime_names()
                    && !crate::exists::native_tuple_only(self.model, op, args, scope)
            })
        {
            return Err(TemporalError::Unsupported(
                crate::RUNTIME_NAMES_UNSUPPORTED.to_owned(),
            ));
        }
        Ok(())
    }

    fn check_atom_context(&self, expr: ExprId, scope: &Scope) -> Result<(), TemporalError> {
        self.check_scoped_context(expr, scope, 0, &mut 0)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "independent scoped purity walk and refusal paths"
    )]
    fn check_scoped_context(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
    ) -> Result<(), TemporalError> {
        // ExprId alone is not a context: inspect each distinct application.
        let mut todo = vec![(expr, scope.clone(), depth)];
        while let Some((e, scope, depth)) = todo.pop() {
            *steps = steps.saturating_add(1);
            if depth > 256 || *steps > 100_000 {
                return Err(TemporalError::Unsupported(
                    "scoped temporal purity resource bound".into(),
                ));
            }
            let ev = &self.model.evaluator;
            let ir = &self.model.ir;
            let meta = ir.nodes().get(e.index()).ok_or_else(|| {
                TemporalError::Unsupported("missing scoped periodic expression".into())
            })?;
            if meta
                .effects
                .intersects(Effects::IMPURE | Effects::TEMPORAL | Effects::USES_ENABLED)
                || ev.scope_shares_side_effects(&scope)
            {
                return Err(TemporalError::Unsupported(
                    "effectful/temporal operand in periodic predicate".into(),
                ));
            }
            match &meta.expr {
                Expr::Let { body, .. } => {
                    let sc = ev
                        .scope_let(&scope, e)
                        .map_err(|error| TemporalError::Unsupported(error.to_string()))?;
                    todo.push((*body, sc, depth + 1));
                    continue;
                }
                Expr::Quant { body, .. } => {
                    let (scopes, _) = self.predicate_scopes(e, &scope, depth + 1, steps)?;
                    todo.extend(scopes.into_iter().map(|sc| (*body, sc, depth + 1)));
                    continue;
                }
                Expr::SetMap { expr: body, .. }
                | Expr::SetFilter { pred: body, .. }
                | Expr::FcnCons { body, .. } => {
                    let (scopes, _) = self.predicate_scopes(e, &scope, depth + 1, steps)?;
                    todo.extend(scopes.into_iter().map(|sc| (*body, sc, depth + 1)));
                    continue;
                }
                Expr::Local(r) => match ev.scope_slot(&scope, *r) {
                    SlotView::Value(_) | SlotView::AnalysisBound => {}
                    SlotView::Lazy(e, sc) => {
                        todo.push((e, sc, depth + 1));
                        continue;
                    }
                    _ => {
                        return Err(TemporalError::Unsupported(
                            "opaque scoped atom in temporal validation".into(),
                        ));
                    }
                },
                Expr::Subst { body, bindings } => {
                    if !crate::context_admission::substitution_is_admitted(
                        self.model, &scope, bindings,
                    ) {
                        return Err(TemporalError::Unsupported(
                            crate::context_admission::STRICT_SUBSTITUTION.into(),
                        ));
                    }
                    let sc = ev
                        .scope_subst(&scope, e)
                        .map_err(|error| TemporalError::Unsupported(error.to_string()))?;
                    todo.push((*body, sc, depth + 1));
                    continue;
                }
                Expr::SubstRef { decl, .. } => match ev.scope_subst_slot(&scope, *decl) {
                    SlotView::Value(_) | SlotView::AnalysisBound => {}
                    SlotView::Lazy(body, sc) => {
                        todo.push((body, sc, depth + 1));
                        continue;
                    }
                    _ => {
                        return Err(TemporalError::Unsupported(
                            "opaque substituted predicate atom".into(),
                        ));
                    }
                },
                Expr::Const(c) => match ev.constant_binding(*c) {
                    Some(ConstBinding::Value(_)) => {}
                    Some(ConstBinding::Def(d)) => {
                        let d = self.checked_definition(d)?;
                        let def = ir.def(d);
                        if def.level != Level::Constant || def.recursive || !def.params.is_empty() {
                            return Err(TemporalError::Unsupported(
                                "nonconstant/recursive/parameterized constant override".into(),
                            ));
                        }
                        if !ev.is_value_override(d) {
                            self.check_native(def.std, &[], &scope)?;
                        }
                        let sc = ev
                            .scope_enter(&scope, d)
                            .map_err(|e| TemporalError::Unsupported(e.to_string()))?;
                        todo.push((def.body, sc, depth + 1));
                    }
                    None => {
                        return Err(TemporalError::Unsupported(
                            "unbound temporal constant".into(),
                        ));
                    }
                },
                _ => {}
            }
            let call = match &meta.expr {
                Expr::Def(d) => Some((OpRef::Def(*d), &[][..])),
                Expr::Apply { callee, args } => Some((callee.clone(), args.as_ref())),
                _ => None,
            };
            if let Some((op, args)) = call {
                let standard = match op {
                    OpRef::Std(s) => Some(s),
                    OpRef::Def(d) => {
                        let d = self.checked_definition(d)?;
                        let def = ir.def(d);
                        if !ev.is_value_override(d) && def.std.is_none() {
                            let sc = self.scoped_call(&scope, d, args, depth + 1, steps)?;
                            todo.push((def.body, sc, depth + 1));
                            continue;
                        }
                        if ev.is_value_override(d) {
                            None
                        } else {
                            def.std
                        }
                    }
                    OpRef::Param(local) => {
                        if self
                            .native_parameter(&scope, local, args, depth + 1, steps)?
                            .is_some()
                        {
                            continue;
                        }
                        let (def, called) =
                            self.parameter_call(&scope, local, args, depth + 1, steps)?;
                        todo.push((ir.def(def).body, called, depth + 1));
                        continue;
                    }
                    OpRef::Subst(_) | OpRef::Const(_) => {
                        return Err(TemporalError::Unsupported(
                            "dynamically bound temporal atom operator".into(),
                        ));
                    }
                    _ => None,
                };
                self.check_native(standard, args, &scope)?;
                if args.iter().any(|a| matches!(a, Arg::Op { .. })) {
                    return Err(TemporalError::Unsupported(
                        "higher-order periodic predicate argument".into(),
                    ));
                }
            }
            todo.extend(
                meta.expr
                    .children()
                    .into_iter()
                    .map(|c| (c, scope.clone(), depth + 1)),
            );
        }
        Ok(())
    }

    fn future(&self, mut values: Vec<bool>, always: bool) -> Vec<bool> {
        let cycle = if always {
            values[self.back_to..].iter().all(|b| *b)
        } else {
            values[self.back_to..].iter().any(|b| *b)
        };
        values[self.back_to..].fill(cycle);
        for i in (0..self.back_to).rev() {
            values[i] = if always {
                values[i] && values[i + 1]
            } else {
                values[i] || values[i + 1]
            };
        }
        values
    }
}
