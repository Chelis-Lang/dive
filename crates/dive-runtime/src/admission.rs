//! Pure admission analysis of a bound model, shared by graph capture and
//! Core. Nothing here evaluates an
//! expression.

use std::sync::Arc;

use dive_config::ModelConfig;
use dive_ir::Ir;
use dive_model::Model;

use crate::CheckOptions;

/// Whether the specification refers to a random operator, so that expanded
/// states need their own random stream.
fn random_op(op: dive_ir::StdOp) -> bool {
    use dive_ir::StdOp;
    matches!(
        op,
        StdOp::TlcRandomElement
            | StdOp::RandomizationRandomSubset
            | StdOp::RandomizationRandomSetOfSubsets
            | StdOp::RandomizationRandomSubsetSet
    )
}

/// Whether a bound model can draw a random value after start-up: whether a
/// definition outside the embedded standard modules, or a node that belongs to
/// no definition, can reach a random operator through the definitions it
/// references. A standard module that merely defines a random helper, such as
/// `Randomization`'s `TestRandomSetOfSubsets`, does not make a specification
/// that never calls it random. Any reference counts, used or not, so an unused
/// random helper in a user module still does. A native definition itself is
/// not a reference: `INSTANCE` of a module that extends `TLC` copies
/// `RandomElement` into the instantiating module without calling it.
///
/// A definition whose value the binder fixed at
/// start-up (a precomputed constant-level definition, or a configured
/// `Op = v` value) is never evaluated again, so neither its body nor what
/// it references can draw. The start-up draws themselves come from the run's
/// reported seed. A configured redirect (`Op <- D`, `C <- D`)
/// evaluates `D`'s body at every use, so it references `D`, and `D` is not
/// fixed ([`dive_eval::Evaluator::startup_fixed`]).
#[must_use]
pub fn uses_random_after_startup(model: &Model) -> bool {
    let ev = &model.evaluator;
    uses_random_except(
        &model.ir,
        |d| ev.startup_fixed(d),
        |d| Some(ev.redirected(d)).filter(|&t| t != d),
        |c| match ev.constant_binding(c) {
            Some(dive_eval::ConstBinding::Def(d)) => Some(ev.redirected(d)),
            _ => None,
        },
    )
}

/// Whether the specification can reach a random operator, treating each
/// definition `fixed` accepts as a value that
/// is never evaluated again, each definition as referencing its `redirect`
/// target, and each constant as referencing its bound definition.
fn uses_random_except(
    ir: &dive_ir::Ir,
    fixed: impl Fn(dive_ir::DefId) -> bool,
    redirect: impl Fn(dive_ir::DefId) -> Option<dive_ir::DefId>,
    constant: impl Fn(dive_ir::ConstId) -> Option<dive_ir::DefId>,
) -> bool {
    use dive_ir::{Expr, OpRef};
    let defs = ir.defs();
    let ids = || (0..defs.len()).map(|i| u32::try_from(i).map(dive_ir::DefId));
    let fixed: Vec<bool> = ids().map(|d| d.is_ok_and(&fixed)).collect();
    let mut owned = vec![false; ir.nodes().len()];
    // Direct references of each definition's body subtree.
    let mut refs: Vec<Vec<usize>> = Vec::with_capacity(defs.len());
    // Whether a definition's own body calls a random native operator.
    let mut direct = vec![false; defs.len()];
    let referenced = |e: &Expr, out: &mut Vec<usize>| -> bool {
        let mut random = false;
        if let Expr::Def(d) = e {
            out.push(d.index());
        }
        if let Expr::Const(c) = e {
            out.extend(constant(*c).map(dive_ir::DefId::index));
        }
        for r in e.op_refs() {
            match r {
                OpRef::Std(op) => random |= random_op(*op),
                OpRef::Def(d) => out.push(d.index()),
                OpRef::Const(c) => out.extend(constant(*c).map(dive_ir::DefId::index)),
                _ => {}
            }
        }
        random
    };
    for (i, def) in defs.iter().enumerate() {
        let mut out = Vec::new();
        let mut stack = vec![def.body];
        while let Some(e) = stack.pop() {
            let Some(slot) = owned.get_mut(e.index()) else {
                // A malformed reference: keep the conservative answer.
                return true;
            };
            *slot = true;
            let node = ir.node(e);
            direct[i] |= referenced(&node.expr, &mut out);
            stack.extend(node.expr.children());
        }
        // A fixed value's body is owned, but never evaluated again.
        if fixed[i] {
            direct[i] = false;
            out.clear();
        }
        // `Op <- D`: a use of `Op` evaluates `D`.
        if let Ok(d) = u32::try_from(i) {
            out.extend(redirect(dive_ir::DefId(d)).map(dive_ir::DefId::index));
        }
        refs.push(out);
    }
    // Close over references until no definition changes. Natives seed it.
    let mut random: Vec<bool> = defs
        .iter()
        .zip(direct.iter().zip(&fixed))
        .map(|(def, (direct, fixed))| *direct || (!fixed && def.std.is_some_and(random_op)))
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..defs.len() {
            if !random[i]
                && refs[i]
                    .iter()
                    .any(|&d| random.get(d).copied().unwrap_or(true))
            {
                random[i] = true;
                changed = true;
            }
        }
    }
    let reaches = |out: &[usize]| out.iter().any(|&d| random.get(d).copied().unwrap_or(true));
    let user = defs
        .iter()
        .zip(direct.iter().zip(&refs))
        .any(|(def, (direct, out))| !ir.module(def.module).standard && (*direct || reaches(out)));
    user || ir.nodes().iter().zip(&owned).any(|(node, owned)| {
        let mut out = Vec::new();
        !owned && (referenced(&node.expr, &mut out) || reaches(&out))
    })
}

/// Whether a configured constant binding or redirect of the bound model
/// targets a random native operator.
///
/// A supplied bound model no longer carries its source cfg. Inspect admitted
/// constant bindings and actual redirects without invoking any expression or
/// lazy callback. Unused native imports alone must not refuse pure captures.
///
/// # Errors
/// The model's context or a configured definition is invalid.
pub fn bound_configured_random(model: &Model) -> Result<bool, String> {
    // The shared capture entry admission already verifies full model context.
    // Keep the helper defensive against any future caller as well.
    model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .map_err(|e| e.to_string())?;
    let native = |id: dive_ir::DefId| -> Result<bool, String> {
        if model.ir.defs().get(id.index()).is_none() {
            return Err("DIVE-EVAL-C001: invalid configured native definition".into());
        }
        let id = model.evaluator.redirected(id);
        let def = model
            .ir
            .defs()
            .get(id.index())
            .ok_or_else(|| "DIVE-EVAL-C001: invalid redirected native definition".to_owned())?;
        Ok(def.std.is_some_and(random_op))
    };
    for (index, _) in model.ir.consts().iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| "DIVE-EVAL-C005: constant index overflow".to_owned())?;
        if let Some(dive_eval::ConstBinding::Def(id)) =
            model.evaluator.constant_binding(dive_ir::ConstId(index))
            && native(id)?
        {
            return Ok(true);
        }
    }
    for (index, _) in model.ir.defs().iter().enumerate() {
        let index = u32::try_from(index)
            .map_err(|_| "DIVE-EVAL-C005: definition index overflow".to_owned())?;
        let id = dive_ir::DefId(index);
        if model.evaluator.redirected(id) != id && native(id)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Whether evaluating `model` may have effects beyond computing a value:
/// `Print`, `TLCSet`, a live `TLCGet`, randomness, I/O or checker
/// introspection. Conservative: any definition of a user module counts,
/// whether or not it is reached, and so does a constant or definition the
/// configuration binds to such a standard operator. Caller-supplied predicate
/// and action scopes are checked for actual captured effects as well. Backends
/// that cannot reproduce TLC's evaluation order for effects refuse such models.
#[must_use]
pub fn effectful(model: &dive_model::Model) -> bool {
    use dive_ir::{Effects, StdSupport};
    let ir = &model.ir;
    let forbidden = Effects::IMPURE.union(Effects::LIVE_TLCGET);
    let native = |def: dive_ir::DefId| {
        let def = model.evaluator.redirected(def);
        ir.defs()
            .get(def.index())
            .and_then(|d| d.std)
            .is_some_and(|op| {
                op.impure() || matches!(op.support(), StdSupport::Io | StdSupport::Introspection)
            })
    };
    let user = ir.defs().iter().enumerate().any(|(i, def)| {
        let id = dive_ir::DefId(u32::try_from(i).unwrap_or(u32::MAX));
        (!ir.module(def.module).standard && ir.node(def.body).effects.intersects(forbidden))
            || (model.evaluator.redirected(id) != id && native(id))
    });
    if user || (0..ir.consts().len()).any(|i| {
        let id = dive_ir::ConstId(u32::try_from(i).unwrap_or(u32::MAX));
        matches!(model.evaluator.constant_binding(id), Some(dive_eval::ConstBinding::Def(d)) if native(d))
    }) {
        return true;
    }
    let predicates = model
        .init
        .iter()
        .map(|init| &init.pred)
        .chain(model.next.iter())
        .chain(model.invariants.iter().map(|named| &named.pred))
        .chain(model.implied_inits.iter().map(|named| &named.pred))
        .chain(model.implied_actions.iter().map(|named| &named.pred))
        .chain(model.temporals.iter().map(|named| &named.pred))
        .chain(model.implied_temporals.iter().map(|named| &named.pred))
        .chain(model.configured_properties.iter().map(|named| &named.pred))
        .chain(model.constraints.iter().map(|named| &named.pred))
        .chain(model.action_constraints.iter().map(|named| &named.pred))
        .chain(model.view.iter().map(|named| &named.pred))
        .chain(
            model
                .alias
                .as_ref()
                .and_then(|alias| alias.as_ref().ok())
                .map(|named| &named.pred),
        )
        .chain(
            model
                .postconditions
                .as_ref()
                .into_iter()
                .flatten()
                .map(|named| &named.pred),
        );
    predicates
        .into_iter()
        .any(|pred| model.evaluator.scope_has_effects(&pred.scope))
        || model
            .init_actions
            .iter()
            .chain(&model.actions)
            .any(|action| model.evaluator.scope_has_effects(&action.scope()))
}

/// Why an ordinary request with temporal properties cannot use the temporal
/// route, or `None` when it can. Callers that refuse such a request name this
/// reason, so the user knows which option to drop.
///
/// ```
/// # fn example(ir: &std::sync::Arc<dive_ir::Ir>, cfg: &dive_config::ModelConfig) {
/// let mut opts = dive_runtime::CheckOptions::tlc(0, 0);
/// opts.chaos = Some(0);
/// assert_eq!(
///     dive_runtime::admission::route_exclusion(ir, cfg, &opts),
///     Some("--chaos")
/// );
/// # }
/// ```
#[must_use]
pub fn route_exclusion(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    opts: &CheckOptions,
) -> Option<&'static str> {
    let mut baseline = match opts.profile {
        dive_eval::Profile::Dive => CheckOptions::dive(opts.seed),
        dive_eval::Profile::Tlc => CheckOptions::tlc(0, opts.seed),
    };
    if opts.profile == dive_eval::Profile::Tlc
        && opts.eval.action_composition == dive_eval::ActionComposition::Tlc
    {
        baseline.eval.action_composition = dive_eval::ActionComposition::Tlc;
    }
    // The seen-state store, fingerprint polynomial, deadlock switch and
    // sequential request are honored by both continuations: the safety engine
    // reads them, and temporal capture retains concrete states with its own
    // deadlock check and worker plan.
    if opts.candidate_memory_bytes.is_some() || opts.candidate_directory.is_some() {
        Some("candidate spill")
    } else if opts.seen_disk.is_some() {
        Some("disk seen-state storage")
    } else if opts.frontier_memory_bytes.is_some() || opts.frontier_directory.is_some() {
        Some("frontier spill")
    } else if opts.predecessor_memory_bytes.is_some() || opts.predecessor_directory.is_some() {
        Some("predecessor spill")
    } else if opts.eval != baseline.eval {
        Some("evaluation options other than the profile defaults")
    } else if opts.chaos != baseline.chaos {
        Some("--chaos")
    } else if dive_eval::validate_ir(ir).is_err() {
        Some("an IR the evaluator does not accept")
    } else if dive_model::transformed_randomness(ir, cfg).is_some() {
        Some("randomness combined with VIEW or SYMMETRY")
    } else {
        None
    }
}
