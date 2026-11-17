//! The initial predicates and the next-state relation as the successor
//! generator's actions (`dive_eval::next`).
//!
//! `dive_ir::actions::split_with_values` builds the tree TLC's `Tool.getActions`
//! walks, with actual value lookups and configuration overrides as redirects;
//! `Evaluator::actions_in` expands it with values (split `\E` domains and
//! inlined arguments evaluated in TLC's empty state) into TLC's action list,
//! numbered after the initial predicates (`Tool.java:225-241`). A next-state
//! relation found under an instance's substitutions is one action, as TLC
//! does not split a `SubstIn` with substitutions.

use std::collections::{BTreeMap, BTreeSet};

use dive_eval::ErrorClass;
use dive_eval::next::Action;
use dive_ir::actions::{ActionName, split_with_values};
use dive_ir::{DefId, Expr};

use crate::bind::{Binder, R};
use crate::error::BindError;
use crate::tables::Look;

impl Binder<'_> {
    /// Actual non-definition lookups, without evaluating or forcing values.
    fn split_values(&self) -> R<BTreeSet<DefId>> {
        let mut out = BTreeSet::new();
        for i in 0..self.t.ir.defs().len() {
            let def = DefId(u32::try_from(i).map_err(|_| {
                BindError::unsupported("too many definitions for action splitting".to_owned())
            })?);
            if matches!(self.t.lookup_def(def), Look::Value(_) | Look::Literal(_)) {
                out.insert(def);
            }
        }
        Ok(out)
    }

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
                    .map_err(|e| {
                        self.eval_error(e, |e| crate::BindError::unsupported(e.to_string()))
                    })
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
                    .map_err(|e| {
                        self.eval_error(e, |e| crate::BindError::unsupported(e.to_string()))
                    })?,
            ]);
        }
        let redirects = self.split_redirects();
        let values = self.split_values()?;
        let mut tree =
            split_with_values(&ir, next.expr, name, next.scope.def(), &redirects, &values);
        // A configured constant can look up a definition even when a direct
        // reference to that definition looks up its start-up value. Configured
        // expansion needs the target's body tree, not a flattened value lookup.
        let targets: BTreeSet<_> = self
            .t
            .consts
            .iter()
            .filter_map(|look| match look {
                Some(Look::Def(def) | Look::Java(_, def)) => Some(*def),
                _ => None,
            })
            .collect();
        for def in targets {
            if tree.defs.contains_key(&def) {
                continue;
            }
            let target = split_with_values(
                &ir,
                ir.def(def).body,
                ActionName::Def(def),
                def,
                &redirects,
                &values,
            );
            tree.defs.insert(def, target.root);
            for (nested, body) in target.defs {
                tree.defs.entry(nested).or_insert(body);
            }
        }
        self.ev()
            .actions_in(&next.scope, &tree, first)
            .map_err(|e| {
                self.eval_error(e, |e| match e.class() {
                    ErrorClass::Unsupported | ErrorClass::ResourceLimit => BindError::unsupported(
                        format!("splitting the next-state relation: {}", e.message()),
                    ),
                    _ => BindError::bug(format!("splitting the next-state relation: {e}")),
                })
            })
    }
}
