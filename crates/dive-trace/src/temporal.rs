//! Direct temporal semantics on an ultimately periodic word.
//!
//! This module has no tableau, graph search, or successor enumerator. Each
//! subformula yields a truth value at every position of the given word.

use dive_eval::{ConstBinding, Scope, SlotView, St};
use dive_ir::{
    Arg, ConstId, DefId, DefScope, Effects, Expr, ExprId, JunctionKind, Level, OpRef, QuantKind,
    StdOp,
};
use dive_model::{Model, Pred};
use dive_value::Value;

use crate::State;
use crate::enabled::enabled;

/// Why temporal evaluation cannot establish a truth value.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemporalError {
    /// A typed cooperative interruption, never a temporal truth value/refusal.
    Interrupted(dive_eval::EvalError),
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
            Self::Interrupted(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for TemporalError {}

impl From<dive_eval::EvalError> for TemporalError {
    fn from(error: dive_eval::EvalError) -> Self {
        Self::evaluation(error, "")
    }
}

impl TemporalError {
    fn evaluation(error: dive_eval::EvalError, context: &str) -> Self {
        if error.class() == dive_eval::ErrorClass::Interrupted {
            Self::Interrupted(error)
        } else {
            Self::Unsupported(format!("{context}{error}"))
        }
    }
}

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
/// Invalid words or nonboolean atoms; side effects, unbounded/dynamic
/// temporal quantification, opaque scoped atoms, checker introspection,
/// resource bounds, or an `ENABLED` the dive-eval evaluator cannot decide. A
/// string or model value created after binding under encounter order
/// refuses ([`crate::RUNTIME_NAMES_UNSUPPORTED`]), whatever the truth.
pub fn evaluate_lasso(
    model: &Model,
    formula: &Pred,
    states: &[State],
    back_to: usize,
) -> Result<Vec<bool>, TemporalError> {
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|error| TemporalError::Unsupported(error.to_string()))?;
    model
        .evaluator
        .validate_predicate(&formula.scope, formula.expr)
        .map_err(TemporalError::from)?;
    if model.has_runtime_names() {
        return Err(TemporalError::Unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED.to_owned(),
        ));
    }
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
    for state in states {
        crate::trace::validate_state_names(model, state).map_err(TemporalError::Unsupported)?;
    }
    let guard = crate::enabled::ComposeGuard::install(model);
    let mut cx = Word {
        model,
        states,
        back_to,
        cells: 0,
    };
    let truth = cx.eval(formula.expr, &formula.scope, 0);
    if matches!(truth, Err(TemporalError::Interrupted(_))) {
        return truth;
    }
    if guard.composed() {
        return Err(TemporalError::Unsupported(
            crate::COMPOSITION_UNSUPPORTED.to_owned(),
        ));
    }
    if model.has_runtime_names() {
        return Err(TemporalError::Unsupported(
            crate::RUNTIME_NAMES_UNSUPPORTED.to_owned(),
        ));
    }
    truth
}

/// Preflight for finite predicates used to certify a periodic behavior.
/// Refuse search-dependent or opaque reads before evaluating anything.
pub(crate) fn check_periodic_context(model: &Model, pred: &Pred) -> Result<(), TemporalError> {
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|error| TemporalError::Unsupported(error.to_string()))?;
    model
        .evaluator
        .validate_predicate(&pred.scope, pred.expr)
        .map_err(TemporalError::from)?;
    if model.ir.nodes().get(pred.expr.index()).is_none() {
        return Err(TemporalError::Unsupported(
            "missing periodic predicate expression".into(),
        ));
    }
    if model.evaluator.source_may_have_effects(pred.expr)
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

/// An `ENABLED` refusal as a temporal refusal. Action composition keeps its
/// feature name, so `ENABLED A` and `[][A]_x` refuse the same composition
/// alike ([`crate::COMPOSITION_UNSUPPORTED`]).
fn enabled_refusal(e: &crate::enabled::EnabledError, context: &str) -> TemporalError {
    match e {
        crate::enabled::EnabledError::Interrupted(error) => {
            TemporalError::Interrupted(error.clone())
        }
        crate::enabled::EnabledError::Unsupported(feature)
            if feature == crate::COMPOSITION_UNSUPPORTED =>
        {
            TemporalError::Unsupported(feature.clone())
        }
        _ => TemporalError::Unsupported(format!("{context}{e}")),
    }
}

/// Entering `INSTANCE ... WITH` evaluates its strict bindings eagerly:
/// refuse a binding from another model's context or with side effects
/// before entering. Names it creates are refused after evaluation
/// ([`crate::RUNTIME_NAMES_UNSUPPORTED`]).
fn substitution_is_pure(
    model: &Model,
    scope: &Scope,
    bindings: &[dive_ir::SubstBinding],
) -> Result<(), TemporalError> {
    let ev = &model.evaluator;
    let pure = ev.validate_scope(scope).is_ok()
        && bindings.iter().all(|binding| {
            model.ir.decls().get(binding.decl.index()).is_some()
                && match binding.value {
                    Arg::Expr { expr, .. } => {
                        ev.validate_predicate(scope, expr).is_ok()
                            && !ev.source_may_have_effects(expr)
                    }
                    Arg::Op { .. } => true,
                }
        });
    if pure {
        Ok(())
    } else {
        Err(TemporalError::Unsupported(
            "INSTANCE substitution with side effects or a foreign context".into(),
        ))
    }
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
        if ev.source_may_have_effects(expr) || ev.scope_shares_side_effects(scope) {
            return Err(TemporalError::Unsupported(
                "side effects or dynamically bound constants in temporal validation".into(),
            ));
        }
        if let Expr::Subst { body, bindings } = &node.expr {
            substitution_is_pure(self.model, scope, bindings)?;
            let sc = ev.scope_subst(scope, expr).map_err(TemporalError::from)?;
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
            let sc = ev.scope_let(scope, expr).map_err(TemporalError::from)?;
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
            let leaf = self.recursive_leaf(scope, *d);
            let d = self.checked_definition(*d)?;
            // A state-level recursive leaf is an atom below, as in TLC.
            if self.model.ir.def(d).std.is_none() && !ev.is_value_override(d) && !leaf {
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
        let level = self.scoped_level(expr, scope, depth, &mut 0)?;
        // Keep established state-level temporal lowering (including direct
        // queries through INSTANCE scopes). Only action-level Boolean bodies
        // need one atom to preserve reached nested-query error ordering.
        if level <= Level::Action
            && !node.effects.intersects(Effects::TEMPORAL)
            && (!node.effects.contains(Effects::USES_ENABLED)
                || self.scoped_level_mode(expr, scope, depth, &mut 0, true)? == Level::Action)
        {
            return self.atom(expr, scope);
        }
        let child = depth + 1;
        match &node.expr {
            Expr::Enabled(action) => self
                .states
                .iter()
                .map(|state| {
                    enabled(self.model, &Pred::new(*action, scope.clone()), None, state)
                        .map_err(|e| enabled_refusal(&e, ""))
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
                let scope = ev.scope_enter(scope, d).map_err(TemporalError::from)?;
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
            let disabled: Vec<bool> = self.states[self.back_to..]
                .iter()
                .map(|state| {
                    enabled(self.model, &action, Some(sub), state)
                        .map(|enabled| !enabled)
                        .map_err(|e| enabled_refusal(&e, "fairness "))
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
        self.check_scoped_context_mode(sub, scope, 0, &mut 0, false)?;
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
            let read = |at: usize| ev.eval_scoped(scope, sub, &St::state(self.states[at].clone()));
            let same = (|| {
                let before = read(i)?;
                let after = read(next)?;
                ev.equal_values(&before, &after)
            })()
            .map_err(|e: dive_eval::EvalError| {
                TemporalError::evaluation(e, &format!("temporal subscript at position {i}: "))
            })?;
            if same {
                out.push(boxed);
                continue;
            }
            let st = St::action(state.clone(), self.states[next].clone());
            match ev.eval_scoped(scope, action, &st) {
                Ok(Value::Bool(b)) => out.push(b),
                Ok(v) => {
                    return Err(TemporalError::Invalid(format!(
                        "temporal action at position {i} is {}, not boolean",
                        v.kind_name()
                    )));
                }
                Err(e) => {
                    return Err(TemporalError::evaluation(
                        e,
                        &format!("temporal action at position {i}: "),
                    ));
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
            match self.model.evaluator.eval_scoped(scope, expr, &st) {
                Ok(Value::Bool(b)) => out.push(b),
                Ok(v) => {
                    return Err(TemporalError::Invalid(format!(
                        "temporal atom at position {i} is {}, not boolean",
                        v.kind_name()
                    )));
                }
                Err(e) => {
                    return Err(TemporalError::evaluation(
                        e,
                        &format!("temporal atom at position {i}: "),
                    ));
                }
            }
        }
        Ok(out)
    }

    // The original declaration is reachable only when substitution lookup is
    // absent, never for an explicitly unset or opaque binding.
    fn declaration_fallback(
        &self,
        scope: &Scope,
        decl: dive_ir::DeclId,
    ) -> Result<Option<Pred>, TemporalError> {
        let declaration = self.model.ir.decl(decl);
        if declaration.variable {
            return Ok(None);
        }
        let constant = declaration.constant.ok_or_else(|| {
            TemporalError::Unsupported("missing original substitution declaration".into())
        })?;
        self.constant_context(scope, constant)
    }

    fn constant_context(
        &self,
        scope: &Scope,
        constant: ConstId,
    ) -> Result<Option<Pred>, TemporalError> {
        let ev = &self.model.evaluator;
        match ev.constant_binding(constant) {
            Some(ConstBinding::Value(_)) => Ok(None),
            Some(ConstBinding::Def(def)) => {
                let def = self.checked_definition(def)?;
                let target = self.model.ir.def(def);
                if target.level != Level::Constant || target.recursive || !target.params.is_empty()
                {
                    return Err(TemporalError::Unsupported(
                        "nonconstant/recursive/parameterized constant override".into(),
                    ));
                }
                if !ev.is_value_override(def) {
                    Self::check_native(target.std)?;
                }
                let scope = ev.scope_enter(scope, def).map_err(TemporalError::from)?;
                Ok(Some(Pred::new(target.body, scope)))
            }
            None => Err(TemporalError::Unsupported(
                "unbound temporal constant".into(),
            )),
        }
    }

    /// Whether a call of `def` in `scope` is a state-level recursive leaf
    /// ([`dive_eval::Evaluator::recursive_leaf`]) outside any INSTANCE
    /// substitution: TLC leaves such a call to evaluation
    /// (`Liveness.astToLive`), so only its actuals carry the caller's context.
    fn recursive_leaf(&self, scope: &Scope, def: DefId) -> bool {
        !scope.has_substitutions() && self.model.evaluator.recursive_leaf(def)
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

    // Resolve the original direct declaration independently of the planner.
    // Return only its lazy call scope; no analysis result is execution authority.
    fn configured_call(
        &self,
        expr: ExprId,
        scope: &Scope,
        constant: ConstId,
        args: &[Arg],
        depth: usize,
        steps: &mut usize,
    ) -> Result<(DefId, Scope), TemporalError> {
        *steps = steps.saturating_add(1);
        if depth > 256 || *steps > 100_000 || args.len() > 4096 {
            return Err(TemporalError::Unsupported(
                "configured temporal call bound".into(),
            ));
        }
        let ev = &self.model.evaluator;
        ev.validate_model_context(&self.model.ir, &self.model.names)
            .map_err(|error| TemporalError::Unsupported(error.to_string()))?;
        ev.validate_predicate(scope, expr)
            .map_err(TemporalError::from)?;
        let Some(ConstBinding::Def(def)) = ev.constant_binding(constant) else {
            return Err(TemporalError::Unsupported(
                "unbound/value-bound temporal atom operator".into(),
            ));
        };
        let def = self.checked_definition(def)?;
        let target = self.model.ir.def(def);
        if self
            .model
            .ir
            .node(target.body)
            .effects
            .contains(Effects::USES_ENABLED)
        {
            return Err(TemporalError::Unsupported(
                "configured ENABLED target".into(),
            ));
        }
        if target.scope != DefScope::TopLevel
            || target.recursive
            || target.std.is_some()
            || ev.is_value_override(def)
            || target.level > Level::Action
            || target.params.iter().any(|p| p.arity != 0)
        {
            return Err(TemporalError::Unsupported(
                "configured temporal atom operator target".into(),
            ));
        }
        let called = ev
            .scope_apply(scope, def, args)
            .map_err(TemporalError::from)?;
        for arg in args {
            let Arg::Expr { expr: actual, .. } = arg else {
                return Err(TemporalError::Unsupported(
                    "higher-order configured temporal argument".into(),
                ));
            };
            self.check_scoped_context_mode(*actual, scope, depth + 1, steps, true)?;
        }
        // Independently inspect the configured target before any domain evaluation;
        // ordinary recursive-source analysis must not erase this call origin.
        self.check_scoped_context_mode(target.body, &called, depth + 1, steps, true)?;
        Ok((def, called))
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
                self.forwarded_context_mode(*expr, scope, depth + 1, steps, true)?;
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
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                let Arg::Expr { expr, .. } = arg else {
                    return Err(TemporalError::Unsupported(
                        "operator temporal argument".into(),
                    ));
                };
                match ev.eval_scoped(scope, *expr, &St::constant()) {
                    Ok(value) => values.push(value),
                    Err(e) if e.is_analysis_only() => {
                        return ev
                            .scope_apply(scope, def, args)
                            .map_err(TemporalError::from);
                    }
                    Err(e) => {
                        return Err(TemporalError::evaluation(e, "constant temporal argument: "));
                    }
                }
            }
            ev.scope_call_values(scope, def, values)
                .map_err(TemporalError::from)
        } else {
            ev.scope_apply(scope, def, args)
                .map_err(TemporalError::from)
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
        let (def, analysis) = if native.map_err(TemporalError::from)?.is_some() {
            (None, scope.clone())
        } else {
            let (def, analysis) = self
                .model
                .evaluator
                .scope_operator_analysis(scope, operator)
                .map_err(TemporalError::from)?
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
            .map_err(TemporalError::from)?
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
        let Some(native) = native.map_err(TemporalError::from)? else {
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
            .map_err(TemporalError::from)
    }

    fn forwarded_context(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
    ) -> Result<(), TemporalError> {
        self.forwarded_context_mode(expr, scope, depth, steps, false)
    }

    // Independently retain queries only in a first-order temporal expression
    // actual. Captures/native/provider paths keep query-free inspection, and
    // actual query or data leaves still pass their original independent guards.
    // Admission proof for a temporal expression argument. Keep its lexical
    // scope; do not force it as a constant or trust a formal's static level.
    // This walk does not interpret temporal truth or materialize binders.
    #[allow(
        clippy::too_many_lines,
        reason = "explicit retained temporal context admission and refusal paths"
    )]
    fn forwarded_context_mode(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
        temporal_actual: bool,
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
            if !temporal_actual && n.effects.contains(Effects::USES_ENABLED) {
                return Err(TemporalError::Unsupported(
                    "ENABLED in forwarded operator capture".into(),
                ));
            }
            match &n.expr {
                Expr::Unbound(_) | Expr::Unevaluable(_) => {
                    return Err(TemporalError::Unsupported(
                        "unbound/unevaluable operator capture".into(),
                    ));
                }
                Expr::Local(r) => match ev.scope_slot(&sc, *r) {
                    SlotView::Value(_) | SlotView::AnalysisBound | SlotView::AnalysisConstant => {
                        continue;
                    }
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
                Expr::SubstRef { decl, primed } => match ev
                    .scope_substitution(&sc, *decl, &St::constant(), *primed)
                    .map_err(TemporalError::from)?
                {
                    Some(
                        SlotView::Value(_) | SlotView::AnalysisBound | SlotView::AnalysisConstant,
                    ) => continue,
                    Some(SlotView::Lazy(body, inner)) => {
                        todo.push((body, inner, depth + 1));
                        continue;
                    }
                    None => {
                        if let Some(pred) = self.declaration_fallback(&sc, *decl)? {
                            todo.push((pred.expr, pred.scope, depth + 1));
                        }
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
                        ev.scope_let(&sc, e).map_err(TemporalError::from)?,
                        depth + 1,
                    ));
                    continue;
                }
                Expr::Subst { body, bindings } => {
                    substitution_is_pure(self.model, &sc, bindings)?;
                    todo.push((
                        *body,
                        ev.scope_subst(&sc, e).map_err(TemporalError::from)?,
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
                        .map_err(TemporalError::from)?;
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
                                ev.scope_apply(&sc, d, args).map_err(TemporalError::from)?,
                                depth + 1,
                            ));
                            continue;
                        }
                        if def.std.is_some_and(|op| {
                            op.impure()
                                || matches!(
                                    op.support(),
                                    dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
                                )
                        }) {
                            return Err(TemporalError::Unsupported(
                                "native callback operator capture".into(),
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
                            || matches!(
                                op.support(),
                                dive_ir::StdSupport::Io | dive_ir::StdSupport::Introspection
                            ) =>
                    {
                        return Err(TemporalError::Unsupported(
                            "native callback operator capture".into(),
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
    fn scoped_level(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
    ) -> Result<Level, TemporalError> {
        self.scoped_level_mode(expr, scope, depth, steps, false)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "independent scoped level resolution and explicit refusal paths"
    )]
    fn scoped_level_mode(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
        query_state: bool,
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
        // Routing only: the query operand is an inner successor namespace.
        // Existing level callers keep their original conservative traversal.
        if query_state && matches!(node.expr, Expr::Enabled(_)) {
            return Ok(Level::State);
        }
        match &node.expr {
            Expr::Prime(body) => {
                return Ok(
                    if self.scoped_level_mode(*body, scope, depth + 1, steps, query_state)?
                        == Level::Constant
                    {
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
                callee: OpRef::Const(constant),
                args,
            } => {
                let (def, called) =
                    self.configured_call(expr, scope, *constant, args, depth + 1, steps)?;
                return self.scoped_level_mode(
                    ir.def(def).body,
                    &called,
                    depth + 1,
                    steps,
                    query_state,
                );
            }
            Expr::Apply {
                callee: OpRef::Param(local),
                args,
            } => {
                if let Some(level) = self.native_parameter(scope, *local, args, depth + 1, steps)? {
                    return Ok(level);
                }
                let (def, called) = self.parameter_call(scope, *local, args, depth + 1, steps)?;
                return self.scoped_level_mode(
                    ir.def(def).body,
                    &called,
                    depth + 1,
                    steps,
                    query_state,
                );
            }
            Expr::Subst { body, bindings } => {
                substitution_is_pure(self.model, scope, bindings)?;
                let sc = ev.scope_subst(scope, expr).map_err(TemporalError::from)?;
                return self.scoped_level_mode(*body, &sc, depth + 1, steps, query_state);
            }
            Expr::SubstRef { decl, primed } => {
                let level = match ev
                    .scope_substitution(scope, *decl, &St::constant(), *primed)
                    .map_err(TemporalError::from)?
                {
                    Some(SlotView::Value(_) | SlotView::AnalysisConstant) => Level::Constant,
                    Some(SlotView::AnalysisBound) => Level::State,
                    Some(SlotView::Lazy(e, sc)) => {
                        self.scoped_level_mode(e, &sc, depth + 1, steps, query_state)?
                    }
                    None => match self.declaration_fallback(scope, *decl)? {
                        Some(pred) => self.scoped_level_mode(
                            pred.expr,
                            &pred.scope,
                            depth + 1,
                            steps,
                            query_state,
                        )?,
                        None if ir.decl(*decl).variable => Level::State,
                        None => Level::Constant,
                    },
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
                let sc = ev.scope_let(scope, expr).map_err(TemporalError::from)?;
                return self.scoped_level_mode(*body, &sc, depth + 1, steps, query_state);
            }
            Expr::Local(r) => {
                return match ev.scope_slot(scope, *r) {
                    SlotView::Value(_) | SlotView::AnalysisConstant => Ok(Level::Constant),
                    SlotView::AnalysisBound => Ok(Level::State),
                    SlotView::Lazy(e, sc) => {
                        self.scoped_level_mode(e, &sc, depth + 1, steps, query_state)
                    }
                    _ => Err(TemporalError::Unsupported(
                        "opaque scoped temporal level".into(),
                    )),
                };
            }
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => {
                let leaf = self.recursive_leaf(scope, *d);
                let d = self.checked_definition(*d)?;
                if leaf {
                    // Its body reaches no substitution, so its own level is
                    // static; the actuals carry the caller's context.
                    let mut level = ir.def(d).level;
                    for arg in args {
                        let Arg::Expr { expr: actual, .. } = arg else {
                            return Err(TemporalError::Unsupported(
                                "operator argument in recursive call".into(),
                            ));
                        };
                        level = level.max(self.scoped_level_mode(
                            *actual,
                            scope,
                            depth + 1,
                            steps,
                            query_state,
                        )?);
                    }
                    return Ok(level);
                }
                if ir.def(d).std.is_none() && !ev.is_value_override(d) {
                    let sc = self.scoped_call(scope, d, args, depth + 1, steps)?;
                    return self.scoped_level_mode(
                        ir.def(d).body,
                        &sc,
                        depth + 1,
                        steps,
                        query_state,
                    );
                }
            }
            Expr::Def(d) => {
                let leaf = self.recursive_leaf(scope, *d);
                let d = self.checked_definition(*d)?;
                if leaf {
                    return Ok(ir.def(d).level);
                }
                if ir.def(d).std.is_none() && !ev.is_value_override(d) {
                    let sc = self.scoped_call(scope, d, &[], depth + 1, steps)?;
                    return self.scoped_level_mode(
                        ir.def(d).body,
                        &sc,
                        depth + 1,
                        steps,
                        query_state,
                    );
                }
            }
            Expr::Quant { body, .. } if node.level != Level::Temporal => {
                let (scopes, domain_level) =
                    self.predicate_scopes(expr, scope, depth + 1, steps)?;
                let mut level = node.level.max(domain_level);
                for sc in scopes {
                    level = level.max(self.scoped_level_mode(
                        *body,
                        &sc,
                        depth + 1,
                        steps,
                        query_state,
                    )?);
                }
                return Ok(level);
            }
            Expr::SetMap { expr: body, .. }
            | Expr::SetFilter { pred: body, .. }
            | Expr::FcnCons { body, .. }
            | Expr::RecFcn { body, .. } => {
                let (scopes, domain_level) =
                    self.predicate_scopes(expr, scope, depth + 1, steps)?;
                let mut level = node.level.max(domain_level);
                for sc in scopes {
                    level = level.max(self.scoped_level_mode(
                        *body,
                        &sc,
                        depth + 1,
                        steps,
                        query_state,
                    )?);
                }
                return Ok(level);
            }
            _ => {}
        }
        if node.level == Level::Temporal {
            return Ok(Level::Temporal);
        }
        if let Expr::Apply { callee, args } = &node.expr {
            let mut work = 100_000_usize.saturating_sub(*steps);
            if let Some((sequence, bodies)) = ev
                .scope_collection_predicate(scope, callee, args, &mut work)
                .map_err(TemporalError::from)?
            {
                *steps = 100_000 - work;
                let mut level = node.level.max(self.scoped_level_mode(
                    sequence,
                    scope,
                    depth + 1,
                    steps,
                    query_state,
                )?);
                for (body, captured) in bodies {
                    let callback_level =
                        self.scoped_level_mode(body, &captured, depth + 1, steps, query_state)?;
                    if callback_level > Level::State {
                        return Err(TemporalError::Unsupported(
                            "SelectSeq action/temporal callback".into(),
                        ));
                    }
                    level = level.max(callback_level);
                }
                return Ok(level);
            }
        }
        let mut level = node.level;
        for child in node.expr.children() {
            level =
                level.max(self.scoped_level_mode(child, scope, depth + 1, steps, query_state)?);
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
            | Expr::FcnCons { bounds, .. }
            | Expr::RecFcn { bounds, .. } => bounds,
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
            let edge_binder = !matches!(self.model.ir.expr(expr), Expr::RecFcn { .. });
            if level > Level::State && !(edge_binder && level == Level::Action) {
                return Err(TemporalError::Unsupported(
                    "action/temporal predicate domain".into(),
                ));
            }
            self.check_scoped_context(dom, scope, depth + 1, steps)?;
            domain_level = domain_level.max(level);
        }
        if domain_level == Level::Constant
            && !matches!(self.model.ir.expr(expr), Expr::RecFcn { .. })
        {
            return self
                .model
                .evaluator
                .scope_constant_binder_analysis(scope, expr)
                .map(|sc| (vec![sc], domain_level))
                .map_err(TemporalError::from);
        }
        if matches!(self.model.ir.expr(expr), Expr::RecFcn { .. }) {
            domain_level = domain_level.max(Level::State);
        }
        if let Expr::RecFcn { body, .. } = self.model.ir.expr(expr) {
            let analysis = self
                .model
                .evaluator
                .scope_binder_analysis(scope, expr)
                .map_err(TemporalError::from)?;
            if self.scoped_level(*body, &analysis, depth + 1, steps)? > Level::State {
                return Err(TemporalError::Unsupported(
                    "action/temporal recursive function capture".into(),
                ));
            }
        }
        self.model
            .evaluator
            .scope_binder_analysis(scope, expr)
            .map(|sc| (vec![sc], domain_level))
            .map_err(TemporalError::from)
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
            | Expr::FcnCons { bounds, .. }
            | Expr::RecFcn { bounds, .. } => bounds,
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
            .map_err(|e| TemporalError::evaluation(e, "temporal quantifier bindings: "))
    }

    fn check_native(op: Option<StdOp>) -> Result<(), TemporalError> {
        if op == Some(StdOp::TlcTLCGet) {
            return Err(TemporalError::Unsupported(
                "TLCGet: checker introspection is not a function of a lasso".into(),
            ));
        }
        Ok(())
    }

    fn check_atom_context(&self, expr: ExprId, scope: &Scope) -> Result<(), TemporalError> {
        self.check_scoped_context_mode(expr, scope, 0, &mut 0, false)
    }

    fn check_scoped_context(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
    ) -> Result<(), TemporalError> {
        self.check_scoped_context_mode(expr, scope, depth, steps, false)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "independent scoped walk; inspection mode never evaluates unused binder domains"
    )]
    fn check_scoped_context_mode(
        &self,
        expr: ExprId,
        scope: &Scope,
        depth: usize,
        steps: &mut usize,
        inspect_only: bool,
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
            if ev.source_may_have_effects(e)
                || meta.effects.intersects(Effects::TEMPORAL)
                || ev.scope_shares_side_effects(&scope)
            {
                return Err(TemporalError::Unsupported(
                    "effectful/temporal operand in periodic predicate".into(),
                ));
            }
            match &meta.expr {
                Expr::StepBox { action, sub } | Expr::StepAngle { action, sub } => {
                    todo.push((*action, scope.clone(), depth + 1));
                    todo.push((*sub, scope.clone(), depth + 1));
                    continue;
                }
                Expr::Enabled(action) => {
                    todo.push((*action, scope.clone(), depth + 1));
                    continue;
                }
                Expr::Not(body) | Expr::Selected(body) | Expr::Label { body, .. } => {
                    todo.push((*body, scope.clone(), depth + 1));
                    continue;
                }
                Expr::Junction { items, .. } => {
                    todo.extend(items.iter().map(|item| (*item, scope.clone(), depth + 1)));
                    continue;
                }
                Expr::Cdot(first, second) => {
                    if ev.options().action_composition != dive_eval::ActionComposition::Tla {
                        return Err(TemporalError::Unsupported(
                            crate::COMPOSITION_UNSUPPORTED.to_owned(),
                        ));
                    }
                    todo.push((*first, scope.clone(), depth + 1));
                    todo.push((*second, scope.clone(), depth + 1));
                    continue;
                }
                Expr::Let { body, .. } => {
                    let sc = ev.scope_let(&scope, e).map_err(TemporalError::from)?;
                    todo.push((*body, sc, depth + 1));
                    continue;
                }
                Expr::Quant { body, .. }
                | Expr::SetMap { expr: body, .. }
                | Expr::SetFilter { pred: body, .. }
                | Expr::FcnCons { body, .. } => {
                    if inspect_only {
                        let sc = ev
                            .scope_binder_analysis(&scope, e)
                            .map_err(TemporalError::from)?;
                        todo.extend(meta.expr.children().into_iter().map(|child| {
                            (
                                child,
                                if child == *body {
                                    sc.clone()
                                } else {
                                    scope.clone()
                                },
                                depth + 1,
                            )
                        }));
                    } else {
                        let (scopes, _) = self.predicate_scopes(e, &scope, depth + 1, steps)?;
                        todo.extend(scopes.into_iter().map(|sc| (*body, sc, depth + 1)));
                    }
                    continue;
                }
                Expr::RecFcn { .. } if inspect_only => {
                    return Err(TemporalError::Unsupported(
                        "recursive function in configured temporal argument".into(),
                    ));
                }
                Expr::RecFcn { body, .. } => {
                    let (scopes, _) = self.predicate_scopes(e, &scope, depth + 1, steps)?;
                    todo.extend(scopes.into_iter().map(|sc| (*body, sc, depth + 1)));
                    continue;
                }
                Expr::Local(r) => match ev.scope_slot(&scope, *r) {
                    SlotView::Function(_) if inspect_only => {
                        return Err(TemporalError::Unsupported(
                            "recursive function in configured temporal context".into(),
                        ));
                    }
                    SlotView::Value(_) | SlotView::AnalysisBound | SlotView::AnalysisConstant => {}
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
                    if inspect_only
                        && bindings.iter().any(|binding| {
                            matches!(
                                binding.value,
                                Arg::Expr {
                                    pass: dive_ir::Pass::Strict,
                                    ..
                                }
                            )
                        })
                    {
                        return Err(TemporalError::Unsupported(
                            "eager substitution in configured temporal argument".into(),
                        ));
                    }
                    substitution_is_pure(self.model, &scope, bindings)?;
                    let sc = ev.scope_subst(&scope, e).map_err(TemporalError::from)?;
                    todo.push((*body, sc, depth + 1));
                    continue;
                }
                Expr::SubstRef { decl, primed } => match ev
                    .scope_substitution(&scope, *decl, &St::constant(), *primed)
                    .map_err(TemporalError::from)?
                {
                    Some(
                        SlotView::Value(_) | SlotView::AnalysisBound | SlotView::AnalysisConstant,
                    ) => {}
                    Some(SlotView::Lazy(body, sc)) => {
                        todo.push((body, sc, depth + 1));
                        continue;
                    }
                    None => {
                        if let Some(pred) = self.declaration_fallback(&scope, *decl)? {
                            todo.push((pred.expr, pred.scope, depth + 1));
                        }
                    }
                    _ => {
                        return Err(TemporalError::Unsupported(
                            "opaque substituted predicate atom".into(),
                        ));
                    }
                },
                Expr::Const(c) => {
                    if let Some(pred) = self.constant_context(&scope, *c)? {
                        todo.push((pred.expr, pred.scope, depth + 1));
                    }
                }
                _ => {}
            }
            let call = match &meta.expr {
                Expr::Def(d) => Some((OpRef::Def(*d), &[][..])),
                Expr::Apply { callee, args } => Some((callee.clone(), args.as_ref())),
                _ => None,
            };
            if let Some((op, args)) = call {
                let standard = match op.clone() {
                    OpRef::Std(s) => Some(s),
                    OpRef::Def(d) => {
                        let leaf = self.recursive_leaf(&scope, d);
                        let d = self.checked_definition(d)?;
                        let def = ir.def(d);
                        if !ev.is_value_override(d) && def.std.is_none() {
                            let sc = if inspect_only {
                                if def.recursive
                                    || args.iter().any(|arg| matches!(arg, Arg::Op { .. }))
                                {
                                    return Err(TemporalError::Unsupported(
                                        "recursive/higher-order configured argument".into(),
                                    ));
                                }
                                todo.extend(args.iter().filter_map(|arg| match arg {
                                    Arg::Expr { expr, .. } => {
                                        Some((*expr, scope.clone(), depth + 1))
                                    }
                                    Arg::Op { .. } => None,
                                }));
                                ev.scope_apply(&scope, d, args)
                                    .map_err(TemporalError::from)?
                            } else if leaf {
                                // A state-level recursive operator is an
                                // evaluation leaf; only its actuals are checked.
                                for arg in args {
                                    let Arg::Expr { expr, .. } = arg else {
                                        return Err(TemporalError::Unsupported(
                                            "operator argument in recursive call".into(),
                                        ));
                                    };
                                    todo.push((*expr, scope.clone(), depth + 1));
                                }
                                continue;
                            } else {
                                self.scoped_call(&scope, d, args, depth + 1, steps)?
                            };
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
                        if inspect_only {
                            return Err(TemporalError::Unsupported(
                                "operator parameter in configured temporal argument".into(),
                            ));
                        }
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
                    OpRef::Const(constant) => {
                        let (def, called) =
                            self.configured_call(e, &scope, constant, args, depth + 1, steps)?;
                        todo.push((ir.def(def).body, called, depth + 1));
                        continue;
                    }
                    OpRef::Subst(_) => {
                        return Err(TemporalError::Unsupported(
                            "dynamically bound temporal atom operator".into(),
                        ));
                    }
                    _ => None,
                };
                Self::check_native(standard)?;
                if inspect_only && args.iter().any(|arg| matches!(arg, Arg::Op { .. })) {
                    return Err(TemporalError::Unsupported(
                        "higher-order configured temporal argument".into(),
                    ));
                }
                if matches!(
                    standard,
                    Some(StdOp::SequencesSelectSeq | StdOp::FiniteSetsExtQuantify)
                ) {
                    let mut work = 100_000_usize.saturating_sub(*steps);
                    let (sequence, bodies) = ev
                        .scope_collection_predicate(&scope, &op, args, &mut work)
                        .map_err(TemporalError::from)?
                        .ok_or_else(|| {
                            TemporalError::Unsupported("SelectSeq callback context/level".into())
                        })?;
                    *steps = 100_000 - work;
                    for (body, captured) in bodies {
                        // Each original captured body is checked for its actual
                        // level and queued for the ordinary pure-source walk.
                        if self.scoped_level(body, &captured, depth + 1, steps)? > Level::State {
                            return Err(TemporalError::Unsupported(
                                "SelectSeq callback context/level".into(),
                            ));
                        }
                        todo.push((body, captured, depth + 1));
                    }
                    todo.push((sequence, scope.clone(), depth + 1));
                    continue;
                }
                if standard == Some(StdOp::FunctionsFoldFunctionOnSet)
                    && let [
                        Arg::Op { op: function, .. },
                        Arg::Expr { .. },
                        Arg::Expr { .. },
                        Arg::Expr { .. },
                    ] = args
                {
                    let mut work = 100_000_usize.saturating_sub(*steps);
                    let native = ev
                        .scope_native_operator_analysis(&scope, function, &mut work)
                        .map_err(TemporalError::from)?;
                    *steps = 100_000 - work;
                    if matches!(native, Some(StdOp::IntegersPlus | StdOp::NaturalsPlus)) {
                        self.operator_context(function, &scope, depth + 1, steps)?;
                        todo.extend(
                            meta.expr
                                .children()
                                .into_iter()
                                .map(|child| (child, scope.clone(), depth + 1)),
                        );
                        continue;
                    }
                }
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
