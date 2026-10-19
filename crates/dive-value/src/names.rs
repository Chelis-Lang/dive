//! Non-evaluating name-table integrity checks for public typed values.

use std::fmt;

use crate::value::{FcnRepr, SetRepr};
use crate::{Atom, Interner, Value, ValueOrder};

/// Why a typed value's name tokens cannot be trusted in a given name table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    /// An atom's text and token do not identify the same name in the table.
    ForeignAtom,
    /// An opaque deferred set would require caller code to inspect its names.
    OpaqueDeferred,
    /// A set, record or function is sorted in another value order than the
    /// one it is checked against.
    ForeignOrder,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}",
            self.code(),
            match self {
                Self::ForeignAtom => "typed value contains a name token from another name table",
                Self::OpaqueDeferred =>
                    "typed value contains an opaque deferred set whose names cannot be checked without evaluating it",
                Self::ForeignOrder =>
                    "typed value contains a set, record or function sorted in another value order",
            }
        )
    }
}

impl NameError {
    /// The stable diagnostic code for typed value name integrity.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ForeignAtom => "DIVE-V0041",
            Self::OpaqueDeferred => "DIVE-V0042",
            Self::ForeignOrder => "DIVE-V0043",
        }
    }
}

impl std::error::Error for NameError {}

fn check_atom(atom: &Atom, names: &Interner) -> Result<(), NameError> {
    if names
        .resolve(atom.tok())
        .is_some_and(|known| known.as_str() == atom.as_str())
    {
        Ok(())
    } else {
        Err(NameError::ForeignAtom)
    }
}

impl Value {
    /// Checks every stored string, model value and record-field name against
    /// `names`, including symbolic set components. Text and token must both
    /// match. This iterative walk does not intern names, normalize or enumerate
    /// sets, or invoke callbacks; it accepts any stored nesting depth.
    ///
    /// ```
    /// use dive_value::{Interner, NameError, Value};
    /// let names = Interner::new();
    /// let owned = Value::string(&names, "a");
    /// assert_eq!(owned.validate_names(&names), Ok(()));
    /// let foreign = Interner::new();
    /// foreign.intern("padding");
    /// let wrong_token = Value::string(&foreign, "a");
    /// assert_eq!(wrong_token.validate_names(&names), Err(NameError::ForeignAtom));
    /// ```
    ///
    /// # Errors
    /// A foreign name token or an opaque deferred set. Even a deferred set
    /// with cached elements is refused because its source may contain names
    /// the cache does not expose.
    pub fn validate_names(&self, names: &Interner) -> Result<(), NameError> {
        self.walk_names(names, None)
    }

    /// [`Self::validate_names`], and also checks that every stored set,
    /// record and non-tuple function is sorted in `order`: a value built under
    /// another run's order would be read in the wrong order by operations
    /// that rely on normalization (`CHOOSE`, enumeration, merging).
    ///
    /// ```
    /// use dive_value::{Ctx, Interner, NameError, Value};
    /// let names = Interner::new();
    /// let set = Value::set(vec![Value::string(&names, "a")], Ctx::dive()).expect("set");
    /// assert_eq!(set.validate_names_in(&names, Ctx::dive().order), Ok(()));
    /// assert_eq!(
    ///     set.validate_names_in(&names, Ctx::tlc().order),
    ///     Err(NameError::ForeignOrder)
    /// );
    /// ```
    ///
    /// # Errors
    /// As [`Self::validate_names`], or a value sorted in another order.
    pub fn validate_names_in(&self, names: &Interner, order: ValueOrder) -> Result<(), NameError> {
        self.walk_names(names, Some(order))
    }

    fn walk_names(&self, names: &Interner, order: Option<ValueOrder>) -> Result<(), NameError> {
        let mut work = vec![self];
        while let Some(value) = work.pop() {
            match value {
                Self::Str(atom) | Self::Model(atom) => check_atom(atom, names)?,
                Self::Fcn(fcn) => match &fcn.0.repr {
                    FcnRepr::Tuple(values) => work.extend(values.iter()),
                    _ if order.is_some_and(|o| fcn.order() != o) => {
                        return Err(NameError::ForeignOrder);
                    }
                    FcnRepr::Record(fields) => {
                        for (atom, value) in fields {
                            check_atom(atom, names)?;
                            work.push(value);
                        }
                    }
                    FcnRepr::General(pairs) => {
                        for (key, value) in pairs {
                            work.push(key);
                            work.push(value);
                        }
                    }
                },
                Self::Set(set) => {
                    // An opaque deferred set is refused as such whatever its order.
                    if matches!(set.repr(), SetRepr::Deferred(_)) {
                        return Err(NameError::OpaqueDeferred);
                    }
                    if order.is_some_and(|o| set.order() != o) {
                        return Err(NameError::ForeignOrder);
                    }
                    match set.repr() {
                        SetRepr::Deferred(_) => return Err(NameError::OpaqueDeferred),
                        SetRepr::Raw(values) | SetRepr::Product(values) => {
                            work.extend(values.iter());
                        }
                        SetRepr::Subset(base) | SetRepr::Seq(base) => work.push(base),
                        SetRepr::FcnSet(domain, range) => {
                            work.push(domain);
                            work.push(range);
                        }
                        SetRepr::RcdSet(fields) => {
                            for (atom, value) in fields {
                                check_atom(atom, names)?;
                                work.push(value);
                            }
                        }
                        SetRepr::Enum
                        | SetRepr::Interval(_, _)
                        | SetRepr::Nat
                        | SetRepr::Int
                        | SetRepr::Strings => {}
                    }
                    if let Some(values) = set.0.elems.get() {
                        work.extend(values.iter());
                    }
                }
                Self::Bool(_) | Self::Int(_) | Self::BigInt(_) => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "test construction")]

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::{Ctx, DeferredKind, DeferredSet};

    #[test]
    fn every_stored_and_symbolic_name_position_is_checked_without_interning() {
        for ctx in [Ctx::tlc(), Ctx::default()] {
            let known = Interner::new();
            known.intern("padding");
            known.intern("name");
            let foreign = Interner::new();
            foreign.intern("name");
            for (names, expected) in [(&known, Ok(())), (&foreign, Err(NameError::ForeignAtom))] {
                let atom = names.intern("name");
                let string = Value::Str(atom.clone());
                let values = [
                    string.clone(),
                    Value::Model(atom.clone()),
                    Value::tuple(vec![string.clone()]),
                    Value::record(vec![(atom.clone(), Value::Int(0))], ctx).expect("record"),
                    Value::fcn(vec![(string.clone(), Value::Int(0))], ctx)
                        .expect("function domain"),
                    Value::fcn(vec![(Value::Int(9), string.clone())], ctx).expect("function range"),
                    Value::set(vec![string.clone()], ctx).expect("set"),
                    Value::set_raw(vec![string.clone()], ctx),
                    Value::subset(string.clone(), ctx),
                    Value::fcn_set(string.clone(), Value::Int(0), ctx),
                    Value::fcn_set(Value::Int(0), string.clone(), ctx),
                    Value::record_set(vec![(atom, Value::Int(0))], ctx).expect("record set"),
                    Value::product(vec![string.clone()], ctx),
                    Value::seq_set(string, ctx),
                ];
                for value in values {
                    assert_eq!(value.validate_names(&known), expected);
                }
            }
            assert_eq!(known.len(), 2);
        }
    }

    #[derive(Debug, Default)]
    struct Opaque(AtomicUsize);

    impl DeferredSet for Opaque {
        fn kind(&self) -> DeferredKind {
            DeferredKind::Map
        }
        fn elements(&self, _: Ctx) -> crate::Result<Vec<Value>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(Vec::new())
        }
        fn member(&self, _: &Value, _: Ctx) -> Option<crate::Result<bool>> {
            None
        }
    }

    #[test]
    fn opaque_values_are_refused_without_callbacks_and_deep_values_are_iterative() {
        let names = Interner::new();
        let source = Arc::new(Opaque::default());
        let value = Value::deferred(source.clone(), Ctx::tlc());
        assert_eq!(value.validate_names(&names), Err(NameError::OpaqueDeferred));
        assert_eq!(source.0.load(Ordering::Relaxed), 0);
        let mut deep = Value::string(&names, "owned");
        for _ in 0..2000 {
            deep = Value::tuple(vec![deep]);
        }
        assert_eq!(deep.validate_names(&names), Ok(()));
    }
}
