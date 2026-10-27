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
use crate::node::Kind;

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
    /// Printing: symbolic sets stay symbolic, and a stored set TLC could
    /// print otherwise is refused.
    Print,
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
    if kind == Kind::BigInt {
        let b = heap.big(h).ok_or("a malformed integer")?;
        return Ok(Value::int(dive_value::Int::from_big(b)));
    }
    // An infinite set is the reference engine's lazy set, as TLC keeps it:
    // a state may hold one until it is fingerprinted.
    let keep = conv == Conversion::Print
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
    let (lo, keys, vals) = (v.lo, v.keys().to_vec(), v.values().to_vec());
    match kind {
        Kind::Set | Kind::SetEnum => {
            let mut elems = Vec::with_capacity(vals.len());
            for e in vals {
                elems.push(to_value(heap, names, e, ctx, conv)?);
            }
            Value::set(elems, ctx).map_err(|e| e.to_string())
        }
        Kind::IntFcn => {
            let mut elems = Vec::with_capacity(vals.len());
            for e in vals {
                elems.push(to_value(heap, names, e, ctx, conv)?);
            }
            if lo == 1 {
                return Ok(Value::tuple(elems));
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
            // TLC prints a function of strings in record syntax when every
            // string is a name, whether or not it is a record literal: the
            // printer decides from the function (`Value::fcn`).
            if conv == Conversion::Report && pairs.iter().all(|(k, _)| matches!(k, Value::Str(_))) {
                let fields = pairs
                    .into_iter()
                    .filter_map(|(k, x)| match k {
                        Value::Str(a) => Some((a, x)),
                        _ => None,
                    })
                    .collect();
                return Value::record(fields, ctx).map_err(|e| e.to_string());
            }
            Value::fcn(pairs, ctx).map_err(|e| e.to_string())
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
