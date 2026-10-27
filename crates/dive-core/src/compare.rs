//! Equality, membership and set-construction rules of the default profile
//! (`02-semantic-contract.md` C1–C3, D-009), as the reference engine applies
//! them (`crates/dive-eval/src/tla_cmp.rs`).
//!
//! - Two values of the same class (boolean, integer, string, set, function)
//!   compare extensionally; inside sets and functions, values of different
//!   classes are unequal.
//! - A direct comparison of values of different classes (`1 = "a"`,
//!   `{1} = 1`, `<<1>> = {1}`) is TLC's error.
//! - Membership that fails to find the element raises TLC's error when the
//!   set holds a scalar of another class than the element: TLC compares them
//!   on its way and fails.
//! - The core builds every set normalized, but TLC normalizes lazily, and
//!   whether and which error a set mixing incomparable values raises depends
//!   on when TLC normalizes it (C1, C2). The core refuses to build such a set
//!   rather than guess.
//!
//! Model values are not represented yet (`03-value-model.md`); when they are,
//! they join [`Class`] with TLC's rules (an untyped model value is unequal to
//! everything else).

use crate::Error;
use crate::handle::{Handle, Tag};
use crate::heap::Heap;
use crate::node::Kind;

/// The comparison class of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Bool,
    Int,
    Str,
    Set,
    Fcn,
}

impl Class {
    const fn is_scalar(self) -> bool {
        matches!(self, Self::Bool | Self::Int | Self::Str)
    }

    const fn noun(self) -> &'static str {
        match self {
            Self::Bool => "boolean",
            Self::Int => "integer",
            Self::Str => "string",
            Self::Set => "set",
            Self::Fcn => "function",
        }
    }

    /// The noun with its indefinite article.
    const fn a(self) -> &'static str {
        match self {
            Self::Bool => "a boolean",
            Self::Int => "an integer",
            Self::Str => "a string",
            Self::Set => "a set",
            Self::Fcn => "a function",
        }
    }
}

/// The class of a handle.
pub(crate) fn class(heap: &Heap, h: Handle) -> Class {
    match h.tag() {
        Tag::Bool => Class::Bool,
        Tag::Int => Class::Int,
        Tag::Str => Class::Str,
        Tag::Persistent | Tag::Transient | Tag::Empty => {
            if !h.is_compound() {
                return Class::Set;
            }
            match heap.view(h).kind {
                Kind::BigInt => Class::Int,
                k if k.is_fcn() => Class::Fcn,
                _ => Class::Set,
            }
        }
    }
}

fn mismatch(a: Class, b: Class) -> Error {
    // `Assert.fail(EC.GENERAL)` in the values' `equals`/`compareTo`.
    Error::eval_class(
        crate::TlcClass::GENERAL,
        format!(
            "Attempted to check equality of {} with a non-{} value: {}",
            a.a(),
            a.noun(),
            b.a()
        ),
    )
}

/// `a = b` on normalized handles.
///
/// # Errors
/// TLC's error for values of different classes.
pub(crate) fn equals(heap: &Heap, a: Handle, b: Handle) -> Result<bool, Error> {
    if a == b {
        return Ok(true);
    }
    let (ca, cb) = (class(heap, a), class(heap, b));
    if ca != cb {
        return Err(mismatch(ca, cb));
    }
    Ok(heap.eq(a, b))
}

/// After a miss of `x` in the enumerated set with sorted elements `elems`:
/// TLC's error if it would have compared `x` with a scalar of another class.
///
/// Elements are sorted by class (booleans, integers, strings, then
/// compounds), so the first and last elements decide.
pub(crate) fn miss(heap: &Heap, x: Handle, elems: &[Handle]) -> Result<(), Error> {
    let (Some(&first), Some(&last)) = (elems.first(), elems.last()) else {
        return Ok(());
    };
    let cx = class(heap, x);
    let (cf, cl) = (class(heap, first), class(heap, last));
    let clash = if cx.is_scalar() {
        if cf == cx {
            (cl != cx).then_some(cl)
        } else {
            Some(cf)
        }
    } else {
        cf.is_scalar().then_some(cf)
    };
    match clash {
        Some(c) => Err(mismatch(cx, c)),
        None => Ok(()),
    }
}

/// Whether a function is record-like (string keys) rather than tuple-like
/// (an integer domain), for TLC's asymmetric record/tuple order (C2, C5).
fn record_like(heap: &Heap, f: Handle) -> bool {
    let v = heap.view(f);
    v.kind == Kind::Fcn && v.keys().first().is_some_and(|k| k.as_str_id().is_some())
}

/// Refuses a set, given its sorted elements, whose elements TLC cannot all
/// compare: values of different classes, or records with tuples.
///
/// # Errors
/// [`crate::ErrorKind::Unsupported`], naming the mixture.
pub(crate) fn check_set(heap: &Heap, elems: &[Handle]) -> Result<(), Error> {
    let (Some(&first), Some(&last)) = (elems.first(), elems.last()) else {
        return Ok(());
    };
    if elems.len() < 2 {
        return Ok(());
    }
    let (cf, cl) = (class(heap, first), class(heap, last));
    let refuse = |a: Class, b: Class| {
        Err(Error::unsupported(format!(
            "a set mixing {} and {}: whether and how TLC fails on it depends on when it normalizes the set (C1, C2)",
            a.a(),
            b.a()
        )))
    };
    if cf != cl {
        return refuse(cf, cl);
    }
    match cf {
        // Finite sets sort before functions and infinite sets after them.
        Class::Set if heap.view(last).kind.is_symbolic() => {
            if elems.iter().any(|&e| class(heap, e) == Class::Fcn) {
                return refuse(Class::Set, Class::Fcn);
            }
            Ok(())
        }
        Class::Fcn => {
            let rec = record_like(heap, first);
            if elems.iter().any(|&e| record_like(heap, e) != rec) {
                return Err(Error::unsupported(
                    "a set mixing records and other functions: TLC's order between them is asymmetric (C2, C5)",
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::many_single_char_names,
        reason = "tests over short value names"
    )]
    use std::sync::Arc;

    use super::*;
    use crate::store::Store;

    fn heap() -> Heap {
        Heap::new(Arc::new(Store::default()), false, false)
    }

    #[test]
    fn different_classes_are_an_error_and_same_classes_compare() {
        let mut h = heap();
        let one = Handle::int(1).unwrap();
        let a = h.string("a").unwrap();
        let s = h.set_from(&mut vec![one]);
        let t = h.tuple(&[one]);
        assert!(equals(&h, one, a).is_err());
        assert!(equals(&h, s, one).is_err());
        assert!(equals(&h, t, s).is_err());
        assert_eq!(
            equals(&h, one, a).unwrap_err().tlc(),
            Some(crate::TlcClass::GENERAL)
        );
        assert!(equals(&h, Handle::TRUE, one).is_err());
        assert!(equals(&h, one, one).unwrap());
        let sa = h.set_from(&mut vec![a]);
        assert!(!equals(&h, s, sa).unwrap(), "nested classes are unequal");
    }

    #[test]
    fn a_miss_against_another_scalar_class_is_an_error() {
        let mut h = heap();
        let (one, two) = (Handle::int(1).unwrap(), Handle::int(2).unwrap());
        let a = h.string("a").unwrap();
        let s = h.set_from(&mut vec![one]);
        assert!(miss(&h, a, &[one, two]).is_err());
        assert!(miss(&h, s, &[one, two]).is_err());
        assert!(miss(&h, Handle::int(3).unwrap(), &[one, two]).is_ok());
        assert!(
            miss(&h, one, &[s]).is_err(),
            "TLC compares the integer with the set"
        );
        let t = h.tuple(&[one]);
        assert!(
            miss(&h, t, &[s]).is_ok(),
            "a function is unequal to a set element"
        );
        assert!(miss(&h, a, &[]).is_ok());
    }

    #[test]
    fn mixed_sets_are_refused() {
        let mut h = heap();
        let one = Handle::int(1).unwrap();
        let a = h.string("a").unwrap();
        let mut v = vec![one, a];
        let s = h.set_from(&mut v);
        let elems = h.view(s).data.to_vec();
        assert!(check_set(&h, &elems).is_err());
        let t = h.tuple(&[one]);
        let r = h.fcn_sorted(&[a], &[one]);
        let mut v = vec![t, r];
        let s = h.set_from(&mut v);
        let elems = h.view(s).data.to_vec();
        assert!(check_set(&h, &elems).is_err());
        let t2 = h.tuple(&[one, one]);
        let mut v = vec![t, t2];
        let s = h.set_from(&mut v);
        let elems = h.view(s).data.to_vec();
        assert!(check_set(&h, &elems).is_ok());
    }
}
