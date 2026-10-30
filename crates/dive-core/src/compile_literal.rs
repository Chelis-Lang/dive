//! Selection of a closed, name-free local literal lane. This is producer and
//! consumer eligibility, never a proof inferred from an extensional handle.

use super::{
    Arg, BuiltinOp, Compiler, DefId, DefScope, Expr, ExprId, Handle, HashMap, OpRef, Slot, StdOp,
};

impl Compiler<'_> {
    fn literal_tree(&self, e: ExprId, depth: usize) -> bool {
        let mut remaining = crate::compare::LITERAL_WORK;
        self.literal_tree_work(e, depth, &mut remaining)
    }

    fn literal_tree_work(&self, e: ExprId, depth: usize, remaining: &mut usize) -> bool {
        if *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        if depth > 64 {
            return false;
        }
        match &self.ir.node(e).expr {
            Expr::Bool(_) => true,
            Expr::Num(text) => text.parse::<i64>().ok().and_then(Handle::int).is_some(),
            Expr::Tuple(items) => items
                .iter()
                .all(|&p| self.literal_tree_work(p, depth + 1, remaining)),
            _ => false,
        }
    }

    fn literal_constructor(&self, e: ExprId) -> bool {
        let mut remaining = crate::compare::LITERAL_WORK - 1;
        matches!(&self.ir.node(e).expr, Expr::SetEnum(items)
            if items.iter().all(|&p| self.literal_tree_work(p, 0, &mut remaining)))
    }

    fn retain_literal_tuples(&mut self, e: ExprId) {
        if let Expr::Tuple(items) = &self.ir.node(e).expr {
            let items = items.clone();
            self.literal_tuples.insert(e);
            for p in items {
                self.retain_literal_tuples(p);
            }
        }
    }

    fn admit_literal(&mut self, e: ExprId) {
        if let Expr::SetEnum(items) = &self.ir.node(e).expr {
            let items = items.clone();
            self.literal_sets.insert(e);
            for p in items {
                self.retain_literal_tuples(p);
            }
        }
    }

    fn is_literal_cardinality(&self, callee: &OpRef) -> bool {
        let op = match callee {
            OpRef::Std(op) => Some(*op),
            OpRef::Def(def) => self.ir.def(self.redirect(*def)).std,
            _ => None,
        };
        op == Some(StdOp::FiniteSetsCardinality)
            && !self
                .h
                .std_redirects
                .contains_key(&StdOp::FiniteSetsCardinality)
    }

    fn local_literal_operand(&self, e: ExprId, roots: &HashMap<Slot, ExprId>) -> bool {
        self.literal_constructor(e)
            || matches!(self.ir.node(e).expr,
            Expr::Local(local) if local.up == 0 && roots.contains_key(&local.slot))
    }

    /// Every reference to a selected LET value must terminate at a covered
    /// consumer. Binding/capture scopes and arbitrary calls are excluded.
    fn safe_literal_uses(
        &self,
        e: ExprId,
        roots: &HashMap<Slot, ExprId>,
        allowed: bool,
        depth: usize,
    ) -> bool {
        if depth > 64 {
            return false;
        }
        let expr = &self.ir.node(e).expr;
        match expr {
            Expr::Local(local) => local.up != 0 || !roots.contains_key(&local.slot) || allowed,
            Expr::Builtin {
                op: BuiltinOp::In | BuiltinOp::NotIn,
                args,
            } if args.len() == 2 => {
                self.literal_tree(args[0], 0)
                    && self.local_literal_operand(args[1], roots)
                    && self.safe_literal_uses(args[1], roots, true, depth + 1)
            }
            Expr::Apply { callee, args } if self.is_literal_cardinality(callee) => {
                let [Arg::Expr { expr, .. }] = &args[..] else {
                    return false;
                };
                self.local_literal_operand(*expr, roots)
                    && self.safe_literal_uses(*expr, roots, true, depth + 1)
            }
            Expr::Bool(_) | Expr::Num(_) | Expr::Var { .. } => true,
            Expr::Builtin {
                op: BuiltinOp::Eq | BuiltinOp::Neq,
                ..
            }
            | Expr::Junction { .. }
            | Expr::Not(_)
            | Expr::Implies(..)
            | Expr::Equiv(..)
            | Expr::If { .. }
            | Expr::Case { .. } => expr
                .children()
                .into_iter()
                .all(|p| self.safe_literal_uses(p, roots, false, depth + 1)),
            Expr::Selected(p) | Expr::Label { body: p, .. } => {
                self.safe_literal_uses(*p, roots, allowed, depth + 1)
            }
            _ => false,
        }
    }

    fn select_local_literals(&mut self, defs: &[DefId], body: ExprId) {
        let mut roots = HashMap::new();
        for &def in defs {
            let d = self.ir.def(def);
            let DefScope::LetValue { slot, .. } = d.scope else {
                return;
            };
            if !d.params.is_empty() {
                return;
            }
            if self.literal_constructor(d.body) {
                roots.insert(slot, d.body);
            }
        }
        // Alias-only definitions reuse the original constructor identity.
        for _ in 0..defs.len() {
            for &def in defs {
                let d = self.ir.def(def);
                let DefScope::LetValue { slot, .. } = d.scope else {
                    return;
                };
                if let Expr::Local(local) = self.ir.node(d.body).expr
                    && local.up == 0
                    && let Some(&root) = roots.get(&local.slot)
                {
                    roots.insert(slot, root);
                }
            }
        }
        if roots.len() != defs.len() || !self.safe_literal_uses(body, &roots, false, 0) {
            return;
        }
        for root in roots.values() {
            self.admit_literal(*root);
        }
        self.retain_local_queries(body, &roots);
    }

    fn retain_local_queries(&mut self, e: ExprId, roots: &HashMap<Slot, ExprId>) {
        let expr = self.ir.node(e).expr.clone();
        if let Expr::Builtin {
            op: BuiltinOp::In | BuiltinOp::NotIn,
            args,
        } = &expr
            && args.len() == 2
            && self.local_literal_operand(args[1], roots)
        {
            self.retain_literal_tuples(args[0]);
        }
        for p in expr.children() {
            self.retain_local_queries(p, roots);
        }
    }

    /// Called only for value-valued check bodies, not state constructors,
    /// VIEW, imports or generator bindings. Unknown uses retain the old path.
    pub(super) fn select_literal_history(&mut self, e: ExprId) {
        self.select_literal_history_at(e, 0);
    }

    fn select_literal_history_at(&mut self, e: ExprId, depth: usize) {
        if depth > 64 {
            return;
        }
        let expr = self.ir.node(e).expr.clone();
        match &expr {
            Expr::Builtin {
                op: BuiltinOp::In | BuiltinOp::NotIn,
                args,
            } if args.len() == 2 => {
                if self.literal_tree(args[0], 0) && self.literal_constructor(args[1]) {
                    self.admit_literal(args[1]);
                    self.retain_literal_tuples(args[0]);
                }
            }
            Expr::Apply { callee, args } if self.is_literal_cardinality(callee) => {
                if let [Arg::Expr { expr, .. }] = &args[..]
                    && self.literal_constructor(*expr)
                {
                    self.admit_literal(*expr);
                }
            }
            Expr::Let { defs, body } => self.select_local_literals(defs, *body),
            _ => {}
        }
        for p in expr.children() {
            self.select_literal_history_at(p, depth + 1);
        }
    }
}
