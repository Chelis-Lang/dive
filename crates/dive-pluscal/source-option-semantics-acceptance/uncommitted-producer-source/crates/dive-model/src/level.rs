//! TLC's level bound (`SymbolNodeValueLookupProvider.getLevelBound` and
//! `getLevelBoundAppl`, tlaplus `94d0c50`).
//!
//! TLC does not use SANY's levels when it binds a model: it computes an upper
//! bound of an expression's level itself, after the configuration has been
//! applied, and decides with it which constant definitions to evaluate at
//! start-up, which conjuncts of a `SPECIFICATION` are initial predicates and
//! how a `PROPERTY` splits. The bound differs from SANY's level:
//!
//! - a constant the configuration assigns a value has level 0, one it binds to
//!   a definition (`C <- D`) has `D`'s level;
//! - a definition TLC has already replaced by a value (start-up evaluation,
//!   `Op = v`) has level 0, a Java override its annotation's `minLevel`;
//! - any operator, parameter or bound variable whose *name* is the name of a
//!   root state variable has level 1 (`opName.getVarLoc() >= 0`);
//! - primes, `UNCHANGED`, `[A]_v`, `<<A>>_v` and `\cdot` are 2, temporal
//!   operators 3 and `ENABLED` 1, without looking inside;
//! - an operator's parameters are not bound when TLC looks into its body,
//!   so a parameter there has level 0 unless the context binds it.

use std::collections::HashSet;

use dive_eval::{Scope, SlotView};
use dive_ir::{
    Arg, BuiltinOp, DeclId, DefId, DefScope, Expr, ExprId, LocalRef, OpRef, SlotKind, StdOp,
};

use crate::tables::{Look, Tables};

/// A substitution in the level context: `decl` is bound to `expr`,
/// evaluated in `scope` with the substitutions `outer`.
#[derive(Clone)]
struct SubstLevel {
    decl: DeclId,
    expr: Option<(ExprId, Scope)>,
    outer: Vec<SubstLevel>,
}

/// The state of one level computation.
pub(crate) struct LevelCx<'a> {
    pub(crate) t: &'a Tables,
    /// Definitions bound to `ValOne` in TLC's context (recursion guard,
    /// `LET` definitions).
    marks: HashSet<DefId>,
    substs: Vec<SubstLevel>,
    depth: u32,
}

impl<'a> LevelCx<'a> {
    pub(crate) fn new(t: &'a Tables) -> Self {
        Self {
            t,
            marks: HashSet::new(),
            substs: Vec::new(),
            depth: 0,
        }
    }

    /// `getLevelBound(expr, c)` where `c` is what `scope` binds.
    pub(crate) fn level(&mut self, e: ExprId, scope: &Scope) -> u8 {
        // Deep recursion only through pathological definitions; TLC would
        // overflow its stack. Treat the rest as level 0, conservatively the
        // bound TLC computes for what it cannot look into.
        if self.depth > 2_000 {
            return 0;
        }
        self.depth += 1;
        let l = self.level_inner(e, scope);
        self.depth -= 1;
        l
    }

    #[allow(clippy::too_many_lines, reason = "one arm per node kind")]
    fn level_inner(&mut self, e: ExprId, scope: &Scope) -> u8 {
        let ir = &*self.t.ir;
        match ir.expr(e) {
            Expr::Bool(_)
            | Expr::Num(_)
            | Expr::Decimal { .. }
            | Expr::Str(_)
            | Expr::ExceptAt
            | Expr::Unbound(_)
            | Expr::Unevaluable(_) => 0,
            Expr::Var { primed, .. } => {
                if *primed {
                    2
                } else {
                    1
                }
            }
            Expr::Const(c) => {
                let decl = ir.consts()[c.index()];
                self.decl_level(decl, scope)
            }
            Expr::SubstRef { decl, primed } => {
                if *primed {
                    return 2;
                }
                self.decl_level(*decl, scope)
            }
            Expr::Local(r) => self.local_level(*r, scope),
            Expr::Def(d) => self.def_level(*d, scope),
            Expr::Apply { callee, args } => {
                let mut l = 0;
                for a in args {
                    if let Arg::Expr { expr, .. } = a {
                        l = l.max(self.level(*expr, scope));
                    }
                }
                l.max(self.callee_level(callee, scope))
            }
            Expr::Builtin { op, args } => match op {
                BuiltinOp::Prime | BuiltinOp::Unchanged | BuiltinOp::Cdot => 2,
                BuiltinOp::Always
                | BuiltinOp::Eventually
                | BuiltinOp::LeadsTo
                | BuiltinOp::WhilePlus => 3,
                BuiltinOp::Enabled => 1,
                _ => self.max_of(args, scope),
            },
            Expr::Prime(_)
            | Expr::Unchanged(_)
            | Expr::StepBox { .. }
            | Expr::StepAngle { .. }
            | Expr::Cdot(..) => 2,
            Expr::Enabled(_) => 1,
            Expr::Always(_)
            | Expr::Eventually(_)
            | Expr::LeadsTo(..)
            | Expr::WhilePlus(..)
            | Expr::Wf { .. }
            | Expr::Sf { .. }
            | Expr::TemporalQuant { .. } => 3,
            Expr::Let { defs, body } => {
                // `LetInKind`: each definition's body, then the body, with the
                // definitions marked (`c1.cons(opDef, ValOne)`).
                let mut l = 0;
                let mut marked = Vec::new();
                for &d in defs {
                    let s = self.t.ev().scope_unbound(scope, d);
                    l = l.max(self.level(ir.def(d).body, &s));
                    if self.marks.insert(d) {
                        marked.push(d);
                    }
                }
                l = l.max(self.level(*body, scope));
                for d in marked {
                    self.marks.remove(&d);
                }
                l
            }
            Expr::Subst { bindings, body } => {
                let outer = self.substs.clone();
                for b in bindings {
                    let expr = match &b.value {
                        Arg::Expr { expr, .. } => Some((*expr, scope.clone())),
                        Arg::Op { .. } => None,
                    };
                    self.substs.push(SubstLevel {
                        decl: b.decl,
                        expr,
                        outer: outer.clone(),
                    });
                }
                let l = self.level(*body, scope);
                self.substs = outer;
                l
            }
            Expr::Label { body, .. } | Expr::Selected(body) => self.level(*body, scope),
            other => {
                let mut l = 0;
                for c in other.children() {
                    l = l.max(self.level(c, scope));
                }
                l
            }
        }
    }

    fn max_of(&mut self, es: &[ExprId], scope: &Scope) -> u8 {
        let mut l = 0;
        for &e in es {
            l = l.max(self.level(e, scope));
        }
        l
    }

    /// The level of a constant or substituted declaration (`OpApplKind` with
    /// a declaration as the operator).
    fn decl_level(&mut self, decl: DeclId, scope: &Scope) -> u8 {
        let ir = &*self.t.ir;
        let d = ir.decl(decl);
        if self.t.is_root_var_name(&d.name) {
            return 1;
        }
        // A substitution bound by an enclosing `SubstIn` of this level
        // computation (`c1.cons(sub.getOp(), LazyValue)`).
        if let Some(pos) = self.substs.iter().rposition(|s| s.decl == decl) {
            let s = self.substs[pos].clone();
            return match s.expr {
                Some((expr, sc)) => {
                    let saved = std::mem::replace(&mut self.substs, s.outer);
                    let l = self.level(expr, &sc);
                    self.substs = saved;
                    l
                }
                None => 0,
            };
        }
        if d.variable {
            return 0;
        }
        match d.constant.and_then(|c| self.t.const_look(c)) {
            Some(Look::Def(def)) => self.def_body_level(def, scope),
            Some(Look::Java(op, _)) => min_level(op),
            _ => 0,
        }
    }

    fn local_level(&mut self, r: LocalRef, scope: &Scope) -> u8 {
        let info = self.t.slot_info(scope.def(), r);
        if let Some((name, _)) = &info
            && self.t.is_root_var_name(name)
        {
            return 1;
        }
        // A `LET` definition: TLC finds its node, marked inside the `LET`
        // and otherwise looked into like any definition.
        if let Some((_, SlotKind::LetValue(d))) = info {
            if self.marks.contains(&d) {
                return 0;
            }
            return self.def_body_level(d, scope);
        }
        match self.t.ev().scope_slot(scope, r) {
            SlotView::Lazy(expr, sc) => self.level(expr, &sc),
            SlotView::AnalysisBound => 1, // Pure unknowns are conservatively state level.
            SlotView::Unset | SlotView::Value(_) | SlotView::Other => 0,
        }
    }

    /// A use of a definition with no arguments.
    fn def_level(&mut self, d: DefId, scope: &Scope) -> u8 {
        if self.t.is_root_var_name(&self.t.ir.def(d).name) {
            return 1;
        }
        match self.t.lookup_def(d) {
            Look::Def(r) => self.def_body_level(r, scope),
            Look::Java(op, _) => min_level(op),
            Look::Value(_) | Look::Literal(_) => 0,
        }
    }

    /// `c.cons(opNode, ValOne)` and the level of the body.
    fn def_body_level(&mut self, d: DefId, scope: &Scope) -> u8 {
        if !self.marks.insert(d) {
            return 0;
        }
        let s = self.t.ev().scope_unbound(scope, d);
        let l = self.level(self.t.ir.def(d).body, &s);
        self.marks.remove(&d);
        l
    }

    fn callee_level(&mut self, callee: &OpRef, scope: &Scope) -> u8 {
        let ir = &*self.t.ir;
        match callee {
            OpRef::Def(d) => {
                if self.t.is_root_var_name(&ir.def(*d).name) {
                    return 1;
                }
                match self.t.lookup_def(*d) {
                    Look::Def(r) => self.def_body_level(r, scope),
                    Look::Java(op, _) => min_level(op),
                    Look::Value(_) | Look::Literal(_) => 0,
                }
            }
            OpRef::Std(op) => min_level(*op),
            OpRef::Builtin(op) => match op {
                BuiltinOp::Prime | BuiltinOp::Unchanged | BuiltinOp::Cdot => 2,
                BuiltinOp::Always
                | BuiltinOp::Eventually
                | BuiltinOp::LeadsTo
                | BuiltinOp::WhilePlus => 3,
                BuiltinOp::Enabled => 1,
                _ => 0,
            },
            OpRef::Const(c) => {
                let decl = ir.consts()[c.index()];
                self.decl_level(decl, scope)
            }
            OpRef::Subst(decl) => self.decl_level(*decl, scope),
            OpRef::Param(r) => {
                if let Some((name, _)) = self.t.slot_info(scope.def(), *r)
                    && self.t.is_root_var_name(&name)
                {
                    return 1;
                }
                0
            }
            OpRef::Unbound(_) => 0,
        }
    }
}

/// The `minLevel` of a Java override's annotation (0 for unannotated module
/// methods): `TLCGet`, `TLCExt!CounterExample` and `Trace`, the trace
/// modules' `_TLCState`, and the `IOUtils` operators that run processes.
fn min_level(op: StdOp) -> u8 {
    match (op.module(), op.name()) {
        ("TLC", "TLCGet")
        | ("TLCExt", "CounterExample" | "Trace")
        | ("_JsonTrace" | "_TLCTrace", "_TLCState")
        | ("IOUtils", "IOExec" | "IOEnvExec" | "IOExecTemplate" | "IOEnvExecTemplate") => 1,
        _ => 0,
    }
}

/// Whether a definition sits in another definition's frame (for slot names).
pub(crate) fn parent_of(scope: DefScope) -> Option<DefId> {
    match scope {
        DefScope::Local { parent }
        | DefScope::Inline { parent }
        | DefScope::LetValue { parent, .. } => Some(parent),
        DefScope::TopLevel => None,
    }
}
