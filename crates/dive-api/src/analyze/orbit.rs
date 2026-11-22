//! Resolved source observers for an explicitly declared carrier permutation.
use std::collections::BTreeSet;

use dive_ir::{Arg, Effects, Expr, OpRef, QuantKind};
use serde_json::{Value, json};

use super::AnalyzeReport;

impl AnalyzeReport {
    /// Inspect source operators that may distinguish names of permuted model values.
    ///
    /// The inventory covers all user-file definitions, including unused definitions.
    /// A bounded choice over a named constant still requires a pointwise-fixed domain.
    /// This preflight does not establish totality or certify an arbitrary symmetry.
    ///
    /// # Errors
    /// Semantic analysis or lowering failed.
    pub fn orbit_observers(&self) -> Result<Value, String> {
        if !self.accepted() {
            return Err("source did not pass semantic analysis".into());
        }
        let ir = dive_ir::lower(self.analysis()).map_err(|e| format!("{e:?}"))?;
        let mut refusals = BTreeSet::new();
        let mut domains = BTreeSet::new();
        for node in ir.nodes() {
            if node
                .origin
                .file
                .is_none_or(|id| ir.files()[id.index()].standard)
            {
                continue;
            }
            let location = || ir.location(&node.origin);
            if node
                .effects
                .intersects(Effects::IMPURE.union(Effects::LIVE_TLCGET))
            {
                refusals.insert(format!("effect at {}", location()));
            }
            let mut operator = |op: &OpRef| {
                let library = match *op {
                    OpRef::Std(s) => Some(s),
                    OpRef::Def(d) => ir.def(d).std,
                    OpRef::Builtin(_) => None,
                    _ => {
                        refusals.insert(format!("dynamic operator at {}", location()));
                        return;
                    }
                };
                if let Some(s) = library
                    && !matches!(
                        s.module(),
                        "Naturals" | "Integers" | "FiniteSets" | "Sequences"
                    )
                {
                    refusals.insert(format!("{}!{} at {}", s.module(), s.name(), location()));
                }
            };
            match &node.expr {
                Expr::Apply { callee, args } => {
                    operator(callee);
                    for arg in args {
                        if let Arg::Op { op, .. } = arg {
                            operator(op);
                        }
                    }
                }
                Expr::Def(d) => operator(&OpRef::Def(*d)),
                Expr::Quant {
                    kind: QuantKind::Choose,
                    bounds,
                    ..
                } => {
                    let constant = match bounds.as_ref() {
                        [b] => b.domain.and_then(|p| match ir.expr(p) {
                            Expr::Const(c) => {
                                let id = ir.consts()[c.index()];
                                let declaration = ir.decl(id);
                                (ir.lookup(ir.root(), &declaration.name)
                                    == Some(dive_ir::Symbol::Decl(id)))
                                .then(|| declaration.name.clone())
                            }
                            _ => None,
                        }),
                        _ => None,
                    };
                    if let Some(name) = constant {
                        domains.insert(name);
                    } else {
                        refusals.insert(format!(
                            "CHOOSE without a direct fixed constant of the root module at {}",
                            location()
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(json!({"choice_domains": domains, "refusals": refusals}))
    }
}
