//! Optional, callback-free canonical producer closure for a private row driver.
//!
//! This is not a purity claim about arbitrary deferred providers. Unknown
//! constructors/caches decline before the driver starts graph capture.

use std::collections::BTreeMap;
use std::sync::{Arc, Weak};

use dive_ir::{
    Arg, BuiltinOp, Effects, Expr, ExprId, OpRef, QuantKind, StdKind, StdOp, StdSupport,
};
use dive_value::{SetKind, Value};

use crate::core::Core;
use crate::env::{Env, Ev, Frame, OpVal, StateRef, Sv};
use crate::{ConstBinding, Evaluator, Scope, SlotView, St};

#[cfg(test)]
#[path = "stability_tests.rs"]
mod tests;

mod budgeted;

#[cfg(test)]
#[path = "stability/budgeted_tests.rs"]
mod budgeted_tests;

/// A producer/atom closure checked without evaluation for one evaluator.
///
/// This fact covers only the supplied roots, ordinary cache domain, and the
/// canonical representation capability. A driver must additionally prove its
/// source roots are complete and its host/runtime is privately owned. There is
/// no public constructor and this does not grant temporal error authority.
///
/// ```
/// use dive_eval::{Evaluator, Scope};
/// use dive_ir::ExprId;
/// fn inspect(evaluator: &Evaluator, root: (ExprId, Scope)) {
///     if let Some(fact) = evaluator.canonical_producer_stability(&[root], &[], 1000) {
///         assert!(fact.belongs_to(evaluator));
///     }
/// }
/// # let _ = inspect;
/// ```
#[derive(Clone)]
pub struct ProducerStability {
    owner: Weak<Core>,
}

impl std::fmt::Debug for ProducerStability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProducerStability").finish_non_exhaustive()
    }
}

impl ProducerStability {
    /// Whether this fact belongs to the actual evaluator being inspected.
    #[must_use]
    pub fn belongs_to(&self, evaluator: &Evaluator) -> bool {
        self.owner.ptr_eq(&Arc::downgrade(&evaluator.core))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fact {
    Canonical,
    // Successful enumeration yields canonical elements, but the domain itself
    // is not admitted to reordered predicates, captured values or caches.
    YieldCanonical,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Producer,
    Reordered,
}

struct Guard<'a> {
    evaluator: &'a Evaluator,
    work: usize,
    // Retain real frame owners while memoizing identity; address reuse is not
    // scope equality. These analysis owners never enter the runtime/certificate.
    frames: BTreeMap<usize, Arc<Frame>>,
    abstracts: Vec<Arc<Frame>>,
    active: Vec<(ExprId, Scope, Mode)>,
    control: Option<&'a dyn crate::HostControl>,
    interrupted: Option<crate::EvalError>,
    poll_after: usize,
}

impl Evaluator {
    /// Inspects canonical producer and predicate storage under the run budget.
    ///
    /// This callback-free inspection uses explicit traversal stacks, without a
    /// formula, value-depth or proof-work ceiling. Unknown representation or
    /// transfer semantics still return `None`. An explicit control is polled
    /// when supplied; otherwise the evaluator's installed control is polled.
    /// No provider, expression or native permission callback is invoked.
    /// Like the bounded proof, this grants neither private-host ownership nor
    /// completeness of the supplied producer roots.
    ///
    /// ```no_run
    /// # fn inspect(ev: &dive_eval::Evaluator, root: (dive_ir::ExprId, dive_eval::Scope))
    /// # -> dive_eval::Result<()> {
    /// if let Some(fact) = ev.canonical_producer_stability_budgeted(&[root], &[], None)? {
    ///     assert!(fact.belongs_to(ev));
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// The original typed interruption, or failed traversal allocation.
    pub fn canonical_producer_stability_budgeted(
        &self,
        producers: &[(ExprId, Scope)],
        predicates: &[(ExprId, Scope)],
        control: Option<&dyn crate::HostControl>,
    ) -> crate::Result<Option<ProducerStability>> {
        budgeted::prove(self, producers, predicates, control)
    }

    /// Inspects a complete producer/atom closure without evaluating an input.
    ///
    /// All supplied scopes must be genuine scopes of this evaluator. Unknown
    /// providers, pending normalization, unproved code, and bounded proof work
    /// return `None`; callers retain their old route before any new atom call.
    /// This supplies no host ownership or complete-model claim by itself.
    #[must_use]
    pub fn canonical_producer_stability(
        &self,
        producers: &[(ExprId, Scope)],
        predicates: &[(ExprId, Scope)],
        work: usize,
    ) -> Option<ProducerStability> {
        self.canonical_producer_stability_inner(producers, predicates, work, None)
            .ok()
            .flatten()
    }

    /// The same callback-free value inspection with cooperative proof-work polls.
    /// The only invoked callback is the explicitly supplied control, never a
    /// value provider. An interruption remains typed, not an unknown proof.
    ///
    /// ```no_run
    /// # fn inspect(ev: &dive_eval::Evaluator, root: (dive_ir::ExprId, dive_eval::Scope),
    /// # control: &dyn dive_eval::HostControl) -> dive_eval::Result<()> {
    /// let _ = ev.canonical_producer_stability_controlled(&[root], &[], 1000, control)?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// The original typed evaluator interruption from the supplied control.
    pub fn canonical_producer_stability_controlled(
        &self,
        producers: &[(ExprId, Scope)],
        predicates: &[(ExprId, Scope)],
        work: usize,
        control: &dyn crate::HostControl,
    ) -> crate::Result<Option<ProducerStability>> {
        self.canonical_producer_stability_inner(producers, predicates, work, Some(control))
    }

    fn canonical_producer_stability_inner(
        &self,
        producers: &[(ExprId, Scope)],
        predicates: &[(ExprId, Scope)],
        work: usize,
        control: Option<&dyn crate::HostControl>,
    ) -> crate::Result<Option<ProducerStability>> {
        let mut guard = Guard {
            evaluator: self,
            work,
            frames: BTreeMap::new(),
            abstracts: Vec::new(),
            active: Vec::new(),
            control,
            interrupted: None,
            poll_after: 0,
        };
        let result = (|| {
            for (expr, scope) in producers {
                self.admit_scoped_expr(scope, *expr).ok()?;
                guard.environment(&scope.env, 0)?;
                guard.expression(scope, *expr, Mode::Producer, 0)?;
            }
            for (expr, scope) in predicates {
                self.admit_scoped_expr(scope, *expr).ok()?;
                guard.environment(&scope.env, 0)?;
                guard.expression(scope, *expr, Mode::Reordered, 0)?;
            }
            Some(ProducerStability {
                owner: Arc::downgrade(&self.core),
            })
        })();
        if let Some(error) = guard.interrupted {
            return Err(error);
        }
        if let Some(control) = control {
            control.check().map_err(crate::EvalError::interrupted)?;
        }
        Ok(result)
    }
}

impl Guard<'_> {
    fn step(&mut self, depth: usize) -> Option<()> {
        if self.poll_after == 0 {
            if let Some(control) = self.control
                && let Err(error) = control.check()
            {
                self.interrupted = Some(crate::EvalError::interrupted(error));
                return None;
            }
            self.poll_after = 1024;
        }
        self.poll_after -= 1;
        self.work = self.work.checked_sub(1)?;
        (depth <= 128).then_some(())
    }

    fn value(&mut self, value: &Value, depth: usize) -> Option<()> {
        self.step(depth)?;
        match value {
            Value::Bool(_) | Value::Int(_) | Value::BigInt(_) => Some(()),
            Value::Str(atom) | Value::Model(atom) => self
                .evaluator
                .core
                .names
                .resolve(atom.tok())
                .filter(|known| known.as_str() == atom.as_str())
                .map(|_| ()),
            Value::Fcn(function) => {
                // Physical domain order does not establish TLC FcnRcdValue
                // isNorm. Only literal tuples/records have this first fact.
                if !function.is_tuple_literal() && !function.is_record_literal() {
                    return None;
                }
                if function.as_tuple().is_none()
                    && function.order() != self.evaluator.core.ctx.order
                {
                    return None;
                }
                // Stored iteration preserves literal/general/domain provenance;
                // this is not a TLC isNorm or unchecked lookup certificate.
                for (key, value) in function.iter() {
                    self.value(&key, depth + 1)?;
                    self.value(value, depth + 1)?;
                }
                Some(())
            }
            Value::Set(set) => {
                if set.kind() != SetKind::Enum || set.order() != self.evaluator.core.ctx.order {
                    return None;
                }
                for child in set.stored_elements()? {
                    self.value(child, depth + 1)?;
                }
                Some(())
            }
        }
    }

    fn state(&mut self, state: &StateRef, depth: usize) -> Option<()> {
        self.step(depth)?;
        match state {
            StateRef::Null | StateRef::Empty => Some(()),
            StateRef::Vals(values) => {
                for value in values.iter().flatten() {
                    self.value(value, depth + 1)?;
                }
                Some(())
            }
            // Query-local named stores have a separate ownership/cutoff contract.
            StateRef::Named(_) => None,
        }
    }

    fn environment(&mut self, env: &Env, depth: usize) -> Option<()> {
        self.step(depth)?;
        if env.dynamic.at.is_some() || env.dynamic.cut.is_some() {
            return None;
        }
        let mut dynamic = env.dynamic.subst.as_deref();
        while let Some(link) = dynamic {
            self.step(depth)?;
            self.slot(&link.val, depth + 1)?;
            dynamic = link.next.as_deref();
        }
        let mut frame = Some(&env.frame);
        while let Some(current) = frame {
            self.step(depth)?;
            let identity = Arc::as_ptr(current) as usize;
            if self.frames.insert(identity, current.clone()).is_none() {
                for slot in &current.slots {
                    self.slot(slot, depth + 1)?;
                }
            }
            frame = current.parent.as_ref();
        }
        Some(())
    }

    fn slot(&mut self, slot: &Sv, depth: usize) -> Option<()> {
        self.step(depth)?;
        match slot {
            Sv::Unset => Some(()),
            Sv::Val(value) => self.value(value, depth + 1),
            Sv::AnalysisBound | Sv::AnalysisConstant | Sv::Fcn(_) => None,
            Sv::Thunk(thunk) => {
                self.environment(&thunk.env, depth + 1)?;
                for (value, current, successor) in thunk.stored_caches().into_iter().flatten() {
                    let Ev::V(value) = value else { return None };
                    self.value(&value, depth + 1)?;
                    self.state(&current, depth + 1)?;
                    self.state(&successor, depth + 1)?;
                }
                let scope = self.scope(&thunk.env);
                self.expression(&scope, thunk.expr, Mode::Reordered, depth + 1)
                    .map(|_| ())
            }
            Sv::Op(operator) => {
                let scope = self.scope(&Env {
                    frame: match &**operator {
                        OpVal::Def {
                            parent: Some(parent),
                            ..
                        } => parent.clone(),
                        _ => return self.operator_storage(operator, depth + 1),
                    },
                    dynamic: match &**operator {
                        OpVal::Def {
                            dynamic: Some(dynamic),
                            ..
                        } => dynamic.clone(),
                        _ => crate::env::Dyn::default(),
                    },
                });
                self.environment(&scope.env, depth + 1)?;
                self.operator_storage(operator, depth + 1)
            }
        }
    }

    fn scope(&self, env: &Env) -> Scope {
        Scope {
            owner: self.evaluator.core.ir.clone(),
            names: self.evaluator.core.names.clone(),
            context: Arc::downgrade(&self.evaluator.core),
            env: env.clone(),
            wrapper: None,
        }
    }

    fn operator_storage(&mut self, operator: &OpVal, depth: usize) -> Option<()> {
        self.step(depth)?;
        match operator {
            OpVal::Def {
                def,
                parent,
                dynamic,
            } => {
                if let Some(dynamic) = dynamic {
                    if dynamic.at.is_some() || dynamic.cut.is_some() {
                        return None;
                    }
                    let mut link = dynamic.subst.as_deref();
                    while let Some(binding) = link {
                        self.step(depth)?;
                        self.slot(&binding.val, depth + 1)?;
                        link = binding.next.as_deref();
                    }
                }
                if let Some(parent) = parent {
                    self.environment(
                        &Env {
                            frame: parent.clone(),
                            dynamic: dynamic.clone().unwrap_or_default(),
                        },
                        depth + 1,
                    )?;
                }
                let target = *def;
                let definition = self.evaluator.ir().defs().get(target.index())?;
                if definition.std.is_some() || definition.recursive {
                    return None;
                }
                let base = if let Some(parent) = parent {
                    self.scope(&Env {
                        frame: parent.clone(),
                        dynamic: dynamic.clone().unwrap_or_default(),
                    })
                } else {
                    let mut base = self.evaluator.scope(target).ok()?;
                    if let Some(dynamic) = dynamic {
                        base.env.dynamic = dynamic.clone();
                    }
                    base
                };
                let (actual, called) = self
                    .evaluator
                    .operator_value_analysis(&base, operator.clone())
                    .ok()??;
                self.abstracts.push(called.env.frame.clone());
                self.canonical(
                    &called,
                    self.evaluator.ir().def(actual).body,
                    Mode::Reordered,
                    depth + 1,
                )
            }
            OpVal::Std(op)
                if !op.impure()
                    && !op.creates_runtime_names()
                    && op.support() == StdSupport::Core
                    && op.kind() == StdKind::Value =>
            {
                // Stored native actuals are templates over canonical values.
                // Unproved or domain-producing templates do not acquire a fact.
                let def = self
                    .evaluator
                    .ir()
                    .defs()
                    .iter()
                    .position(|definition| definition.std == Some(*op))?;
                let source = dive_ir::DefId(u32::try_from(def).ok()?);
                let scope = self.evaluator.scope(source).ok()?;
                (self.native(&scope, &OpRef::Std(*op), *op, &[], depth + 1)? == Fact::Canonical)
                    .then_some(())
            }
            OpVal::Const(constant) => match self.evaluator.constant_binding(*constant)? {
                ConstBinding::Def(def) => self.operator_storage(
                    &OpVal::Def {
                        def,
                        parent: None,
                        dynamic: None,
                    },
                    depth + 1,
                ),
                ConstBinding::Value(_) => None,
            },
            OpVal::Builtin(_) | OpVal::Std(_) | OpVal::Unbound(_) => None,
        }
    }

    fn expression(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        self.step(depth)?;
        self.evaluator.admit_scoped_expr(scope, expr).ok()?;
        let effects = self.evaluator.ir().node(expr).effects;
        if effects.intersects(
            Effects::IMPURE | Effects::LIVE_TLCGET | Effects::USES_ENABLED | Effects::TEMPORAL,
        ) {
            return None;
        }
        if self.active.iter().any(|(prior, captured, prior_mode)| {
            *prior == expr
                && *prior_mode == mode
                && Arc::ptr_eq(&captured.env.frame, &scope.env.frame)
                && match (&captured.env.dynamic.subst, &scope.env.dynamic.subst) {
                    (None, None) => true,
                    (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                    _ => false,
                }
        }) {
            return None;
        }
        self.active.push((expr, scope.clone(), mode));
        let result = self.expression_inner(scope, expr, mode, depth + 1);
        self.active.pop();
        result.filter(|fact| mode == Mode::Producer || *fact == Fact::Canonical)
    }

    fn canonical(&mut self, scope: &Scope, expr: ExprId, mode: Mode, depth: usize) -> Option<()> {
        (self.expression(scope, expr, mode, depth)? == Fact::Canonical).then_some(())
    }

    fn all(
        &mut self,
        scope: &Scope,
        expressions: impl IntoIterator<Item = ExprId>,
        mode: Mode,
        depth: usize,
    ) -> Option<()> {
        for expr in expressions {
            self.canonical(scope, expr, mode, depth + 1)?;
        }
        Some(())
    }

    fn expression_inner(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        use Expr as E;
        let evaluator = self.evaluator;
        match evaluator.ir().expr(expr) {
            E::Bool(_) | E::Num(_) | E::Str(_) | E::Var { .. } => Some(Fact::Canonical),
            E::Local(local) => {
                let value = evaluator.scope_slot(scope, *local);
                if matches!(value, SlotView::AnalysisBound | SlotView::AnalysisConstant) {
                    let actual = scope.env.frame_up(local.up)?;
                    return self
                        .abstracts
                        .iter()
                        .any(|frame| Arc::ptr_eq(frame, actual))
                        .then_some(Fact::Canonical);
                }
                self.slot_view(scope, value, mode, depth)
            }
            E::Const(constant) => self.constant(scope, *constant, mode, depth),
            E::Def(def) => self.call(scope, &OpRef::Def(*def), &[], mode, depth),
            E::Apply { callee, args } => self.call(scope, callee, args, mode, depth),
            E::SubstRef { decl, primed } => {
                if let Some(value) = self
                    .evaluator
                    .scope_substitution(scope, *decl, &St::constant(), *primed)
                    .ok()?
                {
                    self.slot_view(scope, value, mode, depth)
                } else {
                    let declaration = self.evaluator.ir().decls().get(decl.index())?;
                    if declaration.variable {
                        Some(Fact::Canonical)
                    } else {
                        self.constant(scope, declaration.constant?, mode, depth)
                    }
                }
            }
            E::Builtin { op, args } => self.builtin(scope, *op, args, mode, depth),
            E::Tuple(_) | E::Record(_) | E::SetEnum(_) | E::Cartesian(_) => {
                self.constructor(scope, expr, mode, depth)
            }
            E::Not(_)
            | E::Prime(_)
            | E::Unchanged(_)
            | E::Selected(_)
            | E::Label { .. }
            | E::Junction { .. }
            | E::Implies(_, _)
            | E::Equiv(_, _) => self.logic(scope, expr, mode, depth),
            E::If { .. } | E::Case { .. } => self.branch(scope, expr, mode, depth),
            E::Let { body, .. } | E::Subst { body, .. } => {
                self.nested_scope(scope, expr, *body, mode, depth)
            }
            E::Quant {
                kind: QuantKind::Forall | QuantKind::Exists,
                ..
            } => self.quantified(scope, expr, mode, depth),
            E::FcnApply { fcn, arg } => {
                self.all(scope, [*fcn, *arg], mode, depth)?;
                Some(Fact::Canonical) // Original checked selection/wraps stay intact.
            }
            E::RecordSelect { record, .. } => {
                self.canonical(scope, *record, mode, depth)?;
                Some(Fact::Canonical)
            }
            E::StepBox { action, sub } | E::StepAngle { action, sub } => {
                self.all(scope, [*action, *sub], mode, depth)?;
                Some(Fact::Canonical)
            }
            // Constructor-specific normalization/provider/reentrancy rules are
            // unproved, so retain the old route rather than guessing a fact.
            E::Decimal { .. }
            | E::Quant {
                kind: QuantKind::Choose,
                ..
            }
            | E::SetFilter { .. }
            | E::SetMap { .. }
            | E::FcnCons { .. }
            | E::RecFcn { .. }
            | E::RecordSet(_)
            | E::FcnSet { .. }
            | E::Except { .. }
            | E::ExceptAt
            | E::Enabled(_)
            | E::Cdot(_, _)
            | E::Always(_)
            | E::Eventually(_)
            | E::LeadsTo(_, _)
            | E::WhilePlus(_, _)
            | E::Wf { .. }
            | E::Sf { .. }
            | E::TemporalQuant { .. }
            | E::Unbound(_)
            | E::Unevaluable(_) => None,
        }
    }

    fn nested_scope(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        body: ExprId,
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        let called = match self.evaluator.ir().expr(expr) {
            Expr::Let { .. } => self.evaluator.scope_eval_let(scope, expr).ok()?,
            Expr::Subst { bindings, .. } => {
                // scope_subst binds strict actuals by evaluation. A callback-
                // free proof must decline before entering that runtime helper.
                if bindings.iter().any(|binding| {
                    matches!(
                        binding.value,
                        Arg::Expr {
                            pass: dive_ir::Pass::Strict,
                            ..
                        }
                    )
                }) {
                    return None;
                }
                self.evaluator.scope_subst(scope, expr).ok()?
            }
            _ => return None,
        };
        self.environment(&called.env, depth)?;
        self.expression(&called, body, mode, depth)
    }

    fn quantified(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        let evaluator = self.evaluator;
        let Expr::Quant { bounds, body, .. } = evaluator.ir().expr(expr) else {
            return None;
        };
        for bound in bounds {
            self.expression(scope, bound.domain?, mode, depth)?;
        }
        let called = evaluator.scope_binder_analysis(scope, expr).ok()?;
        self.abstracts.push(called.env.frame.clone());
        self.canonical(&called, *body, mode, depth)?;
        Some(Fact::Canonical)
    }

    fn constructor(
        &mut self,
        scope: &Scope,
        expr: ExprId,
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        use Expr as E;
        let evaluator = self.evaluator;
        match evaluator.ir().expr(expr) {
            E::Tuple(items) => {
                self.all(scope, items.iter().copied(), mode, depth)?;
                Some(Fact::Canonical)
            }
            E::Record(fields) => {
                self.all(scope, fields.iter().map(|field| field.value), mode, depth)?;
                Some(Fact::Canonical)
            }
            E::SetEnum(items) => {
                self.all(scope, items.iter().copied(), mode, depth)?;
                Some(Fact::YieldCanonical)
            }
            E::Cartesian(items) => {
                for item in items {
                    self.expression(scope, *item, mode, depth)?;
                }
                Some(Fact::YieldCanonical)
            }
            _ => None,
        }
    }

    fn logic(&mut self, scope: &Scope, expr: ExprId, mode: Mode, depth: usize) -> Option<Fact> {
        use Expr as E;
        let evaluator = self.evaluator;
        match evaluator.ir().expr(expr) {
            E::Not(child)
            | E::Prime(child)
            | E::Unchanged(child)
            | E::Selected(child)
            | E::Label { body: child, .. } => {
                self.canonical(scope, *child, mode, depth)?;
                Some(Fact::Canonical)
            }
            E::Junction { items, .. } => {
                self.all(scope, items.iter().copied(), mode, depth)?;
                Some(Fact::Canonical)
            }
            E::Implies(left, right) | E::Equiv(left, right) => {
                self.all(scope, [*left, *right], mode, depth)?;
                Some(Fact::Canonical)
            }
            _ => None,
        }
    }

    fn branch(&mut self, scope: &Scope, expr: ExprId, mode: Mode, depth: usize) -> Option<Fact> {
        use Expr as E;
        let evaluator = self.evaluator;
        match evaluator.ir().expr(expr) {
            E::If { cond, then, other } => {
                self.canonical(scope, *cond, mode, depth)?;
                let left = self.expression(scope, *then, mode, depth)?;
                let right = self.expression(scope, *other, mode, depth)?;
                Some(if left == right {
                    left
                } else {
                    Fact::YieldCanonical
                })
            }
            E::Case { arms, other } => {
                let mut result = Fact::Canonical;
                for arm in arms {
                    self.canonical(scope, arm.guard, mode, depth)?;
                    if self.expression(scope, arm.value, mode, depth)? != Fact::Canonical {
                        result = Fact::YieldCanonical;
                    }
                }
                if let Some(other) = other
                    && self.expression(scope, *other, mode, depth)? != Fact::Canonical
                {
                    result = Fact::YieldCanonical;
                }
                Some(result)
            }
            _ => None,
        }
    }

    fn slot_view(
        &mut self,
        _scope: &Scope,
        value: SlotView,
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        match value {
            SlotView::Value(value) => {
                self.value(&value, depth)?;
                Some(Fact::Canonical)
            }
            SlotView::Lazy(body, captured) => self.expression(&captured, body, mode, depth),
            _ => None,
        }
    }

    fn constant(
        &mut self,
        scope: &Scope,
        constant: dive_ir::ConstId,
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        match self.evaluator.constant_binding(constant)? {
            ConstBinding::Value(value) => {
                self.value(&value, depth)?;
                Some(Fact::Canonical)
            }
            ConstBinding::Def(def) => self.call(scope, &OpRef::Def(def), &[], mode, depth),
        }
    }

    fn builtin(
        &mut self,
        scope: &Scope,
        op: BuiltinOp,
        args: &[ExprId],
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        use BuiltinOp as B;
        // Gen treats subseteq as assignment from SUBSET(rhs), whose emitted
        // subset representation has not been certified by this capability.
        if mode == Mode::Producer && op == B::Subseteq {
            return None;
        }
        if mode == Mode::Producer && op == B::In {
            self.canonical(scope, *args.first()?, mode, depth)?;
            self.expression(scope, *args.get(1)?, mode, depth)?;
            return Some(Fact::Canonical);
        }
        self.all(scope, args.iter().copied(), mode, depth)?;
        match op {
            B::Eq
            | B::Neq
            | B::In
            | B::NotIn
            | B::Subseteq
            | B::Not
            | B::And
            | B::Or
            | B::Implies
            | B::Equiv
            | B::Prime
            | B::Unchanged
            | B::Boolean => Some(Fact::Canonical),
            B::Times | B::Domain | B::String => Some(Fact::YieldCanonical),
            B::Cup
            | B::Cap
            | B::SetMinus
            | B::PowerSet
            | B::BigUnion
            | B::Enabled
            | B::Cdot
            | B::Always
            | B::Eventually
            | B::LeadsTo
            | B::WhilePlus => None,
        }
    }

    fn call(
        &mut self,
        scope: &Scope,
        operator: &OpRef,
        args: &[Arg],
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        self.step(depth)?;
        if matches!(operator, OpRef::Subst(_)) {
            return None;
        }
        for arg in args {
            match arg {
                Arg::Expr { expr, .. } => self.canonical(scope, *expr, mode, depth)?,
                Arg::Op { op, .. } => self.callback(scope, op, depth)?,
            }
        }
        let value = self.evaluator.core.op_value(&scope.env, operator).ok()?;
        self.call_value(scope, operator, &value, args, mode, depth)
    }

    fn call_value(
        &mut self,
        scope: &Scope,
        original: &OpRef,
        value: &OpVal,
        args: &[Arg],
        mode: Mode,
        depth: usize,
    ) -> Option<Fact> {
        self.step(depth)?;
        match value {
            OpVal::Std(op) => {
                if let Some(&target) = self.evaluator.core.std_redirects.get(op) {
                    return self.call_value(
                        scope,
                        &OpRef::Def(target),
                        &OpVal::Def {
                            def: target,
                            parent: None,
                            dynamic: None,
                        },
                        args,
                        mode,
                        depth + 1,
                    );
                }
                self.native(scope, original, *op, args, depth)
            }
            OpVal::Def { def, .. } => {
                let target = *def;
                // Inspect only source-reachable caches, but never hide their
                // original code behind a successful warm value.
                for cached in [
                    self.evaluator.core.startup.get(target.index()),
                    self.evaluator.core.def_cache.get(target.index()),
                ]
                .into_iter()
                .flatten()
                .filter_map(std::sync::OnceLock::get)
                {
                    self.value(cached, depth + 1)?;
                }
                // Value replacement has different precedence at a direct Def
                // use and an OpVal call. No shared-body guess is a certificate.
                if self.evaluator.core.def_values.contains_key(&target) {
                    return None;
                }
                let definition = self.evaluator.ir().defs().get(target.index())?;
                if let Some(op) = definition.std {
                    return self.call_value(
                        scope,
                        original,
                        &OpVal::Std(op),
                        args,
                        mode,
                        depth + 1,
                    );
                }
                if definition.recursive {
                    return None;
                }
                let body = definition.body;
                let called = match original {
                    OpRef::Param(local) => {
                        self.evaluator
                            .scope_apply_param(scope, *local, args)?
                            .ok()?
                            .1
                    }
                    _ if args.is_empty() => self.evaluator.scope_enter(scope, target).ok()?,
                    _ => self.evaluator.scope_apply(scope, target, args).ok()?,
                };
                let fact = self.expression(&called, body, mode, depth + 1)?;
                if self.evaluator.core.cacheable(target) && fact != Fact::Canonical {
                    return None;
                }
                Some(fact)
            }
            OpVal::Const(constant) => match self.evaluator.constant_binding(*constant)? {
                ConstBinding::Value(value) if args.is_empty() => {
                    self.value(&value, depth)?;
                    Some(Fact::Canonical)
                }
                ConstBinding::Def(_) | ConstBinding::Value(_) => None,
            },
            OpVal::Builtin(_) | OpVal::Unbound(_) => None,
        }
    }

    fn callback(&mut self, scope: &Scope, operator: &OpRef, depth: usize) -> Option<()> {
        self.step(depth)?;
        let value = self.evaluator.core.op_value(&scope.env, operator).ok()?;
        self.operator_storage(&value, depth + 1)?;
        let (def, called) = self
            .evaluator
            .scope_operator_analysis(scope, operator)
            .ok()??;
        self.abstracts.push(called.env.frame.clone());
        let captures = self
            .evaluator
            .scope_operator_captures(&called, &mut self.work)
            .ok()??;
        for (body, captured) in captures {
            self.abstracts.push(captured.env.frame.clone());
            self.canonical(&captured, body, Mode::Reordered, depth + 1)?;
        }
        self.canonical(
            &called,
            self.evaluator.ir().def(def).body,
            Mode::Reordered,
            depth + 1,
        )
    }

    fn native(
        &mut self,
        scope: &Scope,
        original: &OpRef,
        op: StdOp,
        args: &[Arg],
        depth: usize,
    ) -> Option<Fact> {
        use StdOp as S;
        self.step(depth)?;
        if op.impure() || op.support() != StdSupport::Core || op.kind() != StdKind::Value {
            return None;
        }
        if op.creates_runtime_names()
            && !(op == S::SequencesConcat
                && self
                    .evaluator
                    .concat_name_stability(scope, original, args, &mut self.work)
                    .ok()?)
        {
            return None;
        }
        match op {
            S::NaturalsNat
            | S::IntegersNat
            | S::IntegersInt
            | S::StringsSTRING
            | S::NaturalsDotDot
            | S::IntegersDotDot => Some(Fact::YieldCanonical),
            S::NaturalsPlus
            | S::IntegersPlus
            | S::NaturalsMinus
            | S::IntegersMinus
            | S::NaturalsTimes
            | S::IntegersTimes
            | S::NaturalsDiv
            | S::IntegersDiv
            | S::NaturalsMod
            | S::IntegersMod
            | S::NaturalsExpt
            | S::IntegersExpt
            | S::IntegersNeg
            | S::NaturalsLt
            | S::NaturalsLeq
            | S::NaturalsGt
            | S::NaturalsGeq
            | S::IntegersLt
            | S::IntegersLeq
            | S::IntegersGt
            | S::IntegersGeq
            | S::FiniteSetsCardinality
            | S::FiniteSetsIsFiniteSet
            | S::SequencesLen
            | S::SequencesHead
            | S::SequencesTail
            | S::SequencesCons
            | S::SequencesAppend
            | S::SequencesConcat
            | S::SequencesSubSeq
            | S::SequencesSelectSeq
            | S::TlcAssert
            | S::TlcSortSeq => Some(Fact::Canonical),
            // No catch-all purity inference: new/extension kernels need their
            // actual constructor/callback/normalization transfer audited first.
            _ => None,
        }
    }
}
