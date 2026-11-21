//! Three-valued truth of a temporal formula on a finite prefix.
//!
//! A prefix refutes a formula when the formula is false on every infinite
//! behavior that starts with it. Each position carries a `may` bit (some
//! extension can make the subformula true there) and a `must` bit (every
//! extension does). A state predicate is known at every position of the
//! prefix; an action predicate is unknown at the last position, whose
//! successor is not given; `[]`, `<>`, `~>`, `WF` and `SF` see an unknown
//! future after the last position. This is Kleene's strong three-valued
//! logic, so a definite answer holds for every extension: if `may` is false
//! at position 0, no extension satisfies the formula.

use dive_eval::{Scope, SlotView};
use dive_ir::{Expr, ExprId, JunctionKind, Level, OpRef, QuantKind};
use dive_model::{Model, Pred};

use super::{TemporalError, Word, enabled_refusal, substitution_is_pure};
use crate::State;
use crate::enabled::enabled;

/// Per-position truth over the prefix: `must[i]` implies `may[i]`.
struct Tri {
    may: Vec<bool>,
    must: Vec<bool>,
}

impl Tri {
    fn known(values: Vec<bool>) -> Self {
        Self {
            may: values.clone(),
            must: values,
        }
    }

    /// Values of an action-level atom: the last position's step is unknown.
    fn open_last(values: Vec<bool>) -> Self {
        let mut tri = Self::known(values);
        if let (Some(may), Some(must)) = (tri.may.last_mut(), tri.must.last_mut()) {
            *may = true;
            *must = false;
        }
        tri
    }

    fn unknown(n: usize) -> Self {
        Self {
            may: vec![true; n],
            must: vec![false; n],
        }
    }

    fn not(self) -> Self {
        Self {
            may: self.must.into_iter().map(|b| !b).collect(),
            must: self.may.into_iter().map(|b| !b).collect(),
        }
    }

    /// A pointwise monotone Boolean connective.
    fn zip(left: &Self, right: &Self, f: impl Fn(bool, bool) -> bool) -> Self {
        let pairs = |x: &[bool], y: &[bool]| x.iter().zip(y).map(|(a, b)| f(*a, *b)).collect();
        Self {
            may: pairs(&left.may, &right.may),
            must: pairs(&left.must, &right.must),
        }
    }

    fn implies(self, other: &Self) -> Self {
        Self::zip(&self.not(), other, |a, b| a || b)
    }

    /// `[]` (`always`) or `<>` over the prefix followed by an unknown future.
    fn future(self, always: bool) -> Self {
        let n = self.may.len();
        if always {
            let mut may = self.may;
            for i in (0..n.saturating_sub(1)).rev() {
                may[i] = may[i] && may[i + 1];
            }
            Self {
                may,
                must: vec![false; n],
            }
        } else {
            let mut must = self.must;
            for i in (0..n.saturating_sub(1)).rev() {
                must[i] = must[i] || must[i + 1];
            }
            Self {
                may: vec![true; n],
                must,
            }
        }
    }
}

/// Whether the finite prefix `states` refutes `formula`: no infinite
/// extension of it satisfies the formula at its first state. `Ok(false)`
/// means some extension may satisfy it, not that one does. Callers must
/// check separately that the prefix is a behavior prefix of the model.
///
/// ```
/// use std::sync::Arc;
/// use dive_trace::temporal::refutes_prefix;
/// use dive_model::Pred;
/// use dive_value::Value;
/// let src = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
///            Next == x' = x + 1\nP == (x = 0) => [](x < 2)\nQ == <>(x = 5)\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", src, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis)?);
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\n").into_config().unwrap();
/// let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::dive())?;
/// let pred = |name| {
///     let d = model.ir.root_def(name).unwrap();
///     Pred::new(model.ir.def(d).body, model.evaluator.scope(d).expect("scope"))
/// };
/// let states: Vec<_> = (0..3).map(|i| Arc::from([Some(Value::Int(i))])).collect();
/// assert!(refutes_prefix(&model, &pred("P"), &states)?);
/// assert!(!refutes_prefix(&model, &pred("P"), &states[..2])?);
/// assert!(!refutes_prefix(&model, &pred("Q"), &states)?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// The refusals and errors of [`super::evaluate_lasso`].
pub fn refutes_prefix(
    model: &Model,
    formula: &Pred,
    states: &[State],
) -> Result<bool, TemporalError> {
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
    if states.is_empty() {
        return Err(TemporalError::Invalid("an empty prefix".into()));
    }
    if states
        .iter()
        .any(|s| s.len() != model.ir.vars().len() || s.iter().any(Option::is_none))
    {
        return Err(TemporalError::Invalid(
            "a prefix state does not assign every variable".into(),
        ));
    }
    for state in states {
        crate::trace::validate_state_names(model, state).map_err(TemporalError::Unsupported)?;
    }
    let guard = crate::enabled::ComposeGuard::install(model);
    // The last state's own successor stands in for its unknown one; every
    // value read across that step is marked unknown below.
    let mut cx = Word {
        model,
        states,
        back_to: states.len() - 1,
        cells: 0,
    };
    let truth = cx.prefix(formula.expr, &formula.scope, 0);
    if matches!(truth, Err(TemporalError::Interrupted(_))) {
        return truth.map(|_| false);
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
    Ok(!truth?.may[0])
}

impl Word<'_> {
    /// [`Word::eval`]'s constructors in three-valued form.
    #[allow(
        clippy::too_many_lines,
        reason = "one explicit arm per supported temporal constructor, as in eval"
    )]
    fn prefix(&mut self, expr: ExprId, scope: &Scope, depth: usize) -> Result<Tri, TemporalError> {
        self.cells = self.cells.saturating_add(self.states.len());
        if depth > 256 || self.cells > 1_000_000 {
            return Err(TemporalError::Unsupported(
                "temporal validation resource bound".into(),
            ));
        }
        let n = self.states.len();
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
            return self.prefix(*body, &sc, depth + 1);
        }
        if let Expr::SubstRef {
            decl,
            primed: false,
        } = &node.expr
            && let SlotView::Lazy(body, sc) = ev.scope_subst_slot(scope, *decl)
        {
            return self.prefix(body, &sc, depth + 1);
        }
        if let Expr::Let { body, .. } = &node.expr {
            let sc = ev.scope_let(scope, expr).map_err(TemporalError::from)?;
            return self.prefix(*body, &sc, depth + 1);
        }
        if let Expr::Local(r) = &node.expr
            && let SlotView::Lazy(body, sc) = ev.scope_slot(scope, *r)
        {
            return self.prefix(body, &sc, depth + 1);
        }
        if let Expr::Apply {
            callee: OpRef::Def(d),
            args,
        } = &node.expr
        {
            let leaf = self.recursive_leaf(scope, *d);
            let d = self.checked_definition(*d)?;
            if self.model.ir.def(d).std.is_none() && !ev.is_value_override(d) && !leaf {
                let sc = self.scoped_call(scope, d, args, depth + 1, &mut 0)?;
                return self.prefix(self.model.ir.def(d).body, &sc, depth + 1);
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
                return self.prefix_atom(expr, scope, depth);
            }
            let (def, called) = self.parameter_call(scope, *local, args, depth + 1, &mut 0)?;
            return self.prefix(self.model.ir.def(def).body, &called, depth + 1);
        }
        if let Expr::StepAngle { action, sub } | Expr::StepBox { action, sub } = &node.expr {
            return Ok(Tri::open_last(self.temporal_action(
                *action,
                *sub,
                scope,
                matches!(node.expr, Expr::StepBox { .. }),
            )?));
        }
        let level = self.scoped_level(expr, scope, depth, &mut 0)?;
        if level <= Level::Action
            && !node.effects.intersects(dive_ir::Effects::TEMPORAL)
            && (!node.effects.contains(dive_ir::Effects::USES_ENABLED)
                || self.scoped_level_mode(expr, scope, depth, &mut 0, true)? == Level::Action)
        {
            return self.prefix_atom(expr, scope, depth);
        }
        let child = depth + 1;
        match &node.expr {
            Expr::Enabled(action) => Ok(Tri::known(
                self.states
                    .iter()
                    .map(|state| {
                        enabled(self.model, &Pred::new(*action, scope.clone()), None, state)
                            .map_err(|e| enabled_refusal(&e, ""))
                    })
                    .collect::<Result<_, _>>()?,
            )),
            Expr::Always(e) | Expr::Eventually(e) => Ok(self
                .prefix(*e, scope, child)?
                .future(matches!(node.expr, Expr::Always(_)))),
            Expr::Not(e) => Ok(self.prefix(*e, scope, child)?.not()),
            Expr::Implies(a, b) => {
                let a = self.prefix(*a, scope, child)?;
                let b = self.prefix(*b, scope, child)?;
                Ok(a.implies(&b))
            }
            Expr::Equiv(a, b) => {
                let a = self.prefix(*a, scope, child)?;
                let b = self.prefix(*b, scope, child)?;
                let both = Tri::zip(&a, &b, |x, y| x && y);
                let neither = Tri::zip(&a.not(), &b.not(), |x, y| x && y);
                Ok(Tri::zip(&both, &neither, |x, y| x || y))
            }
            Expr::LeadsTo(a, b) => {
                let a = self.prefix(*a, scope, child)?;
                let b = self.prefix(*b, scope, child)?.future(false);
                Ok(a.implies(&b).future(true))
            }
            Expr::Junction { kind, items, .. } => {
                let conjunction = matches!(kind, JunctionKind::And);
                let mut values = Tri::known(vec![conjunction; n]);
                for item in items {
                    let rhs = self.prefix(*item, scope, child)?;
                    values = Tri::zip(
                        &values,
                        &rhs,
                        |a, b| {
                            if conjunction { a && b } else { a || b }
                        },
                    );
                }
                Ok(values)
            }
            Expr::Quant { kind, body, .. } if *kind != QuantKind::Choose => {
                let scopes = self.finite_scopes(expr, scope, child, &mut 0)?;
                let conjunction = *kind == QuantKind::Forall;
                let mut values = Tri::known(vec![conjunction; n]);
                for scope in scopes {
                    let rhs = self.prefix(*body, &scope, child)?;
                    values = Tri::zip(
                        &values,
                        &rhs,
                        |a, b| {
                            if conjunction { a && b } else { a || b }
                        },
                    );
                }
                Ok(values)
            }
            Expr::If { cond, then, other } => {
                if self.scoped_level(*cond, scope, child, &mut 0)? == Level::Action {
                    return Err(TemporalError::Unsupported(
                        "TLC liveness rejects action-level conditional guards".into(),
                    ));
                }
                let guard = self.prefix(*cond, scope, child)?;
                let yes = self.prefix(*then, scope, child)?;
                let no = self.prefix(*other, scope, child)?;
                let mut out = Tri::unknown(n);
                for i in 0..n {
                    out.may[i] = (guard.may[i] && yes.may[i]) || (!guard.must[i] && no.may[i]);
                    out.must[i] = (guard.must[i] && yes.must[i])
                        || (!guard.may[i] && no.must[i])
                        || (yes.must[i] && no.must[i]);
                }
                Ok(out)
            }
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => {
                let d = self.checked_definition(*d)?;
                let scope = self.scoped_call(scope, d, args, child, &mut 0)?;
                self.prefix(self.model.ir.def(d).body, &scope, child)
            }
            Expr::Def(d) => {
                let d = self.checked_definition(*d)?;
                if ev.is_value_override(d) || self.model.ir.def(d).recursive {
                    return Err(TemporalError::Unsupported(
                        "overridden or recursive temporal definition".into(),
                    ));
                }
                let scope = ev.scope_enter(scope, d).map_err(TemporalError::from)?;
                self.prefix(self.model.ir.def(d).body, &scope, child)
            }
            Expr::Selected(e) | Expr::Label { body: e, .. } => self.prefix(*e, scope, child),
            // Fairness constrains only the infinite future.
            Expr::Wf { .. } | Expr::Sf { .. } => Ok(Tri::unknown(n)),
            other => Err(TemporalError::Unsupported(format!(
                "temporal constructor {}",
                other.tag()
            ))),
        }
    }

    fn prefix_atom(&self, expr: ExprId, scope: &Scope, depth: usize) -> Result<Tri, TemporalError> {
        let action = self.scoped_level(expr, scope, depth, &mut 0)? == Level::Action;
        let values = self.atom(expr, scope)?;
        Ok(if action {
            Tri::open_last(values)
        } else {
            Tri::known(values)
        })
    }
}
