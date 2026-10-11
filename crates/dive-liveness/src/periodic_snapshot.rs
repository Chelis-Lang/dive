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
    configuration: ModelConfig,
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
    ) -> Result<Run, EvalError> {
        self.execute_inner(options, limits, policy, None, false, false)
    }
    /// Add an internally generated timer/initial-force lane to actual expansion
    /// claims. Requests join claimed work under real dispatch authority, then
    /// publish the maximal returned contiguous prefix through the shared ordered
    /// safety/source path, then resume retained unclaimed or later work. The
    /// configured hook and temporal graph inspect only actual source publication;
    /// returned output outside that prefix remains uncommitted.
    /// This diagnostic is not ordinary TLC queue/schedule or model-proof evidence.
    ///
    /// # Errors
    /// The same callback-free snapshot/predicate admission as `execute`.
    pub fn execute_requests(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
    ) -> Result<Run, EvalError> {
        self.execute_inner(options, limits, policy, Some(requests), false, false)
    }
    /// Service each genuine request once after its returned prefix has passed
    /// ordered safety/graph publication and successful frontier append. Source
    /// callbacks only publish graph data in this separate diagnostic mode.
    /// An empty request services existing initialization registrations with no
    /// invented source or EOF. The retained configured hook runs under the real
    /// dispatch pause; a deciding candidate, false hook or error prevents resume.
    /// Ordinary scheduling and model-proof certification remain unsupported.
    ///
    /// # Errors
    /// The same callback-free snapshot admission and unchanged cumulative limits.
    /// This entry requires the TLC profile and requested evaluator options exactly
    /// matching the retained bound evaluator; mixed comparison/order, warning or
    /// depth settings refuse before clock/worker setup. Legacy entries are unchanged.
    pub fn execute_service(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
    ) -> Result<Run, EvalError> {
        self.execute_inner(options, limits, policy, Some(requests), true, false)
    }
    /// Run the same request-owned service while retaining bounded original
    /// concrete source authority for independent candidate admission. This is
    /// still a diagnostic: no caller may certify its graph or selected word.
    /// Raw fingerprint collisions and additional provenance retention bounds
    /// refuse; the existing service entry preserves its prior resource policy.
    ///
    /// # Errors
    /// The same source/profile admission and shared limits as `execute_service`.
    pub fn execute_candidate_source(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
    ) -> Result<Run, EvalError> {
        self.execute_inner(options, limits, policy, Some(requests), true, true)
    }
    fn execute_inner(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: Option<dive_check::progress::Options>,
        service: bool,
        retain_sources: bool,
    ) -> Result<Run, EvalError> {
        admit_source(&self.bound.model().ir, &self.configuration)?;
        crate::periodic_hook::admit(self.bound.model(), self.bound.predicate())?;
        let tlc = dive_eval::Options::tlc();
        if service
            && (options.profile != dive_eval::Profile::Tlc
                || options.eval.ctx != tlc.ctx
                || options.eval.comparisons != tlc.comparisons
                || options.eval.exploration != tlc.exploration
                || options.eval != *self.bound.model().evaluator.options())
        {
            return Err(EvalError::unsupported(
                "request-owned service requires matching TLC checker/evaluator mode",
            )
            .at(self.bound.predicate().expr));
        }
        let mut model = self.bound.model().clone();
        match model.unsupported.pop() {
            Some(marker)
                if marker.kind == dive_model::UnsupportedKind::Feature
                    && marker.feature == PeriodicSnapshot::ORDINARY_REFUSAL => {}
            _ => return Err(refusal()),
        }
        let lifecycle = if let Some(requests) = requests.filter(|_| service) {
            owned_service(
                self.bound.model(),
                Some(self.bound.predicate()),
                options,
                limits,
                policy,
                requests,
                retain_sources,
                false,
            )?
        } else if let Some(requests) = requests {
            crate::periodic_hook::execute_requests(
                &model,
                self.bound.predicate(),
                options,
                limits,
                policy,
                requests,
            )?
        } else {
            crate::periodic_hook::execute(&model, self.bound.predicate(), options, limits, policy)?
        };
        Ok(Run {
            lifecycle,
            name: self.configured_name().to_owned(),
            definition: self.definition(),
            expression: self.predicate().expr,
        })
    }
}

/// Producer-owned configured-hook provenance attached to actual diagnostic work.
/// A partial success or independently replayed candidate is never a model proof.
pub struct Run {
    lifecycle: crate::periodic_lifecycle::Run,
    name: String,
    definition: DefId,
    expression: dive_ir::ExprId,
}
impl Run {
    /// Actual underlying lifecycle, pause and stop evidence.
    #[must_use]
    pub fn lifecycle(&self) -> &crate::periodic_lifecycle::Run {
        &self.lifecycle
    }
    /// Original configured name retained by the private snapshot producer.
    #[must_use]
    pub fn configured_name(&self) -> &str {
        &self.name
    }
    /// Original pre-constant selected definition.
    #[must_use]
    pub fn definition(&self) -> DefId {
        self.definition
    }
    /// Original selected definition body, not a caller-replaced predicate.
    #[must_use]
    pub fn expression(&self) -> dive_ir::ExprId {
        self.expression
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
    admit_source(&ir, cfg).map_err(|error| Box::new(failure(&error)))?;
    let bound = dive_model::bind_periodic_snapshot(ir, cfg, &BindOptions::tlc())?;
    crate::periodic_hook::admit(bound.model(), bound.predicate()).map_err(|error| {
        let mut failure = failure(&error);
        failure.output.clone_from(&bound.model().output);
        Box::new(failure)
    })?;
    Ok(Snapshot {
        bound,
        configuration: cfg.clone(),
    })
}

/// Privately bound optional configuration role. Genuine absence skips only the
/// hook stage; graph/controller, source admission and EOF authority are unchanged.
/// This diagnostic never enables ordinary checking or certifies a model proof.
pub struct OptionalSnapshot {
    bound: dive_model::OptionalPeriodicSnapshot,
    configuration: ModelConfig,
}
impl OptionalSnapshot {
    /// Exact owned model, retaining the ordinary-check refusal marker.
    #[must_use]
    pub fn model(&self) -> &dive_model::Model {
        self.bound.model()
    }
    /// Original configured spelling, or actual absence in that configuration.
    #[must_use]
    pub fn configured_name(&self) -> Option<&str> {
        self.bound.configured_name()
    }
    /// Actual pre-constant selected definition, if configured.
    #[must_use]
    pub fn definition(&self) -> Option<DefId> {
        self.bound.definition()
    }
    /// Actual selected body and context, if configured.
    #[must_use]
    pub fn predicate(&self) -> Option<&dive_model::Pred> {
        self.bound.predicate()
    }
    /// Service genuine internally produced requests under owned dispatch pauses.
    /// No hook is evaluated when the original configuration has none. Absence
    /// does not imply successful temporal checking or an ordinary schedule.
    ///
    /// # Errors
    /// Callback-free source/context admission, nonmatching TLC options, or
    /// existing graph, capture, predicate and cumulative resource bounds.
    pub fn execute_service(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
    ) -> Result<OptionalRun, EvalError> {
        self.execute_inner(options, limits, policy, requests, false, false)
    }
    /// Same service, retaining bounded original concrete source authority for
    /// independent candidate replay. No caller-supplied graph or word is trusted.
    ///
    /// # Errors
    /// The same admission and existing source-retention bounds as the service.
    pub fn execute_candidate_source(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
    ) -> Result<OptionalRun, EvalError> {
        self.execute_inner(options, limits, policy, requests, true, false)
    }
    /// Retain real source provenance and service the genuinely exhausted queue
    /// before timer/pool joins, then independently selectable final candidates.
    /// The configured hook is skipped when the finished queue cannot suspend.
    /// This remains diagnostic: ordinary checking and temporal Holds refuse.
    ///
    /// # Errors
    /// Existing source/profile admission and cumulative resource bounds.
    pub fn execute_terminal_candidate_source(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
    ) -> Result<OptionalRun, EvalError> {
        self.execute_inner(options, limits, policy, requests, true, true)
    }
    /// Retain real source provenance and service an eligible ordered worker
    /// failure before joins. Diagnostic faults never replace the original error;
    /// no failed source is promoted to EOF, final checking or temporal Holds.
    ///
    /// # Errors
    /// Existing source/profile admission and cumulative resource bounds.
    pub fn execute_stopped_candidate_source(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
    ) -> Result<OptionalRun, EvalError> {
        self.execute_mode(
            options,
            limits,
            policy,
            requests,
            true,
            ServiceMode::Stopped,
        )
    }
    fn execute_inner(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
        retain_sources: bool,
        terminal: bool,
    ) -> Result<OptionalRun, EvalError> {
        self.execute_mode(
            options,
            limits,
            policy,
            requests,
            retain_sources,
            if terminal {
                ServiceMode::Terminal
            } else {
                ServiceMode::Requests
            },
        )
    }
    fn execute_mode(
        &self,
        options: &dive_check::CheckOptions,
        limits: crate::phase_graph::Limits,
        policy: crate::periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
        retain_sources: bool,
        mode: ServiceMode,
    ) -> Result<OptionalRun, EvalError> {
        admit_source(&self.model().ir, &self.configuration)?;
        let lifecycle = owned_service_mode(
            self.model(),
            self.predicate(),
            options,
            limits,
            policy,
            requests,
            retain_sources,
            mode,
        )?;
        let role = match (self.configured_name(), self.definition(), self.predicate()) {
            (Some(name), Some(definition), Some(predicate)) => {
                Some((name.to_owned(), definition, predicate.expr))
            }
            (None, None, None) => None,
            _ => return Err(refusal()),
        };
        Ok(OptionalRun { lifecycle, role })
    }
}
/// Private optional-hook provenance accompanying actual diagnostic execution.
/// The optional identity is producer-derived, never a caller's absence flag.
pub struct OptionalRun {
    lifecycle: crate::periodic_lifecycle::Run,
    role: Option<(String, DefId, dive_ir::ExprId)>,
}
impl OptionalRun {
    /// Actual request/EOF/ordered failure evidence; never a model proof.
    #[must_use]
    pub fn lifecycle(&self) -> &crate::periodic_lifecycle::Run {
        &self.lifecycle
    }
    /// Actual configured spelling, or genuine absence.
    #[must_use]
    pub fn configured_name(&self) -> Option<&str> {
        self.role.as_ref().map(|r| r.0.as_str())
    }
    /// Actual selected definition, or genuine absence.
    #[must_use]
    pub fn definition(&self) -> Option<DefId> {
        self.role.as_ref().map(|r| r.1)
    }
    /// Actual selected body, or genuine absence.
    #[must_use]
    pub fn expression(&self) -> Option<dive_ir::ExprId> {
        self.role.as_ref().map(|r| r.2)
    }
}
/// Bind an optional configuration-owned diagnostic hook. Pre-startup structural,
/// ownership, effect and bounded grammar guards are the same as mandatory binding.
/// Absence is read from the original configuration, never substituted with TRUE.
///
/// # Errors
/// Original binding error or callback-free source/selected-body refusal. No
/// ordinary schedule, Holds or model-proof certificate is produced.
pub fn bind_optional(
    ir: Arc<Ir>,
    cfg: &ModelConfig,
) -> Result<OptionalSnapshot, Box<dive_model::Failure>> {
    admit_source(&ir, cfg).map_err(|error| Box::new(failure(&error)))?;
    let bound = dive_model::bind_optional_periodic_snapshot(ir, cfg, &BindOptions::tlc())?;
    if let Some(predicate) = bound.predicate() {
        crate::periodic_hook::admit(bound.model(), predicate).map_err(|error| {
            let mut f = failure(&error);
            f.output.clone_from(&bound.model().output);
            Box::new(f)
        })?;
    }
    Ok(OptionalSnapshot {
        bound,
        configuration: cfg.clone(),
    })
}
#[derive(Clone, Copy)]
enum ServiceMode {
    Requests,
    Terminal,
    Stopped,
}
#[allow(
    clippy::too_many_arguments,
    reason = "one shared request-service admission boundary"
)]
fn owned_service(
    bound: &dive_model::Model,
    predicate: Option<&dive_model::Pred>,
    options: &dive_check::CheckOptions,
    limits: crate::phase_graph::Limits,
    policy: crate::periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
    retain_sources: bool,
    terminal: bool,
) -> Result<crate::periodic_lifecycle::Run, EvalError> {
    owned_service_mode(
        bound,
        predicate,
        options,
        limits,
        policy,
        requests,
        retain_sources,
        if terminal {
            ServiceMode::Terminal
        } else {
            ServiceMode::Requests
        },
    )
}
#[allow(
    clippy::too_many_arguments,
    reason = "one shared source-owned admission boundary"
)]
fn owned_service_mode(
    bound: &dive_model::Model,
    predicate: Option<&dive_model::Pred>,
    options: &dive_check::CheckOptions,
    limits: crate::phase_graph::Limits,
    policy: crate::periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
    retain_sources: bool,
    mode: ServiceMode,
) -> Result<crate::periodic_lifecycle::Run, EvalError> {
    bound.validate_context()?;
    let hook = predicate
        .map(|p| crate::periodic_hook::admit(bound, p).map(|work| (p.clone(), work)))
        .transpose()?;
    let tlc = dive_eval::Options::tlc();
    if options.profile != dive_eval::Profile::Tlc
        || options.eval.ctx != tlc.ctx
        || options.eval.comparisons != tlc.comparisons
        || options.eval.exploration != tlc.exploration
        || options.eval != *bound.evaluator.options()
    {
        return Err(EvalError::unsupported(
            "request-owned service requires matching TLC checker/evaluator mode",
        ));
    }
    let mut model = bound.clone();
    match model.unsupported.pop() {
        Some(marker)
            if marker.kind == dive_model::UnsupportedKind::Feature
                && marker.feature == PeriodicSnapshot::ORDINARY_REFUSAL => {}
        _ => return Err(refusal()),
    }
    Ok(match mode {
        ServiceMode::Stopped => crate::phase_execution::execute_periodic_stopped_service(
            &model,
            options,
            limits,
            policy,
            hook,
            requests,
            retain_sources,
        ),
        ServiceMode::Requests | ServiceMode::Terminal => {
            crate::phase_execution::execute_periodic_service(
                &model,
                options,
                limits,
                policy,
                hook,
                requests,
                retain_sources,
                matches!(mode, ServiceMode::Terminal),
            )
        }
    })
}

fn failure(error: &EvalError) -> dive_model::Failure {
    dive_model::Failure {
        error: dive_model::BindError {
            kind: dive_model::ErrorKind::Unsupported {
                feature: error.to_string(),
            },
            message: error.to_string(),
        },
        output: Vec::new(),
    }
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
#[allow(
    clippy::too_many_lines,
    reason = "bounded source/configuration admission stages remain explicit for precedence audit"
)]
fn admit_source(ir: &Arc<Ir>, cfg: &ModelConfig) -> Result<(), EvalError> {
    dive_eval::validate_ir(ir)?;
    let mut work = 50_000;
    // Bound reachability tables/reservations before retaining roots and metadata.
    charge(&mut work, ir.nodes().len())?;
    charge(&mut work, ir.defs().len())?;
    let mut targets = BTreeSet::new();
    let mut replaced = BTreeSet::new();
    let mut substitution_targets = BTreeSet::new();
    let mut substituted_names = BTreeSet::new();
    let mut substituted_constants = HashSet::new();
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
                substituted_names.insert(name.text.as_str());
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
                let before = work;
                config_value(&value.value, 0, &mut work)?;
                for arg in args {
                    config_value(&arg.value, 0, &mut work)?;
                }
                charge(
                    &mut startup_budget,
                    crate::periodic_hook::evaluation_charge(before - work).ok_or_else(refusal)?,
                )?;
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
        charge(&mut work, def.name.len())?;
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
            if substituted_names.contains(name.as_str())
                && let Symbol::Decl(decl) = symbol
                && let Some(constant) = ir.decl(*decl).constant
            {
                substituted_constants.insert(constant);
            }
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
                startup(
                    ir,
                    ir.def(*id).body,
                    *id,
                    &substituted_constants,
                    0,
                    &mut grammar_budget,
                )?;
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
            startup(
                ir,
                def.body,
                DefId(u32::try_from(index).map_err(|_| refusal())?),
                &substituted_constants,
                0,
                &mut grammar_budget,
            )?;
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
#[allow(
    clippy::too_many_lines,
    reason = "explicit bounded startup grammar preserves callback-free admission order"
)]
fn startup(
    ir: &Ir,
    id: dive_ir::ExprId,
    frame: DefId,
    substituted: &HashSet<dive_ir::ConstId>,
    depth: u32,
    work: &mut usize,
) -> Result<(), EvalError> {
    if depth > 128 {
        return Err(refusal());
    }
    charge(work, 1)?;
    let expr = ir.expr(id);
    match expr {
        Expr::Bool(_) => {}
        Expr::Const(constant) => {
            if substituted.contains(constant) {
                return Err(refusal());
            }
        }
        Expr::Local(local) => {
            if local.up > 128 {
                return Err(refusal());
            }
            let mut target = frame;
            if let DefScope::LetValue { parent, .. } = ir.def(target).scope {
                target = parent;
            }
            for _ in 0..local.up {
                target = match ir.def(target).scope {
                    DefScope::Local { parent }
                    | DefScope::Inline { parent }
                    | DefScope::LetValue { parent, .. } => parent,
                    DefScope::TopLevel => return Err(refusal()),
                };
                if let DefScope::LetValue { parent, .. } = ir.def(target).scope {
                    target = parent;
                }
            }
            let Some(slot) = ir.def(target).frame.get(local.slot.index()) else {
                return Err(refusal());
            };
            let dive_ir::SlotKind::LetValue(definition) = slot.kind else {
                return Err(refusal());
            };
            let d = ir.def(definition);
            if d.recursive || !d.params.is_empty() {
                return Err(refusal());
            }
            startup(ir, d.body, definition, substituted, depth + 1, work)?;
        }
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
                startup(ir, d.body, *def, substituted, depth + 1, work)?;
            }
            if let Expr::Apply { args, .. } = expr {
                for arg in args {
                    match arg {
                        Arg::Expr { expr, .. } => {
                            startup(ir, *expr, frame, substituted, depth + 1, work)?;
                        }
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
                    Arg::Expr { expr, .. } => {
                        startup(ir, *expr, frame, substituted, depth + 1, work)?;
                    }
                    Arg::Op { .. } => return Err(refusal()),
                }
            }
        }
        Expr::Builtin { op, .. } => {
            if !crate::periodic_hook::builtin(*op) {
                return Err(refusal());
            }
            for child in expr.children() {
                startup(ir, child, frame, substituted, depth + 1, work)?;
            }
        }
        Expr::Let { defs, body } => {
            for id in defs {
                let d = ir.def(*id);
                if d.recursive || !d.params.is_empty() {
                    return Err(refusal());
                }
                startup(ir, d.body, *id, substituted, depth + 1, work)?;
            }
            startup(ir, *body, frame, substituted, depth + 1, work)?;
        }
        Expr::Record(fields) => {
            for field in fields {
                charge(work, field.name.len())?;
                startup(ir, field.value, frame, substituted, depth + 1, work)?;
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
                startup(ir, child, frame, substituted, depth + 1, work)?;
            }
        }
        _ => return Err(refusal()),
    }
    Ok(())
}
