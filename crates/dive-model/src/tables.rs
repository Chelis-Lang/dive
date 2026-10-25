//! What TLC attaches to the specification while it binds a model: the
//! "tool objects" of definitions, definition bodies and constant
//! declarations, and the name table `defns` that the configuration's names
//! are looked up in (`tlc2.tool.Defns`, filled by `SpecProcessor`).
//!
//! TLC's `SymbolNodeValueLookupProvider.lookup(opNode, c)` returns, in order:
//! a binding in the context `c`, the node's tool object, the tool object of
//! the definition's body (looking through substitution wrappers), or the node
//! itself. [`Tables::lookup_def`] and [`Tables::const_look`] are that lookup
//! for an empty context.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dive_eval::Evaluator;
use dive_ir::{DefId, DefKind, Ir, LocalRef, SlotKind, StdOp, Symbol};
use dive_value::Value;

use crate::level::parent_of;

/// What a lookup finds.
#[derive(Clone, Debug)]
pub(crate) enum Look {
    /// A definition (`OpDefNode`).
    Def(DefId),
    /// A value.
    Value(Value),
    /// A Java override (`MethodValue`, `EvaluatingValue`) and the
    /// definition it overrides.
    Java(StdOp, DefId),
    /// A numeral or string literal that is a definition's whole body:
    /// `processConstants` gives the literal node its value as tool object,
    /// and TLC's lookup of the definition finds it there.
    Literal(dive_ir::ExprId),
}

/// The binding stage's view of TLC's tool objects.
pub(crate) struct Tables {
    pub(crate) ir: Arc<Ir>,
    ev: Evaluator,
    pub(crate) controlled: bool,
    pub(crate) fallible_errors: bool,
    root_vars: HashSet<String>,
    /// Root constants' tool objects, by `ConstId`.
    pub(crate) consts: Vec<Option<Look>>,
    /// Definitions' tool objects (`opDef.setToolObject`): configuration
    /// values (`Op = v`), overrides (`Op <- D`) and start-up values.
    pub(crate) tool_obj: HashMap<DefId, Look>,
    /// Definition bodies' tool objects (`opDef.getBody().setToolObject`):
    /// module-scoped values and overrides (`[M]`). Java overrides are the
    /// IR's [`dive_ir::Def::std`].
    pub(crate) body_obj: HashMap<DefId, Look>,
}

impl Tables {
    pub(crate) fn new(ir: Arc<Ir>) -> Result<Self, crate::BindError> {
        let n = ir.consts().len();
        // A placeholder until the constants are bound, with its own name
        // table so that it interns nothing in the run's.
        let ev = Evaluator::new(
            dive_eval::Model::new(ir.clone()),
            dive_eval::Options::default(),
        )
        .map_err(|e| crate::BindError::unsupported(e.to_string()))?;
        let root_vars = ir.vars().iter().map(|&d| ir.decl(d).name.clone()).collect();
        Ok(Self {
            ir,
            ev,
            controlled: false,
            fallible_errors: false,
            root_vars,
            consts: vec![None; n],
            tool_obj: HashMap::new(),
            body_obj: HashMap::new(),
        })
    }

    pub(crate) fn set_evaluator(&mut self, ev: Evaluator, controlled: bool) {
        self.ev = ev;
        self.controlled = controlled;
    }

    pub(crate) fn eval_error(
        &self,
        error: dive_eval::EvalError,
        legacy: impl FnOnce(dive_eval::EvalError) -> crate::BindError,
    ) -> crate::control::StepError {
        crate::control::eval_error(self.controlled, self.fallible_errors, error, legacy)
    }

    pub(crate) fn is_exception(&self, error: &dive_eval::EvalError) -> bool {
        ((self.controlled || self.fallible_errors)
            && error.class() == dive_eval::ErrorClass::Interrupted)
            || (self.fallible_errors
                && matches!(
                    error.class(),
                    dive_eval::ErrorClass::ResourceLimit | dive_eval::ErrorClass::Bug
                ))
    }

    /// The evaluator (a placeholder until the constants are bound).
    pub(crate) fn ev(&self) -> &Evaluator {
        &self.ev
    }

    /// Whether `name` is the name of a root state variable, which gives any
    /// operator of that name level 1 in TLC's level bound
    /// (`UniqueString.getVarLoc`).
    pub(crate) fn is_root_var_name(&self, name: &str) -> bool {
        self.root_vars.contains(name)
    }

    /// `lookup(opDef, Context.Empty)`.
    pub(crate) fn lookup_def(&self, d: DefId) -> Look {
        if let Some(l) = self.tool_obj.get(&d) {
            return l.clone();
        }
        if let Some(l) = self.body_obj.get(&d) {
            return l.clone();
        }
        if let Some(op) = self.ir.def(d).std {
            return Look::Java(op, d);
        }
        // `while (result == null && body is SubstInNode)`: the literal under
        // an instance's substitutions.
        let mut body = self.ir.def(d).body;
        loop {
            match self.ir.expr(body) {
                dive_ir::Expr::Num(_) | dive_ir::Expr::Str(_) => return Look::Literal(body),
                dive_ir::Expr::Subst { body: b, .. } => body = *b,
                _ => break,
            }
        }
        Look::Def(d)
    }

    /// A root constant's tool object.
    pub(crate) fn const_look(&self, c: dive_ir::ConstId) -> Option<Look> {
        self.consts.get(c.index()).cloned().flatten()
    }

    /// The name and kind of the slot a local reference names, resolved from
    /// the definition whose frame the reference is evaluated in.
    pub(crate) fn slot_info(&self, frame: DefId, r: LocalRef) -> Option<(String, SlotKind)> {
        let ir = &*self.ir;
        let mut target = frame;
        if let dive_ir::DefScope::LetValue { parent, .. } = ir.def(target).scope {
            target = parent;
        }
        for _ in 0..r.up {
            target = parent_of(ir.def(target).scope)?;
            if let dive_ir::DefScope::LetValue { parent, .. } = ir.def(target).scope {
                target = parent;
            }
        }
        let slot = ir.def(target).frame.get(r.slot.index())?;
        Some((slot.name.clone(), slot.kind.clone()))
    }

    /// The root module's definitions in TLC's order (`getOpDefs`): every
    /// operator and function definition of its context, in insertion order.
    pub(crate) fn op_defs(&self, module: dive_ir::ModuleId) -> Vec<(String, DefId)> {
        let ir = &*self.ir;
        ir.module(module)
            .context
            .iter()
            .filter_map(|(n, s)| match s {
                Symbol::Def(d)
                    if matches!(ir.def(*d).kind, DefKind::Operator | DefKind::Function) =>
                {
                    Some((n.clone(), *d))
                }
                _ => None,
            })
            .collect()
    }
}
