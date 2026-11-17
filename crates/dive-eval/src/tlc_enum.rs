//! TLC's `Enumerable.elements()`: the order, and the multiplicity, in which
//! TLC walks a set when nothing asks for normalized order.
//!
//! TLC normalizes before it enumerates in most places (`\E`, `\A`, `CHOOSE`,
//! set filters and maps call `elements(Ordering.NORMALIZED)` or normalize a
//! `SetEnumValue` first), and `dive-value` enumerates in that order. The
//! successor generator's `x' \in S` and `x \in S` are the exception: they call
//! `elements()` (`Tool.java:792`, `:1450`), whose order depends on the kind of
//! set (`tlc2/value/impl/*.java` at tlaplus `94d0c50`):
//!
//! | Set | `elements()` |
//! |---|---|
//! | `SetEnumValue`, `IntervalValue` | normalized (the enumerator normalizes first) |
//! | `SubsetValue` | normalized (`elementsNormalized`) |
//! | `SetOfFcnsValue`, `SetOfRcdsValue`, `SetOfTuplesValue` | an odometer over the components' `elements()`, the **last** position fastest (the domain of `[S -> T]` normalized) |
//! | `SetCupValue` | `S`'s elements, then `T`'s, duplicates included |
//! | `SetCapValue`, `SetDiffValue` | the enumerable operand's elements, filtered |
//! | `UnionValue` | each element set's elements in turn, duplicates included |
//! | `SetPredValue` | the bound set's elements, filtered |
//!
//! Every lazy kind except `SubsetValue` also caches a normalized copy the
//! first time TLC compares, sizes or fingerprints the same object
//! (`convertAndCache`, `SetPredValue.converted`), after which `elements()`
//! is normalized. Whether that has happened depends on what the same value
//! went through before, which dive does not track. So when a lazy set's
//! `elements()` sequence differs from its normalized one (a different order,
//! or duplicates), the enumeration is refused rather than guessed; when the
//! two coincide, which is the usual case, the history does not matter.

use std::cmp::Ordering;
use std::sync::Arc;

use dive_value::{DeferredKind, DeferredSet, SetElementStream, SetKind, Value};

use crate::core::{Core, TLC_ORDER, is_enumerable};
use crate::error::{EvalError, Result};
use crate::lazy::{Filter, SetOp};
use crate::options::ExplorationOrder;
use crate::subset_mask::SingletonMask;

/// A fallible pull cursor; original predicate work happens only when pulled.
/// Frames are outermost first, so their owners drop without a recursive cursor
/// chain. The original assigned value drops while every frame is still alive.
pub(crate) struct InElementStream {
    root: Option<Value>,
    filters: Vec<FilterFrame>,
    source: StreamSource,
    done: bool,
}

enum StreamSource {
    Plain(SetElementStream),
    Subset(Box<MaskedSubset>),
}

struct MaskedSubset {
    stream: dive_value::ConcreteSubsetStream,
    learning: SingletonMask,
}

impl StreamSource {
    fn before_next(&mut self, frame: Option<&FilterFrame>) -> Result<()> {
        if let (Self::Subset(subset), Some(frame)) = (self, frame) {
            let filter = private_filter(&frame.owner)?;
            subset
                .learning
                .before_next(&filter.core, &frame.owner, &mut subset.stream)?;
        }
        Ok(())
    }

    fn next(&mut self) -> Option<Value> {
        match self {
            Self::Plain(source) => source.next(),
            Self::Subset(subset) => subset.stream.next(),
        }
    }

    fn entry(&self) -> Option<(u32, u32)> {
        match self {
            Self::Plain(_) => None,
            Self::Subset(subset) => subset.learning.entry(),
        }
    }

    fn observed(&mut self, entry: Option<(u32, u32)>, kept: bool) {
        if let Self::Subset(subset) = self {
            subset.learning.observed(&subset.stream, entry, kept);
        }
    }
}

fn private_filter(owner: &Arc<dyn DeferredSet>) -> Result<&Filter> {
    owner
        .as_any()
        .and_then(|owner| owner.downcast_ref::<Filter>())
        .ok_or_else(|| EvalError::bug("private filter cursor owner changed type"))
}

struct FilterFrame {
    // Keep the original captured Filter/Core alive through active Env cleanup.
    env: crate::env::Env,
    owner: Arc<dyn DeferredSet>,
}

impl InElementStream {
    #[cfg(test)]
    pub(crate) fn mask_progress(&self) -> Option<(&'static str, usize)> {
        match &self.source {
            StreamSource::Plain(_) => None,
            StreamSource::Subset(subset) => Some(subset.learning.progress()),
        }
    }

    pub(crate) fn with_owner(mut self, root: Value) -> Self {
        self.root = Some(root);
        self
    }
}

impl Drop for InElementStream {
    fn drop(&mut self) {
        // Release the caller's original root while all input owners remain
        // retained, then release frames outermost first. No recursive cursor
        // next/drop chain is introduced by folded filter accumulation.
        drop(self.root.take());
        for frame in self.filters.drain(..) {
            drop(frame);
        }
    }
}

impl Iterator for InElementStream {
    type Item = Result<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        'input: loop {
            if let Err(error) = self.source.before_next(self.filters.first()) {
                self.done = true;
                return Some(Err(error));
            }
            let Some(value) = self.source.next() else {
                self.done = true;
                return None;
            };
            for frame in self.filters.iter_mut().rev() {
                let Some(filter) = frame
                    .owner
                    .as_any()
                    .and_then(|owner| owner.downcast_ref::<Filter>())
                else {
                    self.done = true;
                    return Some(Err(EvalError::bug(
                        "private filter cursor owner changed type",
                    )));
                };
                let entry = self.source.entry();
                match filter.core.filter_keeps(filter, &mut frame.env, &value) {
                    Ok(kept) => {
                        // Only a single authentic filter has a typed source.
                        // Its actual complete predicate result is observed once.
                        self.source.observed(entry, kept);
                        if !kept {
                            continue 'input;
                        }
                    }
                    Err(error) => {
                        self.done = true;
                        return Some(Err(error));
                    }
                }
            }
            return Some(Ok(value));
        }
    }
}

impl Core {
    /// Concrete raw storage or an authentic same-Core filter pull cursor.
    /// Every capability rejection precedes normalization, callbacks and input
    /// consumption. Cached/foreign/deferred-history owners keep their old route.
    pub(crate) fn in_element_stream(
        self: &Arc<Self>,
        set: &dive_value::Set,
    ) -> Result<Option<InElementStream>> {
        // Ordinary explicit assignments retain the existing in_elements path.
        // Explicit/interval streams are needed only below an authentic Filter.
        if matches!(set.kind(), SetKind::Enum | SetKind::Raw | SetKind::Interval) {
            return Ok(None);
        }
        let mut filters = Vec::new();
        let mut current = set;
        loop {
            if current.order() != self.opts.explore_order() {
                return Ok(None);
            }
            if current.kind() != SetKind::Deferred {
                // A single private filter owns the predicate/sink sequence.
                // Accumulated filters retain the original general pull path.
                let source = if filters.len() == 1 && current.kind() == SetKind::Subset {
                    current.concrete_subset_stream(self.ctx)?.map(|stream| {
                        StreamSource::Subset(Box::new(MaskedSubset {
                            stream,
                            learning: SingletonMask::default(),
                        }))
                    })
                } else {
                    current
                        .concrete_element_stream(self.ctx)?
                        .map(StreamSource::Plain)
                };
                return Ok(source.map(|source| InElementStream {
                    root: None,
                    filters,
                    source,
                    done: false,
                }));
            }
            if current.has_cached_elements() {
                return Ok(None);
            }
            let Some(owner) = current.as_deferred() else {
                return Ok(None);
            };
            // Any's blanket identity cannot invoke an unknown provider's
            // overridable as_any/kind/elements methods.
            if std::any::Any::type_id(owner.as_ref()) != std::any::TypeId::of::<Filter>() {
                return Ok(None);
            }
            let Some(filter) = owner
                .as_any()
                .and_then(|value| value.downcast_ref::<Filter>())
            else {
                return Ok(None);
            };
            if !Arc::ptr_eq(self, &filter.core) {
                return Ok(None);
            }
            let Value::Set(input) = &filter.in_val else {
                return Ok(None);
            };
            // Accumulated private filters need not be bounded by IR depth.
            // Preparation uses cooperative control and fallible flat storage.
            self.loop_work()?;
            filters.try_reserve(1).map_err(|error| {
                EvalError::resource_limit(format!("filter cursor frames: {error}"))
            })?;
            filters.push(FilterFrame {
                env: filter.env.clone(),
                owner: owner.clone(),
            });
            current = input;
        }
    }

    /// The elements `x' \in S` (and `x \in S` in an initial predicate) binds,
    /// in TLC's order and multiplicity (`Enumerable.elements()`). Under
    /// `--explore-order=value` the run's value order instead.
    pub(crate) fn in_elements(self: &Arc<Self>, set: &dive_value::Set) -> Result<Arc<[Value]>> {
        self.tlc_elements(set, "x' \\in S")
    }

    /// TLC's `Enumerable.elements()` order and multiplicity for `set`, walked
    /// by `what` (a Java override such as `FoldSet`, or `x' \in S`); under
    /// `--explore-order=value` the run's value order instead.
    pub(crate) fn tlc_elements(
        self: &Arc<Self>,
        set: &dive_value::Set,
        what: &str,
    ) -> Result<Arc<[Value]>> {
        if clean(set) {
            return self.explore(set);
        }
        let raw = self.tlc_raw(&Value::Set(set.clone()))?;
        let mut norm = raw.clone();
        norm.sort_by(|a, b| a.cmp_with(b, TLC_ORDER));
        norm.dedup_by(|a, b| a.cmp_with(b, TLC_ORDER) == Ordering::Equal);
        if self.opts.exploration == ExplorationOrder::Value && raw.len() == norm.len() {
            // The order is the run's; the multiplicity is TLC's.
            return self.explore(set);
        }
        let same = raw.len() == norm.len()
            && raw
                .iter()
                .zip(&norm)
                .all(|(a, b)| a.cmp_with(b, TLC_ORDER) == Ordering::Equal);
        if !same {
            return Err(EvalError::unsupported(format!(
                "enumerating the lazily represented set {} for {what}: TLC's unnormalized enumeration of it ({} elements) differs from its normalized one ({} elements), and which one TLC uses depends on whether it has already compared, sized or fingerprinted the same set object",
                self.ppr(&Value::Set(set.clone())),
                raw.len(),
                norm.len()
            )));
        }
        Ok(raw.into())
    }

    /// TLC's `elements()` sequence for a set, duplicates included.
    pub(crate) fn tlc_raw(self: &Arc<Self>, v: &Value) -> Result<Vec<Value>> {
        let Value::Set(s) = v else {
            return Err(EvalError::bug("enumerating a non-set"));
        };
        match s.kind() {
            SetKind::Enum
            | SetKind::Raw
            | SetKind::Interval
            | SetKind::Subset
            | SetKind::Nat
            | SetKind::Int
            | SetKind::Strings
            | SetKind::Seq => Ok(self.explore(s)?.to_vec()),
            SetKind::FcnSet => {
                self.enum_guard(s)?;
                let comps = s.components();
                let (Some((_, d)), Some((_, r))) = (comps.first(), comps.get(1)) else {
                    return Err(EvalError::bug("function set without components"));
                };
                let Value::Set(ds) = d else {
                    return Err(EvalError::bug("function set with a non-set domain"));
                };
                let dom = self.explore(ds)?;
                if dom.is_empty() {
                    return Ok(vec![Value::fcn(Vec::new(), self.ctx)?]);
                }
                let rng = self.tlc_raw(r)?;
                let columns: Vec<&[Value]> = dom.iter().map(|_| rng.as_slice()).collect();
                let mut out = Vec::new();
                for vals in odometer_last_fastest(&columns) {
                    let pairs = dom.iter().cloned().zip(vals).collect();
                    out.push(Value::fcn(pairs, self.ctx)?);
                }
                Ok(out)
            }
            SetKind::RcdSet => {
                self.enum_guard(s)?;
                let comps = s.components();
                let mut names = Vec::with_capacity(comps.len());
                let mut cols = Vec::with_capacity(comps.len());
                for (name, set) in comps {
                    let Some(name) = name else {
                        return Err(EvalError::bug("record set field without a name"));
                    };
                    names.push(name);
                    cols.push(self.tlc_raw(&set)?);
                }
                let columns: Vec<&[Value]> = cols.iter().map(Vec::as_slice).collect();
                let mut out = Vec::new();
                for vals in odometer_last_fastest(&columns) {
                    out.push(Value::record(
                        names.iter().cloned().zip(vals).collect(),
                        self.ctx,
                    )?);
                }
                Ok(out)
            }
            SetKind::Product => {
                self.enum_guard(s)?;
                let mut cols = Vec::new();
                for (_, set) in s.components() {
                    cols.push(self.tlc_raw(&set)?);
                }
                let columns: Vec<&[Value]> = cols.iter().map(Vec::as_slice).collect();
                Ok(odometer_last_fastest(&columns)
                    .into_iter()
                    .map(Value::tuple)
                    .collect())
            }
            SetKind::Deferred => {
                let Some(any) = s.as_deferred().and_then(|d| d.as_any()) else {
                    return Err(EvalError::unsupported(
                        "enumerating a lazily represented set of an unknown kind for x' \\in S",
                    ));
                };
                if let Some(f) = any.downcast_ref::<Filter>() {
                    let elems = if is_enumerable(&f.in_val) {
                        self.tlc_raw(&f.in_val)?
                    } else {
                        Vec::new()
                    };
                    return self.filter_elements_of(f, &elems);
                }
                if let Some(op) = any.downcast_ref::<SetOp>() {
                    return self.setop_raw(op);
                }
                Err(EvalError::unsupported(
                    "enumerating a lazily represented set of an unknown kind for x' \\in S",
                ))
            }
        }
    }

    /// `SetCupValue`, `SetCapValue`, `SetDiffValue` and `UnionValue`
    /// enumerators over their operands' `elements()`.
    fn setop_raw(self: &Arc<Self>, op: &SetOp) -> Result<Vec<Value>> {
        // The errors for operands that cannot be enumerated are those of the
        // normalized enumeration.
        match op.kind {
            DeferredKind::Cup => {
                if !is_enumerable(&op.a) || !is_enumerable(&op.b) {
                    return self.setop_elements(op);
                }
                let mut out = self.tlc_raw(&op.a)?;
                out.extend(self.tlc_raw(&op.b)?);
                Ok(out)
            }
            DeferredKind::Cap => {
                let (src, other) = if is_enumerable(&op.a) {
                    (&op.a, &op.b)
                } else if is_enumerable(&op.b) {
                    (&op.b, &op.a)
                } else {
                    return self.setop_elements(op);
                };
                let mut out = Vec::new();
                for x in self.tlc_raw(src)? {
                    if self.member(&x, other)? {
                        out.push(x);
                    }
                }
                Ok(out)
            }
            DeferredKind::Diff => {
                if !is_enumerable(&op.a) {
                    return self.setop_elements(op);
                }
                let mut out = Vec::new();
                for x in self.tlc_raw(&op.a)? {
                    if !self.member(&x, &op.b)? {
                        out.push(x);
                    }
                }
                Ok(out)
            }
            DeferredKind::Union => {
                if !is_enumerable(&op.a) {
                    return self.setop_elements(op);
                }
                let mut out = Vec::new();
                for inner in self.tlc_raw(&op.a)? {
                    if !is_enumerable(&inner) {
                        return Err(EvalError::general(
                            "Attempted to enumerate UNION(s), but some element of s is nonenumerable.",
                        ));
                    }
                    out.extend(self.tlc_raw(&inner)?);
                }
                Ok(out)
            }
            DeferredKind::Filter | DeferredKind::Map => {
                Err(EvalError::bug("set operator of the wrong kind"))
            }
        }
    }
}

/// Whether TLC's `elements()` of the set is its normalized enumeration
/// whatever happened to it before: no lazily represented part.
fn clean(set: &dive_value::Set) -> bool {
    match set.kind() {
        SetKind::Deferred => false,
        SetKind::FcnSet | SetKind::RcdSet | SetKind::Product => {
            set.components().iter().all(|(_, c)| match c {
                Value::Set(s) => clean(s),
                _ => true,
            })
        }
        _ => true,
    }
}

/// All combinations of the columns, the last column varying fastest (TLC's
/// product enumerators).
fn odometer_last_fastest(columns: &[&[Value]]) -> Vec<Vec<Value>> {
    let mut out = Vec::new();
    if columns.iter().any(|c| c.is_empty()) {
        return out;
    }
    let n = columns.len();
    let mut idx = vec![0usize; n];
    loop {
        out.push(
            idx.iter()
                .zip(columns)
                .map(|(&i, c)| c[i].clone())
                .collect(),
        );
        let mut k = n;
        loop {
            if k == 0 {
                return out;
            }
            k -= 1;
            idx[k] += 1;
            if idx[k] < columns[k].len() {
                break;
            }
            idx[k] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odometer_varies_the_last_column_fastest() {
        let a = [Value::Int(1), Value::Int(2)];
        let b = [Value::Int(3), Value::Int(4)];
        let got = odometer_last_fastest(&[&a, &b]);
        let want: Vec<Vec<Value>> = [(1, 3), (1, 4), (2, 3), (2, 4)]
            .iter()
            .map(|&(x, y)| vec![Value::Int(x), Value::Int(y)])
            .collect();
        assert_eq!(got, want);
        assert_eq!(odometer_last_fastest(&[&a, &[]]).len(), 0);
    }
}

#[cfg(test)]
#[path = "tlc_enum_cursor_tests.rs"]
mod cursor_tests;
