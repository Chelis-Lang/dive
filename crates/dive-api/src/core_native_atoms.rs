//! Optional native predicates admitted by Core plan facts.
//!
//! A retained atom runs natively when its exact object and source scope bind,
//! the Core bound-test compiler accepts it, its plan is effect-free and cannot
//! add a string to the shared table, and every compiled constant and input row
//! is a materialized Core value (ADRs 0129, 0154). Recursive operators, memo
//! sites (evaluated directly: the analysis Machine memoizes nothing) and
//! powers are admitted. Anything else, and every error, fuel exhaustion,
//! warning or printed output, keeps the reference call.

use std::sync::Arc;

use dive_core::{Handle, Machine, Opts, Store};
use dive_eval::{EvalError, HostControl};
use dive_liveness::analyze::AnalysisError;
use dive_liveness::analyze::periodic::external::{GraphCheck, NativeReader};
use dive_liveness::plan::{AtomTest, PredicateAtom};
use dive_model::Model;
use dive_runtime::budget::EvaluationControl;

use crate::core_native_graph::NativeGraph;
use crate::core_native_values::NativeValues;

struct Compiled {
    identity: *const PredicateAtom,
    plan: dive_core::plan::Plan,
    /// Fuel exhaustions in a row; at [`FUEL_STRIKES`] the atom stops running
    /// natively in this analysis.
    exhausted_in_a_row: u32,
}

/// Node visits one native atom demand may spend before it falls back. The
/// run's time and memory budgets still bound the work through
/// [`QueryControl`], polled at every charge; this allowance only bounds the
/// work wasted, and the memory held until `heap.reset()`, before the
/// reference evaluator answers instead.
const NATIVE_FUEL: u64 = 1 << 20;

/// Consecutive fuel exhaustions after which an atom is left to the reference
/// evaluator for the rest of the analysis: a scheduling choice that changes
/// no truth value.
const FUEL_STRIKES: u32 = 8;

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
    reuse: ReuseMetrics,
    rejections: Rejections,
    fallbacks: Fallbacks,
    /// Per-demand fuel ([`NATIVE_FUEL`]; tests lower it).
    fuel: u64,
    /// Atoms that reached [`FUEL_STRIKES`].
    fuel_retired: u64,
    /// `DIVE_NATIVE_ATOM_AUDIT=1`: re-evaluate every native truth by reference.
    audit: bool,
    audit_mismatch: Option<String>,
}

/// Why a native predicate was not prepared, by its first failing check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rejection {
    NegatedEnabled,
    ForeignScope,
    CompileError,
    Effects,
    FreshString,
    ConstUnclosed,
    Allocation,
}

#[derive(Default)]
struct Rejections {
    negated_enabled: u64,
    foreign_scope: u64,
    compile_error: u64,
    effects: u64,
    fresh_string: u64,
    const_unclosed: u64,
    allocation: u64,
}

impl Rejections {
    fn record(&mut self, reason: Rejection) {
        let slot = match reason {
            Rejection::NegatedEnabled => &mut self.negated_enabled,
            Rejection::ForeignScope => &mut self.foreign_scope,
            Rejection::CompileError => &mut self.compile_error,
            Rejection::Effects => &mut self.effects,
            Rejection::FreshString => &mut self.fresh_string,
            Rejection::ConstUnclosed => &mut self.const_unclosed,
            Rejection::Allocation => &mut self.allocation,
        };
        *slot += 1;
    }

    fn json(&self) -> serde_json::Value {
        serde_json::json!({"negated_enabled": self.negated_enabled,
            "foreign_scope": self.foreign_scope, "compile_error": self.compile_error,
            "effects": self.effects,
            "fresh_string": self.fresh_string, "const_unclosed": self.const_unclosed,
            "allocation": self.allocation})
    }
}

/// Why an actual native demand returned to the reference evaluator.
#[derive(Default)]
struct Fallbacks {
    row_proof: u64,
    fuel_exhausted: u64,
    error: u64,
    warning_or_print: u64,
    not_compiled: u64,
    /// Demands of an atom retired after [`FUEL_STRIKES`] exhaustions.
    fuel_retired: u64,
}

#[derive(Default)]
struct ReuseMetrics {
    state: u64,
    action: u64,
    peak_bytes: usize,
    cap_bypasses: u64,
    allocation_bypasses: u64,
    stored_columns: u64,
}

impl NativeAtoms {
    pub(crate) fn prepare(
        model: &Model,
        store: &Arc<Store>,
        driver: &GraphCheck<'_>,
        control: &Arc<EvaluationControl>,
        strategies: Opts,
    ) -> Result<Self, dive_runtime::Verdict> {
        let opts = Opts {
            memo: false,
            step_memo: false,
            primed_memo: false,
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
            reuse: ReuseMetrics::default(),
            rejections: Rejections::default(),
            fallbacks: Fallbacks::default(),
            fuel: NATIVE_FUEL,
            fuel_retired: 0,
            audit: std::env::var_os("DIVE_NATIVE_ATOM_AUDIT").is_some_and(|value| value == "1"),
            audit_mismatch: None,
        };
        result
            .machine
            .set_work_control(Some(Arc::new(QueryControl(Arc::clone(control)))));
        // A native truth must not add a string token that the skipped
        // reference call would not have added (string order under
        // `--compat=tlc`); a new text is an unsupported error and falls back.
        result.machine.heap.set_existing_strings_only(true);
        // A reference warning Core does not record (2144) must reach the
        // reference evaluator, which prints it.
        result.machine.set_refuse_unrecorded_warnings(true);
        for atom in driver.native_predicates() {
            result.total += 1;
            if matches!(atom.test, AtomTest::NegatedEnabled { .. }) {
                result.rejections.record(Rejection::NegatedEnabled);
                continue;
            }
            // (a) The exact retained atom and its source scope must bind to
            // this model's evaluator; equal source text is not enough.
            let sub = match atom.test {
                AtomTest::Enabled { sub } => sub,
                AtomTest::Nonstuttering { sub } => Some(sub),
                AtomTest::Predicate | AtomTest::NegatedEnabled { .. } => None,
            };
            let ev = &model.evaluator;
            if !ev.admits_scoped_expr(&atom.pred.scope, atom.pred.expr)
                || sub.is_some_and(|sub| !ev.admits_scoped_expr(&atom.pred.scope, sub))
            {
                result.rejections.record(Rejection::ForeignScope);
                continue;
            }
            let plan = match compile(model, store, atom, opts, &result.machine.heap) {
                Ok(plan) => plan,
                Err(reason) => {
                    result.rejections.record(reason);
                    continue;
                }
            };
            if result.compiled.try_reserve(1).is_err() {
                result.rejections.record(Rejection::Allocation);
                continue;
            }
            result.compiled.push(Compiled {
                identity: std::ptr::from_ref(atom),
                plan,
                exhausted_in_a_row: 0,
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
                .unwrap_or_else(|| dive_runtime::Verdict::InternalError {
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
            "fairness_reuse": {"state_consumptions": self.reuse.state, "action_consumptions": self.reuse.action,
                "peak_storage_bytes": self.reuse.peak_bytes, "cap_bypasses": self.reuse.cap_bypasses,
                "allocation_bypasses": self.reuse.allocation_bypasses, "stored_columns": self.reuse.stored_columns,
                "scope": "completed native frozen behavior prefix columns per analysis; actual evaluation counters exclude reuse"},
            "rejections": self.rejections.json(),
            "fallbacks": {"row_proof": self.fallbacks.row_proof, "fuel_exhausted": self.fallbacks.fuel_exhausted,
                "error": self.fallbacks.error, "warning_or_print": self.fallbacks.warning_or_print,
                "not_compiled": self.fallbacks.not_compiled, "fuel_retired": self.fallbacks.fuel_retired},
            "fuel": {"per_demand": self.fuel, "strikes": FUEL_STRIKES, "retired_atoms": self.fuel_retired},
            "audit": self.audit,
            "eligibility": "source-scoped effect-free Core plans without fresh strings over materialized rows; identity-scoped fairness queries"})
    }

    #[cfg(test)]
    fn has(&self, atom: &PredicateAtom) -> bool {
        self.compiled
            .iter()
            .any(|item| std::ptr::eq(item.identity, atom))
    }

    /// The first native-versus-reference disagreement found by the audit.
    pub(crate) fn audit_mismatch(&self) -> Option<&str> {
        self.audit_mismatch.as_deref()
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
            Ok(admissible_row(heap, before) && after.is_none_or(|row| admissible_row(heap, row)))
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
        let Some(index) = self
            .compiled
            .iter()
            .position(|item| std::ptr::eq(item.identity, atom))
        else {
            self.fallback += 1;
            self.fallbacks.not_compiled += 1;
            return Ok(None);
        };
        if self.compiled[index].exhausted_in_a_row >= FUEL_STRIKES {
            self.fallback += 1;
            self.fallbacks.fuel_retired += 1;
            return Ok(None);
        }
        let compiled = &self.compiled[index];
        // The exact object came from this still-live driver's fixed plan Vec;
        // no expression ID, scope guess or extensional row equality is a key.
        if !inputs_closed(&self.machine.heap)? {
            self.fallback += 1;
            self.fallbacks.row_proof += 1;
            return Ok(None);
        }
        self.machine.s0.copy_from_slice(before);
        self.machine.s1.fill(Handle::EMPTY);
        if let Some(after) = after {
            self.machine.s1.copy_from_slice(after);
        }
        self.machine.set_fuel(self.fuel);
        let predicate = &compiled.plan.implied_actions[0];
        let frame = self.machine.enter(predicate.frame);
        for &(slot, value) in &compiled.plan.bound_preset {
            if let Some(cell) = self
                .machine
                .frame
                .get_mut(self.machine.base + slot as usize)
            {
                *cell = value;
            }
        }
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
        let exhausted = result.is_err() && self.machine.fuel_exhausted();
        let strikes = &mut self.compiled[index].exhausted_in_a_row;
        if exhausted {
            *strikes += 1;
            if *strikes == FUEL_STRIKES {
                self.fuel_retired += 1;
            }
        } else {
            *strikes = 0;
        }
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
            if exhausted {
                self.fallbacks.fuel_exhausted += 1;
            } else if result.is_err() {
                self.fallbacks.error += 1;
            } else {
                self.fallbacks.warning_or_print += 1;
            }
            Ok(None)
        }
    }

    fn validate_reuse(
        &mut self,
        atom: &PredicateAtom,
        control: &EvaluationControl,
        inputs_closed: impl FnOnce(&dive_core::Heap) -> Result<bool, AnalysisError>,
    ) -> Result<bool, AnalysisError> {
        check(&self.control, control)?;
        if !self
            .compiled
            .iter()
            .any(|item| std::ptr::eq(item.identity, atom))
        {
            return Ok(false);
        }
        let admitted = inputs_closed(&self.machine.heap)?;
        check(&self.control, control)?;
        if admitted {
            match atom.kind {
                dive_liveness::formula::AtomKind::State => self.reuse.state += 1,
                dive_liveness::formula::AtomKind::Action => self.reuse.action += 1,
            }
        }
        Ok(admitted)
    }
}

fn compile(
    model: &Model,
    store: &Arc<Store>,
    atom: &PredicateAtom,
    opts: Opts,
    heap: &dive_core::Heap,
) -> Result<dive_core::plan::Plan, Rejection> {
    let Ok(mut handoff) = crate::core_engine::handoff(model, &model.ir, Vec::new()) else {
        return Err(Rejection::CompileError);
    };
    let Ok(role) = crate::core_engine::role(
        model,
        "native temporal predicate".into(),
        atom.pred.expr,
        &atom.pred.scope,
    ) else {
        return Err(Rejection::CompileError);
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
    // The run's plan already evaluated the start-up definitions and the
    // ASSUMEs left to the core; an atom reaches none of those definitions
    // (the binder evaluated what temporal properties reach), so its compile
    // evaluates no start-up values, and would compile any body in place.
    handoff.deferred.clear();
    handoff.startup_evaluator = None;
    handoff.assumptions.clear();
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
        AtomTest::NegatedEnabled { .. } => return Err(Rejection::NegatedEnabled),
    }
    .map_err(|_| Rejection::CompileError)?;
    admit_plan(&plan, heap)?;
    Ok(plan)
}

/// The first failing admission check of a compiled plan, in the original order.
fn admit_plan(plan: &dive_core::plan::Plan, heap: &dive_core::Heap) -> Result<(), Rejection> {
    if plan.effects.any {
        return Err(Rejection::Effects);
    }
    // `ToString` prints through the reference name table, so it could add a
    // name there that the skipped reference call would not; the Heap's
    // existing-strings mode covers every other string builder.
    if plan.code.nodes.iter().any(|node| {
        matches!(
            node,
            dive_core::plan::Node::Lib {
                op: dive_ir::StdOp::TlcToString,
                ..
            }
        )
    }) {
        return Err(Rejection::FreshString);
    }
    // Recursive operators run in the analysis Machine's own frames; its
    // call-depth limit and fuel are errors that fall back.
    if plan.code.nodes.iter().any(|node| {
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
        value.is_some_and(|value| !admissible_row(heap, &[value]))
    }) || plan
        .bound_preset
        .iter()
        .any(|&(_, value)| !admissible_row(heap, &[value]))
    {
        return Err(Rejection::ConstUnclosed);
    }
    // A memo site evaluates its body directly: the analysis Machine has
    // memo = step_memo = false and a table-less Memo, so it never reads or
    // fills one. A power's digits are charged to fuel before allocation.
    Ok(())
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
                Some(dive_runtime::Verdict::Stopped(stop)) => AnalysisError::Stopped(stop),
                Some(dive_runtime::Verdict::BudgetMeasurement { error }) => {
                    AnalysisError::BudgetMeasurement(error)
                }
                _ => {
                    AnalysisError::Inconsistent("native predicate interruption lacks run authority")
                }
            }
        })
}

/// Handles the bounded walk visits for a row whose nodes lack the cached
/// [`dive_core::node::flags::CONCRETE`] proof (rows restored from a snapshot
/// or exchanged between workers).
const ROW_WALK_NODES: usize = 1 << 14;

/// The string and model-name bytes a row may hold when some name in the
/// Store is longer than this. Several string builtins (comparison, prefix and
/// replacement operators) scan texts without charging their length to the
/// analysis Machine's fuel, so one scan must stay bounded.
const ROW_NAME_BYTES: usize = 4096;

/// Whether native evaluation may read these input handles: every handle is a
/// materialized Core value (`compare::materialized`), so the native evaluator
/// and value order are the ones safety checking used on the same stored
/// representation (ADR 0154). Symbolic, lazy, unnormalized, transient and
/// empty handles decline, and so does a poisoned heap.
fn admissible_row(heap: &dive_core::Heap, roots: &[Handle]) -> bool {
    if heap.poisoned() {
        return false;
    }
    let short_names = heap.store().strings().longest_text_bytes() <= ROW_NAME_BYTES;
    if short_names && roots.iter().all(|&root| heap.proved_concrete(root)) {
        return !heap.poisoned();
    }
    heap.materialized_within(
        roots,
        ROW_WALK_NODES,
        (!short_names).then_some(ROW_NAME_BYTES),
    )
}

// A positive fact is scoped by an immutable graph borrow, never by a handle's
// bits across Stores or by extensional row equality. It is issued only by
// admissible_row(), independently for each actual row demand (ADR 0134, 0154).
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
        let success = admissible_row(heap, roots);
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

impl Reader<'_, '_> {
    /// Re-evaluates a native truth by reference (`DIVE_NATIVE_ATOM_AUDIT=1`).
    /// A different truth or a reference error ends the run as an internal error.
    fn audit(
        &mut self,
        atom: &PredicateAtom,
        before: usize,
        after: Option<usize>,
        native: bool,
        control: &EvaluationControl,
    ) -> Result<(), AnalysisError> {
        let graph = self.proofs.graph;
        let source = self.values.read(before, graph.row(before)?, control)?;
        let st = match after {
            Some(row) => {
                dive_eval::St::action(source, self.values.read(row, graph.row(row)?, control)?)
            }
            None => dive_eval::St::state(source),
        };
        let model = self.values.model();
        let reference = atom.evaluate(model, &st);
        if matches!(reference, Ok(value) if value == native) {
            return Ok(());
        }
        let location = model.evaluator.location(atom.pred.expr);
        self.atoms.audit_mismatch.get_or_insert_with(|| {
            format!(
                "native atom audit mismatch: atom at {location} ({:?}) on rows {before}/{after:?}: native {native}, reference {reference:?}",
                atom.test
            )
        });
        Err(AnalysisError::Inconsistent("native atom audit mismatch"))
    }
}

impl NativeReader for Reader<'_, '_> {
    fn reads_admitted(&self) -> bool {
        // `NativeValues::admit` converted every retained original, interning
        // names in event order, before the producer retained its row.
        true
    }

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
        let native = self.atoms.evaluate_with(
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
        )?;
        if self.atoms.audit
            && let Some(value) = native
        {
            self.audit(atom, before, after, value, control)?;
        }
        Ok(native)
    }

    fn validate_predicate_inputs(
        &mut self,
        atom: &PredicateAtom,
        before: usize,
        after: Option<usize>,
        control: &EvaluationControl,
    ) -> Result<bool, AnalysisError> {
        self.proofs.graph.row(before)?;
        if let Some(after) = after {
            self.proofs.graph.row(after)?;
        }
        self.atoms.validate_reuse(atom, control, |heap| {
            if !self.proofs.closed(heap, before)? {
                return Ok(false);
            }
            after.map_or(Ok(true), |row| self.proofs.closed(heap, row))
        })
    }

    fn predicate_reuse_storage(&mut self, bytes: usize, cap: u64, allocation: u64, columns: u64) {
        self.atoms.reuse.peak_bytes = self.atoms.reuse.peak_bytes.max(bytes);
        self.atoms.reuse.cap_bypasses += cap;
        self.atoms.reuse.allocation_bypasses += allocation;
        self.atoms.reuse.stored_columns += columns;
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "closed input and actual budget controls"
)]
mod tests {
    use super::*;
    use dive_runtime::budget::{Budget, Phase};

    /// The per-demand fuel the quota tests exhaust: small enough that their
    /// 10,000-element sources run out, as the run's [`NATIVE_FUEL`] does at
    /// larger sizes.
    const QUOTA_FUEL: u64 = 4096;

    fn prepare_source(
        predicate: &str,
    ) -> (crate::core_engine::Prepared, dive_runtime::CheckOptions) {
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
    ) -> (crate::core_engine::Prepared, dive_runtime::CheckOptions) {
        let analysis =
            dive_sema::analyze_source("Atoms.tla", source, &dive_sema::Options::default());
        assert!(analysis.accepted(), "{analysis:?}");
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let config = dive_config::parse(config).into_config().expect("config");
        let mut options = if tlc {
            dive_runtime::CheckOptions::tlc(0, 0)
        } else {
            dive_runtime::CheckOptions::dive(0)
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
        options: &dive_runtime::CheckOptions,
    ) -> (NativeGraph, NativeValues<'a>) {
        let mut sink = crate::core_native_graph::NativeSink::new(
            &prepared.model,
            prepared.store.clone(),
            dive_runtime::capture::Limits::default(),
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

    /// Evaluates `atom` natively on rows converted from `before`/`after`.
    /// When native evaluation produced a truth, the reference evaluator must
    /// return the same truth without printing a warning.
    fn agree(
        prepared: &crate::core_engine::Prepared,
        atoms: &mut NativeAtoms,
        atom: &PredicateAtom,
        control: &EvaluationControl,
        before: &[dive_value::Value],
        after: Option<&[dive_value::Value]>,
    ) -> Option<bool> {
        let ctx = prepared.model.evaluator.options().ctx;
        let mut heap = dive_core::Heap::new(prepared.store.clone(), false, false);
        let mut row = |values: &[dive_value::Value]| -> Vec<Handle> {
            values
                .iter()
                .map(|v| dive_core::convert::from_value(&mut heap, v, ctx).expect("row"))
                .collect()
        };
        let source = row(before);
        let target = after.map(&mut row);
        let native = atoms
            .evaluate(atom, &source, target.as_deref(), control)
            .expect("native");
        let vals = |values: &[dive_value::Value]| -> dive_eval::next::StateVals {
            values.iter().cloned().map(Some).collect()
        };
        let st = match after {
            Some(after) => dive_eval::St::action(vals(before), vals(after)),
            None => dive_eval::St::state(vals(before)),
        };
        // The reference runs only on a native truth: a declined query may be
        // one whose reference evaluation is deliberately enormous.
        if let Some(truth) = native {
            let capture = dive_eval::capture::PrintCapture::start();
            let reference = atom.evaluate(&prepared.model, &st);
            let printed = capture.take();
            assert!(
                matches!(reference, Ok(r) if r == truth),
                "native {truth}, reference {reference:?}"
            );
            assert!(printed.is_empty(), "native truth hid output: {printed:?}");
        }
        native
    }

    fn int(i: i64) -> dive_value::Value {
        dive_value::Value::Int(i)
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
                let expected = admissible_row(&heap, graph.row(row).expect("retained"));
                assert!(expected, "materialized tuples and sets are admitted");
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
                    dive_runtime::CheckOptions::tlc(0, 0)
                } else {
                    dive_runtime::CheckOptions::dive(0)
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
    fn cached_truth_input_hook_keeps_identity_store_poison_and_owned_stop_checks() {
        let source = "---- MODULE Atoms ----\nVARIABLE x\nInit == x = <<0>>\nNext == x' = <<1>>\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>(x = <<1>>)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let options = if tlc {
                    dive_runtime::CheckOptions::tlc(0, 0)
                } else {
                    dive_runtime::CheckOptions::dive(0)
                };
                let (graph, mut values) = capture(prepared, &options);
                let atom = driver
                    .native_predicates()
                    .find(|a| matches!(a.test, AtomTest::Nonstuttering { .. }))
                    .expect("WF action");
                let mut reader = Reader::new(&graph, &mut values, atoms);
                assert!(
                    !reader
                        .validate_predicate_inputs(&atom.clone(), 0, Some(1), control)
                        .expect("copy declines")
                );
                assert!(!reader.proofs.initialized);
                assert!(
                    reader
                        .validate_predicate_inputs(atom, 0, Some(1), control)
                        .expect("current admission")
                );
                assert!(
                    reader
                        .validate_predicate_inputs(atom, 0, Some(1), control)
                        .expect("positive inputs")
                );
                assert_eq!(
                    reader.atoms.evaluated, 0,
                    "hook supplies no evaluation or truth"
                );
                assert_eq!(reader.atoms.reuse.action, 2);
                assert_eq!(reader.proofs.metrics.hits, 2);
                let saved = std::mem::replace(
                    &mut reader.atoms.machine.heap,
                    dive_core::Heap::new(Arc::new(Store::default()), false, false),
                );
                assert!(
                    !reader
                        .validate_predicate_inputs(atom, 0, Some(1), control)
                        .expect("foreign Store")
                );
                reader.atoms.machine.heap = saved;
                let _ = reader.atoms.machine.heap.view(Handle::EMPTY);
                assert!(
                    !reader
                        .validate_predicate_inputs(atom, 0, Some(1), control)
                        .expect("current poison")
                );
                assert_eq!(reader.atoms.reuse.action, 2);
                let watchdog = Budget::new(Some(std::time::Duration::ZERO), None)
                    .start()
                    .expect("watchdog")
                    .expect("finite");
                reader.atoms.control =
                    EvaluationControl::new(watchdog.budget(), Phase::Capture).expect("owner");
                let temporal =
                    EvaluationControl::new(watchdog.budget(), Phase::Temporal).expect("phase");
                assert!(
                    matches!(reader.validate_predicate_inputs(atom,0,Some(1),&temporal),
                    Err(AnalysisError::Stopped(ref stop)) if stop.phase==Phase::Temporal)
                );
                assert_eq!(reader.atoms.reuse.action, 2);
            });
        }
    }

    #[test]
    fn reader_proof_hits_preserve_semantic_fallback_and_owned_stops() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nSpec == Init /\\ [][Next]_x\nP == <> (x \\div 0=0)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let options = if tlc {
                    dive_runtime::CheckOptions::tlc(0, 0)
                } else {
                    dive_runtime::CheckOptions::dive(0)
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
                atoms.fuel = QUOTA_FUEL;
                let options = if tlc {
                    dive_runtime::CheckOptions::tlc(0, 0)
                } else {
                    dive_runtime::CheckOptions::dive(0)
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
    fn concrete_rows_skip_the_walk_and_unproved_rows_keep_its_allowances() {
        let tuple = vec!["0"; ROW_WALK_NODES + 1].join(",");
        let source = format!(
            "---- MODULE Atoms ----\nVARIABLE x\nInit == x = <<{tuple}>>\nNext == UNCHANGED x\nP == <> (x=x)\n====\n"
        );
        for tlc in [false, true] {
            let (prepared, options) =
                prepare_text(&source, "INIT Init\nNEXT Next\nPROPERTY P\n", tlc);
            let (graph, _) = capture(&prepared, &options);
            let heap = dive_core::Heap::new(prepared.store.clone(), false, false);
            let row = graph.row(0).expect("retained");
            assert!(heap.proved_concrete(row[0]), "builders prove the tuple");
            let mut proofs = RowProofs::new(&graph);
            for _ in 0..2 {
                assert!(proofs.closed(&heap, 0).expect("concrete row"));
            }
            assert_eq!(proofs.metrics.scans, 1);
            assert_eq!(proofs.metrics.certified, 1);
            assert_eq!(proofs.metrics.hits, 1);
            // A name longer than the allowance makes every row walk and
            // count its names; this one is over the node allowance.
            let long = "n".repeat(ROW_NAME_BYTES + 1);
            if prepared.store.strings().intern(&long).is_err() {
                assert!(tlc, "only a sealed TLC string table refuses");
                continue;
            }
            let mut walked = RowProofs::new(&graph);
            for _ in 0..2 {
                assert!(!walked.closed(&heap, 0).expect("over the node allowance"));
            }
            assert_eq!(walked.metrics.scans, 2);
            assert_eq!(walked.metrics.certified, 0);
            assert!(!heap.poisoned());
        }
    }

    #[test]
    fn long_names_count_every_occurrence_against_the_row_allowance() {
        let store = Arc::new(Store::default());
        let mut heap = dive_core::Heap::new(store.clone(), false, false);
        let half = heap.string(&"h".repeat(ROW_NAME_BYTES / 2)).expect("text");
        let once = heap.tuple_literal(&[half]);
        let once = heap.intern_exact(once).expect("persistent");
        let twice = heap.tuple_literal(&[half, half, Handle::int(0).expect("zero")]);
        let twice = heap.intern_exact(twice).expect("persistent");
        assert!(
            admissible_row(&heap, &[once, twice]),
            "short names trust the bit"
        );
        heap.string(&"l".repeat(ROW_NAME_BYTES + 1))
            .expect("long text");
        assert!(admissible_row(&heap, &[once]));
        assert!(admissible_row(&heap, &[twice]));
        assert!(!admissible_row(&heap, &[once, twice]));
        assert!(!heap.poisoned());
    }

    #[test]
    fn fairness_checks_use_core_updates_on_any_function_domain() {
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
                // Key 1 is in each domain, so the first candidate changes x
                // and the reference reaches no missing key.
                let ctx = prepared.model.evaluator.options().ctx;
                for (lo, len) in [(0, 2), (1, 1), (1, 3)] {
                    let wrong = dive_value::Value::fcn(
                        (lo..lo + len).map(|k| (int(k), int(0))).collect(),
                        ctx,
                    )
                    .expect("function");
                    assert_eq!(
                        agree(prepared, atoms, enabled, control, &[wrong, int(0)], None),
                        Some(true),
                        "domain {lo}..{len}"
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
    fn materialized_rows_admit_sets_models_and_any_keys_and_refuse_symbolic_values() {
        let store = Arc::new(Store::default());
        let mut heap = dive_core::Heap::new(store.clone(), false, false);
        let zero = Handle::int(0).expect("integer");
        let function = heap.int_fcn(0, &[zero, Handle::bool(true)]);
        let function = heap.intern_exact(function).expect("persistent function");
        assert!(admissible_row(&heap, &[function]));
        let model = Handle::model(store.strings().intern("model").expect("name"));
        assert!(admissible_row(&heap, &[model]));
        let set = heap.set_from(&mut vec![zero, model]);
        let set = heap.intern_exact(set).expect("persistent set");
        assert!(admissible_row(&heap, &[set]));
        let transient = heap.tuple(&[zero]);
        assert!(!admissible_row(&heap, &[transient]));
        let tuple = heap.intern_exact(transient).expect("persistent tuple");
        let aggregate_key = heap.fcn_sorted(&[set, tuple], &[zero, model]);
        let aggregate_key = heap
            .intern_exact(aggregate_key)
            .expect("persistent function");
        assert!(admissible_row(&heap, &[aggregate_key, function, model]));
        let interval = heap.interval(1, 3);
        let interval = heap.intern_exact(interval).expect("persistent interval");
        let raw = heap.set_enum(&[Handle::int(2).expect("two"), zero]);
        let raw = heap.intern_exact(raw).expect("persistent enumeration");
        for symbolic in [interval, raw] {
            let nested = heap.tuple(&[symbolic]);
            let nested = heap.intern_exact(nested).expect("persistent");
            assert!(!admissible_row(&heap, &[symbolic]));
            assert!(!admissible_row(&heap, &[function, nested]));
        }
        assert!(!admissible_row(&heap, &[Handle::EMPTY]));
        assert!(!admissible_row(&heap, &[Handle::string(1 << 20)]));
        assert!(
            !heap.poisoned(),
            "refused handles do not inspect invalid compound handles"
        );
        let _ = heap.view(Handle::EMPTY);
        assert!(
            !admissible_row(&heap, &[function]),
            "current poison declines"
        );
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
    fn binder_alias_shadow_and_update_shapes_agree_with_reference() {
        // `None`: Core declines; a missing EXCEPT key (reference warning 2144)
        // and an EXCEPT path through a non-function stay with the reference.
        for tlc in [false, true] {
            for (defs, action, expected) in [
                (
                    "U(k) == [x EXCEPT ![k] = x[k]+1]",
                    "\\E j \\in 1..2: x'=U(j) /\\ UNCHANGED y",
                    Some(true),
                ),
                (
                    "U(k) == [x EXCEPT ![k] = x[k]+1]",
                    "\\E j \\in 1..2: x'=U(j+0) /\\ UNCHANGED y",
                    Some(true),
                ),
                (
                    "",
                    "\\E j \\in 1..2: (\\E k \\in 3..4: x'=[x EXCEPT ![k]=0]) /\\ UNCHANGED y",
                    None,
                ),
                (
                    "",
                    "\\E j \\in 1..y: x'=[x EXCEPT ![j]=0] /\\ UNCHANGED y",
                    Some(false),
                ),
                (
                    "",
                    "\\E j \\in {1,2}: x'=[x EXCEPT ![j]=0] /\\ UNCHANGED y",
                    Some(false),
                ),
                (
                    "",
                    "\\E j \\in 1..2: x'=[x EXCEPT ![j]=@+1] /\\ UNCHANGED y",
                    Some(true),
                ),
                (
                    "",
                    "\\E j \\in 1..2: x'=[x EXCEPT ![j]=1, ![j]=2] /\\ UNCHANGED y",
                    Some(true),
                ),
                (
                    "",
                    "\\E j \\in 1..2: x'=[x EXCEPT ![j][1]=1] /\\ UNCHANGED y",
                    None,
                ),
                (
                    "",
                    "\\E j \\in 1..2: x'=[(IF TRUE THEN x ELSE x) EXCEPT ![j]=1] /\\ UNCHANGED y",
                    Some(true),
                ),
            ] {
                let source = format!(
                    "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y\nInit == x = <<0,0>> /\\ y=2\nNext == UNCHANGED <<x,y>>\n{defs}\nA == {action}\nSpec == Init /\\ [][Next]_<<x,y>> /\\ WF_<<x,y>>(A)\nP == <> (y=1)\n====\n"
                );
                with_wrappers(&source, tlc, |prepared, driver, atoms, control| {
                    assert_eq!(atoms.eligible, atoms.total, "{action}");
                    let enabled = driver
                        .native_predicates()
                        .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                        .expect("enabled");
                    let x = dive_value::Value::tuple(vec![int(0), int(0)]);
                    let got = agree(prepared, atoms, enabled, control, &[x, int(2)], None);
                    assert_eq!(got, expected, "{action}, tlc={tlc}");
                });
            }
        }
    }

    #[test]
    fn rejection_and_fallback_histograms_name_the_first_reason() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == x'=1\nSpec == Init /\\ [][Next]_x\nP == <> (~ENABLED A)\n====\n";
        with_wrappers(source, false, |_, _, atoms, _| {
            let metrics = atoms.metrics();
            assert_eq!(metrics["rejections"]["negated_enabled"], 1);
            assert_eq!(metrics["eligible_atoms"], 0);
        });
        for (predicate, rejection) in [("2^x > 0", None::<&str>), ("x >= 0", None)] {
            let (prepared, options) = prepare_source(predicate);
            let driver = GraphCheck::prepare(
                &prepared.model,
                &options,
                dive_liveness::analyze::Limits::default(),
                dive_liveness::product::AcceptancePolicy::compact_default(),
                dive_liveness::analyze::GraphLabelPolicy::NoCellLimit,
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
            let rejections = atoms.metrics()["rejections"].clone();
            let rejected = rejections
                .as_object()
                .expect("histogram")
                .values()
                .map(|count| count.as_u64().expect("count"))
                .sum::<u64>();
            assert_eq!(
                rejected + u64::try_from(atoms.eligible).expect("count"),
                u64::try_from(atoms.total).expect("count")
            );
            if let Some(reason) = rejection {
                assert_eq!(rejections[reason], 1, "{predicate}: {rejections}");
                continue;
            }
            let atom = driver.native_predicates().next().expect("atom");
            let zero = [Handle::int(0).expect("zero")];
            assert_eq!(
                atoms
                    .evaluate(&atom.clone(), &zero, None, &control)
                    .expect("copied"),
                None
            );
            assert_eq!(
                atoms
                    .evaluate(atom, &[Handle::EMPTY], None, &control)
                    .expect("unproved row"),
                None
            );
            assert_eq!(
                atoms.evaluate(atom, &zero, None, &control).expect("native"),
                Some(true)
            );
            let fallbacks = atoms.metrics()["fallbacks"].clone();
            assert_eq!(fallbacks["not_compiled"], 1);
            assert_eq!(fallbacks["row_proof"], 1);
            assert_eq!(fallbacks["error"], 0);
            assert_eq!(fallbacks["fuel_exhausted"], 0);
            assert_eq!(atoms.metrics()["fallback"], 2);
        }
        let (prepared, options) = prepare_source("x \\div 0 = 0");
        let driver = GraphCheck::prepare(
            &prepared.model,
            &options,
            dive_liveness::analyze::Limits::default(),
            dive_liveness::product::AcceptancePolicy::compact_default(),
            dive_liveness::analyze::GraphLabelPolicy::NoCellLimit,
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
        let atom = driver.native_predicates().next().expect("atom");
        assert_eq!(
            atoms
                .evaluate(atom, &[Handle::int(0).expect("zero")], None, &control)
                .expect("semantic error"),
            None
        );
        assert_eq!(atoms.metrics()["fallbacks"]["error"], 1);
    }

    #[test]
    fn recursive_operators_powers_and_memo_free_plans_run_natively() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nRECURSIVE Sum(_)\nSum(n) == IF n = 0 THEN 0 ELSE n + Sum(n-1)\nf[n \\in 0..200] == IF n = 0 THEN 0 ELSE f[n-1] + 1\nInit == x=0\nNext == x'=x+1\nSpec == Init /\\ [][Next]_x\nP == <> (Sum(x) = 6 /\\ f[x] = x /\\ 2^(64 * x) > 2^60)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                assert_eq!(atoms.eligible, 1, "{}", atoms.metrics()["rejections"]);
                assert!(
                    !atoms.compiled[0].plan.code.funs.is_empty(),
                    "a recursive plan"
                );
                let atom = driver
                    .native_predicates()
                    .find(|atom| atoms.has(atom))
                    .expect("atom");
                if tlc {
                    // 2^192 overflows a Java int: an error, which falls back.
                    assert_eq!(agree(prepared, atoms, atom, control, &[int(3)], None), None);
                    assert_eq!(atoms.metrics()["fallbacks"]["error"], 1);
                    return;
                }
                assert_eq!(
                    agree(prepared, atoms, atom, control, &[int(3)], None),
                    Some(true)
                );
                assert_eq!(
                    agree(prepared, atoms, atom, control, &[int(2)], None),
                    Some(false)
                );
                // Past Core's call-depth limit the error falls back, and the
                // next row starts from a clean frame and depth.
                assert_eq!(
                    agree(prepared, atoms, atom, control, &[int(100)], None),
                    None
                );
                assert_eq!(atoms.machine.frame, Vec::<Handle>::new());
                assert_eq!(
                    agree(prepared, atoms, atom, control, &[int(3)], None),
                    Some(true)
                );
                let metrics = atoms.metrics();
                assert_eq!(metrics["fallbacks"]["error"], 1);
                assert_eq!(metrics["fallbacks"]["fuel_exhausted"], 0);
                assert_eq!(metrics["evaluated"], 3);
            });
        }
    }

    #[test]
    fn fuel_exhaustion_falls_back_and_retires_an_atom_after_consecutive_strikes() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x+1\nSpec == Init /\\ [][Next]_x\nP == <> (x = 0 \\/ \\A i \\in 1..10000 : i + x > 0)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let atom = driver
                    .native_predicates()
                    .find(|atom| atoms.has(atom))
                    .expect("atom");
                // The run's allowance covers this 10,000-element source.
                assert_eq!(atoms.fuel, NATIVE_FUEL);
                assert_eq!(
                    agree(prepared, atoms, atom, control, &[int(5)], None),
                    Some(true)
                );
                atoms.fuel = QUOTA_FUEL;
                for _ in 1..FUEL_STRIKES {
                    assert_eq!(agree(prepared, atoms, atom, control, &[int(5)], None), None);
                }
                // A demand that completes resets the count.
                assert_eq!(
                    agree(prepared, atoms, atom, control, &[int(0)], None),
                    Some(true)
                );
                for _ in 0..FUEL_STRIKES {
                    assert_eq!(agree(prepared, atoms, atom, control, &[int(5)], None), None);
                }
                // Retired: even a cheap row now goes to the reference evaluator.
                assert_eq!(agree(prepared, atoms, atom, control, &[int(0)], None), None);
                let metrics = atoms.metrics();
                assert_eq!(
                    metrics["fallbacks"]["fuel_exhausted"],
                    u64::from(2 * FUEL_STRIKES - 1)
                );
                assert_eq!(metrics["fallbacks"]["error"], 0);
                assert_eq!(metrics["fallbacks"]["fuel_retired"], 1);
                assert_eq!(metrics["fuel"]["retired_atoms"], 1);
                assert_eq!(metrics["evaluated"], 2);
            });
        }
    }

    #[test]
    fn audit_compares_native_truth_with_reference_and_records_the_first_mismatch() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE y\nInit == y=0\nA == y'=1-y\nSpec == Init /\\ [][A]_y /\\ WF_y(A)\nP == <> (y=1)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let options = if tlc {
                    dive_runtime::CheckOptions::tlc(0, 0)
                } else {
                    dive_runtime::CheckOptions::dive(0)
                };
                let (graph, mut values) = capture(prepared, &options);
                atoms.audit = true;
                let state_atom = driver
                    .native_predicates()
                    .find(|atom| {
                        matches!(atom.test, AtomTest::Predicate)
                            && atom.kind == dive_liveness::formula::AtomKind::State
                    })
                    .expect("state predicate");
                let reads = values.metrics()["read_rows"].as_u64().expect("reads");
                let mut reader = Reader::new(&graph, &mut values, atoms);
                assert!(reader.reads_admitted());
                assert_eq!(
                    reader
                        .predicate(state_atom, 0, None, control)
                        .expect("audited agreement"),
                    Some(false)
                );
                assert!(reader.atoms.audit_mismatch().is_none());
                assert!(matches!(
                    reader.audit(state_atom, 0, None, true, control),
                    Err(AnalysisError::Inconsistent("native atom audit mismatch"))
                ));
                drop(reader);
                assert_eq!(
                    values.metrics()["read_rows"].as_u64().expect("reads"),
                    reads + 2,
                    "the audit reads the reference row only for its own comparison"
                );
                let message = atoms.audit_mismatch().expect("mismatch").to_owned();
                assert!(message.contains("rows 0/None"), "{message}");
                assert!(message.contains("native true"), "{message}");
                assert!(message.contains("Ok(false)"), "{message}");
                assert_eq!(atoms.metrics()["audit"], true);
            });
        }
    }

    /// Sets, SUBSET/UNION, filters, Cardinality, CHOOSE over sets, records,
    /// multi-clause and nested EXCEPT/@, several-binder constructors, strings
    /// and parameterized fairness all run on Core and agree with the
    /// reference on every retained row and row pair, in both profiles.
    #[test]
    fn admitted_core_shapes_agree_with_reference_on_every_row() {
        let source = r#"---- MODULE Atoms ----
EXTENDS Naturals, FiniteSets
VARIABLES S, r, f
vars == <<S, r, f>>
Init == S = {} /\ r = [a |-> 0, b |-> "u"] /\ f = <<<<0, 0>>, <<0, 0>>>>
Add(e) == /\ e \notin S
          /\ S' = S \cup {e}
          /\ r' = [r EXCEPT !.a = (@ + e) % 3, !.b = IF e = 2 THEN "v" ELSE @]
          /\ f' = [f EXCEPT ![1][2] = e, ![2] = <<@[2], @[1]>>]
Drop == /\ S # {}
        /\ S' = S \ {CHOOSE e \in S : \A d \in S : e <= d}
        /\ UNCHANGED <<r, f>>
Next == (\E e \in 1..3 : Add(e)) \/ Drop
Spec == Init /\ [][Next]_vars /\ \A e \in 1..3 : WF_vars(Add(e)) /\ WF_vars(Drop)
Pick == CHOOSE T \in SUBSET S : Cardinality(T) = 1 \/ T = S
Name == CHOOSE s \in {"v", "u", "w"} : s # r.b
P == <>(/\ Pick = {}
        /\ Cardinality({e \in S : e > 1}) = 2
        /\ \E T \in SUBSET S : UNION {T, {7}} = {7, 3}
        /\ [i \in 1..2, j \in S |-> i + j] # [i \in 1..2, j \in {} |-> 0]
        /\ Name \in {"u", "w"}
        /\ (CHOOSE g \in {f[1], f[2]} : TRUE) = f[2])
====
"#;
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                assert_eq!(atoms.eligible, atoms.total, "{}", atoms.metrics());
                let options = if tlc {
                    dive_runtime::CheckOptions::tlc(0, 0)
                } else {
                    dive_runtime::CheckOptions::dive(0)
                };
                let (graph, mut values) = capture(prepared, &options);
                atoms.audit = true;
                let rows = graph.input().retained_rows;
                assert!(rows > 8, "{rows}");
                let mut reader = Reader::new(&graph, &mut values, atoms);
                for atom in driver.native_predicates() {
                    for before in 0..rows {
                        let targets: Vec<Option<usize>> =
                            if atom.kind == dive_liveness::formula::AtomKind::Action {
                                (0..rows.min(12)).map(Some).collect()
                            } else {
                                vec![None]
                            };
                        for after in targets {
                            let got = reader
                                .predicate(atom, before, after, control)
                                .expect("audited agreement");
                            assert!(got.is_some(), "{:?} on {before}/{after:?}", atom.test);
                        }
                    }
                }
                drop(reader);
                assert!(atoms.audit_mismatch().is_none());
                assert_eq!(atoms.fallback, 0, "{}", atoms.metrics());
            });
        }
    }

    #[test]
    fn choose_errors_before_the_first_witness_fall_back_and_to_string_is_refused() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals, TLC\nVARIABLE S\nInit == S = {1, 2}\nNext == UNCHANGED S\nSpec == Init /\\ [][Next]_S\nP == <> ((CHOOSE e \\in S : 2 \\div (e - 1) > 0) = 2)\nQ == <> (ToString(S) = \"x\")\n====\n";
        for tlc in [false, true] {
            let (prepared, options) = prepare_text(
                source,
                "SPECIFICATION Spec\nPROPERTY P\nPROPERTY Q\nCHECK_DEADLOCK FALSE\n",
                tlc,
            );
            let driver = GraphCheck::prepare(
                &prepared.model,
                &options,
                dive_liveness::analyze::Limits::default(),
                dive_liveness::product::AcceptancePolicy::compact_default(),
                dive_liveness::analyze::GraphLabelPolicy::NoCellLimit,
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
            assert_eq!(atoms.metrics()["rejections"]["fresh_string"], 1);
            assert_eq!(atoms.eligible, 1);
            let atom = driver
                .native_predicates()
                .find(|atom| atoms.has(atom))
                .expect("CHOOSE atom");
            let ctx = prepared.model.evaluator.options().ctx;
            let set = dive_value::Value::set(vec![int(1), int(2)], ctx).expect("set");
            // The first element in canonical order, 1, divides by zero
            // before 2 could witness: the reference must raise it.
            assert_eq!(
                agree(&prepared, &mut atoms, atom, &control, &[set], None),
                None
            );
            assert_eq!(atoms.metrics()["fallbacks"]["error"], 1);
            let set = dive_value::Value::set(vec![int(2), int(3)], ctx).expect("set");
            assert_eq!(
                agree(&prepared, &mut atoms, atom, &control, &[set], None),
                Some(true)
            );
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
                    atoms.fuel = QUOTA_FUEL;
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
    fn product_sources_run_natively_and_large_ones_and_forall_recover_after_quota() {
        for tlc in [false, true] {
            for (domain, expected) in [
                ("(1..2) \\X (1..2)", Some(false)),
                ("(1..1000000000) \\X (1..2)", None),
                ("(1..2) \\X ((1..1000000000) \\X (1..2))", None),
            ] {
                let source = format!(
                    "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == \\E <<i,j>> \\in {domain}: x'=i /\\ x=i\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n"
                );
                with_wrappers(&source, tlc, |prepared, driver, atoms, control| {
                    atoms.fuel = QUOTA_FUEL;
                    assert_eq!(atoms.eligible, 1, "Core plan facts admit products");
                    let atom = driver.native_predicates().next().expect("enabled");
                    assert_eq!(
                        agree(prepared, atoms, atom, control, &[int(0)], None),
                        expected,
                        "{domain}"
                    );
                });
            }
            let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == \\A i \\in 1..x: x'=x\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n";
            with_wrappers(source, tlc, |_, driver, atoms, control| {
                atoms.fuel = QUOTA_FUEL;
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
                // Key 1 is the first candidate: wherever it exists, the first
                // successor changes x and the reference reaches no missing key.
                // Boolean keys make the comparison fail, so Core declines.
                let ctx = prepared.model.evaluator.options().ctx;
                let bool = dive_value::Value::Bool;
                for (keys, want) in [
                    (vec![int(1), int(2)], Some(true)),
                    (vec![int(0), int(1)], Some(true)),
                    (vec![int(1)], Some(true)),
                    (vec![int(1), int(2), int(3)], Some(true)),
                    (vec![int(0)], None),
                    (vec![bool(false), bool(true)], None),
                ] {
                    let f = dive_value::Value::fcn(
                        keys.iter().map(|k| (k.clone(), int(0))).collect(),
                        ctx,
                    )
                    .expect("function");
                    assert_eq!(
                        agree(prepared, atoms, atom, control, &[f, int(0)], None),
                        want,
                        "keys={keys:?}"
                    );
                }
            });
            let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == \\E i \\in 1..10000: x'=i /\\ i=x\nSpec == Init /\\ [][Next]_x\nP == <> (ENABLED A)\n====\n";
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                atoms.fuel = QUOTA_FUEL;
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
                atoms.fuel = QUOTA_FUEL;
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
                    atoms.fuel = QUOTA_FUEL;
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
