//! Generator-side compilation helpers (`05-successor-generation.md` §4.5,
//! §4.6, §4.9, §6), kept apart from the value compiler: TLC's `elements()`
//! multiplicity for `x' \in S`, the decomposition of `UNCHANGED`, and the
//! resolution of a function applied as an action.

use dive_ir::{BuiltinOp, DefScope, Expr, ExprId, OpRef, StdOp};
use dive_value::{DeferredKind, SetKind, Value};

use super::{Compiler, ConstBinding, Mode, Sb};
use crate::Error;
use crate::plan::{Node, P, Raw, Unch};

/// How TLC represents the value of a set expression, as far as the
/// multiplicity of its `elements()` is concerned.
enum Class {
    /// `SetEnumValue` (`set` true) or `IntervalValue` (TLC's `Reducible`):
    /// `\cup` with another reducible set is computed eagerly, and `UNION`
    /// of set enumerations too.
    Reducible { set: bool },
    /// A set whose `elements()` lists each element once, of another class.
    Plain,
    /// A set whose `elements()` may repeat elements: its structure.
    Raw(Raw),
}

/// The deepest definition chain followed while classifying a set.
const MAX_DEPTH: u32 = 64;

fn value_class(v: &Value) -> Result<bool, Error> {
    // Whether a startup value is safe to enumerate as normalized.
    let Value::Set(s) = v else {
        return Ok(true);
    };
    match s.kind() {
        SetKind::Deferred => Ok(matches!(
            s.as_deferred().map(|d| d.kind()),
            Some(DeferredKind::Map)
        )),
        SetKind::FcnSet | SetKind::RcdSet | SetKind::Product => {
            for (_, c) in s.components() {
                if !value_class(&c)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(true),
    }
}

impl Compiler<'_> {
    /// The structure TLC's `elements()` of `e` has when it may repeat
    /// elements, or `None` when it cannot (the common case, compiled as a
    /// plain `x' \in S`).
    ///
    /// # Errors
    /// Unsupported when the multiplicity depends on a representation the
    /// core cannot see.
    pub(super) fn raw_plan(&mut self, e: ExprId, env: usize) -> Result<Option<Raw>, Error> {
        if !self.m.heap.store().semantics().tlc_comparisons {
            return Ok(None);
        }
        match self.class(e, env, 0)? {
            Class::Raw(r) => Ok(Some(r)),
            Class::Reducible { .. } | Class::Plain => Ok(None),
        }
    }

    fn leaf(&mut self, e: ExprId, env: usize) -> Result<Raw, Error> {
        Ok(Raw::Leaf(self.p(e, env, Mode::Plain)?))
    }

    fn as_raw(&mut self, c: Class, e: ExprId, env: usize) -> Result<Raw, Error> {
        match c {
            Class::Raw(r) => Ok(r),
            Class::Reducible { .. } | Class::Plain => self.leaf(e, env),
        }
    }

    #[allow(clippy::too_many_lines, reason = "one arm per set constructor")]
    fn class(&mut self, e: ExprId, env: usize, depth: u32) -> Result<Class, Error> {
        if depth > MAX_DEPTH {
            return Err(Error::unsupported(
                "x' \\in a set behind a definition chain deeper than the core follows",
            ));
        }
        let expr = self.ir.node(e).expr.clone();
        match expr {
            Expr::SetEnum(_) | Expr::SetMap { .. } => Ok(Class::Reducible { set: true }),
            Expr::Label { body: a, .. } | Expr::Selected(a) => self.class(a, env, depth + 1),
            Expr::Const(c) => match self.h.constants.get(c.index()).cloned().flatten() {
                Some(ConstBinding::Value(v)) => Ok(startup(&v)?),
                Some(ConstBinding::Def(d)) => self.def_class(d, e, env, depth),
                None => Ok(Class::Plain),
            },
            Expr::Def(d) => self.def_class(d, e, env, depth),
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => {
                let d = self.redirect(d);
                let def = self.ir.def(d);
                if matches!(def.std, Some(StdOp::NaturalsDotDot | StdOp::IntegersDotDot)) {
                    return Ok(Class::Reducible { set: false });
                }
                if args.is_empty() {
                    return self.def_class(d, e, env, depth);
                }
                if def.std.is_none() && risky(self.ir, def.body, 0) {
                    return Err(Error::unsupported(
                        "x' \\in an operator application whose body builds a lazily represented set (core)",
                    ));
                }
                Ok(Class::Plain)
            }
            Expr::Apply {
                callee: OpRef::Std(op),
                ..
            } => Ok(
                if matches!(op, StdOp::NaturalsDotDot | StdOp::IntegersDotDot) {
                    Class::Reducible { set: false }
                } else {
                    Class::Plain
                },
            ),
            Expr::Local(r) => {
                let fe = self.frame_env(env, r.up)?;
                match self.envs[fe].slots.get(r.slot.index()).cloned() {
                    Some(Sb::Arg {
                        expr, env: aenv, ..
                    }) => self.class(expr, aenv, depth + 1),
                    Some(Sb::LetVal { def, env: lenv, .. }) => {
                        let body = self.ir.def(def).body;
                        self.class(body, lenv, depth + 1)
                    }
                    _ => Ok(Class::Plain),
                }
            }
            Expr::Builtin { op, args } => match (op, args.as_ref()) {
                (BuiltinOp::Cup, &[a, b]) => {
                    let ca = self.class(a, env, depth + 1)?;
                    let cb = self.class(b, env, depth + 1)?;
                    if let (Class::Reducible { set: sa }, Class::Reducible { set: sb }) = (&ca, &cb)
                    {
                        // Computed eagerly (`SetEnumValue.cup`), or the
                        // other operand itself when one is empty.
                        return Ok(Class::Reducible { set: *sa && *sb });
                    }
                    let ra = self.as_raw(ca, a, env)?;
                    let rb = self.as_raw(cb, b, env)?;
                    Ok(Class::Raw(Raw::Cup(Box::new(ra), Box::new(rb))))
                }
                (BuiltinOp::Cap, &[a, b]) => {
                    let ca = self.class(a, env, depth + 1)?;
                    let cb = self.class(b, env, depth + 1)?;
                    if !matches!(ca, Class::Raw(_)) && !matches!(cb, Class::Raw(_)) {
                        return Ok(Class::Plain);
                    }
                    let ra = self.as_raw(ca, a, env)?;
                    let rb = self.as_raw(cb, b, env)?;
                    let pa = self.p(a, env, Mode::Plain)?;
                    let pb = self.p(b, env, Mode::Plain)?;
                    Ok(Class::Raw(Raw::Cap(Box::new(ra), Box::new(rb), pa, pb)))
                }
                (BuiltinOp::SetMinus, &[a, b]) => match self.class(a, env, depth + 1)? {
                    Class::Raw(ra) => {
                        let pb = self.p(b, env, Mode::Plain)?;
                        Ok(Class::Raw(Raw::Minus(Box::new(ra), pb)))
                    }
                    _ => Ok(Class::Plain),
                },
                (BuiltinOp::BigUnion, &[a]) => self.union_class(a, env, depth),
                (BuiltinOp::Times, items) => {
                    let items = items.to_vec();
                    self.odometer(e, &items, env, depth)
                }
                _ => Ok(Class::Plain),
            },
            Expr::Cartesian(items) => {
                let items = items.to_vec();
                self.odometer(e, &items, env, depth)
            }
            Expr::RecordSet(fields) => {
                let items: Vec<ExprId> = fields.iter().map(|f| f.value).collect();
                self.odometer(e, &items, env, depth)
            }
            Expr::FcnSet { range, .. } => self.odometer(e, &[range], env, depth),
            Expr::SetFilter { bound, .. } => {
                let Some(dom) = bound.domain else {
                    return Ok(Class::Plain);
                };
                match self.class(dom, env, depth + 1)? {
                    Class::Raw(inner) => {
                        let Expr::SetFilter { bound, pred } = self.ir.node(e).expr.clone() else {
                            return Err(Error::internal("a filter changed shape"));
                        };
                        let (mut binds, saved) =
                            self.binds(std::slice::from_ref(&bound), env, Mode::Plain)?;
                        let p = self.p(pred, env, Mode::Plain);
                        self.unbind(saved);
                        let pred = p?;
                        let bind = binds
                            .pop()
                            .ok_or_else(|| Error::internal("filter without a bound"))?;
                        Ok(Class::Raw(Raw::Filter {
                            bind,
                            pred,
                            inner: Box::new(inner),
                        }))
                    }
                    _ => Ok(Class::Plain),
                }
            }
            Expr::Let { body, .. } => {
                if risky(self.ir, body, 0) {
                    return Err(Error::unsupported(
                        "x' \\in a LET whose body builds a lazily represented set (core)",
                    ));
                }
                Ok(Class::Plain)
            }
            _ => Ok(Class::Plain),
        }
    }

    fn def_class(
        &mut self,
        d: dive_ir::DefId,
        e: ExprId,
        env: usize,
        depth: u32,
    ) -> Result<Class, Error> {
        let d = self.redirect(d);
        if let Some(v) = self.h.precomputed.get(&d) {
            return startup(v);
        }
        let def = self.ir.def(d);
        if matches!(def.std, Some(StdOp::NaturalsDotDot | StdOp::IntegersDotDot)) {
            return Ok(Class::Reducible { set: false });
        }
        if def.std.is_some() || !def.params.is_empty() {
            return Ok(Class::Plain);
        }
        if def.recursive || matches!(def.scope, DefScope::LetValue { .. }) {
            if risky(self.ir, def.body, 0) {
                return Err(Error::unsupported(
                    "x' \\in a definition whose body builds a lazily represented set (core)",
                ));
            }
            return Ok(Class::Plain);
        }
        let _ = e;
        let parent = self.lexical_parent(d, env)?;
        let inner = self.new_env(d, parent);
        self.class(def.body, inner, depth + 1)
    }

    fn union_class(&mut self, a: ExprId, env: usize, depth: u32) -> Result<Class, Error> {
        if let Expr::SetEnum(items) = self.ir.node(a).expr.clone() {
            let mut all_enum = true;
            let mut raws = Vec::with_capacity(items.len());
            for &i in &items {
                let c = self.class(i, env, depth + 1)?;
                all_enum &= matches!(c, Class::Reducible { set: true });
                raws.push(self.as_raw(c, i, env)?);
            }
            if all_enum {
                // `UnionValue.union` combines set enumerations eagerly.
                return Ok(Class::Reducible { set: true });
            }
            return Ok(Class::Raw(Raw::Union(raws.into())));
        }
        if risky(self.ir, a, 0) {
            return Err(Error::unsupported(
                "x' \\in UNION of a set whose members TLC may represent lazily (core)",
            ));
        }
        let p = self.p(a, env, Mode::Plain)?;
        Ok(Class::Raw(Raw::UnionOf(p)))
    }

    fn odometer(
        &mut self,
        e: ExprId,
        items: &[ExprId],
        env: usize,
        depth: u32,
    ) -> Result<Class, Error> {
        let mut any = false;
        let mut raws = Vec::with_capacity(items.len());
        for &i in items {
            let c = self.class(i, env, depth + 1)?;
            any |= matches!(c, Class::Raw(_));
            raws.push(self.as_raw(c, i, env)?);
        }
        if !any {
            return Ok(Class::Plain);
        }
        let p = self.p(e, env, Mode::Plain)?;
        Ok(Class::Raw(Raw::Odometer(raws.into(), p)))
    }

    /// Decomposes `UNCHANGED e` with TLC's `processUnchanged`
    /// (`05-successor-generation.md` §4.6): variables, tuples and
    /// zero-arity definitions are followed; anything else is verified.
    pub(super) fn unchanged_items(
        &mut self,
        e: ExprId,
        env: usize,
        out: &mut Vec<Unch>,
    ) -> Result<(), Error> {
        #[allow(clippy::cast_possible_truncation, reason = "at most 64 variables")]
        match self.ir.node(e).expr.clone() {
            Expr::Var { var, primed: false } => {
                out.push(Unch::Var {
                    var: var.index() as u16,
                    at: e.0,
                });
                Ok(())
            }
            Expr::Tuple(items) => {
                for &i in &items {
                    self.unchanged_items(i, env, out)?;
                }
                Ok(())
            }
            Expr::Label { body: a, .. } | Expr::Selected(a) => self.unchanged_items(a, env, out),
            Expr::Def(d) => {
                let d = self.redirect(d);
                let def = self.ir.def(d);
                if self.h.precomputed.contains_key(&d)
                    || def.std.is_some()
                    || !def.params.is_empty()
                    || def.recursive
                    || matches!(def.scope, DefScope::LetValue { .. })
                {
                    return self.verify_item(e, env, out);
                }
                let parent = self.lexical_parent(d, env)?;
                let inner = self.new_env(d, parent);
                self.unchanged_items(def.body, inner, out)
            }
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } if args.is_empty() => {
                let d = self.redirect(d);
                let def = self.ir.def(d);
                if self.h.precomputed.contains_key(&d)
                    || def.std.is_some()
                    || def.recursive
                    || matches!(def.scope, DefScope::LetValue { .. })
                {
                    return self.verify_item(e, env, out);
                }
                let parent = self.lexical_parent(d, env)?;
                let inner = self.new_env(d, parent);
                self.unchanged_items(def.body, inner, out)
            }
            // An `INSTANCE` substitution (stream 2): a substituted name is
            // followed like an argument, an unsubstituted one is the root's
            // variable.
            Expr::SubstRef {
                decl,
                primed: false,
            } => match self.subst_expr(decl) {
                Some(r) => {
                    let (expr, aenv, depth) = r?;
                    self.outside(depth, |c| c.unchanged_items(expr, aenv, out))
                }
                None => match self.ir.decl(decl).var {
                    Some(v) => {
                        out.push(Unch::Var {
                            var: v.index() as u16,
                            at: e.0,
                        });
                        Ok(())
                    }
                    None => self.verify_item(e, env, out),
                },
            },
            Expr::Subst { bindings, body } => {
                let mut inner = Vec::new();
                let (r, _) = self.subst_scope(&bindings, env, Mode::Plain, |c| {
                    c.unchanged_items(body, env, &mut inner)
                })?;
                // A verified item would read the substitution's slots after
                // its scope ends.
                if inner.iter().any(|u| matches!(u, Unch::Verify(_))) {
                    return Err(Error::unsupported(
                        "UNCHANGED of a non-variable expression under an INSTANCE substitution (core)",
                    ));
                }
                out.extend(inner);
                Ok(r)
            }
            Expr::Local(r) => {
                let fe = self.frame_env(env, r.up)?;
                match self.envs[fe].slots.get(r.slot.index()).cloned() {
                    Some(Sb::Arg {
                        expr, env: aenv, ..
                    }) => self.unchanged_items(expr, aenv, out),
                    Some(Sb::LetVal { def, env: lenv, .. }) => {
                        let body = self.ir.def(def).body;
                        self.unchanged_items(body, lenv, out)
                    }
                    _ => self.verify_item(e, env, out),
                }
            }
            _ => self.verify_item(e, env, out),
        }
    }

    fn verify_item(&mut self, e: ExprId, env: usize, out: &mut Vec<Unch>) -> Result<(), Error> {
        let x = self.p(e, env, Mode::Plain)?;
        let y = self.p(e, env, Mode::Primed)?;
        let same = self.push(Node::Eq(x, y))?;
        out.push(Unch::Verify(same));
        Ok(())
    }

    /// `v = v'` for `<<A>>_v`'s `CHANGED v` item.
    pub(super) fn changed_test(&mut self, e: ExprId, env: usize) -> Result<P, Error> {
        let x = self.p(e, env, Mode::Plain)?;
        let y = self.p(e, env, Mode::Primed)?;
        self.push(Node::Eq(x, y))
    }

    /// The function constructor `f` names, with its environment, when TLC
    /// evaluates `f` (with `KeepLazy`) to an unmaterialized
    /// `FcnLambdaValue`.
    pub(super) fn fcn_lambda(
        &mut self,
        f: ExprId,
        env: usize,
        depth: u32,
    ) -> Result<Option<(ExprId, usize)>, Error> {
        if depth > MAX_DEPTH {
            return Ok(None);
        }
        match self.ir.node(f).expr.clone() {
            Expr::FcnCons { .. } => Ok(Some((f, env))),
            Expr::Label { body: a, .. } | Expr::Selected(a) => self.fcn_lambda(a, env, depth + 1),
            Expr::Def(d)
            | Expr::Apply {
                callee: OpRef::Def(d),
                ..
            } => {
                if let Expr::Apply { args, .. } = &self.ir.node(f).expr
                    && !args.is_empty()
                {
                    return Ok(None);
                }
                let d = self.redirect(d);
                let def = self.ir.def(d);
                if self.h.precomputed.contains_key(&d)
                    || def.std.is_some()
                    || !def.params.is_empty()
                    || def.recursive
                    || matches!(def.scope, DefScope::LetValue { .. })
                {
                    return Ok(None);
                }
                let parent = self.lexical_parent(d, env)?;
                let inner = self.new_env(d, parent);
                self.fcn_lambda(def.body, inner, depth + 1)
            }
            _ => Ok(None),
        }
    }
}

fn startup(v: &Value) -> Result<Class, Error> {
    if let Value::Set(s) = v
        && matches!(s.kind(), SetKind::Enum | SetKind::Raw | SetKind::Interval)
    {
        return Ok(Class::Reducible {
            set: s.kind() != SetKind::Interval,
        });
    }
    if value_class(v)? {
        Ok(Class::Plain)
    } else {
        Err(Error::unsupported(
            "x' \\in a startup value TLC represents lazily, whose enumeration depends on its history (core)",
        ))
    }
}

/// Whether an expression builds a set TLC may keep lazy with repeated
/// elements (`\cup`, `UNION`), at any depth, through definitions.
fn risky(ir: &dive_ir::Ir, e: ExprId, _depth: u32) -> bool {
    let mut seen = std::collections::HashSet::new();
    risky_in(ir, e, &mut seen)
}

fn risky_in(
    ir: &dive_ir::Ir,
    e: ExprId,
    seen: &mut std::collections::HashSet<dive_ir::DefId>,
) -> bool {
    let node = &ir.node(e).expr;
    if matches!(
        node,
        Expr::Builtin {
            op: BuiltinOp::Cup | BuiltinOp::BigUnion,
            ..
        }
    ) {
        return true;
    }
    if let Expr::Def(d)
    | Expr::Apply {
        callee: OpRef::Def(d),
        ..
    } = node
        && seen.insert(*d)
        && risky_in(ir, ir.def(*d).body, seen)
    {
        return true;
    }
    let mut found = false;
    node.for_each_child(|c| {
        if !found && risky_in(ir, c, seen) {
            found = true;
        }
    });
    found
}
