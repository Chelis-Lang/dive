//! Streaming a set's elements in normalized order, without materializing lazy sets.
//!
//! Each lazy set is generated directly in the order normalization would produce (as
//! TLC's `toSetEnum` does): intervals ascending, `SUBSET S` by size and then
//! lexicographically, and `[S -> T]`, `S \X T` and `[a : S]` as odometers whose last
//! position varies fastest. Only the components (`S`, `T`) are materialized.

use std::sync::Arc;

use crate::atom::Atom;
use crate::ctx::{Ctx, IntMode, ValueOrder};
use crate::error::{ErrorKind, Result, ValueError};
use crate::int::Int;
use crate::value::{Fcn, FcnRepr, Set, SetRepr, Value};

/// A stream of elements in normalized order.
pub(crate) type Elems = Box<dyn Iterator<Item = Value> + Send>;

#[path = "subset_cursor.rs"]
mod subset_cursor;
pub use subset_cursor::{ConcreteSubsetStream, SubsetCommit, SubsetScratch};

/// Scalar-component eligibility: a component that needs no heap work-list.
fn concrete_component(value: &Value, order: ValueOrder) -> bool {
    let Value::Set(set) = value else {
        return false;
    };
    scalar_set(set, order)
}

fn scalar_set(set: &Set, order: ValueOrder) -> bool {
    if set.order() != order {
        return false;
    }
    match set.repr() {
        SetRepr::Interval(..) => true,
        SetRepr::Enum | SetRepr::Raw(_) => set.stored_elements().is_some_and(|values| {
            values.iter().all(|value| {
                matches!(
                    value,
                    Value::Bool(_)
                        | Value::Int(_)
                        | Value::BigInt(_)
                        | Value::Str(_)
                        | Value::Model(_)
                )
            })
        }),
        _ => false,
    }
}

fn scalar_compound(set: &Set, order: ValueOrder) -> bool {
    if set.order() != order {
        return false;
    }
    match set.repr() {
        SetRepr::Subset(base) => concrete_component(base, order),
        SetRepr::FcnSet(domain, range) => {
            concrete_component(domain, order) && concrete_component(range, order)
        }
        SetRepr::Product(values) => values.iter().all(|value| concrete_component(value, order)),
        SetRepr::RcdSet(fields) => fields
            .iter()
            .all(|(_, value)| concrete_component(value, order)),
        SetRepr::Enum | SetRepr::Raw(_) | SetRepr::Interval(..) => scalar_set(set, order),
        _ => false,
    }
}

/// Read-only inspection of concrete storage, never provider metadata or values.
/// A failed optional work-list reservation declines before enumeration effects.
enum Inspect<'a> {
    Set(&'a Set),
    Value(&'a Value),
}

fn inspect_push<'a>(pending: &mut Vec<Inspect<'a>>, value: Inspect<'a>) -> bool {
    if pending.try_reserve(1).is_err() {
        return false;
    }
    pending.push(value);
    true
}

fn concrete_set(set: &Set, order: ValueOrder) -> bool {
    if scalar_compound(set, order) {
        return true;
    }
    let mut pending = Vec::new();
    if !inspect_push(&mut pending, Inspect::Set(set)) {
        return false;
    }
    while let Some(item) = pending.pop() {
        let valid = match item {
            Inspect::Set(set) => inspect_set(set, order, &mut pending),
            Inspect::Value(value) => inspect_value(value, order, &mut pending),
        };
        if !valid {
            return false;
        }
    }
    true
}

/// Preflight concrete output and component sizes before identity materialization.
/// Checking every component also bounds singleton function sets whose domain
/// would otherwise require an enormous function despite having only one member.
pub(crate) fn bounded_concrete(set: &Set, ctx: Ctx, limit: u64) -> bool {
    if !concrete_set(set, ctx.order) {
        return false;
    }
    let mut pending = Vec::new();
    let mut sizes = Vec::new();
    if !inspect_push(&mut pending, Inspect::Set(set)) {
        return false;
    }
    while let Some(item) = pending.pop() {
        let valid = match item {
            Inspect::Set(set) => {
                inspect_push(&mut sizes, Inspect::Set(set))
                    && inspect_set(set, ctx.order, &mut pending)
            }
            Inspect::Value(value) => inspect_value(value, ctx.order, &mut pending),
        };
        if !valid {
            return false;
        }
    }
    // A parent's size can normalize raw descendants. Reverse the structural
    // walk so every descendant bound is established before that demand.
    sizes.into_iter().rev().all(|item| match item {
        Inspect::Set(set) => set.size(ctx).is_ok_and(|size| size <= limit),
        Inspect::Value(_) => false,
    })
}

fn inspect_value<'a>(value: &'a Value, order: ValueOrder, pending: &mut Vec<Inspect<'a>>) -> bool {
    match value {
        Value::Set(set) => inspect_push(pending, Inspect::Set(set)),
        Value::Fcn(function) => {
            if function.kind() != crate::FcnKind::Tuple && function.order() != order {
                return false;
            }
            match &function.0.repr {
                FcnRepr::Tuple(values) => values
                    .iter()
                    .all(|v| inspect_push(pending, Inspect::Value(v))),
                FcnRepr::Record(fields) => fields
                    .iter()
                    .all(|(_, v)| inspect_push(pending, Inspect::Value(v))),
                FcnRepr::General(function) => function
                    .domain
                    .keys
                    .iter()
                    .chain(function.values.iter())
                    .all(|v| inspect_push(pending, Inspect::Value(v))),
            }
        }
        Value::Bool(_) | Value::Int(_) | Value::BigInt(_) | Value::Str(_) | Value::Model(_) => true,
    }
}

fn inspect_component<'a>(value: &'a Value, pending: &mut Vec<Inspect<'a>>) -> bool {
    match value {
        Value::Set(set) => inspect_push(pending, Inspect::Set(set)),
        _ => false,
    }
}

fn inspect_set<'a>(set: &'a Set, order: ValueOrder, pending: &mut Vec<Inspect<'a>>) -> bool {
    if set.order() != order {
        return false;
    }
    match set.repr() {
        SetRepr::Interval(..) => true,
        SetRepr::Enum | SetRepr::Raw(_) => set.stored_elements().is_some_and(|values| {
            values
                .iter()
                .all(|v| inspect_push(pending, Inspect::Value(v)))
        }),
        SetRepr::Subset(base) => inspect_component(base, pending),
        SetRepr::FcnSet(domain, range) => {
            inspect_component(domain, pending) && inspect_component(range, pending)
        }
        SetRepr::Product(values) => values.iter().all(|value| inspect_component(value, pending)),
        SetRepr::RcdSet(fields) => fields
            .iter()
            .all(|(_, value)| inspect_component(value, pending)),
        _ => false,
    }
}

/// TLC Value.isEmpty for already-checked concrete components. In particular,
/// Raw does not normalize, whereas Interval calls size and can overflow.
fn concrete_empty(value: &Value, ctx: Ctx) -> Result<bool> {
    let Value::Set(set) = value else {
        return Err(not_enumerable(value, ctx));
    };
    match set.repr() {
        SetRepr::Raw(values) => Ok(values.is_empty()),
        SetRepr::Enum => Ok(set.elements(ctx)?.is_empty()),
        SetRepr::Interval(..) => Ok(crate::enumerate::size(set, ctx)? == 0),
        SetRepr::Subset(_) | SetRepr::FcnSet(..) | SetRepr::Product(_) | SetRepr::RcdSet(_) => {
            compound_empty(set, ctx)
        }
        _ => Err(not_enumerable(value, ctx)),
    }
}

// The newly eligible compounds follow TLC isEmpty without recursively walking
// accumulated product/function storage. Existing scalar cases stay above.
enum EmptyTask<'a> {
    Set(&'a Set),
    Range(&'a Value),
    Columns {
        set: &'a Set,
        next: usize,
        any: bool,
    },
}

fn empty_push<'a>(tasks: &mut Vec<EmptyTask<'a>>, task: EmptyTask<'a>) -> Result<()> {
    tasks.try_reserve(1).map_err(|error| {
        ValueError::new(
            ErrorKind::DiveLimit,
            format!("concrete emptiness frames: {error}"),
        )
    })?;
    tasks.push(task);
    Ok(())
}

fn compound_empty(set: &Set, ctx: Ctx) -> Result<bool> {
    let mut tasks = Vec::new();
    empty_push(&mut tasks, EmptyTask::Set(set))?;
    let mut empty = false;
    while let Some(task) = tasks.pop() {
        match task {
            EmptyTask::Set(set) => match set.repr() {
                SetRepr::Subset(_) => empty = false,
                SetRepr::FcnSet(domain, range) => {
                    empty_push(&mut tasks, EmptyTask::Range(range))?;
                    let Value::Set(domain) = domain else {
                        return Err(not_enumerable(domain, ctx));
                    };
                    empty_push(&mut tasks, EmptyTask::Set(domain))?;
                }
                SetRepr::Product(_) | SetRepr::RcdSet(_) => {
                    empty_push(
                        &mut tasks,
                        EmptyTask::Columns {
                            set,
                            next: 0,
                            any: false,
                        },
                    )?;
                }
                SetRepr::Raw(values) => empty = values.is_empty(),
                SetRepr::Enum => empty = set.elements(ctx)?.is_empty(),
                // In compound TLC isEmpty, unlike the original direct scalar
                // helper, an interval compares its endpoints without size().
                SetRepr::Interval(lo, hi) => empty = hi < lo,
                _ => return Err(not_enumerable(&Value::Set(set.clone()), ctx)),
            },
            EmptyTask::Range(range) => {
                if empty {
                    // An empty function domain makes the function set nonempty
                    // and skips the range, exactly as TLC isEmpty does.
                    empty = false;
                } else {
                    let Value::Set(range) = range else {
                        return Err(not_enumerable(range, ctx));
                    };
                    empty_push(&mut tasks, EmptyTask::Set(range))?;
                }
            }
            EmptyTask::Columns { set, next, mut any } => {
                if next != 0 {
                    any |= empty;
                }
                let value = match set.repr() {
                    SetRepr::Product(values) => values.get(next),
                    SetRepr::RcdSet(fields) => fields.get(next).map(|(_, value)| value),
                    _ => None,
                };
                if let Some(value) = value {
                    // All columns are visited even after an empty column.
                    empty_push(
                        &mut tasks,
                        EmptyTask::Columns {
                            set,
                            next: next + 1,
                            any,
                        },
                    )?;
                    let Value::Set(value) = value else {
                        return Err(not_enumerable(value, ctx));
                    };
                    empty_push(&mut tasks, EmptyTask::Set(value))?;
                } else {
                    empty = any;
                }
            }
        }
    }
    Ok(empty)
}

fn concrete_product_empty<'a>(values: impl Iterator<Item = &'a Value>, ctx: Ctx) -> Result<bool> {
    for value in values {
        if concrete_empty(value, ctx)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn concrete_functions(domain: &Value, range: &Value, ctx: Ctx) -> Result<Elems> {
    // SetOfFcnsValue.elements checks emptiness before normalizing its domain.
    // An empty domain skips the range entirely and yields a FcnRcdValue, not a
    // TupleValue. A nonempty domain with empty range yields no functions.
    if concrete_empty(domain, ctx)? {
        let _normalized_domain = component(domain, ctx, true)?;
        return Ok(Box::new(std::iter::once(Value::tuple_fcn(Vec::new()))));
    }
    if concrete_empty(range, ctx)? {
        return Ok(Box::new(std::iter::empty()));
    }
    let domain = component(domain, ctx, true)?;
    let range = component(range, ctx, true)?;
    let shape = fcn_shape(&domain);
    Ok(odometer(
        vec![range; domain.len()],
        Build::Fcn(domain, shape, ctx.order),
    ))
}

/// Raw output storage for recursively concrete finite components. This is separate
/// from `stream()`, whose normalized bulk callers retain their original checks.
pub(crate) fn concrete_raw(set: &Set, ctx: Ctx) -> Result<Option<Elems>> {
    if !concrete_set(set, set.order()) {
        return Ok(None);
    }
    let ctx = Ctx {
        order: set.order(),
        ..ctx
    };
    let output = match set.repr() {
        SetRepr::Enum | SetRepr::Raw(_) => slice_stream(set.elements(ctx)?),
        SetRepr::Interval(lo, hi) => Box::new(IntervalIter {
            next: lo.clone(),
            hi: hi.clone(),
        }),
        SetRepr::Subset(base) => Box::new(prepare_subset(base, ctx)?) as Elems,
        SetRepr::FcnSet(domain, range) => concrete_functions(domain, range, ctx)?,
        SetRepr::Product(values) => {
            if concrete_product_empty(values.iter(), ctx)? {
                return Ok(Some(Box::new(std::iter::empty())));
            }
            let columns = values
                .iter()
                .map(|value| component(value, ctx, true))
                .collect::<Result<Vec<_>>>()?;
            odometer(columns, Build::Tuple)
        }
        SetRepr::RcdSet(fields) => {
            if concrete_product_empty(fields.iter().map(|(_, value)| value), ctx)? {
                return Ok(Some(Box::new(std::iter::empty())));
            }
            let columns = fields
                .iter()
                .map(|(_, value)| component(value, ctx, true))
                .collect::<Result<Vec<_>>>()?;
            let names = fields.iter().map(|(name, _)| name.clone()).collect();
            odometer(columns, Build::Record(names, ctx.order))
        }
        _ => return Ok(None),
    };
    Ok(Some(output))
}

fn prepare_subset(base: &Value, ctx: Ctx) -> Result<SubsetIter> {
    let Value::Set(component_set) = base else {
        return Err(not_enumerable(base, ctx));
    };
    // SubsetValue.elementsNormalized asks BASE.size before BASE's
    // normalized enumeration, never for this outer powerset's size.
    let base = if component_set.size(ctx)? == 0 {
        // TLC returns its empty enumeration before BASE.toSetEnum:
        // even a dead mixed column must not be normalized here.
        Arc::from([])
    } else {
        component(base, ctx, true)?
    };
    Ok(SubsetIter {
        base,
        order: ctx.order,
        k: 0,
        idx: Vec::new(),
        started: false,
    })
}

pub(crate) fn concrete_subset(set: &Set, ctx: Ctx) -> Result<Option<ConcreteSubsetStream>> {
    let SetRepr::Subset(base) = set.repr() else {
        return Ok(None);
    };
    if !concrete_set(set, set.order()) {
        return Ok(None);
    }
    prepare_subset(
        base,
        Ctx {
            order: set.order(),
            ..ctx
        },
    )
    .map(|inner| Some(ConcreteSubsetStream::new(inner, set.clone())))
}

pub(crate) fn not_enumerable(what: &Value, ctx: Ctx) -> ValueError {
    ValueError::new(
        ErrorKind::NotEnumerable,
        format!(
            "Attempted to enumerate the infinite or non-enumerable set\n{}",
            crate::print::ppr(&crate::print::to_tlc_string_lenient(what, ctx))
        ),
    )
}

/// The elements of a set-valued component, sorted in `order`. `mark` records the
/// normalization of an unnormalized component, as TLC's enumeration does.
pub(crate) fn component(v: &Value, ctx: Ctx, mark: bool) -> Result<Arc<[Value]>> {
    match v {
        Value::Set(s) => elems_in(s, ctx, mark),
        _ => Err(ValueError::new(
            ErrorKind::NotEnumerable,
            format!(
                "Attempted to enumerate the non-set value\n{}",
                crate::print::ppr(&crate::print::to_tlc_string_lenient(v, ctx))
            ),
        )),
    }
}

/// A set's elements sorted in `ctx.order` (re-sorted if the set was built in another
/// order).
pub(crate) fn elems_in(s: &Set, ctx: Ctx, mark: bool) -> Result<Arc<[Value]>> {
    let e = if mark {
        s.elements(ctx)?
    } else {
        s.quiet_elements(ctx)?
    };
    if s.order() == ctx.order {
        return Ok(e);
    }
    let mut v = e.to_vec();
    v.sort_by(|a, b| a.cmp_with(b, ctx.order));
    Ok(v.into())
}

struct SliceIter {
    elems: Arc<[Value]>,
    next: usize,
}

impl Iterator for SliceIter {
    type Item = Value;
    fn next(&mut self) -> Option<Value> {
        let v = self.elems.get(self.next).cloned();
        self.next += 1;
        v
    }
}

pub(crate) fn slice_stream(elems: Arc<[Value]>) -> Elems {
    Box::new(SliceIter { elems, next: 0 })
}

struct IntervalIter {
    next: Int,
    hi: Int,
}

impl Iterator for IntervalIter {
    type Item = Value;
    fn next(&mut self) -> Option<Value> {
        if self.next > self.hi {
            return None;
        }
        let v = Value::int(self.next.clone());
        self.next = match &self.next {
            Int::Small(i) if *i < i64::MAX => Int::Small(i + 1),
            other => other.add(&Int::ONE, IntMode::Big).ok()?,
        };
        Some(v)
    }
}

struct SubsetIter {
    base: Arc<[Value]>,
    order: ValueOrder,
    k: usize,
    idx: Vec<usize>,
    started: bool,
}

impl Iterator for SubsetIter {
    type Item = Value;
    fn next(&mut self) -> Option<Value> {
        let m = self.base.len();
        if self.started {
            // Advance the rightmost index that can still move right, or go to k + 1.
            let k = self.k;
            if let Some(pos) = (0..k).rev().find(|&i| self.idx[i] < m - k + i) {
                self.idx[pos] += 1;
                for j in pos + 1..k {
                    self.idx[j] = self.idx[j - 1] + 1;
                }
            } else {
                if self.k >= m {
                    return None;
                }
                self.k += 1;
                self.idx = (0..self.k).collect();
            }
        }
        self.started = true;
        Some(self.current_value())
    }
}

impl SubsetIter {
    fn current_value(&self) -> Value {
        let elems = self.idx.iter().map(|&i| self.base[i].clone()).collect();
        Value::Set(Set::from_sorted(self.order, elems))
    }
}

/// How an odometer turns a combination of component elements into an element.
#[derive(Clone)]
enum Build {
    Tuple,
    /// A record literal with these fields (from `[a : S]`).
    Record(Arc<[Atom]>, ValueOrder),
    /// A function with this domain (from `[S -> T]`), canonical by domain shape.
    Fcn(Arc<[Value]>, FcnShape, ValueOrder),
}

#[derive(Clone, Copy)]
enum FcnShape {
    Tuple,
    Record,
    General,
}

struct Odometer {
    comps: Vec<Arc<[Value]>>,
    idx: Vec<usize>,
    done: bool,
    build: Build,
}

impl Iterator for Odometer {
    type Item = Value;
    fn next(&mut self) -> Option<Value> {
        if self.done {
            return None;
        }
        let vals: Vec<Value> = self
            .comps
            .iter()
            .zip(&self.idx)
            .map(|(c, &i)| c[i].clone())
            .collect();
        // Advance, last position fastest.
        let mut pos = self.comps.len();
        loop {
            if pos == 0 {
                self.done = true;
                break;
            }
            pos -= 1;
            self.idx[pos] += 1;
            if self.idx[pos] < self.comps[pos].len() {
                break;
            }
            self.idx[pos] = 0;
        }
        Some(match &self.build {
            Build::Tuple => Value::tuple(vals),
            Build::Record(names, order) => Value::Fcn(Fcn::record(
                names.iter().cloned().zip(vals).collect(),
                *order,
                true,
            )),
            Build::Fcn(dom, shape, order) => match shape {
                FcnShape::Tuple => Value::tuple_fcn(vals),
                FcnShape::Record => Value::Fcn(Fcn::record(
                    dom.iter()
                        .filter_map(|d| match d {
                            Value::Str(a) => Some(a.clone()),
                            _ => None,
                        })
                        .zip(vals)
                        .collect(),
                    *order,
                    false,
                )),
                FcnShape::General => Value::Fcn(Fcn::general(
                    dom.iter().cloned().zip(vals).collect(),
                    *order,
                )),
            },
        })
    }
}

fn odometer(comps: Vec<Arc<[Value]>>, build: Build) -> Elems {
    let done = comps.iter().any(|c| c.is_empty());
    let idx = vec![0; comps.len()];
    Box::new(Odometer {
        comps,
        idx,
        done,
        build,
    })
}

fn fcn_shape(dom: &[Value]) -> FcnShape {
    let tuple = dom
        .iter()
        .enumerate()
        .all(|(i, d)| matches!(d, Value::Int(k) if i64::try_from(i).is_ok_and(|i| *k == i + 1)));
    if tuple {
        FcnShape::Tuple
    } else if dom.iter().all(|d| matches!(d, Value::Str(_))) {
        FcnShape::Record
    } else {
        FcnShape::General
    }
}

/// Streams the elements of `s` in normalized order (under `s`'s own order).
///
/// `mark` selects TLC's view: with `mark`, unnormalized sets met on the way count as
/// normalized afterwards (TLC's enumeration normalizes them); without it, nothing
/// observable changes.
pub(crate) fn stream(s: &Set, ctx: Ctx, mark: bool) -> Result<Elems> {
    let ctx = Ctx {
        order: s.order(),
        ..ctx
    };
    let me = || Value::Set(s.clone());
    Ok(match s.repr() {
        SetRepr::Enum | SetRepr::Raw(_) | SetRepr::Deferred(_) => {
            let e = if mark {
                s.elements(ctx)?
            } else {
                s.quiet_elements(ctx)?
            };
            slice_stream(e)
        }
        SetRepr::Interval(lo, hi) => Box::new(IntervalIter {
            next: lo.clone(),
            hi: hi.clone(),
        }),
        SetRepr::Subset(b) => Box::new(SubsetIter {
            base: component(b, ctx, mark)?,
            order: ctx.order,
            k: 0,
            idx: Vec::new(),
            started: false,
        }),
        SetRepr::FcnSet(d, r) => {
            // SetOfFcnsValue.elements tests emptiness before enumerating the
            // domain. Keep quiet operations from normalizing nested operands.
            let _quiet = (!mark).then(crate::value::quiet::Quiet::enter);
            if crate::tlc::is_empty(&me(), ctx)? {
                return Ok(Box::new(std::iter::empty()));
            }
            let dom = component(d, ctx, mark)?;
            if dom.is_empty() {
                return Ok(Box::new(std::iter::once(Value::tuple(Vec::new()))));
            }
            let rng = component(r, ctx, mark)?;
            let shape = fcn_shape(&dom);
            odometer(vec![rng; dom.len()], Build::Fcn(dom, shape, ctx.order))
        }
        SetRepr::RcdSet(fs) => {
            let mut comps = Vec::with_capacity(fs.len());
            for (_, v) in fs {
                comps.push(component(v, ctx, mark)?);
            }
            let names: Arc<[Atom]> = fs.iter().map(|(a, _)| a.clone()).collect();
            odometer(comps, Build::Record(names, ctx.order))
        }
        SetRepr::Product(fs) => {
            let mut comps = Vec::with_capacity(fs.len());
            for v in fs {
                comps.push(component(v, ctx, mark)?);
            }
            odometer(comps, Build::Tuple)
        }
        SetRepr::Seq(r) => {
            if component(r, ctx, mark).is_ok_and(|e| e.is_empty()) {
                Box::new(std::iter::once(Value::tuple(Vec::new())))
            } else {
                return Err(not_enumerable(&me(), ctx));
            }
        }
        SetRepr::Nat | SetRepr::Int | SetRepr::Strings => return Err(not_enumerable(&me(), ctx)),
    })
}

#[cfg(test)]
#[path = "stream_concrete_tests.rs"]
mod concrete_tests;
