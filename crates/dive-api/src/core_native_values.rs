//! Controlled Graph admission and on-demand values for the compact producer.
//!
//! Persistent roots stay with `NativeGraph`. A bounded exact-handle cache reuses
//! successful closed materialized-value conversions during reads.
//!
//! Eager admission keeps the refusal and name encounter order of a full Graph
//! conversion without always building the reference values. It walks each
//! row in conversion order, and it skips the construction of a node only when
//! that construction cannot fail: a tuple or integer-domain function, a
//! function whose keys are all inline integers or all strings, and a set of
//! inline integers, booleans or strings. A string or model value is converted
//! (and so interned) at its first occurrence, in walk order; later occurrences
//! of the same name intern nothing. Every other node is converted exactly as
//! before, and a persistent node that converted or walked successfully is
//! remembered in a bounded lossy table, because a later conversion of the same
//! persistent handle would succeed and intern nothing new.
//!
//! Admission is deferred ([`NativeValues::defer_admission`]) when every
//! liveness atom was prepared natively, so only an actual reference
//! fallback, source-row check or report reads values. Every read first
//! admits, in retention order, the retained rows not yet admitted
//! ([`NativeValues::read_retained`]). A seen successor original that is not
//! retained repeats its representative's handles, so it interns nothing. A
//! seen root may differ from its representative without being retained, so
//! it is admitted at once, after the pending rows. The names interned before
//! any read are thus those of eager admission, in the same order. A row whose
//! conversion fails is refused at that read instead of stopping the capture;
//! a run that never reads it needs no reference value of it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dive_core::{Handle, Heap, Store};
use dive_eval::next::StateVals;
use dive_eval::{EvalError, HostControl};
use dive_liveness::analyze::AnalysisError;
use dive_model::Model;
use dive_runtime::Verdict;
use dive_runtime::budget::{Budget, EvaluationControl, Phase};
use dive_value::Value;

use crate::core_native_graph::NativeGraph;

const MAX_CACHE_BYTES: usize = 128 * 1024 * 1024;
const CACHE_WARMUP: u64 = 1024;
const CACHE_PROBES: usize = 16;
/// Bytes of the admission table of persistent handles proved convertible.
const MAX_MEMO_BYTES: usize = 8 * 1024 * 1024;
struct Entry {
    key: Handle,
    row: StateVals,
    bytes: usize,
}
#[derive(Default)]
struct ConversionCache {
    slots: Vec<Option<Entry>>,
    limit: usize,
    bytes: usize,
    entries: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    bypasses: u64,
    eligible: u64,
    uncovered: u64,
    insertions: u64,
    probe_exhaustions: u64,
    byte_rejections: u64,
    allocation_bypasses: u64,
    direct_lookups: u64,
    direct_hits: u64,
    row_index_fallbacks: u64,
}
impl ConversionCache {
    fn index(&self, key: Handle) -> usize {
        let mut bits = key.bits();
        bits = (bits ^ (bits >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        bits = (bits ^ (bits >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        usize::try_from((bits ^ (bits >> 31)) % self.slots.len() as u64).unwrap_or(0)
    }
    fn get(&mut self, key: Handle, row: Option<usize>) -> Option<&StateVals> {
        if !key.is_transient() && !self.slots.is_empty() {
            if let Some(row) = row.filter(|&row| row < self.slots.len()) {
                self.direct_lookups += 1;
                // The row ID selects storage only; exact Handle equality is
                // still required, including after replacement or a changed ID.
                if let Some(entry) = &self.slots[row]
                    && entry.key == key
                {
                    self.hits += 1;
                    self.direct_hits += 1;
                    return Some(&entry.row);
                }
                self.misses += 1;
                return None;
            }
            if row.is_some() {
                self.row_index_fallbacks += 1;
            }
            let base = self.index(key);
            for probe in 0..CACHE_PROBES.min(self.slots.len()) {
                let slot = &self.slots[(base + probe) % self.slots.len()];
                if let Some(entry) = slot {
                    if entry.key == key {
                        self.hits += 1;
                        return Some(&entry.row);
                    }
                } else {
                    self.misses += 1;
                    return None;
                }
            }
            self.probe_exhaustions += 1;
        }
        self.misses += 1;
        None
    }
    fn insert(
        &mut self,
        key: Handle,
        value: &Value,
        bytes: usize,
        row: Option<usize>,
        proceed: &mut dyn FnMut() -> bool,
    ) {
        if key.is_transient() || self.misses < CACHE_WARMUP || !proceed() {
            return;
        }
        if self.slots.is_empty() {
            let count = self.limit / 4 / std::mem::size_of::<Option<Entry>>();
            if count == 0 || self.slots.try_reserve_exact(count).is_err() {
                self.bypasses += 1;
                self.allocation_bypasses += 1;
                return;
            }
            let table_bytes = self
                .slots
                .capacity()
                .saturating_mul(std::mem::size_of::<Option<Entry>>());
            if table_bytes > self.limit / 4 {
                self.slots = Vec::new();
                self.bypasses += 1;
                self.allocation_bypasses += 1;
                return;
            }
            for at in 0..count {
                if at % 1024 == 0 && !proceed() {
                    self.slots = Vec::new();
                    return;
                }
                self.slots.push(None);
            }
            self.bytes = table_bytes;
        }
        let at = if let Some(row) = row.filter(|&row| row < self.slots.len()) {
            row
        } else {
            let base = self.index(key);
            let mut at = base;
            for probe in 0..CACHE_PROBES.min(self.slots.len()) {
                let candidate = (base + probe) % self.slots.len();
                if self.slots[candidate]
                    .as_ref()
                    .is_none_or(|entry| entry.key == key)
                {
                    at = candidate;
                    break;
                }
            }
            at
        };
        let previous = self.slots[at].as_ref().map_or(0, |entry| entry.bytes);
        let charged = bytes.checked_add(std::mem::size_of::<Option<Value>>());
        let total = charged.and_then(|bytes| {
            self.bytes
                .checked_sub(previous)
                .and_then(|rest| rest.checked_add(bytes))
                .filter(|&total| total <= self.limit)
                .map(|total| (bytes, total))
        });
        let Some((bytes, total)) = total else {
            self.bypasses += 1;
            self.byte_rejections += 1;
            return;
        };
        if !proceed() {
            return;
        }
        if self.slots[at].is_some() {
            self.evictions += 1;
        } else {
            self.entries += 1;
        }
        self.slots[at] = Some(Entry {
            key,
            row: [Some(value.clone())].into(),
            bytes,
        });
        self.bytes = total;
        self.insertions += 1;
    }
}

/// A run-local converter; it never owns the producer's row table or topology.
pub(crate) struct NativeValues<'a> {
    model: &'a Model,
    heap: Heap,
    capture: Arc<EvaluationControl>,
    admitted: u64,
    read: u64,
    converted: u64,
    conversion: Duration,
    cache: ConversionCache,
    /// Lossy table of persistent handles whose Graph conversion succeeded.
    memo: Vec<u64>,
    memo_slots: usize,
    memo_hits: u64,
    /// String and model-value ids already converted (so interned).
    names: Vec<u64>,
    skipped: u64,
    /// Admission deferred to reads (see the module docs).
    deferred: bool,
    deferred_rows: u64,
    /// Under deferred admission, retained rows below this index are admitted.
    admitted_through: usize,
    caught_up_rows: u64,
}

impl<'a> NativeValues<'a> {
    pub(crate) fn new(
        model: &'a Model,
        store: Arc<Store>,
        budget: &Budget,
    ) -> Result<Self, Verdict> {
        if !budget.is_unlimited() && !budget.is_armed() {
            return Err(Verdict::InternalError {
                message: "native graph values require the actual armed run budget".into(),
            });
        }
        let capture =
            EvaluationControl::new(budget, Phase::Capture).map_err(|_| Verdict::InternalError {
                message: "native graph conversion control identity is unrepresentable".into(),
            })?;
        Ok(Self {
            model,
            heap: Heap::new(store, false, false),
            capture,
            admitted: 0,
            read: 0,
            converted: 0,
            conversion: Duration::ZERO,
            cache: ConversionCache {
                limit: budget.memory.map_or(MAX_CACHE_BYTES, |bytes| {
                    usize::try_from(bytes / 64)
                        .unwrap_or(usize::MAX)
                        .min(MAX_CACHE_BYTES)
                }),
                ..ConversionCache::default()
            },
            memo: Vec::new(),
            memo_slots: budget.memory.map_or(MAX_MEMO_BYTES, |bytes| {
                usize::try_from(bytes / 512)
                    .unwrap_or(usize::MAX)
                    .min(MAX_MEMO_BYTES)
            }) / std::mem::size_of::<u64>(),
            memo_hits: 0,
            names: Vec::new(),
            skipped: 0,
            deferred: false,
            deferred_rows: 0,
            admitted_through: 0,
            caught_up_rows: 0,
        })
    }

    /// Every accepted original is admitted before the producer retains metadata.
    /// The bound model whose names and contexts this converter uses.
    pub(crate) fn model(&self) -> &'a Model {
        self.model
    }

    /// Defers admission to reads (see the module docs). The caller
    /// proves that every liveness atom is native, so that only an actual
    /// fallback, source-row check or report reads values.
    pub(crate) fn defer_admission(&mut self) {
        self.deferred = true;
    }

    /// Whether an original of this event is admitted when it is recorded;
    /// otherwise its admission is counted as deferred.
    pub(crate) fn admits_now(&mut self, seen_root: bool) -> bool {
        if self.deferred && !seen_root {
            self.deferred_rows = self.deferred_rows.saturating_add(1);
            return false;
        }
        true
    }

    /// Admits a row: succeeds exactly when its Graph conversion would, and
    /// interns the same new names in the same order (see the module docs).
    pub(crate) fn admit(&mut self, row: &[Handle]) -> Result<(), Verdict> {
        self.admit_row(row).map_err(capture_error)
    }

    fn admit_row(&mut self, row: &[Handle]) -> Result<(), AnalysisError> {
        let control = self.capture.clone();
        check(&self.capture, &control)?;
        if row.len() != self.model.ir.vars().len() {
            return Err(AnalysisError::Inconsistent(
                "native value row variable dimensions",
            ));
        }
        let started = Instant::now();
        let before = self.converted;
        for &handle in row {
            self.admit_value(handle, &control)?;
        }
        check(&self.capture, &control)?;
        self.conversion += started.elapsed();
        if self.converted == before {
            self.skipped = self.skipped.saturating_add(1);
        }
        self.admitted = self.admitted.saturating_add(1);
        Ok(())
    }

    /// Admits, in retention order, every retained row a deferred admission
    /// has not admitted yet.
    pub(crate) fn catch_up(&mut self, graph: &NativeGraph) -> Result<(), AnalysisError> {
        if !self.deferred {
            return Ok(());
        }
        let retained = graph.input().retained_rows;
        while self.admitted_through < retained {
            self.admit_row(graph.row(self.admitted_through)?)?;
            self.admitted_through += 1;
            self.caught_up_rows = self.caught_up_rows.saturating_add(1);
        }
        Ok(())
    }

    /// Admits one value in Graph conversion order. A node is entered without
    /// conversion only when its own reference construction is infallible
    /// given convertible children; any other node is converted in full.
    fn admit_value(
        &mut self,
        handle: Handle,
        control: &EvaluationControl,
    ) -> Result<(), AnalysisError> {
        if !handle.is_compound() {
            let Some(id) = handle.as_str_id().or_else(|| handle.as_model_id()) else {
                // Booleans and inline integers always convert, interning nothing.
                if handle.as_bool().is_some() || handle.as_int().is_some() {
                    return Ok(());
                }
                return self.convert_value(handle, control).map(drop);
            };
            if self.name_converted(id) {
                return Ok(());
            }
            self.convert_value(handle, control)?;
            self.remember_name(id);
            return Ok(());
        }
        if self.memo_contains(handle) {
            self.memo_hits = self.memo_hits.saturating_add(1);
            return Ok(());
        }
        check(&self.capture, control)?;
        let view = self.heap.view(handle);
        let enter = !view.is_lazy()
            && view.is_norm()
            && match view.kind {
                // `Value::tuple`, `Value::tuple_fcn` or distinct integer keys.
                dive_core::node::Kind::IntFcn => true,
                // Same-class scalar sets sort and deduplicate infallibly.
                dive_core::node::Kind::Set => {
                    view.values().is_empty() || scalar_class(view.values()).is_some()
                }
                // Distinct same-class scalar keys: no incomparable pair and no
                // duplicate; a record literal needs string keys.
                dive_core::node::Kind::Fcn => match scalar_class(view.keys()) {
                    Some(Scalar::Int) => !view.is_record_literal() && increasing(view.keys()),
                    Some(Scalar::Str) => view.keys().windows(2).all(|w| w[0] != w[1]),
                    None => view.keys().is_empty(),
                    Some(Scalar::Bool) => false,
                },
                _ => false,
            };
        if !enter {
            self.convert_value(handle, control)?;
            self.memo_insert(handle);
            return Ok(());
        }
        // Graph conversion visits a set's elements and an integer function's
        // values in order, and a function's pairs key first. A view of a
        // live handle is fixed, so it is fetched again per child.
        let (pairs, len) = (!view.keys().is_empty(), view.values().len());
        for at in 0..len {
            const CHANGED: AnalysisError =
                AnalysisError::Inconsistent("native admission view changed during its walk");
            if pairs {
                let key = self.heap.view(handle).keys().get(at).copied();
                self.admit_value(key.ok_or(CHANGED)?, control)?;
            }
            let value = self.heap.view(handle).values().get(at).copied();
            self.admit_value(value.ok_or(CHANGED)?, control)?;
        }
        self.memo_insert(handle);
        Ok(())
    }

    fn name_converted(&self, id: u32) -> bool {
        self.names
            .get(id as usize / 64)
            .is_some_and(|word| word & (1 << (id % 64)) != 0)
    }

    fn remember_name(&mut self, id: u32) {
        let word = id as usize / 64;
        if word >= self.names.len() {
            if self.names.try_reserve(word + 1 - self.names.len()).is_err() {
                return;
            }
            self.names.resize(word + 1, 0);
        }
        self.names[word] |= 1 << (id % 64);
    }

    fn memo_index(&self, handle: Handle) -> usize {
        let mut bits = handle.bits();
        bits = (bits ^ (bits >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        bits ^= bits >> 33;
        usize::try_from(bits % self.memo.len() as u64).unwrap_or(0)
    }

    fn memo_contains(&self, handle: Handle) -> bool {
        handle.is_persistent()
            && !self.memo.is_empty()
            && self.memo[self.memo_index(handle)] == handle.bits()
    }

    /// Remembers a persistent handle whose conversion is proved to succeed.
    /// The table is lossy: an overwritten handle is only walked again.
    fn memo_insert(&mut self, handle: Handle) {
        if !handle.is_persistent() || handle.bits() == 0 {
            return;
        }
        if self.memo.is_empty() {
            let slots = self.memo_slots;
            if slots == 0 || self.memo.try_reserve_exact(slots).is_err() {
                self.memo_slots = 0;
                return;
            }
            self.memo.resize(slots, 0);
        }
        let at = self.memo_index(handle);
        self.memo[at] = handle.bits();
    }

    /// Reads retained row `row_id` of `graph`, after admitting every retained
    /// row a deferred admission has not admitted yet.
    pub(crate) fn read_retained(
        &mut self,
        graph: &NativeGraph,
        row_id: usize,
        control: &EvaluationControl,
    ) -> Result<StateVals, AnalysisError> {
        self.read_retained_as(graph, row_id, graph.row(row_id)?, control)
    }

    /// [`Self::read_retained`] of an explicit original whose storage hint is
    /// `row_id`.
    pub(crate) fn read_retained_as(
        &mut self,
        graph: &NativeGraph,
        row_id: usize,
        row: &[Handle],
        control: &EvaluationControl,
    ) -> Result<StateVals, AnalysisError> {
        self.catch_up(graph)?;
        self.read_row(row_id, row, control)
    }

    /// Reads an explicit row without admitting pending rows first. Producers
    /// call [`Self::read_retained`]; this direct form keeps converter tests.
    #[cfg(test)]
    pub(crate) fn read(
        &mut self,
        row_id: usize,
        row: &[Handle],
        control: &EvaluationControl,
    ) -> Result<StateVals, AnalysisError> {
        self.read_row(row_id, row, control)
    }

    fn read_row(
        &mut self,
        row_id: usize,
        row: &[Handle],
        control: &EvaluationControl,
    ) -> Result<StateVals, AnalysisError> {
        let values = self.convert(row, control, Some(row_id))?;
        self.read = self.read.saturating_add(1);
        Ok(values)
    }

    fn convert(
        &mut self,
        row: &[Handle],
        control: &EvaluationControl,
        row_id: Option<usize>,
    ) -> Result<StateVals, AnalysisError> {
        check(&self.capture, control)?;
        if row.len() != self.model.ir.vars().len() {
            return Err(AnalysisError::Inconsistent(
                "native value row variable dimensions",
            ));
        }
        let reuse = row_id.is_some();
        let single = reuse && row.len() == 1;
        if single {
            check(&self.capture, control)?;
            if let Some(values) = self.cache.get(row[0], row_id) {
                check(&self.capture, control)?;
                return Ok(values.clone());
            }
        }
        let started = Instant::now();
        let mut values = Vec::new();
        values
            .try_reserve_exact(row.len())
            .map_err(|_| AnalysisError::Limit("native conversion row storage"))?;
        for &handle in row {
            check(&self.capture, control)?;
            if reuse
                && !single
                && let Some(row) = self.cache.get(handle, None)
            {
                values.push(row[0].clone());
                continue;
            }
            let value = self.convert_value(handle, control)?;
            if reuse {
                let mut cache_proceed = || check(&self.capture, control).is_ok();
                if let Some(bytes) =
                    value.closed_cache_heap_bytes(&self.model.names, &mut cache_proceed)
                {
                    self.cache.eligible += 1;
                    self.cache.insert(
                        handle,
                        &value,
                        bytes,
                        if single { row_id } else { None },
                        &mut cache_proceed,
                    );
                } else {
                    self.cache.uncovered += 1;
                }
            }
            values.push(Some(value));
        }
        check(&self.capture, control)?;
        self.conversion += started.elapsed();
        Ok(values.into())
    }

    /// One full Graph conversion, with the run's cooperative checkpoints.
    fn convert_value(
        &mut self,
        handle: Handle,
        control: &EvaluationControl,
    ) -> Result<Value, AnalysisError> {
        let mut interrupted = None;
        let mut proceed = || match self.capture.check().and_then(|()| control.check()) {
            Ok(()) => true,
            Err(cause) => {
                interrupted = Some(EvalError::interrupted(cause));
                false
            }
        };
        let value = dive_core::print::to_value_controlled(
            &mut self.heap,
            &self.model.names,
            handle,
            self.model.evaluator.options().ctx,
            dive_core::print::Conversion::Graph,
            &mut proceed,
        )
        .map_err(|error| match error {
            dive_core::print::ConversionError::Construction(
                dive_value::ConstructionError::Interrupted,
            ) => interruption(&self.capture, control, interrupted),
            dive_core::print::ConversionError::Construction(
                dive_value::ConstructionError::Allocation,
            ) => AnalysisError::Limit("native conversion scratch storage"),
            error => AnalysisError::UnsupportedValue(error.to_string()),
        })?;
        self.converted = self.converted.saturating_add(1);
        Ok(value)
    }

    pub(crate) fn metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "admission": if self.deferred { "deferred" } else { "eager" },
            "deferred_rows": self.deferred_rows, "caught_up_rows": self.caught_up_rows,
            "admitted_rows": self.admitted, "read_rows": self.read,
            "converted_values": self.converted,
            "skipped_admission_conversions": self.skipped,
            "admission_memo_hits": self.memo_hits,
            "admission_memo_bytes": self.memo.capacity() * std::mem::size_of::<u64>(),
            "convert_ms": self.conversion.as_secs_f64() * 1e3,
            "convert_ms_scope": "admission and read slow paths; single-variable cache hits are not timed",
            "cache_entries": self.cache.entries, "cache_bytes": self.cache.bytes,
            "cache_enabled": !self.cache.slots.is_empty(),
            "cache_limit_bytes": self.cache.limit,
            "cache_table_bytes": self.cache.slots.capacity() * std::mem::size_of::<Option<Entry>>(),
            "cache_slots": self.cache.slots.len(), "cache_slot_capacity": self.cache.slots.capacity(),
            "cache_entry_bytes": std::mem::size_of::<Option<Entry>>(),
            "cache_hits": self.cache.hits, "cache_misses": self.cache.misses,
            "cache_eligible_misses": self.cache.eligible, "cache_uncovered_misses": self.cache.uncovered,
            "cache_insertions": self.cache.insertions, "cache_probe_exhaustions": self.cache.probe_exhaustions,
            "cache_byte_rejections": self.cache.byte_rejections,
            "cache_allocation_bypasses": self.cache.allocation_bypasses,
            "cache_direct_lookups": self.cache.direct_lookups,
            "cache_direct_hits": self.cache.direct_hits,
            "cache_row_index_fallbacks": self.cache.row_index_fallbacks,
            "cache_evictions": self.cache.evictions, "cache_bypasses": self.cache.bypasses,
            "cache_scope": "requested table capacity, single-component row arrays and closed-function/set payload including potential scalar-domain order storage; string/field-name payload already pinned by run Interner; opaque Arc/allocator overhead excluded; not total RSS",
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scalar {
    Int,
    Str,
    Bool,
}

/// The common class of inline scalars (integers, strings or booleans), if
/// every handle has the same one. Model values and compounds have none.
fn scalar_class(handles: &[Handle]) -> Option<Scalar> {
    let class = |h: Handle| {
        if h.is_compound() {
            None
        } else if h.as_int().is_some() {
            Some(Scalar::Int)
        } else if h.as_str_id().is_some() {
            Some(Scalar::Str)
        } else if h.as_bool().is_some() {
            Some(Scalar::Bool)
        } else {
            None
        }
    };
    let first = class(*handles.first()?)?;
    handles
        .iter()
        .all(|&h| class(h) == Some(first))
        .then_some(first)
}

fn increasing(keys: &[Handle]) -> bool {
    keys.windows(2)
        .all(|w| matches!((w[0].as_int(), w[1].as_int()), (Some(a), Some(b)) if a < b))
}

fn check(owner: &EvaluationControl, control: &EvaluationControl) -> Result<(), AnalysisError> {
    owner
        .check()
        .and_then(|()| control.check())
        .map_err(|cause| interruption(owner, control, Some(EvalError::interrupted(cause))))
}

fn interruption(
    owner: &EvaluationControl,
    control: &EvaluationControl,
    cause: Option<EvalError>,
) -> AnalysisError {
    match cause
        .filter(|cause| owner.owns(cause))
        .and_then(|cause| control.verdict(&cause))
    {
        Some(Verdict::Stopped(stop)) => AnalysisError::Stopped(stop),
        Some(Verdict::BudgetMeasurement { error }) => AnalysisError::BudgetMeasurement(error),
        _ => AnalysisError::Inconsistent(
            "native conversion interruption lacks matching run evidence",
        ),
    }
}

pub(crate) fn capture_error(error: AnalysisError) -> Verdict {
    match error {
        AnalysisError::Stopped(stopped) => Verdict::Stopped(stopped),
        AnalysisError::BudgetMeasurement(error) => Verdict::BudgetMeasurement { error },
        AnalysisError::UnsupportedValue(message) => Verdict::Unsupported {
            feature: format!(
                "a state value outside the reference values (core liveness): {message}"
            ),
        },
        AnalysisError::Limit(bound) => Verdict::Unsupported {
            feature: format!("native graph conversion storage: {bound}"),
        },
        error => Verdict::InternalError {
            message: error.to_string(),
        },
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite native admission fixtures"
)]
mod tests {
    use super::*;

    fn model() -> Model {
        model_source(
            "---- MODULE V ----\nVARIABLE x\nInit == x = 0\nNext == x' = x\nP == <>(x = 0)\n====\n",
        )
    }

    fn model_source(source: &str) -> Model {
        let parsed = dive_sema::analyze_source("V.tla", source, &dive_sema::Options::default());
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("config");
        dive_model::bind(
            Arc::new(dive_ir::lower(&parsed).expect("IR")),
            &cfg,
            &dive_model::BindOptions::dive(),
        )
        .expect("model")
    }

    #[test]
    fn graph_admission_preserves_classes_refuses_symbolic_identity_and_retains_no_values() {
        let model = model();
        let store = Arc::new(Store::default());
        let mut heap = Heap::new(store.clone(), false, false);
        let key = heap.string("field").expect("field");
        let zero = Handle::int(0).expect("integer");
        let function = heap.fcn_sorted(&[key], &[zero]);
        let function = heap.intern_exact(function).expect("persistent function");
        let record = heap.record(&[key], &[zero]);
        let record = heap.intern_exact(record).expect("persistent record");
        let mut values = NativeValues::new(&model, store, &Budget::unlimited()).expect("converter");
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        for (handle, literal) in [(function, false), (record, true)] {
            values.admit(&[handle]).expect("admission");
            let row = values.read(0, &[handle], &control).expect("read");
            assert_eq!(
                row[0]
                    .as_ref()
                    .expect("value")
                    .as_fcn()
                    .expect("function")
                    .is_record_literal(),
                literal
            );
            let released = Arc::downgrade(&row);
            drop(row);
            assert!(
                released.upgrade().is_none(),
                "the converter retains no converted row"
            );
        }
        let raw = heap.set_enum(&[zero]);
        let raw = heap.intern_exact(raw).expect("persistent raw");
        assert!(
            matches!(values.admit(&[raw]), Err(Verdict::Unsupported { feature }) if feature.contains("unmaterialized symbolic"))
        );
        assert_eq!(values.metrics()["cache_entries"], 0);
        assert_eq!(values.metrics()["cache_bytes"], 0);
        assert_eq!(values.metrics()["admitted_rows"], 2);
    }

    fn bound(source: &str, tlc: bool) -> Model {
        let parsed = dive_sema::analyze_source("V.tla", source, &dive_sema::Options::default());
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("config");
        let options = if tlc {
            dive_model::BindOptions::tlc()
        } else {
            dive_model::BindOptions::dive()
        };
        dive_model::bind(
            Arc::new(dive_ir::lower(&parsed).expect("IR")),
            &cfg,
            &options,
        )
        .expect("model")
    }

    fn names_of(model: &Model) -> Vec<String> {
        (0..)
            .map_while(|tok| model.names.resolve(tok))
            .map(|atom| atom.to_string())
            .collect()
    }

    /// Runtime strings (as `ToString` creates them) are converted at their
    /// first admission, so the reference names gain them in exactly the order
    /// and at exactly the event of a full Graph conversion; refusals match.
    #[test]
    fn admission_interns_fresh_names_and_refuses_as_full_conversion_in_both_profiles() {
        let source = "---- MODULE V ----\nVARIABLE x, y\nInit == x = 0 /\\ y = 0\nNext == UNCHANGED <<x, y>>\nP == <>(x = 0)\n====\n";
        for tlc in [false, true] {
            let admitted = bound(source, tlc);
            let converted = bound(source, tlc);
            let store = Arc::new(Store::default());
            let mut heap = Heap::new(store.clone(), false, false);
            let text = |heap: &Heap, name: &str| heap.string(name).expect("runtime string");
            let (sb, sa, sc, sd, se, sf) = (
                text(&heap, "fresh b"),
                text(&heap, "fresh a"),
                text(&heap, "fresh c"),
                text(&heap, "fresh d"),
                text(&heap, "fresh e"),
                text(&heap, "fresh f"),
            );
            let one = Handle::int(1).expect("integer");
            let two = Handle::int(2).expect("integer");
            let mut rows = Vec::new();
            let record = heap.record(&[sb], &[sa]);
            let tuple = heap.tuple_literal(&[sc, record, one]);
            let mut strings = vec![sd, sb, sa];
            let strings = heap.set_from(&mut strings);
            rows.push([tuple, strings]);
            // Repeated rows and components: nothing new is converted.
            rows.push([tuple, strings]);
            let mut pairs = [(se, one), (sa, two)];
            let function = heap.fcn_from_pairs(&mut pairs);
            let mut ints = vec![two, one];
            let ints = heap.set_from(&mut ints);
            let counter = heap.int_fcn(0, &[ints, function]);
            rows.push([counter, strings]);
            // A set of compound elements is converted in full.
            let mut records = vec![record, tuple];
            let records = heap.set_from(&mut records);
            rows.push([records, sf]);
            let mut mixed = vec![one, sf];
            let mixed = heap.set_from(&mut mixed);
            rows.push([mixed, one]);
            let raw = heap.set_enum(&[one]);
            let refused = heap.tuple_literal(&[text(&heap, "fresh g"), raw]);
            rows.push([one, refused]);
            let rows = rows
                .into_iter()
                .map(|row| row.map(|h| heap.intern_exact(h).expect("persistent original")))
                .collect::<Vec<_>>();
            let mut values =
                NativeValues::new(&admitted, store.clone(), &Budget::unlimited()).expect("values");
            let mut reference = Heap::new(store, false, false);
            let ctx = converted.evaluator.options().ctx;
            for (at, row) in rows.iter().enumerate() {
                let admission = values.admit(row);
                // A materialized mixed set is still refused at admission, as
                // its reference construction compares a string with an integer.
                assert_eq!(admission.is_err(), at >= 4, "row {at} tlc={tlc}");
                let full = row.iter().try_for_each(|&handle| {
                    dive_core::print::to_value_controlled(
                        &mut reference,
                        &converted.names,
                        handle,
                        ctx,
                        dive_core::print::Conversion::Graph,
                        &mut || true,
                    )
                    .map(drop)
                });
                match (&admission, &full) {
                    (Ok(()), Ok(())) => {}
                    (Err(Verdict::Unsupported { feature }), Err(error)) => {
                        assert!(feature.ends_with(&error.to_string()), "{feature} {error}");
                    }
                    _ => panic!("row {at} tlc={tlc}: {admission:?} {full:?}"),
                }
                assert_eq!(
                    names_of(&admitted),
                    names_of(&converted),
                    "row {at} tlc={tlc}: name encounter order"
                );
            }
            assert!(names_of(&admitted).iter().any(|name| name == "fresh g"));
            assert!(values.skipped >= 1, "a repeated row converts nothing");
            let control =
                EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
            for (at, row) in rows.iter().enumerate().take(4) {
                let read = values.read(at, row, &control).expect("read");
                for (slot, &handle) in read.iter().zip(row) {
                    let full = dive_core::print::to_value(
                        &mut reference,
                        &converted.names,
                        handle,
                        ctx,
                        dive_core::print::Conversion::Graph,
                    )
                    .expect("full conversion");
                    assert_eq!(slot.as_ref(), Some(&full), "row {at} tlc={tlc}");
                }
            }
        }
    }

    fn interned_names(model: &Model) -> u32 {
        (0..u32::MAX)
            .find(|&tok| model.names.resolve(tok).is_none())
            .unwrap_or(u32::MAX)
    }

    #[test]
    fn reads_after_admission_intern_no_new_name() {
        let model = model();
        let store = Arc::new(Store::default());
        let mut heap = Heap::new(store.clone(), false, false);
        let field = heap.string("admitted_field_name").expect("field");
        let text = heap.string("admitted string payload").expect("text");
        let inner = heap.record(&[field], &[text]);
        let inner = heap.intern_exact(inner).expect("persistent record");
        let outer = heap.fcn_sorted(&[text], &[inner]);
        let outer = heap.intern_exact(outer).expect("persistent function");
        let mut values = NativeValues::new(&model, store, &Budget::unlimited()).expect("converter");
        values.cache.limit = 4096;
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        let before = interned_names(&model);
        values.admit(&[outer]).expect("admission");
        let admitted = interned_names(&model);
        assert!(admitted > before, "admission interns the row's names");
        for _ in 0..3 {
            values.read(0, &[outer], &control).expect("read");
            assert_eq!(
                interned_names(&model),
                admitted,
                "a read after admission interns no new name"
            );
        }
    }

    #[test]
    fn flat_record_and_tuple_hits_keep_exact_classes_and_admission_work() {
        let model = model();
        let store = Arc::new(Store::default());
        let mut heap = Heap::new(store.clone(), false, false);
        let key = heap.string("field").expect("field");
        let one = Handle::int(1).expect("integer");
        let handles = [
            heap.record(&[key], &[one]),
            heap.fcn_sorted(&[key], &[one]),
            heap.tuple_literal(&[one]),
            heap.tuple(&[one]),
        ]
        .into_iter()
        .map(|value| heap.intern_exact(value).expect("persistent value"))
        .collect::<Vec<_>>();
        let mut values = NativeValues::new(&model, store, &Budget::unlimited()).expect("converter");
        values.cache.limit = 4096;
        values.cache.misses = CACHE_WARMUP;
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        for (row_id, handle) in handles.into_iter().enumerate() {
            values.admit(&[handle]).expect("admit");
            let first = values.read(row_id, &[handle], &control).expect("miss");
            let before = values.converted;
            let second = values.read(row_id, &[handle], &control).expect("hit");
            let third = values
                .read(row_id, &[handle], &control)
                .expect("second hit");
            assert!(Arc::ptr_eq(&second, &third), "hits reuse the whole row");
            assert_eq!(values.converted, before, "a read hit does not reconvert");
            let a = first[0]
                .as_ref()
                .expect("value")
                .as_fcn()
                .expect("function");
            let b = second[0]
                .as_ref()
                .expect("value")
                .as_fcn()
                .expect("function");
            assert!(a.ptr_eq(b));
            assert_eq!(a.is_record_literal(), b.is_record_literal());
            assert_eq!(a.is_tuple_literal(), b.is_tuple_literal());
            let skipped = values.skipped;
            values.admit(&[handle]).expect("repeat admission");
            assert_eq!(
                (values.converted, values.skipped),
                (before, skipped + 1),
                "a repeat admission neither converts nor uses the read cache"
            );
        }
        assert_eq!(
            values.cache.entries, 4,
            "equal literal/general values have distinct exact handles"
        );
        assert_eq!(values.cache.hits, 8);
        assert_eq!(values.admitted, 8);
        let raw = heap.set_enum(&[one]);
        let raw = heap.intern_exact(raw).expect("raw");
        assert!(matches!(
            values.read(4, &[raw], &control),
            Err(AnalysisError::UnsupportedValue(_))
        ));
        assert_eq!(
            values.cache.entries, 4,
            "refused history does not populate cache"
        );
    }

    #[test]
    fn multi_variable_reads_reuse_components_without_aliasing_whole_rows() {
        let model = model_source(
            "---- MODULE V ----\nVARIABLE x, y\nInit == x = 0 /\\ y = 1\nNext == x' = x /\\ y' = y\nP == <>(x = 0)\n====\n",
        );
        let store = Arc::new(Store::default());
        let mut values = NativeValues::new(&model, store, &Budget::unlimited()).expect("converter");
        values.cache.limit = 4096;
        values.cache.misses = CACHE_WARMUP;
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        let row = [Handle::int(1).expect("integer"), Handle::bool(true)];
        let first = values.read(0, &row, &control).expect("misses");
        let second = values.read(0, &row, &control).expect("hits");
        assert_eq!(first, second);
        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(values.converted, 2);
        assert_eq!(values.cache.hits, 2);
        assert_eq!(values.cache.misses, CACHE_WARMUP + 2);
        values.admit(&row).expect("admit");
        assert_eq!(
            (values.converted, values.skipped),
            (2, 1),
            "scalar rows are admitted without conversion"
        );
        assert_eq!(values.cache.hits, 2);
    }

    #[test]
    fn retained_row_indices_are_only_hints_and_large_indices_use_exact_handle_fallback() {
        let model = model();
        let mut values =
            NativeValues::new(&model, Arc::new(Store::default()), &Budget::unlimited())
                .expect("converter");
        values.cache.limit = 4096;
        values.cache.misses = CACHE_WARMUP;
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        let one = Handle::int(1).expect("integer");
        let two = Handle::int(2).expect("integer");
        values.read(7, &[one], &control).expect("first miss");
        let first = values.read(7, &[one], &control).expect("first hit");
        let second = values.read(7, &[one], &control).expect("second hit");
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(values.converted, 1);
        let changed = values
            .read(7, &[two], &control)
            .expect("changed exact handle");
        assert_eq!(changed[0], Some(Value::Int(2)));
        assert_eq!(values.converted, 2, "a row ID alone cannot mint a hit");
        values
            .read(8, &[two], &control)
            .expect("same handle at another ID");
        assert_eq!(values.converted, 3, "cross-row aliases may reconvert");
        let before = values.cache.row_index_fallbacks;
        values
            .read(usize::MAX, &[one], &control)
            .expect("large ID miss");
        let a = values
            .read(usize::MAX, &[one], &control)
            .expect("large ID hit");
        let b = values
            .read(usize::MAX, &[one], &control)
            .expect("another large ID hit");
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(values.cache.row_index_fallbacks, before + 3);
        assert!(values.cache.bytes <= values.cache.limit);
        assert_eq!(values.cache.direct_hits, 2);
        assert_eq!(
            first[0],
            Some(Value::Int(1)),
            "replacement preserves an owned row"
        );
        values.cache = ConversionCache {
            limit: std::mem::size_of::<Option<Entry>>() * 4 - 1,
            misses: CACHE_WARMUP,
            ..ConversionCache::default()
        };
        let before = values.converted;
        values
            .read(0, &[one], &control)
            .expect("tiny allowance bypass");
        values
            .read(usize::MAX, &[one], &control)
            .expect("tiny large-ID bypass");
        assert_eq!(values.converted, before + 2);
        assert!(values.cache.slots.is_empty());
        assert_eq!(values.cache.bytes, 0);
    }

    #[test]
    fn table_payload_bound_collision_eviction_and_disabled_cache_are_explicit() {
        let mut cache = ConversionCache {
            limit: 4096,
            misses: CACHE_WARMUP,
            ..ConversionCache::default()
        };
        let first = Handle::int(0).expect("integer");
        cache.insert(first, &Value::Int(0), 0, None, &mut || true);
        let table_bytes = cache.slots.capacity() * std::mem::size_of::<Option<Entry>>();
        let row_bytes = std::mem::size_of::<Option<Value>>();
        assert_eq!(cache.bytes, table_bytes + row_bytes);
        let at = cache.index(first);
        let collisions = (1..10000)
            .filter_map(Handle::int)
            .filter(|&key| cache.index(key) == at)
            .take(CACHE_PROBES)
            .collect::<Vec<_>>();
        assert_eq!(collisions.len(), CACHE_PROBES);
        for &key in &collisions {
            cache.insert(key, &Value::Int(1), 0, None, &mut || true);
        }
        assert_eq!(cache.evictions, 1);
        assert!(cache.get(first, None).is_none());
        assert!(cache.get(collisions[CACHE_PROBES - 1], None).is_some());
        assert_eq!(cache.bytes, table_bytes + CACHE_PROBES * row_bytes);
        let before = cache.entries;
        let key = Handle::int(10001).expect("integer");
        cache.insert(key, &Value::Int(0), cache.limit, None, &mut || true);
        assert_eq!(cache.entries, before);
        assert_eq!(cache.bypasses, 1);
        assert!(cache.bytes <= cache.limit);
        let payload = cache.limit - cache.bytes;
        cache.insert(first, &Value::Int(0), payload, None, &mut || true);
        assert_eq!(
            cache.bytes, cache.limit,
            "exact requested-byte boundary is admitted"
        );
        cache.insert(key, &Value::Int(0), payload + 1, None, &mut || true);
        assert!(cache.bytes <= cache.limit);
        let mut disabled = ConversionCache {
            misses: CACHE_WARMUP,
            ..ConversionCache::default()
        };
        disabled.insert(first, &Value::Int(0), 0, None, &mut || true);
        assert!(disabled.slots.is_empty());
    }

    #[test]
    fn actual_temporal_stop_precedes_a_populated_hit_and_cache_fraction_is_bounded() {
        let model = model();
        let watchdog = Budget::new(Some(Duration::ZERO), None)
            .start()
            .expect("start")
            .expect("watchdog");
        let mut values = NativeValues::new(&model, Arc::new(Store::default()), watchdog.budget())
            .expect("converter");
        values.cache.limit = 4096;
        values.cache.misses = CACHE_WARMUP;
        let one = Handle::int(1).expect("integer");
        let handle = values.heap.set_sorted(&[one]);
        let handle = values.heap.intern_exact(handle).expect("persistent set");
        let converted = dive_core::print::to_value_controlled(
            &mut values.heap,
            &model.names,
            handle,
            model.evaluator.options().ctx,
            dive_core::print::Conversion::Graph,
            &mut || true,
        )
        .expect("successful immutable conversion");
        let bytes = converted
            .closed_cache_heap_bytes(&model.names, &mut || true)
            .expect("closed set payload");
        values
            .cache
            .insert(handle, &converted, bytes, Some(0), &mut || true);
        assert_eq!(values.cache.entries, 1);
        let control = EvaluationControl::new(watchdog.budget(), Phase::Temporal).expect("control");
        assert!(
            matches!(values.read(0, &[handle], &control), Err(AnalysisError::Stopped(stop)) if stop.phase == Phase::Temporal)
        );
        assert_eq!(values.cache.hits, 0);
        let limited = Budget::new(None, Some(64 * 4096));
        let watchdog = limited.start().expect("arm memory").expect("watchdog");
        let restricted = NativeValues::new(&model, Arc::new(Store::default()), watchdog.budget())
            .expect("converter");
        assert_eq!(restricted.cache.limit, 4096);
    }

    #[test]
    fn foreign_interruption_and_bad_rows_cannot_become_success_or_this_runs_stop() {
        let model = model();
        let mut values =
            NativeValues::new(&model, Arc::new(Store::default()), &Budget::unlimited())
                .expect("converter");
        let watchdog = Budget::new(Some(Duration::ZERO), None)
            .start()
            .expect("start")
            .expect("watchdog");
        let foreign =
            EvaluationControl::new(watchdog.budget(), Phase::Temporal).expect("foreign control");
        assert!(matches!(
            values.read(0, &[Handle::bool(true)], &foreign),
            Err(AnalysisError::Inconsistent(_))
        ));
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        assert!(matches!(
            values.read(0, &[], &control),
            Err(AnalysisError::Inconsistent(_))
        ));
        let mut stopped = NativeValues::new(&model, Arc::new(Store::default()), watchdog.budget())
            .expect("armed converter");
        assert!(
            matches!(stopped.admit(&[Handle::bool(true)]), Err(Verdict::Stopped(stop)) if stop.phase == Phase::Capture)
        );
        assert!(
            matches!(stopped.read(0, &[Handle::bool(true)], &foreign), Err(AnalysisError::Stopped(stop)) if stop.phase == Phase::Temporal)
        );
    }

    #[test]
    fn nested_closed_function_hits_preserve_tlc_behavior_and_eager_admission() {
        let model = model();
        let store = Arc::new(Store::default());
        let mut heap = Heap::new(store.clone(), false, false);
        let key = heap.string("message").expect("field");
        let text = heap.string("tok").expect("string");
        let one = Handle::int(1).expect("integer");
        let mut handles = Vec::new();
        for literal in [false, true] {
            let child = if literal {
                heap.record(&[key], &[text])
            } else {
                heap.fcn_sorted(&[key], &[text])
            };
            let nested = heap.tuple_literal(&[child]);
            handles.push(heap.intern_exact(nested).expect("exact original"));
        }
        let counter = heap.int_fcn(0, &[one, one]);
        handles.push(heap.intern_exact(counter).expect("integer domain"));
        let mut values = NativeValues::new(&model, store, &Budget::unlimited()).expect("converter");
        values.cache.limit = 16384;
        values.cache.misses = CACHE_WARMUP;
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        let rhs = Value::tuple(vec![Value::tuple(vec![Value::Int(0), Value::Int(0)])]);
        for (row_id, &handle) in handles.iter().enumerate() {
            values.admit(&[handle]).expect("eager conversion");
            let first = values.read(row_id, &[handle], &control).expect("miss");
            let before = values.converted;
            let hit = values.read(row_id, &[handle], &control).expect("hit");
            assert!(
                first[0]
                    .as_ref()
                    .expect("first value")
                    .as_fcn()
                    .expect("first function")
                    .ptr_eq(
                        hit[0]
                            .as_ref()
                            .expect("hit value")
                            .as_fcn()
                            .expect("hit function")
                    )
            );
            assert_eq!(values.converted, before);
            if row_id < 2 {
                let comparison = dive_value::tlc::equals(
                    hit[0].as_ref().expect("value"),
                    &rhs,
                    dive_value::Ctx::tlc(),
                );
                assert_eq!(
                    comparison.is_err(),
                    row_id == 1,
                    "literal record and general function remain behaviorally distinct"
                );
                if row_id == 0 {
                    assert!(!comparison.expect("general comparison"));
                }
            }
            let skipped = values.skipped;
            values.admit(&[handle]).expect("repeat admission");
            assert_eq!((values.converted, values.skipped), (before, skipped + 1));
        }
        assert_eq!(values.cache.entries, 3);
        let other = NativeValues::new(&model, Arc::new(Store::default()), &Budget::unlimited())
            .expect("other owner");
        assert_eq!(other.cache.entries, 0);
        // History-bearing descendants remain refused even with a hot cache.
        let raw = heap.set_enum(&[one]);
        let nested_raw = heap.tuple_literal(&[raw]);
        let nested_raw = heap.intern_exact(nested_raw).expect("original raw child");
        assert!(matches!(
            values.read(7, &[nested_raw], &control),
            Err(AnalysisError::UnsupportedValue(_))
        ));
        assert_eq!(values.cache.entries, 3);
        let normalized = heap.set_sorted(&[one]);
        let normalized = heap.intern_exact(normalized).expect("persistent set");
        let before = values.converted;
        values
            .read(8, &[normalized], &control)
            .expect("first normalized set read");
        values
            .read(8, &[normalized], &control)
            .expect("cached normalized set read");
        assert_eq!(values.converted, before + 1);
        assert_eq!(values.cache.entries, 4);
    }

    #[test]
    fn concrete_set_hits_keep_nested_classes_exact_handles_and_payload_limits() {
        let model = model();
        let store = Arc::new(Store::default());
        let mut heap = Heap::new(store.clone(), false, false);
        let key = heap.string("field").expect("field");
        let one = Handle::int(1).expect("integer");
        let record = heap.record(&[key], &[one]);
        let function = heap.fcn_sorted(&[key], &[one]);
        let mut handles = Vec::new();
        for child in [record, function] {
            let set = heap.set_sorted(&[child]);
            let tuple = heap.tuple_literal(&[set]);
            handles.push(heap.intern_exact(tuple).expect("persistent nested set"));
        }
        let mut values = NativeValues::new(&model, store, &Budget::unlimited()).expect("converter");
        values.cache.limit = 4096;
        values.cache.misses = CACHE_WARMUP;
        let control =
            EvaluationControl::new(&Budget::unlimited(), Phase::Temporal).expect("control");
        for (row, &handle) in handles.iter().enumerate() {
            values.admit(&[handle]).expect("eager admission");
            let miss = values.read(row, &[handle], &control).expect("conversion");
            let converted = values.converted;
            let hit = values.read(row, &[handle], &control).expect("cached set");
            assert_eq!(miss, hit);
            assert_eq!(values.converted, converted);
            let root = hit[0].as_ref().expect("value").as_fcn().expect("tuple");
            assert!(root.is_tuple_literal());
            let set = root
                .values()
                .next()
                .expect("tuple element")
                .as_set()
                .expect("set child");
            let child = set.stored_elements().expect("materialized")[0]
                .as_fcn()
                .expect("function child");
            assert_eq!(child.is_record_literal(), row == 0);
            let skipped = values.skipped;
            values.admit(&[handle]).expect("repeat admission");
            assert_eq!(
                (values.converted, values.skipped),
                (converted, skipped + 1),
                "a proved original is not converted again"
            );
        }
        assert_eq!(
            values.cache.entries, 2,
            "equal mathematical sets retain their exact source representations"
        );
        assert!(values.cache.bytes <= values.cache.limit);
        values.cache = ConversionCache {
            limit: 1600,
            misses: CACHE_WARMUP,
            ..ConversionCache::default()
        };
        let mut many = Vec::new();
        for n in 0..128 {
            many.push(Handle::int(n).expect("integer"));
        }
        let large = heap.set_sorted(&many);
        let large = heap.intern_exact(large).expect("persistent set");
        values
            .read(0, &[large], &control)
            .expect("oversize converts");
        let before = values.converted;
        values
            .read(0, &[large], &control)
            .expect("oversize reconverts");
        assert_eq!(values.converted, before + 1);
        assert_eq!(values.cache.entries, 0);
        assert!(values.cache.byte_rejections > 0);
        assert!(values.cache.bytes <= values.cache.limit);
    }
}
