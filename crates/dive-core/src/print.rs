//! Printing core values as TLC prints them (`ToString`, `Print`, `PrintT`).
//!
//! A value is converted to the reference engine's [`Value`] and printed by
//! `dive_value::print`, the port of TLC's `toString`, so that the core and
//! the reference engine print alike. The conversion is the reverse of the
//! binder boundary ([`crate::convert`]).
//!
//! TLC prints some sets by their representation: an interval as `1..3`, a
//! large `SUBSET S`, `[S -> T]`, `[a : S]` or `S \X T` in its lazy form
//! (`dive_value::print`). [`Conversion::Print`] keeps the core's symbolic
//! sets symbolic for that reason. But the core enumerates such a set when
//! it normalizes or interns a value, and `DOMAIN` of a sequence is TLC's
//! interval and the core's enumerated set. So when the plan may build an
//! interval or a lazy set ([`crate::Store::lazy_sets`]), printing refuses
//! an enumerated set that TLC could print otherwise (`ambiguous`).

use std::sync::Arc;

use dive_value::{Ctx, Interner, Value};

use crate::Error;
use crate::handle::Handle;
use crate::heap::Heap;
use crate::node::{Kind, flags};

/// The run's name table and value semantics, as the reference engine
/// prints with them ([`crate::Store::set_printer`]).
#[derive(Clone)]
pub struct Printer {
    /// The model's name table: strings and model values convert to its
    /// atoms, which carry TLC's token order.
    pub names: Arc<Interner>,
    /// The run's value semantics.
    pub ctx: Ctx,
}

impl std::fmt::Debug for Printer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Printer").finish_non_exhaustive()
    }
}

/// What a conversion is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conversion {
    /// Reports and validation: finite symbolic sets are enumerated, as a
    /// stored state holds them.
    Report,
    /// Temporal graph evaluation: preserve function/tuple/record literal
    /// classes recursively. Only closed materialized values are admitted;
    /// symbolic or raw sets need independent representation-history support.
    /// The caller must supply original evaluation representations retained
    /// through import, folding and capture, never canonical identity handles
    /// whose receiver class was erased (ADR 0121).
    Graph,
    /// Printing: symbolic sets stay symbolic, and a stored set TLC could
    /// print otherwise is refused.
    Print,
    /// A random draw's argument: symbolic sets stay symbolic, as TLC draws
    /// from them (`random.rs`).
    Draw,
}

/// Converts a handle to a reference value.
///
/// # Errors
/// A value the reference engine cannot represent, and (for
/// [`Conversion::Print`]) a set whose printed form the core cannot know.
pub fn to_value(
    heap: &mut Heap,
    names: &Interner,
    h: Handle,
    ctx: Ctx,
    conv: Conversion,
) -> Result<Value, String> {
    if let Some(b) = h.as_bool() {
        return Ok(Value::Bool(b));
    }
    if let Some(i) = h.as_int() {
        return Ok(Value::Int(i));
    }
    if let Some(id) = h.as_str_id() {
        let text = heap
            .store()
            .strings()
            .text(id)
            .ok_or_else(|| format!("unknown string {id}"))?;
        return Ok(Value::Str(names.intern(&text)));
    }
    if let Some(id) = h.as_model_id() {
        let text = heap
            .store()
            .strings()
            .text(id)
            .ok_or_else(|| format!("unknown model value {id}"))?;
        return Ok(Value::Model(names.intern(&text)));
    }
    if !h.is_compound() {
        return Err("a value the core cannot report".into());
    }
    if heap.view(h).is_lazy() {
        return Err("printing a function or set the core keeps lazy (M1 core)".into());
    }
    let kind = heap.view(h).kind;
    if conv == Conversion::Graph && kind.is_symbolic() {
        return Err(format!(
            "an unmaterialized symbolic {kind:?} in core graph conversion"
        ));
    }
    if kind == Kind::BigInt {
        let b = heap.big(h).ok_or("a malformed integer")?;
        return Ok(Value::int(dive_value::Int::from_big(b)));
    }
    // An infinite set is the reference engine's lazy set, as TLC keeps it:
    // a state may hold one until it is fingerprinted.
    let keep = conv != Conversion::Report
        && matches!(
            kind,
            Kind::Interval | Kind::Subset | Kind::FcnSet | Kind::RecSet | Kind::Product
        );
    if keep || !heap.is_finite(h) {
        return symbolic_value(heap, names, h, ctx, conv);
    }
    if kind.is_symbolic() {
        let m = heap.materialize(h).map_err(|e| e.message().to_owned())?;
        return to_value(heap, names, m, ctx, conv);
    }
    if conv == Conversion::Print
        && let Some(why) = ambiguous(heap, h)
    {
        return Err(why);
    }
    let v = heap.view(h);
    let (lo, view_flags, keys, vals) = (v.lo, v.flags, v.keys().to_vec(), v.values().to_vec());
    match kind {
        Kind::Set | Kind::SetEnum => {
            let mut elems = Vec::with_capacity(vals.len());
            for e in vals {
                elems.push(to_value(heap, names, e, ctx, conv)?);
            }
            Value::set_canonical(elems, ctx).map_err(|error| error.to_string())
        }
        Kind::IntFcn => {
            let mut elems = Vec::with_capacity(vals.len());
            for e in vals {
                elems.push(to_value(heap, names, e, ctx, conv)?);
            }
            if lo == 1 {
                return Ok(
                    if conv == Conversion::Graph && view_flags & flags::TUPLE == 0 {
                        Value::tuple_fcn(elems)
                    } else {
                        Value::tuple(elems)
                    },
                );
            }
            let pairs = elems
                .into_iter()
                .zip(lo..)
                .map(|(v, k)| (Value::Int(k), v))
                .collect();
            Value::fcn(pairs, ctx).map_err(|e| e.to_string())
        }
        _ => {
            let mut pairs = Vec::with_capacity(keys.len());
            for (k, x) in keys.into_iter().zip(vals) {
                pairs.push((
                    to_value(heap, names, k, ctx, conv)?,
                    to_value(heap, names, x, ctx, conv)?,
                ));
            }
            function_value(pairs, ctx, conv, view_flags)
        }
    }
}

fn function_value(
    pairs: Vec<(Value, Value)>,
    ctx: Ctx,
    conv: Conversion,
    view_flags: u8,
) -> Result<Value, String> {
    let strings = pairs.iter().all(|(k, _)| matches!(k, Value::Str(_)));
    let literal = conv == Conversion::Graph && view_flags & flags::RECORD != 0;
    if literal && !strings {
        return Err("a core graph record literal with non-string fields".into());
    }
    // Report syntax stays unchanged. Evaluation preserves whether a string
    // domain came from a record literal or a general function constructor.
    if strings && (conv == Conversion::Report || literal) {
        let fields = pairs
            .into_iter()
            .filter_map(|(k, x)| match k {
                Value::Str(a) => Some((a, x)),
                _ => None,
            })
            .collect();
        Value::record(fields, ctx).map_err(|e| e.to_string())
    } else {
        Value::fcn(pairs, ctx).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
#[path = "print_tests.rs"]
mod tests;
/// Failure in cooperative graph conversion.
#[derive(Debug)]
pub enum ConversionError {
    /// The persistent value cannot be represented by the reference engine.
    Unsupported(String),
    /// Normalization, scratch storage, or cooperative interruption failed.
    Construction(dive_value::ConstructionError),
}

impl From<dive_value::ConstructionError> for ConversionError {
    fn from(error: dive_value::ConstructionError) -> Self {
        Self::Construction(error)
    }
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(reason) => f.write_str(reason),
            Self::Construction(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for ConversionError {}

/// Converts a graph value with checkpoints in materialized traversal and
/// normalization. `proceed` returns false to interrupt.
///
/// Symbolic values retain the legacy conversion/materialization path, which is
/// not yet cooperative. Final shared allocations and diagnostic rendering also
/// retain their existing behavior. This helper does not establish a memory cap.
///
/// # Errors
/// The existing conversion refusal, normalization error, interruption, or
/// unavailable scratch storage.
#[allow(
    clippy::too_many_lines,
    reason = "preserve conversion and name encounter order in one value-class match"
)]
pub fn to_value_controlled(
    heap: &mut Heap,
    names: &Interner,
    h: Handle,
    ctx: Ctx,
    conv: Conversion,
    proceed: &mut dyn FnMut() -> bool,
) -> Result<Value, ConversionError> {
    fn check(proceed: &mut dyn FnMut() -> bool) -> Result<(), ConversionError> {
        if proceed() {
            Ok(())
        } else {
            Err(dive_value::ConstructionError::Interrupted.into())
        }
    }
    fn storage<T>(
        len: usize,
        proceed: &mut dyn FnMut() -> bool,
    ) -> Result<Vec<T>, ConversionError> {
        check(proceed)?;
        let mut out = Vec::new();
        out.try_reserve_exact(len).map_err(|_| {
            ConversionError::Construction(dive_value::ConstructionError::Allocation)
        })?;
        Ok(out)
    }
    check(proceed)?;
    if !h.is_compound()
        || heap.view(h).is_lazy()
        || heap.view(h).kind.is_symbolic()
        || heap.view(h).kind == Kind::BigInt
    {
        let value = to_value(heap, names, h, ctx, conv).map_err(ConversionError::Unsupported)?;
        check(proceed)?;
        return Ok(value);
    }
    if conv == Conversion::Print
        && let Some(reason) = ambiguous(heap, h)
    {
        return Err(ConversionError::Unsupported(reason));
    }
    let view = heap.view(h);
    let (kind, lo, view_flags) = (view.kind, view.lo, view.flags);
    let mut keys = storage(view.keys().len(), proceed)?;
    for &key in view.keys() {
        check(proceed)?;
        keys.push(key);
    }
    let mut handles = storage(view.values().len(), proceed)?;
    for &value in view.values() {
        check(proceed)?;
        handles.push(value);
    }
    match kind {
        Kind::Set | Kind::SetEnum => {
            let mut elements = storage(handles.len(), proceed)?;
            for value in handles {
                elements.push(to_value_controlled(heap, names, value, ctx, conv, proceed)?);
            }
            Value::set_controlled(elements, ctx, proceed).map_err(Into::into)
        }
        Kind::IntFcn => {
            let mut elements = storage(handles.len(), proceed)?;
            for value in handles {
                elements.push(to_value_controlled(heap, names, value, ctx, conv, proceed)?);
            }
            check(proceed)?;
            if lo == 1 {
                return Ok(
                    if conv == Conversion::Graph && view_flags & flags::TUPLE == 0 {
                        Value::tuple_fcn(elements)
                    } else {
                        Value::tuple(elements)
                    },
                );
            }
            let mut pairs = storage(elements.len(), proceed)?;
            for (value, key) in elements.into_iter().zip(lo..) {
                check(proceed)?;
                pairs.push((Value::Int(key), value));
            }
            Value::fcn_controlled(pairs, ctx, proceed).map_err(Into::into)
        }
        _ => {
            let mut pairs = storage(keys.len(), proceed)?;
            for (key, value) in keys.into_iter().zip(handles) {
                pairs.push((
                    to_value_controlled(heap, names, key, ctx, conv, proceed)?,
                    to_value_controlled(heap, names, value, ctx, conv, proceed)?,
                ));
            }
            let mut strings = true;
            for (key, _) in &pairs {
                check(proceed)?;
                strings &= matches!(key, Value::Str(_));
            }
            let literal = conv == Conversion::Graph && view_flags & flags::RECORD != 0;
            if literal && !strings {
                return Err(ConversionError::Unsupported(
                    "a core graph record literal with non-string fields".into(),
                ));
            }
            if strings && (conv == Conversion::Report || literal) {
                let mut fields = storage(pairs.len(), proceed)?;
                for (key, value) in pairs {
                    check(proceed)?;
                    if let Value::Str(name) = key {
                        fields.push((name, value));
                    }
                }
                Value::record_controlled(fields, ctx, proceed).map_err(Into::into)
            } else {
                Value::fcn_controlled(pairs, ctx, proceed).map_err(Into::into)
            }
        }
    }
}

/// The smallest stored set TLC might print in a lazy form: `SUBSET S` of a
/// seven-element `S` (TLC expands smaller ones).
const LAZY_PRINT_MIN: usize = 128;

/// Why an enumerated set may print differently in TLC, if it may: when the
/// plan may build intervals or lazy sets, a set of consecutive integers may
/// be TLC's interval (printed `1..3`), and one of at least
/// [`LAZY_PRINT_MIN`] elements TLC's lazy `SUBSET`, function, record or
/// product set.
fn ambiguous(heap: &Heap, h: Handle) -> Option<String> {
    if !heap.store().lazy_sets() {
        return None;
    }
    let v = heap.view(h);
    if !v.kind.is_set() {
        return None;
    }
    let elems = v.values();
    if elems.len() >= LAZY_PRINT_MIN {
        return Some(format!(
            "printing a set of {} elements, which TLC may hold as a lazy set and print in its lazy form (M1 core)",
            elems.len()
        ));
    }
    let ints: Option<Vec<i64>> = elems.iter().map(|e| e.as_int()).collect();
    if let Some(ints) = ints
        && !ints.is_empty()
        && ints.windows(2).all(|w| w[1] == w[0] + 1)
    {
        return Some(
            "printing a set of consecutive integers, which TLC may hold as an interval and print as `a..b` (M1 core)".into(),
        );
    }
    None
}

/// The reference engine's value of a symbolic set of the core's.
fn symbolic_value(
    heap: &mut Heap,
    names: &Interner,
    h: Handle,
    ctx: Ctx,
    conv: Conversion,
) -> Result<Value, String> {
    let v = heap.view(h);
    let (kind, lo, data) = (v.kind, v.lo, v.data.to_vec());
    if kind == Kind::Interval {
        let hi = data
            .first()
            .and_then(|h| h.as_int())
            .ok_or("an interval without its bound")?;
        return Ok(Value::interval(
            dive_value::Int::from(lo),
            dive_value::Int::from(hi),
            ctx,
        ));
    }
    let mut parts = Vec::with_capacity(data.len());
    let n = if kind == Kind::RecSet {
        data.len() / 2
    } else {
        0
    };
    for (i, &d) in data.iter().enumerate() {
        // A record set's field names are strings, which convert as they are.
        parts.push(to_value(heap, names, d, ctx, conv).map_err(|e| {
            if i < n {
                format!("a record set's field name: {e}")
            } else {
                e
            }
        })?);
    }
    let one = |parts: &mut Vec<Value>| parts.pop().ok_or("a set without its parts");
    match kind {
        Kind::Nat => Ok(Value::nat(ctx)),
        Kind::Int => Ok(Value::int_set(ctx)),
        Kind::SeqSet => Ok(Value::seq_set(one(&mut parts)?, ctx)),
        Kind::Subset => Ok(Value::subset(one(&mut parts)?, ctx)),
        Kind::FcnSet if parts.len() == 2 => {
            let range = one(&mut parts)?;
            Ok(Value::fcn_set(one(&mut parts)?, range, ctx))
        }
        Kind::Product => Ok(Value::product(parts, ctx)),
        Kind::RecSet => {
            let sets = parts.split_off(n);
            let fields = parts
                .into_iter()
                .zip(sets)
                .map(|(k, x)| match k {
                    Value::Str(a) => Ok((a, x)),
                    _ => Err("a record set's field name that is not a string".to_owned()),
                })
                .collect::<Result<_, _>>()?;
            Value::record_set(fields, ctx).map_err(|e| e.to_string())
        }
        _ => Err(format!("an infinite {kind:?} the core cannot report")),
    }
}

/// `ToString(v)` (`Value.toString`, as `TLC.ToString` calls it): the
/// single-line form.
///
/// # Errors
/// Unsupported when the store has no printer or the value's printed form
/// is unknown to the core; TLC's printing errors are unsupported too.
pub fn tlc_string(heap: &mut Heap, h: Handle) -> Result<String, Error> {
    print_with(heap, h, dive_value::print::to_tlc_string)
}

/// The text `Print` and `PrintT` print for a value: TLC's pretty-printed
/// form (`tlc2.pprint`, as the reference engine prints it).
///
/// # Errors
/// As [`tlc_string`].
pub fn pretty(heap: &mut Heap, h: Handle) -> Result<String, Error> {
    print_with(heap, h, dive_value::print::print_value)
}

fn print_with(
    heap: &mut Heap,
    h: Handle,
    f: fn(&Value, Ctx) -> dive_value::Result<String>,
) -> Result<String, Error> {
    let Some(p) = heap.store().printer().cloned() else {
        return Err(Error::unsupported(
            "printing without the model's names (M1 core)",
        ));
    };
    let v = to_value(heap, &p.names, h, p.ctx, Conversion::Print).map_err(Error::unsupported)?;
    f(&v, p.ctx)
        .map_err(|e| Error::unsupported(format!("an error while printing a value (M1 core): {e}")))
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "conversion fixtures")]
mod controlled_tests {
    use super::*;
    use crate::Store;

    #[test]
    fn materialized_conversion_keeps_results_and_name_interning_order() {
        for ctx in [Ctx::tlc(), Ctx::default()] {
            let source_names = Interner::new();
            let input = Value::tuple(vec![
                Value::string(&source_names, "z"),
                Value::set(vec![Value::Int(2), Value::Int(1)], ctx).expect("set"),
                Value::record(
                    vec![(source_names.intern("a"), Value::model(&source_names, "M_x"))],
                    ctx,
                )
                .expect("record"),
            ]);
            let mut heap = Heap::new(Arc::new(Store::default()), false, false);
            let handle = crate::convert::from_value(&mut heap, &input, ctx).expect("import");
            let baseline_names = Interner::new();
            let controlled_names = Interner::new();
            let expected = to_value(&mut heap, &baseline_names, handle, ctx, Conversion::Report)
                .expect("baseline");
            let mut checks = 0;
            let actual = to_value_controlled(
                &mut heap,
                &controlled_names,
                handle,
                ctx,
                Conversion::Report,
                &mut || {
                    checks += 1;
                    true
                },
            )
            .expect("controlled");
            assert_eq!(expected, actual);
            for at in 0..baseline_names.len() {
                let at = u32::try_from(at).expect("small names");
                assert_eq!(
                    baseline_names
                        .resolve(at)
                        .map(|name| name.as_str().to_owned()),
                    controlled_names
                        .resolve(at)
                        .map(|name| name.as_str().to_owned())
                );
            }
            assert!(checks > 20);
            for stop in 0..checks {
                let mut visited = 0;
                let result = to_value_controlled(
                    &mut heap,
                    &Interner::new(),
                    handle,
                    ctx,
                    Conversion::Report,
                    &mut || {
                        let proceed = visited != stop;
                        visited += 1;
                        proceed
                    },
                );
                assert!(
                    matches!(
                        result,
                        Err(ConversionError::Construction(
                            dive_value::ConstructionError::Interrupted
                        ))
                    ),
                    "{stop}: {result:?}"
                );
            }
        }
    }

    #[test]
    fn symbolic_report_conversion_keeps_explicit_legacy_fallback() {
        let mut heap = Heap::new(Arc::new(Store::default()), false, false);
        let interval = heap.interval(1, 3);
        let names = Interner::new();
        let expected = to_value(&mut heap, &names, interval, Ctx::tlc(), Conversion::Report)
            .expect("baseline");
        let actual = to_value_controlled(
            &mut heap,
            &names,
            interval,
            Ctx::tlc(),
            Conversion::Report,
            &mut || true,
        )
        .expect("controlled");
        assert_eq!(expected, actual);
    }

    #[test]
    fn controlled_graph_conversion_keeps_literal_classes_and_symbolic_refusals() {
        for ctx in [Ctx::dive(), Ctx::tlc()] {
            let mut heap = Heap::new(Arc::new(Store::default()), true, false);
            let names = Interner::new();
            let key = heap.string("a").expect("field");
            let zero = Handle::int(0).expect("integer");
            let functions = [
                heap.fcn_sorted(&[key], &[zero]),
                heap.record(&[key], &[zero]),
                heap.tuple(&[zero]),
                heap.tuple_literal(&[zero]),
            ];
            for handle in functions {
                let original =
                    to_value(&mut heap, &names, handle, ctx, Conversion::Graph).expect("baseline");
                let converted = to_value_controlled(
                    &mut heap,
                    &names,
                    handle,
                    ctx,
                    Conversion::Graph,
                    &mut || true,
                )
                .expect("controlled");
                let (original, converted) = (
                    original.as_fcn().expect("function"),
                    converted.as_fcn().expect("function"),
                );
                assert_eq!(original.kind(), converted.kind());
                assert_eq!(original.is_record_literal(), converted.is_record_literal());
                assert_eq!(original.is_tuple_literal(), converted.is_tuple_literal());
            }
            for handle in [heap.set_enum(&[zero]), heap.interval(0, 1)] {
                let expected = to_value(&mut heap, &names, handle, ctx, Conversion::Graph)
                    .expect_err("unknown history");
                let actual = to_value_controlled(
                    &mut heap,
                    &names,
                    handle,
                    ctx,
                    Conversion::Graph,
                    &mut || true,
                )
                .expect_err("controlled unknown history");
                assert_eq!(expected, actual.to_string());
            }
        }
    }
}
