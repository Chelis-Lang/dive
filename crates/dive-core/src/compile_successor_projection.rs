//! Static read and write sets of complete generative branches.
use super::{Compiler, G, GNode, P};
use crate::plan::{SuccProjection, Unch};

impl Compiler<'_> {
    pub(super) fn successor_projections(&self) -> Vec<Option<SuccProjection>> {
        let all = u64::MAX
            .checked_shr(64u32.saturating_sub(self.ir.vars().len() as u32))
            .unwrap_or(0);
        let mut plans = Vec::with_capacity(self.code.gnodes.len());
        for node in &self.code.gnodes {
            plans.push(self.successor_projection(node, all, &plans));
        }
        plans
    }
    fn successor_projection(
        &self,
        node: &GNode,
        all: u64,
        plans: &[Option<SuccProjection>],
    ) -> Option<SuccProjection> {
        let child = |g: G| plans.get(g as usize)?.clone();
        let mut s = SuccProjection::default();
        let val = |s: &mut SuccProjection, p: P| {
            let v = self.info.get(p as usize)?;
            if v.calls || v.effect || v.nested_query {
                return None;
            }
            s.read |= v.vars | v.primed;
            for &k in &v.free {
                if !s.slots.contains(&k) {
                    s.slots.push(k);
                }
            }
            Some(())
        };
        match node {
            GNode::Conj(items) | GNode::LabelArm(items, _) => {
                s.block = items.len() > 1;
                for &(g, p) in items {
                    if s.write | s.copy == all {
                        val(&mut s, p)?;
                    }
                    s.merge(&child(g)?, true);
                }
            }
            GNode::Disj(gs) => {
                for &g in gs {
                    s.merge(&child(g)?, false);
                }
            }
            GNode::Scope { body, .. } => s = child(*body)?,
            GNode::Assign { var, rhs } => {
                val(&mut s, *rhs)?;
                s.write = 1u64.checked_shl((*var).into())?;
            }
            GNode::Guard(p) | GNode::LabelGuard(p, _) => {
                val(&mut s, *p)?;
            }
            GNode::Unchanged(items) => {
                for u in items {
                    let Unch::Var { var, .. } = u else {
                        return None;
                    };
                    let bit = 1u64.checked_shl((*var).into())?;
                    s.read |= s.copy & bit;
                    s.copy |= bit;
                }
            }
            GNode::If { cond, then, other } => {
                val(&mut s, *cond)?;
                s.merge(&child(*then)?, false);
                s.merge(&child(*other)?, false);
            }
            GNode::Implies { lhs, rhs } => {
                val(&mut s, *lhs)?;
                s.merge(&child(*rhs)?, false);
            }
            GNode::Exists { binds, body, .. } => {
                s = child(*body)?;
                for bind in binds.iter().rev() {
                    s.slots.retain(|v| !bind.slots.contains(v));
                    val(&mut s, bind.domain)?;
                    if let Some(p) = bind.eligible {
                        val(&mut s, p)?;
                    }
                }
            }
            // A \\A can revisit the same writes and all-assigned boolean paths.
            _ => return None,
        }
        s.slots.sort_unstable();
        Some(s)
    }
}
