//! Optional native predicates admitted by Core plan facts.
//!
//! A retained atom runs natively when its exact object and source scope bind,
//! the Core bound-test compiler accepts it, its plan is effect-free and cannot
//! add a string to the shared table, and every compiled constant and input row
//! is a materialized Core value. Recursive operators, memo
//! sites (evaluated directly: the analysis Machine memoizes nothing) and
//! powers are admitted. Anything else, and every error, fuel exhaustion,
//! warning or printed output, keeps the dive-eval evaluator's call.

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

pub(crate) mod bound_next;
mod emissions;
mod prefix;

struct Compiled {
    batch: Option<dive_core::compile::ActionBatch>,
    column: Option<prefix::Column>,
    emission: Option<(usize, dive_core::compile::ClosedEnabled)>,
    producer: Option<usize>,
    identity: *const PredicateAtom,
    plan: dive_core::plan::Plan,
    false_guard: Option<dive_core::plan::P>,
    unchanged_guard: Option<(dive_core::plan::P, usize)>,
    /// A directly syntactic `~ENABLED A` atom: the plan searches `A` and the
    /// truth is its negation, as the dive-eval evaluator negates its search.
    negate: bool,
    enabled_prefix: Option<(dive_core::plan::P, Projection)>,
    direct_stutter: bool,
    state_key: Option<Projection>,
    guard_key: Option<Projection>,
    /// Fuel exhaustions in a row; at [`FUEL_STRIKES`] the atom stops running
    /// natively in this analysis.
    exhausted_in_a_row: u32,
}

/// A site is private to one compiled atom, its bound parameters, the run's
/// store and one query (whole truth or source prefix). Only graph handles
/// surviving heap resets may enter its exact key.
#[derive(Clone, Copy)]
struct Projection {
    mask: u64,
    site: u32,
}

impl Projection {
    fn new(mask: Option<u64>, width: usize, lengths: &mut Vec<usize>) -> Option<Self> {
        let mask = mask?;
        let len = mask.count_ones() as usize;
        if len >= width || len > dive_core::memo::MAX_KEY {
            return None;
        }
        let site = u32::try_from(lengths.len()).ok()?;
        lengths.push(len);
        Some(Self { mask, site })
    }

    fn bind(self, row: &[Handle]) -> Option<ProjectedKey> {
        let mut values = [Handle::EMPTY; dive_core::memo::MAX_KEY];
        let mut mask = self.mask;
        let mut len = 0;
        while mask != 0 {
            let val = *row.get(mask.trailing_zeros() as usize)?;
            if val.is_transient() || val.is_empty() {
                return None;
            }
            values[len] = val;
            len += 1;
            mask &= mask - 1;
        }
        Some(ProjectedKey {
            site: self.site,
            values,
            len,
        })
    }
}

struct ProjectedKey {
    site: u32,
    values: [Handle; dive_core::memo::MAX_KEY],
    len: usize,
}

impl ProjectedKey {
    fn get(&self, memo: &mut dive_core::memo::Memo) -> Option<bool> {
        memo.get(self.site, &self.values[..self.len])?.as_bool()
    }

    fn put(&self, memo: &mut dive_core::memo::Memo, truth: bool) {
        memo.put(self.site, &self.values[..self.len], Handle::bool(truth));
    }
}

/// Node visits one native atom demand may spend before it falls back. The
/// run's time and memory budgets still bound the work through
/// [`QueryControl`], polled at least once per
/// [`dive_core::eval::WORK_POLL_UNITS`] charged units; this allowance only
/// bounds the work wasted, and the memory held until `heap.reset()`, before
/// the dive-eval evaluator answers instead.
const NATIVE_FUEL: u64 = 1 << 20;

/// Consecutive fuel exhaustions after which an atom is left to the dive-eval
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
    prefix_owner: Option<Arc<()>>,
    prefix_bytes: usize,
    prefix_cells_reused: u64,
    live_emissions: bool,
    producer_images: [Option<dive_core::compile::ClosedEnabled>; 64],
    image_hits: u64,
    emission_hits: [u64; 2],
    emission_peak_bytes: usize,
    projected: dive_core::memo::Memo,
    projected_state_hits: u64,
    projected_enabled_hits: u64,
    disabled_prefixes: u64,
    projected_guard_hits: u64,
    stutter_hits: u64,
    control: Arc<EvaluationControl>,
    compiled: Vec<Compiled>,
    /// The last atom [`NativeAtoms::position`] found, with its index.
    last_position: std::cell::Cell<Option<(*const PredicateAtom, usize)>>,
    eligible: usize,
    total: usize,
    evaluated: u64,
    fallback: u64,
    eligible_state: usize,
    eligible_action: usize,
    batch_rows: u64,
    guard_probes: u64,
    unchanged_probes: u64,
    unchanged_rows: u64,
    unchanged_edges: u64,
    guard_rows: u64,
    evaluated_state: u64,
    evaluated_action: u64,
    evaluated_enabled: u64,
    evaluated_nonstuttering: u64,
    evaluated_negated_enabled: u64,
    /// Label demands the caller answered with the dive-eval evaluator because no
    /// native truth was available (unprepared atoms and fallbacks).
    reference: ReferenceMetrics,
    row_proofs: RowProofMetrics,
    reuse: ReuseMetrics,
    analysis_ms: std::collections::BTreeMap<&'static str, f64>,
    rejections: Rejections,
    fallbacks: Fallbacks,
    /// Per-demand fuel ([`NATIVE_FUEL`]; tests lower it).
    fuel: u64,
    /// Atoms that reached [`FUEL_STRIKES`].
    fuel_retired: u64,
    /// `DIVE_NATIVE_ATOM_AUDIT=1`: re-evaluate every native truth with the dive-eval evaluator.
    audit: bool,
    audit_mismatch: Option<String>,
}

/// Why a native predicate was not prepared, by its first failing check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rejection {
    ForeignScope,
    CompileError,
    Effects,
    FreshString,
    ConstUnclosed,
    Allocation,
}

#[derive(Default)]
struct Rejections {
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
        serde_json::json!({"foreign_scope": self.foreign_scope, "compile_error": self.compile_error,
            "effects": self.effects,
            "fresh_string": self.fresh_string, "const_unclosed": self.const_unclosed,
            "allocation": self.allocation})
    }
}

/// Why an actual native demand returned to the dive-eval evaluator.
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

/// Reference label evaluations by atom domain.
#[derive(Default)]
struct ReferenceMetrics {
    state: u64,
    action: u64,
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
        Self::prepare_atoms(
            model,
            store,
            driver.native_predicates(),
            control,
            strategies,
        )
    }

    /// [`NativeAtoms::prepare`] for an explicit set of retained atoms, which
    /// must outlive every evaluation (simulation's per-run claims).
    pub(crate) fn prepare_atoms<'a>(
        model: &Model,
        store: &Arc<Store>,
        atoms: impl IntoIterator<Item = &'a PredicateAtom>,
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
        let mut result = Self::empty(model, store, control, opts);
        let mut key_lengths = Vec::new();
        for atom in atoms {
            result.total += 1;
            // (a) The exact retained atom and its source scope must bind to
            // this model's evaluator; equal source text is not enough. A
            // `~ENABLED A` atom validates both its source expression and the
            // pure action it searches, as the dive-eval evaluator does.
            let sub = match atom.test {
                AtomTest::Enabled { sub } => sub,
                AtomTest::Nonstuttering { sub } => Some(sub),
                AtomTest::NegatedEnabled { action } => Some(action),
                AtomTest::Predicate => None,
            };
            let ev = &model.evaluator;
            if !ev.admits_scoped_expr(&atom.pred.scope, atom.pred.expr)
                || sub.is_some_and(|sub| !ev.admits_scoped_expr(&atom.pred.scope, sub))
            {
                result.rejections.record(Rejection::ForeignScope);
                continue;
            }
            let (plan, false_guard) = match compile(model, store, atom, opts, &result.machine.heap)
            {
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
                producer: if false_guard
                    .batch
                    .as_ref()
                    .is_some_and(dive_core::compile::ActionBatch::accepts_generated_image)
                {
                    bound_next::action_index(model, atom.pred.expr, &atom.pred.scope)
                } else {
                    None
                },
                batch: false_guard.batch,
                column: None,
                emission: false_guard.closed_enabled.and_then(|cert| {
                    let action = match atom.test {
                        AtomTest::Enabled { .. } => atom.pred.expr,
                        AtomTest::NegatedEnabled { action } => action,
                        _ => return None,
                    };
                    bound_next::action_index(model, action, &atom.pred.scope)
                        .map(|index| (index, cert))
                }),
                enabled_prefix: false_guard.enabled_prefix.and_then(|(p, reads)| {
                    Projection::new(Some(reads), model.ir.vars().len(), &mut key_lengths)
                        .map(|site| (p, site))
                }),
                direct_stutter: false_guard.direct_stutter,
                state_key: Projection::new(
                    false_guard.state_reads,
                    model.ir.vars().len(),
                    &mut key_lengths,
                ),
                guard_key: Projection::new(
                    false_guard.guard_reads,
                    model.ir.vars().len(),
                    &mut key_lengths,
                ),
                identity: std::ptr::from_ref(atom),
                plan,
                false_guard: false_guard.source,
                unchanged_guard: false_guard.unchanged,
                negate: matches!(atom.test, AtomTest::NegatedEnabled { .. }),
                exhausted_in_a_row: 0,
            });
            result.eligible += 1;
            match atom.kind {
                dive_liveness::formula::AtomKind::State => result.eligible_state += 1,
                dive_liveness::formula::AtomKind::Action => result.eligible_action += 1,
            }
        }
        result.bind_generated_images();
        result.projected = dive_core::memo::Memo::new(&key_lengths);
        result.projected.set_cap(8 << 20);
        control.check().map_err(|cause| {
            control
                .verdict(&EvalError::interrupted(cause))
                .unwrap_or_else(|| dive_runtime::Verdict::InternalError {
                    message: "native predicate preparation interruption lacks run authority".into(),
                })
        })?;
        Ok(result)
    }

    fn bind_generated_images(&mut self) {
        for compiled in &self.compiled {
            if let Some((id, cert)) = compiled.emission
                && let Some(item) = self.producer_images.get_mut(id)
            {
                *item = Some(cert);
            }
        }
    }

    fn empty(
        model: &Model,
        store: &Arc<Store>,
        control: &Arc<EvaluationControl>,
        opts: Opts,
    ) -> Self {
        let mut result = Self {
            prefix_owner: None,
            prefix_bytes: 0,
            prefix_cells_reused: 0,
            live_emissions: false,
            producer_images: [None; 64],
            image_hits: 0,
            emission_hits: [0; 2],
            emission_peak_bytes: 0,
            projected: dive_core::memo::Memo::default(),
            projected_state_hits: 0,
            projected_enabled_hits: 0,
            disabled_prefixes: 0,
            projected_guard_hits: 0,
            stutter_hits: 0,
            machine: Machine::new(
                Arc::clone(store),
                model.ir.vars().len(),
                opts,
                dive_core::memo::Memo::default(),
            ),
            control: Arc::clone(control),
            compiled: Vec::new(),
            last_position: std::cell::Cell::new(None),
            eligible: 0,
            total: 0,
            evaluated: 0,
            fallback: 0,
            eligible_state: 0,
            eligible_action: 0,
            batch_rows: 0,
            guard_probes: 0,
            unchanged_probes: 0,
            unchanged_rows: 0,
            unchanged_edges: 0,
            guard_rows: 0,
            evaluated_state: 0,
            evaluated_action: 0,
            evaluated_enabled: 0,
            evaluated_nonstuttering: 0,
            evaluated_negated_enabled: 0,
            reference: ReferenceMetrics::default(),
            row_proofs: RowProofMetrics::default(),
            reuse: ReuseMetrics::default(),
            analysis_ms: std::collections::BTreeMap::new(),
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
        // dive-eval call would not have added (string order under
        // `--compat=tlc`); a new text is an unsupported error and falls back.
        result.machine.heap.set_existing_strings_only(true);
        // A warning Core does not record (2144) must reach the
        // dive-eval evaluator, which prints it.
        result.machine.set_refuse_unrecorded_warnings(true);
        result
    }

    pub(crate) fn analysis_ms(&self) -> &std::collections::BTreeMap<&'static str, f64> {
        &self.analysis_ms
    }

    pub(crate) fn metrics(&self) -> serde_json::Value {
        serde_json::json!({"total_atoms": self.total, "eligible_atoms": self.eligible,
            "action_batches":{"plans":self.compiled.iter().filter(|p|p.batch.is_some()).count(),"rows":self.batch_rows, "producer_images": self.image_hits},
            "unchanged_target_guards":{"plans":self.compiled.iter().filter(|p|p.unchanged_guard.is_some()).count(),"probes":self.unchanged_probes,"rows":self.unchanged_rows,"edges":self.unchanged_edges},
            "action_source_guards": {"plans": self.compiled.iter().filter(|p| p.false_guard.is_some()).count(),
                "probes": self.guard_probes, "false_rows": self.guard_rows},
            "closed_enabled": {"plans": self.compiled.iter().filter(|c|c.emission.is_some()).count(),
                "absent": self.emission_hits[0], "present": self.emission_hits[1], "peak_mask_bytes": self.emission_peak_bytes},
            "periodic_prefix": {"cells_reused": self.prefix_cells_reused, "table_bytes":self.prefix_bytes},
            "projected_truth": {"state_hits": self.projected_state_hits, "enabled_hits": self.projected_enabled_hits,
                "enabled_prefix_false": self.disabled_prefixes, "guard_hits": self.projected_guard_hits, "stutters": self.stutter_hits, "table_bytes": self.projected.bytes()},
            "evaluated": self.evaluated, "fallback": self.fallback,
            "eligible_state_atoms": self.eligible_state, "eligible_action_atoms": self.eligible_action,
            "state_evaluations": self.evaluated_state, "action_evaluations": self.evaluated_action,
            "enabled_evaluations": self.evaluated_enabled, "nonstuttering_evaluations": self.evaluated_nonstuttering,
            "negated_enabled_evaluations": self.evaluated_negated_enabled,
            "reference_evaluations": {"state": self.reference.state, "action": self.reference.action,
                "scope": "label demands answered by the reference evaluator: unprepared atoms and fallbacks"},
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
        self.position(atom).is_some()
    }

    /// The prepared plan of exactly this atom object. The analysis asks for
    /// one atom on every row and edge before the next, so the last hit is
    /// remembered; `compiled` is fixed after preparation, so its index stays
    /// valid.
    fn position(&self, atom: &PredicateAtom) -> Option<usize> {
        let identity = std::ptr::from_ref(atom);
        if let Some((last, index)) = self.last_position.get()
            && std::ptr::eq(last, identity)
        {
            return Some(index);
        }
        let index = self
            .compiled
            .iter()
            .position(|item| std::ptr::eq(item.identity, identity))?;
        self.last_position.set(Some((identity, index)));
        Some(index)
    }

    /// Whether every retained liveness atom was prepared natively, so the
    /// dive-eval evaluator answers only an actual demand fallback.
    pub(crate) fn all_native(&self) -> bool {
        self.eligible == self.total
    }

    /// The first disagreement between native and dive-eval evaluation found by the audit.
    pub(crate) fn audit_mismatch(&self) -> Option<&str> {
        self.audit_mismatch.as_deref()
    }

    /// Whether `DIVE_NATIVE_ATOM_AUDIT=1` asks the caller to re-evaluate
    /// every native truth by reference.
    pub(crate) const fn audits(&self) -> bool {
        self.audit
    }

    /// The native truth of `atom` on rows the caller holds outside a checked
    /// graph, each certified materialized here (`admissible_row`); `None`
    /// leaves the demand to the dive-eval evaluator.
    pub(crate) fn evaluate(
        &mut self,
        atom: &PredicateAtom,
        before: &[Handle],
        after: Option<&[Handle]>,
        control: &EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        self.evaluate_with(atom, before, after, control, false, |heap| {
            Ok(admissible_row(heap, before) && after.is_none_or(|row| admissible_row(heap, row)))
        })
    }

    fn evaluate_with(
        &mut self,
        atom: &PredicateAtom,
        before: &[Handle],
        after: Option<&[Handle]>,
        control: &EvaluationControl,
        graph_owned: bool,
        inputs_closed: impl FnOnce(&dive_core::Heap) -> Result<bool, AnalysisError>,
    ) -> Result<Option<bool>, AnalysisError> {
        check(&self.control, control)?;
        let Some(index) = self.active_atom(atom) else {
            return Ok(None);
        };
        let compiled = &self.compiled[index];
        let negate = compiled.negate;
        // The exact object came from this still-live driver's fixed plan Vec;
        // no expression ID, scope guess or extensional row equality is a key.
        if !inputs_closed(&self.machine.heap)? {
            self.fallback += 1;
            self.fallbacks.row_proof += 1;
            return Ok(None);
        }
        if graph_owned
            && compiled.direct_stutter
            && self.fuel == NATIVE_FUEL
            && after.is_some_and(|row| std::ptr::eq(row, before))
        {
            self.stutter_hits += 1;
            self.compiled[index].exhausted_in_a_row = 0;
            return Ok(Some(false));
        }
        let key = compiled
            .state_key
            .filter(|p| {
                graph_owned
                    && after.is_none()
                    && self.fuel == NATIVE_FUEL
                    && self.projected.active(p.site)
            })
            .and_then(|p| p.bind(before));
        if let Some(value) = key.as_ref().and_then(|k| k.get(&mut self.projected)) {
            self.projected_state_hits += 1;
            self.projected_enabled_hits += u64::from(matches!(
                atom.test,
                AtomTest::Enabled { .. } | AtomTest::NegatedEnabled { .. }
            ));
            self.compiled[index].exhausted_in_a_row = 0;
            return Ok(Some(value));
        }
        if graph_owned
            && after.is_none()
            && self.fuel == NATIVE_FUEL
            && Self::prefix_false(
                compiled,
                before,
                &mut self.projected,
                &mut self.machine,
                (&self.control, control),
            )?
        {
            self.disabled_prefixes += 1;
            self.compiled[index].exhausted_in_a_row = 0;
            if let Some(key) = key {
                key.put(&mut self.projected, negate);
            }
            return Ok(Some(negate));
        }
        let result = run_bound(
            &compiled.plan,
            &mut self.machine,
            before,
            after,
            self.fuel,
            None,
        );
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
            self.count_native(atom);
            let truth = value != negate;
            if let Some(key) = key
                && !self.machine.heap.poisoned()
            {
                key.put(&mut self.projected, truth);
            }
            Ok(Some(truth))
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

    fn active_atom(&mut self, atom: &PredicateAtom) -> Option<usize> {
        let Some(index) = self.position(atom) else {
            self.fallback += 1;
            self.fallbacks.not_compiled += 1;
            return None;
        };
        if self.compiled[index].exhausted_in_a_row >= FUEL_STRIKES {
            self.fallback += 1;
            self.fallbacks.fuel_retired += 1;
            return None;
        }
        Some(index)
    }

    fn count_native(&mut self, atom: &PredicateAtom) {
        self.evaluated += 1;
        match atom.test {
            AtomTest::Enabled { .. } => self.evaluated_enabled += 1,
            AtomTest::Nonstuttering { .. } => self.evaluated_nonstuttering += 1,
            AtomTest::NegatedEnabled { .. } => self.evaluated_negated_enabled += 1,
            AtomTest::Predicate => {}
        }
        match atom.kind {
            dive_liveness::formula::AtomKind::State => self.evaluated_state += 1,
            dive_liveness::formula::AtomKind::Action => self.evaluated_action += 1,
        }
    }

    fn prefix_false(
        compiled: &Compiled,
        before: &[Handle],
        memo: &mut dive_core::memo::Memo,
        machine: &mut Machine,
        control: (&EvaluationControl, &EvaluationControl),
    ) -> Result<bool, AnalysisError> {
        if let Some((prefix, site)) = compiled.enabled_prefix
            && memo.active(site.site)
            && let Some(prefix_key) = site.bind(before)
        {
            let prefix_value = if let Some(v) = prefix_key.get(memo) {
                Some(v)
            } else {
                let result = run_bound(
                    &compiled.plan,
                    machine,
                    before,
                    None,
                    NATIVE_FUEL,
                    Some(prefix),
                );
                check(control.0, control.1)?;
                result
                    .ok()
                    .filter(|_| {
                        !machine.heap.poisoned()
                            && machine.warnings.is_empty()
                            && machine.printed.is_empty()
                    })
                    .inspect(|&v| prefix_key.put(memo, v))
            };
            return Ok(prefix_value == Some(false));
        }
        Ok(false)
    }

    /// All target rows in this checked graph have already been certified.
    /// Try to disprove the bound action from the source alone. Any unknown
    /// result keeps every ordinary edge demand, including its error path.
    fn false_guard(
        &mut self,
        atom: &PredicateAtom,
        before: &[Handle],
        control: &EvaluationControl,
    ) -> Result<bool, AnalysisError> {
        check(&self.control, control)?;
        let Some(index) = self.position(atom) else {
            return Ok(false);
        };
        let compiled = &mut self.compiled[index];
        if compiled.exhausted_in_a_row >= FUEL_STRIKES || self.audit {
            return Ok(false);
        }
        let Some(guard) = compiled.false_guard else {
            return Ok(false);
        };
        // The full action starts with And, Not and a proven-direct SameVars.
        let Some(fuel) = self.fuel.checked_sub(3) else {
            return Ok(false);
        };
        let key = compiled
            .guard_key
            .filter(|p| self.fuel == NATIVE_FUEL && self.projected.active(p.site))
            .and_then(|p| p.bind(before));
        if let Some(holds) = key.as_ref().and_then(|k| k.get(&mut self.projected)) {
            self.projected_guard_hits += 1;
            if !holds {
                compiled.exhausted_in_a_row = 0;
            }
            return Ok(!holds);
        }
        self.guard_probes += 1;
        let result = run_bound(
            &compiled.plan,
            &mut self.machine,
            before,
            None,
            fuel,
            Some(guard),
        );
        check(&self.control, control)?;
        let quiet = !self.machine.heap.poisoned()
            && self.machine.warnings.is_empty()
            && self.machine.printed.is_empty();
        if let (Some(key), Ok(holds)) = (key, &result)
            && quiet
        {
            key.put(&mut self.projected, *holds);
        }
        let proved = matches!(result, Ok(false)) && quiet;
        if proved {
            compiled.exhausted_in_a_row = 0;
        }
        Ok(proved)
    }

    /// A quiet FALSE from this prefix depends on the source, the bound atom
    /// and one destination variable. Probe its value from this source row.
    /// Matching actual destinations can then inherit FALSE without evaluating
    /// any later conjunct or assuming the entire action is deterministic.
    fn unchanged_target_guard(
        &mut self,
        atom: &PredicateAtom,
        source: usize,
        before: &[Handle],
        control: &EvaluationControl,
    ) -> Result<Option<UnchangedTarget>, AnalysisError> {
        let Some(index) = self.position(atom) else {
            return Ok(None);
        };
        let compiled = &self.compiled[index];
        let Some((guard, variable)) = compiled.unchanged_guard else {
            return Ok(None);
        };
        let Some(fuel) = self.fuel.checked_sub(3) else {
            return Ok(None);
        };
        if self.machine.heap.poisoned() || compiled.exhausted_in_a_row >= FUEL_STRIKES {
            return Ok(None);
        }
        self.unchanged_probes += 1;
        let result = run_bound(
            &compiled.plan,
            &mut self.machine,
            before,
            Some(before),
            fuel,
            Some(guard),
        );
        check(&self.control, control)?;
        if matches!(result, Ok(false))
            && !self.machine.heap.poisoned()
            && self.machine.warnings.is_empty()
            && self.machine.printed.is_empty()
            && let Some(&value) = before.get(variable)
        {
            self.unchanged_rows += 1;
            return Ok(Some(UnchangedTarget {
                atom: index,
                source,
                variable,
                value,
            }));
        }
        Ok(None)
    }

    fn validate_reuse(
        &mut self,
        atom: &PredicateAtom,
        control: &EvaluationControl,
        inputs_closed: impl FnOnce(&dive_core::Heap) -> Result<bool, AnalysisError>,
    ) -> Result<bool, AnalysisError> {
        check(&self.control, control)?;
        if self.position(atom).is_none() {
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
) -> Result<(dive_core::plan::Plan, dive_core::compile::BoundGuards), Rejection> {
    let Ok(mut handoff) = crate::core_engine::handoff(model, &model.ir, Vec::new()) else {
        return Err(Rejection::CompileError);
    };
    // `~ENABLED A` compiles the search of its pure action `A`; the caller
    // negates the truth.
    let expr = match atom.test {
        AtomTest::NegatedEnabled { action } => action,
        _ => atom.pred.expr,
    };
    let Ok(role) = crate::core_engine::role(
        model,
        "native temporal predicate".into(),
        expr,
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
    // ASSUMEs left to Core; an atom reaches none of those definitions
    // (the binder evaluated what temporal properties reach), so its compile
    // evaluates no start-up values, and would compile any body in place.
    handoff.deferred.clear();
    handoff.startup_evaluator = None;
    handoff.assumptions.clear();
    let test = match atom.test {
        AtomTest::Predicate => dive_core::compile::BoundTest::Predicate,
        AtomTest::Enabled { sub } => dive_core::compile::BoundTest::Enabled(sub),
        AtomTest::NegatedEnabled { .. } => dive_core::compile::BoundTest::Enabled(None),
        AtomTest::Nonstuttering { sub } => dive_core::compile::BoundTest::Nonstuttering(sub),
    };
    let (plan, false_guard) =
        dive_core::compile::compile_bound_test_with_guards(&handoff, Arc::clone(store), opts, test)
            .map_err(|_| Rejection::CompileError)?;
    admit_plan(&plan, heap)?;
    Ok((plan, false_guard))
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

/// Runs one source-scoped bound test, or its certified necessary prefix,
/// in a fresh role frame. The caller certifies materialization of the
/// exact input rows and the prepared plan's preset values.
fn run_bound(
    plan: &dive_core::plan::Plan,
    machine: &mut Machine,
    before: &[Handle],
    after: Option<&[Handle]>,
    fuel: u64,
    guard: Option<dive_core::plan::P>,
) -> Result<bool, dive_core::Error> {
    machine.s0.copy_from_slice(before);
    machine.s1.fill(Handle::EMPTY);
    if let Some(after) = after {
        machine.s1.copy_from_slice(after);
    }
    // `inputs_closed` just proved both rows materialized, and admission
    // proved the bound scope parameters (`admit_plan`): the machine's
    // identity tests need not prove those exact handles again.
    machine.set_proven_materialized(
        before,
        after.unwrap_or(&[]),
        plan.bound_preset.iter().map(|&(_, value)| value),
    );
    machine.set_fuel(fuel);
    let predicate = &plan.implied_actions[0];
    let frame = machine.enter(predicate.frame);
    for &(slot, value) in &plan.bound_preset {
        if let Some(cell) = machine.frame.get_mut(machine.base + slot as usize) {
            *cell = value;
        }
    }
    machine.warnings.clear();
    machine.printed.clear();
    let result = if let Some(guard) = guard {
        machine.eval_bool(&plan.code, guard)
    } else {
        predicate.conjuncts.iter().try_fold(true, |holds, part| {
            if holds {
                machine.eval_bool(&plan.code, part.p)
            } else {
                Ok(false)
            }
        })
    };
    machine.leave(frame);
    machine.heap.reset();
    result
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
/// representation. Symbolic, lazy, unnormalized, transient and
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
// admissible_row(), independently for each actual row demand.
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
    allow_foreign_scalars: bool,
    metrics: RowProofMetrics,
}

impl<'a> RowProofs<'a> {
    fn new(graph: &'a NativeGraph) -> Self {
        Self {
            graph,
            bits: Vec::new(),
            initialized: false,
            allow_foreign_scalars: false,
            metrics: RowProofMetrics::default(),
        }
    }

    fn closed(&mut self, heap: &dive_core::Heap, row: usize) -> Result<bool, AnalysisError> {
        let roots = self.graph.row(row)?;
        // Persistent coordinates do not identify their issuing Store. Current
        // poison is deliberately not cached, including after an earlier success.
        if heap.poisoned() {
            return Ok(false);
        }
        if !self.graph.owns_store(heap.store()) {
            // Only immediates carry their entire value across freshly bound stores.
            return Ok(self.allow_foreign_scalars
                && roots
                    .iter()
                    .all(|h| h.as_int().is_some() || h.as_bool().is_some()));
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

/// The reader owns a fixed graph; its compiled atom identity and physical
/// source row fix the entire unprimed input and lexical environment.
#[derive(Clone, Copy)]
struct UnchangedTarget {
    atom: usize,
    source: usize,
    variable: usize,
    value: Handle,
}

pub(crate) struct Reader<'a, 'model> {
    prefix: Option<bool>,
    prefix_validated: usize,
    emissions: emissions::Rows,
    emission_capture: Option<dive_runtime::capture::CsrCapture<'a>>,
    proofs: RowProofs<'a>,
    pub(crate) values: &'a mut NativeValues<'model>,
    atoms: &'a mut NativeAtoms,
    unchanged: Option<UnchangedTarget>,
}

impl<'a, 'model> Reader<'a, 'model> {
    pub(crate) fn new(
        graph: &'a NativeGraph,
        values: &'a mut NativeValues<'model>,
        atoms: &'a mut NativeAtoms,
    ) -> Self {
        Self {
            prefix: None,
            prefix_validated: 0,
            emissions: emissions::Rows::default(),
            emission_capture: None,
            proofs: RowProofs::new(graph),
            unchanged: None,
            values,
            atoms,
        }
    }
}

impl<'a> Reader<'a, '_> {
    /// Only the live explorer can pair this reader with its own checked capture.
    /// Bind the source masks for this reader's lifetime, including when another
    /// view shares its offsets. Edit reuse and simulation do not opt in.
    pub(crate) fn live_emission_capture(&mut self, csr: &dive_runtime::capture::CsrCapture<'a>) {
        self.emission_capture.get_or_insert(*csr);
    }
}

impl Reader<'_, '_> {
    /// Retain the ordinary owner check for every store-relative handle.
    pub(crate) fn allow_inline_rows(&mut self) {
        self.proofs.allow_foreign_scalars = true;
    }
}

impl Drop for Reader<'_, '_> {
    fn drop(&mut self) {
        self.atoms.emission_peak_bytes = self.atoms.emission_peak_bytes.max(self.emissions.bytes());
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
        let source = self.values.read_retained(graph, before, control)?;
        let st = match after {
            Some(row) => {
                dive_eval::St::action(source, self.values.read_retained(graph, row, control)?)
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
        // names in event order, before the producer retained its row. Under
        // deferred admission every atom is native, so reads stay limited to
        // actual fallbacks; each read first admits the pending rows in
        // retention order, and an unconvertible row is refused at that read.
        true
    }

    fn has_predicate(&self, atom: &PredicateAtom) -> bool {
        self.atoms.position(atom).is_some()
    }

    fn read(
        &mut self,
        row: usize,
        control: &EvaluationControl,
    ) -> Result<dive_eval::next::StateVals, AnalysisError> {
        self.values.read_retained(self.proofs.graph, row, control)
    }
    fn predicate(
        &mut self,
        atom: &PredicateAtom,
        before: usize,
        after: Option<usize>,
        control: &EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        let graph = self.proofs.graph;
        let native = if let Some(proof) = self.unchanged
            && proof.source == before
            && let Some(compiled) = self.atoms.compiled.get(proof.atom)
            && std::ptr::eq(compiled.identity, std::ptr::from_ref(atom))
            && compiled.exhausted_in_a_row < FUEL_STRIKES
            && let Some(after) = after
            && graph.row(after)?.get(proof.variable) == Some(&proof.value)
            && self.proofs.closed(&self.atoms.machine.heap, before)?
            && self.proofs.closed(&self.atoms.machine.heap, after)?
        {
            // Audit still independently evaluates each certified edge below.
            check(&self.atoms.control, control)?;
            self.atoms.compiled[proof.atom].exhausted_in_a_row = 0;
            self.atoms.unchanged_edges += 1;
            Some(false)
        } else {
            self.atoms.evaluate_with(
                atom,
                graph.row(before)?,
                after.map(|row| graph.row(row)).transpose()?,
                control,
                graph.owns_store(self.atoms.machine.heap.store()),
                |heap| {
                    if !self.proofs.closed(heap, before)? {
                        return Ok(false);
                    }
                    after.map_or(Ok(true), |row| self.proofs.closed(heap, row))
                },
            )?
        };
        if self.atoms.audit
            && let Some(value) = native
        {
            self.audit(atom, before, after, value, control)?;
        }
        Ok(native)
    }

    fn action_row(
        &mut self,
        atom: &PredicateAtom,
        graph: &dive_runtime::capture::CsrCapture<'_>,
        source: usize,
        control: &EvaluationControl,
    ) -> Result<Option<u64>, AnalysisError> {
        let Some(index) = self.atoms.position(atom) else {
            return Ok(None);
        };
        let compiled = &self.atoms.compiled[index];
        let Some(batch) = &compiled.batch else {
            return Ok(None);
        };
        let retained = self.proofs.graph;
        if !std::ptr::eq(graph.offsets(), retained.input().offsets)
            || self.atoms.fuel != NATIVE_FUEL
            || compiled.exhausted_in_a_row >= FUEL_STRIKES
            || self.proofs.metrics.certified != retained.input().retained_rows as u64
            || !retained.owns_store(self.atoms.machine.heap.store())
            || self.atoms.machine.heap.poisoned()
        {
            return Ok(None);
        }
        let range = graph
            .outgoing_range(source)
            .ok_or(AnalysisError::Inconsistent("native action batch row"))?;
        let len = range.len();
        if !(2..=16).contains(&len) {
            return Ok(None);
        }
        let before = graph
            .representative_row(source)
            .ok_or(AnalysisError::Inconsistent("native action batch source"))?;
        let source_row = retained.row(before)?;
        let producer = compiled.producer.and_then(|id| {
            let cert = self.atoms.producer_images.get(id).copied().flatten()?;
            (self.atoms.live_emissions
                && self.prefix.is_some()
                && self
                    .emission_capture
                    .as_ref()
                    .is_some_and(|original| original.same_validation(graph))
                && source < graph.closed_rows()
                && compiled.exhausted_in_a_row == 0
                && ((cert.quiet_suffix && cert.quiet_guards)
                    || !self.atoms.machine.heap.store().saw_missing_update()))
            .then_some((id, cert.boolean_guard_prefix))
        });
        let mut emission = None;
        let mut rows = [source_row; 16];
        let mut ids = [before; 16];
        for pos in 0..len - 1 {
            let Some(original) = graph.original_row(source, pos) else {
                return Ok(None);
            };
            let to = graph
                .target(range.start + pos)
                .ok_or(AnalysisError::Inconsistent("native action batch target"))?;
            if graph.representative_row(to) != Some(original) {
                return Ok(None);
            }
            rows[pos] = retained.row(original)?;
            if let Some((id, quiet)) = producer
                && emission.is_none()
                && graph.original_action(source, pos) == u32::try_from(id).ok()
            {
                emission = Some((rows[pos], quiet));
            }
            ids[pos] = original;
        }
        check(&self.atoms.control, control)?;
        let bits = batch.evaluate_image(
            &compiled.plan,
            &mut self.atoms.machine,
            source_row,
            &rows[..len],
            self.atoms.fuel,
            emission,
        );
        check(&self.atoms.control, control)?;
        if let Some((bits, consumed)) = bits {
            self.atoms.batch_rows += 1;
            self.atoms.image_hits += u64::from(consumed);
            if self.atoms.audit {
                for (pos, &after) in ids[..len].iter().enumerate() {
                    self.audit(atom, before, Some(after), bits & (1 << pos) != 0, control)?;
                }
            }
        }
        Ok(bits.map(|(truth, _)| truth))
    }

    fn closed_state_predicate(
        &mut self,
        atom: &PredicateAtom,
        graph: &dive_runtime::capture::CsrCapture<'_>,
        source: usize,
        before: usize,
        control: &EvaluationControl,
    ) -> Result<Option<bool>, AnalysisError> {
        self.emitted_truth(atom, graph, source, before, control)
    }

    fn action_row_false(
        &mut self,
        atom: &PredicateAtom,
        before: usize,
        rows: usize,
        control: &EvaluationControl,
    ) -> Result<bool, AnalysisError> {
        // Each certified bit refers to a physical row in this graph and
        // its owning Store. The checked graph may describe a prefix.
        self.unchanged = None;
        let retained = self.proofs.graph.input().retained_rows;
        if self.proofs.metrics.certified != retained as u64
            || rows > retained
            || self.atoms.machine.heap.poisoned()
            || !self
                .proofs
                .graph
                .owns_store(self.atoms.machine.heap.store())
        {
            return Ok(false);
        }
        let answer = self
            .atoms
            .false_guard(atom, self.proofs.graph.row(before)?, control)?;
        self.atoms.guard_rows += u64::from(answer);
        if !answer
            && self
                .atoms
                .position(atom)
                .is_none_or(|index| self.atoms.compiled[index].batch.is_none())
        {
            self.unchanged = self.atoms.unchanged_target_guard(
                atom,
                before,
                self.proofs.graph.row(before)?,
                control,
            )?;
        }
        Ok(answer)
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

    fn label_prefix(
        &mut self,
        atom: &PredicateAtom,
        graph: &dive_runtime::capture::CsrCapture<'_>,
        control: &EvaluationControl,
    ) -> Result<Option<dive_liveness::analyze::periodic::external::LabelPrefix<'_>>, AnalysisError>
    {
        self.previous_column(atom, graph, control)
    }

    fn save_label_prefix(
        &mut self,
        atom: &PredicateAtom,
        graph: &dive_runtime::capture::CsrCapture<'_>,
        words: &mut dyn ExactSizeIterator<Item = u64>,
        cap: usize,
        control: &EvaluationControl,
    ) -> Result<(), AnalysisError> {
        self.store_column(atom, graph, words, cap, control)
    }

    fn reference_predicate(&mut self, atom: &PredicateAtom) {
        match atom.kind {
            dive_liveness::formula::AtomKind::State => self.atoms.reference.state += 1,
            dive_liveness::formula::AtomKind::Action => self.atoms.reference.action += 1,
        }
    }

    fn analysis_time(
        &mut self,
        stage: dive_liveness::analyze::Stage,
        elapsed: std::time::Duration,
    ) {
        *self.atoms.analysis_ms.entry(stage.name()).or_default() += elapsed.as_secs_f64() * 1000.0;
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
    /// When native evaluation produced a truth, the dive-eval evaluator must
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
    fn enabled_projects_copied_variables_only_after_checking_its_actual_source() {
        let source = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLES gate, payload
vars == <<gate,payload>>
Init == gate = 0 /\ payload = 0
Next == gate' \in 0..2 /\ payload' \in 0..3
Act == /\ 2 \div (2-gate)=1
       /\ gate'=1 /\ UNCHANGED payload
Spec == Init /\ [][Next]_vars /\ WF_vars(Act)
P == <>(gate=1)
====";
        with_wrappers(source, false, |prepared, driver, atoms, control| {
            let atom = driver
                .native_predicates()
                .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                .expect("bound enabledness");
            let opts = dive_runtime::CheckOptions::dive(0);
            let (graph, mut values) = capture(prepared, &opts);
            let mut reader = Reader::new(&graph, &mut values, atoms);
            for row in 0..graph.input().retained_rows {
                let gate = graph.row(row).expect("row")[0].as_int().expect("gate");
                let expected = match gate {
                    0 => Some(true),
                    1 => Some(false),
                    _ => None,
                };
                assert_eq!(
                    reader.predicate(atom, row, None, control).expect("query"),
                    expected
                );
            }
            assert_eq!(reader.atoms.projected_enabled_hits, 6);
            assert_eq!(reader.atoms.fallbacks.error, 4);
            reader.atoms.fuel = 0;
            assert_eq!(
                reader.predicate(atom, 0, None, control).expect("fuel"),
                None
            );
            assert_eq!(reader.atoms.projected_enabled_hits, 6);
        });
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

    fn committed_frontier<'a>(
        action_labels: &'a [String],
        limits: dive_runtime::capture::Limits,
        control: &EvaluationControl,
    ) -> dive_runtime::capture::CsrCapture<'a> {
        dive_runtime::capture::CsrCapture::checked(
            dive_runtime::capture::CsrCaptureInput {
                roots: &[0],
                offsets: &[0, 2, 2],
                targets: &[1, 0],
                ordinals: &[Some(0), None][..],
                actions: &[Some(0), None][..],
                edge_rows: &[1, 0],
                representatives: &[0, 1],
                action_labels,
                retained_rows: 2,
                closed_rows: 1,
            },
            limits,
            control,
        )
        .expect("committed frontier")
    }

    const OVERLAPPING_ACTIONS: &str = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLES x, r
vars == <<x,r>>
Init == x=0 /\ r = <<0,0>>
A(i) == /\ x=i
        /\ x'=1-i
        /\ r'=[r EXCEPT ![1]=@]
Stut == UNCHANGED <<x,r>>
Other == /\ x'=1-x /\ UNCHANGED r
Next == (\E i \in 0..1: A(i)) \/ Other \/ Stut
Spec == Init /\ [][Next]_vars /\ (\A i \in 0..1: WF_vars(A(i))) /\ WF_vars(Stut)
P == <>(x=1)
====";

    #[test]
    fn producer_images_require_the_original_capture_on_overlapping_bound_actions() {
        use dive_runtime::capture::CsrCapture;
        let src = OVERLAPPING_ACTIONS;
        with_wrappers(src, false, |prepared, driver, atoms, control| {
            let options = dive_runtime::CheckOptions::dive(0);
            atoms.enable_live_emissions(&prepared.model, &options);
            let (graph, mut values) = capture(prepared, &options);
            let csr =
                CsrCapture::checked_targets(graph.input(), graph.limits(), control).expect("csr");
            // A caller can construct another checked view over the same
            // offsets with every generated action tag missing. Only the
            // original producer view may supply semantic evidence.
            let unknown_actions = vec![None; *csr.offsets().last().expect("edge count")];
            let replaced = CsrCapture::checked_targets(
                dive_runtime::capture::CsrTargetStorage {
                    actions: dive_runtime::capture::CsrActions::Expanded(&unknown_actions),
                    ..graph.input()
                },
                graph.limits(),
                control,
            )
            .expect("another shape-valid view");
            let mut reader = Reader::new(&graph, &mut values, atoms);
            reader.live_prefix(true);
            reader.live_emission_capture(&csr);
            reader.live_emission_capture(&replaced);
            reader.atoms.audit = true;
            // Certify every physical input. Original producer tags may shortcut
            // writes, including transitions shared with Other. Replacing the
            // capture token must give the same truth without consuming an image.
            for row in 0..graph.input().retained_rows {
                assert!(
                    reader
                        .proofs
                        .closed(&reader.atoms.machine.heap, row)
                        .expect("closed")
                );
            }
            for action in driver
                .native_predicates()
                .filter(|a| matches!(a.test, AtomTest::Nonstuttering { .. }))
            {
                for source in 0..csr.closed_rows() {
                    let bits = reader
                        .action_row(action, &csr, source, control)
                        .expect("original");
                    let hits = reader.atoms.image_hits;
                    assert_eq!(
                        reader
                            .action_row(action, &replaced, source, control)
                            .expect("replacement"),
                        bits
                    );
                    assert_eq!(reader.atoms.image_hits, hits);
                }
            }
            assert!(reader.atoms.image_hits > 0);
            assert!(reader.atoms.audit_mismatch().is_none());
        });
    }

    #[test]
    fn closed_emissions_bind_arguments_and_skip_only_quiet_complete_originals() {
        use dive_runtime::capture::CsrCapture;

        let src = OVERLAPPING_ACTIONS;
        with_wrappers(src, false, |prepared, driver, atoms, control| {
            let options = dive_runtime::CheckOptions::dive(0);
            atoms.enable_live_emissions(&prepared.model, &options);
            let (graph, mut values) = capture(prepared, &options);
            let csr =
                CsrCapture::checked_targets(graph.input(), graph.limits(), control).expect("csr");
            // A caller can construct another checked view over the same
            // offsets with every generated action tag missing. Only the
            // original producer view may supply semantic evidence.
            let unknown_actions = vec![None; *csr.offsets().last().expect("edge count")];
            let replaced = CsrCapture::checked_targets(
                dive_runtime::capture::CsrTargetStorage {
                    actions: dive_runtime::capture::CsrActions::Expanded(&unknown_actions),
                    ..graph.input()
                },
                graph.limits(),
                control,
            )
            .expect("another shape-valid view");
            let mut reader = Reader::new(&graph, &mut values, atoms);
            reader.live_prefix(true);
            reader.live_emission_capture(&csr);
            reader.live_emission_capture(&replaced);
            reader.atoms.audit = true;
            let mut answers = Vec::new();
            for atom in driver
                .native_predicates()
                .filter(|a| matches!(a.test, AtomTest::Enabled { .. }))
            {
                let mut row = Vec::new();
                for source in 0..csr.state_count() {
                    let before = csr.representative_row(source).expect("rep");
                    assert_eq!(
                        reader
                            .closed_state_predicate(atom, &replaced, source, before, control)
                            .expect("substituted capture"),
                        None
                    );
                    row.push(
                        reader
                            .closed_state_predicate(atom, &csr, source, before, control)
                            .expect("audit"),
                    );
                }
                answers.push(row);
                assert_eq!(
                    reader
                        .closed_state_predicate(atom, &csr, csr.closed_rows(), 0, control)
                        .expect("open"),
                    None
                );
            }
            answers.sort();
            assert_eq!(
                answers,
                [
                    vec![Some(false), Some(false)],
                    vec![Some(false), Some(true)],
                    vec![Some(true), Some(false)]
                ]
            );
            assert_eq!(reader.atoms.emission_hits, [2, 4]);
            drop(reader);
            // A checked committed prefix retains a real representative with an
            // empty frontier row. That empty row is no evidence of disabledness.
            let partial = committed_frontier(graph.input().action_labels, graph.limits(), control);
            let mut frontier = Reader::new(&graph, &mut values, atoms);
            frontier.live_prefix(true);
            frontier.live_emission_capture(&partial);
            let atom = driver
                .native_predicates()
                .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                .expect("fairness");
            assert_eq!(
                frontier
                    .closed_state_predicate(atom, &partial, 1, 1, control)
                    .expect("frontier"),
                None
            );
            drop(frontier);
            let mut read_missing = Reader::new(&graph, &mut values, atoms);
            read_missing.live_prefix(true);
            read_missing.live_emission_capture(&replaced);
            assert_eq!(
                read_missing
                    .closed_state_predicate(atom, &replaced, 0, 0, control)
                    .expect("missing tags"),
                None
            );
        });
    }

    #[test]
    fn missing_update_from_an_earlier_action_vetoes_later_emission_reuse() {
        use dive_runtime::capture::CsrCapture;
        // Repeated callers share a memoizable noisy source computation. Its
        // first miss survives every frame/reset and all duplicate emissions.
        let src = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLES x,r
vars == <<x,r>>
Init == x=0 /\ r = <<0,0>>
Noisy == [r EXCEPT ![3]=0]
A == /\ x'=1-x /\ r'=Noisy
Other == /\ x'=x /\ r'=Noisy
Conflict == /\ x'=1-x /\ UNCHANGED x /\ UNCHANGED r
Next == A \/ Other \/ Conflict
Spec == Init /\ [][Next]_vars /\ WF_vars(A) /\ WF_vars(Conflict)
P == <>(x=1)
====";
        with_wrappers(src, false, |prepared, driver, atoms, control| {
            let options = dive_runtime::CheckOptions::dive(0);
            atoms.enable_live_emissions(&prepared.model, &options);
            let (graph, mut values) = capture(prepared, &options);
            assert!(prepared.store.saw_missing_update());
            let csr =
                CsrCapture::checked_targets(graph.input(), graph.limits(), control).expect("csr");
            let mut reader = Reader::new(&graph, &mut values, atoms);
            reader.live_prefix(true);
            reader.live_emission_capture(&csr);
            for atom in driver
                .native_predicates()
                .filter(|a| matches!(a.test, AtomTest::Enabled { .. }))
            {
                assert_eq!(
                    reader
                        .closed_state_predicate(atom, &csr, 0, 0, control)
                        .expect("decline"),
                    None
                );
            }
            assert_eq!(reader.atoms.emission_hits, [0, 0]);
        });
    }

    #[test]
    fn only_sequential_value_guards_certify_boolean_action_demand() {
        let src = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLES x,y
vars == <<x,y>>
Init == x=0 /\ y=0
Plain == /\ x=0 /\ y/=1 /\ x'=1 /\ UNCHANGED y
Quantified == /\ (\E k \in {x}: k=0) /\ x'=1 /\ UNCHANGED y
Next == Plain \/ Quantified
Spec == Init /\ [][Next]_vars /\ WF_vars(Plain) /\ WF_vars(Quantified)
P == <> (x=1)
====";
        with_wrappers(src, false, |prepared, driver, atoms, _| {
            let mut found = Vec::new();
            for atom in driver
                .native_predicates()
                .filter(|a| matches!(a.test, AtomTest::Enabled { .. }))
            {
                let compiled = &atoms.compiled[atoms.position(atom).expect("prepared")];
                let (index, cert) = compiled.emission.expect("both enabled generators admitted");
                let label = prepared.model.actions[index].label();
                found.push((label.starts_with("<Plain "), cert.boolean_guard_prefix));
            }
            found.sort_unstable();
            assert_eq!(found, [(false, false), (true, true)]);
        });
    }

    #[test]
    fn compile_time_update_miss_and_empty_assignment_domain_preserve_fallback() {
        use dive_runtime::capture::CsrCapture;
        let src = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLES x,r
vars == <<x,r>>
Init == x=0 /\ r = <<0,0>>
Flip == /\ x'=1-x /\ r'=IF x=0 THEN [<<0,0>> EXCEPT ![3]=1] ELSE r
Empty == /\ x=0 /\ x' \in {} /\ UNCHANGED r
BadGuard == /\ [r EXCEPT ![3]=0] /= r /\ UNCHANGED <<x,r>>
Next == Flip \/ Empty \/ BadGuard
Spec == Init /\ [][Next]_vars /\ WF_vars(Flip) /\ WF_vars(Empty) /\ WF_vars(BadGuard)
P == <> (x=1)
====";
        with_wrappers(src, false, |prepared, driver, atoms, control| {
            assert!(
                prepared.store.saw_missing_update(),
                "the literal EXCEPT was folded before exploration"
            );
            let options = dive_runtime::CheckOptions::dive(0);
            atoms.enable_live_emissions(&prepared.model, &options);
            let (graph, mut values) = capture(prepared, &options);
            let csr =
                CsrCapture::checked_targets(graph.input(), graph.limits(), control).expect("csr");
            let mut reader = Reader::new(&graph, &mut values, atoms);
            reader.live_prefix(true);
            reader.live_emission_capture(&csr);
            for atom in driver
                .native_predicates()
                .filter(|a| matches!(a.test, AtomTest::Enabled { .. }))
            {
                assert_eq!(
                    reader
                        .closed_state_predicate(atom, &csr, 0, 0, control)
                        .expect("decline"),
                    None
                );
                let index =
                    bound_next::action_index(&prepared.model, atom.pred.expr, &atom.pred.scope)
                        .expect("original action");
                let branch = &prepared.model.actions[index];
                if branch.label().starts_with("<BadGuard ") {
                    let source = reader
                        .values
                        .read_retained(&graph, 0, control)
                        .expect("source");
                    let printed = dive_eval::capture::PrintCapture::start();
                    assert!(
                        !atom
                            .evaluate(&prepared.model, &dive_eval::St::state(source))
                            .expect("reference disabled")
                    );
                    assert!(
                        printed
                            .take()
                            .iter()
                            .any(|line| line.warning && line.text.contains("2144"))
                    );
                }
            }
        });
    }

    #[test]
    fn periodic_column_requires_its_capture_even_when_the_store_and_shape_match() {
        use dive_runtime::capture::CsrCapture;

        let source = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Next == x'=1-x
Spec == Init /\ [][Next]_x
P == <>(x=1)
====";
        with_wrappers(source, false, |prepared, driver, atoms, control| {
            let options = dive_runtime::CheckOptions::dive(0);
            let (graph, mut values) = capture(prepared, &options);
            let atom = driver
                .native_predicates()
                .find(|atom| matches!(atom.test, AtomTest::Predicate))
                .expect("state predicate");
            let csr = CsrCapture::checked_targets(graph.input(), graph.limits(), control)
                .expect("capture");
            let mut reader = Reader::new(&graph, &mut values, atoms);
            reader.live_prefix(true);
            assert_eq!(
                reader.predicate(atom, 0, None, control).expect("zero"),
                Some(false)
            );
            assert_eq!(
                reader.predicate(atom, 1, None, control).expect("one"),
                Some(true)
            );
            reader
                .save_label_prefix(atom, &csr, &mut [0b10].into_iter(), 8, control)
                .expect("save");
            drop(reader);

            let mut reader = Reader::new(&graph, &mut values, atoms);
            reader.live_prefix(false);
            let prefix = reader
                .label_prefix(atom, &csr, control)
                .expect("same graph")
                .expect("saved truths");
            assert_eq!(
                (prefix.sources, prefix.cells, prefix.words),
                (2, 2, &[0b10][..])
            );
            reader.atoms.fuel = NATIVE_FUEL - 1;
            assert!(
                reader
                    .label_prefix(atom, &csr, control)
                    .expect("limited fuel")
                    .is_none()
            );
            reader.atoms.fuel = NATIVE_FUEL;
            reader.atoms.audit = true;
            assert!(
                reader
                    .label_prefix(atom, &csr, control)
                    .expect("audit")
                    .is_none()
            );
            reader.atoms.audit = false;
            let (replacement, mut other_values) = capture(prepared, &options);
            let other =
                CsrCapture::checked_targets(replacement.input(), replacement.limits(), control)
                    .expect("second capture");
            assert_eq!(csr.offsets(), other.offsets());
            assert!(
                reader
                    .label_prefix(atom, &other, control)
                    .expect("foreign CSR")
                    .is_none()
            );
            drop(reader);

            let mut reader = Reader::new(&replacement, &mut other_values, atoms);
            reader.live_prefix(true);
            assert!(
                reader
                    .label_prefix(atom, &other, control)
                    .expect("new owner")
                    .is_none()
            );
            assert_eq!(reader.atoms.prefix_bytes, 0);
        });
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
    fn nested_variable_subscripts_compare_variables_directly_with_reference_truth() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLES x,y,z\nInner == <<y,z>>\nvars == <<x,Inner>>\nInit == x=0 /\\ y={} /\\ z=0\nNext == UNCHANGED vars\nA == x'=x+1 /\\ UNCHANGED <<y,z>>\nSpec == Init /\\ [][Next]_vars /\\ WF_vars(A)\nP == <> (x=1)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                let wrappers: Vec<_> = driver
                    .native_predicates()
                    .filter(|a| a.test != AtomTest::Predicate)
                    .collect();
                assert_eq!(wrappers.len(), 2);
                for atom in &wrappers {
                    let compiled = atoms
                        .compiled
                        .iter()
                        .find(|item| std::ptr::eq(item.identity, *atom))
                        .expect("compiled wrapper");
                    assert!(
                        compiled
                            .plan
                            .code
                            .nodes
                            .iter()
                            .any(|node| matches!(node, dive_core::plan::Node::SameVars { leaves, .. } if leaves.len() == 3)),
                        "{:?}",
                        atom.test
                    );
                }
                let nonstuttering = wrappers
                    .iter()
                    .find(|a| matches!(a.test, AtomTest::Nonstuttering { .. }))
                    .expect("nonstuttering");
                let enabled = wrappers
                    .iter()
                    .find(|a| matches!(a.test, AtomTest::Enabled { .. }))
                    .expect("enabled");
                let int = |i| dive_value::Value::Int(i);
                let ctx = prepared.model.evaluator.options().ctx;
                let set = |items: Vec<i64>| {
                    dive_value::Value::set(items.into_iter().map(int).collect(), ctx).expect("set")
                };
                let rows = [
                    vec![int(0), set(vec![]), int(0)],
                    vec![int(1), set(vec![]), int(0)],
                    vec![int(0), set(vec![1]), int(0)],
                    vec![int(0), set(vec![]), int(2)],
                    vec![int(1), set(vec![1, 2]), int(2)],
                ];
                let mut decided = 0;
                for before in &rows {
                    decided += usize::from(
                        agree(prepared, atoms, enabled, control, before, None).is_some(),
                    );
                    for after in &rows {
                        decided += usize::from(
                            agree(prepared, atoms, nonstuttering, control, before, Some(after))
                                .is_some(),
                        );
                    }
                }
                assert_eq!(decided, rows.len() * (rows.len() + 1), "every truth native");
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
            assert!(metrics["rejections"].get("negated_enabled").is_none());
            assert_eq!(metrics["eligible_atoms"], 1);
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

    #[test]
    fn action_batch_matches_each_original_with_overlapping_branches_and_partial_subscript() {
        use dive_runtime::capture::CsrCapture;
        let text = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLES x,y
vars == <<x,y>>
Init == x=0 /\ y=0
Act(k) == /\ x'=k /\ y'=1-y
Next == Act(0) \/ Act(1) \/ Act(1)
Spec == Init /\ [][Next]_vars /\ WF_<<x>>(Act(0)) /\ SF_<<x>>(Act(1))
P == <> (y=2)
====";
        with_wrappers(text, false, |prepared, driver, atoms, control| {
            let options = dive_runtime::CheckOptions::dive(0);
            let (graph, mut values) = capture(prepared, &options);
            let csr = CsrCapture::checked_targets(graph.input(), graph.limits(), control)
                .expect("valid captured row");
            let mut reader = Reader::new(&graph, &mut values, atoms);
            reader.atoms.audit = true;
            for row in 0..graph.input().retained_rows {
                assert!(
                    reader
                        .proofs
                        .closed(&reader.atoms.machine.heap, row)
                        .expect("closed")
                );
            }
            let mut seen = (0, 0);
            for atom in driver
                .native_predicates()
                .filter(|atom| matches!(atom.test, AtomTest::Nonstuttering { .. }))
            {
                for source in 0..csr.closed_rows() {
                    let bits = reader
                        .action_row(atom, &csr, source, control)
                        .expect("audited physical row")
                        .expect("batch admitted");
                    seen.0 += bits.count_ones();
                    seen.1 += usize::from(bits == 0);
                }
            }
            assert!(seen.0 > 0 && seen.1 > 0);
            assert!(reader.atoms.audit_mismatch().is_none());
        });
    }

    #[test]
    fn action_batch_declines_a_demanded_type_error_and_skips_an_unreachable_one() {
        let text = r#"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLES x,y
vars == <<x,y>>
Init == x=0 /\ y=0
Next == x'=1-x /\ y'=0
Act == /\ x'=1 /\ y'="bad"
Spec == Init /\ [][Next]_vars /\ WF_vars(Act)
P == <> (y=2)
===="#;
        with_wrappers(text, false, |_prepared, driver, atoms, _control| {
            let atom = driver
                .native_predicates()
                .find(|a| matches!(a.test, AtomTest::Nonstuttering { .. }))
                .expect("action");
            let index = atoms.position(atom).expect("compiled");
            let compiled = &atoms.compiled[index];
            let batch = compiled.batch.as_ref().expect("deterministic plan");
            let zero = Handle::int(0).expect("inline zero");
            let one = Handle::int(1).expect("inline one");
            let before = [zero, zero];
            let mismatch = [zero, one];
            let reaches_error = [one, zero];
            assert_eq!(
                batch.evaluate(
                    &compiled.plan,
                    &mut atoms.machine,
                    &before,
                    &[&before, &mismatch],
                    NATIVE_FUEL
                ),
                Some(0)
            );
            assert_eq!(
                batch.evaluate(
                    &compiled.plan,
                    &mut atoms.machine,
                    &before,
                    &[&mismatch, &reaches_error],
                    NATIVE_FUEL
                ),
                None
            );
        });
    }

    #[test]
    fn unchanged_target_prefix_keeps_source_identity_and_late_evaluation_errors() {
        let text = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLES x,y
vars == <<x,y>>
Init == x=0 /\ y=0
Next == x' \in 0..1 /\ y' \in 0..2
Act(k) == \E i \in {k} : /\ x'=1-x
                         /\ IF y'=2 THEN 1 \div 0=0 ELSE y'=i
Spec == Init /\ [][Next]_vars /\ WF_vars(Act(0)) /\ WF_vars(Act(1))
P == <>(x=2)
====";
        with_wrappers(text, false, |prepared, driver, atoms, control| {
            let (graph, mut values) = capture(prepared, &dive_runtime::CheckOptions::dive(0));
            let count = graph.input().retained_rows;
            let mut reader = Reader::new(&graph, &mut values, atoms);
            let actions: Vec<_> = driver
                .native_predicates()
                .filter(|a| matches!(a.test, AtomTest::Nonstuttering { .. }))
                .collect();
            assert_eq!(actions.len(), 2);
            reader.atoms.audit = true;
            assert!(
                !reader
                    .action_row_false(actions[0], 0, count, control)
                    .expect("uncertified rows")
            );
            assert!(reader.unchanged.is_none());
            for row in 0..count {
                assert!(
                    reader
                        .proofs
                        .closed(&reader.atoms.machine.heap, row)
                        .expect("closed row")
                );
            }
            let mut errors = 0;
            for (index, &atom) in actions.iter().enumerate() {
                for source in 0..count {
                    assert!(
                        !reader
                            .action_row_false(atom, source, count, control)
                            .expect("source proof")
                    );
                    let proof = reader.unchanged.expect("certified prefix");
                    reader.atoms.compiled[proof.atom].exhausted_in_a_row = FUEL_STRIKES - 1;
                    assert_eq!(
                        reader
                            .predicate(atom, source, Some(source), control)
                            .expect("quiet prefix success"),
                        Some(false)
                    );
                    assert_eq!(reader.atoms.compiled[proof.atom].exhausted_in_a_row, 0);
                    // The captured rows are fixed, but the other closure and source have no certificate.
                    let hits = reader.atoms.unchanged_edges;
                    reader
                        .predicate(actions[1 - index], source, Some(source), control)
                        .expect("other closure");
                    let other = (source + 1) % count;
                    reader
                        .predicate(atom, other, Some(other), control)
                        .expect("other source");
                    assert_eq!(reader.atoms.unchanged_edges, hits);
                    for target in 0..count {
                        let before = reader
                            .values
                            .read_retained(&graph, source, control)
                            .expect("source");
                        let after = reader
                            .values
                            .read_retained(&graph, target, control)
                            .expect("target");
                        let reference = atom
                            .evaluate(reader.values.model(), &dive_eval::St::action(before, after));
                        let native = reader
                            .predicate(atom, source, Some(target), control)
                            .expect("audited action");
                        if let Ok(value) = reference {
                            assert_eq!(native, Some(value));
                        } else {
                            assert!(native.is_none());
                            errors += 1;
                        }
                    }
                }
            }
            assert!(errors > 0);
            assert!(reader.atoms.unchanged_edges > 0);
            assert!(reader.atoms.audit_mismatch().is_none());
        });
    }

    #[test]
    fn nonstuttering_source_guard_preserves_quantified_truth_and_errors() {
        let text = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLE x
vars == <<x>>
Init == x=0
Next == x' \in 0..3
Act == \E i \in 1..2 : /\ (IF x=1 THEN 1 \div 0=0 ELSE x=3)
                          /\ i=1
                          /\ (x'=0 \/ (1 \div 0=0))
Spec == Init /\ [][Next]_vars /\ WF_vars(Act)
P == <>(x=0)
====";
        for tlc in [false, true] {
            with_wrappers(text, tlc, |prepared, driver, atoms, control| {
                let options = if tlc {
                    dive_runtime::CheckOptions::tlc(0, 0)
                } else {
                    dive_runtime::CheckOptions::dive(0)
                };
                let (graph, mut values) = capture(prepared, &options);
                let count = graph.input().retained_rows;
                let atom = driver
                    .native_predicates()
                    .find(|a| matches!(a.test, AtomTest::Nonstuttering { .. }))
                    .expect("fairness action");
                let mut reader = Reader::new(&graph, &mut values, atoms);
                assert!(
                    !reader
                        .action_row_false(atom, 0, count, control)
                        .expect("unproved rows")
                );
                for row in 0..count {
                    assert!(
                        reader
                            .proofs
                            .closed(&reader.atoms.machine.heap, row)
                            .expect("complete row")
                    );
                }
                assert!(
                    !reader
                        .action_row_false(atom, 0, count + 1, control)
                        .expect("out-of-graph target set")
                );
                reader.atoms.audit = true;
                assert!(
                    !reader
                        .action_row_false(atom, 0, count, control)
                        .expect("audit keeps individual edges")
                );
                reader.atoms.audit = false;
                let ordinary_fuel = reader.atoms.fuel;
                reader.atoms.fuel = 3;
                assert!(
                    !reader
                        .action_row_false(atom, 0, count, control)
                        .expect("guard must fit fuel")
                );
                reader.atoms.fuel = ordinary_fuel;
                let mut hits = 0;
                let mut errors = 0;
                for before in 0..count {
                    let false_row = reader
                        .action_row_false(atom, before, count, control)
                        .expect("optional row proof");
                    if false_row {
                        hits += 1;
                    }
                    for after in 0..count {
                        let b = reader
                            .values
                            .read_retained(&graph, before, control)
                            .expect("before");
                        let a = reader
                            .values
                            .read_retained(&graph, after, control)
                            .expect("after");
                        let reference =
                            atom.evaluate(&prepared.model, &dive_eval::St::action(b, a));
                        if false_row {
                            assert!(matches!(reference, Ok(false)));
                        }
                        if reference.is_err() {
                            errors += 1;
                            assert!(!false_row);
                        }
                    }
                }
                assert!(errors > 0);
                if tlc {
                    assert_eq!(hits, 0);
                } else {
                    assert!(hits > 0);
                }
            });
        }
    }

    #[test]
    fn successor_independent_action_keeps_a_failing_nonstuttering_subscript() {
        let source = r"---- MODULE Atoms ----
EXTENDS Naturals
VARIABLE x
vars == <<x>>
Init == x=0
Next == x' \in 0..2
Act == /\ x=2
       /\ x'=1
sub == 1 \div x
Spec == Init /\ [][Next]_vars /\ WF_sub(Act)
P == <>(x=0)
====";
        with_wrappers(source, false, |prepared, driver, atoms, control| {
            let (graph, mut values) = capture(prepared, &dive_runtime::CheckOptions::dive(0));
            let count = graph.input().retained_rows;
            let atom = driver
                .native_predicates()
                .find(|a| matches!(a.test, AtomTest::Nonstuttering { .. }))
                .expect("fairness action");
            let mut reader = Reader::new(&graph, &mut values, atoms);
            for row in 0..count {
                assert!(
                    reader
                        .proofs
                        .closed(&reader.atoms.machine.heap, row)
                        .expect("complete row")
                );
            }
            assert!(
                !reader
                    .action_row_false(atom, 0, count, control)
                    .expect("partial subscript")
            );
            let before = reader
                .values
                .read_retained(&graph, 0, control)
                .expect("zero source");
            let after = reader
                .values
                .read_retained(&graph, 1, control)
                .expect("successor");
            assert!(
                atom.evaluate(&prepared.model, &dive_eval::St::action(before, after))
                    .is_err()
            );
        });
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
    fn negated_enabled_negates_the_native_search() {
        // `A` is enabled exactly when x < 2; `B` reads a recursive operator.
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nR[n \\in Nat] == IF n = 0 THEN 0 ELSE R[n-1] + 1\nA == x < 2 /\\ x'=x+1\nB == R[x] = 1 /\\ x' = 0\nSpec == Init /\\ [][Next]_x\nP == <> (~ENABLED A) /\\ <>(~ENABLED B)\n====\n";
        for tlc in [false, true] {
            with_wrappers(source, tlc, |prepared, driver, atoms, control| {
                assert_eq!(atoms.eligible, atoms.total);
                let negated: Vec<_> = driver
                    .native_predicates()
                    .filter(|atom| matches!(atom.test, AtomTest::NegatedEnabled { .. }))
                    .collect();
                assert_eq!(negated.len(), 2);
                for (atom, expected) in negated
                    .iter()
                    .zip([[false, false, true], [true, false, true]])
                {
                    for (x, want) in (0..3).zip(expected) {
                        let got = agree(prepared, atoms, atom, control, &[int(x)], None);
                        assert_eq!(got, Some(want), "x={x}, tlc={tlc}");
                    }
                }
                let metrics = atoms.metrics();
                assert_eq!(metrics["negated_enabled_evaluations"], 6);
                assert_eq!(metrics["enabled_evaluations"], 0);
            });
        }
    }

    #[test]
    fn negated_enabled_errors_fall_back_to_the_reference() {
        let source = "---- MODULE Atoms ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nA == 1 \\div x = 1 /\\ x'=x\nSpec == Init /\\ [][Next]_x\nP == <> (~ENABLED A)\n====\n";
        with_wrappers(source, false, |prepared, driver, atoms, control| {
            let atom = driver
                .native_predicates()
                .find(|atom| matches!(atom.test, AtomTest::NegatedEnabled { .. }))
                .expect("negated");
            assert_eq!(agree(prepared, atoms, atom, control, &[int(0)], None), None);
            assert_eq!(atoms.metrics()["fallbacks"]["error"], 1);
            assert_eq!(
                agree(prepared, atoms, atom, control, &[int(1)], None),
                Some(false)
            );
        });
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
                // The first poll is the first charge after installation; the
                // second comes within `WORK_POLL_UNITS` charged units, or at
                // a precharge for larger work, so it is inside enumeration.
                if self.calls.fetch_add(1, Ordering::Relaxed) == 1 {
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
                        poll.calls.load(Ordering::Relaxed) >= 2,
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
