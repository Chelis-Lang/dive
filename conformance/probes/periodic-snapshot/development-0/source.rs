//! Pure configuration-owned periodic snapshot diagnostic (ADR 0057).
//!
//! Ordinary binding and ordinary temporal verdicts remain unsupported. Source
//! admission happens before constant startup evaluation, not after its effects.
use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use dive_config::{ConfigValue, ConstantEntry, ModelConfig};
use dive_eval::EvalError;
use dive_ir::{Arg, DefId, DefScope, Effects, Expr, Ir, OpRef, StdOp, Symbol};
use dive_model::{BindOptions, PeriodicSnapshot};

/// A privately retained actual configuration selection. Neither a caller-supplied
/// predicate nor a supplied boolean can replace its body or bound context.
pub struct Snapshot {
    bound: PeriodicSnapshot,
}
impl Snapshot {
    /// Exact model bound by the producer, available for inspection only.
    #[must_use]
    pub fn model(&self) -> &dive_model::Model {
        self.bound.model()
    }
    /// Original `_PERIODIC` spelling; replacements retain their own definition ID.
    #[must_use]
    pub fn configured_name(&self) -> &str {
        self.bound.configured_name()
    }
    /// Definition chosen from the pre-constant snapshot, after substitutions.
    #[must_use]
    pub fn definition(&self) -> DefId {
        self.bound.definition()
    }
    /// Retained original body, never a caller-replaced supplied predicate.
    #[must_use]
    pub fn predicate(&self) -> &dive_model::Pred {
        self.bound.predicate()
    }
    /// Compose the retained configuration body with real native dispatch pauses.
    /// No ordinary schedule, completed model proof or validated violation is
    /// certified. The original graph/Session and shared hook quotas still apply.
    ///
    /// # Errors
    /// Changed/foreign ownership, unsupported zero-state body or existing bounds,
    /// before any native clock or worker setup.
    pub fn execute(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
    ) -> Result<crate::periodic_lifecycle::Run, EvalError> {
        crate::periodic_hook::execute(
            self.bound.model(),
            self.bound.predicate(),
            options,
            limits,
            policy,
        )
    }
}

/// Bind a TLC-profile, configuration-owned diagnostic hook without permitting
/// host callbacks during startup. Shared structural input validation runs first.
/// Conservative source admission roots all nonnative user bodies, standard
/// top-level zero-arity startup bodies and actual matching module-context alias
/// targets. Effects, runtime names, opaque operator actuals and recursion refuse.
/// Temporal model obligations may exist; the selected hook must satisfy the
/// existing bounded pure zero-state grammar after binding.
///
/// Source inspection and captured-body admission each retain the existing
/// 50,000-unit/128-depth policy. These are inspection reservations, not proven
/// evaluator instruction, heap or byte bounds: pure constant precomputation and
/// constructed-value equality can cost more. Native hooks reserve the existing
/// checked cubic policy charge from the shared graph work quota.
///
/// # Errors
/// Callback-free source refusal before binder startup, original binding failures,
/// unsupported non-definition role (actual TLC Java exception boundary), or
/// unsupported bounded pure zero-state selection. Never a checking verdict.
pub fn bind(ir: Arc<Ir>, cfg: &ModelConfig) -> Result<Snapshot, Box<dive_model::Failure>> {
    admit_source(&ir, cfg).map_err(failure)?;
    let bound = dive_model::bind_periodic_snapshot(ir, cfg, &BindOptions::tlc())?;
    crate::periodic_hook::admit(bound.model(), bound.predicate()).map_err(failure)?;
    Ok(Snapshot { bound })
}
fn failure(error: EvalError) -> Box<dive_model::Failure> {
    Box::new(dive_model::Failure {
        error: dive_model::BindError {
            kind: dive_model::ErrorKind::Unsupported {
                feature: error.to_string(),
            },
            message: error.to_string(),
        },
        output: Vec::new(),
    })
}
fn refusal() -> EvalError {
    EvalError::unsupported("bounded callback-free configured PERIODIC source (DIVE-L0058)")
}
fn charge(work: &mut usize, units: usize) -> Result<(), EvalError> {
    *work = work.checked_sub(units).ok_or_else(refusal)?;
    Ok(())
}
fn native(op: StdOp) -> Result<(), EvalError> {
    if op.impure()
        || op.creates_runtime_names()
        || matches!(
            op,
            StdOp::TlcTLCGet | StdOp::TlcTLCSet | StdOp::TlcExtTLCGetOrDefault
        )
    {
        return Err(refusal());
    }
    Ok(())
}
fn admit_source(ir: &Arc<Ir>, cfg: &ModelConfig) -> Result<(), EvalError> {
    dive_eval::validate_ir(ir)?;
    let mut work = 50_000;
    // Bound reachability tables/reservations before retaining roots and metadata.
    charge(&mut work, ir.nodes().len())?;
    charge(&mut work, ir.defs().len())?;
    let mut targets = BTreeSet::new();
    let mut replaced = BTreeSet::new();
    let mut substitution_targets = BTreeSet::new();
    let mut replaced_definitions = HashSet::new();
    let mut startup_budget = 50_000;
    for entry in &cfg.constants {
        charge(&mut work, 1)?;
        match entry {
            ConstantEntry::Substitution {
                name,
                target,
                module,
            } => {
                charge(&mut work, name.text.len())?;
                charge(&mut work, target.text.len())?;
                replaced.insert(name.text.as_str());
                targets.insert(target.text.as_str());
                substitution_targets.insert(target.text.as_str());
                if let Some(module) = module {
                    charge(&mut work, module.text.len())?;
                }
            }
            ConstantEntry::Assignment {
                name,
                args,
                value,
                module,
            } => {
                charge(&mut work, name.text.len())?;
                replaced.insert(name.text.as_str());
                if let Some(module) = module {
                    charge(&mut work, module.text.len())?;
                }
                config_value(&value.value, 0, &mut work)?;
                for arg in args {
                    config_value(&arg.value, 0, &mut work)?;
                }
            }
        }
    }
    for role in [
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
        charge(&mut work, role.text.len())?;
        charge(&mut work, 1)?;
        targets.insert(role.text.as_str());
    }
    for roles in [
        &cfg.invariants,
        &cfg.properties,
        &cfg.constraints,
        &cfg.action_constraints,
        &cfg.postconditions,
        &cfg.possible,
    ] {
        for role in roles {
            charge(&mut work, role.text.len())?;
            charge(&mut work, 1)?;
            targets.insert(role.text.as_str());
        }
    }
    let mut roots = Vec::new();
    for (index, def) in ir.defs().iter().enumerate() {
        let startup = !ir.module(def.module).standard
            || (def.params.is_empty() && matches!(def.scope, DefScope::TopLevel));
        if (startup && def.std.is_none()) || targets.contains(def.name.as_str()) {
            charge(&mut work, 1)?;
            roots.push(DefId(u32::try_from(index).map_err(|_| refusal())?));
        }
    }
    for module in ir.modules() {
        charge(&mut work, 1)?;
        for (name, symbol) in &module.context {
            charge(&mut work, name.len())?;
            charge(&mut work, 1)?;
            if replaced.contains(name.as_str())
                && let Symbol::Def(definition) = symbol
            {
                replaced_definitions.insert(*definition);
            }
            if targets.contains(name.as_str())
                && let Symbol::Def(definition) = symbol
            {
                charge(&mut work, 1)?;
                roots.push(*definition);
            }
        }
    }
    // Actual context aliases can resolve a substitution target to a local body
    // whose real Def.name differs; require its startup grammar too.
    for module in ir.modules() {
        for (name, symbol) in &module.context {
            if substitution_targets.contains(name.as_str())
                && let Symbol::Def(id) = symbol
            {
                let mut grammar_budget = 50_000;
                startup(ir, ir.def(*id).body, 0, &mut grammar_budget)?;
                charge(
                    &mut startup_budget,
                    crate::periodic_hook::evaluation_charge(50_000 - grammar_budget)
                        .ok_or_else(refusal)?,
                )?;
            }
        }
    }
    let reached = ir.reachable_defs(&roots);
    let mut seen = HashSet::new();
    for (index, def) in ir.defs().iter().enumerate() {
        if !reached[index] {
            continue;
        }
        if def.recursive
            || def.depth > 128
            || def
                .effects
                .intersects(Effects::IMPURE.union(Effects::LIVE_TLCGET))
        {
            return Err(refusal());
        }
        if let Some(op) = def.std {
            native(op)?;
            continue;
        }
        if def.params.is_empty()
            && (replaced.contains(def.name.as_str())
                || !nonconstant(ir, def.body, &replaced_definitions, 0, &mut work)?)
        {
            let mut grammar_budget = 50_000;
            startup(ir, def.body, 0, &mut grammar_budget)?;
            let weight = crate::periodic_hook::evaluation_charge(50_000 - grammar_budget)
                .ok_or_else(refusal)?;
            charge(&mut startup_budget, weight)?;
        }
        let mut pending = vec![(def.body, 0u32)];
        while let Some((id, depth)) = pending.pop() {
            if depth > 128 {
                return Err(refusal());
            }
            if !seen.insert(id) {
                continue;
            }
            charge(&mut work, 1)?;
            let expression = ir.expr(id);
            if matches!(expression, Expr::Unbound { .. } | Expr::Unevaluable { .. }) {
                return Err(refusal());
            }
            for reference in expression.op_refs() {
                match reference {
                    OpRef::Std(op) => native(*op)?,
                    OpRef::Param(_) | OpRef::Const(_) | OpRef::Subst(_) | OpRef::Unbound(_) => {
                        return Err(refusal());
                    }
                    OpRef::Def(_) | OpRef::Builtin(_) => {}
                }
            }
            for child in expression.children() {
                charge(&mut work, 1)?;
                pending.push((child, depth + 1));
            }
        }
    }
    Ok(())
}

fn config_value(value: &ConfigValue, depth: u32, work: &mut usize) -> Result<(), EvalError> {
    if depth > 128 {
        return Err(refusal());
    }
    charge(work, 1)?;
    match value {
        ConfigValue::String(text) | ConfigValue::ModelValue(text) => charge(work, text.len())?,
        ConfigValue::Set(values) => {
            charge(work, values.len())?;
            for child in values {
                config_value(&child.value, depth + 1, work)?;
            }
        }
        ConfigValue::Int(_) | ConfigValue::Bool(_) => {}
    }
    Ok(())
}
// A structural witness only: edited level/effect annotations cannot turn an
// unproved startup expression into a state/temporal exemption. Overrides may
// erase a called definition's original nonconstant witness, so never follow one.
fn nonconstant(
    ir: &Ir,
    id: dive_ir::ExprId,
    replaced: &HashSet<DefId>,
    depth: u32,
    work: &mut usize,
) -> Result<bool, EvalError> {
    if depth > 128 {
        return Err(refusal());
    }
    charge(work, 1)?;
    let expr = ir.expr(id);
    if matches!(
        expr,
        Expr::Var { .. }
            | Expr::Prime(_)
            | Expr::Unchanged(_)
            | Expr::StepBox { .. }
            | Expr::StepAngle { .. }
            | Expr::Enabled(_)
            | Expr::Cdot(..)
            | Expr::Always(_)
            | Expr::Eventually(_)
            | Expr::LeadsTo(..)
            | Expr::WhilePlus(..)
            | Expr::Wf { .. }
            | Expr::Sf { .. }
            | Expr::TemporalQuant { .. }
    ) {
        return Ok(true);
    }
    if let Expr::Def(def)
    | Expr::Apply {
        callee: OpRef::Def(def),
        ..
    } = expr
    {
        let d = ir.def(*def);
        if d.std.is_none()
            && !replaced.contains(def)
            && nonconstant(ir, d.body, replaced, depth + 1, work)?
        {
            return Ok(true);
        }
    }
    for child in expr.children() {
        if nonconstant(ir, child, replaced, depth + 1, work)? {
            return Ok(true);
        }
    }
    Ok(false)
}
// Deliberately smaller than general evaluation. No quantifiers, symbolic
// constructors, function application, parameterized user calls or callbacks.
fn startup(ir: &Ir, id: dive_ir::ExprId, depth: u32, work: &mut usize) -> Result<(), EvalError> {
    if depth > 128 {
        return Err(refusal());
    }
    charge(work, 1)?;
    let expr = ir.expr(id);
    match expr {
        Expr::Bool(_) | Expr::Const(_) | Expr::Local(_) => {}
        Expr::Num(text) | Expr::Str(text) => charge(work, text.len())?,
        Expr::Def(def)
        | Expr::Apply {
            callee: OpRef::Def(def),
            ..
        } => {
            let d = ir.def(*def);
            if let Some(op) = d.std {
                if !crate::periodic_hook::native(op) {
                    return Err(refusal());
                }
            } else {
                if d.recursive || !d.params.is_empty() {
                    return Err(refusal());
                }
                startup(ir, d.body, depth + 1, work)?;
            }
            if let Expr::Apply { args, .. } = expr {
                for arg in args {
                    match arg {
                        Arg::Expr { expr, .. } => startup(ir, *expr, depth + 1, work)?,
                        Arg::Op { .. } => return Err(refusal()),
                    }
                }
            }
        }
        Expr::Apply {
            callee: OpRef::Std(op),
            args,
        } => {
            if !crate::periodic_hook::native(*op) {
                return Err(refusal());
            }
            for arg in args {
                match arg {
                    Arg::Expr { expr, .. } => startup(ir, *expr, depth + 1, work)?,
                    Arg::Op { .. } => return Err(refusal()),
                }
            }
        }
        Expr::Builtin { op, .. } => {
            if !crate::periodic_hook::builtin(*op) {
                return Err(refusal());
            }
            for child in expr.children() {
                startup(ir, child, depth + 1, work)?;
            }
        }
        Expr::Let { defs, body } => {
            for id in defs {
                let d = ir.def(*id);
                if d.recursive || !d.params.is_empty() {
                    return Err(refusal());
                }
                startup(ir, d.body, depth + 1, work)?;
            }
            startup(ir, *body, depth + 1, work)?;
        }
        Expr::Record(fields) => {
            for field in fields {
                charge(work, field.name.len())?;
                startup(ir, field.value, depth + 1, work)?;
            }
        }
        Expr::Junction { .. }
        | Expr::Not(_)
        | Expr::Implies(..)
        | Expr::Equiv(..)
        | Expr::If { .. }
        | Expr::Case { .. }
        | Expr::SetEnum(_)
        | Expr::Tuple(_)
        | Expr::Selected(_)
        | Expr::Label { .. } => {
            for child in expr.children() {
                startup(ir, child, depth + 1, work)?;
            }
        }
        _ => return Err(refusal()),
    }
    Ok(())
}
