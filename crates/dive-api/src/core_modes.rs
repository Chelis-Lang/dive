//! Finite execution over the existing compiled plan and value machine.
//!
//! Search order belongs to each mode; generation and predicate evaluation retain
//! the same plan/frame rules as the BFS core. Report conversion is a boundary.

pub(crate) mod dfid;

use std::sync::Arc;

use dive_core::generate::Sink;
use dive_core::memo::Memo;
use dive_core::plan::{Action, Check, Code, Plan};
use dive_core::{Error, ErrorKind, Handle, Machine, Opts};
use dive_eval::next::StateVals;
use dive_model::Model;
use dive_sim::{Outcome, Report, Trace};
use dive_stdlib::random::{RandomSource, Streams};

pub(crate) enum SetupError {
    Unsupported(String),
    Internal(String),
    Stopped,
}

impl SetupError {
    pub(crate) fn dfid(self) -> dive_check::dfid::Outcome {
        match self {
            Self::Unsupported(message) => dive_check::dfid::Outcome::Unsupported(message),
            Self::Internal(message) => dive_check::dfid::Outcome::Internal(message),
            Self::Stopped => dive_check::dfid::Outcome::Cancelled,
        }
    }

    pub(crate) fn simulation(self) -> Outcome {
        match self {
            Self::Unsupported(message) => Outcome::Unsupported(message),
            Self::Internal(message) => Outcome::Internal(message),
            Self::Stopped => Outcome::Cancelled,
        }
    }
}

pub(crate) struct Native {
    machine: Machine,
    plan: Plan,
    budget: dive_check::budget::Budget,
}

impl Native {
    pub(crate) fn probe(
        model: &crate::trace::SpecModel,
        seed: i64,
        mode: dive_stdlib::random::RandomMode,
        budget: &dive_check::budget::Budget,
        control: Arc<dyn dive_eval::HostControl>,
    ) -> Result<Self, SetupError> {
        let fresh = model.preparation_model(seed, mode, control);
        if budget.poll() != Ok(None) {
            return Err(SetupError::Stopped);
        }
        Self::prepare(&fresh.map_err(SetupError::Unsupported)?, budget)
    }

    /// Preparation does not check assumptions, generate states or emit output.
    pub(crate) fn prepare(
        model: &Model,
        budget: &dive_check::budget::Budget,
    ) -> Result<Self, SetupError> {
        if model.runtime_name_order {
            return Err(SetupError::Unsupported(
                "native finite mode runtime string encounter order".into(),
            ));
        }
        let scopes = model
            .invariants
            .iter()
            .chain(&model.constraints)
            .chain(&model.action_constraints)
            .chain(&model.implied_inits)
            .chain(&model.implied_actions);
        if scopes
            .into_iter()
            .any(|role| model.evaluator.scope_shares_side_effects(&role.pred.scope))
            || model
                .init_actions
                .iter()
                .chain(&model.actions)
                .any(|action| model.evaluator.action_shares_side_effects(action))
        {
            return Err(SetupError::Unsupported(
                "native finite mode preparation with effectful captured lazy arguments".into(),
            ));
        }
        let opts = std::env::var("DIVE_CORE_OPT")
            .ok()
            .map_or(Ok(Opts::default()), |text| Opts::parse(&text))
            .map_err(SetupError::Unsupported)?;
        Self::prepare_with_options(model, budget, opts)
    }

    fn prepare_with_options(
        model: &Model,
        budget: &dive_check::budget::Budget,
        opts: Opts,
    ) -> Result<Self, SetupError> {
        let opts = Opts {
            memo_cap: crate::core_engine::memo_cap(budget.memory, 1),
            ..opts
        };
        let store = crate::core_engine::store_for(
            model,
            opts.hashcons,
            crate::core_engine::semantics(model.evaluator.options()),
        )
        .map(Arc::new)
        .map_err(SetupError::Internal)?;
        let handoff = crate::core_engine::handoff(model, &model.ir, Vec::new())
            .map_err(SetupError::Unsupported)?;
        let plan = dive_core::compile(&handoff, store.clone(), opts).map_err(|error| {
            if error.kind() == ErrorKind::Internal {
                SetupError::Internal(error.to_string())
            } else {
                SetupError::Unsupported(error.to_string())
            }
        })?;
        let lengths: Vec<_> = plan
            .code
            .sites
            .iter()
            .map(dive_core::plan::MemoSite::key_len)
            .collect();
        let mut machine = Machine::new(store, plan.vars.len(), opts, Memo::new(&lengths));
        machine.deny_checker_access();
        machine.set_work_control(Some(crate::core_engine::search_work_control(budget)));
        Ok(Self {
            machine,
            plan,
            budget: budget.clone(),
        })
    }
    pub(crate) fn selection_refusal(&self) -> Option<&'static str> {
        (self.plan.effects.draws || self.plan.effects.registers || self.plan.effects.level)
            .then_some("native finite mode random/register/runtime-level execution")
    }
}

struct Emission<'a, F>(&'a mut F);
impl<F> Sink for Emission<'_, F>
where
    F: FnMut(&mut Machine) -> Result<(), Error>,
{
    fn emit(&mut self, machine: &mut Machine, _: &Code) -> Result<(), Error> {
        (self.0)(machine)
    }
}

fn generate(
    machine: &mut Machine,
    code: &Code,
    action: &Action,
    emit: &mut impl FnMut(&mut Machine) -> Result<(), Error>,
) -> Result<(), Error> {
    generate_selected(machine, code, action, emit, None)
}

fn generate_selected(
    machine: &mut Machine,
    code: &Code,
    action: &Action,
    emit: &mut impl FnMut(&mut Machine) -> Result<(), Error>,
    choose: Option<&mut dyn FnMut(u64) -> u64>,
) -> Result<(), Error> {
    let old = machine.enter(action.frame);
    for &(slot, value) in &action.preset {
        if let Some(target) = machine.frame.get_mut(machine.base + slot as usize) {
            *target = value;
        }
    }
    let result = if let Some(choose) = choose {
        machine.run_action_generation(code, action.g, &mut Emission(emit), choose)
    } else {
        machine.run_action(code, action.g, &mut Emission(emit))
    };
    machine.leave(old);
    result
}

/// State roles read the candidate; step roles read the original source/target.
fn predicate(machine: &mut Machine, code: &Code, check: &Check, step: bool) -> Result<bool, Error> {
    if !step {
        std::mem::swap(&mut machine.s0, &mut machine.s1);
    }
    let old = machine.enter(check.frame);
    let mut result = Ok(true);
    for conjunct in &check.conjuncts {
        match machine.eval_bool(code, conjunct.p) {
            Ok(true) => {}
            other => {
                result = other;
                break;
            }
        }
    }
    machine.leave(old);
    if !step {
        std::mem::swap(&mut machine.s0, &mut machine.s1);
    }
    result
}

fn allowed(
    machine: &mut Machine,
    plan: &Plan,
    checks: &[Check],
    step: bool,
) -> Result<bool, Error> {
    for check in checks {
        if !predicate(machine, &plan.code, check, step)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn roles(
    machine: &mut Machine,
    plan: &Plan,
    checks: &[Check],
    step: bool,
    kind: &'static str,
    code: u32,
    error_code: u32,
) -> Option<Outcome> {
    for check in checks {
        match predicate(machine, &plan.code, check, step) {
            Ok(true) => {}
            Ok(false) => {
                return Some(Outcome::Violation {
                    kind,
                    name: Some(check.name.clone()),
                    code,
                    trace: 0,
                });
            }
            Err(error) => return Some(classify(&error, Some(error_code))),
        }
    }
    None
}

fn retain(machine: &mut Machine) -> Result<Vec<Handle>, Error> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(machine.s1.len())
        .map_err(|_| Error::unsupported("native finite state allocation"))?;
    for index in 0..machine.s1.len() {
        let value = machine.s1[index];
        if value.is_empty() {
            values.push(value);
        } else if value.is_compound() && machine.heap.view(value).is_lazy() {
            return Err(Error::unsupported(
                "native finite state retains an unclosed lazy value",
            ));
        } else {
            values.push(machine.heap.intern_exact(value)?);
        }
    }
    Ok(values)
}

fn finish_batch(machine: &mut Machine, mark: dive_core::arena::Mark) -> Result<(), Error> {
    if machine.base != 0
        || !machine.frame.is_empty()
        || machine.s0.iter().any(|value| value.is_transient())
    {
        return Err(Error::internal(
            "native finite batch retains an active transient source/frame",
        ));
    }
    machine.s1.fill(Handle::EMPTY);
    machine.assigned = 0;
    machine.heap.rollback(mark);
    if machine.heap.poisoned() {
        return Err(Error::internal("native finite batch read a stale value"));
    }
    Ok(())
}

fn values(machine: &mut Machine, model: &Model, row: &[Handle]) -> Result<StateVals, String> {
    let mark = machine.heap.mark();
    let result = row
        .iter()
        .map(|&value| {
            if value.is_empty() {
                Ok(None)
            } else {
                crate::core_engine::to_value(
                    &mut machine.heap,
                    model,
                    value,
                    model.evaluator.options().ctx,
                )
                .map(Some)
            }
        })
        .collect();
    if row.iter().all(|value| !value.is_transient()) {
        machine.heap.rollback(mark);
    }
    result
}

fn pending(outcome: &Outcome) -> Option<dive_sim::PendingViolation> {
    if let Outcome::Violation {
        kind, name, code, ..
    } = outcome
    {
        Some(dive_sim::PendingViolation {
            kind,
            name: name.clone(),
            code: *code,
        })
    } else {
        None
    }
}

fn classify(error: &Error, wrapper: Option<u32>) -> Outcome {
    match error.kind() {
        ErrorKind::Internal => Outcome::Internal(error.to_string()),
        ErrorKind::Eval if error.tlc().is_some() && wrapper.is_some() => Outcome::Error {
            code: wrapper.unwrap_or(1000),
            message: error.to_string(),
        },
        ErrorKind::Eval => Outcome::Unsupported(format!(
            "unverified native finite evaluation-error context: {error}"
        )),
        ErrorKind::Unsupported => Outcome::Unsupported(error.to_string()),
        ErrorKind::Stop => Outcome::Cancelled,
    }
}

pub(crate) fn preflight_report(outcome: Outcome) -> Report {
    Report {
        stage: "preflight",
        outcome,
        generated: 0,
        initial_candidates: 0,
        pending_violation: None,
        traces_started: 0,
        traces: Vec::new(),
        action_flow: None,
        runtime_output: Vec::new(),
    }
}

fn assumptions(model: &Model) -> Option<Outcome> {
    match model.check_assumptions() {
        dive_eval::AssumptionOutcome::AllTrue => None,
        dive_eval::AssumptionOutcome::False { message, .. } => Some(Outcome::Error {
            code: 2104,
            message,
        }),
        dive_eval::AssumptionOutcome::Error { error, message, .. } => Some(match error.class() {
            dive_eval::ErrorClass::Eval => Outcome::Error {
                code: 2105,
                message,
            },
            dive_eval::ErrorClass::Unsupported => Outcome::Unsupported(message),
            dive_eval::ErrorClass::ResourceLimit => Outcome::Limit(message),
            dive_eval::ErrorClass::Bug | dive_eval::ErrorClass::Interrupted => {
                Outcome::Internal(message)
            }
        }),
    }
}

impl Native {
    pub(crate) fn simulate(
        &mut self,
        model: &Model,
        options: &dive_sim::Options,
        poll: &dyn Fn() -> bool,
    ) -> Report {
        if let Some(outcome) = dive_sim::preflight(model, options) {
            return preflight_report(outcome);
        }
        dive_sim::simulate_with_backend(model, options, |remaining| {
            let mut report = self.sample(model, options, poll, remaining);
            report.runtime_output.extend(
                std::mem::take(&mut self.machine.printed)
                    .into_iter()
                    .map(|text| dive_eval::capture::Captured {
                        text,
                        warning: false,
                    }),
            );
            if let Some(reason) = self.machine.output_failure() {
                report.outcome = Outcome::Limit(reason.into());
            }
            if self.machine.heap.poisoned() {
                report.outcome = Outcome::Internal("native finite mode read a stale value".into());
            }
            if !self.machine.warnings.is_empty() {
                report.outcome =
                    Outcome::Unsupported("native simulation warning/output interleaving".into());
            }
            report
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "finite simulation keeps emitted-candidate checks and chosen transitions in one driver"
    )]
    fn sample(
        &mut self,
        model: &Model,
        options: &dive_sim::Options,
        poll: &dyn Fn() -> bool,
        remaining: &dyn Fn() -> (usize, usize),
    ) -> Report {
        let mut report = preflight_report(Outcome::Sampled);
        let (events, bytes) = remaining();
        self.machine.set_output_limits(events, bytes);
        if options.action_stats {
            match dive_sim::ActionFlow::new(model) {
                Ok(flow) => report.action_flow = Some(flow),
                Err(error) => {
                    report.outcome = Outcome::Unsupported(error.to_string());
                    return report;
                }
            }
        }
        if poll() {
            report.outcome = Outcome::Cancelled;
            return report;
        }
        report.stage = "assumptions";
        if let Some(outcome) = assumptions(model) {
            report.outcome = outcome;
            return report;
        }
        let budget = &self.budget;
        match Self::prepare_with_options(model, budget, self.machine.opts) {
            Ok(actual) => *self = actual,
            Err(error) => {
                report.outcome = error.simulation();
                return report;
            }
        }
        let (events, bytes) = remaining();
        self.machine.set_output_limits(events, bytes);
        let Self { machine, plan, .. } = self;
        report.stage = "initialization";
        let mut initial = Vec::new();
        for (index, action) in plan.init.iter().enumerate() {
            let mark = machine.heap.mark();
            let mut limited = false;
            let result = generate(machine, &plan.code, action, &mut |machine| {
                if poll() {
                    return Err(Error::stop());
                }
                if initial.len() >= options.max_candidates
                    || report.initial_candidates >= options.max_generated
                {
                    limited = true;
                    return Err(Error::stop());
                }
                report.initial_candidates += 1;
                initial.push((retain(machine)?, index));
                Ok(())
            });
            if let Err(error) = finish_batch(machine, mark) {
                report.outcome = classify(&error, None);
                return report;
            }
            if let Err(error) = result {
                report.outcome = if limited {
                    Outcome::Limit("simulation initial-state bound".into())
                } else {
                    classify(&error, Some(1000))
                };
                return report;
            }
        }
        report.generated = report.initial_candidates;
        let mut eligible = Vec::new();
        for (state, action) in initial {
            let mark = machine.heap.mark();
            if state.iter().any(|value| value.is_empty()) {
                report.outcome = Outcome::Error {
                    code: 2106,
                    message: "incomplete initial state".into(),
                };
                return report;
            }
            machine.s1.clone_from(&state);
            if let Some(outcome) = roles(
                machine,
                plan,
                &plan.invariants,
                false,
                "invariant",
                2107,
                2102,
            ) {
                if options.max_retained == 0 {
                    report.pending_violation = pending(&outcome);
                    report.outcome = Outcome::Limit("simulation retained-state bound".into());
                    return report;
                }
                report.outcome = outcome;
                match values(machine, model, &state) {
                    Ok(state) => report.traces.push(Trace {
                        id: 0,
                        states: vec![state],
                    }),
                    Err(message) => report.outcome = Outcome::Unsupported(message),
                }
                return report;
            }
            match allowed(machine, plan, &plan.constraints, false) {
                Ok(true) => eligible.push((state, action)),
                Ok(false) => {}
                Err(error) => {
                    report.outcome = classify(&error, None);
                    return report;
                }
            }
            if let Err(error) = finish_batch(machine, mark) {
                report.outcome = classify(&error, None);
                return report;
            }
        }
        if eligible.is_empty() {
            report.outcome = Outcome::Error {
                code: if report.initial_candidates == 0 {
                    2118
                } else {
                    2256
                },
                message: "no initial state satisfies the specification and constraints".into(),
            };
            return report;
        }
        let streams = Streams::new(options.seed, options.mode);
        let mut retained = 0usize;
        report.stage = "sampling";
        for id in 0..options.traces {
            if poll() {
                report.outcome = Outcome::Cancelled;
                return report;
            }
            let mut rng = streams.simulation_trace(id);
            for _ in 0..options.aril {
                if poll() {
                    report.outcome = Outcome::Cancelled;
                    return report;
                }
                let _ = rng.next_double();
            }
            report.traces_started += 1;
            let choice = usize::try_from(rng.next_index(eligible.len() as u64)).unwrap_or(0);
            let (mut current, action) = eligible[choice].clone();
            let mut previous = model.init_actions[action].id();
            if retained >= options.max_retained {
                report.outcome = Outcome::Limit("simulation retained-state bound".into());
                return report;
            }
            let mut states = match values(machine, model, &current) {
                Ok(state) => vec![state],
                Err(message) => {
                    report.outcome = Outcome::Unsupported(message);
                    return report;
                }
            };
            retained += 1;
            for _ in 0..options.depth {
                if poll() {
                    report.traces.push(Trace { id, states });
                    report.outcome = Outcome::Cancelled;
                    return report;
                }
                machine.s0.clone_from(&current);
                let mut candidates = Vec::new();
                let mut index =
                    usize::try_from(rng.next_index(plan.next.len() as u64)).unwrap_or(0);
                let sampled_prime = usize::try_from(rng.next_prime()).unwrap_or(0);
                let prime = if options.generation { 1 } else { sampled_prime };
                let mut failed = None;
                for _ in 0..plan.next.len() {
                    let mark = machine.heap.mark();
                    let mut selected = false;
                    let mut emit = |machine: &mut Machine| {
                        if poll() {
                            return Err(Error::stop());
                        }
                        if candidates.len() >= options.max_candidates
                            || report.generated >= options.max_generated
                        {
                            failed = Some(Outcome::Limit("simulation successor bound".into()));
                            return Err(Error::stop());
                        }
                        if machine.assigned != machine.s1.len() {
                            failed = Some(Outcome::Error {
                                code: 2109,
                                message: "incomplete successor state".into(),
                            });
                            return Err(Error::stop());
                        }
                        report.generated += 1;
                        failed = roles(
                            machine,
                            plan,
                            &plan.invariants,
                            false,
                            "invariant",
                            2110,
                            2111,
                        )
                        .or_else(|| {
                            roles(
                                machine,
                                plan,
                                &plan.implied_actions,
                                true,
                                "property",
                                2112,
                                2113,
                            )
                        });
                        if failed.is_some() {
                            if retained >= options.max_retained {
                                failed =
                                    Some(Outcome::Limit("simulation retained-state bound".into()));
                            } else {
                                match values(machine, model, &machine.s1.clone()) {
                                    Ok(next) => {
                                        states.push(next);
                                        retained += 1;
                                    }
                                    Err(message) => failed = Some(Outcome::Unsupported(message)),
                                }
                            }
                            return Err(Error::stop());
                        }
                        if allowed(machine, plan, &plan.constraints, false)?
                            && allowed(machine, plan, &plan.action_constraints, true)?
                        {
                            candidates.push(retain(machine)?);
                            if options.generation {
                                selected = true;
                                return Err(Error::stop());
                            }
                        }
                        Ok(())
                    };
                    let mut choose = |bound| rng.next_index(bound);
                    let choices = if options.generation {
                        Some(&mut choose as &mut dyn FnMut(u64) -> u64)
                    } else {
                        None
                    };
                    let result = generate_selected(
                        machine,
                        &plan.code,
                        &plan.next[index],
                        &mut emit,
                        choices,
                    );
                    if let Err(error) = finish_batch(machine, mark) {
                        failed = Some(classify(&error, None));
                    }
                    if let Err(error) = result
                        && failed.is_none()
                        && !(selected && error.kind() == ErrorKind::Stop)
                    {
                        failed = Some(classify(&error, None));
                    }
                    if failed.is_some() || !candidates.is_empty() {
                        break;
                    }
                    index = (index + prime) % plan.next.len();
                }
                if let Some(mut outcome) = failed {
                    if let Outcome::Violation { trace, .. } = &mut outcome {
                        *trace = report.traces.len();
                    }
                    report.traces.push(Trace { id, states });
                    report.outcome = outcome;
                    return report;
                }
                if candidates.is_empty() {
                    if plan.check_deadlock {
                        report.outcome = Outcome::Violation {
                            kind: "deadlock",
                            name: None,
                            code: 2114,
                            trace: report.traces.len(),
                        };
                        report.traces.push(Trace { id, states });
                        return report;
                    }
                    break;
                }
                if retained >= options.max_retained {
                    report.outcome = Outcome::Limit("simulation retained-state bound".into());
                    return report;
                }
                retained += 1;
                let choice = if options.generation {
                    0
                } else {
                    usize::try_from(rng.next_index(candidates.len() as u64)).unwrap_or(0)
                };
                current = candidates.swap_remove(choice);
                match values(machine, model, &current) {
                    Ok(next) => states.push(next),
                    Err(message) => {
                        report.outcome = Outcome::Unsupported(message);
                        return report;
                    }
                }
                let chosen = model.actions[index].id();
                if let Some(flow) = &mut report.action_flow
                    && let Err(error) = flow.record(previous, chosen)
                {
                    report.outcome = Outcome::Internal(error.to_string());
                    report.traces.push(Trace { id, states });
                    return report;
                }
                previous = chosen;
            }
            report.traces.push(Trace { id, states });
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{SpecModel, TraceOptions};
    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
    fn model(source: &str, config: &str) -> Result<SpecModel> {
        Ok(SpecModel::from_source(
            "M.tla",
            source,
            config,
            &TraceOptions::default(),
        )?)
    }
    fn prepare(model: &Model) -> Result<Native> {
        Native::prepare(model, &dive_check::budget::Budget::unlimited())
            .map_err(|_| "native preparation".into())
    }

    #[test]
    fn constructive_million_way_producer_keeps_one_candidate_and_honors_limits() -> Result {
        let source = model(
            "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x>=0 /\\ (\\E n \\in 1..1000000 : x'=n)\n====\n",
            "INIT Init\nNEXT Next\n",
        )?;
        assert_eq!(source.model().actions.len(), 1);
        for mode in [
            dive_stdlib::random::RandomMode::Dive,
            dive_stdlib::random::RandomMode::Tlc,
        ] {
            let mut options = dive_sim::Options::new(7, mode, 4, 3);
            options.generation = true;
            options.max_candidates = 1;
            let mut native = prepare(source.model())?;
            let report = native.simulate(source.model(), &options, &|| false);
            assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
            assert_eq!(report.generated, 13);
            assert_eq!(report.traces.len(), 3);
            options.max_generated = 3;
            let mut native = prepare(source.model())?;
            let stopped = native.simulate(source.model(), &options, &|| false);
            assert!(matches!(stopped.outcome, Outcome::Limit(_)), "{stopped:?}");
            assert_eq!(stopped.generated, 3);
        }
        Ok(())
    }

    #[test]
    fn feasibility_runtime_owns_fresh_scopes_and_does_not_evaluate_assumptions() -> Result {
        let source = model(
            "---- MODULE M ----\nEXTENDS Naturals,TLC\nVARIABLE x\nASSUME PrintT(\"assume\")\nInit == x = 0\nNext == x' = x + 1\n====\n",
            "INIT Init\nNEXT Next\n",
        )?;
        let budget = dive_check::budget::Budget::unlimited();
        let control = dive_check::budget::EvaluationControl::new(
            &budget,
            dive_check::budget::Phase::Simulation,
        )?;
        let captured = dive_eval::capture::PrintCapture::start();
        let fresh =
            source.preparation_model(7, dive_stdlib::random::RandomMode::Dive, control.clone())?;
        assert!(!Arc::ptr_eq(&source.model().names, &fresh.names));
        assert!(
            source
                .model()
                .evaluator
                .validate_scope(&fresh.actions[0].scope())
                .is_err()
        );
        assert!(
            source
                .model()
                .evaluator
                .validate_scope(&source.model().actions[0].scope())
                .is_ok()
        );
        let _probe = Native::probe(
            &source,
            7,
            dive_stdlib::random::RandomMode::Dive,
            &budget,
            control,
        )
        .map_err(|_| "native probe")?;
        assert_eq!(captured.take(), [] as [dive_eval::capture::Captured; 0]);
        Ok(())
    }

    #[test]
    fn producer_output_bounds_cover_assumptions_init_and_native_steps() -> Result {
        let text = "---- MODULE M ----\nEXTENDS Naturals,TLC\nVARIABLE x\nASSUME PrintT(\"assume\")\nInit == x = 0 /\\ PrintT(\"init\")\nNext == x' = x + 1 /\\ PrintT(\"next\")\n====\n";
        for events in [0, 1, 2, 3] {
            let native_model = model(text, "INIT Init\nNEXT Next\n")?;
            let reference_model = model(text, "INIT Init\nNEXT Next\n")?;
            let mut options =
                dive_sim::Options::new(7, dive_stdlib::random::RandomMode::Dive, 2, 1);
            options.max_output_events = events;
            let native =
                prepare(native_model.model())?.simulate(native_model.model(), &options, &|| false);
            let reference = dive_sim::simulate(reference_model.model(), &options);
            assert!(matches!(native.outcome, Outcome::Limit(_)));
            assert!(matches!(reference.outcome, Outcome::Limit(_)));
            assert_eq!(native.runtime_output, reference.runtime_output);
            assert_eq!(native.initial_candidates, reference.initial_candidates);
            assert_eq!(native.generated, reference.generated);
        }
        Ok(())
    }

    #[test]
    fn print_argument_errors_precede_output_admission_limits() -> Result {
        let source = model(
            "---- MODULE M ----\nEXTENDS Integers,TLC\nVARIABLE x\nInit == x = 0 /\\ PrintT(1 \\div 0)\nNext == x' = x\n====\n",
            "INIT Init\nNEXT Next\n",
        )?;
        let mut options = dive_sim::Options::new(7, dive_stdlib::random::RandomMode::Dive, 1, 1);
        options.max_output_events = 0;
        let result = prepare(source.model())?.simulate(source.model(), &options, &|| false);
        assert!(matches!(result.outcome, Outcome::Error { code: 1000, .. }));
        assert_eq!(
            result.runtime_output,
            [] as [dive_eval::capture::Captured; 0]
        );
        Ok(())
    }

    #[test]
    fn an_unretained_initial_violation_keeps_pending_evidence_and_no_word() -> Result {
        let source = model(
            "---- MODULE M ----\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\nInv == x # 0\n====\n",
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        )?;
        let mut options = dive_sim::Options::new(7, dive_stdlib::random::RandomMode::Dive, 1, 1);
        options.max_retained = 0;
        let result = prepare(source.model())?.simulate(source.model(), &options, &|| false);
        assert!(matches!(result.outcome, Outcome::Limit(_)));
        assert_eq!(result.traces.len(), 0);
        assert_eq!(
            result.pending_violation.as_ref().map(|pending| (
                pending.kind,
                pending.name.as_deref(),
                pending.code
            )),
            Some(("invariant", Some("Inv"), 2107))
        );
        Ok(())
    }
}
