//! Stored-value admission for the first random-stream replay slice with VIEW.
//!
//! This is a representation check, not a reachability or replay certificate.
//! It deliberately excludes the compound-set decoding boundary in issue #190.

use dive_value::{FcnKind, Value};

/// Maximum number of visited values in one admitted state or VIEW value.
pub const MAX_VALUES: usize = 16_384;
/// Maximum nesting in the admitted stored representation.
pub const MAX_DEPTH: usize = 64;

/// Refuse shapes whose concrete JSON replay representation is not yet proved.
///
/// Admission visits stored atoms, literal tuples/records, general functions
/// with homogeneous atomic domains, and explicit homogeneous atomic sets.
/// Symbolic/deferred sets, compound or mixed sets, and nonliteral tuple/record
/// representations are refused. No provider, comparison, normalization or
/// evaluator callback is invoked, and no input-sized work vector is allocated.
/// These limits apply to the combined values in `values`.
///
/// # Errors
/// A named representation or bounded-work refusal. Passing this check alone
/// does not certify a counterexample, its stream position or its VIEW.
pub fn admit_values(values: &[Value]) -> Result<(), &'static str> {
    let mut remaining = MAX_VALUES;
    if values.len() > remaining {
        return Err("random VIEW value work limit");
    }
    for value in values {
        visit(value, 0, &mut remaining)?;
    }
    Ok(())
}

fn atom_class(value: &Value) -> Option<u8> {
    match value {
        Value::Bool(_) => Some(0),
        Value::Int(_) | Value::BigInt(_) => Some(1),
        Value::Str(_) => Some(2),
        Value::Model(_) => Some(3),
        Value::Set(_) | Value::Fcn(_) => None,
    }
}

fn room(len: usize, remaining: usize) -> Result<(), &'static str> {
    if len > remaining {
        return Err("random VIEW value work limit");
    }
    Ok(())
}

fn visit(value: &Value, depth: usize, remaining: &mut usize) -> Result<(), &'static str> {
    if depth > MAX_DEPTH {
        return Err("random VIEW value nesting limit");
    }
    *remaining = remaining
        .checked_sub(1)
        .ok_or("random VIEW value work limit")?;
    match value {
        Value::Bool(_) | Value::Int(_) | Value::BigInt(_) | Value::Str(_) | Value::Model(_) => {}
        Value::Set(set) => {
            let elements = set
                .stored_elements()
                .ok_or("random VIEW requires explicit stored sets")?;
            room(elements.len(), *remaining)?;
            let mut class = None;
            for element in elements {
                let next = atom_class(element)
                    .ok_or("random VIEW compound sets have no proved concrete replay shape")?;
                if class.is_some_and(|prior| prior != next) {
                    return Err("random VIEW mixed sets have no proved concrete replay shape");
                }
                class = Some(next);
                visit(element, depth + 1, remaining)?;
            }
        }
        Value::Fcn(function) => match function.kind() {
            FcnKind::Tuple => {
                if !function.is_tuple_literal() {
                    return Err(
                        "random VIEW nonliteral tuples have no proved replay representation",
                    );
                }
                let values = function
                    .as_tuple()
                    .ok_or("random VIEW tuple representation")?;
                room(values.len(), *remaining)?;
                for value in values {
                    visit(value, depth + 1, remaining)?;
                }
            }
            FcnKind::Record => {
                if !function.is_record_literal() {
                    return Err(
                        "random VIEW nonliteral records have no proved replay representation",
                    );
                }
                let fields = function
                    .as_record()
                    .ok_or("random VIEW record representation")?;
                room(fields.len(), *remaining)?;
                for (_, value) in fields {
                    visit(value, depth + 1, remaining)?;
                }
            }
            FcnKind::General => {
                let count = function
                    .len()
                    .checked_mul(2)
                    .ok_or("random VIEW value work limit")?;
                room(count, *remaining)?;
                let mut class = None;
                for (domain, range) in function.iter() {
                    let next = atom_class(&domain)
                        .ok_or("random VIEW requires atomic function domains")?;
                    if class.is_some_and(|prior| prior != next) {
                        return Err("random VIEW requires homogeneous function domains");
                    }
                    class = Some(next);
                    visit(&domain, depth + 1, remaining)?;
                    visit(range, depth + 1, remaining)?;
                }
            }
        },
    }
    Ok(())
}
