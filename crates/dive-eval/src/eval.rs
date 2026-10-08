//! The interpreter: a port of TLC's `Tool.evalImpl`/`evalAppl`
//! (`tlc2/tool/impl/Tool.java:1795-2850` at tlaplus `94d0c50`) over the IR.
//!
//! Each arm cites the TLC opcode it ports. Evaluation order, laziness, the
//! points where TLC checks for booleans, and every error message follow TLC.

#![allow(
    clippy::many_single_char_names,
    clippy::unused_self,
    clippy::similar_names,
    reason = "short names follow the TLC code each arm ports; helpers stay methods of the shared state like their siblings"
)]

use std::sync::Arc;

use dive_ir::{
    Arg, Bound, BuiltinOp, CaseArm, DefScope, Expr, ExprId, JunctionKind, LocalRef, OpRef, Pass,
    QuantKind,
};
use dive_value::{DeferredKind, Int, Value, ops, tlc};

use crate::ConstBinding;
use crate::core::{Core, Lit, is_enumerable, is_reducible, kind_string};
use crate::env::{Dyn, Env, Ev, OpVal, St, StateRef, Sv, Thunk, control};
use crate::error::{Ec, EvalError, Result};
use crate::lazy::{BoundVal, Except, Filter, LazyFcn, SetOp};
use crate::options::Comparisons;

/// What a quantifier binds per element: its slots and whether it is a tuple
/// pattern.
struct Binder {
    slots: Vec<usize>,
    tuple: bool,
}

impl Core {
    // -----------------------------------------------------------------------------
    // Entry points
    // -----------------------------------------------------------------------------

    /// Evaluates `e`.
    pub(crate) fn eval(self: &Arc<Self>, env: &mut Env, st: &St, e: ExprId) -> Result<Ev> {
        self.eval_inner(env, st, e).map_err(|err| err.at(e))
    }

    /// Evaluates `e` and computes a lazy function's value (TLC's implicit
    /// `toFcnRcd`, as in `equals`, `compareTo` and printing).
    pub(crate) fn eval_v(self: &Arc<Self>, env: &mut Env, st: &St, e: ExprId) -> Result<Value> {
        let ev = self.eval(env, st, e)?;
        self.force_fcn(ev)
    }

    /// Evaluates a value that is stored inside a compound value (a set
    /// element, tuple component, record field or function value).
    pub(crate) fn eval_embed(self: &Arc<Self>, env: &mut Env, st: &St, e: ExprId) -> Result<Value> {
        let ev = self.eval(env, st, e)?;
        self.embed(ev)
    }

    /// Evaluates a boolean, with TLC's message for a non-boolean.
    fn eval_bool(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        e: ExprId,
        msg: impl FnOnce(&Self, &'static str) -> EvalError,
    ) -> Result<bool> {
        match self.eval(env, st, e)? {
            Ev::V(Value::Bool(b)) => Ok(b),
            other => Err(msg(self, self.kind_of(&other))),
        }
    }

    pub(crate) fn kind_of(&self, ev: &Ev) -> &'static str {
        match ev {
            Ev::V(v) => kind_string(v),
            Ev::F(_) => "a function of the form  [x \\in S |-> expr]",
        }
    }

    // -----------------------------------------------------------------------------
    // Lazy functions as values
    // -----------------------------------------------------------------------------

    /// `FcnLambdaValue.toFcnRcd`: computes the function.
    pub(crate) fn force_fcn(self: &Arc<Self>, ev: Ev) -> Result<Value> {
        match ev {
            Ev::V(v) => Ok(v),
            Ev::F(f) => self.materialize(&f),
        }
    }

    /// A value about to be stored inside another value. TLC keeps a lambda
    /// there; dive computes it when that is indistinguishable (the body is
    /// pure and every application succeeds), and refuses otherwise.
    pub(crate) fn embed(self: &Arc<Self>, ev: Ev) -> Result<Value> {
        match ev {
            Ev::V(v) => Ok(v),
            Ev::F(f) => {
                let body = self.ir.node(f.body);
                if body.effects.intersects(dive_ir::Effects::IMPURE) {
                    return Err(EvalError::unsupported(
                        "a lazily evaluated function (TLC's FcnLambdaValue) with side effects, stored inside another value",
                    ));
                }
                match self.materialize(&f) {
                    Ok(v) => Ok(v),
                    Err(e) if e.is_eval() => Err(EvalError::unsupported(format!(
                        "a lazily evaluated function (TLC's FcnLambdaValue) stored inside another value, which TLC would not compute here (computing it fails: {})",
                        e.message().lines().next().unwrap_or("")
                    ))),
                    Err(e) => Err(e),
                }
            }
        }
    }

    pub(crate) fn materialize(self: &Arc<Self>, f: &Arc<LazyFcn>) -> Result<Value> {
        if let Some(v) = f.materialized.get() {
            return Ok(v.clone());
        }
        for b in &f.bounds {
            if !is_enumerable(&b.domain) {
                return Err(EvalError::unsupported(format!(
                    "computing a function whose domain {} is not enumerable",
                    self.ppr(&b.domain)
                )));
            }
        }
        let _g = self.enter(f.node)?;
        let ctl = f.st.control;
        let st = f.st.with_control(ctl);
        let mut pairs: Vec<(Value, Value)> = Vec::new();
        let mut env = f.body_env();
        let mut result: Option<Value> = None;
        if f.arity() == 1 {
            let b = &f.bounds[0];
            let Value::Set(set) = &b.domain else {
                return Err(EvalError::bug("lazy function with a non-set domain"));
            };
            let elems = self.explore(set)?;
            for x in elems.iter() {
                self.bind_one(&mut env, &b.slots, b.tuple, x, f.node)?;
                let v = self.eval_embed(&mut env, &st, f.body)?;
                pairs.push((x.clone(), v));
            }
            if set.as_interval().is_some() {
                result = Some(self.intv_fcn(set, pairs.iter().map(|(_, v)| v.clone()).collect())?);
            }
        } else {
            let groups = self.param_tuples(&f.bounds)?;
            for args in groups {
                let mut k = 0;
                for b in &f.bounds {
                    if b.tuple {
                        self.bind_one(&mut env, &b.slots, true, &args[k], f.node)?;
                        k += 1;
                    } else {
                        for &s in &b.slots {
                            env.bind(s, Sv::Val(args[k].clone()));
                            k += 1;
                        }
                    }
                }
                let v = self.eval_embed(&mut env, &st, f.body)?;
                pairs.push((Value::tuple(args), v));
            }
        }
        let mut v = match result {
            Some(v) => v,
            None => Value::fcn(pairs, self.ctx)?,
        };
        for ex in &f.excepts {
            let new = self.embed(ex.value.clone())?;
            v = self.take_except(&v, &ex.path[ex.idx..], new)?;
        }
        let _ = f.materialized.set(v.clone());
        Ok(v)
    }

    /// A function over an interval domain `lo..hi` (TLC's `FcnRcdValue` with
    /// `intv`).
    fn intv_fcn(&self, set: &dive_value::Set, vals: Vec<Value>) -> Result<Value> {
        let Some((lo, _)) = set.as_interval() else {
            return Err(EvalError::bug("not an interval"));
        };
        if *lo == Int::ONE || vals.is_empty() {
            return Ok(Value::tuple_fcn(vals));
        }
        let mut pairs = Vec::with_capacity(vals.len());
        let mut k = lo.clone();
        for v in vals {
            pairs.push((Value::int(k.clone()), v));
            k = k.add(&Int::ONE, dive_value::IntMode::Big)?;
        }
        Ok(Value::fcn(pairs, self.ctx)?)
    }

    /// `FcnParams.Enumerator`: the argument tuples of a multi-argument
    /// function, first argument varying fastest.
    fn param_tuples(self: &Arc<Self>, bounds: &[BoundVal]) -> Result<Vec<Vec<Value>>> {
        let mut columns: Vec<Arc<[Value]>> = Vec::new();
        for b in bounds {
            let Value::Set(set) = &b.domain else {
                return Err(EvalError::general(
                    "The domains of the parameters must be enumerable.",
                ));
            };
            let elems = self.explore(set)?;
            let n = if b.tuple { 1 } else { b.slots.len() };
            for _ in 0..n {
                columns.push(elems.clone());
            }
        }
        Ok(odometer(&columns))
    }

    // -----------------------------------------------------------------------------
    // The interpreter
    // -----------------------------------------------------------------------------

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per construct, kept together like Tool.evalAppl"
    )]
    fn eval_inner(self: &Arc<Self>, env: &mut Env, st: &St, e: ExprId) -> Result<Ev> {
        let ir = &*self.ir.clone();
        let node = ir.node(e);
        match &node.expr {
            Expr::Bool(_) | Expr::Num(_) | Expr::Str(_) | Expr::Decimal { .. } => {
                match &self.lits[e.index()] {
                    Lit::Val(v) => Ok(Ev::V(v.clone())),
                    Lit::Err(err) => Err(err.clone()),
                    Lit::None => Err(EvalError::bug("literal without a plan entry")),
                }
            }
            Expr::Var { var, primed } => {
                let (s, msg_node) = if *primed { (&st.s1, e) } else { (&st.s0, e) };
                if let Some(v) = s.get(var.index()) {
                    Ok(Ev::V(v.clone()))
                } else {
                    let name = ir.decl(ir.vars()[var.index()]).name.clone();
                    Err(self.undefined(&name, msg_node))
                }
            }
            Expr::Const(c) => self.eval_const(env, st, *c, e),
            Expr::Local(lr) => {
                let sv = self.slot(env, *lr)?.clone();
                self.force(env, &sv, st, e)
            }
            Expr::SubstRef { decl, primed } => self.eval_subst_ref(env, st, *decl, *primed, e),
            Expr::Def(d) => self.eval_def_use(env, st, *d, e),
            Expr::Apply { callee, args } => self.eval_apply(env, st, callee, args, e),
            Expr::Builtin { op, args } => self.eval_builtin(env, st, *op, args, e),
            Expr::Junction {
                kind,
                bulleted,
                items,
            } => self.eval_junction(env, st, *kind, *bulleted, items, e),
            Expr::Not(a) => {
                // OPCODE_lnot
                let v = self.eval(env, st, *a)?;
                match v {
                    Ev::V(Value::Bool(b)) => Ok(Ev::V(Value::Bool(!b))),
                    other => Err(EvalError::general(format!(
                        "Attempted to apply the operator ~ to a non-boolean\n({})\n{}",
                        self.kind_of(&other),
                        self.loc(e)
                    ))),
                }
            }
            Expr::Implies(a, b) => {
                // OPCODE_implies
                let p = self.eval_bool(env, st, *a, |c, v| {
                    EvalError::general(format!(
                        "Attempted to evaluate an expression of form P => Q when P was\n{}.\n{}",
                        v,
                        c.loc(e)
                    ))
                })?;
                if !p {
                    return Ok(Ev::V(Value::TRUE));
                }
                let q = self.eval_bool(env, st, *b, |c, v| {
                    EvalError::general(format!(
                        "Attempted to evaluate an expression of form P => Q when Q was\n{}.\n{}",
                        v,
                        c.loc(e)
                    ))
                })?;
                Ok(Ev::V(Value::Bool(q)))
            }
            Expr::Equiv(a, b) => {
                // OPCODE_equiv: both sides are evaluated first.
                let p = self.eval(env, st, *a)?;
                let q = self.eval(env, st, *b)?;
                match (p, q) {
                    (Ev::V(Value::Bool(p)), Ev::V(Value::Bool(q))) => {
                        Ok(Ev::V(Value::Bool(p == q)))
                    }
                    _ => Err(EvalError::general(format!(
                        "Attempted to evaluate an expression of form P <=> Q when P or Q was not a boolean.\n{}",
                        self.loc(e)
                    ))),
                }
            }
            Expr::If { cond, then, other } => {
                // OPCODE_ite
                let c = self.eval_bool(env, st, *cond, |c, v| {
                    EvalError::general(format!(
                        "A non-boolean expression ({}) was used as the condition of an IF.\n{}",
                        v,
                        c.loc(e)
                    ))
                })?;
                self.eval(env, st, if c { *then } else { *other })
            }
            Expr::Case { arms, other } => self.eval_case(env, st, arms, *other, e),
            Expr::Quant { kind, bounds, body } => match kind {
                QuantKind::Exists | QuantKind::Forall => {
                    self.eval_quant(env, st, *kind == QuantKind::Exists, bounds, *body, e)
                }
                QuantKind::Choose => self.eval_choose(env, st, bounds, *body, e),
            },
            Expr::SetFilter { bound, pred } => self.eval_filter(env, st, bound, *pred, e),
            Expr::SetMap { expr, bounds } => {
                // OPCODE_soa: eager, by the context enumerator.
                let mut vals = Vec::new();
                self.for_each_context(env, st, bounds, e, &mut |core, env| {
                    vals.push(core.eval_embed(env, st, *expr)?);
                    Ok(true)
                })?;
                Ok(Ev::V(Value::set_raw(vals, self.ctx)))
            }
            Expr::FcnCons { bounds, body } => self.eval_fcn_cons(env, st, bounds, *body, None, e),
            Expr::RecFcn { name, bounds, body } => {
                self.eval_fcn_cons(env, st, bounds, *body, Some(name.index()), e)
            }
            Expr::SetEnum(items) => {
                // OPCODE_se: an unnormalized set enumeration.
                let mut vals = Vec::with_capacity(items.len());
                for &i in items {
                    vals.push(self.eval_embed(env, st, i)?);
                }
                Ok(Ev::V(Value::set_raw(vals, self.ctx)))
            }
            Expr::Tuple(items) => {
                // OPCODE_tup
                let mut vals = Vec::with_capacity(items.len());
                for &i in items {
                    vals.push(self.eval_embed(env, st, i)?);
                }
                Ok(Ev::V(Value::tuple(vals)))
            }
            Expr::Record(fields) => {
                // OPCODE_rc
                let names = self.field_atoms(e)?;
                let mut out = Vec::with_capacity(fields.len());
                for (f, a) in fields.iter().zip(names.iter()) {
                    out.push((a.clone(), self.eval_embed(env, st, f.value)?));
                }
                Ok(Ev::V(Value::record(out, self.ctx)?))
            }
            Expr::RecordSet(fields) => {
                // OPCODE_sor
                let names = self.field_atoms(e)?;
                let mut out = Vec::with_capacity(fields.len());
                for (f, a) in fields.iter().zip(names.iter()) {
                    out.push((a.clone(), self.eval_embed(env, st, f.value)?));
                }
                Ok(Ev::V(Value::record_set(out, self.ctx)?))
            }
            Expr::FcnSet { domain, range } => {
                // OPCODE_sof
                let d = self.eval_embed(env, st, *domain)?;
                let r = self.eval_embed(env, st, *range)?;
                Ok(Ev::V(Value::fcn_set(d, r, self.ctx)))
            }
            Expr::Cartesian(items) => {
                // OPCODE_cp
                let mut sets = Vec::with_capacity(items.len());
                for &i in items {
                    sets.push(self.eval_embed(env, st, i)?);
                }
                Ok(Ev::V(Value::product(sets, self.ctx)))
            }
            Expr::FcnApply { fcn, arg } => {
                // OPCODE_fa: the function is evaluated with KeepLazy.
                let _g = self.enter(e)?;
                let f = self.eval(env, &st.keep_lazy(), *fcn)?;
                match f {
                    Ev::F(lf) => {
                        let a = self.eval_v(env, st, *arg)?;
                        self.apply_lazy(&lf, &a, st.control)
                    }
                    Ev::V(Value::Fcn(fv)) => {
                        let a = self.eval_v(env, st, *arg)?;
                        self.apply_fcn(&fv, &a).map(Ev::V)
                    }
                    Ev::V(other) => Err(EvalError::general(format!(
                        "A non-function ({}) was applied as a function.\n{}",
                        kind_string(&other),
                        self.loc(e)
                    ))),
                }
            }
            Expr::RecordSelect { record, .. } => {
                // OPCODE_rs
                let r = self.eval(env, st, *record)?;
                let name = self.field_atoms(e)?[0].clone();
                self.select_field(r, &name, e)
            }
            Expr::Except { base, updates } => self.eval_except(env, st, *base, updates, e),
            Expr::ExceptAt => match &env.dynamic.at {
                Some(v) => Ok(Ev::V(v.clone())),
                None => Err(EvalError::bug("@ outside EXCEPT")),
            },
            Expr::Let { defs, body } => {
                // Tool.evalImplLetInKind: each zero-arity definition becomes a
                // fresh LazyValue over the context that includes the previous ones.
                for &d in defs {
                    let def = ir.def(d);
                    if let DefScope::LetValue { slot, pass, .. } = def.scope {
                        let sv = match pass {
                            Pass::Strict => Sv::from(self.eval(env, st, def.body)?),
                            Pass::Lazy | Pass::Direct => {
                                Sv::Thunk(Thunk::new(def.body, env.clone(), true))
                            }
                        };
                        env.bind(slot.index(), sv);
                    }
                }
                self.eval(env, st, *body)
            }
            Expr::Subst { bindings, body } => {
                // Tool.evalImplSubstInKind: every substitution is a LazyValue over
                // the outer context.
                let outer = env.clone();
                let mut dynamic = env.dynamic.clone();
                for b in bindings {
                    let val = match &b.value {
                        Arg::Expr { expr, pass } => match pass {
                            Pass::Strict => Sv::from(self.eval(&mut outer.clone(), st, *expr)?),
                            Pass::Lazy | Pass::Direct => {
                                Sv::Thunk(Thunk::new(*expr, outer.clone(), true))
                            }
                        },
                        Arg::Op { op: o, .. } => Sv::Op(self.op_value(&outer, o)?),
                    };
                    dynamic.push(b.decl, val);
                }
                let saved = std::mem::replace(&mut env.dynamic, dynamic);
                let r = self.eval(env, st, *body);
                env.dynamic = saved;
                r
            }
            Expr::Label { body, .. } | Expr::Selected(body) => self.eval(env, st, *body),
            Expr::Prime(a) => {
                // OPCODE_prime
                let st1 = St {
                    s0: st.s1.clone(),
                    s1: StateRef::Null,
                    control: St::primed_if_enabled(st.control),
                };
                self.eval(env, &st1, *a)
            }
            Expr::Unchanged(a) => {
                // OPCODE_unchanged
                let v0 = self.eval_v(
                    env,
                    &St {
                        s0: st.s0.clone(),
                        s1: StateRef::Empty,
                        control: st.control,
                    },
                    *a,
                )?;
                let v1 = self.eval_v(
                    env,
                    &St {
                        s0: st.s1.clone(),
                        s1: StateRef::Null,
                        control: St::primed_if_enabled(st.control),
                    },
                    *a,
                )?;
                Ok(Ev::V(Value::Bool(self.equals(&v0, &v1)?)))
            }
            Expr::StepBox { action, sub } | Expr::StepAngle { action, sub } => {
                let angle = matches!(node.expr, Expr::StepAngle { .. });
                let form = if angle { "<A>_e" } else { "[A]_e" };
                let a = self.eval_bool(env, st, *action, |c, _| {
                    EvalError::general(format!(
                        "Attempted to evaluate an expression of form {form}, but A was not a boolean.\n{}",
                        c.loc(e)
                    ))
                })?;
                if angle && !a {
                    return Ok(Ev::V(Value::FALSE));
                }
                if !angle && a {
                    return Ok(Ev::V(Value::TRUE));
                }
                let v0 = self.eval_v(
                    env,
                    &St {
                        s0: st.s0.clone(),
                        s1: StateRef::Empty,
                        control: st.control,
                    },
                    *sub,
                )?;
                let v1 = self.eval_v(
                    env,
                    &St {
                        s0: st.s1.clone(),
                        s1: StateRef::Null,
                        control: St::primed_if_enabled(st.control),
                    },
                    *sub,
                )?;
                let same = self.equals(&v0, &v1)?;
                Ok(Ev::V(Value::Bool(if angle { !same } else { same })))
            }
            Expr::Enabled(a) => self.eval_enabled(env, st, *a, e),
            Expr::Cdot(..) => Err(EvalError::general(format!(
                "The current version of TLC does not support action composition.  An incomplete implementation can be enabled via the tlc2.tool.impl.Tool.cdot=true java property.\n{}",
                self.loc(e)
            ))),
            Expr::Always(_) => Err(self.temporal("[]A", e)),
            Expr::Eventually(_) => Err(self.temporal("<>A", e)),
            Expr::LeadsTo(..) => Err(self.temporal("a ~> b", e)),
            Expr::WhilePlus(..) => Err(self.temporal("a -+-> formula", e)),
            Expr::Wf { .. } => Err(self.temporal("WF", e)),
            Expr::Sf { .. } => Err(self.temporal("SF", e)),
            Expr::TemporalQuant { exists, .. } => {
                Err(self.temporal(if *exists { "\\EE" } else { "\\AA" }, e))
            }
            Expr::Unbound(name) => Err(self.undefined(name, e)),
            Expr::Unevaluable(_) => Err(EvalError::general(format!(
                "Attempted to evaluate an expression that cannot be evaluated.\n{}",
                self.loc(e)
            ))),
        }
    }

    fn temporal(&self, what: &str, e: ExprId) -> EvalError {
        EvalError::coded(
            Ec::FormulaInPredicate,
            format!(
                "TLC encountered a temporal formula ({what}) when evaluating a predicate or action.\n{}",
                self.loc(e)
            ),
        )
    }

    pub(crate) fn undefined(&self, name: &str, e: ExprId) -> EvalError {
        EvalError::coded(
            Ec::UndefinedOrNoOperator,
            format!(
                "In evaluation, the identifier {name} is either undefined or not an operator.\n{}",
                self.loc(e)
            ),
        )
    }

    fn field_atoms(&self, e: ExprId) -> Result<&[dive_value::Atom]> {
        self.fields[e.index()]
            .as_deref()
            .ok_or_else(|| EvalError::bug("record node without interned fields"))
    }

    // -----------------------------------------------------------------------------
    // Names: slots, constants, substitutions, definitions
    // -----------------------------------------------------------------------------

    pub(crate) fn slot<'a>(&self, env: &'a Env, lr: LocalRef) -> Result<&'a Sv> {
        env.frame_up(lr.up)
            .and_then(|f| f.slots.get(lr.slot.index()))
            .ok_or_else(|| EvalError::bug(format!("unbound slot {lr:?}")))
    }

    /// Forces what a name is bound to (`Tool.evalAppl`, the `LazyValue` branch).
    pub(crate) fn force(
        self: &Arc<Self>,
        env: &mut Env,
        sv: &Sv,
        st: &St,
        e: ExprId,
    ) -> Result<Ev> {
        match sv {
            Sv::Val(v) => Ok(Ev::V(v.clone())),
            Sv::Fcn(f) => Ok(Ev::F(f.clone())),
            Sv::Thunk(t) => {
                let _g = self.enter(e)?;
                if matches!(st.s1, StateRef::Null) {
                    // TLC does not use or fill the cache when s1 is null.
                    let st1 = St {
                        s0: st.s0.clone(),
                        s1: StateRef::Null,
                        control: st.control,
                    };
                    return self.eval(&mut t.env.clone(), &st1, t.expr);
                }
                if let Some(v) = t.cached(st) {
                    return Ok(v);
                }
                let v = self.eval(&mut t.env.clone(), st, t.expr)?;
                t.store(st, &v);
                Ok(v)
            }
            Sv::Op(op) => {
                let op = op.clone();
                self.apply_op(&op, env, st, &[], e)
            }
            Sv::Unset => Err(EvalError::bug("read of an unbound slot")),
        }
    }

    fn eval_const(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        c: dive_ir::ConstId,
        e: ExprId,
    ) -> Result<Ev> {
        match self.consts.get(c.index()).and_then(Option::as_ref) {
            Some(ConstBinding::Value(v)) => Ok(Ev::V(v.clone())),
            Some(ConstBinding::Def(d)) => {
                let d = *d;
                self.eval_def_use(env, st, d, e)
            }
            None => {
                let name = self
                    .ir
                    .consts()
                    .get(c.index())
                    .map_or("?", |d| self.ir.decl(*d).name.as_str());
                Err(self.undefined(name, e))
            }
        }
    }

    fn eval_subst_ref(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        decl: dive_ir::DeclId,
        primed: bool,
        e: ExprId,
    ) -> Result<Ev> {
        let d = self.ir.decl(decl);
        // `Tool.evalAppl` looks a name up with `cutoff = isPrimed(control)`;
        // under `ENABLED` that hides substitutions of variables made outside
        // it (`Context.branch`).
        let cutoff = d.variable && (st.is(control::PRIMED) || (primed && st.is(control::ENABLED)));
        let found = env.dynamic.lookup_cut(decl, cutoff).cloned();
        if found.is_none() && cutoff && d.var.is_none() {
            // The lookup falls back to the declaration itself, and TLC reads
            // the state by the variable's name.
            let s = if primed { &st.s1 } else { &st.s0 };
            return match self.root_var_named(&d.name).and_then(|i| s.get(i)) {
                Some(v) => Ok(Ev::V(v.clone())),
                None => Err(self.undefined(&d.name, e)),
            };
        }
        if let Some(sv) = found {
            if primed {
                // `x'` with `x <- e`: OPCODE_prime evaluates the substitute in the
                // successor state, with no successor (so the cache is bypassed).
                let st1 = St {
                    s0: st.s1.clone(),
                    s1: StateRef::Null,
                    control: St::primed_if_enabled(st.control),
                };
                return self.force(env, &sv, &st1, e);
            }
            return self.force(env, &sv, st, e);
        }
        if let Some(var) = d.var {
            let s = if primed { &st.s1 } else { &st.s0 };
            return match s.get(var.index()) {
                Some(v) => Ok(Ev::V(v.clone())),
                None => Err(self.undefined(&d.name, e)),
            };
        }
        if let Some(c) = d.constant {
            if primed {
                return self.eval_const(
                    env,
                    &St {
                        s0: st.s1.clone(),
                        s1: StateRef::Null,
                        control: st.control,
                    },
                    c,
                    e,
                );
            }
            return self.eval_const(env, st, c, e);
        }
        Err(self.undefined(&d.name, e))
    }

    /// The root variable TLC finds by `name` (`TLCState.lookup` by the
    /// `UniqueString`'s variable location).
    pub(crate) fn root_var_named(&self, name: &str) -> Option<usize> {
        self.ir
            .vars()
            .iter()
            .position(|&v| self.ir.decl(v).name == name)
    }

    pub(crate) fn redirect(&self, d: dive_ir::DefId) -> dive_ir::DefId {
        let mut d = d;
        for _ in 0..64 {
            match self.redirects.get(&d) {
                Some(&t) if t != d => d = t,
                _ => break,
            }
        }
        d
    }

    /// A use of a zero-arity definition.
    fn eval_def_use(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        d: dive_ir::DefId,
        e: ExprId,
    ) -> Result<Ev> {
        let d = self.redirect(d);
        if let Some(v) = self.def_values.get(&d) {
            return Ok(Ev::V(v.clone()));
        }
        let def = self.ir.def(d);
        if let Some(op) = def.std {
            return self.eval_std(env, st, op, &[], e);
        }
        if self.cacheable(d) {
            if let Some(v) = self.def_cache[d.index()].get() {
                return Ok(Ev::V(v.clone()));
            }
            // TLC evaluates constant definitions once, at start-up, in an empty
            // context; only successes are kept.
            let _g = self.enter(e)?;
            let mut fresh = Env::new(d, def.depth, def.frame.len(), None, Dyn::default());
            let v = self.eval(&mut fresh, &St::constant(), def.body)?;
            if let Ev::V(v) = &v {
                let _ = self.def_cache[d.index()].set(v.clone());
            }
            return Ok(v);
        }
        self.call_def(env, st, d, None, &[], e)
    }

    /// Whether a definition's value can be computed once per run, as TLC's
    /// `SpecProcessor.processConstantDefns` does: zero arity, constant level,
    /// top level, and not from a module that is instantiated (whose
    /// definitions depend on the instance's substitutions).
    pub(crate) fn cacheable(&self, d: dive_ir::DefId) -> bool {
        let def = self.ir.def(d);
        def.params.is_empty()
            && def.level == dive_ir::Level::Constant
            && def.scope == DefScope::TopLevel
            && matches!(
                def.kind,
                dive_ir::DefKind::Operator | dive_ir::DefKind::Function
            )
            && !self.ir.module(def.module).instantiated
            && !def.effects.intersects(dive_ir::Effects::IMPURE)
            && (self.open_ok || !def.effects.intersects(dive_ir::Effects::OPEN))
    }

    /// Calls a user definition: a new frame with the arguments bound
    /// (`getOpContext`: every expression argument is a `LazyValue`).
    fn call_def(
        self: &Arc<Self>,
        caller: &mut Env,
        st: &St,
        d: dive_ir::DefId,
        parent: Option<Arc<crate::env::Frame>>,
        args: &[Arg],
        e: ExprId,
    ) -> Result<Ev> {
        let mut callee = self.callee_env(caller, st, d, parent, args)?;
        let _g = self.enter(e)?;
        self.eval(&mut callee, st, self.ir.def(d).body)
    }

    /// The environment a call of `d` evaluates its body in: a new frame with
    /// the arguments bound (`getOpContext`), in the caller's dynamic context.
    pub(crate) fn callee_env(
        self: &Arc<Self>,
        caller: &mut Env,
        st: &St,
        d: dive_ir::DefId,
        parent: Option<Arc<crate::env::Frame>>,
        args: &[Arg],
    ) -> Result<Env> {
        let def = self.ir.def(d);
        if args.len() != def.params.len() {
            return Err(EvalError::bug(format!(
                "{} applied to {} arguments, expects {}",
                def.name,
                args.len(),
                def.params.len()
            )));
        }
        let parent = match def.scope {
            DefScope::Local { .. } | DefScope::Inline { .. } => match parent {
                Some(p) => Some(p),
                None => Some(
                    caller
                        .frame_at_depth(def.depth.saturating_sub(1))
                        .cloned()
                        .ok_or_else(|| {
                            EvalError::bug(format!("no enclosing frame for {}", def.name))
                        })?,
                ),
            },
            _ => None,
        };
        let mut callee = Env::new(
            d,
            def.depth,
            def.frame.len(),
            parent,
            caller.dynamic.clone(),
        );
        for (i, a) in args.iter().enumerate() {
            let sv = self.arg_value(caller, st, a)?;
            callee.bind(i, sv);
        }
        Ok(callee)
    }

    /// What an argument binds a parameter to.
    pub(crate) fn arg_value(self: &Arc<Self>, caller: &mut Env, st: &St, a: &Arg) -> Result<Sv> {
        Ok(match a {
            Arg::Expr { expr, pass } => match pass {
                Pass::Lazy => Sv::Thunk(Thunk::new(*expr, caller.clone(), true)),
                Pass::Strict | Pass::Direct => Sv::from(self.eval(caller, st, *expr)?),
            },
            Arg::Op { op: o, .. } => Sv::Op(self.op_value(caller, o)?),
        })
    }

    /// The operator an operator argument denotes (`getVal` for an
    /// `OpArgNode`: looked up in the current context).
    pub(crate) fn op_value(&self, env: &Env, o: &OpRef) -> Result<OpVal> {
        Ok(match o {
            OpRef::Def(d) => {
                let d = self.redirect(*d);
                let def = self.ir.def(d);
                let parent = match def.scope {
                    DefScope::Local { .. } | DefScope::Inline { .. } => {
                        env.frame_at_depth(def.depth.saturating_sub(1)).cloned()
                    }
                    _ => None,
                };
                OpVal::Def {
                    def: d,
                    parent,
                    dynamic: None,
                }
            }
            OpRef::Param(lr) => match self.slot(env, *lr)? {
                Sv::Op(op) => op.clone(),
                _ => return Err(EvalError::bug("operator parameter bound to a value")),
            },
            OpRef::Std(op) => OpVal::Std(*op),
            OpRef::Builtin(op) => OpVal::Builtin(*op),
            OpRef::Const(c) => match self.consts.get(c.index()).and_then(Option::as_ref) {
                Some(ConstBinding::Def(d)) => OpVal::Def {
                    def: self.redirect(*d),
                    parent: None,
                    dynamic: None,
                },
                _ => OpVal::Const(*c),
            },
            OpRef::Subst(decl) => match env.dynamic.lookup(*decl) {
                Some(Sv::Op(op)) => op.clone(),
                Some(_) => {
                    return Err(EvalError::unsupported(
                        "an operator substituted by a non-operator expression",
                    ));
                }
                None => match self.ir.decl(*decl).constant {
                    Some(c) => self.op_value(env, &OpRef::Const(c))?,
                    None => OpVal::Unbound(self.ir.decl(*decl).name.as_str().into()),
                },
            },
            OpRef::Unbound(name) => OpVal::Unbound(name.as_ref().into()),
        })
    }

    fn eval_apply(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        callee: &OpRef,
        args: &[Arg],
        e: ExprId,
    ) -> Result<Ev> {
        match callee {
            OpRef::Def(d) => {
                let d = self.redirect(*d);
                if let Some(op) = self.ir.def(d).std {
                    return self.eval_std(env, st, op, args, e);
                }
                if args.is_empty() {
                    return self.eval_def_use(env, st, d, e);
                }
                self.call_def(env, st, d, None, args, e)
            }
            OpRef::Std(op) => self.eval_std(env, st, *op, args, e),
            _ => {
                let op = self.op_value(env, callee)?;
                self.apply_op(&op, env, st, args, e)
            }
        }
    }

    /// Applies an operator value to arguments evaluated in `env`.
    pub(crate) fn apply_op(
        self: &Arc<Self>,
        op: &OpVal,
        env: &mut Env,
        st: &St,
        args: &[Arg],
        e: ExprId,
    ) -> Result<Ev> {
        match op {
            OpVal::Def {
                def,
                parent,
                dynamic,
            } => {
                if let Some(s) = self.ir.def(*def).std {
                    return self.eval_std(env, st, s, args, e);
                }
                match dynamic {
                    Some(d) => {
                        let saved = std::mem::replace(&mut env.dynamic, d.clone());
                        let r = self.call_def(env, st, *def, parent.clone(), args, e);
                        env.dynamic = saved;
                        r
                    }
                    None => self.call_def(env, st, *def, parent.clone(), args, e),
                }
            }
            OpVal::Std(s) => self.eval_std(env, st, *s, args, e),
            OpVal::Builtin(b) => {
                let mut exprs = Vec::with_capacity(args.len());
                for a in args {
                    match a {
                        Arg::Expr { expr, .. } => exprs.push(*expr),
                        Arg::Op { .. } => {
                            return Err(EvalError::unsupported(
                                "a built-in operator applied to an operator argument",
                            ));
                        }
                    }
                }
                self.eval_builtin(env, st, *b, &exprs, e)
            }
            OpVal::Const(c) => match self.consts.get(c.index()).and_then(Option::as_ref) {
                Some(ConstBinding::Def(d)) => {
                    self.call_def(env, st, self.redirect(*d), None, args, e)
                }
                Some(ConstBinding::Value(v)) if args.is_empty() => Ok(Ev::V(v.clone())),
                _ => {
                    let name = self
                        .ir
                        .consts()
                        .get(c.index())
                        .map_or("?", |d| self.ir.decl(*d).name.as_str());
                    Err(self.undefined(name, e))
                }
            },
            OpVal::Unbound(name) => Err(self.undefined(name, e)),
        }
    }

    // -----------------------------------------------------------------------------
    // Logic
    // -----------------------------------------------------------------------------

    fn eval_junction(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        kind: JunctionKind,
        bulleted: bool,
        items: &[ExprId],
        e: ExprId,
    ) -> Result<Ev> {
        let and = kind == JunctionKind::And;
        if bulleted || items.len() != 2 {
            // OPCODE_cl / OPCODE_dl
            let what = if and { "conjunction" } else { "disjunction" };
            for &i in items {
                let b = self.eval_bool(env, st, i, |c, v| {
                    EvalError::general(format!(
                        "A non-boolean expression ({}) was used as a formula in a {what}.\n{}",
                        v,
                        c.loc(i)
                    ))
                })?;
                if b != and {
                    return Ok(Ev::V(Value::Bool(b)));
                }
            }
            return Ok(Ev::V(Value::Bool(and)));
        }
        // OPCODE_land / OPCODE_lor
        let sym = if and { "P /\\ Q" } else { "P \\/ Q" };
        let p = self.eval_bool(env, st, items[0], |c, v| {
            EvalError::general(format!(
                "Attempted to evaluate an expression of form {sym} when P was\n{}.\n{}",
                v,
                c.loc(e)
            ))
        })?;
        if p != and {
            return Ok(Ev::V(Value::Bool(p)));
        }
        let q = self.eval_bool(env, st, items[1], |c, v| {
            EvalError::general(format!(
                "Attempted to evaluate an expression of form {sym} when Q was\n{}.\n{}",
                v,
                c.loc(e)
            ))
        })?;
        Ok(Ev::V(Value::Bool(q)))
    }

    fn eval_case(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        arms: &[CaseArm],
        other: Option<ExprId>,
        e: ExprId,
    ) -> Result<Ev> {
        // OPCODE_case
        for arm in arms {
            let g = arm.guard;
            let b = self.eval_bool(env, st, g, |c, v| {
                EvalError::general(format!(
                    "A non-boolean expression ({}) was used as a condition of a CASE. {}",
                    v,
                    c.loc(g)
                ))
            })?;
            if b {
                return self.eval(env, st, arm.value);
            }
        }
        match other {
            Some(o) => self.eval(env, st, o),
            None => Err(EvalError::general(format!(
                "Attempted to evaluate a CASE with no conditions true.\n{}",
                self.loc(e)
            ))),
        }
    }

    // -----------------------------------------------------------------------------
    // Binders
    // -----------------------------------------------------------------------------

    /// Binds one element to a group's variables (a tuple pattern
    /// destructures it).
    pub(crate) fn bind_one(
        &self,
        env: &mut Env,
        slots: &[usize],
        tuple: bool,
        x: &Value,
        e: ExprId,
    ) -> Result<()> {
        if !tuple {
            if let Some(&s) = slots.first() {
                env.bind(s, Sv::Val(x.clone()));
            }
            return Ok(());
        }
        let elems = match x {
            Value::Fcn(f) => f.as_tuple(),
            _ => None,
        };
        match elems {
            Some(t) if t.len() == slots.len() => {
                for (&s, v) in slots.iter().zip(t.iter()) {
                    env.bind(s, Sv::Val(v.clone()));
                }
                Ok(())
            }
            _ => Err(EvalError::unsupported(format!(
                "a tuple pattern bound to the non-matching value {} ({})",
                self.ppr(x),
                self.loc(e)
            ))),
        }
    }

    /// `Tool.contexts` and `ContextEnumerator`: evaluates every group's
    /// domain in order, then calls `body` for each binding, the first
    /// variable varying fastest. `body` returns `false` to stop.
    pub(crate) fn for_each_context(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        bounds: &[Bound],
        e: ExprId,
        body: &mut dyn FnMut(&Arc<Self>, &mut Env) -> Result<bool>,
    ) -> Result<()> {
        let mut binders: Vec<Binder> = Vec::new();
        let mut columns: Vec<Arc<[Value]>> = Vec::new();
        for b in bounds {
            let Some(dom) = b.domain else {
                return Err(EvalError::bug("unbounded binder in a bounded construct"));
            };
            let set = self.eval_v(env, st, dom)?;
            let Value::Set(s) = &set else {
                return Err(self.non_enumerable_bound(&set, dom));
            };
            if !is_enumerable(&set) {
                return Err(self.non_enumerable_bound(&set, dom));
            }
            let elems = self.explore(s)?;
            if b.tuple {
                binders.push(Binder {
                    slots: b.vars.iter().map(|s| s.index()).collect(),
                    tuple: true,
                });
                columns.push(elems);
            } else {
                for v in &b.vars {
                    binders.push(Binder {
                        slots: vec![v.index()],
                        tuple: false,
                    });
                    columns.push(elems.clone());
                }
            }
        }
        if columns.iter().any(|c| c.is_empty()) {
            return Ok(());
        }
        let mut idx = vec![0usize; columns.len()];
        loop {
            for (k, b) in binders.iter().enumerate() {
                let x = &columns[k][idx[k]];
                if b.tuple {
                    match x {
                        Value::Fcn(f) if f.as_tuple().is_some_and(|t| t.len() == b.slots.len()) => {
                            self.bind_one(env, &b.slots, true, x, e)?;
                        }
                        _ => {
                            return Err(EvalError::coded(
                                Ec::ArgumentMismatch,
                                "Argument mismatch in operator application.%1",
                            ));
                        }
                    }
                } else {
                    env.bind(b.slots[0], Sv::Val(x.clone()));
                }
            }
            if !body(self, env)? {
                return Ok(());
            }
            // Advance the odometer, first position fastest.
            let mut k = 0;
            loop {
                if k == columns.len() {
                    return Ok(());
                }
                idx[k] += 1;
                if idx[k] < columns[k].len() {
                    break;
                }
                idx[k] = 0;
                k += 1;
            }
        }
    }

    fn non_enumerable_bound(&self, v: &Value, dom: ExprId) -> EvalError {
        // `Value.getNonEnumerableErrorMsg`, overridden by `Naturals`,
        // `Integers` and `Sequences`.
        let p = self.ppr(v);
        let loc = self.loc(dom);
        let kind = match v {
            Value::Set(s) => Some(s.kind()),
            _ => None,
        };
        let infinite = |set: &str| {
            format!(
                "TLC encountered the non-enumerable quantifier bound\n{p}\n{loc}\nThe set {set} contains infinitely many elements. As a result, TLC cannot evaluate expressions that\nuniversally (\\A) or existentially (\\E) quantify over {p}, because this would require checking an\ninfinite number of cases. Note that TLC handles set membership like T \\subseteq {set} for any finite set T."
            )
        };
        EvalError::general(match kind {
            Some(dive_value::SetKind::Nat) => infinite("Nat"),
            Some(dive_value::SetKind::Int) => infinite("Int"),
            Some(dive_value::SetKind::Seq) => format!(
                "TLC encountered the non-enumerable quantifier bound\n{p}\n{loc}\nIn TLA+, Seq(S) represents the set of all finite sequences whose elements come from the set S. Even when S\nis a finite set, the number of possible sequences in Seq(S) is unbounded because sequences can have any\nfinite length (e.g., length 0, 1, 2, and so on). As a result, TLC cannot evaluate expressions that\nuniversally (\\A) or existentially (\\E) quantify over {p}, because this would require checking an\ninfinite number of cases. Note that for a finite set of sequences s, TLC handles s \\subseteq Seq(S).\nSee https://explain.tlapl.us/seq-unenumerable for additional details."
            ),
            _ => format!("TLC encountered a non-enumerable quantifier bound\n{p}.\n{loc}"),
        })
    }

    fn eval_quant(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        exists: bool,
        bounds: &[Bound],
        body: ExprId,
        e: ExprId,
    ) -> Result<Ev> {
        if bounds.iter().any(|b| b.domain.is_none()) {
            // OPCODE_ue / OPCODE_uf
            let (q, form) = if exists {
                ("\\E", "\\E x \\in S: P(x)")
            } else {
                ("\\A", "\\A x \\in S: P(x)")
            };
            return Err(EvalError::general(format!(
                "TLC attempted to evaluate an unbounded {q}.\nMake sure that the expression is of form {form}.\n{}",
                self.loc(e)
            )));
        }
        // OPCODE_be / OPCODE_bf
        let mut result = !exists;
        self.for_each_context(env, st, bounds, e, &mut |core, env| {
            let b = core.eval_bool(env, st, body, |c, _| {
                EvalError::coded(
                    Ec::ExpectedValue,
                    format!(
                        "TLC expected a boolean value, but did not find one. {}",
                        c.loc(e)
                    ),
                )
            })?;
            if b == exists {
                result = exists;
                return Ok(false);
            }
            Ok(true)
        })?;
        Ok(Ev::V(Value::Bool(result)))
    }

    fn eval_choose(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        bounds: &[Bound],
        body: ExprId,
        e: ExprId,
    ) -> Result<Ev> {
        let Some(b) = bounds.first() else {
            return Err(EvalError::bug("CHOOSE without a bound"));
        };
        let Some(dom) = b.domain else {
            // OPCODE_uc
            return Err(EvalError::general(format!(
                "TLC attempted to evaluate an unbounded CHOOSE.\nMake sure that the expression is of form CHOOSE x \\in S: P(x).\n{}",
                self.loc(e)
            )));
        };
        // OPCODE_bc: the set is normalized and walked in the run's value order.
        let set = self.eval_v(env, st, dom)?;
        let Value::Set(s) = &set else {
            return Err(self.choose_not_enumerable(e));
        };
        if !is_enumerable(&set) {
            return Err(self.choose_not_enumerable(e));
        }
        self.enum_guard(s)?;
        let elems = s.elements(self.ctx)?;
        let slots: Vec<usize> = b.vars.iter().map(|s| s.index()).collect();
        for x in elems.iter() {
            if b.tuple {
                let ok = matches!(x, Value::Fcn(f) if f.as_tuple().is_some_and(|t| t.len() == slots.len()));
                if !ok {
                    return Err(EvalError::general(format!(
                        "Attempted to compute the value of an expression of form\nCHOOSE <<x1, ... , xN>> \\in S: P, but S was not a set\nof N-tuples.\n{}",
                        self.loc(e)
                    )));
                }
            }
            self.bind_one(env, &slots, b.tuple, x, e)?;
            let p = self.eval_bool(env, st, body, |c, _| {
                EvalError::coded(
                    Ec::ExpectedValue,
                    format!(
                        "TLC expected a boolean value, but did not find one. {}",
                        c.loc(e)
                    ),
                )
            })?;
            if p {
                return Ok(Ev::V(x.clone()));
            }
        }
        Err(EvalError::general(format!(
            "Attempted to compute the value of an expression of form\nCHOOSE x \\in S: P, but no element of S satisfied P.\n{}",
            self.loc(e)
        )))
    }

    fn choose_not_enumerable(&self, e: ExprId) -> EvalError {
        EvalError::general(format!(
            "Attempted to compute the value of an expression of\nform CHOOSE x \\in S: P, but S was not enumerable.\n{}",
            self.loc(e)
        ))
    }

    fn eval_filter(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        bound: &Bound,
        pred: ExprId,
        e: ExprId,
    ) -> Result<Ev> {
        // OPCODE_sso
        let Some(dom) = bound.domain else {
            return Err(EvalError::bug("set filter without a domain"));
        };
        let in_val = self.eval_v(env, st, dom)?;
        let slots: Vec<usize> = bound.vars.iter().map(|s| s.index()).collect();
        if is_reducible(&in_val) {
            let Value::Set(s) = &in_val else {
                return Err(EvalError::bug("reducible non-set"));
            };
            let elems = self.explore(s)?;
            let mut out = Vec::new();
            for x in elems.iter() {
                if bound.tuple && !matches!(x, Value::Fcn(f) if f.as_tuple().is_some()) {
                    return Err(EvalError::unsupported(format!(
                        "a set filter with a tuple pattern over a set containing the non-tuple {}",
                        self.ppr(x)
                    )));
                }
                self.bind_one(env, &slots, bound.tuple, x, e)?;
                let keep = self.eval_bool(env, st, pred, |c, v| {
                    EvalError::general(format!(
                        "Attempted to evaluate an expression of form {{x \\in S : P(x)}} when P was {}.\n{}",
                        v,
                        c.loc(pred)
                    ))
                })?;
                if keep {
                    out.push(x.clone());
                }
            }
            // `new SetEnumValue(vals, inVal.isNormalized())`: enumerating a set
            // enumeration normalizes it, so the result is normalized.
            return Ok(Ev::V(Value::set(out, self.ctx)?));
        }
        let f = Filter {
            core: self.clone(),
            node: e,
            slots,
            tuple: bound.tuple,
            in_val,
            pred,
            env: env.clone(),
            st: st.clone(),
        };
        Ok(Ev::V(Value::deferred(Arc::new(f), self.ctx)))
    }

    /// `SetPredValue.member`.
    pub(crate) fn filter_member(self: &Arc<Self>, f: &Filter, elem: &Value) -> Result<bool> {
        if !self.member(elem, &f.in_val)? {
            return Ok(false);
        }
        let mut env = f.env.clone();
        if f.tuple {
            match elem {
                Value::Fcn(t) if t.as_tuple().is_some_and(|t| t.len() == f.slots.len()) => {
                    self.bind_one(&mut env, &f.slots, true, elem, f.node)?;
                }
                _ => {
                    return Err(EvalError::general(format!(
                        "Attempted to check if the value:\n{}\nis an element of a set of {}-tuples.",
                        self.ppr(elem),
                        f.slots.len()
                    )));
                }
            }
        } else {
            self.bind_one(&mut env, &f.slots, false, elem, f.node)?;
        }
        match self.eval(&mut env, &f.st, f.pred)? {
            Ev::V(Value::Bool(b)) => Ok(b),
            _ => Err(EvalError::general(format!(
                "The evaluation of predicate {} yielded non-Boolean value.",
                self.loc(f.pred)
            ))),
        }
    }

    /// `SetPredValue.Enumerator`.
    pub(crate) fn filter_elements(self: &Arc<Self>, f: &Filter) -> Result<Vec<Value>> {
        let Value::Set(s) = &f.in_val else {
            return Err(self.filter_not_enumerable(f));
        };
        if !is_enumerable(&f.in_val) {
            return Err(self.filter_not_enumerable(f));
        }
        let elems = self.explore_unsized(s)?;
        self.filter_elements_of(f, &elems)
    }

    /// The elements of `elems` the filter keeps, in order.
    pub(crate) fn filter_elements_of(
        self: &Arc<Self>,
        f: &Filter,
        elems: &[Value],
    ) -> Result<Vec<Value>> {
        if !is_enumerable(&f.in_val) {
            return Err(self.filter_not_enumerable(f));
        }
        let mut env = f.env.clone();
        let mut out = Vec::new();
        for x in elems {
            if f.tuple {
                match x {
                    Value::Fcn(t) if t.as_tuple().is_some_and(|t| t.len() == f.slots.len()) => {
                        self.bind_one(&mut env, &f.slots, true, x, f.node)?;
                    }
                    _ => {
                        return Err(EvalError::general(format!(
                            "Attempted to check if the value:\n{}\nis an element of a set of {}-tuples.",
                            self.ppr(x),
                            f.slots.len()
                        )));
                    }
                }
            } else {
                self.bind_one(&mut env, &f.slots, false, x, f.node)?;
            }
            match self.eval(&mut env, &f.st, f.pred)? {
                Ev::V(Value::Bool(true)) => out.push(x.clone()),
                Ev::V(Value::Bool(false)) => {}
                _ => {
                    return Err(EvalError::general(format!(
                        "The evaluation of predicate {} yielded non-Boolean value.",
                        self.loc(f.pred)
                    )));
                }
            }
        }
        Ok(out)
    }

    fn filter_not_enumerable(&self, f: &Filter) -> EvalError {
        EvalError::general(format!(
            "Attempted to enumerate {{ x \\in S : p(x) }} when S:\n{}\nis not enumerable",
            self.ppr(&f.in_val)
        ))
    }

    fn eval_fcn_cons(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        bounds: &[Bound],
        body: ExprId,
        rec_slot: Option<usize>,
        e: ExprId,
    ) -> Result<Ev> {
        // OPCODE_fc / OPCODE_nrfs / OPCODE_rfs
        let mut vals = Vec::with_capacity(bounds.len());
        let mut reducible = true;
        for b in bounds {
            let Some(dom) = b.domain else {
                return Err(EvalError::bug("function constructor without a domain"));
            };
            let d = self.eval_v(env, st, dom)?;
            reducible &= is_reducible(&d);
            vals.push(BoundVal {
                slots: b.vars.iter().map(|s| s.index()).collect(),
                tuple: b.tuple,
                domain: d,
            });
        }
        let mut control = st.control;
        if rec_slot.is_some() {
            control |= control::KEEP_LAZY;
            reducible = false;
        }
        let f = Arc::new(LazyFcn {
            core: self.clone(),
            node: e,
            bounds: vals,
            body,
            env: env.clone(),
            st: st.with_control(control),
            rec_slot,
            excepts: Vec::new(),
            materialized: std::sync::OnceLock::new(),
        });
        if reducible && !st.is(control::KEEP_LAZY) {
            return self.materialize(&f).map(Ev::V);
        }
        Ok(Ev::F(f))
    }

    // -----------------------------------------------------------------------------
    // Functions and records
    // -----------------------------------------------------------------------------

    /// `FcnLambdaValue.apply`.
    #[allow(
        clippy::too_many_lines,
        reason = "one port of FcnLambdaValue.apply, kept together for comparison"
    )]
    pub(crate) fn apply_lazy(
        self: &Arc<Self>,
        f: &Arc<LazyFcn>,
        arg: &Value,
        control: u8,
    ) -> Result<Ev> {
        if let Some(v) = f.materialized.get() {
            let Value::Fcn(fv) = v else {
                return Err(EvalError::bug(
                    "materialized lazy function is not a function",
                ));
            };
            return self.apply_fcn(fv, arg).map(Ev::V);
        }
        // The pending EXCEPTs, latest first.
        let mut rest: Vec<Except> = Vec::new();
        let mut result: Option<Ev> = None;
        for ex in f.excepts.iter().rev() {
            if tlc::equals(ex.current(), arg, self.ctx)? {
                if ex.is_last() {
                    result = Some(ex.value.clone());
                    break;
                }
                rest.push(ex.advanced());
            }
        }
        let res = if let Some(r) = result {
            r
        } else {
            {
                let mut env = f.body_env();
                if f.arity() == 1 {
                    let b = &f.bounds[0];
                    if !self.member(arg, &b.domain)? {
                        return Err(EvalError::general(format!(
                            "In applying the function\n{},\nthe first argument is:\n{}\nwhich is not in its domain.\n",
                            self.lazy_string(f),
                            self.ppr(arg)
                        )));
                    }
                    if b.tuple {
                        match arg {
                            Value::Fcn(t) if t.as_tuple().is_some() => {
                                let n = t.as_tuple().map_or(0, <[Value]>::len);
                                if n != b.slots.len() {
                                    return Err(EvalError::unsupported(
                                        "applying a function with a tuple parameter to a tuple of the wrong length",
                                    ));
                                }
                                self.bind_one(&mut env, &b.slots, true, arg, f.node)?;
                            }
                            _ => {
                                return Err(EvalError::general(format!(
                                    "In applying the function\n{},\nthe first argument is:\n{}\nwhich does not match its formal parameter.\n",
                                    self.lazy_string(f),
                                    self.ppr(arg)
                                )));
                            }
                        }
                    } else {
                        env.bind(b.slots[0], Sv::Val(arg.clone()));
                    }
                } else {
                    let Some(elems) = (match arg {
                        Value::Fcn(t) => t.as_tuple(),
                        _ => None,
                    }) else {
                        return Err(EvalError::general(format!(
                            "In applying the function\n{},\nthe argument list is:\n{}\nwhich does not match its formal parameter.\n",
                            self.lazy_string(f),
                            self.ppr(arg)
                        )));
                    };
                    if elems.len() != f.arity() {
                        return Err(EvalError::unsupported(
                            "applying a function to an argument tuple of the wrong length",
                        ));
                    }
                    let mut k = 0;
                    for b in &f.bounds {
                        if b.tuple {
                            if !self.member(&elems[k], &b.domain)? {
                                return Err(EvalError::general(format!(
                                    "In applying the function\n{},\nthe argument number {} is:\n{}\nwhich is not in its domain.\n",
                                    self.lazy_string(f),
                                    k + 1,
                                    self.ppr(&elems[k])
                                )));
                            }
                            self.bind_one(&mut env, &b.slots, true, &elems[k], f.node)?;
                            k += 1;
                        } else {
                            for &s in &b.slots {
                                if !self.member(&elems[k], &b.domain)? {
                                    return Err(EvalError::unsupported(
                                        "applying a multi-argument lazy function outside its domain (TLC's message prints the domain)",
                                    ));
                                }
                                env.bind(s, Sv::Val(elems[k].clone()));
                                k += 1;
                            }
                        }
                    }
                }
                let st = f.st.with_control(control);
                let _g = self.enter(f.node)?;
                self.eval(&mut env, &st, f.body)?
            }
        };
        if rest.is_empty() {
            return Ok(res);
        }
        // `res.takeExcept(excepts2)`, oldest first.
        rest.reverse();
        let mut v = res;
        for ex in rest {
            v = self.take_except_ev(v, &ex.path[ex.idx..], ex.value.clone())?;
        }
        Ok(v)
    }

    /// TLC's string for a lambda in messages (`FcnLambdaValue.toString` with
    /// `TLCGlobals.expand`): its computed value if that succeeds.
    fn lazy_string(self: &Arc<Self>, f: &Arc<LazyFcn>) -> String {
        if let Ok(v) = self.materialize(f) {
            self.ppr(&v)
        } else {
            {
                // `FcnLambdaValue.toString` when `toFcnRcd` fails:
                // `[x \in S |-> <expression loc>]` (`FcnParams.toString`).
                let def = self.ir.def(f.env.frame.def);
                let name = |slot: usize| def.frame.get(slot).map_or("?", |s| s.name.as_str());
                let params: Vec<String> = f
                    .bounds
                    .iter()
                    .map(|b| {
                        let ids = if b.tuple {
                            format!(
                                "<<{}>>",
                                b.slots
                                    .iter()
                                    .map(|&s| name(s))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        } else {
                            b.slots.first().map_or("?", |&s| name(s)).to_owned()
                        };
                        format!(
                            "{ids} \\in {}",
                            dive_value::print::to_tlc_string_lenient(&b.domain, self.ctx)
                        )
                    })
                    .collect();
                format!(
                    "[{} |-> <expression {}>]",
                    params.join(", "),
                    self.loc(f.body)
                )
            }
        }
    }

    /// `FunctionValue.apply` for a computed function, with TLC's errors.
    pub(crate) fn apply_fcn(&self, f: &dive_value::Fcn, arg: &Value) -> Result<Value> {
        let fv = Value::Fcn(f.clone());
        if let Some(t) = f.as_tuple() {
            if f.is_tuple_literal() {
                // TupleValue.apply
                let Value::Int(i) = arg else {
                    return Err(EvalError::general(format!(
                        "Attempted to access tuple at a non integral index: {}",
                        self.ppr(arg)
                    )));
                };
                return usize::try_from(*i)
                    .ok()
                    .filter(|&i| i >= 1 && i <= t.len())
                    .map(|i| t[i - 1].clone())
                    .ok_or_else(|| {
                        EvalError::general(format!(
                            "Attempted to access index {i} of tuple\n{}\nwhich is out of bounds.",
                            self.ppr(&fv)
                        ))
                    });
            }
            // FcnRcdValue with an interval domain.
            let Value::Int(i) = arg else {
                return Err(EvalError::general(format!(
                    "Attempted to apply function with integer domain to the non-integer argument {}",
                    self.ppr(arg)
                )));
            };
            return usize::try_from(*i)
                .ok()
                .filter(|&i| i >= 1 && i <= t.len())
                .map(|i| t[i - 1].clone())
                .ok_or_else(|| self.not_in_domain(&fv, arg));
        }
        if f.is_record_literal() {
            // RecordValue.apply
            let Value::Str(name) = arg else {
                return Err(EvalError::general(format!(
                    "Attempted to access record by a non-string argument: {}",
                    self.ppr(arg)
                )));
            };
            return f.apply(arg).cloned().ok_or_else(|| {
                EvalError::general(format!(
                    "Attempted to access nonexistent field '{}' of record\n{}",
                    name.as_str(),
                    self.ppr(&fv)
                ))
            });
        }
        // FcnRcdValue.select: a linear search with TLC's equality.
        match self.fcn_select(f, arg)? {
            Some(v) => Ok(v),
            None => Err(self.not_in_domain(&fv, arg)),
        }
    }

    /// `FcnRcdValue.select` (array domain): `None` when `arg` is not in the
    /// domain; TLC's equality errors for incomparable arguments.
    fn fcn_select(&self, f: &dive_value::Fcn, arg: &Value) -> Result<Option<Value>> {
        // TLC compares `arg` with the domain elements with its checked
        // equality (`selectLinearSearch`), which may fail. Rust `==` is not
        // used: it may change TLC's view of nested unnormalized sets.
        if matches!(arg, Value::Bool(_) | Value::Int(_) | Value::Str(_))
            && let Some(v) = f.apply(arg)
        {
            return Ok(Some(v.clone()));
        }
        let mut pairs: Vec<(Value, &Value)> = f.iter().collect();
        if f.order() != crate::core::TLC_ORDER {
            pairs.sort_by(|a, b| a.0.cmp_with(&b.0, crate::core::TLC_ORDER));
        }
        for (d, v) in &pairs {
            match tlc::equals(d, arg, self.ctx) {
                Ok(true) => return Ok(Some((*v).clone())),
                Ok(false) => {}
                Err(err) => {
                    if pairs.len() >= 32 {
                        return Err(EvalError::unsupported(
                            "applying a function of 32 or more arguments to a value TLC cannot compare with its domain (TLC's error depends on whether it has normalized the function)",
                        ));
                    }
                    return Err(err.into());
                }
            }
        }
        Ok(None)
    }

    fn not_in_domain(&self, fv: &Value, arg: &Value) -> EvalError {
        EvalError::general(format!(
            "Attempted to apply function:\n{}\nto argument {}, which is not in the domain of the function.",
            self.ppr(fv),
            self.ppr(arg)
        ))
    }

    /// `FunctionValue.select`: `None` when `arg` is not in the domain.
    fn select(self: &Arc<Self>, f: &Ev, arg: &Value) -> Result<Option<Ev>> {
        match f {
            Ev::F(lf) => self.select_lazy(lf, arg),
            Ev::V(Value::Fcn(fv)) => {
                if let Some(t) = fv.as_tuple() {
                    let Value::Int(i) = arg else {
                        return Err(EvalError::general(if fv.is_tuple_literal() {
                            format!(
                                "Attempted to access tuple at a non integral index: {}",
                                self.ppr(arg)
                            )
                        } else {
                            format!(
                                "Attempted to apply function with integer domain to the non-integer argument {}",
                                self.ppr(arg)
                            )
                        }));
                    };
                    return Ok(usize::try_from(*i)
                        .ok()
                        .filter(|&i| i >= 1 && i <= t.len())
                        .map(|i| Ev::V(t[i - 1].clone())));
                }
                if fv.is_record_literal() {
                    if !matches!(arg, Value::Str(_)) {
                        return Err(EvalError::general(format!(
                            "Attempted to access record by a non-string argument: {}",
                            self.ppr(arg)
                        )));
                    }
                    return Ok(fv.apply(arg).cloned().map(Ev::V));
                }
                Ok(self.fcn_select(fv, arg)?.map(Ev::V))
            }
            Ev::V(other) => Err(EvalError::general(format!(
                "Attempted to apply EXCEPT construct to the value {}.",
                self.ppr(other)
            ))),
        }
    }

    /// `FcnLambdaValue.select`.
    pub(crate) fn select_lazy(
        self: &Arc<Self>,
        f: &Arc<LazyFcn>,
        arg: &Value,
    ) -> Result<Option<Ev>> {
        if let Some(v) = f.materialized.get() {
            return self.select(&Ev::V(v.clone()), arg);
        }
        if f.arity() != 1 || f.bounds[0].tuple {
            return Err(EvalError::unsupported(
                "EXCEPT on a lazy function of several arguments",
            ));
        }
        let b = &f.bounds[0];
        let mut rest: Vec<Except> = Vec::new();
        for ex in f.excepts.iter().rev() {
            if tlc::equals(ex.current(), arg, self.ctx)? {
                if ex.is_last() {
                    let mut v = ex.value.clone();
                    rest.reverse();
                    for r in rest {
                        v = self.take_except_ev(v, &r.path[r.idx..], r.value.clone())?;
                    }
                    return Ok(Some(v));
                }
                rest.push(ex.advanced());
            }
        }
        if !self.member(arg, &b.domain)? {
            return Ok(None);
        }
        let mut env = f.body_env();
        env.bind(b.slots[0], Sv::Val(arg.clone()));
        let _g = self.enter(f.node)?;
        let mut v = self.eval(&mut env, &f.st, f.body)?;
        rest.reverse();
        for r in rest {
            v = self.take_except_ev(v, &r.path[r.idx..], r.value.clone())?;
        }
        Ok(Some(v))
    }

    fn select_field(self: &Arc<Self>, r: Ev, name: &dive_value::Atom, e: ExprId) -> Result<Ev> {
        let sval = Value::Str(name.clone());
        match r {
            Ev::V(Value::Fcn(f)) if f.is_record_literal() => {
                f.apply(&sval).cloned().map(Ev::V).ok_or_else(|| {
                    EvalError::general(format!(
                        "Attempted to select nonexistent field {} from the record\n{}\n{}",
                        self.ppr(&sval),
                        self.ppr(&Value::Fcn(f.clone())),
                        self.loc(e)
                    ))
                })
            }
            Ev::V(Value::Fcn(f)) => {
                // `rval.toFcnRcd()`: a tuple becomes a function with an interval
                // domain.
                let f = match f.as_tuple() {
                    Some(t) if f.is_tuple_literal() => match Value::tuple_fcn(t.to_vec()) {
                        Value::Fcn(g) => g,
                        _ => f,
                    },
                    _ => f,
                };
                self.apply_fcn(&f, &sval).map(Ev::V)
            }
            Ev::F(f) => self.apply_lazy(&f, &sval, control::CLEAR),
            Ev::V(other) => Err(EvalError::general(format!(
                "Attempted to select field {} from a non-record value {}\n{}",
                self.ppr(&Value::Str(name.clone())),
                self.ppr(&other),
                self.loc(e)
            ))),
        }
    }

    fn eval_except(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        base: ExprId,
        updates: &[dive_ir::ExceptUpdate],
        _e: ExprId,
    ) -> Result<Ev> {
        // OPCODE_exc
        let mut result = self.eval(env, st, base)?;
        for u in updates {
            let mut path = Vec::with_capacity(u.path.len());
            for &p in &u.path {
                path.push(self.eval_v(env, st, p)?);
            }
            // `result.select(lhs)`
            let mut at = Some(result.clone());
            for p in &path {
                let Some(cur) = at else { break };
                at = match &cur {
                    Ev::V(Value::Fcn(_)) | Ev::F(_) => self.select(&cur, p)?,
                    Ev::V(other) => {
                        return Err(EvalError::general(format!(
                            "Attempted to apply EXCEPT construct to the value {}.",
                            self.ppr(other)
                        )));
                    }
                };
            }
            match at {
                None => {
                    self.warn(
                        2144,
                        &format!(
                            "The EXCEPT was applied to non-existing fields of the value at\n{}",
                            self.loc(base)
                        ),
                    );
                }
                Some(at) => {
                    let at = self.embed(at)?;
                    let saved = env.dynamic.at.replace(at);
                    let rhs = self.eval(env, st, u.value);
                    env.dynamic.at = saved;
                    let rhs = rhs?;
                    result = self.take_except_ev(result, &path, rhs)?;
                }
            }
        }
        Ok(result)
    }

    /// Prints a TLC warning message.
    /// As `MP.printWarning`, only while warnings are on, and each distinct
    /// message once per run.
    pub(crate) fn warn(&self, code: u32, text: &str) {
        if !self.opts.warnings {
            return;
        }
        let msg = format!("@!@!@STARTMSG {code}:3 @!@!@\n{text}\n@!@!@ENDMSG {code} @!@!@");
        let fresh = self
            .warned
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(msg.clone());
        if fresh {
            self.print(msg);
        }
    }

    /// `takeExcept` along a path whose every step exists.
    fn take_except_ev(self: &Arc<Self>, base: Ev, path: &[Value], new: Ev) -> Result<Ev> {
        if path.is_empty() {
            return Ok(new);
        }
        match base {
            Ev::F(f) => {
                if let Some(v) = f.materialized.get() {
                    return self.take_except_ev(Ev::V(v.clone()), path, new);
                }
                Ok(Ev::F(f.with_except(Except {
                    path: path.into(),
                    idx: 0,
                    value: new,
                })))
            }
            Ev::V(v) => {
                let new = self.embed(new)?;
                self.take_except(&v, path, new).map(Ev::V)
            }
        }
    }

    /// `Value.takeExcept` for computed values.
    pub(crate) fn take_except(&self, v: &Value, path: &[Value], new: Value) -> Result<Value> {
        let Some((first, rest)) = path.split_first() else {
            return Ok(new);
        };
        let Value::Fcn(f) = v else {
            return Err(EvalError::general(format!(
                "Attempted to apply EXCEPT construct to the value {}.",
                self.ppr(v)
            )));
        };
        if let Some(t) = f.as_tuple() {
            let Some(i) = first
                .as_int()
                .and_then(|i| i.to_i64())
                .and_then(|i| usize::try_from(i).ok())
            else {
                return Ok(v.clone());
            };
            if i == 0 || i > t.len() {
                return Ok(v.clone());
            }
            let mut elems = t.to_vec();
            elems[i - 1] = self.take_except(&t[i - 1], rest, new)?;
            return Ok(if f.is_tuple_literal() {
                Value::tuple(elems)
            } else {
                Value::tuple_fcn(elems)
            });
        }
        if let Some(fields) = f.as_record() {
            let Value::Str(name) = first else {
                return Ok(v.clone());
            };
            let mut out = Vec::with_capacity(fields.len());
            let mut found = false;
            for (a, x) in fields {
                if a == name {
                    found = true;
                    out.push((a.clone(), self.take_except(x, rest, new.clone())?));
                } else {
                    out.push((a.clone(), x.clone()));
                }
            }
            if !found {
                return Ok(v.clone());
            }
            if f.is_record_literal() {
                return Ok(Value::record(out, self.ctx)?);
            }
            return Ok(Value::fcn(
                out.into_iter().map(|(a, x)| (Value::Str(a), x)).collect(),
                self.ctx,
            )?);
        }
        let mut out = Vec::with_capacity(f.len());
        let mut found = false;
        for (d, x) in f.iter() {
            if !found && tlc::equals(&d, first, self.ctx)? {
                found = true;
                out.push((d, self.take_except(x, rest, new.clone())?));
            } else {
                out.push((d, x.clone()));
            }
        }
        if !found {
            return Ok(v.clone());
        }
        Ok(Value::fcn(out, self.ctx)?)
    }

    // -----------------------------------------------------------------------------
    // Built-in operators
    // -----------------------------------------------------------------------------

    /// TLC's `Value.equals`, or TLA+'s answer under `--comparisons=tla`.
    pub(crate) fn equals(&self, a: &Value, b: &Value) -> Result<bool> {
        match self.opts.comparisons {
            Comparisons::Tlc => Ok(tlc::equals(a, b, self.ctx)?),
            Comparisons::Tla => crate::tla_cmp::equals(self, a, b),
        }
    }

    /// TLC's `set.member(elem)`, or TLA+'s answer under `--comparisons=tla`.
    pub(crate) fn member(&self, elem: &Value, set: &Value) -> Result<bool> {
        match self.opts.comparisons {
            Comparisons::Tlc => Ok(tlc::member(elem, set, self.ctx)?),
            Comparisons::Tla => crate::tla_cmp::member(self, elem, set),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per built-in, as in Tool.evalAppl"
    )]
    pub(crate) fn eval_builtin(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: BuiltinOp,
        args: &[ExprId],
        e: ExprId,
    ) -> Result<Ev> {
        let arg = |k: usize| {
            args.get(k)
                .copied()
                .ok_or_else(|| EvalError::bug("missing operand"))
        };
        Ok(Ev::V(match op {
            BuiltinOp::Eq | BuiltinOp::Neq => {
                // OPCODE_eq / OPCODE_noteq
                let a = self.eval_v(env, st, arg(0)?)?;
                let b = self.eval_v(env, st, arg(1)?)?;
                let eq = self.equals(&a, &b)?;
                Value::Bool(if op == BuiltinOp::Eq { eq } else { !eq })
            }
            BuiltinOp::In | BuiltinOp::NotIn => {
                // OPCODE_in / OPCODE_notin
                let a = self.eval_v(env, st, arg(0)?)?;
                let s = self.eval_v(env, st, arg(1)?)?;
                let m = self.member(&a, &s)?;
                Value::Bool(if op == BuiltinOp::In { m } else { !m })
            }
            BuiltinOp::Subseteq => {
                // OPCODE_subseteq
                let a = self.eval_v(env, st, arg(0)?)?;
                let b = self.eval_v(env, st, arg(1)?)?;
                if !is_enumerable(&a) {
                    return Err(EvalError::general(format!(
                        "Attempted to evaluate an expression of form S \\subseteq T, but S was not enumerable.\n{}",
                        self.loc(e)
                    )));
                }
                Value::Bool(self.subseteq(&a, &b)?)
            }
            BuiltinOp::Cup | BuiltinOp::Cap | BuiltinOp::SetMinus => {
                let a = self.eval_v(env, st, arg(0)?)?;
                let b = self.eval_v(env, st, arg(1)?)?;
                self.set_op(op, a, b)?
            }
            BuiltinOp::PowerSet => {
                // OPCODE_subset
                let a = self.eval_v(env, st, arg(0)?)?;
                Value::subset(a, self.ctx)
            }
            BuiltinOp::BigUnion => {
                // OPCODE_union
                let a = self.eval_v(env, st, arg(0)?)?;
                self.big_union(a)?
            }
            BuiltinOp::Domain => {
                // OPCODE_domain
                match self.eval(env, st, arg(0)?)? {
                    Ev::F(f) => self.lazy_domain(&f),
                    Ev::V(Value::Fcn(f)) => self.domain(&f)?,
                    Ev::V(other) => {
                        return Err(EvalError::general(format!(
                            "Attempted to apply the operator DOMAIN to a non-function\n({})\n{}",
                            kind_string(&other),
                            self.loc(e)
                        )));
                    }
                }
            }
            BuiltinOp::Times => {
                let mut sets = Vec::with_capacity(args.len());
                for &a in args {
                    sets.push(self.eval_embed(env, st, a)?);
                }
                Value::product(sets, self.ctx)
            }
            BuiltinOp::Boolean => Value::boolean_set(self.ctx),
            BuiltinOp::String => Value::string_set(self.ctx),
            BuiltinOp::Not => return self.eval_inner_not(env, st, arg(0)?, e),
            BuiltinOp::And | BuiltinOp::Or => {
                let kind = if op == BuiltinOp::And {
                    JunctionKind::And
                } else {
                    JunctionKind::Or
                };
                return self.eval_junction(env, st, kind, false, args, e);
            }
            BuiltinOp::Implies | BuiltinOp::Equiv => {
                return Err(EvalError::unsupported(
                    "=> or <=> passed as an operator argument",
                ));
            }
            BuiltinOp::Prime => {
                let st1 = St {
                    s0: st.s1.clone(),
                    s1: StateRef::Null,
                    control: St::primed_if_enabled(st.control),
                };
                return self.eval(env, &st1, arg(0)?);
            }
            BuiltinOp::Unchanged => {
                let a = arg(0)?;
                let v0 = self.eval_v(
                    env,
                    &St {
                        s0: st.s0.clone(),
                        s1: StateRef::Empty,
                        control: st.control,
                    },
                    a,
                )?;
                let v1 = self.eval_v(
                    env,
                    &St {
                        s0: st.s1.clone(),
                        s1: StateRef::Null,
                        control: St::primed_if_enabled(st.control),
                    },
                    a,
                )?;
                Value::Bool(self.equals(&v0, &v1)?)
            }
            BuiltinOp::Enabled => return self.eval_enabled(env, st, arg(0)?, e),
            BuiltinOp::Cdot => {
                return Err(EvalError::general(format!(
                    "The current version of TLC does not support action composition.  An incomplete implementation can be enabled via the tlc2.tool.impl.Tool.cdot=true java property.\n{}",
                    self.loc(e)
                )));
            }
            BuiltinOp::Always => return Err(self.temporal("[]A", e)),
            BuiltinOp::Eventually => return Err(self.temporal("<>A", e)),
            BuiltinOp::LeadsTo => return Err(self.temporal("a ~> b", e)),
            BuiltinOp::WhilePlus => return Err(self.temporal("a -+-> formula", e)),
        }))
    }

    fn eval_inner_not(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        a: ExprId,
        e: ExprId,
    ) -> Result<Ev> {
        let v = self.eval(env, st, a)?;
        match v {
            Ev::V(Value::Bool(b)) => Ok(Ev::V(Value::Bool(!b))),
            other => Err(EvalError::general(format!(
                "Attempted to apply the operator ~ to a non-boolean\n({})\n{}",
                self.kind_of(&other),
                self.loc(e)
            ))),
        }
    }

    /// `Enumerable.isSubsetEq` (`EnumerableValue.isSubsetEq`): every element
    /// of `a` is a member of `b`.
    fn subseteq(self: &Arc<Self>, a: &Value, b: &Value) -> Result<bool> {
        let Value::Set(s) = a else {
            return Err(EvalError::bug("subseteq of a non-set"));
        };
        let elems = self.explore(s)?;
        for x in elems.iter() {
            if !self.member(x, b)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `\cup`, `\cap` and `\` as TLC chooses between its eager
    /// (`Reducible`) and lazy forms.
    fn set_op(self: &Arc<Self>, op: BuiltinOp, a: Value, b: Value) -> Result<Value> {
        let lazy = |kind, a, b| {
            Value::deferred(
                Arc::new(SetOp {
                    core: self.clone(),
                    kind,
                    a,
                    b,
                }),
                self.ctx,
            )
        };
        Ok(match op {
            BuiltinOp::Cup => {
                if is_reducible(&a) {
                    self.cup(a, b, &lazy)?
                } else if is_reducible(&b) {
                    self.cup(b, a, &lazy)?
                } else {
                    lazy(DeferredKind::Cup, a, b)
                }
            }
            BuiltinOp::Cap => {
                if is_reducible(&a) {
                    ops::intersect(&a, &b, self.ctx)?
                } else if is_reducible(&b) {
                    ops::intersect(&b, &a, self.ctx)?
                } else {
                    lazy(DeferredKind::Cap, a, b)
                }
            }
            _ => {
                if is_reducible(&a) {
                    ops::difference(&a, &b, self.ctx)?
                } else {
                    lazy(DeferredKind::Diff, a, b)
                }
            }
        })
    }

    /// `SetEnumValue.cup` / `IntervalValue.cup` with a `Reducible` receiver.
    fn cup(
        &self,
        a: Value,
        b: Value,
        lazy: &dyn Fn(DeferredKind, Value, Value) -> Value,
    ) -> Result<Value> {
        let empty = match &a {
            Value::Set(s) => match s.raw_elements() {
                Some(raw) => raw.is_empty(),
                None => s.size(self.ctx)? == 0,
            },
            _ => false,
        };
        if empty {
            return Ok(b);
        }
        if is_reducible(&b) {
            return Ok(ops::union(&a, &b, self.ctx)?);
        }
        Ok(lazy(DeferredKind::Cup, a, b))
    }

    /// `UnionValue.union`.
    fn big_union(self: &Arc<Self>, a: Value) -> Result<Value> {
        let combinable = |v: &Value| matches!(v, Value::Set(s) if matches!(s.kind(), dive_value::SetKind::Enum | dive_value::SetKind::Raw));
        if let Value::Set(s) = &a
            && combinable(&a)
        {
            let elems: Vec<Value> = match s.raw_elements() {
                Some(r) => r.to_vec(),
                None => s.elements(self.ctx)?.to_vec(),
            };
            if elems.iter().all(combinable) {
                let mut out: Vec<Value> = Vec::new();
                for inner in &elems {
                    let Value::Set(is) = inner else { continue };
                    let ie: Vec<Value> = match is.raw_elements() {
                        Some(r) => r.to_vec(),
                        None => is.elements(self.ctx)?.to_vec(),
                    };
                    for x in ie {
                        // `result.member(elem)` on the unnormalized result: a linear
                        // scan with TLC's equality.
                        let mut found = false;
                        for y in &out {
                            if tlc::equals(y, &x, self.ctx)? {
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            out.push(x);
                        }
                    }
                }
                return Ok(Value::set_raw(out, self.ctx));
            }
        }
        Ok(Value::deferred(
            Arc::new(SetOp {
                core: self.clone(),
                kind: DeferredKind::Union,
                a,
                b: Value::FALSE,
            }),
            self.ctx,
        ))
    }

    /// `UnionValue.member`.
    pub(crate) fn union_member(self: &Arc<Self>, set: &Value, elem: &Value) -> Result<bool> {
        let Value::Set(s) = set else {
            return Err(EvalError::general(format!(
                "Attempted to check if:\n {}\nis an element of the non-enumerable set:\n {}",
                self.ppr(elem),
                self.ppr(set)
            )));
        };
        if !is_enumerable(set) {
            return Err(EvalError::general(format!(
                "Attempted to check if:\n {}\nis an element of the non-enumerable set:\n {}",
                self.ppr(elem),
                self.ppr(set)
            )));
        }
        for inner in self.explore(s)?.iter() {
            if self.member(elem, inner)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The elements of a lazy `\cup`, `\cap`, `\` or `UNION`.
    pub(crate) fn setop_elements(self: &Arc<Self>, op: &SetOp) -> Result<Vec<Value>> {
        let enumerate = |v: &Value| -> Result<Arc<[Value]>> {
            match v {
                Value::Set(s) => self.explore_unsized(s),
                _ => Err(EvalError::bug("enumerating a non-set")),
            }
        };
        match op.kind {
            DeferredKind::Cup => {
                if !is_enumerable(&op.a) || !is_enumerable(&op.b) {
                    return Err(EvalError::general(format!(
                        "Attempted to enumerate S \\cup T when S:\n{}\nand T:\n{}\nare not both enumerable",
                        self.ppr(&op.a),
                        self.ppr(&op.b)
                    )));
                }
                let mut out = enumerate(&op.a)?.to_vec();
                out.extend(enumerate(&op.b)?.iter().cloned());
                Ok(out)
            }
            DeferredKind::Cap => {
                let (src, other) = if is_enumerable(&op.a) {
                    (&op.a, &op.b)
                } else if is_enumerable(&op.b) {
                    (&op.b, &op.a)
                } else {
                    return Err(EvalError::general(format!(
                        "Attempted to enumerate S \\cap T when neither S:\n{}\nnor T:\n{}\nis enumerable",
                        self.ppr(&op.a),
                        self.ppr(&op.b)
                    )));
                };
                let mut out = Vec::new();
                for x in enumerate(src)?.iter() {
                    if self.member(x, other)? {
                        out.push(x.clone());
                    }
                }
                Ok(out)
            }
            DeferredKind::Diff => {
                if !is_enumerable(&op.a) {
                    return Err(EvalError::general(format!(
                        "Attempted to enumerate S \\ T when S:\n{}\nis not enumerable.",
                        self.ppr(&op.a)
                    )));
                }
                let mut out = Vec::new();
                for x in enumerate(&op.a)?.iter() {
                    if !self.member(x, &op.b)? {
                        out.push(x.clone());
                    }
                }
                Ok(out)
            }
            DeferredKind::Union => {
                if !is_enumerable(&op.a) {
                    return Err(EvalError::general(format!(
                        "Attempted to enumerate the nonenumerable set:\n{}",
                        self.ppr(&Value::deferred(
                            Arc::new(SetOp {
                                core: self.clone(),
                                kind: DeferredKind::Union,
                                a: op.a.clone(),
                                b: Value::FALSE
                            }),
                            self.ctx
                        ))
                    )));
                }
                let mut out = Vec::new();
                for inner in enumerate(&op.a)?.iter() {
                    if !is_enumerable(inner) {
                        return Err(EvalError::general(
                            "Attempted to enumerate UNION(s), but some element of s is nonenumerable.",
                        ));
                    }
                    out.extend(enumerate(inner)?.iter().cloned());
                }
                Ok(out)
            }
            DeferredKind::Filter | DeferredKind::Map => {
                Err(EvalError::bug("set operator of the wrong kind"))
            }
        }
    }

    /// `FunctionValue.getDomain` for a computed function.
    fn domain(&self, f: &dive_value::Fcn) -> Result<Value> {
        if let Some(t) = f.as_tuple() {
            let n = i64::try_from(t.len()).unwrap_or(i64::MAX);
            return Ok(Value::interval(Int::ONE, Int::Small(n), self.ctx));
        }
        Ok(Value::set(f.iter().map(|(d, _)| d).collect(), self.ctx)?)
    }

    /// `FcnLambdaValue.getDomain`.
    fn lazy_domain(&self, f: &Arc<LazyFcn>) -> Value {
        if let Some(Value::Fcn(m)) = f.materialized.get()
            && let Ok(d) = self.domain(m)
        {
            return d;
        }
        if f.arity() == 1 {
            return f.bounds[0].domain.clone();
        }
        let mut sets = Vec::new();
        for b in &f.bounds {
            let n = if b.tuple { 1 } else { b.slots.len() };
            for _ in 0..n {
                sets.push(b.domain.clone());
            }
        }
        Value::product(sets, self.ctx)
    }
}

/// All combinations of the columns, the first column varying fastest.
pub(crate) fn odometer(columns: &[Arc<[Value]>]) -> Vec<Vec<Value>> {
    let mut out = Vec::new();
    if columns.iter().any(|c| c.is_empty()) {
        return out;
    }
    let mut idx = vec![0usize; columns.len()];
    loop {
        out.push(
            idx.iter()
                .zip(columns)
                .map(|(&i, c)| c[i].clone())
                .collect(),
        );
        let mut k = 0;
        loop {
            if k == columns.len() {
                return out;
            }
            idx[k] += 1;
            if idx[k] < columns[k].len() {
                break;
            }
            idx[k] = 0;
            k += 1;
        }
    }
}
