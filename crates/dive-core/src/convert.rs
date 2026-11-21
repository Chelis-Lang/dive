//! The binder boundary: converts startup values of
//! `dive-model`'s binder (`dive_value::Value`) to persistent handles.
//!
//! Lazy set classes that the core represents symbolically (`a..b`, `Nat`,
//! `Int`, `Seq(S)`, `SUBSET S`, `[D -> R]`, record sets, products) stay
//! symbolic; other sets are enumerated in the reference's normalized order
//! and rebuilt in the core's canonical order.

use dive_value::{Ctx, DeferredKind, DeferredSet, FcnKind, Set, SetKind, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::Error;
use crate::handle::Handle;
use crate::heap::Heap;
use crate::node::Kind;

/// The mathematical union of two already constructed record domains. The
/// converter recognizes this concrete value without calling a provider or
/// trusting a claimed kind. Ordinary value consumers can still enumerate it.
/// Returns `None` unless each operand is a record set or a union built here
/// and the expanded tree contains at most 128 record and union nodes.
/// Import into a native heap checks the field domains for compatibility.
#[must_use]
pub fn record_union(left: Set, right: Set, ctx: Ctx) -> Option<Value> {
    fn nodes(set: &Set) -> Option<usize> {
        if set.as_rcd_set().is_some() {
            return Some(1);
        }
        let provider: &dyn std::any::Any = set.as_deferred()?.as_ref();
        provider.downcast_ref::<RecordUnion>().map(|u| u.2)
    }
    let count = nodes(&left)?.checked_add(nodes(&right)?)?.checked_add(1)?;
    (count <= MAX_UNION_NODES)
        .then(|| Value::deferred(Arc::new(RecordUnion(left, right, count)), ctx))
}

const MAX_UNION_NODES: usize = 128;
struct RecordUnion(Set, Set, usize);

/// Marks a union imported from a closed record type. Its ordinary conversion
/// would have produced a concrete set.
pub(crate) const BOUND_RECORD_UNION: i64 = 1;

impl std::fmt::Debug for RecordUnion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("record domain union")
    }
}

impl DeferredSet for RecordUnion {
    fn kind(&self) -> DeferredKind {
        DeferredKind::Cup
    }

    fn elements(&self, ctx: Ctx) -> dive_value::Result<Vec<Value>> {
        let mut all = self.0.elements(ctx)?.to_vec();
        all.extend(self.1.elements(ctx)?.iter().cloned());
        Ok(all)
    }

    fn member(&self, element: &Value, ctx: Ctx) -> Option<dive_value::Result<bool>> {
        Some(
            dive_value::tlc::member(element, &Value::Set(self.0.clone()), ctx).and_then(|found| {
                if found {
                    Ok(true)
                } else {
                    dive_value::tlc::member(element, &Value::Set(self.1.clone()), ctx)
                }
            }),
        )
    }

    fn key(&self) -> Vec<Value> {
        vec![Value::Set(self.0.clone()), Value::Set(self.1.clone())]
    }
}

/// Converts a reference value to a persistent (or scalar) handle.
///
/// # Errors
/// Values the core does not represent (`STRING`), and sets the reference
/// cannot enumerate.
pub fn from_value(heap: &mut Heap, v: &Value, ctx: Ctx) -> Result<Handle, Error> {
    let mut remaining = MAX_UNION_NODES;
    let h = transient(heap, v, ctx, &mut remaining)?;
    heap.intern_const(h)
}

fn int(i: &dive_value::Int) -> Result<Handle, Error> {
    i.to_i64()
        .and_then(Handle::int)
        .ok_or_else(|| Error::unsupported("an interval bound outside the core's 61-bit range"))
}

// Check the converted domains as well as their source-independent representation.
// A public caller can construct a record type with a scalar function domain.
fn valid_domain(heap: &Heap, set: Handle, remaining: &mut usize) -> bool {
    let Some(next) = remaining.checked_sub(1) else {
        return false;
    };
    *remaining = next;
    match heap.kind(set) {
        Some(Kind::Set | Kind::Interval) => true,
        Some(Kind::FcnSet | Kind::Product | Kind::Subset | Kind::RecSet) => {
            let view = heap.view(set);
            let domains = if view.kind == Kind::RecSet {
                &view.data[view.data.len() / 2..]
            } else {
                view.data
            };
            domains.iter().all(|&d| valid_domain(heap, d, remaining))
        }
        _ => false,
    }
}

fn compatible_family(
    heap: &Heap,
    family: Handle,
    fields: &mut HashMap<Handle, Handle>,
    remaining: &mut usize,
) -> bool {
    let Some(next) = remaining.checked_sub(1) else {
        return false;
    };
    *remaining = next;
    if heap.kind(family) == Some(Kind::Union) {
        let v = heap.view(family);
        return v.lo == BOUND_RECORD_UNION
            && v.data
                .iter()
                .all(|&d| compatible_family(heap, d, fields, remaining));
    }
    if heap.kind(family) != Some(Kind::RecSet) {
        return false;
    }
    let v = heap.view(family);
    let (names, domains) = v.data.split_at(v.data.len() / 2);
    let string_label = |d| {
        heap.kind(d) == Some(Kind::Set)
            && matches!(heap.view(d).data, [x] if x.as_str_id().is_some())
    };
    for (&name, &domain) in names.iter().zip(domains) {
        if !valid_domain(heap, domain, remaining) {
            return false;
        }
        if let Some(&other) = fields.get(&name) {
            if !(string_label(other) && string_label(domain)) && heap.cmp(other, domain).is_ne() {
                return false;
            }
        } else {
            fields.insert(name, domain);
        }
    }
    true
}

#[allow(clippy::too_many_lines, reason = "one arm per value class")]
fn transient(heap: &mut Heap, v: &Value, ctx: Ctx, remaining: &mut usize) -> Result<Handle, Error> {
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
                let b = transient(heap, base, ctx, remaining)?;
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
                let d = transient(heap, d, ctx, remaining)?;
                let d = heap.norm(d)?;
                let r = transient(heap, r, ctx, remaining)?;
                Ok(heap.symbolic(Kind::FcnSet, 0, &[d, r]))
            }
            SetKind::RcdSet => {
                let Some(fields) = s.as_rcd_set() else {
                    return Err(Error::internal("record set without fields"));
                };
                let mut pairs = Vec::new();
                for (name, set) in fields {
                    pairs.push((
                        heap.string(name.as_str())?,
                        transient(heap, set, ctx, remaining)?,
                    ));
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
                    data.push(transient(heap, f, ctx, remaining)?);
                }
                Ok(heap.symbolic(Kind::Product, 0, &data))
            }
            SetKind::Enum | SetKind::Raw | SetKind::Deferred => {
                if let Some(provider) = s.as_deferred() {
                    let any: &dyn std::any::Any = provider.as_ref();
                    if let Some(union) = any.downcast_ref::<RecordUnion>() {
                        *remaining = remaining.checked_sub(1).ok_or_else(|| {
                            Error::unsupported(
                                "closed record union exceeds the conversion work limit",
                            )
                        })?;
                        let a = transient(heap, &Value::Set(union.0.clone()), ctx, remaining)?;
                        let b = transient(heap, &Value::Set(union.1.clone()), ctx, remaining)?;
                        let mut fields = HashMap::new();
                        let mut visits = 512;
                        if !compatible_family(heap, a, &mut fields, &mut visits)
                            || !compatible_family(heap, b, &mut fields, &mut visits)
                        {
                            return Err(Error::unsupported(
                                "incompatible or malformed closed record domains",
                            ));
                        }
                        return Ok(heap.symbolic(Kind::Union, BOUND_RECORD_UNION, &[a, b]));
                    }
                }
                if !heap.store().semantics().tlc_comparisons
                    && let Some(elems) = s.raw_elements()
                {
                    let mut out = Vec::with_capacity(elems.len());
                    for e in elems {
                        let h = transient(heap, e, ctx, remaining)?;
                        out.push(heap.norm(h)?);
                    }
                    return Ok(heap.set_from(&mut out));
                }
                let elems = s.elements(ctx).map_err(|e| {
                    Error::unsupported(format!("a startup set the core cannot convert: {e}"))
                })?;
                let mut out = Vec::with_capacity(elems.len());
                for e in elems.iter() {
                    let h = transient(heap, e, ctx, remaining)?;
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
                    let h = transient(heap, e, ctx, remaining)?;
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
                    let k = transient(heap, &k, ctx, remaining)?;
                    let k = heap.norm(k)?;
                    let x = transient(heap, x, ctx, remaining)?;
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
