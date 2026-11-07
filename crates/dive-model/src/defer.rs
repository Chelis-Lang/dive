//! Start-up constant definitions the binder may leave to the core engine.
//!
//! TLC evaluates every constant-level zero-arity definition at start-up
//! (`SpecProcessor.processConstantDefns`) and swallows the failures. For a
//! pure definition that evaluation is unobservable except through the value
//! it leaves behind, and only where the binder itself looks at that value:
//! where it splits the behavior specification, the initial predicate or the
//! next-state relation and finds a definition in a role position, and where
//! the reference evaluator evaluates something after binding on the core
//! route (`ASSUME`, `ALIAS`, `VIEW`, `SYMMETRY`, `POSTCONDITION` and the
//! configuration's substitutions). A definition used only as a value by the
//! roles the core compiles is therefore evaluated by the core instead, once,
//! when it compiles the plan (issue #846).
//!
//! A pure `ASSUME` of the root module is a value too: TLC's
//! `Tool.checkAssumptions` evaluates it once, and only its truth is
//! observable. The core evaluates such an `ASSUME` when it compiles the
//! plan, and the reference evaluator checks only those the core does not
//! find `TRUE`, so the definitions it reaches may be left to the core.

use std::collections::HashSet;

use dive_ir::{Arg, DefId, Expr, ExprId, OpRef, QuantKind};

use crate::bind::Binder;
use crate::tables::Look;

/// What the second phase of `processConstantDefns` leaves to the core.
pub(crate) struct Deferral {
    /// The pure startup definitions the binder does not evaluate: never in a
    /// role position, not reached by anything the reference evaluator
    /// evaluates on the core route or by a definition the binder does
    /// evaluate, and not named by the configuration.
    pub skip: HashSet<DefId>,
    /// Those a configured role reaches, which the core evaluates; the
    /// others are evaluated by no one, which is unobservable, since TLC
    /// swallows the failure of a pure startup evaluation.
    pub evaluate: HashSet<DefId>,
    /// The leading pure `ASSUME`s of the root module, in order, which the
    /// core evaluates before the reference evaluator checks the others.
    pub assumptions: Vec<DefId>,
}

pub(crate) fn candidates(b: &Binder<'_>) -> Deferral {
    let cfg = b.cfg;
    let mut named: HashSet<DefId> = HashSet::new();
    let mut roles = Vec::new();
    let mut reference = Vec::new();
    let look = |name: &str, out: &mut Vec<DefId>, named: &mut HashSet<DefId>| {
        resolve(b, name, out, named);
    };
    let role_names = cfg
        .init
        .iter()
        .chain(cfg.next.iter())
        .chain(cfg.specification.iter())
        .chain(cfg.invariants.iter())
        .chain(cfg.properties.iter())
        .chain(cfg.constraints.iter())
        .chain(cfg.action_constraints.iter());
    for n in role_names {
        look(&n.text, &mut roles, &mut named);
    }
    let reference_names = cfg
        .view
        .iter()
        .chain(cfg.symmetry.iter())
        .chain(cfg.alias.iter())
        .chain(cfg.periodic.iter())
        .chain(cfg.rl_reward.iter())
        .chain(cfg.postconditions.iter())
        .chain(cfg.possible.iter());
    for n in reference_names {
        look(&n.text, &mut reference, &mut named);
    }
    for e in &cfg.constants {
        match e {
            // A replaced definition is not evaluated; its replacement is,
            // wherever the definition is used (the walk follows the
            // replacement), and a constant's replacement is a reference
            // root below.
            dive_config::ConstantEntry::Substitution { name, target, .. } => {
                look(&name.text, &mut Vec::new(), &mut named);
                look(&target.text, &mut Vec::new(), &mut named);
            }
            dive_config::ConstantEntry::Assignment { name, .. } => {
                look(&name.text, &mut Vec::new(), &mut named);
            }
        }
    }
    let ir = &*b.t.ir;
    let assumptions = pure_assumptions(b);
    for m in 0..ir.modules().len() {
        let module = ir.module(dive_ir::ModuleId(u32::try_from(m).unwrap_or(u32::MAX)));
        reference.extend(
            module
                .assumptions
                .iter()
                .copied()
                .filter(|d| !assumptions.contains(d)),
        );
    }
    // Constants the configuration binds to definitions: their targets are
    // evaluated at start-up by the reference evaluator.
    for c in 0..ir.consts().len() {
        if let Some(Look::Def(d) | Look::Java(_, d)) =
            b.t.const_look(dive_ir::ConstId(u32::try_from(c).unwrap_or(u32::MAX)))
        {
            reference.push(d);
            named.insert(d);
        }
    }
    let by_reference = Walk::run(b, &reference, false, false);
    let by_roles = Walk::run(b, &roles, true, false);
    let by_assumptions = Walk::run(b, &assumptions, false, false);
    // What the binder evaluates at start-up must not reach a deferred
    // definition: the reference evaluator would evaluate it lazily there,
    // once per use. Removing a candidate makes it one of those, so this
    // runs to a fixed point.
    let startup = startup_defs(b);
    let mut deferred: HashSet<DefId> = startup
        .iter()
        .copied()
        .filter(|d| {
            !by_roles.position.contains(d)
                && !by_reference.value.contains(d)
                && !by_reference.position.contains(d)
                && !named.contains(d)
                && crate::reach::pure(b, *d)
        })
        .collect();
    loop {
        let precomputed: Vec<DefId> = startup
            .iter()
            .copied()
            .filter(|d| !deferred.contains(d))
            .collect();
        // Constant-level definitions: an instance's substitutions for its
        // variables are never evaluated there.
        let reached = Walk::run(b, &precomputed, false, true);
        let before = deferred.len();
        deferred.retain(|d| !reached.value.contains(d) && !reached.position.contains(d));
        if deferred.len() == before {
            let evaluate = deferred
                .iter()
                .copied()
                .filter(|d| by_roles.value.contains(d) || by_assumptions.value.contains(d))
                .collect();
            return Deferral {
                skip: deferred,
                evaluate,
                assumptions,
            };
        }
    }
}

/// The leading run of the root module's `ASSUME`s (the ones TLC checks,
/// `AXIOM`s aside) that apply no impure operator, in order: the core
/// evaluates them. TLC's `Tool.checkAssumptions` stops at the first `ASSUME`
/// that is not `TRUE`, so an `ASSUME` may guard the ones after it. The core
/// evaluates its `ASSUME`s before the reference evaluator checks the others,
/// so it takes only those no impure `ASSUME` precedes, and stops at the first
/// that is not `TRUE` itself.
fn pure_assumptions(b: &Binder<'_>) -> Vec<DefId> {
    let ir = &*b.t.ir;
    let root = ir.module(ir.root());
    root.assumptions
        .iter()
        .zip(root.axioms.iter().chain(std::iter::repeat(&false)))
        .filter(|(_, axiom)| !**axiom)
        .map(|(d, _)| *d)
        .take_while(|d| crate::reach::pure(b, *d))
        .collect()
}

/// The definitions the reference evaluator may evaluate after binding on the
/// temporal route: those the temporal properties, the behavior
/// specification's fairness and the configured properties reach, including
/// through the arguments their scopes bind lazily. The liveness checker and
/// its validation evaluate these, so the binder evaluates the start-up
/// definitions among them, as TLC does; the core keeps the others.
pub(crate) fn reference_temporal(b: &Binder<'_>, model: &crate::Model) -> HashSet<DefId> {
    let mut roots = std::collections::BTreeSet::new();
    let mut exprs = Vec::new();
    for named in model
        .temporals
        .iter()
        .chain(&model.implied_temporals)
        .chain(&model.configured_properties)
    {
        crate::reach::role_defs(
            b,
            named.pred.expr,
            &named.pred.scope,
            &mut roots,
            &mut exprs,
            0,
        );
    }
    let roots: Vec<DefId> = roots.into_iter().collect();
    let walk = Walk::run(b, &roots, false, false);
    walk.value.into_iter().chain(walk.position).collect()
}

/// The definitions the second phase of `processConstantDefns` evaluates:
/// the zero-arity constant-level definitions of the modules it processes,
/// after the configuration's overrides. A definition whose level cannot be
/// computed counts as one.
fn startup_defs(b: &Binder<'_>) -> HashSet<DefId> {
    let ir = &*b.t.ir;
    let mut out = HashSet::new();
    let mut modules = crate::walk::file_modules(ir);
    while let Some(m) = modules.pop() {
        modules.extend(crate::walk::inner_modules(ir, m));
        if !b.processes_constant_defns(m) {
            continue;
        }
        for (_, d) in b.t.op_defs(m) {
            let def = ir.def(d);
            if !def.params.is_empty() || !b.processes_constant_defns(def.module) {
                continue;
            }
            let real = match b.t.tool_obj.get(&d) {
                Some(Look::Def(r)) => *r,
                Some(_) => continue,
                None => match b.t.lookup_def(d) {
                    Look::Def(r) => r,
                    Look::Value(_) | Look::Java(..) | Look::Literal(_) => continue,
                },
            };
            let level = b.ev().scope(real).ok().and_then(|scope| {
                crate::level::LevelCx::new(&b.t)
                    .try_level(ir.def(real).body, &scope)
                    .ok()
            });
            if level.is_none_or(|l| l == 0) {
                out.insert(real);
            }
        }
    }
    out
}

/// The definitions a configuration name may denote.
fn resolve(b: &Binder<'_>, name: &str, out: &mut Vec<DefId>, named: &mut HashSet<DefId>) {
    if let Some(Look::Def(d) | Look::Java(_, d)) = b.defns.get(name) {
        out.push(*d);
        named.insert(*d);
    }
    // Configuration names resolve in the root module; a name that does not
    // resolve there stays precomputed wherever it is defined.
    for (n, d) in b.t.op_defs(b.t.ir.root()) {
        if n == name {
            out.push(d);
            named.insert(d);
        }
    }
}

/// The definitions reached from some roots, split by the context they are
/// reached in: a role position (where the binder may look at a start-up
/// value) or a value position (where only the value matters).
#[derive(Default)]
struct Walk {
    position: HashSet<DefId>,
    value: HashSet<DefId>,
}

impl Walk {
    /// With `constant`, the roots are constant-level, and the walk does not
    /// follow an instance's substitutions for variables.
    fn run(b: &Binder<'_>, roots: &[DefId], position: bool, constant: bool) -> Walk {
        let mut w = Walk::default();
        let mut work: Vec<(ExprId, bool)> = Vec::new();
        for &d in roots {
            w.def(b, d, position, &mut work);
        }
        let ir = &*b.t.ir;
        while let Some((e, pos)) = work.pop() {
            let expr = ir.expr(e);
            // The children of a value-making expression are values.
            let inner = pos && !value_making(expr);
            if constant && let Expr::Subst { bindings, body } = expr {
                work.push((*body, inner));
                for binding in bindings {
                    if ir.decl(binding.decl).variable {
                        continue;
                    }
                    match &binding.value {
                        Arg::Expr { expr, .. } => work.push((*expr, inner)),
                        Arg::Op {
                            op: OpRef::Def(d), ..
                        } => w.def(b, *d, inner, &mut work),
                        Arg::Op { .. } => {}
                    }
                }
                continue;
            }
            match expr {
                Expr::Def(d) => w.def(b, *d, pos, &mut work),
                Expr::Const(c) => {
                    if let Some(Look::Def(d) | Look::Java(_, d)) = b.t.const_look(*c) {
                        w.def(b, d, pos, &mut work);
                    }
                }
                _ => {}
            }
            for r in expr.op_refs() {
                match r {
                    OpRef::Def(d) => w.def(b, *d, inner, &mut work),
                    OpRef::Const(c) => {
                        if let Some(Look::Def(d) | Look::Java(_, d)) = b.t.const_look(*c) {
                            w.def(b, d, inner, &mut work);
                        }
                    }
                    _ => {}
                }
            }
            if let Expr::Apply { args, .. } = expr {
                for a in args {
                    if let Arg::Expr { expr, .. } = a {
                        work.push((*expr, inner));
                    }
                }
            } else {
                expr.for_each_child(|child| work.push((child, inner)));
            }
        }
        w
    }

    fn def(&mut self, b: &Binder<'_>, d: DefId, position: bool, work: &mut Vec<(ExprId, bool)>) {
        let set = if position {
            &mut self.position
        } else {
            &mut self.value
        };
        if !set.insert(d) {
            return;
        }
        let ir = &*b.t.ir;
        work.push((ir.def(d).body, position));
        if let Look::Def(r) | Look::Java(_, r) = b.t.lookup_def(d)
            && r != d
        {
            self.def(b, r, position, work);
        }
    }
}

/// Whether TLC evaluates every operand of `e` as a value, never as a part of
/// the behavior specification, an initial predicate or an action.
fn value_making(e: &Expr) -> bool {
    match e {
        Expr::Apply { callee, .. } => matches!(callee, OpRef::Std(_)),
        Expr::Quant { kind, .. } => *kind == QuantKind::Choose,
        Expr::Builtin { .. }
        | Expr::SetFilter { .. }
        | Expr::SetMap { .. }
        | Expr::FcnCons { .. }
        | Expr::RecFcn { .. }
        | Expr::SetEnum(_)
        | Expr::Tuple(_)
        | Expr::Record(_)
        | Expr::RecordSet(_)
        | Expr::FcnSet { .. }
        | Expr::Cartesian(_)
        | Expr::FcnApply { .. }
        | Expr::RecordSelect { .. }
        | Expr::Except { .. } => true,
        _ => false,
    }
}
