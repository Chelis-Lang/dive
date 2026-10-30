//! Controlled Graph admission and on-demand values for the compact producer.
//!
//! Persistent roots stay with `NativeGraph`. A bounded exact-handle cache reuses
//! successful immediate/flat-value conversions during reads; eager admission
//! always converts, preserving its refusal and name encounter order.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dive_check::Verdict;
use dive_check::budget::{Budget, EvaluationControl, Phase};
use dive_core::{Handle, Heap, Store};
use dive_eval::next::StateVals;
use dive_eval::{EvalError, HostControl};
use dive_liveness::analyze::AnalysisError;
use dive_model::Model;
use dive_value::Value;

const MAX_CACHE_BYTES: usize = 128 * 1024 * 1024;
const CACHE_WARMUP: u64 = 1024;
const CACHE_PROBES: usize = 16;
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
        })
    }

    /// Every accepted original is admitted before the producer retains metadata.
    pub(crate) fn admit(&mut self, row: &[Handle]) -> Result<(), Verdict> {
        let control = self.capture.clone();
        self.convert(row, &control, None).map_err(capture_error)?;
        self.admitted = self.admitted.saturating_add(1);
        Ok(())
    }

    pub(crate) fn read(
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
            if reuse {
                let mut cache_proceed = || check(&self.capture, control).is_ok();
                if let Some(bytes) =
                    value.flat_cache_heap_bytes(&self.model.names, &mut cache_proceed)
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

    pub(crate) fn metrics(&self) -> serde_json::Value {
        serde_json::json!({
            "admitted_rows": self.admitted, "read_rows": self.read,
            "converted_values": self.converted,
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
            "cache_scope": "requested table capacity, single-component row arrays and eligible function/boxed payload; field-name payload already pinned by run Interner; opaque Arc/allocator overhead excluded; not total RSS",
        })
    }
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

fn capture_error(error: AnalysisError) -> Verdict {
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
#[allow(clippy::expect_used, reason = "finite native admission fixtures")]
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
    fn graph_admission_preserves_classes_refuses_unknown_history_and_retains_no_values() {
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
            matches!(values.admit(&[raw]), Err(Verdict::Unsupported { feature }) if feature.contains("normalization history"))
        );
        assert_eq!(values.metrics()["cache_entries"], 0);
        assert_eq!(values.metrics()["cache_bytes"], 0);
        assert_eq!(values.metrics()["admitted_rows"], 2);
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
            values.admit(&[handle]).expect("repeat admission");
            assert_eq!(
                values.converted,
                before + 1,
                "eager admission never uses cache"
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
        assert_eq!(values.converted, 4);
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
        let handle = Handle::int(1).expect("integer");
        let converted = dive_core::print::to_value_controlled(
            &mut values.heap,
            &model.names,
            handle,
            model.evaluator.options().ctx,
            dive_core::print::Conversion::Graph,
            &mut || true,
        )
        .expect("successful immutable conversion");
        values
            .cache
            .insert(handle, &converted, 0, Some(0), &mut || true);
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
}
