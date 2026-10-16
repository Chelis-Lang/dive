//! Scopes: where the binding stage's predicates are evaluated.
//!
//! TLC keeps every role of a model (an initial predicate, an action, an
//! invariant, an implied action, a constraint) as an `Action`: an expression
//! with the `Context` it is evaluated in (`tlc2.tool.Action`). The context is
//! what `SpecProcessor.processConfigSpec` and `Tool.getActions` built while
//! they walked down to the expression: the parameters of operators they
//! inlined (`getOpContext`, lazily), the values of the `\E` variables they
//! split on, and the instance substitutions they passed (`Specs.addSubsts`).
//!
//! A [`Scope`] is that context over dive's environments: a frame of the
//! definition the expression belongs to (with its parents) and the dynamic
//! substitutions. The binding stage builds scopes with the methods on
//! [`crate::Evaluator`] below and hands each role to the checker as an
//! expression plus a scope.

use std::sync::Arc;

use dive_ir::{Arg, DefId, DefScope, Expr, ExprId, LocalRef, OpRef, Pass, Slot};
use dive_value::Value;

use crate::Evaluator;
use crate::env::{Dyn, Env, Ev, St, Sv, Thunk};
use crate::error::{EvalError, Result};

pub(crate) type BindingColumn = (Vec<usize>, bool, Arc<[Value]>);

/// TLC's `Context` of a role (see the module documentation). Cheap to clone.
#[derive(Clone, Debug)]
pub struct Scope {
    pub(crate) env: Env,
    pub(crate) owner: Arc<dive_ir::Ir>,
    pub(crate) names: Arc<dive_value::Interner>,
    pub(crate) context: std::sync::Weak<crate::core::Core>,
    /// The innermost substitution entered, whose location labels the
    /// actions found under it.
    pub(crate) wrapper: Option<ExprId>,
}

impl Scope {
    /// Whether this context retains an INSTANCE substitution. Temporal
    /// argument admission uses this to preserve the pinned reference's
    /// explicit refusal for exported parameterized INSTANCE formulas.
    #[must_use]
    pub fn has_substitutions(&self) -> bool {
        self.env.dynamic.subst.is_some()
    }

    /// The definition whose frame the scope's expressions are evaluated in.
    #[must_use]
    pub fn def(&self) -> DefId {
        self.env.frame.def
    }
}

/// What a slot of a scope holds, as the binding stage needs to see it.
#[derive(Clone, Debug)]
pub enum SlotView {
    /// Nothing is bound.
    Unset,
    /// A value.
    Value(Value),
    /// A pure unknown value for conservative state-level analysis only.
    /// Forcing it is an explicit error; it is never a guessed runtime value.
    AnalysisBound,
    /// A lazy argument (`LazyValue`): the expression and the scope it is
    /// evaluated in.
    Lazy(ExprId, Scope),
    /// An actual recursive function. Callers must inspect its owned domains,
    /// captured context and body; this does not establish purity or finiteness.
    Function(Arc<crate::LazyFcn>),
    /// An operator argument or another lazy function: not an expression.
    Other,
}

impl Evaluator {
    /// Resolves a known user operator for non-evaluating context analysis.
    ///
    /// Preserves its lexical parent and dynamic substitution context. Each
    /// ordinary value formal is bound to [`SlotView::AnalysisBound`], which is
    /// state-level for analysis and cannot be forced as a runtime value. This
    /// does not prove purity: callers must inspect the returned body and scope.
    /// Native, built-in, configured constant, unresolved, recursive, overridden
    /// and higher-order targets return `None` without evaluation or callbacks.
    /// A definition used by any configured operator constant is also refused,
    /// even when explicitly passed, because captured bindings erase its origin.
    /// Callers must also prove [`Self::scope_operator_captures`], including
    /// unused lexical/dynamic operator arguments, under their own work budget.
    /// Use [`Self::scope_apply_param`] with real arguments for execution.
    ///
    /// # Errors
    /// An invalid definition, enclosing frame or foreign captured context.
    pub fn scope_operator_analysis(
        &self,
        scope: &Scope,
        operator: &OpRef,
    ) -> Result<Option<(DefId, Scope)>> {
        self.validate_scope(scope)?;
        let value = match operator {
            OpRef::Def(def) => {
                self.admit_def(*def)?;
                let target = self.core.redirect(*def);
                self.admit_def(target)?;
                self.admit_parent(scope, target)?;
                self.core.op_value(&scope.env, operator)?
            }
            OpRef::Param(local) => {
                let Some(frame) = scope.env.frame_up(local.up) else {
                    return Ok(None);
                };
                let Some(Sv::Op(value)) = frame.slots.get(local.slot.index()) else {
                    return Ok(None);
                };
                value.clone()
            }
            _ => return Ok(None),
        };
        self.operator_value_analysis(scope, value)
    }

    /// Resolves an authoritative, bounded pure native operator actual.
    ///
    /// Only `Sequences!Len`, `Sequences!Head`, `Sequences!Append` and
    /// `FiniteSets!Cardinality`
    /// are admitted. This is callback-free metadata inspection, not a purity
    /// or truth certificate. Callers must prove the captured context, arity
    /// and actual expression arguments independently. Configured targets and
    /// replacements conservatively refuse, including a directly supplied
    /// same native target; opaque, builtin and other native targets refuse.
    /// Each target/configuration inspection charges `work` before access.
    ///
    /// # Errors
    /// Invalid ownership/reference or exhausted inspection work.
    pub fn scope_native_operator_analysis(
        &self,
        scope: &Scope,
        operator: &OpRef,
        work: &mut usize,
    ) -> Result<Option<dive_ir::StdOp>> {
        self.validate_scope(scope)?;
        Self::operator_capture_step(work)?;
        let value = match operator {
            OpRef::Def(def) => {
                self.admit_def(*def)?;
                let target = self.core.redirect(*def);
                self.admit_def(target)?;
                self.admit_parent(scope, target)?;
                self.core.op_value(&scope.env, operator)?
            }
            OpRef::Std(op) => crate::env::OpVal::Std(*op),
            OpRef::Param(local) => {
                let Some(frame) = scope.env.frame_up(local.up) else {
                    return Ok(None);
                };
                let Some(Sv::Op(value)) = frame.slots.get(local.slot.index()) else {
                    return Ok(None);
                };
                value.clone()
            }
            _ => return Ok(None),
        };
        self.native_operator_value(&value, work)
    }

    fn native_operator_value(
        &self,
        value: &crate::env::OpVal,
        work: &mut usize,
    ) -> Result<Option<dive_ir::StdOp>> {
        use dive_ir::StdOp;
        let native = match value {
            crate::env::OpVal::Std(op) => *op,
            crate::env::OpVal::Def {
                def,
                parent,
                dynamic,
            } => {
                // Native calls have no lexical body. Do not discard a supplied
                // foreign/opaque lexical or dynamic capture as irrelevant.
                if parent.is_some() || dynamic.is_some() {
                    return Ok(None);
                }
                Self::operator_capture_step(work)?;
                let target = self.admit_def(*def)?;
                if self.is_value_override(*def) {
                    return Ok(None);
                }
                let Some(op) = target.std else {
                    return Ok(None);
                };
                op
            }
            _ => return Ok(None),
        };
        if !matches!(
            native,
            StdOp::SequencesLen
                | StdOp::SequencesHead
                | StdOp::SequencesAppend
                | StdOp::FiniteSetsCardinality
        ) {
            return Ok(None);
        }
        // Native values, like user Def values, do not retain configured origin.
        for binding in &self.core.consts {
            Self::operator_capture_step(work)?;
            if let Some(crate::ConstBinding::Def(bound)) = binding {
                let target = self.admit_def(self.core.redirect(*bound))?;
                if target.std == Some(native) {
                    return Ok(None);
                }
            }
        }
        for (index, target) in self.core.ir.defs().iter().enumerate() {
            Self::operator_capture_step(work)?;
            if target.std == Some(native) {
                let def =
                    DefId(u32::try_from(index).map_err(|_| {
                        EvalError::resource_limit("native operator definition index")
                    })?);
                if self.is_value_override(def) || self.core.redirect(def) != def {
                    return Ok(None);
                }
            }
        }
        Ok(Some(native))
    }

    pub(crate) fn operator_value_analysis(
        &self,
        scope: &Scope,
        value: crate::env::OpVal,
    ) -> Result<Option<(DefId, Scope)>> {
        let crate::env::OpVal::Def {
            def,
            parent,
            dynamic,
        } = value
        else {
            return Ok(None);
        };
        let target = self.admit_def(def)?;
        // OpVal::Def does not retain configured-constant origin. Conservatively
        // refuse every such target, including an explicitly passed same def.
        if self.core.consts.iter().flatten().any(|binding| {
            matches!(binding, crate::ConstBinding::Def(bound) if self.core.redirect(*bound) == def)
        }) {
            return Ok(None);
        }
        if target.recursive
            || target.std.is_some()
            || self.is_value_override(def)
            || target.params.iter().any(|param| param.arity != 0)
        {
            return Ok(None);
        }
        let mut env = Env::new(
            def,
            target.depth,
            target.frame.len(),
            parent,
            dynamic.unwrap_or_else(|| scope.env.dynamic.clone()),
        );
        for i in 0..target.params.len() {
            env.bind(i, Sv::AnalysisBound);
        }
        Ok(Some((
            def,
            Scope {
                owner: self.core.ir.clone(),
                names: self.core.names.clone(),
                context: Arc::downgrade(&self.core),
                env,
                wrapper: scope.wrapper,
            },
        )))
    }

    pub(crate) fn selectseq_value_analysis(
        &self,
        scope: &Scope,
        value: crate::env::OpVal,
        work: &mut usize,
    ) -> Result<Option<(DefId, Scope)>> {
        self.validate_scope(scope)?;
        *work = work
            .checked_sub(self.core.consts.len().saturating_add(1))
            .ok_or_else(|| EvalError::resource_limit("SelectSeq target work"))?;
        let crate::env::OpVal::Def {
            def,
            parent,
            dynamic: None,
        } = &value
        else {
            return Err(EvalError::unsupported(
                "SelectSeq retained dynamic/native callback",
            ));
        };
        let definition = self.admit_def(*def)?;
        match definition.scope {
            DefScope::Local { parent: owner } | DefScope::Inline { parent: owner }
                if parent.as_ref().is_none_or(|frame| {
                    frame.def != owner || frame.depth != definition.depth.saturating_sub(1)
                }) =>
            {
                return Err(EvalError::unsupported(
                    "SelectSeq incompatible lexical callback owner",
                ));
            }
            DefScope::TopLevel if parent.is_some() => {
                return Err(EvalError::unsupported(
                    "SelectSeq unexpected lexical callback owner",
                ));
            }
            _ => {}
        }
        self.operator_value_analysis(scope, value)
    }

    /// Projects every captured lazy expression and known user operator body.
    ///
    /// This performs no evaluation, normalization or callback. Returned bodies
    /// retain lexical/dynamic ownership and analysis-only ordinary formals;
    /// callers must independently prove each one, including unused captures.
    /// Opaque functions/operators and configured/recursive/native targets return
    /// `None`. Each context, substitution link and slot consumes `work` before
    /// inspection or enqueue; retained context depth is capped at 128.
    /// Stored values are not forced and this is not a value-purity certificate.
    /// Dynamic `@` bindings conservatively refuse instead of hashing values.
    ///
    /// # Errors
    /// Invalid ownership/reference or the supplied work/context depth bound.
    pub fn scope_operator_captures(
        &self,
        scope: &Scope,
        work: &mut usize,
    ) -> Result<Option<Vec<(ExprId, Scope)>>> {
        self.operator_captures(scope, work, false)
    }

    pub(crate) fn operator_captures(
        &self,
        scope: &Scope,
        work: &mut usize,
        selectseq: bool,
    ) -> Result<Option<Vec<(ExprId, Scope)>>> {
        self.validate_scope(scope)?;
        Self::operator_capture_step(work)?;
        let mut captures = Vec::new();
        let mut todo = vec![(scope.env.clone(), 0_usize)];
        let mut frames = std::collections::HashSet::new();
        let mut links = std::collections::HashSet::new();
        // Retain every traversed owner for the walk: allocation addresses must
        // not be recycled while they occur in identity keys.
        let mut owners = Vec::new();
        while let Some((env, depth)) = todo.pop() {
            if depth > 128 {
                return Err(EvalError::resource_limit("operator capture context depth"));
            }
            if env.dynamic.at.is_some() {
                return Ok(None); // No Value hash/equality is capture authority.
            }
            Self::operator_capture_step(work)?;
            owners.push(env.clone());
            let context_scope = Scope {
                env: env.clone(),
                ..scope.clone()
            };
            let mut slots = Vec::new();
            let dynamic_key = (
                env.dynamic
                    .subst
                    .as_ref()
                    .map_or(0, |head| Arc::as_ptr(head) as usize),
                env.dynamic.cut,
            );
            let frame_key = (Arc::as_ptr(&env.frame) as usize, dynamic_key);
            if !frames.contains(&frame_key) {
                Self::operator_capture_step(work)?;
                frames.insert(frame_key);
                for slot in &env.frame.slots {
                    Self::operator_capture_step(work)?;
                    slots.push(slot);
                }
                if let Some(parent) = &env.frame.parent {
                    Self::operator_capture_step(work)?;
                    todo.push((
                        Env {
                            frame: parent.clone(),
                            dynamic: env.dynamic.clone(),
                        },
                        depth + 1,
                    ));
                }
            }
            let mut link = env.dynamic.subst.as_deref();
            while let Some(binding) = link {
                let link_key = (std::ptr::from_ref(binding) as usize, dynamic_key);
                if links.contains(&link_key) {
                    break;
                }
                Self::operator_capture_step(work)?;
                links.insert(link_key);
                slots.push(&binding.val);
                link = binding.next.as_deref();
            }
            for slot in slots {
                Self::operator_capture_step(work)?;
                let (expr, source) = match slot {
                    Sv::Thunk(thunk) => {
                        self.admit_expr(thunk.expr)?;
                        (
                            thunk.expr,
                            Scope {
                                env: thunk.env.clone(),
                                ..scope.clone()
                            },
                        )
                    }
                    Sv::Op(operator) => {
                        if selectseq
                            && !matches!(operator, crate::env::OpVal::Def { dynamic: None, .. })
                        {
                            return Ok(None);
                        }
                        if !selectseq && self.native_operator_value(operator, work)?.is_some() {
                            // The whitelist has no lexical body/captured callback.
                            // Actual arguments still require independent proof.
                            continue;
                        }
                        let resolved = if selectseq {
                            self.selectseq_value_analysis(&context_scope, operator.clone(), work)?
                        } else {
                            self.operator_value_analysis(&context_scope, operator.clone())?
                        };
                        let Some((def, source)) = resolved else {
                            return Ok(None);
                        };
                        (self.core.ir.def(def).body, source)
                    }
                    Sv::Fcn(_) => return Ok(None),
                    Sv::Unset | Sv::Val(_) | Sv::AnalysisBound => continue,
                };
                self.validate_scope(&source)?;
                Self::operator_capture_step(work)?;
                todo.push((source.env.clone(), depth + 1));
                captures.push((expr, source));
            }
        }
        Ok(Some(captures))
    }

    fn operator_capture_step(work: &mut usize) -> Result<()> {
        *work = work
            .checked_sub(1)
            .ok_or_else(|| EvalError::resource_limit("operator capture work"))?;
        Ok(())
    }

    /// Enters an application of `def` to `args` in `scope`, as TLC's
    /// `getOpContext` does: each expression argument is bound lazily (a
    /// `LazyValue` over `scope`), each operator argument to the operator.
    ///
    /// # Errors
    /// An arity mismatch or a missing enclosing frame (bugs), or an operator
    /// argument TLC cannot look up.
    pub fn scope_apply(&self, scope: &Scope, def: DefId, args: &[Arg]) -> Result<Scope> {
        self.validate_scope(scope)?;
        self.admit_args(scope, def, args)?;
        let core = &self.core;
        let d = self.admit_def(def)?;
        if args.len() != d.params.len() {
            return Err(EvalError::bug(format!(
                "{} applied to {} arguments, expects {}",
                d.name,
                args.len(),
                d.params.len()
            )));
        }
        let parent = self.admit_parent(scope, def)?;
        let mut callee = Env::new(
            def,
            d.depth,
            d.frame.len(),
            parent,
            scope.env.dynamic.clone(),
        );
        for (i, a) in args.iter().enumerate() {
            let sv = match a {
                Arg::Expr { expr, .. } => Sv::Thunk(Thunk::new(*expr, scope.env.clone(), true)),
                Arg::Op { op, .. } => Sv::Op(core.op_value(&scope.env, op)?),
            };
            callee.bind(i, sv);
        }
        Ok(Scope {
            owner: self.core.ir.clone(),
            names: self.core.names.clone(),
            context: Arc::downgrade(&self.core),
            env: callee,
            wrapper: scope.wrapper,
        })
    }

    /// Enters an application of an operator parameter: the definition TLC's
    /// lookup of the parameter gives (the operator passed for it), entered as
    /// [`Evaluator::scope_apply`] enters a definition. `None` if the
    /// parameter is not bound to a user definition.
    ///
    /// # Errors
    /// As [`Evaluator::scope_apply`].
    #[must_use]
    pub fn scope_apply_param(
        &self,
        scope: &Scope,
        r: LocalRef,
        args: &[Arg],
    ) -> Option<Result<(DefId, Scope)>> {
        if let Err(e) = self.validate_scope(scope) {
            return Some(Err(e));
        }
        let core = &self.core;
        let frame = scope.env.frame_up(r.up)?;
        let Some(Sv::Op(crate::env::OpVal::Def {
            def,
            parent,
            dynamic,
        })) = frame.slots.get(r.slot.index())
        else {
            return None;
        };
        let def = *def;
        if let Err(e) = self.admit_args(scope, def, args) {
            return Some(Err(e));
        }
        let d = core.ir.def(def);
        if args.len() != d.params.len() || core.ir.def(def).std.is_some() {
            return None;
        }
        let parent = match d.scope {
            DefScope::Local { .. } | DefScope::Inline { .. } => parent.clone(),
            _ => None,
        };
        let mut callee = Env::new(
            def,
            d.depth,
            d.frame.len(),
            parent,
            dynamic.clone().unwrap_or_else(|| scope.env.dynamic.clone()),
        );
        for (i, a) in args.iter().enumerate() {
            let sv = match a {
                Arg::Expr { expr, .. } => Sv::Thunk(Thunk::new(*expr, scope.env.clone(), true)),
                Arg::Op { op, .. } => match core.op_value(&scope.env, op) {
                    Ok(v) => Sv::Op(v),
                    Err(e) => return Some(Err(e)),
                },
            };
            callee.bind(i, sv);
        }
        Some(Ok((
            def,
            Scope {
                owner: self.core.ir.clone(),
                names: self.core.names.clone(),
                context: Arc::downgrade(&self.core),
                env: callee,
                wrapper: scope.wrapper,
            },
        )))
    }

    /// The scope of `def`'s body with none of its parameters bound, as TLC's
    /// `getLevelBound` sees the body of an operator it looks into: a
    /// parameter it meets is not in the context. A `LET` operator or `LAMBDA`
    /// keeps `scope`'s frame as its parent; a zero-arity `LET` definition is
    /// evaluated in `scope` itself.
    ///
    /// # Errors
    /// A foreign scope, missing definition or missing enclosing frame.
    pub fn scope_unbound(&self, scope: &Scope, def: DefId) -> Result<Scope> {
        self.validate_scope(scope)?;
        let d = self.admit_def(def)?;
        let parent = self.admit_parent(scope, def)?;
        Ok(match d.scope {
            DefScope::LetValue { .. } => scope.clone(),
            DefScope::TopLevel => return self.scope(def),
            DefScope::Local { .. } | DefScope::Inline { .. } => Scope {
                owner: self.core.ir.clone(),
                names: self.core.names.clone(),
                context: Arc::downgrade(&self.core),
                env: Env::new(
                    def,
                    d.depth,
                    d.frame.len(),
                    parent,
                    scope.env.dynamic.clone(),
                ),
                wrapper: scope.wrapper,
            },
        })
    }

    /// Enters an application of `def` whose parameters are bound to values, as
    /// `Tool.getActionsAppl` does when it inlines an operator
    /// (`con1.cons(formals[i], aval)`).
    ///
    /// # Errors
    /// An arity mismatch or a missing enclosing frame (bugs).
    pub fn scope_call_values(
        &self,
        scope: &Scope,
        def: DefId,
        values: Vec<Value>,
    ) -> Result<Scope> {
        self.validate_scope(scope)?;
        let d = self.admit_def(def)?;
        if d.params.iter().any(|p| p.arity != 0) {
            return Err(crate::ContextError::new(
                crate::ContextErrorKind::Arity,
                "operator parameter requires an operator binding",
            )
            .into());
        }
        if values.len() != d.params.len() {
            return Err(crate::ContextError::new(
                crate::ContextErrorKind::Arity,
                "value argument count",
            )
            .into());
        }
        let mut s = match d.scope {
            DefScope::LetValue { .. } => return Ok(scope.clone()),
            _ => self.scope_unbound(scope, def)?,
        };
        if matches!(d.scope, DefScope::TopLevel) {
            s.env.dynamic = scope.env.dynamic.clone();
        }
        for (i, v) in values.into_iter().enumerate() {
            s.env.bind(i, Sv::Val(v));
        }
        Ok(s)
    }

    /// The scope of a zero-arity `LET` definition's body (`DefScope::LetValue`
    /// evaluates in its parent's frame) or of a definition with no parameters
    /// entered from `scope`.
    ///
    /// # Errors
    /// As [`Evaluator::scope_apply`].
    pub fn scope_enter(&self, scope: &Scope, def: DefId) -> Result<Scope> {
        self.validate_scope(scope)?;
        if matches!(self.admit_def(def)?.scope, DefScope::LetValue { .. }) {
            self.admit_parent(scope, def)?;
            return Ok(scope.clone());
        }
        self.scope_apply(scope, def, &[])
    }

    /// A fresh frame of the top-level definition `def`: the context in which
    /// `SpecProcessor` finds a configured formula's parts.
    ///
    /// # Errors
    /// A missing definition or a definition requiring an enclosing lexical frame.
    pub fn scope(&self, def: DefId) -> Result<Scope> {
        let d = self.admit_def(def)?;
        if d.scope != DefScope::TopLevel {
            return Err(crate::ContextError::new(
                crate::ContextErrorKind::Structure,
                "definition requires an enclosing lexical frame",
            )
            .into());
        }
        Ok(Scope {
            owner: self.core.ir.clone(),
            names: self.core.names.clone(),
            context: Arc::downgrade(&self.core),
            env: Env::new(def, d.depth, d.frame.len(), None, Dyn::default()),
            wrapper: None,
        })
    }

    /// `scope` under the substitutions of `subst`, an [`Expr::Subst`] node of
    /// its frame, as TLC wraps a formula found under an `INSTANCE` in a
    /// substitution (`Specs.addSubsts`) that the generator then evaluates as
    /// uncached lazy values (`Tool.java:960-990`, `:488-498`). Labels of
    /// actions in the result name the substitution's location, as TLC's
    /// wrapper does.
    ///
    /// # Errors
    /// A bug if `subst` is not a substitution; an evaluation error of a
    /// binding TLC evaluates eagerly (none: every expression binding is lazy).
    pub fn scope_subst(&self, scope: &Scope, subst: ExprId) -> Result<Scope> {
        self.admit_scoped_expr(scope, subst)?;
        self.validate_scope(scope)?;
        let Expr::Subst { bindings, .. } = &self.core.ir.node(subst).expr else {
            return Err(EvalError::bug(
                "scope_subst on a node that is not a substitution",
            ));
        };
        let env = self
            .core
            .bind_subst(&scope.env, &St::default(), bindings, false)?;
        Ok(Scope {
            owner: self.core.ir.clone(),
            names: self.core.names.clone(),
            context: Arc::downgrade(&self.core),
            env,
            wrapper: Some(subst),
        })
    }

    /// The context of a call `apply` (an application of a user definition)
    /// made in `scope`: a fresh frame of the definition with its arguments
    /// bound as uncached lazy values (`getOpContext(..., false)`, as
    /// `SpecProcessor.processConfigSpec` inlines a parameterized formula).
    /// Returns the definition called.
    ///
    /// # Errors
    /// A bug if `apply` is not such an application.
    pub fn scope_call(&self, scope: &Scope, apply: ExprId) -> Result<(Scope, DefId)> {
        self.admit_scoped_expr(scope, apply)?;
        self.validate_scope(scope)?;
        let (d, args): (&DefId, &[Arg]) = match &self.core.ir.node(apply).expr {
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => (d, args),
            Expr::Def(d) => (d, &[]),
            _ => return Err(EvalError::bug("scope_call on a node that is not a call")),
        };
        let d = self.core.redirect(*d);
        self.admit_args(scope, d, args)?;
        let def = self.admit_def(d)?;
        let parent = self.admit_parent(scope, d)?;
        let mut env = Env::new(
            d,
            def.depth,
            def.frame.len(),
            parent,
            scope.env.dynamic.clone(),
        );
        for (i, a) in args.iter().enumerate() {
            let sv = match a {
                Arg::Expr { expr, pass } => match pass {
                    Pass::Strict => Sv::from(self.core.eval(
                        &mut scope.env.clone(),
                        &St::constant(),
                        *expr,
                    )?),
                    Pass::Lazy | Pass::Direct => {
                        Sv::Thunk(Thunk::new(*expr, scope.env.clone(), false))
                    }
                },
                Arg::Op { op, .. } => Sv::Op(self.core.op_value(&scope.env, op)?),
            };
            env.bind(i, sv);
        }
        Ok((
            Scope {
                owner: self.core.ir.clone(),
                names: self.core.names.clone(),
                context: Arc::downgrade(&self.core),
                env,
                wrapper: scope.wrapper,
            },
            d,
        ))
    }

    /// Binds the zero-arity definitions of a `LET` (an [`Expr::Let`]) in
    /// `scope`, so that its body can be evaluated there. TLC does not bind
    /// them when it walks a specification (`processConfigSpec`, `getActions`):
    /// it finds each definition by its node and evaluates its body at every
    /// use. The thunks are therefore not cached, which is indistinguishable
    /// from TLC.
    ///
    /// # Errors
    /// If `let_expr` is not a `LET`.
    pub fn scope_let(&self, scope: &Scope, let_expr: ExprId) -> Result<Scope> {
        self.admit_scoped_expr(scope, let_expr)?;
        self.validate_scope(scope)?;
        let core = &self.core;
        let Expr::Let { defs, .. } = core.ir.expr(let_expr) else {
            return Err(EvalError::bug("scope_let on a node that is not a LET"));
        };
        let mut env = scope.env.clone();
        for &d in defs {
            if let DefScope::LetValue { slot, .. } = core.ir.def(d).scope {
                let body = core.ir.def(d).body;
                env.bind(
                    slot.index(),
                    Sv::Thunk(Thunk::new(body, env.clone(), false)),
                );
            }
        }
        Ok(Scope {
            owner: self.core.ir.clone(),
            names: self.core.names.clone(),
            context: Arc::downgrade(&self.core),
            env,
            wrapper: scope.wrapper,
        })
    }

    /// The scope without its instance substitutions: what TLC's context holds
    /// while `processConfigProps` walks under a substitution (TLC keeps the
    /// substitutions apart, to wrap them around what it finds).
    ///
    /// # Errors
    /// A foreign scope or inadmissible captured value.
    pub fn scope_without_substs(&self, scope: &Scope) -> Result<Scope> {
        self.validate_scope(scope)?;
        let mut env = scope.env.clone();
        env.dynamic.subst = None;
        Ok(Scope {
            owner: self.core.ir.clone(),
            names: self.core.names.clone(),
            context: Arc::downgrade(&self.core),
            env,
            wrapper: scope.wrapper,
        })
    }

    /// Binds a slot of the scope's own frame to a value: a variable of a split
    /// `\E`, or a parameter of an operator inlined with its value
    /// (`Tool.getActionsAppl`).
    ///
    /// # Errors
    /// A foreign scope, unknown slot or inadmissible value.
    pub fn scope_bind(&self, scope: &Scope, slot: Slot, v: Value) -> Result<Scope> {
        self.validate_scope(scope)?;
        if slot.index() >= scope.env.frame.slots.len() {
            return Err(crate::ContextError::new(
                crate::ContextErrorKind::UnknownId,
                "scope binding slot",
            )
            .into());
        }
        if self
            .admit_def(scope.def())?
            .params
            .get(slot.index())
            .is_some_and(|p| p.arity != 0)
        {
            return Err(crate::ContextError::new(
                crate::ContextErrorKind::Arity,
                "operator parameter requires an operator binding",
            )
            .into());
        }
        let mut env = scope.env.clone();
        env.bind(slot.index(), Sv::Val(v));
        Ok(Scope {
            owner: self.core.ir.clone(),
            names: self.core.names.clone(),
            context: Arc::downgrade(&self.core),
            env,
            wrapper: scope.wrapper,
        })
    }

    /// The values bound to the parameters of `def` in the innermost frame of
    /// `def` in `scope` (walking out through enclosing frames), in parameter
    /// order; `None` for a parameter bound to anything but a value. This is
    /// what TLC prints in an action's label (`Action.getLocation`).
    #[must_use]
    pub fn scope_params(&self, scope: &Scope, def: DefId) -> Vec<Option<Value>> {
        if self.validate_scope(scope).is_err() {
            return Vec::new();
        }
        let Some(d) = self.core.ir.defs().get(def.index()) else {
            return Vec::new();
        };
        let n = d.params.len();
        let mut f = Some(&scope.env.frame);
        while let Some(frame) = f {
            if frame.def == def {
                return (0..n)
                    .map(|i| match frame.slots.get(i) {
                        Some(Sv::Val(v)) => Some(v.clone()),
                        _ => None,
                    })
                    .collect();
            }
            f = frame.parent.as_ref();
        }
        vec![None; n]
    }

    /// What the innermost substitution of `decl` in `scope` holds, if any.
    #[must_use]
    pub fn scope_subst_slot(&self, scope: &Scope, decl: dive_ir::DeclId) -> SlotView {
        if self.validate_scope(scope).is_err() {
            return SlotView::Other;
        }
        match scope.env.dynamic.lookup(decl) {
            None | Some(Sv::Unset) => SlotView::Unset,
            Some(Sv::Val(v)) => SlotView::Value(v.clone()),
            Some(Sv::AnalysisBound) => SlotView::AnalysisBound,
            Some(Sv::Thunk(t)) => SlotView::Lazy(
                t.expr,
                Scope {
                    owner: self.core.ir.clone(),
                    names: self.core.names.clone(),
                    context: Arc::downgrade(&self.core),
                    env: t.env.clone(),
                    wrapper: scope.wrapper,
                },
            ),
            Some(_) => SlotView::Other,
        }
    }

    /// What a local reference in `scope` holds.
    #[must_use]
    pub fn scope_slot(&self, scope: &Scope, r: LocalRef) -> SlotView {
        if self.validate_scope(scope).is_err() {
            return SlotView::Other;
        }
        let Some(frame) = scope.env.frame_up(r.up) else {
            return SlotView::Unset;
        };
        match frame.slots.get(r.slot.index()) {
            None | Some(Sv::Unset) => SlotView::Unset,
            Some(Sv::Val(v)) => SlotView::Value(v.clone()),
            Some(Sv::AnalysisBound) => SlotView::AnalysisBound,
            Some(Sv::Thunk(t)) => SlotView::Lazy(
                t.expr,
                Scope {
                    owner: self.core.ir.clone(),
                    names: self.core.names.clone(),
                    context: Arc::downgrade(&self.core),
                    env: t.env.clone(),
                    wrapper: scope.wrapper,
                },
            ),
            Some(Sv::Fcn(function))
                if function.rec_slot.is_some() && function.excepts.is_empty() =>
            {
                SlotView::Function(function.clone())
            }
            Some(_) => SlotView::Other,
        }
    }

    /// The bindings of a bounded `\E` (an [`Expr::Quant`]) in `scope`, in
    /// TLC's enumeration order (`ContextEnumerator`, first variable fastest):
    /// one scope per binding, each with the variables bound. The domains are
    /// evaluated in `st` (TLC's `getActions` uses the empty state).
    ///
    /// # Errors
    /// Any error evaluating or enumerating a domain.
    pub fn scope_bindings(&self, scope: &Scope, quant: ExprId, st: &St) -> Result<Vec<Scope>> {
        self.admit_scoped_expr(scope, quant)?;
        self.validate_scope(scope)?;
        let core = &self.core;
        let Expr::Quant { bounds, .. } = core.ir.expr(quant) else {
            return Err(EvalError::bug(
                "scope_bindings on a node that is not a quantifier",
            ));
        };
        let mut env = scope.env.clone();
        let mut out = Vec::new();
        core.for_each_context(&mut env, st, bounds, quant, &mut |_, env| {
            out.push(Scope {
                owner: self.core.ir.clone(),
                names: self.core.names.clone(),
                context: Arc::downgrade(&self.core),
                env: env.clone(),
                wrapper: scope.wrapper,
            });
            Ok(true)
        })?;
        Ok(out)
    }

    /// Enumerates finite bindings of a quantifier, set filter/map or function
    /// constructor with entry-count bounds.
    /// Explicit sets, intervals and structurally finite lazy constructors are
    /// supported. Domain entries, materialized component/value slots and
    /// Cartesian bindings are each bounded by `limit`, before retaining
    /// them. Pure captured filters over bounded structural inputs are admitted;
    /// infinite, opaque and unresolved-effect domains remain explicit refusals.
    /// Order is the profile's exploration order, first variable fastest.
    /// This is expression binding, not successor generation.
    ///
    /// # Errors
    /// Missing/nonfinite bounds, domain evaluation or tuple-pattern errors,
    /// and explicit domain/binding resource limits.
    pub fn scope_bindings_bounded(
        &self,
        scope: &Scope,
        quant: ExprId,
        st: &St,
        limit: usize,
    ) -> Result<Vec<Scope>> {
        let columns = self.bounded_binding_columns(scope, quant, st, limit)?;
        self.bounded_body_contexts(scope, quant, &columns, limit)
    }

    // All domain evaluation, bounded-domain inspection and enumeration remains
    // before any body. The public eager API and private cursor share this order.
    fn bounded_binding_columns(
        &self,
        scope: &Scope,
        quant: ExprId,
        st: &St,
        limit: usize,
    ) -> Result<Vec<BindingColumn>> {
        self.admit_scoped_expr(scope, quant)?;
        self.validate_scope(scope)?;
        let core = &self.core;
        let bounds: &[dive_ir::Bound] = match core.ir.expr(quant) {
            Expr::Quant { bounds, .. }
            | Expr::SetMap { bounds, .. }
            | Expr::FcnCons { bounds, .. } => bounds,
            Expr::SetFilter { bound, .. } => std::slice::from_ref(bound),
            _ => return Err(EvalError::bug("bounded bindings require a finite binder")),
        };
        let mut columns = Vec::new();
        let mut entries = 0_usize;
        let mut budget = crate::bounded_domain::Budget::new(limit, core.opts.max_depth, core.ctx);
        for b in bounds {
            let dom = b
                .domain
                .ok_or_else(|| EvalError::unsupported("unbounded quantifier"))?;
            let Value::Set(set) = self.eval_scoped(scope, dom, st)? else {
                return Err(EvalError::unsupported("quantifier domain is not a set"));
            };
            budget.set(&set, 0)?;
            let size = usize::try_from(set.size(core.ctx)?)
                .map_err(|_| EvalError::resource_limit("quantifier domain entries"))?;
            entries = entries
                .checked_add(size)
                .filter(|n| *n <= limit)
                .ok_or_else(|| EvalError::resource_limit("quantifier domain entries"))?;
            let values = core.explore(&set)?;
            if b.tuple {
                columns.push((
                    b.vars.iter().map(|slot| slot.index()).collect(),
                    true,
                    values,
                ));
            } else {
                for &v in &b.vars {
                    columns.push((vec![v.index()], false, values.clone()));
                }
            }
        }
        Ok(columns)
    }

    // Only non-analysis scalar Quant walks select this private route. Defensive
    // eager fallback keeps tuple errors ahead of any body error, as before.
    pub(crate) fn for_each_scalar_quantifier_scope(
        &self,
        scope: &Scope,
        quant: ExprId,
        st: &St,
        limit: usize,
        mut body: impl FnMut(&Scope) -> Result<()>,
    ) -> Result<()> {
        let columns = self.bounded_binding_columns(scope, quant, st, limit)?;
        if !matches!(self.core.ir.expr(quant), Expr::Quant { .. })
            || columns.iter().any(|(_, tuple, _)| *tuple)
        {
            for bound in self.bounded_body_contexts(scope, quant, &columns, limit)? {
                body(&bound)?;
            }
            return Ok(());
        }
        let count = Self::bounded_binding_count(&columns, limit)?;
        let mut bound = scope.clone();
        for i in 0..count {
            let mut quotient = i;
            for (slots, _, values) in &columns {
                let value = &values[quotient % values.len()];
                quotient /= values.len();
                // Each prior body borrow ended. Any retained snapshot forces
                // ordinary Env::bind COW; no caller/capture can be overwritten.
                self.core
                    .bind_one(&mut bound.env, slots, false, value, quant)?;
            }
            body(&bound)?;
        }
        Ok(())
    }

    fn bounded_binding_count(columns: &[BindingColumn], limit: usize) -> Result<usize> {
        columns
            .iter()
            .try_fold(1_usize, |n, (_, _, ds)| n.checked_mul(ds.len()))
            .filter(|n| *n <= limit)
            .ok_or_else(|| EvalError::resource_limit("quantifier bindings"))
    }

    pub(crate) fn bounded_body_contexts(
        &self,
        scope: &Scope,
        quant: ExprId,
        columns: &[BindingColumn],
        limit: usize,
    ) -> Result<Vec<Scope>> {
        let count = Self::bounded_binding_count(columns, limit)?;
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            let mut scope = scope.clone();
            let mut quotient = i;
            for (slots, tuple, values) in columns {
                let value = &values[quotient % values.len()];
                quotient /= values.len();
                self.core
                    .bind_one(&mut scope.env, slots, *tuple, value, quant)?;
            }
            result.push(scope);
        }
        Ok(result)
    }

    /// Binds each finite constructor/quantifier variable to a pure unknown.
    /// This context is only for conservative level and purity analysis of
    /// state-dependent domains. It does not evaluate a domain, assume it is
    /// finite, or choose a sample value. Every slot is conservatively state
    /// level and cannot be forced by the interpreter. Runtime evaluation
    /// must use the original scope with concrete bounded domain bindings.
    ///
    /// # Errors
    /// A non-binder expression or an unbounded domain.
    pub fn scope_binder_analysis(&self, scope: &Scope, expr: ExprId) -> Result<Scope> {
        self.admit_scoped_expr(scope, expr)?;
        self.validate_scope(scope)?;
        let bounds: &[dive_ir::Bound] = match self.core.ir.expr(expr) {
            Expr::Quant { bounds, .. }
            | Expr::SetMap { bounds, .. }
            | Expr::FcnCons { bounds, .. }
            | Expr::RecFcn { bounds, .. } => bounds,
            Expr::SetFilter { bound, .. } => std::slice::from_ref(bound),
            _ => return Err(EvalError::bug("analysis scope requires a finite binder")),
        };
        let mut result = scope.clone();
        if let Expr::RecFcn { name, .. } = self.core.ir.expr(expr) {
            result.env.bind(name.index(), Sv::AnalysisBound);
        }
        for bound in bounds {
            if bound.domain.is_none() {
                return Err(EvalError::unsupported("unbounded analysis domain"));
            }
            for slot in &bound.vars {
                result.env.bind(slot.index(), Sv::AnalysisBound);
            }
        }
        Ok(result)
    }

    /// Evaluates `expr` in `scope`.
    ///
    /// # Errors
    /// Any evaluation error, refusal or resource limit.
    pub fn eval_scoped(&self, scope: &Scope, expr: ExprId, st: &St) -> Result<Value> {
        self.admit_scoped_expr(scope, expr)?;
        self.validate_scope(scope)?;
        if st.selectseq.as_ref().is_some_and(|policy| {
            !std::sync::Weak::ptr_eq(&policy.context, &Arc::downgrade(&self.core))
        }) {
            return Err(EvalError::unsupported(
                "foreign pure predicate invocation context",
            ));
        }
        let mut env = scope.env.clone();
        let ev = self.core.eval(&mut env, st, expr)?;
        self.core.force_fcn(ev)
    }

    /// Evaluates an argument of an operator application in `scope`, as TLC's
    /// `getActionsAppl` does before it inlines the operator
    /// (`this.eval(args[i], con, TLCState.Empty)`).
    ///
    /// # Errors
    /// Any evaluation error; an operator argument is not a value.
    pub fn eval_arg(&self, scope: &Scope, arg: &Arg, st: &St) -> Result<Value> {
        self.validate_scope(scope)?;
        match arg {
            Arg::Expr { expr, .. } => self.eval_scoped(scope, *expr, st),
            Arg::Op { .. } => Err(EvalError::unsupported(
                "an operator argument where TLC expects a value",
            )),
        }
    }

    /// Evaluates the body of a zero-arity definition at start-up and keeps
    /// the value for every later use, as `SpecProcessor.processConstantDefns`
    /// does (`WorkerValue.demux(opDefEvaluator, opDef.getBody())`, then
    /// `opDef.setToolObject(toolId, val)`). The body is evaluated every time
    /// this is called, as TLC evaluates it every time it meets it; the first
    /// value is kept (TLC keeps the last, which is the same value unless the
    /// definition draws random values). On failure nothing is kept and the
    /// definition stays lazy; TLC swallows the error.
    ///
    /// A function TLC would keep as a lambda (`FcnLambdaValue`) is not kept:
    /// the definition stays lazy, and `Ok(None)` is returned. dive has no
    /// value form for such a function, and re-evaluating it is
    /// indistinguishable unless it is impure.
    ///
    /// # Errors
    /// The evaluation error, which the binding stage swallows as TLC does.
    pub fn precompute(&self, def: DefId) -> Result<Option<Value>> {
        let core = &self.core;
        let d = self.admit_def(def)?;
        let mut env = Env::new(def, d.depth, d.frame.len(), None, Dyn::default());
        let ev = core.eval(&mut env, &St::constant(), d.body)?;
        let Ev::V(v) = ev else {
            return Ok(None);
        };
        if let Some(cell) = core.startup.get(def.index()) {
            let _ = cell.set(v.clone());
        }
        Ok(Some(v))
    }

    /// The value [`Evaluator::precompute`] kept for a definition.
    #[must_use]
    pub fn precomputed(&self, def: DefId) -> Option<Value> {
        self.core
            .startup
            .get(def.index())
            .and_then(|c| c.get())
            .cloned()
    }
}

impl crate::core::Core {
    /// Binds `bindings` (an instance's substitutions) over `env`: lazy values
    /// over the outer context, cached or not as TLC creates them.
    pub(crate) fn bind_subst(
        self: &Arc<Self>,
        env: &Env,
        st: &St,
        bindings: &[dive_ir::SubstBinding],
        cachable: bool,
    ) -> Result<Env> {
        let outer = env.clone();
        let mut env1 = env.clone();
        for b in bindings {
            let val = match &b.value {
                Arg::Expr { expr, pass } => match pass {
                    Pass::Strict => Sv::from(self.eval(&mut outer.clone(), st, *expr)?),
                    Pass::Lazy | Pass::Direct => {
                        Sv::Thunk(Thunk::new(*expr, outer.clone(), cachable))
                    }
                },
                Arg::Op { op, .. } => Sv::Op(self.op_value(&outer, op)?),
            };
            env1.dynamic.push(b.decl, val);
        }
        Ok(env1)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "owned capture-identity fixtures")]
mod operator_capture_tests {
    use super::*;
    use crate::env::OpVal;

    #[test]
    fn same_lexical_frame_is_inspected_under_each_dynamic_context() {
        let analysis = dive_sema::analyze_source(
            "CaptureIdentity.tla",
            "---- MODULE CaptureIdentity ----\nEXTENDS TLC\nCONSTANT A(_)\nGood(v)==v\nEvil(v)==PrintT(v)\nProbe(v)==A(v)\nHolder(F(_),saved)==TRUE\nRoot(a,b)==TRUE\n====\n",
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
        let ev = Evaluator::new(crate::Model::new(ir.clone()), crate::Options::default())
            .expect("evaluator");
        let def = |name| ir.root_def(name).expect("definition");
        let operator = |def| {
            Sv::Op(OpVal::Def {
                def,
                parent: None,
                dynamic: None,
            })
        };
        let mut shared = ev.scope(def("Holder")).expect("holder").env;
        shared.bind(0, operator(def("Probe")));
        shared.bind(1, Sv::AnalysisBound);
        let decl = ir
            .decls()
            .iter()
            .position(|d| d.name.as_str() == "A")
            .map(|i| dive_ir::DeclId(u32::try_from(i).expect("decl")))
            .expect("A");
        let contexts: Vec<_> = ["Good", "Evil"]
            .into_iter()
            .map(|name| {
                let mut env = shared.clone();
                env.dynamic.push(decl, operator(def(name)));
                env
            })
            .collect();
        assert!(Arc::ptr_eq(&contexts[0].frame, &contexts[1].frame));
        let mut scope = ev.scope(def("Root")).expect("root");
        for (i, env) in contexts.into_iter().enumerate() {
            scope.env.bind(
                i,
                Sv::Thunk(Thunk::new(ir.def(def("Root")).body, env, false)),
            );
        }
        let projected = ev
            .scope_operator_captures(&scope, &mut 1_000)
            .expect("ownership and work")
            .expect("known user targets");
        assert!(
            projected
                .iter()
                .any(|(expr, _)| *expr == ir.def(def("Evil")).body)
        );
        let probe_contexts: Vec<_> = projected
            .iter()
            .filter(|(expr, _)| *expr == ir.def(def("Probe")).body)
            .collect();
        assert_eq!(probe_contexts.len(), 2);
        let bindings: Vec<_> = probe_contexts
            .iter()
            .map(
                |(_, context)| match context.env.dynamic.lookup(decl).expect("substitution") {
                    Sv::Op(OpVal::Def { def, .. }) => *def,
                    _ => def("Root"),
                },
            )
            .collect();
        assert!(bindings.contains(&def("Good")) && bindings.contains(&def("Evil")));
        let mut at = scope.clone();
        at.env.dynamic.at = Some(true.into());
        assert!(
            ev.scope_operator_captures(&at, &mut 1_000)
                .expect("at refusal")
                .is_none()
        );
        assert_eq!(ev.take_output(), Vec::<String>::new());
    }
}

#[cfg(test)]
mod bounded_count_tests {
    use super::*;

    #[test]
    fn no_columns_is_one_binding_but_empty_domain_is_zero_even_at_zero_limit() {
        assert_eq!(Evaluator::bounded_binding_count(&[], 1), Ok(1));
        assert_eq!(
            Evaluator::bounded_binding_count(&[], 0),
            Err(EvalError::resource_limit("quantifier bindings"))
        );
        let columns = vec![(vec![0], false, Arc::<[Value]>::from([]))];
        assert_eq!(Evaluator::bounded_binding_count(&columns, 0), Ok(0));
    }
}
