//! The binder boundary (`04-plan-compiler.md` §2): converts startup values of
//! the reference binder (`dive_value::Value`) to persistent handles.
//!
//! Lazy set classes that the core represents symbolically (`a..b`, `Nat`,
//! `Int`, `Seq(S)`, `SUBSET S`, `[D -> R]`, record sets, products) stay
//! symbolic; other sets are enumerated in the reference's normalized order
//! and rebuilt in the core's canonical order.

use dive_value::{Ctx, FcnKind, SetKind, Value};

use crate::Error;
use crate::handle::Handle;
use crate::heap::Heap;
use crate::node::Kind;

/// Converts a reference value to a persistent (or scalar) handle.
///
/// # Errors
/// Values the core does not represent (`STRING`), and sets the reference
/// cannot enumerate.
pub fn from_value(heap: &mut Heap, v: &Value, ctx: Ctx) -> Result<Handle, Error> {
    let h = transient(heap, v, ctx)?;
    heap.intern_const(h)
}

fn int(i: &dive_value::Int) -> Result<Handle, Error> {
    i.to_i64()
        .and_then(Handle::int)
        .ok_or_else(|| Error::unsupported("an interval bound outside the core's 61-bit range"))
}

#[allow(clippy::too_many_lines, reason = "one arm per value class")]
fn transient(heap: &mut Heap, v: &Value, ctx: Ctx) -> Result<Handle, Error> {
    match v {
        Value::Bool(b) => Ok(Handle::bool(*b)),
        Value::Int(i) => Ok(heap.int_from_i64(*i)),
        Value::BigInt(b) => Ok(heap.int_from_big(b)),
        Value::Str(a) => heap.string(a.as_str()),
        Value::Model(a) => Ok(Handle::model(heap.store().strings().intern(a.as_str())?)),
        Value::Set(s) => match s.kind() {
            SetKind::Interval => {
                let Some((lo, hi)) = s.as_interval() else {
                    return Err(Error::internal("interval without bounds"));
                };
                let (lo, hi) = (int(lo)?, int(hi)?);
                let lo = lo.as_int().unwrap_or(0);
                let hi = hi.as_int().unwrap_or(-1);
                Ok(heap.interval(lo, hi))
            }
            SetKind::Nat => Ok(heap.symbolic(Kind::Nat, 0, &[])),
            SetKind::Int => Ok(heap.symbolic(Kind::Int, 0, &[])),
            SetKind::Strings => Err(Error::unsupported("STRING (M0 core)")),
            SetKind::Seq | SetKind::Subset => {
                let comps = s.components();
                let Some((_, base)) = comps.first() else {
                    return Err(Error::internal("set without a base"));
                };
                let b = transient(heap, base, ctx)?;
                let kind = if s.kind() == SetKind::Seq {
                    Kind::SeqSet
                } else {
                    Kind::Subset
                };
                Ok(heap.symbolic(kind, 0, &[b]))
            }
            SetKind::FcnSet => {
                let Some((d, r)) = s.as_fcn_set() else {
                    return Err(Error::internal("function set without components"));
                };
                let d = transient(heap, d, ctx)?;
                let d = heap.norm(d)?;
                let r = transient(heap, r, ctx)?;
                Ok(heap.symbolic(Kind::FcnSet, 0, &[d, r]))
            }
            SetKind::RcdSet => {
                let Some(fields) = s.as_rcd_set() else {
                    return Err(Error::internal("record set without fields"));
                };
                let mut pairs = Vec::new();
                for (name, set) in fields {
                    pairs.push((heap.string(name.as_str())?, transient(heap, set, ctx)?));
                }
                pairs.sort_by(|a, b| heap.cmp(a.0, b.0));
                let mut data: Vec<Handle> = pairs.iter().map(|p| p.0).collect();
                data.extend(pairs.iter().map(|p| p.1));
                Ok(heap.symbolic(Kind::RecSet, 0, &data))
            }
            SetKind::Product => {
                let Some(factors) = s.as_product() else {
                    return Err(Error::internal("product without factors"));
                };
                let mut data = Vec::new();
                for f in factors {
                    data.push(transient(heap, f, ctx)?);
                }
                Ok(heap.symbolic(Kind::Product, 0, &data))
            }
            SetKind::Enum | SetKind::Raw | SetKind::Deferred => {
                if !heap.store().semantics().tlc_comparisons
                    && let Some(elems) = s.raw_elements()
                {
                    let mut out = Vec::with_capacity(elems.len());
                    for e in elems {
                        let h = transient(heap, e, ctx)?;
                        out.push(heap.norm(h)?);
                    }
                    return Ok(heap.set_from(&mut out));
                }
                let elems = s.elements(ctx).map_err(|e| {
                    Error::unsupported(format!("a startup set the core cannot convert: {e}"))
                })?;
                let mut out = Vec::with_capacity(elems.len());
                for e in elems.iter() {
                    let h = transient(heap, e, ctx)?;
                    out.push(heap.norm(h)?);
                }
                Ok(heap.set_from(&mut out))
            }
        },
        Value::Fcn(f) => match f.kind() {
            FcnKind::Tuple => {
                let Some(items) = f.as_tuple() else {
                    return Err(Error::internal("tuple without items"));
                };
                let mut out = Vec::with_capacity(items.len());
                for e in items {
                    let h = transient(heap, e, ctx)?;
                    out.push(heap.norm(h)?);
                }
                Ok(if f.is_tuple_literal() {
                    heap.tuple_literal(&out)
                } else {
                    heap.tuple(&out)
                })
            }
            FcnKind::Record | FcnKind::General => {
                let mut pairs = Vec::with_capacity(f.len());
                for (k, x) in f.iter() {
                    let k = transient(heap, &k, ctx)?;
                    let k = heap.norm(k)?;
                    let x = transient(heap, x, ctx)?;
                    pairs.push((k, heap.norm(x)?));
                }
                if f.is_record_literal() {
                    pairs.sort_by(|a, b| heap.cmp(a.0, b.0));
                    let keys: Vec<_> = pairs.iter().map(|(k, _)| *k).collect();
                    let vals: Vec<_> = pairs.iter().map(|(_, v)| *v).collect();
                    Ok(heap.record(&keys, &vals))
                } else {
                    Ok(heap.fcn_from_pairs(&mut pairs))
                }
            }
        },
    }
}
