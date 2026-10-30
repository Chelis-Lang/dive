//! Optional native predicates with the existing reference stability certificate.
//!
//! This is deliberately narrower than general compiler acceptance. Successful
//! reference stability inspection, exact retained predicate object identity and
//! closed input handles are all required before replacing a reference call.

use std::sync::Arc;

use dive_check::budget::EvaluationControl;
use dive_core::{Handle, Machine, Opts, Store};
use dive_eval::{EvalError, HostControl};
use dive_liveness::analyze::AnalysisError;
use dive_liveness::analyze::periodic::external::{GraphCheck, NativeReader};
use dive_liveness::plan::{AtomTest, PredicateAtom};
use dive_model::Model;

use crate::core_native_graph::NativeGraph;
use crate::core_native_values::NativeValues;

struct Compiled {
    identity: *const PredicateAtom,
    plan: dive_core::plan::Plan,
}

pub(crate) struct NativeAtoms {
    machine: Machine,
    control: Arc<EvaluationControl>,
    compiled: Vec<Compiled>,
    eligible: usize,
    total: usize,
    evaluated: u64,
    fallback: u64,
    eligible_state: usize,
    eligible_action: usize,
    evaluated_state: u64,
    evaluated_action: u64,
}

impl NativeAtoms {
    pub(crate) fn prepare(
        model: &Model,
        store: &Arc<Store>,
        driver: &GraphCheck<'_>,
        control: &Arc<EvaluationControl>,
        strategies: Opts,
    ) -> Result<Self, dive_check::Verdict> {
        let opts = Opts {
            memo: false,
            step_memo: false,
            fold: false,
            ..strategies
        };
        let mut result = Self {
            machine: Machine::new(
                Arc::clone(store),
                model.ir.vars().len(),
                opts,
                dive_core::memo::Memo::default(),
            ),
            control: Arc::clone(control),
            compiled: Vec::new(),
            eligible: 0,
            total: 0,
            evaluated: 0,
            fallback: 0,
            eligible_state: 0,
            eligible_action: 0,
            evaluated_state: 0,
            evaluated_action: 0,
        };
        for atom in driver.native_predicates() {
            result.total += 1;
            if atom.test != AtomTest::Predicate {
                continue;
            }
            // This existing callback-free certificate checks actual scopes,
            // configured constants, cached bodies, intermediate constructors,
            // effects and name capabilities. Native compiler acceptance alone
            // does not provide that representation/history guarantee.
            let stable = match model.evaluator.closed_predicate_stability_budgeted(
                &atom.pred.scope,
                atom.pred.expr,
                Some(control.as_ref()),
            ) {
                Ok(stable) => stable,
                Err(error) => {
                    if let Some(verdict) = control.verdict(&error) {
                        return Err(verdict);
                    }
                    continue;
                }
            };
            if !stable {
                continue;
            }
            let Some(plan) = compile(model, store, atom, opts) else {
                continue;
            };
            if result.compiled.try_reserve(1).is_err() {
                continue;
            }
            result.compiled.push(Compiled {
                identity: std::ptr::from_ref(atom),
                plan,
            });
            result.eligible += 1;
            match atom.kind {
                dive_liveness::formula::AtomKind::State => result.eligible_state += 1,
                dive_liveness::formula::AtomKind::Action => result.eligible_action += 1,
            }
        }
        control.check().map_err(|cause| {
            control
                .verdict(&EvalError::interrupted(cause))
                .unwrap_or_else(|| dive_check::Verdict::InternalError {
                    message: "native predicate preparation interruption lacks run authority".into(),
                })
        })?;
        Ok(result)
    }

    pub(crate) fn metrics(&self) -> serde_json::Value {
        serde_json::json!({"total_atoms": self.total, "eligible_atoms": self.eligible,
            "evaluated": self.evaluated, "fallback": self.fallback,
            "eligible_state_atoms": self.eligible_state, "eligible_action_atoms": self.eligible_action,
            "state_evaluations": self.evaluated_state, "action_evaluations": self.evaluated_action,
            "eligibility": "closed function-tree inputs and source-scoped stable predicates with immutable interval domains"})
    }

    fn evaluate(
        &mut self,
        atom: &PredicateAtom,
        before: &[Handle],
        after: Option<&[Handle]>,
        control: &EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        check(&self.control, control)?;
        let Some(compiled) = self
            .compiled
            .iter()
            .find(|item| std::ptr::eq(item.identity, atom))
        else {
            self.fallback += 1;
            return Ok(None);
        };
        // The exact object came from this still-live driver's fixed plan Vec;
        // no expression ID, scope guess or extensional row equality is a key.
        if !closed(&self.machine.heap, before)
            || after.is_some_and(|row| !closed(&self.machine.heap, row))
        {
            self.fallback += 1;
            return Ok(None);
        }
        self.machine.s0.copy_from_slice(before);
        self.machine.s1.fill(Handle::EMPTY);
        if let Some(after) = after {
            self.machine.s1.copy_from_slice(after);
        }
        self.machine.set_fuel(4096);
        let predicate = &compiled.plan.implied_actions[0];
        let frame = self.machine.enter(predicate.frame);
        let result = predicate.conjuncts.iter().try_fold(true, |holds, part| {
            if holds {
                self.machine.eval_bool(&compiled.plan.code, part.p)
            } else {
                Ok(false)
            }
        });
        self.machine.leave(frame);
        self.machine.heap.reset();
        check(&self.control, control)?;
        if let Ok(value) = result {
            self.evaluated += 1;
            match atom.kind {
                dive_liveness::formula::AtomKind::State => self.evaluated_state += 1,
                dive_liveness::formula::AtomKind::Action => self.evaluated_action += 1,
            }
            Ok(Some(value))
        } else {
            self.fallback += 1;
            Ok(None)
        }
    }
}

fn compile(
    model: &Model,
    store: &Arc<Store>,
    atom: &PredicateAtom,
    opts: Opts,
) -> Option<dive_core::plan::Plan> {
    let Ok(mut handoff) = crate::core_engine::handoff(model, &model.ir, Vec::new()) else {
        return None;
    };
    let Ok(role) = crate::core_engine::role(
        model,
        "native temporal predicate".into(),
        atom.pred.expr,
        &atom.pred.scope,
    ) else {
        return None;
    };
    handoff.init.clear();
    handoff.next.clear();
    handoff.invariants.clear();
    handoff.constraints.clear();
    handoff.action_constraints.clear();
    handoff.implied_inits.clear();
    handoff.implied_actions = vec![role];
    handoff.view = None;
    handoff.check_deadlock = false;
    let Ok(plan) = dive_core::compile(&handoff, Arc::clone(store), opts) else {
        return None;
    };
    if plan.effects.any
        || plan.code.nodes.len() > 4096
        || !plan.code.funs.is_empty()
        || plan.code.nodes.iter().any(|node| {
            matches!(
                node,
                dive_core::plan::Node::Memo { .. }
                    | dive_core::plan::Node::Arith(dive_core::plan::Arith::Pow, _, _)
            )
        })
    {
        return None;
    }
    Some(plan)
}

fn check(owner: &EvaluationControl, control: &EvaluationControl) -> Result<(), AnalysisError> {
    owner
        .check()
        .and_then(|()| control.check())
        .map_err(|cause| {
            let error = EvalError::interrupted(cause);
            if !owner.owns(&error) {
                return AnalysisError::Inconsistent(
                    "native predicate interruption has a foreign owner",
                );
            }
            match control.verdict(&error) {
                Some(dive_check::Verdict::Stopped(stop)) => AnalysisError::Stopped(stop),
                Some(dive_check::Verdict::BudgetMeasurement { error }) => {
                    AnalysisError::BudgetMeasurement(error)
                }
                _ => {
                    AnalysisError::Inconsistent("native predicate interruption lacks run authority")
                }
            }
        })
}

fn closed(heap: &dive_core::Heap, roots: &[Handle]) -> bool {
    let mut pending = Vec::new();
    if pending.try_reserve(roots.len()).is_err() {
        return false;
    }
    pending.extend_from_slice(roots);
    let mut work = 0;
    while let Some(handle) = pending.pop() {
        work += 1;
        if work > 4096 || handle.is_empty() || handle.is_transient() {
            return false;
        }
        if handle.as_bool().is_some() || handle.as_int().is_some() || handle.as_str_id().is_some() {
            continue;
        }
        if !handle.is_compound() {
            return false;
        }
        let view = heap.view(handle);
        if !matches!(
            view.kind,
            dive_core::node::Kind::IntFcn | dive_core::node::Kind::Fcn
        ) {
            return false;
        }
        if view.kind == dive_core::node::Kind::Fcn
            && view.keys().iter().any(|key| {
                key.as_int().is_none() && key.as_bool().is_none() && key.as_str_id().is_none()
            })
        {
            return false;
        }
        if view.values().len() > 4096 - work
            || pending.len() + view.values().len() > 4096
            || pending.try_reserve(view.values().len()).is_err()
        {
            return false;
        }
        pending.extend_from_slice(view.values());
    }
    !heap.poisoned()
}

pub(crate) struct Reader<'a, 'model> {
    pub(crate) graph: &'a NativeGraph,
    pub(crate) values: &'a mut NativeValues<'model>,
    pub(crate) atoms: &'a mut NativeAtoms,
}

impl NativeReader for Reader<'_, '_> {
    fn has_predicate(&self, atom: &PredicateAtom) -> bool {
        self.atoms
            .compiled
            .iter()
            .any(|item| std::ptr::eq(item.identity, atom))
    }

    fn read(
        &mut self,
        row: usize,
        control: &EvaluationControl,
    ) -> Result<dive_eval::next::StateVals, AnalysisError> {
        self.values.read(row, self.graph.row(row)?, control)
    }
    fn predicate(
        &mut self,
        atom: &PredicateAtom,
        before: usize,
        after: Option<usize>,
        control: &EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        self.atoms.evaluate(
            atom,
            self.graph.row(before)?,
            after.map(|row| self.graph.row(row)).transpose()?,
            control,
        )
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "closed input and actual budget controls"
)]
mod tests {
    use super::*;
    use dive_check::budget::{Budget, Phase};

    fn prepare_source(predicate: &str) -> (crate::core_engine::Prepared, dive_check::CheckOptions) {
        let source = format!(
            "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nP == <>({predicate})\n====\n"
        );
        let analysis =
            dive_sema::analyze_source("Atoms.tla", &source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\nCHECK_DEADLOCK FALSE\n")
            .into_config()
            .expect("config");
        let mut options = dive_check::CheckOptions::dive(0);
        options.budget = Budget::unlimited();
        let prepared =
            crate::core_engine::prepare(&ir, &config, &options, false, Opts::default(), true)
                .expect("fresh source-bound core");
        (prepared, options)
    }

    #[test]
    fn only_the_actual_retained_atom_executes_and_semantic_errors_keep_reference_fallback() {
        use dive_liveness::analyze::{GraphLabelPolicy, Limits};
        use dive_liveness::product::AcceptancePolicy;
        for (predicate, expected) in [("x>=0", Some(true)), ("x \\div 0 = 0", None)] {
            let (prepared, options) = prepare_source(predicate);
            let driver = GraphCheck::prepare(
                &prepared.model,
                &options,
                Limits::default(),
                AcceptancePolicy::compact_default(),
                GraphLabelPolicy::NoCellLimit,
                None,
            )
            .expect("driver");
            let control = EvaluationControl::new(&options.budget, Phase::Capture).expect("control");
            let mut atoms = NativeAtoms::prepare(
                &prepared.model,
                &prepared.store,
                &driver,
                &control,
                Opts::default(),
            )
            .expect("prepare");
            assert_eq!(atoms.eligible, 1);
            let atom = driver.native_predicates().next().expect("atom");
            let before = prepared.store.stats();
            assert_eq!(
                atoms
                    .evaluate(atom, &[Handle::int(0).expect("zero")], None, &control)
                    .expect("original evaluation"),
                expected
            );
            let after = prepared.store.stats();
            assert_eq!(
                (after.nodes, after.handles),
                (before.nodes, before.handles),
                "analysis does not intern into the producer store"
            );
            assert_eq!(
                atoms
                    .evaluate(
                        &atom.clone(),
                        &[Handle::int(0).expect("zero")],
                        None,
                        &control
                    )
                    .expect("copied descriptor"),
                None,
                "copied source descriptors supply no binding authority"
            );
            let (other, _) = prepare_source(predicate);
            let foreign = NativeAtoms::prepare(
                &other.model,
                &other.store,
                &driver,
                &control,
                Opts::default(),
            )
            .expect("foreign source declines");
            assert_eq!(foreign.eligible, 0);
        }
    }

    #[test]
    fn exact_closed_functions_admit_scalar_keys_and_refuse_sets_and_models_without_poisoning() {
        let store = Arc::new(Store::default());
        let mut heap = dive_core::Heap::new(store.clone(), false, false);
        let zero = Handle::int(0).expect("integer");
        let function = heap.int_fcn(0, &[zero, Handle::bool(true)]);
        let function = heap.intern_exact(function).expect("persistent function");
        assert!(closed(&heap, &[function]));
        let set = heap.set_from(&mut vec![zero]);
        let set = heap.intern_exact(set).expect("persistent set");
        assert!(!closed(&heap, &[set]));
        let model = Handle::model(store.strings().intern("model").expect("name"));
        assert!(!closed(&heap, &[model]));
        assert!(
            !heap.poisoned(),
            "refused scalar classes do not inspect invalid compound handles"
        );
        let transient = heap.tuple(&[zero]);
        assert!(!closed(&heap, &[transient]));
        let tuple = heap.intern_exact(transient).expect("persistent tuple");
        let aggregate_key = heap.fcn_sorted(&[tuple], &[zero]);
        let aggregate_key = heap
            .intern_exact(aggregate_key)
            .expect("persistent function");
        assert!(!closed(&heap, &[aggregate_key]));
        assert!(!heap.poisoned());
    }

    #[test]
    fn actual_owned_stops_keep_the_analysis_phase_and_foreign_stops_cannot_be_adopted() {
        let watchdog = Budget::new(Some(std::time::Duration::ZERO), None)
            .start()
            .expect("watchdog")
            .expect("finite");
        let owner = EvaluationControl::new(watchdog.budget(), Phase::Capture).expect("owner");
        let temporal = EvaluationControl::new(watchdog.budget(), Phase::Temporal).expect("phase");
        assert!(
            matches!(check(&owner, &temporal), Err(AnalysisError::Stopped(ref stop)) if stop.phase == Phase::Temporal)
        );
        let unrelated =
            EvaluationControl::new(&Budget::unlimited(), Phase::Capture).expect("unrelated");
        assert!(matches!(
            check(&unrelated, &temporal),
            Err(AnalysisError::Inconsistent(_))
        ));
        assert!(matches!(
            check(&owner, &unrelated),
            Err(AnalysisError::Inconsistent(_))
        ));
    }
}
