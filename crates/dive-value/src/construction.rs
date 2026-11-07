//! Cooperative construction of closed, already materialized values.
//!
//! These helpers are used at the core's graph-value boundary. Symbolic values
//! retain the existing conversion path; they are not certified by this module.

use std::borrow::Cow;
use std::cmp::Ordering;

use crate::value::{Fcn, FcnRepr, Set, SetRepr};
use crate::{Atom, Ctx, ErrorKind, Value, ValueError, ValueOrder};

/// Failure while constructing a materialized graph value.
#[derive(Debug)]
pub enum ConstructionError {
    /// The same value error as an uncontrolled constructor.
    Value(ValueError),
    /// The caller's cooperative checkpoint requested interruption.
    Interrupted,
    /// Temporary construction storage could not be reserved.
    Allocation,
}

impl From<ValueError> for ConstructionError {
    fn from(error: ValueError) -> Self {
        Self::Value(error)
    }
}

impl std::fmt::Display for ConstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Value(error) => error.fmt(f),
            Self::Interrupted => f.write_str("graph value construction interrupted"),
            Self::Allocation => f.write_str("graph value construction storage unavailable"),
        }
    }
}

impl std::error::Error for ConstructionError {}

type Result<T> = std::result::Result<T, ConstructionError>;
type Poll<'a> = dyn FnMut() -> bool + 'a;

pub(crate) fn checkpoint(poll: &mut Poll<'_>) -> Result<()> {
    if poll() {
        Ok(())
    } else {
        Err(ConstructionError::Interrupted)
    }
}

pub(crate) fn reserve<T>(out: &mut Vec<T>, additional: usize, poll: &mut Poll<'_>) -> Result<()> {
    checkpoint(poll)?;
    out.try_reserve(additional)
        .map_err(|_| ConstructionError::Allocation)
}

/// Stable merge sort with fallible scratch allocation and comparison.
fn sort<T: Clone>(
    items: &mut [T],
    poll: &mut Poll<'_>,
    compare: &mut dyn FnMut(&T, &T, &mut Poll<'_>) -> Result<Ordering>,
) -> Result<()> {
    checkpoint(poll)?;
    if items.len() < 2 {
        return Ok(());
    }
    let mut sorted = true;
    for pair in items.windows(2) {
        checkpoint(poll)?;
        if compare(&pair[0], &pair[1], poll)? == Ordering::Greater {
            sorted = false;
            break;
        }
    }
    if sorted {
        return Ok(());
    }
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(items.len())
        .map_err(|_| ConstructionError::Allocation)?;
    let mut width = 1;
    while width < items.len() {
        scratch.clear();
        for start in (0..items.len()).step_by(width.saturating_mul(2)) {
            let mid = start.saturating_add(width).min(items.len());
            let end = mid.saturating_add(width).min(items.len());
            let (mut left, mut right) = (start, mid);
            while left < mid || right < end {
                checkpoint(poll)?;
                let take_left = right == end
                    || left < mid
                        && compare(&items[left], &items[right], poll)? != Ordering::Greater;
                let at = if take_left {
                    let at = left;
                    left += 1;
                    at
                } else {
                    let at = right;
                    right += 1;
                    at
                };
                scratch.push(items[at].clone());
            }
        }
        for (destination, source) in items.iter_mut().zip(&scratch) {
            checkpoint(poll)?;
            destination.clone_from(source);
        }
        width = width.saturating_mul(2);
    }
    Ok(())
}

fn domain(f: &Fcn, at: usize) -> Value {
    match &f.0.repr {
        FcnRepr::Tuple(_) => Value::Int(i64::try_from(at).unwrap_or(i64::MAX).saturating_add(1)),
        FcnRepr::Record(fields) => Value::Str(fields[at].0.clone()),
        FcnRepr::General(pairs) => pairs.domain.keys[at].clone(),
    }
}

fn image(f: &Fcn, at: usize) -> &Value {
    match &f.0.repr {
        FcnRepr::Tuple(values) => &values[at],
        FcnRepr::Record(fields) => &fields[at].1,
        FcnRepr::General(pairs) => &pairs.values[at],
    }
}

fn explicit(s: &Set) -> Option<&[Value]> {
    if matches!(s.repr(), SetRepr::Enum) {
        s.stored_elements()
    } else {
        None
    }
}

/// Whether every node is explicit and uses this construction's name order.
/// The walk itself is cooperative and never materializes a symbolic value.
pub(crate) fn closed(value: &Value, ctx: Ctx, poll: &mut Poll<'_>) -> Result<bool> {
    explicit_tree(value, ctx, poll, true)
}

/// Raw explicit storage has exact finite children even before normalization.
/// The strict mode is retained for fingerprinting and controlled constructors.
fn explicit_tree(value: &Value, ctx: Ctx, poll: &mut Poll<'_>, normalized: bool) -> Result<bool> {
    if !matches!(value, Value::Set(_) | Value::Fcn(_)) {
        checkpoint(poll)?;
        return Ok(true);
    }
    if !needs_walk(value) {
        checkpoint(poll)?;
        return Ok(true);
    }
    let mut work = Vec::new();
    reserve(&mut work, 1, poll)?;
    work.push(value);
    while let Some(value) = work.pop() {
        checkpoint(poll)?;
        match value {
            Value::Set(set) => {
                let elements = match set.repr() {
                    SetRepr::Enum => set.stored_elements(),
                    SetRepr::Raw(elements) if !normalized => Some(elements.as_ref()),
                    _ => None,
                };
                let Some(elements) = elements else {
                    return Ok(false);
                };
                if normalized && set.order() != ctx.order {
                    return Ok(false);
                }
                reserve(&mut work, elements.len(), poll)?;
                work.extend(elements.iter().filter(|v| needs_walk(v)));
            }
            Value::Fcn(f) => {
                if normalized && !matches!(f.0.repr, FcnRepr::Tuple(_)) && f.order() != ctx.order {
                    return Ok(false);
                }
                reserve(&mut work, f.len().saturating_mul(2), poll)?;
                match &f.0.repr {
                    FcnRepr::Tuple(values) => work.extend(values.iter().filter(|v| needs_walk(v))),
                    FcnRepr::Record(fields) => work.extend(
                        fields
                            .iter()
                            .map(|(_, value)| value)
                            .filter(|v| needs_walk(v)),
                    ),
                    FcnRepr::General(pairs) => {
                        work.extend(pairs.domain.keys.iter().filter(|v| needs_walk(v)));
                        work.extend(pairs.values.iter().filter(|v| needs_walk(v)));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(true)
}

/// Whether [`explicit_tree`] must look inside `value`: a scalar, or a tuple
/// of scalars (the common element of a set of sequences), is explicit
/// whatever the name order.
fn needs_walk(value: &Value) -> bool {
    match value {
        Value::Set(_) => true,
        Value::Fcn(f) => match &f.0.repr {
            FcnRepr::Tuple(values) => values
                .iter()
                .any(|v| matches!(v, Value::Set(_) | Value::Fcn(_))),
            _ => true,
        },
        _ => false,
    }
}

fn ordered_elements<'a>(set: &'a Set, ctx: Ctx, poll: &mut Poll<'_>) -> Result<Cow<'a, [Value]>> {
    let Some(elements) = explicit(set) else {
        return Err(not_closed());
    };
    checkpoint(poll)?;
    if set.order() == ctx.order {
        return Ok(Cow::Borrowed(elements));
    }
    let mut out = Vec::new();
    reserve(&mut out, elements.len(), poll)?;
    for value in elements {
        checkpoint(poll)?;
        out.push(value.clone());
    }
    sort(&mut out, poll, &mut |a, b, poll| total(a, b, ctx, poll))?;
    Ok(Cow::Owned(out))
}

enum Pairs<'a> {
    Stored(&'a Fcn),
    Sorted(Vec<(Value, Value)>),
}
impl Pairs<'_> {
    fn key(&self, at: usize) -> Value {
        match self {
            Self::Stored(f) => domain(f, at),
            Self::Sorted(pairs) => pairs[at].0.clone(),
        }
    }
    fn value(&self, at: usize) -> &Value {
        match self {
            Self::Stored(f) => image(f, at),
            Self::Sorted(pairs) => &pairs[at].1,
        }
    }
}

fn ordered_pairs<'a>(f: &'a Fcn, ctx: Ctx, poll: &mut Poll<'_>) -> Result<Pairs<'a>> {
    checkpoint(poll)?;
    if matches!(f.0.repr, FcnRepr::Tuple(_)) || f.order() == ctx.order {
        return Ok(Pairs::Stored(f));
    }
    let mut out = Vec::new();
    reserve(&mut out, f.len(), poll)?;
    for at in 0..f.len() {
        checkpoint(poll)?;
        out.push((domain(f, at), image(f, at).clone()));
    }
    sort(&mut out, poll, &mut |a, b, poll| {
        total(&a.0, &b.0, ctx, poll)
    })?;
    Ok(Pairs::Sorted(out))
}

fn total(a: &Value, b: &Value, ctx: Ctx, poll: &mut Poll<'_>) -> Result<Ordering> {
    checkpoint(poll)?;
    if let Some(comparison) = crate::order::scalar_cmp(a, b, ctx.order) {
        return Ok(comparison);
    }
    let kinds = crate::order::rank(a).cmp(&crate::order::rank(b));
    if kinds != Ordering::Equal {
        return Ok(kinds);
    }
    let _guard = crate::depth::Guard::enter()?;
    match (a, b) {
        (Value::Set(s), Value::Set(t)) => {
            let (s, t) = (
                ordered_elements(s, ctx, poll)?,
                ordered_elements(t, ctx, poll)?,
            );
            if s.len() != t.len() {
                return Ok(s.len().cmp(&t.len()));
            }
            for (a, b) in s.iter().zip(t.iter()) {
                let comparison = total(a, b, ctx, poll)?;
                if comparison != Ordering::Equal {
                    return Ok(comparison);
                }
            }
        }
        (Value::Fcn(f), Value::Fcn(g)) => {
            if f.len() != g.len() {
                return Ok(f.len().cmp(&g.len()));
            }
            // Two tuples of one length have the same domain, `1..n`: only
            // their components, in order, decide.
            if let (FcnRepr::Tuple(xs), FcnRepr::Tuple(ys)) = (&f.0.repr, &g.0.repr) {
                for (x, y) in xs.iter().zip(ys.iter()) {
                    let comparison = match crate::order::scalar_cmp(x, y, ctx.order) {
                        Some(comparison) => comparison,
                        None => total(x, y, ctx, poll)?,
                    };
                    if comparison != Ordering::Equal {
                        return Ok(comparison);
                    }
                }
                return Ok(Ordering::Equal);
            }
            let (first, second) = (ordered_pairs(f, ctx, poll)?, ordered_pairs(g, ctx, poll)?);
            for at in 0..f.len() {
                let comparison = total(&first.key(at), &second.key(at), ctx, poll)?;
                if comparison != Ordering::Equal {
                    return Ok(comparison);
                }
            }
            for at in 0..f.len() {
                let comparison = total(first.value(at), second.value(at), ctx, poll)?;
                if comparison != Ordering::Equal {
                    return Ok(comparison);
                }
            }
        }
        _ => {}
    }
    Ok(Ordering::Equal)
}

fn not_closed() -> ConstructionError {
    ValueError::new(
        ErrorKind::NotEnumerable,
        "controlled construction requires closed materialized values",
    )
    .into()
}

fn checked_compare(a: &Value, b: &Value, ctx: Ctx, poll: &mut Poll<'_>) -> Result<Ordering> {
    checkpoint(poll)?;
    match (a, b) {
        (Value::Set(s), Value::Set(t)) => {
            let _guard = crate::depth::Guard::enter()?;
            let (s, t) = (
                ordered_elements(s, ctx, poll)?,
                ordered_elements(t, ctx, poll)?,
            );
            if s.len() != t.len() {
                return Ok(s.len().cmp(&t.len()));
            }
            for (a, b) in s.iter().zip(t.iter()) {
                let comparison = checked_compare(a, b, ctx, poll)?;
                if comparison != Ordering::Equal {
                    return Ok(comparison);
                }
            }
            Ok(Ordering::Equal)
        }
        (Value::Fcn(f), Value::Fcn(g)) => {
            let _guard = crate::depth::Guard::enter()?;
            // A literal record compares only with another string-domain
            // function (or the empty tuple), exactly as TLC's toRcd does.
            if f.is_record_literal() && !(matches!(g.0.repr, FcnRepr::Record(_)) || g.is_empty()) {
                return crate::tlc::compare(a, b, ctx).map_err(Into::into);
            }
            if f.len() != g.len() {
                return Ok(f.len().cmp(&g.len()));
            }
            let (first, second) = (ordered_pairs(f, ctx, poll)?, ordered_pairs(g, ctx, poll)?);
            for at in 0..f.len() {
                let (a, b) = (first.key(at), second.key(at));
                // Tuple/general-function comparisons reject a noninteger
                // general domain before invoking a scalar compare.
                if matches!(f.0.repr, FcnRepr::Tuple(_))
                    && !matches!(g.0.repr, FcnRepr::Tuple(_))
                    && b.as_int().is_none()
                {
                    return Err(crate::tlc::non_int_domain(&b, ctx, false).into());
                }
                if !matches!(f.0.repr, FcnRepr::Tuple(_))
                    && matches!(g.0.repr, FcnRepr::Tuple(_))
                    && a.as_int().is_none()
                {
                    return Err(crate::tlc::non_int_domain(&a, ctx, false).into());
                }
                let comparison = checked_compare(&a, &b, ctx, poll)?;
                if comparison != Ordering::Equal {
                    return Ok(comparison);
                }
            }
            for at in 0..f.len() {
                let comparison = checked_compare(first.value(at), second.value(at), ctx, poll)?;
                if comparison != Ordering::Equal {
                    return Ok(comparison);
                }
            }
            Ok(Ordering::Equal)
        }
        // Scalar comparisons and kind errors have no nested comparison work.
        _ => crate::tlc::compare(a, b, ctx).map_err(Into::into),
    }
}

fn duplicate(value: &Value, ctx: Ctx) -> ConstructionError {
    ValueError::new(
        ErrorKind::Duplicate,
        format!(
            "The value\n{}\noccurs multiple times in the function domain.",
            crate::print::to_tlc_string_lenient(value, ctx)
        ),
    )
    .into()
}

/// Canonical order is mathematical only for the complete materialized tree.
pub(crate) fn canonical_set(
    mut elements: Vec<Value>,
    ctx: Ctx,
    poll: &mut Poll<'_>,
) -> Result<Value> {
    for value in &mut elements {
        // A closed value is its own materialized identity, and closed
        // implies the raw explicit tree `require_closed` checks.
        if closed(value, ctx, poll)? {
            continue;
        }
        *value = materialized_identity(value, ctx, poll, 0)?;
        require_closed(value, ctx, poll)?;
    }
    // Elements already in strictly ascending order (a set enumerated in
    // canonical order, such as a function set's) are sorted and distinct.
    let mut ascending = true;
    for pair in elements.windows(2) {
        if total(&pair[0], &pair[1], ctx, poll)? != Ordering::Less {
            ascending = false;
            break;
        }
    }
    if ascending {
        return Ok(Value::Set(Set::from_sorted(ctx.order, elements)));
    }
    sort(&mut elements, poll, &mut |a, b, poll| {
        total(a, b, ctx, poll)
    })?;
    let mut out = Vec::new();
    reserve(&mut out, elements.len(), poll)?;
    for value in elements {
        if let Some(last) = out.last()
            && total(&value, last, ctx, poll)? == Ordering::Equal
        {
            continue;
        }
        out.push(value);
    }
    Ok(Value::Set(Set::from_sorted(ctx.order, out)))
}

/// Materialize explicit/deferred results, never infer symbolic identity.
pub(crate) fn materialized_identity(
    value: &Value,
    ctx: Ctx,
    poll: &mut Poll<'_>,
    depth: usize,
) -> Result<Value> {
    if closed(value, ctx, poll)? {
        return Ok(value.clone());
    }
    if depth >= 64 {
        return Err(not_materialized());
    }
    match value {
        Value::Set(set) => {
            let elements = match set.repr() {
                SetRepr::Enum => set.stored_elements().unwrap_or_default().to_vec(),
                SetRepr::Raw(elements) => elements.to_vec(),
                SetRepr::Interval(..) => {
                    // Match Core's existing finite materialization ceiling;
                    // large interval domains remain symbolic operators.
                    if !set.size(ctx).is_ok_and(|size| size <= 1 << 24) {
                        return Err(not_materialized());
                    }
                    set.elements(ctx)?.to_vec()
                }
                SetRepr::Subset(_)
                | SetRepr::FcnSet(..)
                | SetRepr::Product(_)
                | SetRepr::RcdSet(_) => {
                    if !crate::stream::bounded_concrete(set, ctx, 1 << 24) {
                        return Err(not_materialized());
                    }
                    set.concrete_element_stream(ctx)?
                        .ok_or_else(not_materialized)?
                        .collect()
                }
                SetRepr::Deferred(source) => source.elements(ctx).map_err(|error| {
                    if error.kind() == ErrorKind::NotEnumerable {
                        not_materialized()
                    } else {
                        error.into()
                    }
                })?,
                _ => return Err(not_materialized()),
            };
            let mut out = Vec::new();
            reserve(&mut out, elements.len(), poll)?;
            for child in elements {
                // `canonical_set` keeps a closed child as it is.
                if closed(&child, ctx, poll)? {
                    out.push(child);
                } else {
                    out.push(materialized_identity(&child, ctx, poll, depth + 1)?);
                }
            }
            canonical_set(out, ctx, poll)
        }
        Value::Fcn(function) => {
            let mut out = Vec::new();
            reserve(&mut out, function.len(), poll)?;
            for (key, value) in function.iter() {
                out.push((
                    materialized_identity(&key, ctx, poll, depth + 1)?,
                    materialized_identity(value, ctx, poll, depth + 1)?,
                ));
            }
            Ok(match &function.0.repr {
                FcnRepr::Tuple(_) if function.is_tuple_literal() => {
                    Value::tuple(out.into_iter().map(|(_, value)| value).collect())
                }
                FcnRepr::Tuple(_) => {
                    Value::tuple_fcn(out.into_iter().map(|(_, value)| value).collect())
                }
                FcnRepr::Record(fields) => Value::Fcn(Fcn::record(
                    crate::value::sort_fields(
                        fields
                            .iter()
                            .zip(out)
                            .map(|((name, _), (_, value))| (name.clone(), value))
                            .collect(),
                        ctx.order,
                    )?,
                    ctx.order,
                    function.is_record_literal(),
                )),
                FcnRepr::General(_) => Value::fcn(out, ctx)?,
            })
        }
        _ => Err(not_materialized()),
    }
}

fn not_materialized() -> ConstructionError {
    ValueError::new(
        ErrorKind::Unsupported,
        "mathematical identity requires materialized values; symbolic or cross-order children are unsupported",
    ).into()
}

pub(crate) fn require_closed(value: &Value, ctx: Ctx, poll: &mut Poll<'_>) -> Result<()> {
    if explicit_tree(value, ctx, poll, false)? {
        Ok(())
    } else {
        Err(not_materialized())
    }
}

pub(crate) fn set(elements: Vec<Value>, ctx: Ctx, poll: &mut Poll<'_>) -> Result<Value> {
    if crate::order::all_comparable_controlled(&elements, poll)? {
        let mut elements = elements;
        sort(&mut elements, poll, &mut |a, b, poll| {
            total(a, b, ctx, poll)
        })?;
        let mut out = Vec::new();
        reserve(&mut out, elements.len(), poll)?;
        for value in elements {
            checkpoint(poll)?;
            if let Some(last) = out.last()
                && total(&value, last, ctx, poll)? == Ordering::Equal
            {
                continue;
            }
            out.push(value);
        }
        return Ok(Value::Set(Set::from_sorted(ctx.order, out)));
    }
    let mut out = Vec::new();
    reserve(&mut out, elements.len(), poll)?;
    for element in elements {
        checkpoint(poll)?;
        if out.is_empty() {
            out.push(element);
            continue;
        }
        let (mut low, mut high) = (0, out.len());
        let mut equal = false;
        while low < high {
            let mid = low + (high - low) / 2;
            match checked_compare(
                &element,
                &out[mid],
                Ctx {
                    order: ValueOrder::Tlc,
                    ..ctx
                },
                poll,
            )? {
                Ordering::Equal => {
                    equal = true;
                    break;
                }
                Ordering::Less => high = mid,
                Ordering::Greater => low = mid + 1,
            }
        }
        if !equal {
            // Shift explicitly to keep interruption checks inside wide moves.
            out.push(element);
            for at in (low..out.len() - 1).rev() {
                checkpoint(poll)?;
                out.swap(at, at + 1);
            }
        }
    }
    if ctx.order != ValueOrder::Tlc {
        sort(&mut out, poll, &mut |a, b, poll| total(a, b, ctx, poll))?;
    }
    checkpoint(poll)?;
    Ok(Value::Set(Set::from_sorted(ctx.order, out)))
}

pub(crate) fn fcn(mut pairs: Vec<(Value, Value)>, ctx: Ctx, poll: &mut Poll<'_>) -> Result<Value> {
    let mut domains = Vec::new();
    reserve(&mut domains, pairs.len(), poll)?;
    for (domain, _) in &pairs {
        checkpoint(poll)?;
        domains.push(domain.clone());
    }
    if crate::order::all_comparable_controlled(&domains, poll)? {
        sort(&mut pairs, poll, &mut |a, b, poll| {
            total(&a.0, &b.0, ctx, poll)
        })?;
        for pair in pairs.windows(2) {
            if total(&pair[0].0, &pair[1].0, ctx, poll)? == Ordering::Equal {
                return Err(duplicate(&pair[1].0, ctx));
            }
        }
    } else {
        let tlc = Ctx {
            order: ValueOrder::Tlc,
            ..ctx
        };
        for at in 1..pairs.len() {
            match checked_compare(&pairs[0].0, &pairs[at].0, tlc, poll)? {
                Ordering::Equal => return Err(duplicate(&pairs[at].0, tlc)),
                Ordering::Greater => pairs.swap(0, at),
                Ordering::Less => {}
            }
        }
        for at in 2..pairs.len() {
            let item = pairs[at].clone();
            let mut to = at;
            let mut comparison = Ordering::Less;
            while to > 0 {
                comparison = checked_compare(&item.0, &pairs[to - 1].0, tlc, poll)?;
                if comparison != Ordering::Less {
                    break;
                }
                pairs[to] = pairs[to - 1].clone();
                to -= 1;
            }
            if comparison == Ordering::Equal {
                return Err(duplicate(&pairs[at].0, tlc));
            }
            pairs[to] = item;
        }
        if ctx.order != ValueOrder::Tlc {
            sort(&mut pairs, poll, &mut |a, b, poll| {
                total(&a.0, &b.0, ctx, poll)
            })?;
        }
    }
    let mut tuple = true;
    let mut record = true;
    for (at, (key, _)) in pairs.iter().enumerate() {
        checkpoint(poll)?;
        tuple &= matches!(key, Value::Int(k) if i64::try_from(at).is_ok_and(|at| *k == at + 1));
        record &= matches!(key, Value::Str(_));
    }
    if tuple {
        let mut values = Vec::new();
        reserve(&mut values, pairs.len(), poll)?;
        for (_, value) in pairs {
            checkpoint(poll)?;
            values.push(value);
        }
        Ok(Value::tuple_fcn(values))
    } else if record {
        let mut fields = Vec::new();
        reserve(&mut fields, pairs.len(), poll)?;
        for (key, value) in pairs {
            checkpoint(poll)?;
            if let Value::Str(name) = key {
                fields.push((name, value));
            }
        }
        Ok(Value::Fcn(Fcn::record(fields, ctx.order, false)))
    } else {
        checkpoint(poll)?;
        Ok(Value::Fcn(Fcn::general(pairs, ctx.order)))
    }
}

pub(crate) fn record(
    mut fields: Vec<(Atom, Value)>,
    ctx: Ctx,
    poll: &mut Poll<'_>,
) -> Result<Value> {
    sort(&mut fields, poll, &mut |a, b, poll| {
        checkpoint(poll)?;
        Ok(a.0.cmp_with(&b.0, ctx.order))
    })?;
    for pair in fields.windows(2) {
        checkpoint(poll)?;
        if pair[0].0 == pair[1].0 {
            return Err(ValueError::new(
                ErrorKind::Duplicate,
                format!("Field name {} occurs multiple times in record.", pair[1].0),
            )
            .into());
        }
    }
    checkpoint(poll)?;
    if fields.is_empty() {
        Ok(Value::tuple(Vec::new()))
    } else {
        Ok(Value::Fcn(Fcn::record(fields, ctx.order, true)))
    }
}

#[cfg(test)]
mod allocation_tests {
    use super::*;

    #[test]
    fn scratch_capacity_failure_is_distinct_from_interruption() {
        let result = reserve(&mut Vec::<u64>::new(), usize::MAX, &mut || true);
        assert!(matches!(result, Err(ConstructionError::Allocation)));
        let result = reserve(&mut Vec::<u64>::new(), usize::MAX, &mut || false);
        assert!(matches!(result, Err(ConstructionError::Interrupted)));
    }
}
