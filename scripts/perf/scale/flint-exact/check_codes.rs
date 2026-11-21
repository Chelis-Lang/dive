//! Run each mode in a fresh process so its column dictionaries start empty.
use std::collections::BTreeMap;
use std::sync::Arc;
use tlc_rs::state_codes::state;
use tlc_rs::value::{Func, Value};

fn main() -> Result<(), String> {
    if std::env::args().nth(1).as_deref() == Some("overflow") {
        let mut row = vec![Value::Bool(false); 65];
        state(&row)?;
        for i in 0..63 {
            row[i] = Value::Bool(true);
            state(&row)?;
        }
        row[63] = Value::Bool(true);
        let error = state(&row).expect_err("the direct layout must refuse its 64th bit");
        assert!(error.contains("exceeded 63 bits"));
        return Ok(());
    }
    let sequence = Value::Seq(Arc::from([Value::Int(9)]));
    let function = Value::Func(Arc::new(Func {
        keys: Arc::from([Value::Int(1)]),
        vals: Box::from([Value::Int(9)]),
    }));
    let mut seen = BTreeMap::new();
    for i in 0..32 {
        for j in 0..32 {
            let row = [Value::Int(i), Value::Int(j), sequence.clone()];
            let key = state(&row)?;
            assert_ne!(key, 0);
            assert!(seen.insert(key, (i, j)).is_none());
        }
    }
    // Revisit all rows after columns expanded. Functions with the sequence's
    // integer domain and values are mathematically the same state component.
    for (key, (i, j)) in seen {
        assert_eq!(
            key,
            state(&[Value::Int(i), Value::Int(j), function.clone()])?
        );
    }
    let lazy = Value::Lazy(Arc::new(tlc_rs::value::Lazy::Nat));
    assert!(state(&[Value::Int(0), Value::Int(0), lazy]).is_err());
    Ok(())
}
