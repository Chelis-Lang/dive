//! Non-evaluating boundaries for independent context inspection.

use dive_eval::{ConstBinding, Scope, SlotView};
use dive_ir::{Arg, Expr, ExprId, Pass, SubstBinding};
use dive_model::Model;
use dive_value::{Atom, Value};

const VISITS: usize = 512;
const DEPTH: usize = 64;
const TEXT_BYTES: usize = 4096;

/// Admit eager bindings only from actual closed structural operands.
/// Every binding uses the original outer scope, matching `Core::bind_subst`.
/// Caller effect annotations never authorize eager work. Cached literal
/// errors may still fail ordinary entry; this proves no predicate truth.
pub(crate) fn substitution_is_admitted(
    model: &Model,
    scope: &Scope,
    bindings: &[SubstBinding],
) -> bool {
    let ev = &model.evaluator;
    if bindings.len() > VISITS
        || ev.validate_model_context(&model.ir, &model.names).is_err()
        || ev.validate_scope(scope).is_err()
        || !bindings.iter().all(|binding| {
            model.ir.decls().get(binding.decl.index()).is_some()
                && match binding.value {
                    Arg::Expr { expr, .. } => ev.validate_predicate(scope, expr).is_ok(),
                    Arg::Op { .. } => true,
                }
        })
    {
        return false;
    }
    let mut proof = Eager {
        model,
        fuel: VISITS,
    };
    bindings.iter().all(|binding| {
        proof.charge(0)
            && match binding.value {
                Arg::Expr {
                    expr,
                    pass: Pass::Strict,
                } => proof.expr(expr, scope, 0),
                _ => true,
            }
    })
}

struct Eager<'model> {
    model: &'model Model,
    fuel: usize,
}

impl Eager<'_> {
    fn charge(&mut self, depth: usize) -> bool {
        if depth > DEPTH || self.fuel == 0 {
            return false;
        }
        self.fuel -= 1;
        true
    }

    fn atom(&self, atom: &Atom) -> bool {
        atom.as_str().len() <= TEXT_BYTES
            && self
                .model
                .names
                .get(atom.as_str())
                .is_some_and(|receiving| receiving.tok() == atom.tok())
    }

    fn value(&mut self, value: &Value, depth: usize) -> bool {
        if !self.charge(depth) {
            return false;
        }
        match value {
            Value::Bool(_) | Value::Int(_) => true,
            Value::Str(atom) | Value::Model(atom) => self.atom(atom),
            Value::Fcn(function) => {
                if let Some(items) = function.as_tuple() {
                    items.len() <= self.fuel
                        && items.iter().all(|value| self.value(value, depth + 1))
                } else if let Some(fields) = function.as_record() {
                    fields.len() <= self.fuel / 2
                        && fields.iter().all(|(name, value)| {
                            self.charge(depth + 1)
                                && self.atom(name)
                                && self.value(value, depth + 1)
                        })
                } else {
                    false
                }
            }
            // No provider, general function or unbounded bigint work enters
            // constructor hashing while deriving this admission.
            Value::Set(_) | Value::BigInt(_) => false,
        }
    }

    fn slot(&mut self, slot: SlotView, depth: usize) -> bool {
        match slot {
            SlotView::Value(value) => self.value(&value, depth),
            SlotView::Lazy(expr, captured) => self.expr(expr, &captured, depth),
            _ => false,
        }
    }

    fn constant(&mut self, constant: dive_ir::ConstId, depth: usize) -> bool {
        match self.model.evaluator.constant_binding(constant) {
            Some(ConstBinding::Value(value)) => self.value(&value, depth),
            // No user/native/redirected definition executes during this proof.
            Some(ConstBinding::Def(_)) | None => false,
        }
    }

    fn expr(&mut self, expr: ExprId, scope: &Scope, depth: usize) -> bool {
        if !self.charge(depth)
            || self
                .model
                .evaluator
                .validate_predicate(scope, expr)
                .is_err()
        {
            return false;
        }
        let Some(node) = self.model.ir.nodes().get(expr.index()) else {
            return false;
        };
        match &node.expr {
            // Core::new plans these literals; eval_inner reads its cached Lit
            // (possibly an error), without interning or executing an operator.
            Expr::Bool(_) => true,
            Expr::Num(text) => {
                !text.is_empty()
                    && text.len() <= TEXT_BYTES
                    && text.bytes().all(|byte| byte.is_ascii_digit())
            }
            Expr::Str(text) => text.len() <= TEXT_BYTES && self.model.names.get(text).is_some(),
            Expr::Tuple(items) => {
                items.len() <= self.fuel
                    && items.iter().all(|&item| self.expr(item, scope, depth + 1))
            }
            Expr::Record(fields) => {
                fields.len() <= self.fuel / 2
                    && fields.iter().all(|field| {
                        self.charge(depth + 1)
                            && field.name.len() <= TEXT_BYTES
                            && self.model.names.get(&field.name).is_some()
                            && self.expr(field.value, scope, depth + 1)
                    })
            }
            Expr::Local(local) => {
                self.slot(self.model.evaluator.scope_slot(scope, *local), depth + 1)
            }
            Expr::Const(constant) => self.constant(*constant, depth + 1),
            Expr::SubstRef {
                decl,
                primed: false,
            } => match self.model.evaluator.scope_subst_slot(scope, *decl) {
                SlotView::Unset => self
                    .model
                    .ir
                    .decls()
                    .get(decl.index())
                    .filter(|declaration| !declaration.variable && declaration.var.is_none())
                    .and_then(|declaration| declaration.constant)
                    .is_some_and(|constant| self.constant(constant, depth + 1)),
                slot => self.slot(slot, depth + 1),
            },
            // No state/prime, evaluator calls, LET/Subst entry, providers,
            // redirects or caller effect annotations.
            _ => false,
        }
    }
}

pub(crate) const STRICT_SUBSTITUTION: &str =
    "Strict substitution requires evaluation before independent context admission";

#[cfg(test)]
#[allow(clippy::expect_used, reason = "independent eager admission controls")]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn fresh_model() -> Model {
        let source = "---- MODULE EagerAdmission ----\nEXTENDS Naturals, TLC\nCONSTANT C\nVARIABLE x\nCompound(p) == <<TRUE,7,\"fixed\",[f|->p]>>\nDanger(p) == PrintT(p)\nConstruct(p) == ToString(p)\nInit == x=TRUE\nNext == x'=x\n====\n";
        let analysis =
            dive_sema::analyze_source("EagerAdmission.tla", source, &dive_sema::Options::default());
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        dive_model::bind(
            Arc::new(dive_ir::lower(&analysis).expect("IR")),
            &dive_config::parse("INIT Init\nNEXT Next\nCONSTANT C=1\n")
                .into_config()
                .expect("cfg"),
            &dive_model::BindOptions::tlc(),
        )
        .expect("bind")
    }

    fn declaration(model: &Model) -> dive_ir::DeclId {
        let index = model
            .ir
            .decls()
            .iter()
            .position(|decl| decl.name == "C")
            .expect("constant");
        dive_ir::DeclId(u32::try_from(index).expect("declaration index"))
    }

    fn names(model: &Model) -> Vec<(String, u32)> {
        (0..model.names.len())
            .map(|index| {
                let atom = model
                    .names
                    .resolve(u32::try_from(index).expect("token"))
                    .expect("atom");
                (atom.as_str().to_owned(), atom.tok())
            })
            .collect()
    }

    fn binding(model: &Model, expr: ExprId) -> [SubstBinding; 1] {
        [SubstBinding {
            decl: declaration(model),
            value: Arg::Expr {
                expr,
                pass: Pass::Strict,
            },
        }]
    }

    #[test]
    fn literal_tuple_record_and_actual_stored_forwarding_admit_without_effects() {
        let model = fresh_model();
        model.validate_context().expect("all roles owned");
        let before = names(&model);
        let def = model.ir.root_def("Compound").expect("Compound");
        let next = model.next.as_ref().expect("Next");
        let scope = model
            .evaluator
            .scope_call_values(&next.scope, def, vec![Value::Bool(true)])
            .expect("stored parameter");
        assert!(substitution_is_admitted(
            &model,
            &scope,
            &binding(&model, model.ir.def(def).body)
        ));
        assert_eq!(names(&model), before);
        assert!(!model.evaluator.has_print_event());
        assert_eq!(model.take_output(), Vec::<String>::new());
    }

    #[test]
    fn calls_unknown_ids_and_foreign_scopes_refuse_without_effects() {
        let model = fresh_model();
        let before = names(&model);
        let next = model.next.as_ref().expect("Next");
        for name in ["Danger", "Construct"] {
            let def = model.ir.root_def(name).expect("definition");
            let scope = model
                .evaluator
                .scope_call_values(&next.scope, def, vec![Value::Int(0)])
                .expect("owned parameter");
            assert!(!substitution_is_admitted(
                &model,
                &scope,
                &binding(&model, model.ir.def(def).body)
            ));
        }
        assert!(!substitution_is_admitted(
            &model,
            &next.scope,
            &binding(&model, ExprId(u32::MAX))
        ));
        let other = fresh_model();
        assert!(!substitution_is_admitted(
            &model,
            &other.next.as_ref().expect("other Next").scope,
            &binding(&model, next.expr)
        ));
        assert_eq!(names(&model), before);
        assert!(!model.evaluator.has_print_event());
        assert_eq!(model.take_output(), Vec::<String>::new());
    }

    #[test]
    fn stored_atoms_require_tokens_and_visit_depth_bounds_do_not_grow() {
        let model = fresh_model();
        let before = names(&model);
        let atom = model.names.get("fixed").expect("planned literal");
        let foreign = dive_value::Interner::new();
        for index in 0..=atom.tok() {
            drop(foreign.intern(&format!("padding-{index}")));
        }
        let foreign_atom = foreign.intern("fixed");
        assert_ne!(foreign_atom.tok(), atom.tok());
        let mut proof = Eager {
            model: &model,
            fuel: VISITS,
        };
        assert!(!proof.value(&Value::Str(foreign_atom), 0));
        let mut proof = Eager {
            model: &model,
            fuel: 0,
        };
        assert!(!proof.value(&Value::Bool(true), 0));
        let mut proof = Eager {
            model: &model,
            fuel: VISITS,
        };
        assert!(!proof.value(&Value::Bool(true), DEPTH + 1));
        assert_eq!(names(&model), before);
        assert!(!model.evaluator.has_print_event());
        assert_eq!(model.take_output(), Vec::<String>::new());
    }
}
