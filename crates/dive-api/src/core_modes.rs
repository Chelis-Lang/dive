//! Finite execution over the existing compiled plan and value machine.
//!
//! Search order belongs to each mode; generation and predicate evaluation retain
//! the same plan/frame rules as the BFS core. Report conversion is a boundary.

pub(crate) mod dfid;
mod sample;
#[cfg(test)]
mod sim_tests;

use std::sync::Arc;

use dive_core::generate::Sink;
use dive_core::memo::Memo;
use dive_core::plan::{Action, Check, Code, Plan};
use dive_core::{Error, ErrorKind, Handle, Machine, Opts};
use dive_eval::next::StateVals;
use dive_model::Model;
use dive_sim::{Outcome, Report};

pub(crate) enum SetupError {
    Unsupported(String),
    Internal(String),
    Stopped,
}

impl SetupError {
    pub(crate) fn dfid(self) -> crate::dfid_admission::Outcome {
        match self {
            Self::Unsupported(message) => crate::dfid_admission::Outcome::Unsupported(message),
            Self::Internal(message) => crate::dfid_admission::Outcome::Internal(message),
            Self::Stopped => crate::dfid_admission::Outcome::Cancelled,
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
    budget: dive_runtime::budget::Budget,
}

impl Native {
    pub(crate) fn probe(
        model: &crate::trace::SpecModel,
        seed: i64,
        mode: dive_stdlib::random::RandomMode,
        budget: &dive_runtime::budget::Budget,
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
        budget: &dive_runtime::budget::Budget,
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
        Self::prepare_with_options(model, budget, opts, 1)
    }

    fn prepare_with_options(
        model: &Model,
        budget: &dive_runtime::budget::Budget,
        opts: Opts,
        workers: usize,
    ) -> Result<Self, SetupError> {
        let opts = Opts {
            memo_cap: crate::core_engine::memo_cap(budget.memory, workers),
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

/// Runs `work` with `TLCGet("level")` at `level`, then restores the level
/// generation continues at.
fn at_level<T>(machine: &mut Machine, level: i64, work: impl FnOnce(&mut Machine) -> T) -> T {
    let previous = std::mem::replace(&mut machine.level, level);
    let result = work(machine);
    machine.level = previous;
    result
}

/// A trace position (1-based) as a `TLCGet("level")`.
fn level_of(position: usize) -> i64 {
    i64::try_from(position).unwrap_or(i64::MAX)
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
        sampled_sets: Vec::new(),
        traces_stalled: 0,
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

/// The first implied initial predicate (`PROPERTY` part) the initial state in
/// `machine.s1` violates, as `dive check` reports it (2108).
fn implied_initial(machine: &mut Machine, plan: &Plan) -> Option<Outcome> {
    for check in &plan.implied_inits {
        match predicate(machine, &plan.code, check, false) {
            Ok(true) => {}
            Ok(false) => {
                return Some(Outcome::Violation {
                    kind: "property",
                    name: Some(check.name.clone()),
                    code: 2108,
                    trace: 0,
                });
            }
            Err(error) => return Some(classify(&error, None)),
        }
    }
    None
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
        Native::prepare(model, &dive_runtime::budget::Budget::unlimited())
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
            let report = native.simulate(
                source.model(),
                &options,
                &dive_sim::Runtime::new(options.clone()),
                &|| false,
            );
            assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
            assert_eq!(report.generated, 13);
            assert_eq!(report.traces.len(), 3);
            assert!(report.traces.iter().all(|trace| trace.states.len() == 5));
            options.max_generated = 3;
            let mut native = prepare(source.model())?;
            let stopped = native.simulate(
                source.model(),
                &options,
                &dive_sim::Runtime::new(options.clone()),
                &|| false,
            );
            assert!(matches!(stopped.outcome, Outcome::Limit(_)), "{stopped:?}");
            // Trace 0's four successors would cross the bound of three
            // after Init's one state, so only Init is counted.
            assert_eq!(stopped.generated, 1);
        }
        Ok(())
    }

    #[test]
    fn feasibility_runtime_owns_fresh_scopes_and_does_not_evaluate_assumptions() -> Result {
        let source = model(
            "---- MODULE M ----\nEXTENDS Naturals,TLC\nVARIABLE x\nASSUME PrintT(\"assume\")\nInit == x = 0\nNext == x' = x + 1\n====\n",
            "INIT Init\nNEXT Next\n",
        )?;
        let budget = dive_runtime::budget::Budget::unlimited();
        let control = dive_runtime::budget::EvaluationControl::new(
            &budget,
            dive_runtime::budget::Phase::Simulation,
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
        // The figures the retired reference simulator reported (#791): the
        // printed lines, initial candidates and generated states at each bound.
        let pinned: [(&[&str], u64, u64); 4] = [
            (&[], 0, 0),
            (&["\"assume\""], 0, 0),
            (&["\"assume\"", "\"init\""], 1, 1),
            (&["\"assume\"", "\"init\"", "\"next\""], 1, 2),
        ];
        for (events, (lines, initial, generated)) in pinned.into_iter().enumerate() {
            let native_model = model(text, "INIT Init\nNEXT Next\n")?;
            let mut options =
                dive_sim::Options::new(7, dive_stdlib::random::RandomMode::Dive, 2, 1);
            options.max_output_events = events;
            let native = prepare(native_model.model())?.simulate(
                native_model.model(),
                &options,
                &dive_sim::Runtime::new(options.clone()),
                &|| false,
            );
            assert!(matches!(native.outcome, Outcome::Limit(_)));
            let printed = native
                .runtime_output
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>();
            assert_eq!(printed, lines);
            assert!(native.runtime_output.iter().all(|line| !line.warning));
            assert_eq!(native.initial_candidates, initial);
            assert_eq!(native.generated, generated);
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
        let result = prepare(source.model())?.simulate(
            source.model(),
            &options,
            &dive_sim::Runtime::new(options.clone()),
            &|| false,
        );
        assert!(matches!(result.outcome, Outcome::Error { code: 1000, .. }));
        assert_eq!(
            result.runtime_output,
            [] as [dive_eval::capture::Captured; 0]
        );
        Ok(())
    }

    const LEVELS: &str = "---- MODULE M ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == PrintT(<<\"init\", TLCGet(\"level\")>>) /\\ x = 0\nNext == PrintT(<<\"next\", x, TLCGet(\"level\")>>) /\\ x' = x + 1\nConstr == PrintT(<<\"constr\", x, TLCGet(\"level\")>>)\nActConstr == PrintT(<<\"actconstr\", x, x', TLCGet(\"level\")>>)\nInv == PrintT(<<\"inv\", x, TLCGet(\"level\")>>) /\\ x < 2\nProp == [][PrintT(<<\"actprop\", x, x', TLCGet(\"level\")>>)]_x\n====\n";

    fn printed(report: &Report) -> Vec<&str> {
        report
            .runtime_output
            .iter()
            .map(|line| line.text.as_str())
            .collect()
    }

    /// The transcripts are the pinned TLC's (`-simulate num=1 -depth 20
    /// -seed 0 -workers 1`): 0 in Init, a state's trace position in its
    /// state checks, the source's in Next and in action checks.
    #[test]
    fn simulation_unprimed_tlcget_level_follows_tlc_in_each_role() -> Result {
        let cases: [(&str, &[&str]); 2] = [
            (
                "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Prop\n",
                &[
                    "<<\"init\", 0>>",
                    "<<\"inv\", 0, 1>>",
                    "<<\"next\", 0, 1>>",
                    "<<\"inv\", 1, 2>>",
                    "<<\"actprop\", 0, 1, 1>>",
                    "<<\"next\", 1, 2>>",
                    "<<\"inv\", 2, 3>>",
                ],
            ),
            (
                "INIT Init\nNEXT Next\nCONSTRAINT Constr\nACTION_CONSTRAINT ActConstr\nINVARIANT Inv\n",
                &[
                    "<<\"init\", 0>>",
                    "<<\"inv\", 0, 1>>",
                    "<<\"constr\", 0, 1>>",
                    "<<\"next\", 0, 1>>",
                    "<<\"inv\", 1, 2>>",
                    "<<\"constr\", 1, 2>>",
                    "<<\"actconstr\", 0, 1, 1>>",
                    "<<\"next\", 1, 2>>",
                    "<<\"inv\", 2, 3>>",
                ],
            ),
        ];
        for (config, expected) in cases {
            let source = model(LEVELS, config)?;
            let mut native = prepare(source.model())?;
            let options = dive_sim::Options::new(0, dive_stdlib::random::RandomMode::Dive, 20, 1);
            let report = native.simulate(
                source.model(),
                &options,
                &dive_sim::Runtime::new(options.clone()),
                &|| false,
            );
            assert!(
                matches!(&report.outcome, Outcome::Violation { name: Some(name), .. } if name == "Inv"),
                "{report:?}"
            );
            assert_eq!(printed(&report), expected);
            assert_eq!(report.traces[0].states.len(), 3);
        }
        Ok(())
    }

    /// TLC evaluates a primed `TLCGet("level")` at the successor's level
    /// (`-simulate` prints `<<0, 1, 1, 2>>` for
    /// `<<x, x', TLCGet("level"), (TLCGet("level"))'>>` in an action
    /// property); Core refuses it by name rather than reading the source's.
    #[test]
    fn simulation_refuses_a_primed_tlcget_level() -> Result {
        let cases = [
            (
                "Init == x = 0\nNext == x' = x + 1\nProp == [][(TLCGet(\"level\"))' = TLCGet(\"level\") + 1]_x\nInv == x < 5\n",
                "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Prop\n",
            ),
            (
                "Init == x = 1\nNext == x' = (TLCGet(\"level\"))'\nInv == x < 3\n",
                "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            ),
            (
                "L == TLCGet(\"level\")\nInit == x = 0\nNext == x' = x + 1\nAC == L' > L\nInv == x < 5\n",
                "INIT Init\nNEXT Next\nACTION_CONSTRAINT AC\nINVARIANT Inv\n",
            ),
        ];
        for (body, config) in cases {
            let source = model(
                &format!("---- MODULE M ----\nEXTENDS Naturals, TLC\nVARIABLE x\n{body}====\n"),
                config,
            )?;
            let refused =
                Native::prepare(source.model(), &dive_runtime::budget::Budget::unlimited());
            assert!(
                matches!(&refused, Err(SetupError::Unsupported(why)) if why.contains("TLCGet(\"level\") under a prime")),
                "{body}: {}",
                match &refused {
                    Ok(_) => "prepared",
                    Err(SetupError::Unsupported(why) | SetupError::Internal(why)) => why,
                    Err(SetupError::Stopped) => "stopped",
                }
            );
        }
        Ok(())
    }

    #[test]
    fn simulation_keeps_refusing_reached_tlc_registers() -> Result {
        let source = model(
            "---- MODULE M ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nInv == TLCGet(1) = 0\n====\n",
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        )?;
        let options = dive_sim::Options::new(0, dive_stdlib::random::RandomMode::Dive, 3, 1);
        let report = prepare(source.model())?.simulate(
            source.model(),
            &options,
            &dive_sim::Runtime::new(options.clone()),
            &|| false,
        );
        assert!(
            matches!(&report.outcome, Outcome::Unsupported(why) if why.contains("TLCGet")),
            "{report:?}"
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
        let result = prepare(source.model())?.simulate(
            source.model(),
            &options,
            &dive_sim::Runtime::new(options.clone()),
            &|| false,
        );
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
