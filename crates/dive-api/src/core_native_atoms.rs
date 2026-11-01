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

#[derive(Debug)]
struct QueryControl(Arc<EvaluationControl>);
impl dive_core::eval::WorkControl for QueryControl {
    fn proceed(&self) -> bool {
        self.0.check().is_ok()
    }
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
    evaluated_enabled: u64,
    evaluated_nonstuttering: u64,
    row_proofs: RowProofMetrics,
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
            evaluated_enabled: 0,
            evaluated_nonstuttering: 0,
            row_proofs: RowProofMetrics::default(),
        };
        result
            .machine
            .set_work_control(Some(Arc::new(QueryControl(Arc::clone(control)))));
        for atom in driver.native_predicates() {
            result.total += 1;
            if matches!(atom.test, AtomTest::NegatedEnabled { .. }) {
                continue;
            }
            // This existing callback-free certificate checks actual scopes,
            // configured constants, cached bodies, intermediate constructors,
            // effects and name capabilities. Native compiler acceptance alone
            // does not provide that representation/history guarantee.
            let inspected = stable(model, atom, control);
            let stable = match inspected {
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
            let Some(plan) = compile(model, store, atom, opts, &result.machine.heap) else {
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
            "enabled_evaluations": self.evaluated_enabled, "nonstuttering_evaluations": self.evaluated_nonstuttering,
            "row_proofs": {"scans": self.row_proofs.scans, "hits": self.row_proofs.hits,
                "certified": self.row_proofs.certified, "peak_storage_bytes": self.row_proofs.peak_storage_bytes},
            "eligibility": "source-scoped closed ordinary constructions and scalar updates; identity-scoped fairness queries"})
    }

    #[cfg(test)]
    fn evaluate(
        &mut self,
        atom: &PredicateAtom,
        before: &[Handle],
        after: Option<&[Handle]>,
        control: &EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        self.evaluate_with(atom, before, after, control, |heap| {
            Ok(closed(heap, before) && after.is_none_or(|row| closed(heap, row)))
        })
    }

    fn evaluate_with(
        &mut self,
        atom: &PredicateAtom,
        before: &[Handle],
        after: Option<&[Handle]>,
        control: &EvaluationControl,
        inputs_closed: impl FnOnce(&dive_core::Heap) -> Result<bool, AnalysisError>,
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
        if !inputs_closed(&self.machine.heap)? {
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
        self.machine.warnings.clear();
        self.machine.printed.clear();
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
        if let Ok(value) = result
            && self.machine.warnings.is_empty()
            && self.machine.printed.is_empty()
        {
            self.evaluated += 1;
            match atom.test {
                AtomTest::Enabled { .. } => self.evaluated_enabled += 1,
                AtomTest::Nonstuttering { .. } => self.evaluated_nonstuttering += 1,
                _ => {}
            }
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

fn stable(
    model: &Model,
    atom: &PredicateAtom,
    control: &EvaluationControl,
) -> Result<bool, EvalError> {
    let ev = &model.evaluator;
    let scoped = &atom.pred.scope;
    match atom.test {
        AtomTest::Predicate => {
            ev.guarded_action_stability_budgeted(scoped, atom.pred.expr, Some(control))
        }
        AtomTest::Enabled { sub } => {
            if !ev.guarded_action_stability_budgeted(scoped, atom.pred.expr, Some(control))? {
                return Ok(false);
            }
            sub.map_or(Ok(true), |sub| {
                ev.closed_predicate_stability_budgeted(scoped, sub, Some(control))
            })
        }
        AtomTest::Nonstuttering { sub } => {
            Ok(
                ev.guarded_action_stability_budgeted(scoped, atom.pred.expr, Some(control))?
                    && ev.guarded_action_stability_budgeted(scoped, sub, Some(control))?,
            )
        }
        AtomTest::NegatedEnabled { .. } => Ok(false),
    }
}

fn compile(
    model: &Model,
    store: &Arc<Store>,
    atom: &PredicateAtom,
    opts: Opts,
    heap: &dive_core::Heap,
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
    let plan = match atom.test {
        AtomTest::Predicate => dive_core::compile::compile_bound_test(
            &handoff,
            Arc::clone(store),
            opts,
            dive_core::compile::BoundTest::Predicate,
        ),
        AtomTest::Enabled { sub } => dive_core::compile::compile_bound_test(
            &handoff,
            Arc::clone(store),
            opts,
            dive_core::compile::BoundTest::Enabled(sub),
        ),
        AtomTest::Nonstuttering { sub } => dive_core::compile::compile_bound_test(
            &handoff,
            Arc::clone(store),
            opts,
            dive_core::compile::BoundTest::Nonstuttering(sub),
        ),
        AtomTest::NegatedEnabled { .. } => return None,
    }
    .ok()?;
    if plan.effects.any
        || plan.code.nodes.len() > 4096
        || plan.code.gnodes.len() > 4096
        || !plan.code.funs.is_empty()
        || plan.code.nodes.iter().any(|node| {
            let value = match node {
                dive_core::plan::Node::Const(value)
                    if value.as_str_id().is_some()
                        || value.is_compound()
                            && matches!(
                                heap.view(*value).kind,
                                dive_core::node::Kind::IntFcn | dive_core::node::Kind::Fcn
                            ) =>
                {
                    Some(*value)
                }
                dive_core::plan::Node::Select(_, key) => Some(*key),
                _ => None,
            };
            value.is_some_and(|value| !closed(heap, &[value]))
        })
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

fn string_payload(heap: &dive_core::Heap, value: Handle, work: &mut usize) -> bool {
    let Some(id) = value.as_str_id() else {
        return true;
    };
    let Some(text) = heap.store().strings().text(id) else {
        return false;
    };
    if text.len() > 4096usize.saturating_sub(*work) {
        return false;
    }
    *work += text.len();
    true
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
        if handle.as_str_id().is_some() {
            if !string_payload(heap, handle, &mut work) {
                return false;
            }
            continue;
        }
        if handle.as_bool().is_some() || handle.as_int().is_some() {
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
        if view
            .keys()
            .iter()
            .any(|&key| !string_payload(heap, key, &mut work))
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

// A positive fact is scoped by an immutable graph borrow, never by a handle's
// bits across Stores or by extensional row equality. It is issued only by the
// unchanged closed() check, independently for each actual row demand (ADR 0134).
const MAX_ROW_PROOF_BYTES: usize = 1024 * 1024;

#[derive(Default)]
struct RowProofMetrics {
    scans: u64,
    hits: u64,
    certified: u64,
    peak_storage_bytes: usize,
}

fn row_proof_storage(rows: usize, reserve: impl FnOnce(&mut Vec<u8>, usize) -> bool) -> Vec<u8> {
    let Some(bytes) = rows.checked_add(7).map(|count| count / 8) else {
        return Vec::new();
    };
    let mut bits = Vec::new();
    if bytes > MAX_ROW_PROOF_BYTES
        || !reserve(&mut bits, bytes)
        || bits.capacity() > MAX_ROW_PROOF_BYTES
    {
        return Vec::new();
    }
    bits.resize(bytes, 0);
    bits
}

struct RowProofs<'a> {
    graph: &'a NativeGraph,
    bits: Vec<u8>,
    initialized: bool,
    metrics: RowProofMetrics,
}

impl<'a> RowProofs<'a> {
    fn new(graph: &'a NativeGraph) -> Self {
        Self {
            graph,
            bits: Vec::new(),
            initialized: false,
            metrics: RowProofMetrics::default(),
        }
    }

    fn closed(&mut self, heap: &dive_core::Heap, row: usize) -> Result<bool, AnalysisError> {
        let roots = self.graph.row(row)?;
        // Persistent coordinates do not identify their issuing Store. Current
        // poison is deliberately not cached, including after an earlier success.
        if !self.graph.owns_store(heap.store()) || heap.poisoned() {
            return Ok(false);
        }
        if !self.initialized {
            self.bits = row_proof_storage(self.graph.input().retained_rows, |bits, bytes| {
                bits.try_reserve_exact(bytes).is_ok()
            });
            self.initialized = true;
            self.metrics.peak_storage_bytes = self.bits.capacity();
        }
        let bit = 1 << (row % 8);
        if self.bits.get(row / 8).is_some_and(|byte| byte & bit != 0) {
            self.metrics.hits += 1;
            return Ok(true);
        }
        self.metrics.scans += 1;
        let success = closed(heap, roots);
        if success && let Some(byte) = self.bits.get_mut(row / 8) {
            *byte |= bit;
            self.metrics.certified += 1;
        }
        Ok(success)
    }
}

pub(crate) struct Reader<'a, 'model> {
    proofs: RowProofs<'a>,
    pub(crate) values: &'a mut NativeValues<'model>,
    atoms: &'a mut NativeAtoms,
}

impl<'a, 'model> Reader<'a, 'model> {
    pub(crate) fn new(
        graph: &'a NativeGraph,
        values: &'a mut NativeValues<'model>,
        atoms: &'a mut NativeAtoms,
    ) -> Self {
        Self {
            proofs: RowProofs::new(graph),
            values,
            atoms,
        }
    }
}

impl Drop for Reader<'_, '_> {
    fn drop(&mut self) {
        let total = &mut self.atoms.row_proofs;
        total.scans += self.proofs.metrics.scans;
        total.hits += self.proofs.metrics.hits;
        total.certified += self.proofs.metrics.certified;
        total.peak_storage_bytes = total
            .peak_storage_bytes
            .max(self.proofs.metrics.peak_storage_bytes);
    }
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
        self.values.read(row, self.proofs.graph.row(row)?, control)
    }
    fn predicate(
        &mut self,
        atom: &PredicateAtom,
        before: usize,
        after: Option<usize>,
        control: &EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        let graph = self.proofs.graph;
        self.atoms.evaluate_with(
            atom,
            graph.row(before)?,
            after.map(|row| graph.row(row)).transpose()?,
            control,
            |heap| {
                if !self.proofs.closed(heap, before)? {
                    return Ok(false);
                }
                after.map_or(Ok(true), |row| self.proofs.closed(heap, row))
            },
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
        prepare_text(
            &source,
            "INIT Init\nNEXT Next\nPROPERTY P\nCHECK_DEADLOCK FALSE\n",
            false,
        )
    }

    fn prepare_text(
        source: &str,
        config: &str,
        tlc: bool,
    ) -> (crate::core_engine::Prepared, dive_check::CheckOptions) {
        let analysis =
            dive_sema::analyze_source("Atoms.tla", source, &dive_sema::Options::default());
        assert!(analysis.accepted(), "{analysis:?}");
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let config = dive_config::parse(config).into_config().expect("config");
        let mut options = if tlc {
            dive_check::CheckOptions::tlc(0, 0)
        } else {
            dive_check::CheckOptions::dive(0)
        };
        options.budget = Budget::unlimited();
        let prepared =
            crate::core_engine::prepare(&ir, &config, &options, tlc, Opts::default(), true)
                .expect("fresh source-bound core");
        (prepared, options)
    }

    fn with_wrappers(
        source: &str,
        tlc: bool,
        test: impl FnOnce(
            &crate::core_engine::Prepared,
            &GraphCheck<'_>,
            &mut NativeAtoms,
            &EvaluationControl,
        ),
    ) {
        use dive_liveness::analyze::{GraphLabelPolicy, Limits};
        use dive_liveness::product::AcceptancePolicy;
        let (prepared, options) = prepare_text(
            source,
            "SPECIFICATION Spec\nPROPERTY P\nCHECK_DEADLOCK FALSE\n",
            tlc,
        );
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
        .expect("atoms");
        test(&prepared, &driver, &mut atoms, &control);
    }

    fn capture<'a>(
        prepared: &'a crate::core_engine::Prepared,
        options: &dive_check::CheckOptions,
    ) -> (NativeGraph, NativeValues<'a>) {
        let mut sink = crate::core_native_graph::NativeSink::new(
            &prepared.model,
            prepared.store.clone(),
            dive_check::capture::Limits::default(),
            options,
            None,
        )
        .expect("sink");
        let result = dive_explore::explore_with(
            &prepared.plan,
            &prepared.store,
            dive_explore::Config {
                opts: Opts::default(),
                workers: 1,
                warn: true,
                max_states: None,
                max_depth: None,
                sample: None,
            },
            &|| false,
            Some(&mut sink),
        );
        assert_eq!(result.verdict, dive_explore::Verdict::Success);
        assert!(sink.take_error().is_none());
        sink.finish()
    }

    #[test]
    fn row_proof_storage_bounds_overflow_and_allocation_failure() {
        let allocate = |bits: &mut Vec<u8>, bytes| bits.try_reserve_exact(bytes).is_ok();
        assert_eq!(row_proof_storage(9, allocate), vec![0; 2]);
        assert_eq!(
            row_proof_storage(MAX_ROW_PROOF_BYTES * 8, allocate).len(),
            MAX_ROW_PROOF_BYTES
        );
        for rows in [MAX_ROW_PROOF_BYTES * 8 + 1, usize::MAX] {
            let mut attempted = false;
            assert_eq!(
                row_proof_storage(rows, |_, _| {
                    attempted = true;
                    false
                }),
                [] as [u8; 0]
            );
            assert!(!attempted, "over-cap storage never asks the allocator");
        }
        let mut attempted = false;
        assert_eq!(
            row_proof_storage(8, |_, _| {
                attempted = true;
                false
            }),
            [] as [u8; 0]
        );
        assert!(attempted);
    }

    #[test]
    fn row_proofs_are_positive_only_exact_and_bound_to_the_graph_store() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = <<0>> \\/ x={0}\nNext == UNCHANGED x\nP == <> (x=x)\n====\n";
        for tlc in [false, true] {
            let (prepared, options) =
                prepare_text(source, "INIT Init\nNEXT Next\nPROPERTY P\n", tlc);
            let (graph, _) = capture(&prepared, &options);
            let heap = dive_core::Heap::new(prepared.store.clone(), false, false);
            let rows = graph.input().retained_rows;
            assert_eq!(rows, 2);
            let mut proofs = RowProofs::new(&graph);
            assert!(!proofs.initialized);
            for row in 0..rows {
                let expected = closed(&heap, graph.row(row).expect("retained"));
                let scans = proofs.metrics.scans;
                let hits = proofs.metrics.hits;
                assert_eq!(proofs.closed(&heap, row).expect("first"), expected);
                assert_eq!(proofs.closed(&heap, row).expect("again"), expected);
                assert_eq!(proofs.metrics.scans - scans, if expected { 1 } else { 2 });
                assert_eq!(proofs.metrics.hits - hits, u64::from(expected));
            }
            assert!(matches!(
                proofs.closed(&heap, rows),
                Err(AnalysisError::Inconsistent(_))
            ));
            assert!(!heap.poisoned());
            let foreign_heap = dive_core::Heap::new(Arc::new(Store::default()), false, false);
            for row in 0..rows {
                assert!(
                    !proofs
                        .closed(&foreign_heap, row)
                        .expect("foreign Store declines")
                );
            }
            assert!(
                !foreign_heap.poisoned(),
                "foreign coordinates never resolve"
            );
            let mut fresh = RowProofs::new(&graph);
            assert_eq!(fresh.bits, [] as [u8; 0]);
            for row in 0..rows {
                let _ = fresh.closed(&heap, row).expect("fresh analysis");
            }
            assert_eq!(fresh.metrics.hits, 0);
            assert_eq!(fresh.metrics.scans, 2);
            let _ = heap.view(Handle::EMPTY);
            assert!(heap.poisoned());
            for row in 0..rows {
                assert!(!proofs.closed(&heap, row).expect("current poison declines"));
            }
        }
    }

    #[test]
    fn disabled_proof_storage_keeps_the_original_closed_scan_on_every_demand() {
        for tlc in [false, true] {
            let (prepared, options) = prepare_text(
                "---- MODULE Atoms ----\nVARIABLE x\nInit == x = <<0>>\nNext == UNCHANGED x\nP == <> (x=x)\n====\n",
                "INIT Init\nNEXT Next\nPROPERTY P\n",
                tlc,
            );
            let (graph, _) = capture(&prepared, &options);
            let heap = dive_core::Heap::new(prepared.store.clone(), false, false);
            let mut proofs = RowProofs::new(&graph);
            proofs.bits = row_proof_storage(1, |_, _| false);
            proofs.initialized = true;
            assert!(proofs.closed(&heap, 0).expect("allocation bypass"));
            assert!(proofs.closed(&heap, 0).expect("uncached again"));
            assert_eq!(proofs.metrics.scans, 2);
            assert_eq!(proofs.metrics.hits, 0);
            assert_eq!(proofs.metrics.certified, 0);
        }
    }

    #[test]
    fn actual_reader_reuses_independent_source_target_proofs_and_rejects_copied_atoms() {
        let tuple = vec!["0"; 2200].join(",");
        let source = format!(
            "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x = <<{tuple}>> /\\ y=0\nA == UNCHANGED x /\\ y'=1-y\nSpec == Init /\\ [][A]_<<x,y>> /\\ WF_y(A)\nP == <> (y=1)\n====\n"
        );
        for tlc in [false, true] {
            with_wrappers(&source, tlc, |prepared, driver, atoms, control| {
                let options = if tlc {
                    dive_check::CheckOptions::tlc(0, 0)
                } else {
                    dive_check::CheckOptions::dive(0)
                };
                let (graph, mut values) = capture(prepared, &options);
                assert_eq!(graph.input().retained_rows, 2);
                let atom = driver
                    .native_predicates()
                    .find(|atom| matches!(atom.test, AtomTest::Nonstuttering { .. }))
                    .expect("nonstuttering");
                let mut reader = Reader::new(&graph, &mut values, atoms);
                assert_eq!(
                    reader
                        .predicate(&atom.clone(), 0, Some(1), control)
                        .expect("copied"),
                    None
                );
                assert!(
                    !reader.proofs.initialized,
                    "unretained atom cannot issue proofs"
                );
                assert_eq!(
                    reader
                        .predicate(atom, 0, Some(1), control)
                        .expect("separate quotas"),
                    Some(true)
                );
                assert_eq!(reader.proofs.metrics.scans, 2);
                assert_eq!(reader.proofs.metrics.certified, 2);
                assert_eq!(
                    reader.predicate(atom, 0, Some(1), control).expect("reuse"),
                    Some(true)
                );
                assert_eq!(reader.proofs.metrics.scans, 2);
                assert_eq!(reader.proofs.metrics.hits, 2);
                let stats = prepared.store.stats();
                assert_eq!(
                    reader
                        .predicate(atom, 0, Some(0), control)
                        .expect("stutter"),
                    Some(false)
                );
                assert_eq!(reader.proofs.metrics.hits, 4);
                let state_atom = driver
                    .native_predicates()
                    .find(|atom| matches!(atom.test, AtomTest::Predicate))
                    .expect("state predicate");
                assert_eq!(
                    reader
                        .predicate(state_atom, 0, None, control)
                        .expect("another atom"),
                    Some(false)
                );
                assert_eq!(
                    reader.proofs.metrics.hits, 5,
                    "closed facts are shared across retained atoms, not their truths"
                );
                assert_eq!(
                    (prepared.store.stats().nodes, prepared.store.stats().handles),
                    (stats.nodes, stats.handles)
                );
                drop(reader);
                assert_eq!(atoms.row_proofs.scans, 2);
                assert_eq!(atoms.row_proofs.hits, 5);
                assert_eq!(atoms.fallback, 1, "only the copied atom falls back");
            });
        }
    }

    #[test]
    fn reader_proof_hits_preserve_semantic_fallback_and_owned_stops() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nSpec == Init /\\ [][Next]_x\nP == <> (x \\div 0=0)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let options = if tlc {
                    dive_check::CheckOptions::tlc(0, 0)
                } else {
                    dive_check::CheckOptions::dive(0)
                };
                let (graph, mut values) = capture(prepared, &options);
                let atom = driver.native_predicates().next().expect("predicate");
                let mut reader = Reader::new(&graph, &mut values, atoms);
                for _ in 0..2 {
                    assert_eq!(
                        reader
                            .predicate(atom, 0, None, control)
                            .expect("semantic fallback"),
                        None
                    );
                }
                assert_eq!(reader.proofs.metrics.scans, 1);
                assert_eq!(reader.proofs.metrics.hits, 1);
                assert_eq!(reader.atoms.fallback, 2);
                assert_eq!(reader.atoms.evaluated, 0);
                let watchdog = Budget::new(Some(std::time::Duration::ZERO), None)
                    .start()
                    .expect("watchdog")
                    .expect("finite");
                reader.atoms.control =
                    EvaluationControl::new(watchdog.budget(), Phase::Capture).expect("owner");
                let phase =
                    EvaluationControl::new(watchdog.budget(), Phase::Temporal).expect("phase");
                assert!(
                    matches!(reader.predicate(atom, 0, None, &phase), Err(AnalysisError::Stopped(ref stop)) if stop.phase==Phase::Temporal)
                );
                assert_eq!(
                    reader.proofs.metrics.hits, 1,
                    "owned precheck runs before proof reuse"
                );
            });
        }
    }

    #[test]
    fn reader_proof_hits_do_not_replace_optional_machine_fuel() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == \\A i \\in 1..10000: x'=x\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let options = if tlc {
                    dive_check::CheckOptions::tlc(0, 0)
                } else {
                    dive_check::CheckOptions::dive(0)
                };
                let (graph, mut values) = capture(prepared, &options);
                let atom = driver.native_predicates().next().expect("enabled");
                let mut reader = Reader::new(&graph, &mut values, atoms);
                for _ in 0..2 {
                    assert_eq!(
                        reader
                            .predicate(atom, 0, None, control)
                            .expect("optional quota"),
                        None
                    );
                }
                assert_eq!(reader.proofs.metrics.scans, 1);
                assert_eq!(reader.proofs.metrics.hits, 1);
                assert_eq!(reader.atoms.fallback, 2);
                assert_eq!(reader.atoms.evaluated, 0);
                assert_eq!(reader.atoms.machine.frame.len(), 0);
                assert_eq!(reader.atoms.machine.assigned, 0);
            });
        }
    }

    #[test]
    fn over_quota_closed_inputs_never_gain_a_positive_row_proof() {
        let tuple = vec!["0"; 4096].join(",");
        let source = format!(
            "---- MODULE Atoms ----\nVARIABLE x\nInit == x = <<{tuple}>>\nNext == UNCHANGED x\nP == <> (x=x)\n====\n"
        );
        for tlc in [false, true] {
            let (prepared, options) =
                prepare_text(&source, "INIT Init\nNEXT Next\nPROPERTY P\n", tlc);
            let (graph, _) = capture(&prepared, &options);
            let heap = dive_core::Heap::new(prepared.store.clone(), false, false);
            let mut proofs = RowProofs::new(&graph);
            for _ in 0..2 {
                assert!(!proofs.closed(&heap, 0).expect("over-quota"));
            }
            assert_eq!(proofs.metrics.scans, 2);
            assert_eq!(proofs.metrics.certified, 0);
            assert_eq!(proofs.metrics.hits, 0);
        }
    }

    #[test]
    fn fairness_checks_use_closed_updates_and_require_the_actual_interval_domain() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x = <<0,0>> /\\ y=0\nA == \\E i \\in 1..2: x'=[x EXCEPT ![i]=x[i]+1] /\\ UNCHANGED y\nSpec == Init /\\ [][A]_<<x,y>> /\\ WF_<<x,y>>(A)\nP == <> (y=1)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                assert_eq!(atoms.eligible, 3);
                let mut heap = dive_core::Heap::new(prepared.store.clone(), false, false);
                let zero = Handle::int(0).expect("zero");
                let one = Handle::int(1).expect("one");
                let x = heap.int_fcn(1, &[zero, zero]);
                let x = heap.intern_exact(x).expect("before");
                let y = heap.int_fcn(1, &[one, zero]);
                let y = heap.intern_exact(y).expect("after");
                let native_before = prepared.store.stats();
                for atom in driver
                    .native_predicates()
                    .filter(|a| a.test != AtomTest::Predicate)
                {
                    let after =
                        matches!(atom.test, AtomTest::Nonstuttering { .. }).then_some([y, zero]);
                    assert_eq!(
                        atoms
                            .evaluate(
                                atom,
                                &[x, zero],
                                after.as_ref().map(<[_; 2]>::as_slice),
                                control
                            )
                            .expect("call"),
                        Some(true)
                    );
                    if after.is_some() {
                        assert_eq!(
                            atoms
                                .evaluate(atom, &[x, zero], Some(&[x, zero]), control)
                                .expect("stutter"),
                            Some(false)
                        );
                    }
                }
                assert_eq!(
                    prepared.store.stats().nodes,
                    native_before.nodes,
                    "analysis never interns"
                );
                let enabled = driver
                    .native_predicates()
                    .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                    .expect("enabled");
                for (lo, values) in [
                    (0, vec![zero, zero]),
                    (1, vec![zero]),
                    (1, vec![zero, zero, zero]),
                ] {
                    let wrong = heap.int_fcn(lo, &values);
                    let wrong = heap.intern_exact(wrong).expect("wrong domain");
                    assert_eq!(
                        atoms
                            .evaluate(enabled, &[wrong, zero], None, control)
                            .expect("fallback"),
                        None
                    );
                }
                assert_eq!(
                    atoms
                        .evaluate(enabled, &[zero, zero], None, control)
                        .expect("wrong class"),
                    None
                );
                assert_eq!(
                    atoms
                        .evaluate(enabled, &[x, zero], None, control)
                        .expect("clean after failures"),
                    Some(true)
                );
            });
        }
    }

    #[test]
    fn missing_update_key_falls_back_before_reference_warning_history_changes() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x = <<0,0>> /\\ y=0\nA == \\E i \\in 1..2: x'=[x EXCEPT ![i]=0] /\\ UNCHANGED y\nSpec == Init /\\ [][A]_<<x,y>> /\\ WF_<<x,y>>(A)\nP == <> (y=1)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let atom = driver
                    .native_predicates()
                    .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                    .expect("enabled");
                let mut heap = dive_core::Heap::new(prepared.store.clone(), false, false);
                let zero = Handle::int(0).expect("zero");
                let wrong = heap.int_fcn(1, &[zero]);
                let wrong = heap.intern_exact(wrong).expect("short domain");
                let capture = dive_eval::capture::PrintCapture::start();
                assert_eq!(
                    atoms
                        .evaluate(atom, &[wrong, zero], None, control)
                        .expect("declines"),
                    None
                );
                assert!(
                    capture.take().is_empty(),
                    "optional attempt does not alter warning history"
                );
                let state = dive_eval::St::state(Arc::from([
                    Some(dive_value::Value::tuple(vec![dive_value::Value::Int(0)])),
                    Some(dive_value::Value::Int(0)),
                ]));
                assert!(
                    !atom
                        .evaluate(&prepared.model, &state)
                        .expect("reference false")
                );
                let lines = capture.take();
                assert_eq!(lines.len(), 1);
                assert!(lines[0].warning && lines[0].text.contains("2144"));
            });
        }
    }

    #[test]
    fn enabled_retains_cutoff_while_ordinary_nonstuttering_preserves_substitution_truth() {
        for mapping in ["x <- x, y <- y", "x <- y, y <- y"] {
            let source = format!(
                "---- MODULE Atoms ----\nEXTENDS Naturals\n---- MODULE Inner ----\nEXTENDS Naturals\nVARIABLES x,y\nStep == x'=0 /\\ y'=1\nFair == WF_<<x,y>>(Step)\n====\nVARIABLES x,y\nI == INSTANCE Inner WITH {mapping}\nInit == x=0 /\\ y=1\nNext == UNCHANGED <<x,y>>\nSpec == Init /\\ [][Next]_<<x,y>> /\\ I!Fair\nP == <> (x=1)\n====\n"
            );
            for tlc in [false, true] {
                with_wrappers(&source, tlc, |prepared, driver, atoms, control| {
                    let wrappers = driver
                        .native_predicates()
                        .filter(|a| a.test != AtomTest::Predicate);
                    for atom in wrappers {
                        let result = atoms
                            .evaluate(
                                atom,
                                &[Handle::int(0).expect("zero"), Handle::int(1).expect("one")],
                                matches!(atom.test, AtomTest::Nonstuttering { .. })
                                    .then_some([
                                        Handle::int(0).expect("zero"),
                                        Handle::int(1).expect("one"),
                                    ])
                                    .as_ref()
                                    .map(<[_; 2]>::as_slice),
                                control,
                            )
                            .expect("attempt");
                        if mapping.starts_with("x <- y")
                            && matches!(atom.test, AtomTest::Enabled { .. })
                        {
                            assert_eq!(result, None, "ENABLED retains its branch cutoff proof");
                        } else {
                            assert_eq!(result, Some(false));
                            if matches!(atom.test, AtomTest::Nonstuttering { .. }) {
                                let row: Arc<[Option<dive_value::Value>]> = Arc::from([
                                    Some(dive_value::Value::Int(0)),
                                    Some(dive_value::Value::Int(1)),
                                ]);
                                assert_eq!(
                                    result,
                                    Some(
                                        atom.evaluate(
                                            &prepared.model,
                                            &dive_eval::St::action(row.clone(), row)
                                        )
                                        .expect("reference scoped truth")
                                    )
                                );
                            }
                        }
                    }
                });
            }
        }
    }

    #[test]
    fn constructed_instance_nonstuttering_compares_actual_before_and_after_projections() {
        let source = "---- MODULE Atoms ----\nEXTENDS Integers\n---- MODULE Inner ----\nEXTENDS Integers\nVARIABLE a\nStep == a'=[a EXCEPT ![0]=@+1]\nFair == WF_a(Step)\n====\nVARIABLE x\nProjection == [i \\in 0..1 |-> x[i]]\nI == INSTANCE Inner WITH a <- Projection\nInit == x=[i \\in 0..1 |-> 0]\nNext == UNCHANGED x\nSpec == Init /\\ [][Next]_x /\\ I!Fair\nP == <> (x[0]=2)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let atom = driver
                    .native_predicates()
                    .find(|a| matches!(a.test, AtomTest::Nonstuttering { .. }))
                    .expect("ordinary scoped wrapper");
                let enabled = driver
                    .native_predicates()
                    .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                    .expect("hidden query wrapper");
                let mut heap = dive_core::Heap::new(prepared.store.clone(), false, false);
                let zero = Handle::int(0).expect("zero");
                let before = heap.int_fcn(0, &[zero, zero]);
                let before = heap.intern_exact(before).expect("before");
                let ctx = if tlc {
                    dive_value::Ctx::tlc()
                } else {
                    dive_value::Ctx::dive()
                };
                let reference = |first| {
                    Arc::from([Some(
                        dive_value::Value::fcn(
                            vec![
                                (dive_value::Value::Int(0), dive_value::Value::Int(first)),
                                (dive_value::Value::Int(1), dive_value::Value::Int(0)),
                            ],
                            ctx,
                        )
                        .expect("closed original"),
                    )])
                };
                for first in [0, 1, 2, 0] {
                    let after = heap.int_fcn(0, &[Handle::int(first).expect("first"), zero]);
                    let after = heap.intern_exact(after).expect("after");
                    let truth = atom
                        .evaluate(
                            &prepared.model,
                            &dive_eval::St::action(reference(0), reference(first)),
                        )
                        .expect("reference");
                    assert_eq!(
                        atoms
                            .evaluate(atom, &[before], Some(&[after]), control)
                            .expect("native scoped wrapper"),
                        Some(truth),
                        "{tlc}/{first}"
                    );
                }
                assert_eq!(
                    atoms
                        .evaluate(enabled, &[before], None, control)
                        .expect("hidden query remains reference"),
                    None
                );
            });
        }
    }

    #[test]
    fn nonstuttering_equal_subscript_does_not_evaluate_a_failing_action() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == (1 \\div 0=0) /\\ x'=x\nSpec == Init /\\ [][Next]_x /\\ WF_x(A)\nP == <> (x=1)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |_, driver, atoms, control| {
                let atom = driver
                    .native_predicates()
                    .find(|a| matches!(a.test, AtomTest::Nonstuttering { .. }))
                    .expect("nonstutter");
                let zero = Handle::int(0).expect("zero");
                let one = Handle::int(1).expect("one");
                assert_eq!(
                    atoms
                        .evaluate(atom, &[zero], Some(&[zero]), control)
                        .expect("same"),
                    Some(false)
                );
                assert_eq!(
                    atoms
                        .evaluate(atom, &[zero], Some(&[one]), control)
                        .expect("reference error fallback"),
                    None
                );
                assert_eq!(
                    atoms
                        .evaluate(atom, &[one], Some(&[one]), control)
                        .expect("frame reset"),
                    Some(false)
                );
            });
        }
    }

    #[test]
    fn enabled_keeps_partial_first_success_and_searches_past_stuttering_candidates() {
        for tlc in [false, true] {
            for (action, fairness) in [
                ("x'=1", false),
                ("UNCHANGED <<x,y>> \\/ (x'=x+1 /\\ UNCHANGED y)", true),
            ] {
                let property = if fairness {
                    "P == <> (x=1)"
                } else {
                    "P == <> (ENABLED A)"
                };
                let fair = if fairness { "/\\ WF_<<x,y>>(A)" } else { "" };
                let source = format!(
                    "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x=0 /\\ y=0\nNext == UNCHANGED <<x,y>>\nA == {action}\nSpec == Init /\\ [][Next]_<<x,y>> {fair}\n{property}\n====\n"
                );
                with_wrappers(&source, tlc, |_, driver, atoms, control| {
                    let atom = driver
                        .native_predicates()
                        .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                        .expect("enabled");
                    assert_eq!(
                        atoms
                            .evaluate(atom, &[Handle::int(0).expect("zero"); 2], None, control)
                            .expect("enabled"),
                        Some(true)
                    );
                });
            }
        }
    }

    #[test]
    fn pure_let_generator_bindings_and_query_frames_do_not_leak_between_rows() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x=0 /\\ y=0\nNext == UNCHANGED <<x,y>>\nA == LET K == x+1 IN (x'=K /\\ UNCHANGED y) \\/ (x'=K+1 /\\ UNCHANGED y)\nSpec == Init /\\ [][Next]_<<x,y>> /\\ WF_<<x,y>>(A)\nP == <> (y=1)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |_, driver, atoms, control| {
                for x in [0, 5, 0] {
                    let before = [
                        Handle::int(x).expect("before"),
                        Handle::int(0).expect("zero"),
                    ];
                    let after = [
                        Handle::int(x + 1).expect("after"),
                        Handle::int(0).expect("zero"),
                    ];
                    for atom in driver
                        .native_predicates()
                        .filter(|a| a.test != AtomTest::Predicate)
                    {
                        assert_eq!(
                            atoms
                                .evaluate(
                                    atom,
                                    &before,
                                    matches!(atom.test, AtomTest::Nonstuttering { .. })
                                        .then_some(after.as_slice()),
                                    control
                                )
                                .expect("query/LET"),
                            Some(true)
                        );
                    }
                }
                assert_eq!(atoms.fallback, 0);
            });
        }
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

    #[test]
    fn actual_binder_alias_and_shadow_controls() {
        for tlc in [false, true] {
            for (defs, action, eligible) in [
                (
                    "U(k) == [x EXCEPT ![k] = x[k]+1]",
                    "\\E j \\in 1..2: x'=U(j) /\\ UNCHANGED y",
                    true,
                ),
                (
                    "U(k) == [x EXCEPT ![k] = x[k]+1]",
                    "\\E j \\in 1..2: x'=U(j+0) /\\ UNCHANGED y",
                    false,
                ),
                (
                    "",
                    "\\E j \\in 1..2: (\\E k \\in 3..4: x'=[x EXCEPT ![k]=0]) /\\ UNCHANGED y",
                    true,
                ),
                (
                    "",
                    "\\E j \\in 1..y: x'=[x EXCEPT ![j]=0] /\\ UNCHANGED y",
                    false,
                ),
                (
                    "",
                    "\\E j \\in {1,2}: x'=[x EXCEPT ![j]=0] /\\ UNCHANGED y",
                    true,
                ),
                (
                    "",
                    "\\E j \\in 1..2: x'=[x EXCEPT ![j]=@+1] /\\ UNCHANGED y",
                    false,
                ),
                (
                    "",
                    "\\E j \\in 1..2: x'=[x EXCEPT ![j]=1, ![j]=2] /\\ UNCHANGED y",
                    false,
                ),
                (
                    "",
                    "\\E j \\in 1..2: x'=[x EXCEPT ![j][1]=1] /\\ UNCHANGED y",
                    false,
                ),
                (
                    "",
                    "\\E j \\in 1..2: x'=[(IF TRUE THEN x ELSE x) EXCEPT ![j]=1] /\\ UNCHANGED y",
                    false,
                ),
            ] {
                let source = format!(
                    "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x = <<0,0>> /\\ y=2\nNext == UNCHANGED <<x,y>>\n{defs}\nA == {action}\nSpec == Init /\\ [][Next]_<<x,y>> /\\ WF_<<x,y>>(A)\nP == <> (y=1)\n====\n"
                );
                with_wrappers(&source, tlc, |prepared, driver, atoms, control| {
                    let enabled = driver
                        .native_predicates()
                        .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                        .expect("enabled");
                    let mut heap = dive_core::Heap::new(prepared.store.clone(), false, false);
                    let zero = Handle::int(0).expect("zero");
                    let x = heap.int_fcn(1, &[zero, zero]);
                    let x = heap.intern_exact(x).expect("persistent");
                    let two = Handle::int(2).expect("two");
                    let got = atoms
                        .evaluate(enabled, &[x, two], None, control)
                        .expect("optional");
                    if !eligible || action.contains("3..4") || action.contains("{1,2}") {
                        assert_eq!(got, None, "{action}, tlc={tlc}");
                    } else {
                        assert_eq!(got, Some(true), "{action}, tlc={tlc}");
                        let state = dive_eval::St::state(Arc::from([
                            Some(dive_value::Value::tuple(vec![
                                dive_value::Value::Int(0),
                                dive_value::Value::Int(0),
                            ])),
                            Some(dive_value::Value::Int(2)),
                        ]));
                        assert!(
                            enabled
                                .evaluate(&prepared.model, &state)
                                .expect("reference")
                        );
                    }
                });
            }
        }
    }

    #[test]
    fn negated_enabled_retains_reference_lane() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == x'=1\nSpec == Init /\\ [][Next]_x\nP == <> (~ENABLED A)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |_, driver, atoms, control| {
                for atom in driver.native_predicates() {
                    let got = atoms
                        .evaluate(atom, &[Handle::int(0).expect("zero")], None, control)
                        .expect("query");
                    if matches!(atom.test, AtomTest::NegatedEnabled { .. }) {
                        assert_eq!(got, None);
                    }
                }
            });
        }
    }

    #[test]
    fn native_query_reports_an_owned_interruption_from_inside_enumeration() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        #[derive(Debug)]
        struct Pause {
            owner: Arc<EvaluationControl>,
            calls: AtomicUsize,
        }
        impl dive_core::eval::WorkControl for Pause {
            fn proceed(&self) -> bool {
                if self.calls.fetch_add(1, Ordering::Relaxed) == 3 {
                    std::thread::sleep(std::time::Duration::from_millis(120));
                }
                self.owner.check().is_ok()
            }
        }
        let owned_text = "a";
        for (action, text) in [
            ("\\E i \\in 1..10000: x'=i /\\ i=x", None),
            ("((1..10000) \\subseteq (1..10000)) /\\ x'=x", None),
            ("Len(x)=1 /\\ x'=x", Some(owned_text)),
        ] {
            let init = text.map_or_else(|| "0".into(), |text| format!("\"{text}\""));
            let source = format!(
                "---- MODULE Atoms ----\nEXTENDS Naturals, Sequences\nVARIABLE x\nInit == x={init}\nNext == UNCHANGED x\nA == {action}\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n"
            );
            for tlc in [false, true] {
                with_wrappers(&source, tlc, |prepared, driver, atoms, _| {
                    let watchdog = Budget::new(Some(std::time::Duration::from_millis(50)), None)
                        .start()
                        .expect("watchdog")
                        .expect("finite");
                    let owner =
                        EvaluationControl::new(watchdog.budget(), Phase::Capture).expect("owner");
                    let phase =
                        EvaluationControl::new(watchdog.budget(), Phase::Temporal).expect("phase");
                    let poll = Arc::new(Pause {
                        owner: owner.clone(),
                        calls: AtomicUsize::new(0),
                    });
                    atoms.control = owner;
                    atoms.machine.set_work_control(Some(poll.clone()));
                    let atom = driver.native_predicates().next().expect("enabled");
                    let value = text.map_or_else(
                        || Handle::int(0).expect("zero"),
                        |text| {
                            Handle::string(
                                prepared
                                    .store
                                    .strings()
                                    .intern(text)
                                    .expect("startup-owned name"),
                            )
                        },
                    );
                    assert!(
                        matches!(atoms.evaluate(atom,&[value],None,&phase),Err(AnalysisError::Stopped(ref stop)) if stop.phase==Phase::Temporal)
                    );
                    assert!(
                        poll.calls.load(Ordering::Relaxed) >= 4,
                        "query reached its internal checkpoint"
                    );
                    assert_eq!(atoms.evaluated, 0);
                    assert!(
                        atoms.machine.heap.arena().peak().1 < 100,
                        "owned interruption precedes materialization"
                    );
                });
            }
        }
    }

    #[test]
    fn universal_and_nested_query_contexts_respect_the_optional_quota() {
        for tlc in [false, true] {
            for action in [
                "\\A i \\in 1..10000: x'=x",
                "\\E i \\in 1..100: \\E j \\in 1..100: x'=i+j /\\ x=i+j",
                "\\E i,j \\in 1..100: x'=i+j /\\ x=i+j",
            ] {
                let source = format!(
                    "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == {action}\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n"
                );
                with_wrappers(&source, tlc, |_, driver, atoms, control| {
                    let atom = driver.native_predicates().next().expect("enabled");
                    assert_eq!(
                        atoms
                            .evaluate(atom, &[Handle::int(0).expect("zero")], None, control)
                            .expect("bounded"),
                        None,
                        "{action}"
                    );
                    assert_eq!(atoms.machine.frame.len(), 0);
                    assert_eq!(atoms.machine.assigned, 0);
                });
            }
        }
    }

    #[test]
    fn unproved_product_sources_stay_reference_and_forall_recovers_after_quota() {
        for tlc in [false, true] {
            for (domain, expected) in [
                ("(1..2) \\X (1..2)", None),
                ("(1..1000000000) \\X (1..2)", None),
                ("(1..2) \\X ((1..1000000000) \\X (1..2))", None),
            ] {
                let source = format!(
                    "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == \\E <<i,j>> \\in {domain}: x'=i /\\ x=i\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n"
                );
                with_wrappers(&source, tlc, |_, driver, atoms, control| {
                    assert_eq!(
                        atoms.eligible, 0,
                        "existing product source certificate stays conservative"
                    );
                    let atom = driver.native_predicates().next().expect("enabled");
                    assert_eq!(
                        atoms
                            .evaluate(atom, &[Handle::int(0).expect("zero")], None, control)
                            .expect("bounded product"),
                        expected
                    );
                });
            }
            let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == \\A i \\in 1..x: x'=x\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n";
            with_wrappers(source, tlc, |_, driver, atoms, control| {
                let atom = driver.native_predicates().next().expect("enabled");
                assert_eq!(
                    atoms
                        .evaluate(atom, &[Handle::int(10000).expect("large")], None, control)
                        .expect("forall quota"),
                    None
                );
                assert_eq!(
                    atoms
                        .evaluate(atom, &[Handle::int(1).expect("one")], None, control)
                        .expect("forall frame after quota"),
                    Some(true)
                );
            });
        }
    }

    #[test]
    fn query_candidate_value_normalization_is_preflighted_before_materialization() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == x'=(1..1000000000) /\\ FALSE\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |_, driver, atoms, control| {
                assert_eq!(atoms.eligible, 1);
                let atom = driver.native_predicates().next().expect("enabled");
                assert_eq!(
                    atoms
                        .evaluate(atom, &[Handle::int(0).expect("zero")], None, control)
                        .expect("optional value allocation quota"),
                    None
                );
                assert!(
                    atoms.machine.heap.arena().peak().1 < 100,
                    "no large candidate payload was allocated"
                );
            });
        }
    }

    #[test]
    fn generic_function_domain_membership_and_optional_fuel() {
        for tlc in [false, true] {
            let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x = <<0,0>> /\\ y=0\nNext == UNCHANGED <<x,y>>\nA == \\E i \\in 1..2: x'=[x EXCEPT ![i]=1] /\\ UNCHANGED y\nSpec == Init /\\ [][Next]_<<x,y>> /\\ WF_<<x,y>>(A)\nP == <> (y=1)\n====\n";
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let atom = driver
                    .native_predicates()
                    .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                    .expect("enabled");
                let mut heap = dive_core::Heap::new(prepared.store.clone(), false, false);
                let z = Handle::int(0).expect("zero");
                let one = Handle::int(1).expect("one");
                let two = Handle::int(2).expect("two");
                for (keys, want) in [
                    (vec![one, two], Some(true)),
                    (vec![z, one], None),
                    (vec![one], None),
                    (vec![one, two, Handle::int(3).expect("three")], None),
                    (vec![Handle::bool(false), Handle::bool(true)], None),
                ] {
                    let f = heap.fcn_sorted(&keys, &vec![z; keys.len()]);
                    let f = heap.intern_exact(f).expect("persistent");
                    assert_eq!(
                        atoms.evaluate(atom, &[f, z], None, control).expect("query"),
                        want,
                        "keys={keys:?}"
                    );
                }
            });
            let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == \\E i \\in 1..10000: x'=i /\\ i=x\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n";
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let atom = driver
                    .native_predicates()
                    .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                    .expect("enabled");
                assert_eq!(
                    atoms
                        .evaluate(atom, &[Handle::int(0).expect("zero")], None, control)
                        .expect("fuel fallback"),
                    None
                );
                assert!(
                    !atom
                        .evaluate(
                            &prepared.model,
                            &dive_eval::St::state(Arc::from([Some(dive_value::Value::Int(0))]))
                        )
                        .expect("reference false")
                );
                assert_eq!(
                    atoms
                        .evaluate(atom, &[Handle::int(1).expect("one")], None, control)
                        .expect("clean next call"),
                    Some(true)
                );
            });
        }
    }

    #[test]
    fn subseteq_materialization_respects_preallocation_quota() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == ((1..10000) \\subseteq (1..10000)) /\\ x'=x\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |_, driver, atoms, control| {
                assert_eq!(atoms.eligible, 1, "probe must reach actual native code");
                let atom = driver.native_predicates().next().expect("atom");
                let got = atoms
                    .evaluate(atom, &[Handle::int(0).expect("zero")], None, control)
                    .expect("optional query");
                assert_eq!(got, None, "large work declines");
                assert!(
                    atoms.machine.heap.arena().peak().1 < 100,
                    "optional preflight must reject before large allocation; actual peak={:?}, tlc={tlc}",
                    atoms.machine.heap.arena().peak()
                );
            });
        }
    }

    #[test]
    fn huge_first_partial_success_and_fake_hook_authority() {
        #[derive(Debug)]
        struct Deny;
        impl dive_core::eval::WorkControl for Deny {
            fn proceed(&self) -> bool {
                false
            }
        }
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x=0 /\\ y=0\nNext == UNCHANGED <<x,y>>\nA == \\E i \\in 1..1000000000: x'=i\nSpec == Init /\\ [][Next]_<<x,y>>\nP == <> (ENABLED A)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |_, driver, atoms, control| {
                assert_eq!(atoms.eligible, 1);
                let atom = driver.native_predicates().next().expect("atom");
                let z = Handle::int(0).expect("zero");
                assert_eq!(
                    atoms
                        .evaluate(atom, &[z, z], None, control)
                        .expect("large partial"),
                    Some(true)
                );
                assert!(atoms.machine.heap.arena().peak().1 < 100);
                atoms.machine.set_work_control(Some(Arc::new(Deny)));
                assert_eq!(
                    atoms
                        .evaluate(atom, &[z, z], None, control)
                        .expect("fake stop cannot become owned stop"),
                    None
                );
                atoms
                    .machine
                    .set_work_control(Some(Arc::new(QueryControl(atoms.control.clone()))));
                assert_eq!(
                    atoms
                        .evaluate(atom, &[z, z], None, control)
                        .expect("clean reset after fake stop"),
                    Some(true)
                );
                assert_eq!(atoms.machine.frame.len(), 0);
                assert_eq!(atoms.machine.assigned, 0);
            });
        }
    }
    #[test]
    fn admitted_bulk_operations_preflight_and_recover_on_small_rows() {
        for tlc in [false, true] {
            for expression in ["((1..x) \\subseteq (1..x))", "Cardinality(1..x)=x"] {
                let source = format!(
                    "---- MODULE Atoms ----\nEXTENDS Naturals, FiniteSets\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == {expression} /\\ x'=x\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n"
                );
                with_wrappers(&source, tlc, |_, driver, atoms, control| {
                    assert_eq!(atoms.eligible, 1, "actual admitted source");
                    let atom = driver.native_predicates().next().expect("enabled");
                    assert_eq!(
                        atoms
                            .evaluate(atom, &[Handle::int(10000).expect("large")], None, control)
                            .expect("preflight"),
                        None
                    );
                    assert!(
                        atoms.machine.heap.arena().peak().1 < 100,
                        "large payload never allocated"
                    );
                    assert_eq!(
                        atoms
                            .evaluate(atom, &[Handle::int(2).expect("small")], None, control)
                            .expect("clean next row"),
                        Some(true)
                    );
                });
            }
        }
    }

    #[test]
    fn optional_materialization_does_not_intern_in_the_producer_store() {
        use dive_liveness::analyze::{GraphLabelPolicy, Limits};
        use dive_liveness::product::AcceptancePolicy;
        let (prepared, options) = prepare_source("x \\in 1..3");
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
        atoms.machine.opts.symbolic_member = false;
        let atom = driver.native_predicates().next().expect("predicate");
        let before = prepared.store.stats();
        assert_eq!(
            atoms
                .evaluate(atom, &[Handle::int(2).expect("two")], None, &control)
                .expect("materialized member"),
            Some(true)
        );
        let after = prepared.store.stats();
        assert_eq!((before.nodes, before.handles), (after.nodes, after.handles));
    }

    #[test]
    fn utf16_length_work_is_charged() {
        let long = "a".repeat(10000);
        let source = format!(
            "---- MODULE Atoms ----\nEXTENDS Naturals, Sequences\nVARIABLE x\nInit == x=\"{long}\"\nNext == UNCHANGED x\nA == (Len(x)=10000) /\\ x'=x\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n"
        );
        for tlc in [false, true] {
            with_wrappers(&source, tlc, |prepared, driver, atoms, control| {
                assert_eq!(atoms.eligible, 1, "actual admitted native source");
                let h = Handle::string(
                    prepared
                        .store
                        .strings()
                        .intern(&long)
                        .expect("already-owned startup name"),
                );
                let atom = driver.native_predicates().next().expect("atom");
                assert_eq!(
                    atoms.evaluate(atom, &[h], None, control).expect("query"),
                    None,
                    "10000 UTF16 units exceed4096 loop fuel, tlc={tlc}"
                );
            });
        }
    }

    #[test]
    fn scalar_string_length_preserves_utf16_units_and_recovers_after_quota() {
        let long = "a".repeat(10000);
        let source = format!(
            "---- MODULE Atoms ----\nEXTENDS Naturals, Sequences\nVARIABLE x\nInit == x \\in {{\"{long}\",\"a\",\"é😀\",\"\"}}\nNext == UNCHANGED x\nA == Len(x)=(IF x=\"\" THEN 0 ELSE IF x=\"é😀\" THEN 3 ELSE 1) /\\ x'=x\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n"
        );
        for tlc in [false, true] {
            with_wrappers(&source, tlc, |prepared, driver, atoms, control| {
                assert_eq!(atoms.eligible, 1);
                let atom = driver.native_predicates().next().expect("atom");
                for (text, expected) in [
                    (&long[..], None),
                    ("a", Some(true)),
                    ("é😀", Some(true)),
                    ("", Some(true)),
                ] {
                    let value = Handle::string(
                        prepared
                            .store
                            .strings()
                            .intern(text)
                            .expect("startup-owned string"),
                    );
                    assert_eq!(
                        atoms
                            .evaluate(atom, &[value], None, control)
                            .expect("query"),
                        expected,
                        "{tlc}"
                    );
                }
            });
        }
    }
}
