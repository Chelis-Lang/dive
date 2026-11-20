//! Side effects that remain after start-up.
//!
//! The binder evaluates each zero-arity constant-level definition once at
//! start-up and keeps its value, as TLC's `SpecProcessor.processConstantDefns`
//! does; a configured `Op = v` likewise replaces a definition by a value.
//! Checking then reads the kept value and never evaluates the body again, so
//! a `RandomElement` in such a body draws once, from the run's start-up
//! stream (reported with its seed), and never while checking. The IR's
//! syntactic [`Effects::IMPURE`] flag cannot see this: it propagates through
//! every reference. [`Evaluator::impure_after_startup`] answers the same
//! question with the fixed definitions as values, and is conservative in
//! every case it does not resolve.

use std::sync::OnceLock;

use dive_ir::{Arg, DefId, Effects, Expr, ExprId, OpRef, StdOp};
use dive_value::Value;

use crate::{ConstBinding, Evaluator};

impl Evaluator {
    /// Whether start-up fixed the value of `def` for the whole run: it was
    /// precomputed to an eager value, or the configuration assigns it a
    /// value, it is not redirected to another definition, and no configured
    /// redirect ([`Self::redirect_target`]) evaluates its body.
    ///
    /// A precomputed value that holds a deferred set anywhere inside it
    /// (`{x \in S : P}` or `{e : x \in S}` kept lazy) is not fixed: each
    /// membership test or enumeration evaluates its predicate or map again
    /// while checking, so its body still runs after start-up.
    ///
    /// ```
    /// # fn example(ev: &dive_eval::Evaluator, d: dive_ir::DefId) {
    /// if ev.startup_fixed(d) {
    ///     assert!(ev.precomputed(d).is_some() || ev.is_value_override(d));
    ///     assert!(!ev.redirect_target(d));
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn startup_fixed(&self, def: DefId) -> bool {
        self.redirected(def) == def
            && (self.precomputed_eager(def) || self.is_value_override(def))
            && !self.redirect_target(def)
    }

    /// Whether `def` has a precomputed value with no deferred set anywhere
    /// inside it. The answer is kept once the value is set, which start-up
    /// does at most once.
    fn precomputed_eager(&self, def: DefId) -> bool {
        let core = &self.core;
        let (Some(value), Some(eager)) = (
            core.startup.get(def.index()).and_then(OnceLock::get),
            core.startup_eager.get(def.index()),
        ) else {
            return false;
        };
        *eager.get_or_init(|| no_deferred_set(value))
    }

    /// Whether a configured redirect evaluates `def`'s body at each use,
    /// whatever value start-up gave `def` itself: it is the target of an
    /// `Op <- def` override (of a definition or of a Java-implemented
    /// operator), or of a constant bound by `C <- def`
    /// (`SpecProcessor.java:276-316`).
    ///
    /// ```
    /// # fn example(ev: &dive_eval::Evaluator, d: dive_ir::DefId) {
    /// if ev.redirect_target(d) {
    ///     assert!(!ev.startup_fixed(d));
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn redirect_target(&self, def: DefId) -> bool {
        let core = &self.core;
        core.redirects
            .iter()
            .any(|(&s, &t)| s != t && core.redirect(t) == def)
            || core.consts.iter().flatten().any(|b| match b {
                ConstBinding::Def(t) => core.redirect(*t) == def,
                ConstBinding::Value(_) => false,
            })
    }

    /// The definition a constant bound by `C <- D` evaluates at each use.
    fn bound_def(&self, c: dive_ir::ConstId) -> Option<DefId> {
        match self.core.consts.get(c.index()) {
            Some(Some(ConstBinding::Def(d))) => Some(*d),
            _ => None,
        }
    }

    /// Whether some configured redirect evaluates a body whose IR effects
    /// are impure. The IR's flags propagate through syntactic references
    /// only, so a node that reaches such a redirect can look pure.
    fn impure_redirect(&self) -> bool {
        let ir = self.ir();
        let core = &self.core;
        let impure = |t: DefId| {
            ir.defs().get(core.redirect(t).index()).is_none_or(|d| {
                d.effects.intersects(Effects::IMPURE) || d.std.is_some_and(StdOp::impure)
            })
        };
        core.redirects.iter().any(|(&s, &t)| s != t && impure(t))
            || core.consts.iter().flatten().any(|b| match b {
                ConstBinding::Def(t) => impure(*t),
                ConstBinding::Value(_) => false,
            })
    }

    /// Project known plain scalar assignments through INSTANCE and operator
    /// arguments with the same effect propagation as source lowering. Configured
    /// redirects can introduce effects absent from the source inventory.
    pub(crate) fn configured_effects(&self) -> Option<&[Effects]> {
        self.core
            .configured_effects
            .get_or_init(|| {
                let ir = self.ir();
                let mut fixed = std::collections::BTreeSet::new();
                for (&id, value) in &self.core.def_values {
                    let def = ir.defs().get(id.index())?;
                    if matches!(value, Value::Str(_) | Value::Bool(_) | Value::Int(_))
                        && def.std.is_none()
                        && def.params.is_empty()
                        && def.effects.intersects(Effects::IMPURE)
                        && self.redirected(id) == id
                        && !self.redirect_target(id)
                    {
                        fixed.insert(id);
                    }
                }
                if fixed.is_empty() || self.impure_redirect() {
                    return None;
                }
                Some(ir.effects_with_fixed_definitions(&fixed))
            })
            .as_deref()
    }

    /// Whether the source expression is marked impure after accounting for
    /// plain scalar configuration assignments. Use the evaluator's separate
    /// scope and callable checks for dynamically supplied arguments.
    ///
    /// ```
    /// # fn example(ev: &dive_eval::Evaluator, e: dive_ir::ExprId, scope: &dive_eval::Scope) {
    /// let may_run_effect = ev.source_may_have_effects(e) || ev.scope_shares_side_effects(scope);
    /// # let _ = may_run_effect;
    /// # }
    /// ```
    #[must_use]
    pub fn source_may_have_effects(&self, expr: ExprId) -> bool {
        self.configured_effects()
            .and_then(|es| es.get(expr.index()).copied())
            .or_else(|| self.ir().nodes().get(expr.index()).map(|n| n.effects))
            .is_none_or(|flags| flags.intersects(Effects::IMPURE))
    }

    /// Whether evaluating `expr` while checking can have a side effect
    /// (output, registers, randomness, time, I/O or checker state): its
    /// IR [`Effects::IMPURE`] flag, except that a definition
    /// [`Self::startup_fixed`] is a value, and that a configured redirect
    /// (`Op <- D`, `C <- D`) reaches `D`'s body. When some redirect reaches
    /// an impure body, the IR's flags, which follow syntactic references
    /// only, are not trusted and the walk visits every reachable node. Every
    /// source the walk does not resolve (operator parameters, instance
    /// substitutions, `LET` and parameter reads, an unknown node) counts as
    /// impure.
    ///
    /// ```
    /// # fn example(ev: &dive_eval::Evaluator, e: dive_ir::ExprId) {
    /// if !ev.impure_after_startup(e) {
    ///     // Evaluating `e` while checking has no side effect.
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn impure_after_startup(&self, expr: ExprId) -> bool {
        let ir = self.ir();
        let core = &self.core;
        let trust = !self.impure_redirect();
        let impure = |e: ExprId| {
            !trust
                || ir
                    .nodes()
                    .get(e.index())
                    .is_none_or(|n| n.effects.intersects(Effects::IMPURE))
        };
        if trust && !self.source_may_have_effects(expr) {
            return false;
        }
        let mut seen_defs = vec![false; ir.defs().len()];
        let mut seen = vec![false; ir.nodes().len()];
        let mut work = vec![expr];
        // A definition the walk reaches: its body, and its redirect target.
        let mut reach = |d: DefId, work: &mut Vec<ExprId>| -> bool {
            for d in [d, self.redirected(d)] {
                if self.startup_fixed(d) {
                    continue;
                }
                let Some(slot) = seen_defs.get_mut(d.index()) else {
                    return true;
                };
                if *slot {
                    continue;
                }
                *slot = true;
                let Some(def) = ir.defs().get(d.index()) else {
                    return true;
                };
                if trust && !def.effects.intersects(Effects::IMPURE) {
                    continue;
                }
                if let Some(op) = def.std {
                    if trust || op.impure() {
                        return true;
                    }
                    continue;
                }
                work.push(def.body);
            }
            false
        };
        while let Some(e) = work.pop() {
            match seen.get_mut(e.index()) {
                Some(s) if !*s => *s = true,
                Some(_) => continue,
                None => return true,
            }
            if !impure(e) {
                continue;
            }
            let node = ir.node(e);
            match &node.expr {
                Expr::Def(d) if reach(*d, &mut work) => return true,
                Expr::Const(c) if self.bound_def(*c).is_some_and(|d| reach(d, &mut work)) => {
                    return true;
                }
                Expr::Local(_) | Expr::SubstRef { .. } | Expr::Subst { .. } => return true,
                Expr::Apply { callee, args } => {
                    let key = args.first().and_then(|a| match a {
                        Arg::Expr { expr, .. } => Some(ir.expr(*expr)),
                        Arg::Op { .. } => None,
                    });
                    let mut src = |op: &OpRef, key, work: &mut Vec<ExprId>| match op {
                        OpRef::Const(c) => self.bound_def(*c).is_some_and(|d| reach(d, work)),
                        // `Java <- D` replaces the Java method by `D`.
                        OpRef::Std(op) if core.std_redirects.contains_key(op) => {
                            core.std_redirects.get(op).is_none_or(|&d| reach(d, work))
                        }
                        op => opref_impure(op, key, work, &mut reach),
                    };
                    if src(callee, key, &mut work) {
                        return true;
                    }
                    for a in args {
                        if let Arg::Op { op, .. } = a
                            && src(op, None, &mut work)
                        {
                            return true;
                        }
                    }
                }
                Expr::Let { defs, .. } => {
                    for d in defs {
                        if reach(*d, &mut work) {
                            return true;
                        }
                    }
                }
                _ => {}
            }
            node.expr.for_each_child(|c| work.push(c));
        }
        false
    }
}

/// Whether an operator reference is itself an impure source; a definition
/// is walked instead.
fn opref_impure(
    op: &OpRef,
    key: Option<&Expr>,
    work: &mut Vec<ExprId>,
    reach: &mut impl FnMut(DefId, &mut Vec<ExprId>) -> bool,
) -> bool {
    match op {
        OpRef::Def(d) => reach(*d, work),
        OpRef::Std(StdOp::TlcTLCGet) => match key {
            Some(Expr::Str(k)) => dive_ir::effects::tlcget_string_is_live(k),
            _ => true,
        },
        OpRef::Std(op) => op.impure(),
        // Operator parameters and substitutions carry the effects of any
        // operator passed as an argument anywhere.
        OpRef::Param(_) | OpRef::Subst(_) => true,
        OpRef::Builtin(_) | OpRef::Const(_) | OpRef::Unbound(_) => false,
    }
}

/// Whether `value` holds no deferred set anywhere: in set elements, in the
/// components of a lazy set (`SUBSET S`, `[S -> T]`, `S \X T`, records,
/// `Seq(S)`), or in function keys and values. The walk reads stored
/// structure only and never invokes a deferred provider.
fn no_deferred_set(value: &Value) -> bool {
    let mut work = vec![value.clone()];
    while let Some(value) = work.pop() {
        match value {
            Value::Set(set) => {
                if set.as_deferred().is_some() {
                    return false;
                }
                if let Some(elements) = set.stored_elements() {
                    work.extend(elements.iter().cloned());
                }
                work.extend(set.components().into_iter().map(|(_, c)| c));
            }
            Value::Fcn(f) => {
                for i in 0..f.len() {
                    if let Some((key, val)) = f.entry_cow(i) {
                        work.push(key.into_owned());
                        work.push(val.clone());
                    }
                }
            }
            Value::Bool(_) | Value::Int(_) | Value::BigInt(_) | Value::Str(_) | Value::Model(_) => {
            }
        }
    }
    true
}
