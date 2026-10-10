//! The initial predicates and the next-state relation as the successor
//! generator's actions (`dive_eval::next`, WP-C2).
//!
//! `dive_ir::actions::split` builds the tree TLC's `Tool.getActions` walks,
//! with the configuration's overrides as redirects;
//! `Evaluator::actions_in` expands it with values (split `\E` domains and
//! inlined arguments evaluated in TLC's empty state) into TLC's action list,
//! numbered after the initial predicates (`Tool.java:225-241`). A next-state
//! relation found under an instance's substitutions is one action, as TLC
//! does not split a `SubstIn` with substitutions.

use std::collections::BTreeMap;

use dive_eval::ErrorClass;
use dive_eval::next::Action;
use dive_ir::actions::{ActionName, split};
use dive_ir::{DefId, Expr};

use crate::bind::{Binder, R};
use crate::error::BindError;
use crate::tables::Look;

impl Binder<'_> {
    /// The configuration's overrides of definitions, as `split` takes them.
    fn split_redirects(&self) -> BTreeMap<DefId, DefId> {
        let mut out = BTreeMap::new();
        for map in [&self.t.tool_obj, &self.t.body_obj] {
            for (d, l) in map {
                if let Look::Def(r) | Look::Java(_, r) = l {
                    out.entry(*d).or_insert(*r);
                }
            }
        }
        out
    }

    /// The initial predicates (`getInitStateSpec`) as actions, numbered from 0.
    pub(crate) fn init_actions(&self) -> R<Vec<Action>> {
        self.init
            .iter()
            .enumerate()
            .map(|(i, init)| {
                let name = init.def.map_or(ActionName::Unnamed, ActionName::Def);
                self.ev()
                    .predicate_in(&init.pred.scope, init.pred.expr, name, i)
                    .map_err(|e| crate::BindError::unsupported(e.to_string()))
            })
            .collect()
    }

    /// `Tool.getActions(next)`.
    pub(crate) fn expand_actions(&self) -> R<Vec<Action>> {
        let Some(next) = self.next.clone() else {
            return Ok(Vec::new());
        };
        let ir = self.t.ir.clone();
        let first = self.init.len();
        let name = self.next_def.map_or(ActionName::Unnamed, ActionName::Def);
        if next.location != next.expr
            && let Expr::Subst { bindings, .. } = ir.expr(next.location)
            && !bindings.is_empty()
        {
            return Ok(vec![
                self.ev()
                    .predicate_in(&next.scope, next.expr, name, first)
                    .map_err(|e| crate::BindError::unsupported(e.to_string()))?,
            ]);
        }
        let tree = split(
            &ir,
            next.expr,
            name,
            next.scope.def(),
            &self.split_redirects(),
        );
        self.ev()
            .actions_in(&next.scope, &tree, first)
            .map_err(|e| match e.class() {
                ErrorClass::Unsupported | ErrorClass::ResourceLimit => BindError::unsupported(
                    format!("splitting the next-state relation: {}", e.message()),
                ),
                _ => BindError::bug(format!("splitting the next-state relation: {e}")),
            })
    }
}
