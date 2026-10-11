//! Callback-free conservative pre-bind admission for the final-only facade.
//! Integrates with the shared fallible IR/context authority in the shipping tree.
use dive_config::ModelConfig;
use dive_ir::{DefScope, Effects, Ir, OpRef, StdOp, StdSupport};
use std::collections::BTreeSet;
fn native(op: StdOp) -> bool {
    !op.impure()
        && !op.creates_runtime_names()
        && !matches!(op.support(), StdSupport::Io | StdSupport::Introspection)
        && !matches!(
            op,
            StdOp::TlcTLCGet | StdOp::TlcTLCSet | StdOp::TlcExtTLCGetOrDefault
        )
}
#[allow(
    clippy::too_many_lines,
    reason = "review callback-free config/native authority admission together"
)]
pub(super) fn check(ir: &Ir, cfg: &ModelConfig, maximum: usize) -> Result<(), &'static str> {
    if [
        ir.nodes().len(),
        ir.defs().len(),
        ir.decls().len(),
        ir.modules().len(),
    ]
    .into_iter()
    .any(|n| n > maximum)
    {
        return Err("pre-bind IR arena entries");
    }
    let mut targets: BTreeSet<&str> = cfg
        .constants
        .iter()
        .filter_map(|c| match c {
            dive_config::ConstantEntry::Substitution { target, .. } => Some(target.text.as_str()),
            dive_config::ConstantEntry::Assignment { .. } => None,
        })
        .collect();
    for name in [
        &cfg.init,
        &cfg.next,
        &cfg.specification,
        &cfg.view,
        &cfg.symmetry,
        &cfg.alias,
        &cfg.periodic,
        &cfg.rl_reward,
    ]
    .into_iter()
    .flatten()
    {
        targets.insert(name.text.as_str());
    }
    for names in [
        &cfg.invariants,
        &cfg.properties,
        &cfg.constraints,
        &cfg.action_constraints,
        &cfg.postconditions,
        &cfg.possible,
    ] {
        targets.extend(names.iter().map(|n| n.text.as_str()));
    }
    let mut roots = Vec::new();
    for (i, d) in ir.defs().iter().enumerate() {
        // Even unused nonnative definitions can be eagerly evaluated at startup.
        // Native documentation bodies cannot execute through genuine Def.std.
        if (d.std.is_none()
            && (!ir.module(d.module).standard
                || (d.params.is_empty() && d.scope == DefScope::TopLevel)))
            || targets.contains(d.name.as_str())
        {
            roots.push(dive_ir::DefId(
                u32::try_from(i).map_err(|_| "pre-bind definition index")?,
            ));
        }
    }
    // Binding resolves configuration names through module contexts. A named
    // INSTANCE/alias can point at a definition whose internal name differs.
    // Root every matching context candidate before any eager substitution work;
    // textual Def.name matching alone is not resolved-target authority.
    for module in ir.modules() {
        for (name, symbol) in &module.context {
            if targets.contains(name.as_str())
                && let dive_ir::Symbol::Def(def) = symbol
            {
                roots.push(*def);
            }
        }
    }
    let reached = ir.reachable_defs(&roots);
    let mut seen = BTreeSet::new();
    let mut work = 0usize;
    for (i, d) in ir.defs().iter().enumerate() {
        if !reached[i] {
            continue;
        }
        if d.effects
            .intersects(Effects::IMPURE.union(Effects::LIVE_TLCGET))
        {
            return Err("pre-bind impure/live ASSUME/constant/user definition");
        }
        if let Some(op) = d.std {
            if !native(op) {
                return Err("pre-bind native replacement/role effects");
            }
            continue;
        }
        let mut pending = vec![d.body];
        while let Some(id) = pending.pop() {
            work = work
                .checked_add(1)
                .filter(|n| *n <= maximum)
                .ok_or("pre-bind expression inspections")?;
            if !seen.insert(id) {
                continue;
            }
            for op in ir.expr(id).op_refs() {
                match op {
                    OpRef::Std(op) if !native(*op) => {
                        return Err("pre-bind native callback/runtime-name effects");
                    }
                    OpRef::Unbound(_) => {
                        return Err("pre-bind opaque operator capture");
                    }
                    _ => {}
                }
            }
            pending.extend(ir.expr(id).children());
        }
    }
    Ok(())
}
