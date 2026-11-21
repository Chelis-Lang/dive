//! Import finite closed message types from the owning binder.

use dive_eval::Evaluator;
use dive_value::{Set, SetKind, Value};
use std::collections::BTreeMap;

/// Bound work across the entire candidate, including repeated visits to a
/// shared input node. Over-budget bindings follow ordinary conversion.
const MAX_VISITS: usize = 512;

fn tick(left: &mut usize) -> Option<()> {
    *left = left.checked_sub(1)?;
    Some(())
}

fn valid_domain(set: &Set, left: &mut usize) -> Option<()> {
    tick(left)?;
    match set.kind() {
        SetKind::Enum | SetKind::Raw | SetKind::Interval => Some(()),
        SetKind::Deferred => {
            let v = dive_eval::deferred_filter_normalization_operand(set.as_deferred()?.as_ref())?;
            let Value::Set(base) = v else { return None };
            valid_domain(base, left)
        }
        SetKind::FcnSet | SetKind::Product | SetKind::RcdSet | SetKind::Subset => {
            for (_, v) in set.components() {
                let Value::Set(domain) = v else { return None };
                valid_domain(&domain, left)?;
            }
            Some(())
        }
        _ => None,
    }
}

fn singleton_string(domain: &Set) -> bool {
    matches!(domain.stored_elements(), Some([Value::Str(_)]))
}

fn same_field(a: &Set, b: &Set, left: &mut usize) -> bool {
    if tick(left).is_none() {
        return false;
    }
    if a.ptr_eq(b) || (singleton_string(a) && singleton_string(b)) {
        return true;
    }
    if let (Some(x), Some(y)) = (a.as_interval(), b.as_interval()) {
        return x == y;
    }
    if a.kind() == SetKind::Subset && b.kind() == SetKind::Subset {
        let (a, b) = (a.components(), b.components());
        if let ([(None, Value::Set(a))], [(None, Value::Set(b))]) = (a.as_slice(), b.as_slice()) {
            return same_field(a, b, left);
        }
    }
    false
}

fn family(
    set: &Set,
    eval: &Evaluator,
    fields: &mut BTreeMap<String, Set>,
    left: &mut usize,
) -> Option<Value> {
    tick(left)?;
    if let Some(row) = set.as_rcd_set() {
        for (name, value) in row {
            let Value::Set(domain) = value else {
                return None;
            };
            valid_domain(domain, left)?;
            if let Some(previous) = fields.get(name.as_str()) {
                if !same_field(previous, domain, left) {
                    return None;
                }
            } else {
                fields.insert(name.as_str().to_owned(), domain.clone());
            }
        }
        return Some(Value::Set(set.clone()));
    }
    if set.has_cached_elements() {
        return None;
    }
    let (Value::Set(a), Value::Set(b)) = eval.owned_union_operands(set)? else {
        return None;
    };
    let Value::Set(a) = family(a, eval, fields, left)? else {
        return None;
    };
    let Value::Set(b) = family(b, eval, fields, left)? else {
        return None;
    };
    dive_core::convert::record_union(a, b, eval.options().ctx)
}

pub(crate) fn compact(value: &Value, eval: &Evaluator) -> Option<Value> {
    let Value::Set(root) = value else { return None };
    if root.has_cached_elements() {
        return None;
    }
    let mut left = MAX_VISITS;
    if eval.owned_union_operands(root).is_some() {
        return family(root, eval, &mut BTreeMap::new(), &mut left);
    }
    let mut result = Vec::new();
    let mut changed = false;
    for (name, field) in root.as_rcd_set()? {
        tick(&mut left)?;
        let Value::Set(domain) = field else {
            return None;
        };
        let v = if eval.owned_union_operands(domain).is_some() {
            changed = true;
            family(domain, eval, &mut BTreeMap::new(), &mut left)?
        } else {
            field.clone()
        };
        result.push((name.clone(), v));
    }
    if changed {
        Value::record_set(result, eval.options().ctx).ok()
    } else {
        None
    }
}
