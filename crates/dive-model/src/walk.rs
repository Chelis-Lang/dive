//! TLC's `SpecProcessor.processConstants` walk (tlaplus `94d0c50`).
//!
//! Before it binds the configuration, TLC visits every module in the module
//! table, and in each its definitions (`getOpDefs`), inner modules,
//! assumptions and theorems, and converts every literal to a value: a numeral
//! too big for TLC's integers fails with `TLC_INTEGER_TOO_BIG`, a decimal with
//! `TLC_CANT_HANDLE_REAL_NUMBERS`. The walk visits an operator application's
//! arguments before its bounded-quantifier domains, so in
//! `\E x \in {3000000000} : x = 1.5` the decimal is met first. (It also
//! creates `StringValue`s, but SANY has interned their contents already;
//! see [`crate::intern`].)

use std::collections::HashSet;

use dive_ir::{Arg, DefId, Expr, ExprId, Ir, ModuleId, OpRef};
use dive_value::{Int, IntMode};

use crate::error::{BindError, Ec};
use crate::tables::Tables;

/// What the walk found.
pub(crate) struct ConstWalk {
    /// The first literal TLC cannot handle.
    pub(crate) error: Option<BindError>,
}

struct Walker<'a> {
    ir: &'a Ir,
    mode: IntMode,
    out: ConstWalk,
    processed: HashSet<DefId>,
}

/// The file-level modules in TLC's module-table order (each after the
/// modules it depends on, the root last).
pub(crate) fn file_modules(ir: &Ir) -> Vec<ModuleId> {
    (0..ir.modules().len())
        .map(|i| ModuleId(u32::try_from(i).unwrap_or(u32::MAX)))
        .filter(|&m| ir.module(m).nesting == 0)
        .collect()
}

/// The inner modules directly nested in `m`, in the IR's order.
pub(crate) fn inner_modules(ir: &Ir, m: ModuleId) -> Vec<ModuleId> {
    (0..ir.modules().len())
        .map(|i| ModuleId(u32::try_from(i).unwrap_or(u32::MAX)))
        .filter(|&i| ir.module(i).parent == Some(m))
        .collect()
}

pub(crate) fn process_constants(t: &Tables, mode: IntMode) -> ConstWalk {
    let ir = &*t.ir;
    let mut w = Walker {
        ir,
        mode,
        out: ConstWalk { error: None },
        processed: HashSet::new(),
    };
    for m in file_modules(ir) {
        w.module(t, m);
    }
    w.out
}

impl Walker<'_> {
    fn module(&mut self, t: &Tables, m: ModuleId) {
        let ir = self.ir;
        for (_, d) in t.op_defs(m) {
            self.expr(ir.def(d).body);
        }
        for inner in inner_modules(ir, m) {
            self.module(t, inner);
        }
        let module = ir.module(m);
        for &a in &module.assumptions {
            self.expr(ir.def(a).body);
        }
        for &th in &module.theorems {
            self.expr(ir.def(th).body);
        }
    }

    fn fail(&mut self, e: BindError) {
        if self.out.error.is_none() {
            self.out.error = Some(e);
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one arm per node kind, in SANY's child order"
    )]
    fn expr(&mut self, root: ExprId) {
        let ir = self.ir;
        // An explicit stack in reverse visiting order.
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            let mut kids: Vec<ExprId> = Vec::new();
            match ir.expr(e) {
                Expr::Num(text) => {
                    if Int::from_literal(text, self.mode).is_err()
                        && text.bytes().all(|b| b.is_ascii_digit())
                    {
                        self.fail(BindError::tlc(Ec::TlcIntegerTooBig, &[text]));
                    }
                }
                Expr::Decimal { int, frac } => {
                    self.fail(BindError::tlc(
                        Ec::TlcCantHandleRealNumbers,
                        &[&format!("{int}.{frac}")],
                    ));
                }
                Expr::Quant { bounds, body, .. }
                | Expr::FcnCons { bounds, body }
                | Expr::RecFcn { bounds, body, .. } => {
                    kids.push(*body);
                    kids.extend(bounds.iter().filter_map(|b| b.domain));
                }
                Expr::SetFilter { bound, pred } => {
                    kids.push(*pred);
                    kids.extend(bound.domain);
                }
                Expr::SetMap { expr, bounds } => {
                    kids.push(*expr);
                    kids.extend(bounds.iter().filter_map(|b| b.domain));
                }
                Expr::Let { defs, body } => {
                    for &d in defs {
                        kids.push(ir.def(d).body);
                    }
                    kids.push(*body);
                }
                Expr::Apply { args, .. } => {
                    for a in args {
                        match a {
                            Arg::Expr { expr, .. } => kids.push(*expr),
                            Arg::Op {
                                op: OpRef::Def(d), ..
                            } => {
                                // `OpArgKind`: a user-defined operator passed as
                                // an argument has its body processed once.
                                if self.processed.insert(*d) {
                                    kids.push(ir.def(*d).body);
                                }
                            }
                            Arg::Op { .. } => {}
                        }
                    }
                }
                other => kids = other.children(),
            }
            for k in kids.into_iter().rev() {
                stack.push(k);
            }
        }
    }
}
