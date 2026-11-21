//! Split existential witnesses bind values in a fresh frame at execution.
//! Given the same expression and ordered slot layout, their pure value
//! instructions use identical frame offsets. Keep generator occurrences
//! and guard memo sites separate so their labels, counters and cache
//! histories still belong to individual actions.

use std::collections::HashMap;
use std::ops::Range;

use super::{
    Action, Compiler, DefId, Error, ExprId, FOLD_BUDGET, FOLD_FUEL, G, GNode, Node, Role, Slot,
};

struct SplitBody {
    generators: Range<usize>,
    /// Logical fold budget on entry and exit of the original compilation.
    /// A fold normally gets `FOLD_FUEL`, capped by the remaining budget.
    fold_before: u64,
    fold_after: u64,
    root: G,
    frame: u32,
}

impl SplitBody {
    fn fold_cost(&self) -> u64 {
        self.fold_before - self.fold_after
    }

    fn folds_fit(&self, remaining: u64) -> bool {
        remaining == self.fold_before
            || (self.fold_after >= FOLD_FUEL && remaining >= self.fold_cost() + FOLD_FUEL)
    }
}

impl Compiler<'_> {
    pub(super) fn remaining_fold_fuel(&self) -> u64 {
        FOLD_BUDGET.saturating_sub(self.stats.fold_fuel + self.shared_fold_fuel)
    }

    pub(super) fn next_actions(&mut self, roles: &[Role]) -> Result<Vec<Action>, Error> {
        let mut bodies: HashMap<(DefId, ExprId, Vec<Slot>), Option<SplitBody>> = HashMap::new();
        let mut actions = Vec::with_capacity(roles.len());
        let mut compiler_context = (self.def_slots.len(), self.settled.len());
        for role in roles {
            // These two compiler tables grow monotonically during role compilation.
            // apply_value uses definition thunk offsets to select eager application
            // or beta reduction. A settled constant can change the slot allocation
            // of a later occurrence. Recompile when either table grows.
            let current = (self.def_slots.len(), self.settled.len());
            if current != compiler_context {
                bodies.clear();
                compiler_context = current;
            }
            if role.slots.is_empty() || !role.substs.is_empty() || self.code.source_map.is_some() {
                actions.push(self.action(role, false)?);
                continue;
            }
            let key = (
                role.def,
                role.expr,
                role.slots.iter().map(|(s, _)| *s).collect(),
            );
            let fold_before = self.remaining_fold_fuel();
            match bodies.get(&key) {
                Some(Some(body)) if body.folds_fit(fold_before) => {
                    actions.push(self.instantiate_split(role, body)?);
                    continue;
                }
                Some(None) => {
                    actions.push(self.action(role, false)?);
                    continue;
                }
                _ => {}
            }
            let values = self.info.len();
            let generators = self.code.gnodes.len();
            let sites = self.code.sites.len();
            let compositions = self.compositions.len();
            let action = self.action(role, false)?;
            // A compiled call or a nested query owns another frame. Composition
            // patches its generator's resets after compilation. A step memo is
            // embedded in a value expression, unlike a direct generator guard.
            let share = self.compositions.len() == compositions
                && self.info[values..]
                    .iter()
                    .all(|i| !i.effect && !i.calls && !i.nested_query)
                && self.code.sites[sites..].iter().all(|s| !s.step);
            bodies.insert(
                key,
                share.then_some(SplitBody {
                    generators: generators..self.code.gnodes.len(),
                    fold_before,
                    fold_after: self.remaining_fold_fuel(),
                    root: action.g,
                    frame: action.frame,
                }),
            );
            actions.push(action);
        }
        Ok(actions)
    }

    fn instantiate_split(&mut self, role: &Role, body: &SplitBody) -> Result<Action, Error> {
        // Convert this occurrence's actual values in their original order,
        // including any conversion error, before executing the shared code.
        let (_, preset) = self.role_env(role)?;
        // Preserve the point at which later occurrences cease folding. Only
        // real work goes into the published compilation statistics.
        self.shared_fold_fuel += body.fold_cost();
        let next = self.code.gnodes.len();
        G::try_from(next.saturating_add(body.generators.len()))
            .map_err(|_| Error::unsupported("a plan larger than the generator index limit"))?;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "checked the full batch above"
        )]
        let offset = (next - body.generators.start) as G;
        for index in body.generators.clone() {
            let mut node = self.code.gnodes[index].clone();
            if let GNode::Guard(p) = node
                && let Node::Memo { body, .. } = self.code.nodes[p as usize]
            {
                self.guard(body)?;
            } else {
                shift_children(&mut node, offset);
                self.gpush(node);
            }
        }
        Ok(Action {
            name: role.name.clone(),
            g: body.root + offset,
            frame: body.frame,
            preset,
        })
    }
}

/// Generator compilation appends a closed batch of generator nodes for
/// each role. Only generator edges move. Value and flat slot indices
/// still refer to the original instructions and their frame layout.
fn shift_children(node: &mut GNode, offset: G) {
    match node {
        GNode::Composition {
            first: then,
            then: other,
            ..
        }
        | GNode::If { then, other, .. } => {
            *then += offset;
            *other += offset;
        }
        GNode::Conj(parts) => {
            for (g, _) in parts {
                *g += offset;
            }
        }
        GNode::Disj(parts) => {
            for g in parts {
                *g += offset;
            }
        }
        GNode::Case { arms, other } => {
            for (_, g) in arms {
                *g += offset;
            }
            if let Some(g) = other {
                *g += offset;
            }
        }
        GNode::Exists { body, .. }
        | GNode::Forall { body, .. }
        | GNode::FcnAction { body, .. }
        | GNode::Scope { body, .. }
        | GNode::Implies { rhs: body, .. }
        | GNode::StepBox { action: body, .. }
        | GNode::StepAngle { action: body, .. } => *body += offset,
        GNode::Guard(_)
        | GNode::Assign { .. }
        | GNode::AssignIn { .. }
        | GNode::AssignInRaw { .. }
        | GNode::Unchanged(_) => {}
    }
}
