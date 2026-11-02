//! Mathematical disk identities remain separate from receiver representation.
#![allow(
    clippy::expect_used,
    reason = "canonical disk identity regression assertions"
)]

use std::sync::Arc;

use dive_core::key::{KeyError, canonical};
use dive_core::{Handle, Heap, Store};

fn heap() -> Heap {
    Heap::new(Arc::new(Store::new(true)), true, false)
}

#[test]
fn equal_function_receivers_and_store_name_tokens_have_equal_keys() {
    let mut a = heap();
    let mut b = heap();
    let one = Handle::int(1).expect("inline");
    let two = Handle::int(2).expect("inline");
    let x = a.string("x").expect("name");
    let _ = b.string("earlier unrelated token").expect("name");
    let y = b.string("x").expect("name");
    assert_ne!(x.bits(), y.bits());
    let tuple = a.tuple_literal(&[x, two]);
    let tuple = a.intern(tuple).expect("canonical tuple");
    let function = b.fcn_from_pairs(&mut [(two, two), (one, y)]);
    let function = b.intern(function).expect("canonical function");
    assert_eq!(
        canonical(&a, &[tuple], 4096, &|| true),
        canonical(&b, &[function], 4096, &|| true)
    );
}

#[test]
fn set_identity_ignores_duplicates_and_preserves_original_receiver_bits() {
    let mut h = heap();
    let one = Handle::int(1).expect("inline");
    let two = Handle::int(2).expect("inline");
    let x = h.set_from(&mut vec![two, one, two]);
    let x = h.intern(x).expect("closed");
    let y = h.set_from(&mut vec![one, two]);
    let y = h.intern(y).expect("closed");
    assert_eq!(
        canonical(&h, &[x], 4096, &|| true),
        canonical(&h, &[y], 4096, &|| true)
    );
    let original = h.tuple_literal(&[one, two]);
    let original = h.intern_exact(original).expect("original receiver");
    let before = h.view(original).flags;
    assert!(matches!(
        canonical(&h, &[original], 4096, &|| true),
        Err(KeyError::Unsupported(_))
    ));
    assert_eq!(h.view(original).flags, before);
}

#[test]
fn cancellation_and_explicit_key_bounds_return_no_partial_identity() {
    let h = heap();
    assert_eq!(
        canonical(&h, &[], 4096, &|| false),
        Err(KeyError::Interrupted)
    );
    assert!(matches!(
        canonical(&h, &[Handle::TRUE], 8, &|| true),
        Err(KeyError::Limit(_))
    ));
    assert!(matches!(
        canonical(&h, &[Handle::EMPTY], 4096, &|| true),
        Err(KeyError::Internal(_))
    ));
}
