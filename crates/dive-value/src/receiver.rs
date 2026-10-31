//! Sealed receiver construction facts. They are not value identity or a wire format.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::value::Fcn;
use crate::{Value, ValueOrder};

const LITERAL: u64 = 1 << 63;
const TLC_ORDER: u64 = 1 << 62;
const DIAGNOSTIC: u64 = 1 << 61;
const DOMAIN_MASK: u32 = (1 << 29) - 1;
static NEXT_DOMAIN: AtomicU32 = AtomicU32::new(1);

/// One evaluator's nonreused receiver-construction namespace.
///
/// The owner keeps this capability private. Exhaustion declines provenance;
/// it never reuses another owner's identity or affects value semantics.
///
/// ```
/// use dive_value::{Ctx, ReceiverDomain, Value, tlc};
/// if let Some(owner) = ReceiverDomain::new() {
///     let tuple = owner.tuple(vec![Value::Int(1)], 7, true);
///     let caught = tlc::equals(&tuple, &Value::FALSE, Ctx::tlc())
///         .err().and_then(|error| error.receiver_failure());
///     assert_eq!(caught.and_then(|failure| owner.recover(failure)), Some((7, true)));
/// }
/// ```
#[derive(Debug)]
pub struct ReceiverDomain(u32);

impl ReceiverDomain {
    /// Creates an owner capability, or declines if its packed identity is exhausted.
    #[must_use]
    pub fn new() -> Option<Self> {
        NEXT_DOMAIN
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                (n < DOMAIN_MASK).then_some(n + 1)
            })
            .ok()
            .map(Self)
    }

    /// Builds an actual tuple constructor with this owner's source-hook facts.
    /// Reusing a cached/shared value never calls this constructor.
    #[must_use]
    pub fn tuple(&self, values: Vec<Value>, node: u32, diagnostic: bool) -> Value {
        Value::Fcn(Fcn::tuple_metadata(
            values,
            self.metadata(ValueOrder::Canonical, node, diagnostic),
        ))
    }

    /// Builds an actual record constructor with this owner's source-hook facts.
    ///
    /// # Errors
    /// Duplicate fields, exactly as `Value::record`.
    pub fn record(
        &self,
        fields: Vec<(crate::Atom, Value)>,
        ctx: crate::Ctx,
        node: u32,
        diagnostic: bool,
    ) -> crate::Result<Value> {
        if fields.is_empty() {
            return Ok(self.tuple(Vec::new(), node, diagnostic));
        }
        Ok(Value::Fcn(Fcn::record_metadata(
            crate::value::sort_fields(fields, ctx.order)?,
            self.metadata(ctx.order, node, diagnostic),
        )))
    }

    fn metadata(&self, order: ValueOrder, node: u32, diagnostic: bool) -> FunctionMetadata {
        let mut metadata = FunctionMetadata::new(order, true);
        metadata.0 |= (u64::from(self.0) << 32) | u64::from(node);
        if diagnostic {
            metadata.0 |= DIAGNOSTIC;
        }
        metadata
    }

    /// Recovers a catch only for this owner: constructor node and source-hook state.
    #[must_use]
    pub fn recover(&self, failure: ReceiverFailure) -> Option<(u32, bool)> {
        let origin = failure.origin;
        (origin.domain == self.0).then_some((origin.node, origin.diagnostic))
    }
}

/// The actual literal receiver method that caught a value-layer error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiverMethod {
    /// TLC's `TupleValue.equals`.
    TupleEquals,
    /// TLC's `RecordValue.equals`.
    RecordEquals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReceiverOrigin {
    domain: u32,
    node: u32,
    diagnostic: bool,
}

/// A value-method catch with sealed receiver construction, recoverable only by
/// the evaluator that constructed it. Text and an error code cannot create it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiverFailure {
    method: ReceiverMethod,
    origin: ReceiverOrigin,
}

impl ReceiverFailure {
    /// The method that actually caught this failure.
    #[must_use]
    pub fn method(self) -> ReceiverMethod {
        self.method
    }

    pub(crate) const fn sourced(self) -> bool {
        self.origin.diagnostic
    }

    pub(crate) fn equals(f: &Fcn) -> Option<Self> {
        let method = if f.is_tuple_literal() {
            ReceiverMethod::TupleEquals
        } else if f.is_record_literal() {
            ReceiverMethod::RecordEquals
        } else {
            return None;
        };
        Some(Self {
            method,
            origin: f.0.metadata.origin()?,
        })
    }
}

/// Replaces the former order/literal bytes and their padding, not an added
/// allocation or a larger function header. Never participates in identity.
#[derive(Clone, Copy)]
pub(crate) struct FunctionMetadata(pub(crate) u64);

impl FunctionMetadata {
    pub(crate) const fn new(order: ValueOrder, literal: bool) -> Self {
        Self(
            (if literal { LITERAL } else { 0 })
                | if matches!(order, ValueOrder::Tlc) {
                    TLC_ORDER
                } else {
                    0
                },
        )
    }

    pub(crate) const fn literal(self) -> bool {
        self.0 & LITERAL != 0
    }

    pub(crate) const fn order(self) -> ValueOrder {
        if self.0 & TLC_ORDER == 0 {
            ValueOrder::Canonical
        } else {
            ValueOrder::Tlc
        }
    }

    fn origin(self) -> Option<ReceiverOrigin> {
        let domain = u32::try_from((self.0 >> 32) & u64::from(DOMAIN_MASK)).ok()?;
        (domain != 0).then_some(ReceiverOrigin {
            domain,
            node: u32::try_from(self.0 & u64::from(u32::MAX)).ok()?,
            diagnostic: self.0 & DIAGNOSTIC != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receiver_facts_are_owner_bound_and_do_not_relabel_cached_aliases() {
        let Some(owner) = ReceiverDomain::new() else {
            return;
        };
        let Some(foreign) = ReceiverDomain::new() else {
            return;
        };
        let value = owner.tuple(vec![Value::Int(1)], 17, false);
        let hash = value.hash64();
        let alias = value.clone();
        let error = crate::tlc::equals(&alias, &Value::FALSE, crate::Ctx::tlc());
        let failure = error.err().and_then(|e| e.receiver_failure());
        assert_eq!(failure.and_then(|f| owner.recover(f)), Some((17, false)));
        assert_eq!(failure.and_then(|f| foreign.recover(f)), None);
        assert_eq!(value.hash64(), hash);
        assert_eq!(value, Value::tuple(vec![Value::Int(1)]));
        assert_eq!(
            crate::ValueError::new(crate::ErrorKind::Incomparable, "apparent source")
                .receiver_failure(),
            None
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(std::mem::size_of::<crate::value::FcnInner>(), 64);
        assert_eq!(std::mem::size_of::<Value>(), 16);
    }
}
